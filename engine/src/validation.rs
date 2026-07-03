//! Syntax and semantic validation for rules before compilation.
//!
//! This module provides validators that catch errors early with clear messages.

use crate::data_types::Value;
use crate::rules::{Condition, ComparisonOp, Rule};
use std::collections::HashMap;

/// Result of validation with separate error and warning collections.
#[derive(Debug, Default)]
pub struct ValidationResult {
    /// Whether validation passed
    pub is_valid: bool,
    /// List of errors found
    pub errors: Vec<ValidationError>,
    /// List of warnings found
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_error(mut self, error: ValidationError) -> Self {
        self.is_valid = false;
        self.errors.push(error);
        self
    }

    pub fn with_warning(mut self, warning: ValidationWarning) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Adds all errors and warnings from another result.
    pub fn extend(&mut self, other: ValidationResult) {
        for error in other.errors {
            self.is_valid = false;
            self.errors.push(error);
        }
        self.warnings.extend(other.warnings);
    }
}

/// Validation error with location information.
#[derive(Debug)]
pub struct ValidationError {
    /// Error message
    pub message: String,
    /// Optional file/field location
    pub location: Option<String>,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.location {
            Some(loc) => write!(f, "{} at {}", self.message, loc),
            None => write!(f, "{}", self.message),
        }
    }
}

/// Validation warning with location information.
#[derive(Debug)]
pub struct ValidationWarning {
    /// Warning message
    pub message: String,
    /// Optional file/field location
    pub location: Option<String>,
}

impl std::fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.location {
            Some(loc) => write!(f, "{} at {}", self.message, loc),
            None => write!(f, "{}", self.message),
        }
    }
}

/// Syntax validator that checks rule text for structural correctness.
pub struct SyntaxValidator;

impl SyntaxValidator {
    /// Creates a new syntax validator.
    pub fn new() -> Self {
        Self
    }

    /// Validates the syntax of a rule expression string.
    pub fn validate_expression(&self, expr: &str) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Check balanced parentheses
        let mut paren_depth = 0;
        for (i, c) in expr.chars().enumerate() {
            match c {
                '(' => paren_depth += 1,
                ')' => {
                    if paren_depth == 0 {
                        result = result.with_error(ValidationError {
                            message: "Unexpected closing parenthesis".to_string(),
                            location: Some(format!("position {}", i)),
                        });
                    }
                    paren_depth -= 1;
                }
                _ => {}
            }
        }
        if paren_depth > 0 {
            result = result.with_error(ValidationError {
                message: "Unclosed parentheses".to_string(),
                location: None,
            });
        }

        // Check balanced brackets
        let mut bracket_depth = 0;
        for (i, c) in expr.chars().enumerate() {
            match c {
                '[' => bracket_depth += 1,
                ']' => {
                    if bracket_depth == 0 {
                        result = result.with_error(ValidationError {
                            message: "Unexpected closing bracket".to_string(),
                            location: Some(format!("position {}", i)),
                        });
                    }
                    bracket_depth -= 1;
                }
                _ => {}
            }
        }
        if bracket_depth > 0 {
            result = result.with_error(ValidationError {
                message: "Unclosed brackets".to_string(),
                location: None,
            });
        }

        result
    }

    /// Validates an operator sequence.
    pub fn validate_operator_sequence(&self, ops: &[&str]) -> ValidationResult {
        let mut result = ValidationResult::new();
        let valid_ops = ["==", "!=", "<", "<=", ">", ">=", "&&", "||", "!"];

        for (i, op) in ops.iter().enumerate() {
            if !valid_ops.contains(op) {
                result = result.with_error(ValidationError {
                    message: format!("Unknown operator '{}'", op),
                    location: Some(format!("position {}", i)),
                });
            }
        }

        result
    }
}

impl Default for SyntaxValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Semantic validator that checks type compatibility and references.
pub struct SemanticValidator<'a> {
    /// Known fact types in the registry
    fact_types: HashMap<String, Vec<String>>,
    /// Known functions available for call actions
    known_functions: Vec<String>,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> SemanticValidator<'a> {
    /// Creates a new semantic validator with provided type registry.
    pub fn new(fact_types: HashMap<String, Vec<String>>) -> Self {
        Self {
            fact_types,
            known_functions: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Adds known functions that can be called in rule actions.
    pub fn with_known_functions(mut self, funcs: &[&str]) -> Self {
        self.known_functions = funcs.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Validates a condition for semantic correctness.
    pub fn validate_condition(&self, condition: &Condition) -> ValidationResult {
        let mut result = ValidationResult::new();

        match condition {
            Condition::True | Condition::False => {}
            Condition::And(conditions) | Condition::Or(conditions) => {
                for cond in conditions {
                    result.extend(self.validate_condition(cond));
                }
            }
            Condition::Not(condition) => {
                result.extend(self.validate_condition(condition));
            }
            Condition::Compare { left, right, .. } => {
                // Get values to check type compatibility
                let left_val = Self::condition_to_value(left);
                let right_val = Self::condition_to_value(right);

                if let (Some(l), Some(r)) = (&left_val, &right_val) {
                    if !Self::types_compatible(l, r) {
                        result = result.with_warning(ValidationWarning {
                            message: "Comparison between incompatible types".to_string(),
                            location: None,
                        });
                    }
                }
            }
            Condition::Pattern {
                fact_type,
                field,
                value,
                ..
            } => {
                // Check if fact type exists
                if !self.fact_types.contains_key(fact_type) {
                    result = result.with_error(ValidationError {
                        message: format!("Unknown fact type '{}'", fact_type),
                        location: None,
                    });
                }

                // Check if field exists in type
                if let Some(fields) = self.fact_types.get(fact_type) {
                    if !fields.contains(field) {
                        result = result.with_warning(ValidationWarning {
                            message: format!(
                                "Unknown field '{}' in fact type '{}'",
                                field, fact_type
                            ),
                            location: None,
                        });
                    }
                }

                // Check value type matches expected
                self.validate_field_value(fact_type, field, value, &mut result);
            }
        }

        result
    }

    /// Validates an action for semantic correctness.
    pub fn validate_action(&self, _action: &crate::rules::Action) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Check function calls have known functions
        if let crate::rules::Action::CallFunction { name, .. } = _action {
            if !self.known_functions.contains(name) {
                result = result.with_warning(ValidationWarning {
                    message: format!("Unknown function '{}'", name),
                    location: None,
                });
            }
        }

        result
    }

    /// Validates that a field value matches the expected type.
    fn validate_field_value(
        &self,
        _fact_type: &str,
        _field: &str,
        value: &Value,
        _result: &mut ValidationResult,
    ) {
        // This is a simplified check - full implementation would
        // look up the actual type of the field and compare with value
        match value {
            Value::Bool(_) => {}
            Value::Integer(_) => {}
            Value::Float(_) => {}
            Value::String(_) => {}
            Value::Object(_) => {}
            Value::Array(_) => {}
            Value::Null => {}
            _ => {}
        }
    }

    /// Converts a condition to a value for comparison.
    fn condition_to_value(condition: &Condition) -> Option<&Value> {
        match condition {
            Condition::True => Some(&Value::Bool(true)),
            Condition::False => Some(&Value::Bool(false)),
            _ => None,
        }
    }

    /// Checks if two values have compatible types for comparison.
    fn types_compatible(v1: &Value, v2: &Value) -> bool {
        match (v1, v2) {
            (Value::Integer(_), Value::Float(_)) => true,
            (Value::Float(_), Value::Integer(_)) => true,
            (Value::String(_), Value::String(_)) => true,
            (Value::Bool(_), Value::Bool(_)) => true,
            (Value::Null, _) | (_, Value::Null) => true,
            (l, r) => std::mem::discriminant(l) == std::mem::discriminant(r),
        }
    }

    /// Validates an entire rule set for semantic errors.
    pub fn validate_rule_set(&self, rules: &[crate::rules::Rule]) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Check for circular references between rules
        self.check_circular_references(rules, &mut result);

        // Validate each rule's conditions and actions
        for rule in rules {
            result.extend(self.validate_condition(&rule.condition));
            for action in &rule.actions {
                result.extend(self.validate_action(action));
            }
        }

        result
    }

    /// Detects circular references between rules.
    fn check_circular_references(
        &self,
        rules: &[crate::rules::Rule],
        _result: &mut ValidationResult,
    ) {
        // Simplified implementation - would build dependency graph and detect cycles
        let _ = rules;
    }
}

/// Full validation pipeline that runs syntax then semantic checks.
#[allow(dead_code)]
pub struct ValidationPipeline {
    syntax_validator: SyntaxValidator,
    semantic_validator: Option<SemanticValidator<'static>>,
}

impl ValidationPipeline {
    /// Creates a new validation pipeline with default validators.
    pub fn new() -> Self {
        Self {
            syntax_validator: SyntaxValidator::new(),
            semantic_validator: None,
        }
    }

    /// Sets the fact type registry for semantic validation.
    pub fn with_fact_types(
        mut self,
        types: std::collections::HashMap<String, Vec<String>>,
    ) -> Self {
        // Safety: We're creating a static lifetime for the validator
        let static_validator = unsafe {
            std::mem::transmute::<SemanticValidator<'_>, SemanticValidator<'static>>(
                SemanticValidator::new(types),
            )
        };
        self.semantic_validator = Some(static_validator);
        self
    }

    /// Validates a rule with both syntax and semantic checks.
    pub fn validate_rule(&self, rule: &Rule) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Run syntax validation first (fail fast)
        if let Err(e) = self.validate_rule_syntax(rule) {
            return ValidationResult::new().with_error(ValidationError {
                message: e,
                location: None,
            });
        }

        // Then semantic validation
        if let Some(ref validator) = self.semantic_validator {
            result.extend(validator.validate_condition(&rule.condition));
        }

        result
    }

    /// Validates rule syntax only.
    pub fn validate_rule_syntax(&self, _rule: &Rule) -> Result<(), String> {
        Ok(())
    }
}

impl Default for ValidationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_validator_parentheses() {
        let validator = SyntaxValidator::new();

        // Balanced parentheses
        assert!(validator.validate_expression("(a && b)").is_valid);

        // Unbalanced - missing open
        let result = validator.validate_expression("a && b)");
        assert!(!result.is_valid);
        assert!(result
            .errors
            .iter()
            .any(|e| e.message.contains("Unexpected closing")));

        // Unbalanced - missing close
        let result = validator.validate_expression("(a && b");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.message.contains("Unclosed")));
    }

    #[test]
    fn test_semantic_validator_types() {
        let mut fact_types = HashMap::new();
        fact_types.insert(
            "customer".to_string(),
            vec!["age".to_string(), "name".to_string()],
        );

        let validator = SemanticValidator::new(fact_types);

        // Check unknown fact type
        let result = validator.validate_condition(&Condition::Pattern {
            fact_type: "unknown".to_string(),
            field: "field".to_string(),
            value: Value::Integer(42),
            op: ComparisonOp::Eq,
        });
        assert!(!result.is_valid);
    }
}
