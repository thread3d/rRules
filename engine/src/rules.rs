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
        /// Comparison operator applied between the fact's field value and
        /// `value` (D-05). Defaults to `Eq` at construction sites that do
        /// not specify an operator.
        op: ComparisonOp,
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
    /// Evaluates the condition against the supplied fact slice.
    ///
    /// Full `ComparisonOp` coverage (D-05): `Compare` resolves each side via
    /// `to_value` and applies the shared `compare_values` helper across the
    /// Integer/Float/String/Boolean/Null type matrix. `Pattern` performs real
    /// matching — it filters `facts` by `fact_type_id`, reads the named
    /// `field` from each matching fact's payload, and returns true if any
    /// matching fact's field value satisfies `op` against `value`. A `Pattern`
    /// with no matching fact type returns `false` (T-23-04 mitigation — never
    /// returns unconditionally true).
    pub fn evaluate(&self, facts: &[&crate::fact::Fact]) -> bool {
        match self {
            Condition::True => true,
            Condition::False => false,
            Condition::And(conditions) => conditions.iter().all(|c| c.evaluate(facts)),
            Condition::Or(conditions) => conditions.iter().any(|c| c.evaluate(facts)),
            Condition::Not(condition) => !condition.evaluate(facts),
            Condition::Compare { left, op, right } => {
                // Resolve each side to a Value (True/False → Bool; else None)
                // and apply the shared comparison helper. Conditions that do
                // not reduce to a Value (e.g. nested Pattern) compare as
                // false — the Compare variant is a boolean-comparison leaf,
                // not a fact-pattern matcher (Pattern handles that).
                match (left.to_value(), right.to_value()) {
                    (Some(lv), Some(rv)) => compare_values(*op, &lv, &rv),
                    _ => false,
                }
            }
            Condition::Pattern {
                fact_type,
                field,
                value,
                op,
            } => {
                // Real pattern matching (D-05 / T-23-04): scan facts of the
                // requested type, read the field, and return true if any
                // matching fact's field value satisfies `op` against `value`.
                // Returns false when no fact of that type exists or the field
                // is absent (Null compared via compare_values).
                facts
                    .iter()
                    .filter(|f| f.fact_type_id == *fact_type)
                    .any(|f| {
                        f.payload
                            .get(field)
                            .map(|fv| compare_values(*op, fv, value))
                            .unwrap_or(false)
                    })
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
                //
                // Rete-only — FEEL variants handled in engine/src/dmn/feel/
                // (30-RESEARCH.md Pitfall 1). The numeric Rete path only
                // produces Integer/Float; FEEL values (Number/Date/Time/…)
                // fall through the `as_float()` guard (DynamicCast returns
                // None for them) and yield `Value::Null` here — correct for
                // the Rete path, which never legitimately sees FEEL values.
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
                // Rete-only — FEEL variants handled in engine/src/dmn/feel/
                // (30-RESEARCH.md Pitfall 1). `as_string()` (DynamicCast)
                // returns `None` for every FEEL variant (see data_types.rs
                // is_compatible_with), so FEEL operands fall through to
                // `Value::Null` without needing a Value-level wildcard here —
                // this wildcard is on `Option<&str>`, not on `Value` directly.
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
///
/// Rete-only — FEEL variants handled in engine/src/dmn/feel/
/// (30-RESEARCH.md Pitfall 1). The Rete numeric path only produces
/// Integer/Float; FEEL values (`Number`/`Date`/`Time`/…) fall through the
/// `as_float()` guard (DynamicCast returns `None` for every FEEL variant) and
/// yield `Value::Null`. The FEEL numeric path lives in
/// `engine/src/dmn/feel/numeric.rs` (Plan 02) and never routes through here.
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
            // Rete-only — FEEL variants handled in engine/src/dmn/feel/
            // (30-RESEARCH.md Pitfall 1). The `_ => {}` here lets non-Integer
            // operands (including FEEL variants) fall through to the
            // float/string/boolean fallback paths below; FEEL variants return
            // `false` from those fallbacks (as_float/as_string return None for
            // them). The FEEL comparison path is Phase 31 DMN-13.
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

    mod condition {
        //! Tests for the rewritten `Condition::evaluate` (D-05) — full
        //! `ComparisonOp` matrix + real `Pattern` matching. Filter:
        //! `cargo test -p engine --lib rules::tests::condition`
        use super::*;
        use crate::fact::Fact;
        use std::collections::HashMap;

        /// Build a scorecard fact with the given integer `score` payload.
        fn scorecard_fact(score: i64) -> Fact {
            let mut payload = HashMap::new();
            payload.insert("score".to_string(), Value::Integer(score));
            Fact::new("scorecard", payload)
        }

        // ---- Compare: Integer matrix ----

        #[test]
        fn pattern_eq_integer_match() {
            // Pattern { scorecard, score, value: 10, op: Eq } against score=10 → true.
            let fact = scorecard_fact(10);
            let facts: [&Fact; 1] = [&fact];
            let cond = Condition::Pattern {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
                value: Value::Integer(10),
                op: ComparisonOp::Eq,
            };
            assert!(cond.evaluate(&facts));
        }

        #[test]
        fn compare_integer_ne() {
            // 10 Ne 5 → true; 10 Ne 10 → false (modeled as direct Compare on True-leaf
            // is awkward; instead test compare_values directly via Pattern matching.)
            // Here we verify Pattern with op Ne against scorecard facts.
            let cond = Condition::Pattern {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
                value: Value::Integer(5),
                op: ComparisonOp::Ne,
            };
            let fact = scorecard_fact(10);
            let facts: [&Fact; 1] = [&fact];
            assert!(cond.evaluate(&facts), "10 Ne 5 should be true");
        }

        #[test]
        fn compare_integer_lt_le_gt_ge() {
            // Use Pattern with each op against a scorecard score=10.
            let fact = scorecard_fact(10);
            let facts: [&Fact; 1] = [&fact];

            let lt_cond = Condition::Pattern {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
                value: Value::Integer(20),
                op: ComparisonOp::Lt,
            };
            assert!(lt_cond.evaluate(&facts), "10 Lt 20 should be true");

            let le_cond = Condition::Pattern {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
                value: Value::Integer(10),
                op: ComparisonOp::Le,
            };
            assert!(le_cond.evaluate(&facts), "10 Le 10 should be true");

            let gt_cond = Condition::Pattern {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
                value: Value::Integer(5),
                op: ComparisonOp::Gt,
            };
            assert!(gt_cond.evaluate(&facts), "10 Gt 5 should be true");

            let ge_cond = Condition::Pattern {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
                value: Value::Integer(10),
                op: ComparisonOp::Ge,
            };
            assert!(ge_cond.evaluate(&facts), "10 Ge 10 should be true");
        }

        #[test]
        fn compare_integer_false_cases() {
            let fact = scorecard_fact(10);
            let facts: [&Fact; 1] = [&fact];

            let not_lt = Condition::Pattern {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
                value: Value::Integer(5),
                op: ComparisonOp::Lt,
            };
            assert!(!not_lt.evaluate(&facts), "10 Lt 5 should be false");

            let not_gt = Condition::Pattern {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
                value: Value::Integer(20),
                op: ComparisonOp::Gt,
            };
            assert!(!not_gt.evaluate(&facts), "10 Gt 20 should be false");
        }

        // ---- Compare: Float + cross-type promotion ----

        #[test]
        fn compare_float_lt() {
            // 1.5 Lt 2.5 → true. Use Pattern with Float value.
            let mut payload = HashMap::new();
            payload.insert("rate".to_string(), Value::Float(1.5));
            let fact = Fact::new("metric", payload);
            let facts: [&Fact; 1] = [&fact];

            let cond = Condition::Pattern {
                fact_type: "metric".to_string(),
                field: "rate".to_string(),
                value: Value::Float(2.5),
                op: ComparisonOp::Lt,
            };
            assert!(cond.evaluate(&facts), "1.5 Lt 2.5 should be true");
        }

        #[test]
        fn compare_cross_type_integer_lt_float() {
            // Integer field (10) Lt Float value (10.5) → true via promotion.
            let fact = scorecard_fact(10);
            let facts: [&Fact; 1] = [&fact];

            let cond = Condition::Pattern {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
                value: Value::Float(10.5),
                op: ComparisonOp::Lt,
            };
            assert!(cond.evaluate(&facts), "10 Lt 10.5 should be true (cross-type)");
        }

        // ---- Compare: String ----

        #[test]
        fn compare_string_eq_ne() {
            let mut payload = HashMap::new();
            payload.insert("status".to_string(), Value::String("premium".to_string()));
            let fact = Fact::new("customer", payload);
            let facts: [&Fact; 1] = [&fact];

            let eq_cond = Condition::Pattern {
                fact_type: "customer".to_string(),
                field: "status".to_string(),
                value: Value::String("premium".to_string()),
                op: ComparisonOp::Eq,
            };
            assert!(eq_cond.evaluate(&facts), "premium Eq premium → true");

            let ne_cond = Condition::Pattern {
                fact_type: "customer".to_string(),
                field: "status".to_string(),
                value: Value::String("basic".to_string()),
                op: ComparisonOp::Ne,
            };
            assert!(ne_cond.evaluate(&facts), "premium Ne basic → true");
        }

        #[test]
        fn compare_string_lexicographic() {
            let mut payload = HashMap::new();
            payload.insert("name".to_string(), Value::String("apple".to_string()));
            let fact = Fact::new("item", payload);
            let facts: [&Fact; 1] = [&fact];

            let lt_cond = Condition::Pattern {
                fact_type: "item".to_string(),
                field: "name".to_string(),
                value: Value::String("banana".to_string()),
                op: ComparisonOp::Lt,
            };
            assert!(lt_cond.evaluate(&facts), "apple Lt banana → true");

            let gt_cond = Condition::Pattern {
                fact_type: "item".to_string(),
                field: "name".to_string(),
                value: Value::String("almond".to_string()),
                op: ComparisonOp::Gt,
            };
            assert!(gt_cond.evaluate(&facts), "apple Gt almond → true");
        }

        // ---- Compare: Boolean ----

        #[test]
        fn compare_boolean_eq_ne() {
            let mut payload = HashMap::new();
            payload.insert("active".to_string(), Value::Bool(true));
            let fact = Fact::new("account", payload);
            let facts: [&Fact; 1] = [&fact];

            let eq_cond = Condition::Pattern {
                fact_type: "account".to_string(),
                field: "active".to_string(),
                value: Value::Bool(true),
                op: ComparisonOp::Eq,
            };
            assert!(eq_cond.evaluate(&facts), "true Eq true → true");

            let ne_cond = Condition::Pattern {
                fact_type: "account".to_string(),
                field: "active".to_string(),
                value: Value::Bool(false),
                op: ComparisonOp::Ne,
            };
            assert!(ne_cond.evaluate(&facts), "true Ne false → true");
        }

        // ---- Pattern: real matching ----

        #[test]
        fn pattern_match_score_zero_eq_yields_true() {
            // Pattern { scorecard, score, value: 0, op: Eq } against score=0 → true
            let fact = scorecard_fact(0);
            let facts: [&Fact; 1] = [&fact];
            let cond = Condition::Pattern {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
                value: Value::Integer(0),
                op: ComparisonOp::Eq,
            };
            assert!(cond.evaluate(&facts));
        }

        #[test]
        fn pattern_match_score_ten_against_value_zero_yields_false() {
            // Pattern { scorecard, score, value: 0, op: Eq } against score=10 → false
            let fact = scorecard_fact(10);
            let facts: [&Fact; 1] = [&fact];
            let cond = Condition::Pattern {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
                value: Value::Integer(0),
                op: ComparisonOp::Eq,
            };
            assert!(!cond.evaluate(&facts), "score=10 Eq 0 should be false");
        }

        #[test]
        fn pattern_no_matching_fact_type_yields_false() {
            // No fact of type "nonexistent" → false (not unconditionally true).
            let fact = scorecard_fact(0);
            let facts: [&Fact; 1] = [&fact];
            let cond = Condition::Pattern {
                fact_type: "nonexistent".to_string(),
                field: "score".to_string(),
                value: Value::Integer(0),
                op: ComparisonOp::Eq,
            };
            assert!(!cond.evaluate(&facts), "no matching fact_type should be false");
        }

        #[test]
        fn pattern_missing_field_yields_false() {
            // Fact of correct type but missing the field → false (Null Eq 0 is false).
            let fact = scorecard_fact(0);
            let facts: [&Fact; 1] = [&fact];
            let cond = Condition::Pattern {
                fact_type: "scorecard".to_string(),
                field: "absent".to_string(),
                value: Value::Integer(0),
                op: ComparisonOp::Eq,
            };
            assert!(!cond.evaluate(&facts), "missing field should be false");
        }
    }
}
