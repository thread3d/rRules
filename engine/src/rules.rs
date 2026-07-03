//! Rule definition, parsing, and compilation structures.
//!
//! This module implements the Rule AST with conditions and actions,
//! along with RuleSet for managing collections of rules.

use crate::data_types::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// A rule with a condition (LHS) and actions (RHS).
///
/// Note: We use PartialEq but not Eq because Condition contains Value which
/// has Float(f64) that doesn't implement Eq.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    /// Unique identifier for the rule
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Salience/priority (-1000 to 1000, higher fires first)
    pub salience: i32,
    /// Prevent this rule from firing again if it just fired
    pub no_loop: bool,
    /// Prevent other rules with same agenda group from firing when this one fires
    pub lock_on_active: bool,
    /// Condition that must be true for the rule to fire
    pub condition: Condition,
    /// Actions to execute when condition is met
    pub actions: Vec<Action>,
}

impl Rule {
    /// Creates a new rule with default settings.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            salience: 0,
            no_loop: false,
            lock_on_active: false,
            condition: Condition::True,
            actions: Vec::new(),
        }
    }

    /// Validates the rule's syntax and semantics.
    pub fn validate(&self) -> Result<(), RuleValidationError> {
        if self.id.is_empty() {
            return Err(RuleValidationError::MissingId);
        }
        if self.name.is_empty() {
            return Err(RuleValidationError::MissingName);
        }
        // More validation can be added here
        Ok(())
    }

    /// Creates a builder for configuring a rule.
    pub fn builder(id: impl Into<String>, name: impl Into<String>) -> RuleBuilder {
        RuleBuilder::new(id, name)
    }
}

/// Builder for constructing rules with fluent API.
pub struct RuleBuilder {
    rule: Rule,
}

impl RuleBuilder {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            rule: Rule::new(id, name),
        }
    }

    pub fn salience(mut self, value: i32) -> Self {
        self.rule.salience = value;
        self
    }

    pub fn no_loop(mut self, value: bool) -> Self {
        self.rule.no_loop = value;
        self
    }

    pub fn lock_on_active(mut self, value: bool) -> Self {
        self.rule.lock_on_active = value;
        self
    }

    pub fn condition(mut self, condition: Condition) -> Self {
        self.rule.condition = condition;
        self
    }

    pub fn action(mut self, action: Action) -> Self {
        self.rule.actions.push(action);
        self
    }

    pub fn actions(mut self, actions: Vec<Action>) -> Self {
        self.rule.actions = actions;
        self
    }

    pub fn build(self) -> Rule {
        self.rule
    }
}

/// Condition AST for rule LHS expressions.
///
/// Note: We use PartialEq but not Eq because Value contains Float(f64) which
/// doesn't implement Eq (due to NaN comparisons).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// Always true condition
    True,
    /// Always false condition
    False,
    /// AND combination of conditions
    And(Vec<Condition>),
    /// OR combination of conditions
    Or(Vec<Condition>),
    /// NOT of a condition
    Not(Box<Condition>),
    /// Comparison between two values
    Compare {
        left: Box<Condition>,
        op: ComparisonOp,
        right: Box<Condition>,
    },
    /// Pattern match against facts
    Pattern {
        fact_type: String,
        field: String,
        value: Value,
    },
}

/// Comparison operators for conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOp {
    Eq, // ==
    Ne, // !=
    Lt, // <
    Le, // <=
    Gt, // >
    Ge, // >=
}

impl Condition {
    /// Evaluates the condition against facts.
    pub fn evaluate(&self, _facts: &[&crate::fact::Fact]) -> bool {
        match self {
            Condition::True => true,
            Condition::False => false,
            Condition::And(conditions) => conditions.iter().all(|c| c.evaluate(_facts)),
            Condition::Or(conditions) => conditions.iter().any(|c| c.evaluate(_facts)),
            Condition::Not(condition) => !condition.evaluate(_facts),
            Condition::Compare { left, op, right } => {
                // Simplified evaluation - full implementation would use working memory
                let left_val = left.to_value();
                let right_val = right.to_value();

                match (left_val, right_val, op) {
                    (Some(Value::Integer(l)), Some(Value::Integer(r)), ComparisonOp::Eq) => l == r,
                    (Some(Value::Float(l)), Some(Value::Float(r)), ComparisonOp::Eq) => {
                        (l - r).abs() < f64::EPSILON
                    }
                    (Some(Value::String(l)), Some(Value::String(r)), ComparisonOp::Eq) => l == r,
                    _ => false,
                }
            }
            Condition::Pattern { .. } => {
                // Full pattern matching implemented in Rete integration
                true
            }
        }
    }

    /// Converts condition to a value (for comparison evaluation).
    fn to_value(&self) -> Option<Value> {
        match self {
            Condition::True => Some(Value::Bool(true)),
            Condition::False => Some(Value::Bool(false)),
            _ => None,
        }
    }
}

/// Expression AST for computed action values (D-06).
///
/// Models the right-hand side of `score := score + 10` — a structured
/// expression language layered on top of the existing `Value` / `ComparisonOp`
/// primitives. `Expression::evaluate` resolves `FieldRef`s against the
/// supplied fact slice and computes arithmetic / string / comparison results
/// without panicking on ill-typed operands or division by zero.
///
/// Note: We use PartialEq but not Eq because `Value` contains `Float(f64)`
/// which doesn't implement `Eq` (due to NaN comparisons).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    /// A literal value leaf.
    Literal(Value),
    /// Read a field from the first fact of `fact_type` in working memory.
    FieldRef { fact_type: String, field: String },
    /// `left + right` (numeric promotion; Integer+Integer stays Integer).
    Add(Box<Expression>, Box<Expression>),
    /// `left - right`.
    Sub(Box<Expression>, Box<Expression>),
    /// `left * right`.
    Mul(Box<Expression>, Box<Expression>),
    /// `left / right` (division by zero yields `Value::Null`).
    Div(Box<Expression>, Box<Expression>),
    /// String concatenation: `left` + `right` via `as_string`.
    StringConcat(Box<Expression>, Box<Expression>),
    /// Comparison: applies `op` to the two evaluated operands → `Value::Bool`.
    Compare {
        op: ComparisonOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    /// Call a registered function by name. Stub for Phase 23 — returns
    /// `Value::Null`; full function dispatch is out of scope (the variant
    /// exists so the AST is complete).
    Call {
        name: String,
        args: Vec<Expression>,
    },
}

impl Expression {
    /// Evaluates the expression against the supplied fact slice.
    ///
    /// `FieldRef` scans `facts` for the first fact whose `fact_type_id ==
    /// fact_type` and reads `fact.payload.get(field)` (or `Value::Null` if
    /// absent). Arithmetic variants use the `DynamicCast` helpers so
    /// Integer-vs-Float promotion is correct; division by zero yields
    /// `Value::Null` rather than panicking (T-23-03 mitigation). `Compare`
    /// delegates to the shared `compare_values` helper.
    pub fn evaluate(&self, facts: &[&crate::fact::Fact]) -> Value {
        use crate::data_types::DynamicCast;
        match self {
            Expression::Literal(v) => v.clone(),
            Expression::FieldRef { fact_type, field } => {
                // Scan for the first fact of the requested type, read the field.
                facts
                    .iter()
                    .find(|f| f.fact_type_id == *fact_type)
                    .and_then(|f| f.payload.get(field))
                    .cloned()
                    .unwrap_or(Value::Null)
            }
            Expression::Add(l, r) => {
                let lv = l.evaluate(facts);
                let rv = r.evaluate(facts);
                numeric_op(&lv, &rv, |a, b| a + b, |a, b| a + b)
            }
            Expression::Sub(l, r) => {
                let lv = l.evaluate(facts);
                let rv = r.evaluate(facts);
                numeric_op(&lv, &rv, |a, b| a - b, |a, b| a - b)
            }
            Expression::Mul(l, r) => {
                let lv = l.evaluate(facts);
                let rv = r.evaluate(facts);
                numeric_op(&lv, &rv, |a, b| a * b, |a, b| a * b)
            }
            Expression::Div(l, r) => {
                let lv = l.evaluate(facts);
                let rv = r.evaluate(facts);
                // Division by zero yields Null (T-23-03 mitigation — do not panic).
                match (&lv, &rv) {
                    (Value::Integer(a), Value::Integer(b)) if *b != 0 => Value::Integer(a / b),
                    _ if lv.as_float().is_some() && rv.as_float().is_some() => {
                        let a = lv.as_float().unwrap();
                        let b = rv.as_float().unwrap();
                        if b == 0.0 || b.is_nan() {
                            Value::Null
                        } else {
                            Value::Float(a / b)
                        }
                    }
                    _ => Value::Null,
                }
            }
            Expression::StringConcat(l, r) => {
                let lv = l.evaluate(facts);
                let rv = r.evaluate(facts);
                match (lv.as_string(), rv.as_string()) {
                    (Some(ls), Some(rs)) => Value::String(format!("{}{}", ls, rs)),
                    _ => Value::Null,
                }
            }
            Expression::Compare { op, left, right } => {
                let lv = left.evaluate(facts);
                let rv = right.evaluate(facts);
                Value::Bool(compare_values(*op, &lv, &rv))
            }
            Expression::Call { .. } => {
                // Stub for Phase 23 — full function dispatch is out of scope.
                Value::Null
            }
        }
    }
}

/// Apply a binary numeric op to two `Value`s with Integer-preserving
/// promotion: `Integer + Integer` stays `Integer`; any `Float` operand
/// promotes both sides to `f64`. Non-numeric operands yield `Value::Null`.
fn numeric_op(
    left: &Value,
    right: &Value,
    int_op: impl Fn(i64, i64) -> i64,
    float_op: impl Fn(f64, f64) -> f64,
) -> Value {
    use crate::data_types::DynamicCast;
    match (left, right) {
        (Value::Integer(l), Value::Integer(r)) => Value::Integer(int_op(*l, *r)),
        _ if left.as_float().is_some() && right.as_float().is_some() => {
            Value::Float(float_op(left.as_float().unwrap(), right.as_float().unwrap()))
        }
        _ => Value::Null,
    }
}

/// Shared comparison helper used by both `Expression::Compare` (D-06) and
/// `Condition::evaluate` (D-05). Applies `op` to `left` / `right` across the
/// full `Value` type matrix without panicking on ill-typed operands.
///
/// Semantics:
/// - `Eq` / `Ne`: structural equality (`PartialEq`) — handles Integer, Float,
///   String, Boolean, Null, Array, Object.
/// - `Lt`/`Le`/`Gt`/`Ge` on numerics: `Integer`+`Integer` compare as `i64`;
///   any `Float` operand promotes both to `f64` via `as_float`.
/// - Ordered ops on `String`: lexicographic via `as_string`.
/// - Ordered ops on `Boolean` / `Null` / `Array` / `Object`: return `false`
///   (T-23-03 mitigation — never panic).
pub fn compare_values(op: ComparisonOp, left: &Value, right: &Value) -> bool {
    use crate::data_types::DynamicCast;
    match op {
        ComparisonOp::Eq => left == right,
        ComparisonOp::Ne => left != right,
        ComparisonOp::Lt | ComparisonOp::Le | ComparisonOp::Gt | ComparisonOp::Ge => {
            // Integer + Integer: stay integral (avoid f64 precision loss).
            match (left, right) {
                (Value::Integer(l), Value::Integer(r)) => return match op {
                    ComparisonOp::Lt => l < r,
                    ComparisonOp::Le => l <= r,
                    ComparisonOp::Gt => l > r,
                    ComparisonOp::Ge => l >= r,
                    _ => unreachable!(),
                },
                _ => {}
            }
            // Any Float operand: promote both to f64.
            if left.as_float().is_some() && right.as_float().is_some() {
                let l = left.as_float().unwrap();
                let r = right.as_float().unwrap();
                return match op {
                    ComparisonOp::Lt => l < r,
                    ComparisonOp::Le => l <= r,
                    ComparisonOp::Gt => l > r,
                    ComparisonOp::Ge => l >= r,
                    _ => unreachable!(),
                };
            }
            // String: lexicographic.
            if let (Some(l), Some(r)) = (left.as_string(), right.as_string()) {
                return match op {
                    ComparisonOp::Lt => l < r,
                    ComparisonOp::Le => l <= r,
                    ComparisonOp::Gt => l > r,
                    ComparisonOp::Ge => l >= r,
                    _ => unreachable!(),
                };
            }
            // Boolean / Null / Array / Object: ordered ops return false (no panic).
            false
        }
    }
}

/// Action for rule RHS execution.
///
/// Note: We use PartialEq but not Eq because Value contains Float(f64) which
/// doesn't implement Eq.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
    /// Insert a new fact into working memory
    InsertFact {
        fact_type: String,
        fields: HashMap<String, Value>,
    },
    /// Update an existing fact with computed expressions (D-06).
    /// Each field value is an `Expression` evaluated against the working
    /// facts at action-application time, so `score := score + 10` is
    /// representable.
    UpdateFact {
        id: String,
        fields: HashMap<String, Expression>,
    },
    /// Remove a fact from working memory
    RetractFact { id: String },
    /// Call a registered function
    CallFunction { name: String, args: Vec<Value> },
    /// Log a message for debugging
    LogMessage { level: LogLevel, message: String },
}

/// Log levels for debug messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// Container for managing a collection of rules with compilation support.
#[derive(Debug, Clone)]
pub struct RuleSet {
    /// Rules in the set
    pub rules: Vec<Rule>,
    /// Compiled representation (placeholder for KIE base)
    compiled: bool,
}

impl RuleSet {
    /// Creates an empty rule set.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            compiled: false,
        }
    }

    /// Adds a rule to the set.
    pub fn add_rule(&mut self, rule: Rule) -> Result<(), RuleSetError> {
        // Validate the rule before adding
        rule.validate()?;

        // Check for duplicate IDs
        if self.rules.iter().any(|r| r.id == rule.id) {
            return Err(RuleSetError::DuplicateRuleId(rule.id));
        }

        self.rules.push(rule);
        Ok(())
    }

    /// Adds multiple rules at once.
    pub fn add_rules(&mut self, rules: Vec<Rule>) -> Result<(), RuleSetError> {
        for rule in rules {
            self.add_rule(rule)?;
        }
        Ok(())
    }

    /// Validates all rules in the set.
    pub fn validate_all(&self) -> Result<Vec<String>, RuleValidationResult> {
        let mut errors = Vec::new();
        let warnings = Vec::new();

        for rule in &self.rules {
            if let Err(e) = rule.validate() {
                errors.push(format!("Rule '{}': {}", rule.id, e));
            }
        }

        if !errors.is_empty() {
            Err(RuleValidationResult { errors, warnings })
        } else {
            Ok(warnings)
        }
    }

    /// Compiles the rule set to a KIE base representation.
    pub fn compile(&mut self) -> Result<(), RuleSetError> {
        // Validate before compilation
        self.validate_all()?;

        // Here we would build the Rete network and compile rules
        self.compiled = true;
        Ok(())
    }

    /// Checks if the rule set has been compiled.
    pub fn is_compiled(&self) -> bool {
        self.compiled
    }
}

impl Default for RuleSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation error with categorized errors and warnings.
#[derive(Debug, Error)]
pub struct RuleValidationResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl std::fmt::Display for RuleValidationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Rule validation failed with {} errors and {} warnings",
            self.errors.len(),
            self.warnings.len()
        )?;
        for error in &self.errors {
            writeln!(f, "  Error: {}", error)?;
        }
        Ok(())
    }
}

/// Errors during rule set operations.
#[derive(Debug, Error)]
pub enum RuleSetError {
    #[error("Duplicate rule ID: {0}")]
    DuplicateRuleId(String),
    #[error("Rule not found: {0}")]
    RuleNotFound(String),
    #[error("Compilation error: {0}")]
    Compilation(String),
    #[error(transparent)]
    Validation(RuleValidationError),
}

impl From<RuleValidationError> for RuleSetError {
    fn from(err: RuleValidationError) -> Self {
        RuleSetError::Validation(err)
    }
}

impl From<RuleValidationResult> for RuleSetError {
    fn from(result: RuleValidationResult) -> Self {
        RuleSetError::Compilation(format!(
            "{} errors and {} warnings",
            result.errors.len(),
            result.warnings.len()
        ))
    }
}

/// Errors during rule validation.
#[derive(Debug, Error)]
pub enum RuleValidationError {
    #[error("Missing required field: id")]
    MissingId,
    #[error("Missing required field: name")]
    MissingName,
    #[error("Invalid salience value: must be between -1000 and 1000")]
    InvalidSalience,
    #[error("Validation failed: {0}")]
    Field(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_creation() {
        let rule = Rule::new("rule-1", "Test Rule");
        assert_eq!(rule.id, "rule-1");
        assert_eq!(rule.name, "Test Rule");
        assert_eq!(rule.salience, 0);
    }

    #[test]
    fn test_rule_builder() {
        let rule = Rule::builder("r1", "My Rule")
            .salience(10)
            .no_loop(true)
            .build();

        assert_eq!(rule.id, "r1");
        assert_eq!(rule.salience, 10);
        assert!(rule.no_loop);
    }

    #[test]
    fn test_rule_set_operations() {
        let mut set = RuleSet::new();

        let rule1 = Rule::new("r1", "Rule 1");
        let rule2 = Rule::new("r2", "Rule 2");

        set.add_rule(rule1).unwrap();
        set.add_rule(rule2).unwrap();

        assert_eq!(set.rules.len(), 2);
    }

    #[test]
    fn test_duplicate_rule_error() {
        let mut set = RuleSet::new();
        let rule = Rule::new("r1", "Rule");

        set.add_rule(rule).unwrap();

        let duplicate = Rule::new("r1", "Duplicate");
        assert!(matches!(
            set.add_rule(duplicate),
            Err(RuleSetError::DuplicateRuleId(_))
        ));
    }

    #[test]
    fn test_rule_set_add_multiple_rules() {
        let mut set = RuleSet::new();

        let rules: Vec<Rule> = (0..10)
            .map(|i| Rule::new(format!("r{}", i), format!("Rule {}", i)))
            .collect();

        set.add_rules(rules).unwrap();
        assert_eq!(set.rules.len(), 10);
    }

    #[test]
    fn test_rule_set_validation_empty() {
        let set = RuleSet::new();
        // Empty rule set should validate successfully
        assert!(set.validate_all().is_ok());
    }

    #[test]
    fn test_rule_builder_default_values() {
        let rule = Rule::builder("test", "Test Rule").build();

        assert_eq!(rule.salience, 0);
        assert!(!rule.no_loop);
        assert!(!rule.lock_on_active);
    }

    #[test]
    fn test_rule_all_config_options() {
        let rule = Rule::builder("test", "Test Rule")
            .salience(100)
            .no_loop(true)
            .lock_on_active(true)
            .build();

        assert_eq!(rule.salience, 100);
        assert!(rule.no_loop);
        assert!(rule.lock_on_active);
    }

    #[test]
    fn test_condition_evaluate_true_false() {
        let true_cond = Condition::True;
        let false_cond = Condition::False;

        // Facts are not used in True/False evaluation, pass empty slice
        assert!(true_cond.evaluate(&[]));
        assert!(!false_cond.evaluate(&[]));
    }

    #[test]
    fn test_condition_evaluate_and_or() {
        // Use cloned values to avoid moved value issues since Condition doesn't implement Copy
        let true_cond = Condition::True;
        let false_cond = Condition::False;

        // Test AND with all true conditions
        let and_true = Condition::And(vec![true_cond.clone(), true_cond.clone()]);
        assert!(and_true.evaluate(&[]));

        // Test OR with at least one true condition
        let or_with_true = Condition::Or(vec![false_cond, true_cond]);
        assert!(or_with_true.evaluate(&[]));
    }

    #[test]
    fn test_condition_evaluate_not() {
        // For Not, we use different values to avoid moved value issues
        let not_true = Condition::Not(Box::new(Condition::True));

        assert!(!not_true.evaluate(&[]));
    }

    mod expression {
        //! Tests for the `Expression` evaluator (D-06). Filter:
        //! `cargo test -p engine --lib rules::tests::expression`
        use super::*;
        use crate::fact::Fact;
        use std::collections::HashMap;

        /// Build a scorecard fact with the given integer `score` payload.
        fn scorecard_fact(score: i64) -> Fact {
            let mut payload = HashMap::new();
            payload.insert("score".to_string(), Value::Integer(score));
            Fact::new("scorecard", payload)
        }

        #[test]
        fn literal_yields_literal_value() {
            let expr = Expression::Literal(Value::Integer(10));
            assert_eq!(expr.evaluate(&[]), Value::Integer(10));
        }

        #[test]
        fn fieldref_reads_fact_payload() {
            let fact = scorecard_fact(0);
            let facts: [&Fact; 1] = [&fact];
            let expr = Expression::FieldRef {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
            };
            assert_eq!(expr.evaluate(&facts), Value::Integer(0));
        }

        #[test]
        fn fieldref_missing_fact_type_yields_null() {
            let fact = scorecard_fact(0);
            let facts: [&Fact; 1] = [&fact];
            let expr = Expression::FieldRef {
                fact_type: "nonexistent".to_string(),
                field: "score".to_string(),
            };
            assert_eq!(expr.evaluate(&facts), Value::Null);
        }

        #[test]
        fn fieldref_missing_field_yields_null() {
            let fact = scorecard_fact(0);
            let facts: [&Fact; 1] = [&fact];
            let expr = Expression::FieldRef {
                fact_type: "scorecard".to_string(),
                field: "absent".to_string(),
            };
            assert_eq!(expr.evaluate(&facts), Value::Null);
        }

        #[test]
        fn add_fieldref_plus_literal() {
            // score := score + 10 against score=0 → Integer(10)
            let fact = scorecard_fact(0);
            let facts: [&Fact; 1] = [&fact];
            let expr = Expression::Add(
                Box::new(Expression::FieldRef {
                    fact_type: "scorecard".to_string(),
                    field: "score".to_string(),
                }),
                Box::new(Expression::Literal(Value::Integer(10))),
            );
            assert_eq!(expr.evaluate(&facts), Value::Integer(10));
        }

        #[test]
        fn add_integer_plus_integer_stays_integer() {
            let expr = Expression::Add(
                Box::new(Expression::Literal(Value::Integer(3))),
                Box::new(Expression::Literal(Value::Integer(4))),
            );
            assert_eq!(expr.evaluate(&[]), Value::Integer(7));
        }

        #[test]
        fn add_integer_plus_float_promotes_to_float() {
            let expr = Expression::Add(
                Box::new(Expression::Literal(Value::Integer(3))),
                Box::new(Expression::Literal(Value::Float(0.5))),
            );
            assert_eq!(expr.evaluate(&[]), Value::Float(3.5));
        }

        #[test]
        fn sub_mul_basic() {
            assert_eq!(
                Expression::Sub(
                    Box::new(Expression::Literal(Value::Integer(10))),
                    Box::new(Expression::Literal(Value::Integer(4))),
                )
                .evaluate(&[]),
                Value::Integer(6),
            );
            assert_eq!(
                Expression::Mul(
                    Box::new(Expression::Literal(Value::Integer(3))),
                    Box::new(Expression::Literal(Value::Integer(7))),
                )
                .evaluate(&[]),
                Value::Integer(21),
            );
        }

        #[test]
        fn div_basic() {
            assert_eq!(
                Expression::Div(
                    Box::new(Expression::Literal(Value::Integer(20))),
                    Box::new(Expression::Literal(Value::Integer(4))),
                )
                .evaluate(&[]),
                Value::Integer(5),
            );
        }

        #[test]
        fn div_by_zero_yields_null_not_panic() {
            // T-23-03 mitigation: division by zero must not panic.
            let result = Expression::Div(
                Box::new(Expression::Literal(Value::Integer(10))),
                Box::new(Expression::Literal(Value::Integer(0))),
            )
            .evaluate(&[]);
            assert_eq!(result, Value::Null);
        }

        #[test]
        fn string_concat() {
            let expr = Expression::StringConcat(
                Box::new(Expression::Literal(Value::String("a".to_string()))),
                Box::new(Expression::Literal(Value::String("b".to_string()))),
            );
            assert_eq!(expr.evaluate(&[]), Value::String("ab".to_string()));
        }

        #[test]
        fn compare_gt_yields_bool() {
            // Gt(score, 5) against score=0 → false; against score=10 → true
            let expr = Expression::Compare {
                op: ComparisonOp::Gt,
                left: Box::new(Expression::FieldRef {
                    fact_type: "scorecard".to_string(),
                    field: "score".to_string(),
                }),
                right: Box::new(Expression::Literal(Value::Integer(5))),
            };
            let fact_low = scorecard_fact(0);
            let facts_low: [&Fact; 1] = [&fact_low];
            assert_eq!(expr.evaluate(&facts_low), Value::Bool(false));

            let fact_high = scorecard_fact(10);
            let facts_high: [&Fact; 1] = [&fact_high];
            assert_eq!(expr.evaluate(&facts_high), Value::Bool(true));
        }

        #[test]
        fn call_stub_returns_null() {
            // Call is a stub for Phase 23 — returns Null (function dispatch out of scope).
            let expr = Expression::Call {
                name: "abs".to_string(),
                args: vec![Expression::Literal(Value::Integer(-1))],
            };
            assert_eq!(expr.evaluate(&[]), Value::Null);
        }

        #[test]
        fn updatefact_with_expression_serializes_roundtrip() {
            // Action::UpdateFact fields now carry Expressions; verify serde round-trip.
            let mut fields = HashMap::new();
            fields.insert(
                "score".to_string(),
                Expression::Add(
                    Box::new(Expression::FieldRef {
                        fact_type: "scorecard".to_string(),
                        field: "score".to_string(),
                    }),
                    Box::new(Expression::Literal(Value::Integer(10))),
                ),
            );
            let action = Action::UpdateFact {
                id: "f1".to_string(),
                fields,
            };
            let json = serde_json::to_string(&action).expect("serialize");
            let parsed: Action = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, action);
        }
    }
}
