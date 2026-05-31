//! Rule definition, parsing, and compilation structures.
//!
//! This module implements the Rule AST with conditions and actions,
//! along with RuleSet for managing collections of rules.

use crate::data_types::{DynamicCast, TypeInfo, TypeClass, Value};
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
    Eq,      // ==
    Ne,      // !=
    Lt,      // <
    Le,      // <=
    Gt,      // >
    Ge,      // >=
}

impl Condition {
    /// Evaluates the condition against facts.
    pub fn evaluate(&self, _facts: &[&crate::fact::Fact]) -> bool {
        match self {
            Condition::True => true,
            Condition::False => false,
            Condition::And(conditions) => {
                conditions.iter().all(|c| c.evaluate(_facts))
            }
            Condition::Or(conditions) => {
                conditions.iter().any(|c| c.evaluate(_facts))
            }
            Condition::Not(condition) => !condition.evaluate(_facts),
            Condition::Compare { left, op, right } => {
                // Simplified evaluation - full implementation would use working memory
                let left_val = left.to_value();
                let right_val = right.to_value();

                match (left_val, right_val, op) {
                    (Some(Value::Integer(l)), Some(Value::Integer(r)), ComparisonOp::Eq) => l == r,
                    (Some(Value::Float(l)), Some(Value::Float(r)), ComparisonOp::Eq) => (l - r).abs() < f64::EPSILON,
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

/// Action for rule RHS execution.
///
/// Note: We use PartialEq but not Eq because Value contains Float(f64) which
/// doesn't implement Eq.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
    /// Insert a new fact into working memory
    InsertFact { fact_type: String, fields: HashMap<String, Value> },
    /// Update an existing fact
    UpdateFact { id: String, fields: HashMap<String, Value> },
    /// Remove a fact from working memory
    RetractFact { id: String },
    /// Call a registered function
    CallFunction {
        name: String,
        args: Vec<Value>,
    },
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
        let mut warnings = Vec::new();

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
        writeln!(f, "Rule validation failed with {} errors and {} warnings", self.errors.len(), self.warnings.len())?;
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
}
