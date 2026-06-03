//! Automatic chaining mode selection based on rule structure and query type.
//!
//! This module implements ChainingSelector that automatically determines whether
//! forward or backward chaining is optimal for a given set of rules.

use crate::data_types::Value;
use crate::rules::{Condition, Rule};

/// Result of analyzing rule structure for inference pattern classification.
#[derive(Debug, Clone, PartialEq)]
pub enum InferencePattern {
    /// Rules are data-driven, matching existing facts (forward chaining)
    DataDriven,
    /// Rules are query/goal-driven, need to prove something (backward chaining)
    GoalDriven,
    /// Mixed rules that could use both modes
    Mixed,
}

/// Automatic selection of optimal inference mode.
#[derive(Debug, Clone)]
pub struct ChainingSelector {
    /// Enable debug tracing for interactive debugging
    debug_mode: bool,
}

impl ChainingSelector {
    /// Creates a new chaining selector with default settings.
    pub fn new() -> Self {
        Self { debug_mode: false }
    }

    /// Enables or disables debug mode (for interactive debugging).
    pub fn with_debug(mut self, enabled: bool) -> Self {
        self.debug_mode = enabled;
        self
    }

    /// Analyzes rule structure to classify its inference pattern.
    ///
    /// - Data-driven rules have conditions that match existing facts
    /// - Goal-driven rules (queries) start from a goal/fact pattern needing proof
    pub fn analyze_rule_structure(&self, rule: &Rule) -> InferencePattern {
        // Check if the rule has any condition patterns that look like queries
        let has_query_pattern = self.is_query_driven(&rule.condition);

        match has_query_pattern {
            true => InferencePattern::GoalDriven,
            false => InferencePattern::DataDriven,
        }
    }

    /// Recursively checks if a condition is query-driven (needs to prove something).
    fn is_query_driven(&self, condition: &Condition) -> bool {
        match condition {
            Condition::True | Condition::False => false,
            Condition::And(conditions) => conditions.iter().any(|c| self.is_query_driven(c)),
            Condition::Or(conditions) => conditions.iter().any(|c| self.is_query_driven(c)),
            Condition::Not(condition) => self.is_query_driven(condition),
            Condition::Compare { .. } => false,
            Condition::Pattern { fact_type, .. } => {
                // A pattern match on a specific fact type could be either:
                // - Data-driven if the fact type is commonly known
                // - Goal-driven if it's a query-specific type (starts with "query_")
                fact_type.starts_with("query_")
            }
        }
    }

    /// Selects the optimal chaining mode for the given rules and target.
    ///
    /// Returns:
    /// - ForwardOnly if all rules are data-driven and no query provided
    /// - BackwardOnly if a target is provided with no matching facts (query context)
    /// - Automatic if mixed or complex conditions detected
    pub fn select(&self, rules: &[Rule], target_fact_type: Option<&str>) -> ChainingMode {
        // Check for query context first
        if let Some(target) = target_fact_type {
            // If we have a specific goal/fact to prove, use backward chaining
            if target.starts_with("query_") || target.contains("?") {
                return ChainingMode::BackwardOnly;
            }
        }

        // Analyze all rules
        let mut has_data_driven = false;
        let mut has_goal_driven = false;

        for rule in rules {
            match self.analyze_rule_structure(rule) {
                InferencePattern::DataDriven => has_data_driven = true,
                InferencePattern::GoalDriven => has_goal_driven = true,
                InferencePattern::Mixed => {
                    has_data_driven = true;
                    has_goal_driven = true;
                }
            }
        }

        match (has_data_driven, has_goal_driven) {
            (true, false) => ChainingMode::ForwardOnly,
            (false, true) => ChainingMode::BackwardOnly,
            (true, true) | (false, false) => ChainingMode::Automatic,
        }
    }

    /// Returns the chaining mode as a string for debugging.
    pub fn mode_name(&self, mode: &ChainingMode) -> &'static str {
        match mode {
            ChainingMode::ForwardOnly => "forward-only",
            ChainingMode::BackwardOnly => "backward-only",
            ChainingMode::Automatic => "automatic",
        }
    }
}

impl Default for ChainingSelector {
    fn default() -> Self {
        Self::new()
    }
}

/// Chaining mode selection strategy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChainingMode {
    /// Automatic mode selection based on rule structure and query type
    Automatic,
    /// Only use forward chaining
    ForwardOnly,
    /// Only use backward chaining
    BackwardOnly,
}

/// Collector for inference traces during backward chaining execution.
#[derive(Debug, Default)]
pub struct InferenceTraceCollector {
    /// Recorded activations with justifications
    activations: Vec<ActivationWithJustification>,
    /// Facts that triggered each activation
    fact_triggers: Vec<String>,
}

impl InferenceTraceCollector {
    /// Creates a new trace collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an activation with its justification (why it fired).
    pub fn record_activation(
        &mut self,
        rule_id: impl Into<String>,
        rule_name: impl Into<String>,
        facts_matched: Vec<&str>,
        justification: impl Into<String>,
    ) {
        self.activations.push(ActivationWithJustification {
            rule_id: rule_id.into(),
            rule_name: rule_name.into(),
            facts_matched: facts_matched.into_iter().map(|s| s.to_string()).collect(),
            justification: justification.into(),
        });
    }

    /// Adds a fact trigger to the trace.
    pub fn record_fact_trigger(&mut self, fact_info: impl Into<String>) {
        self.fact_triggers.push(fact_info.into());
    }

    /// Returns all recorded activations.
    pub fn activations(&self) -> &[ActivationWithJustification] {
        &self.activations
    }

    /// Returns all fact triggers.
    pub fn fact_triggers(&self) -> &[String] {
        &self.fact_triggers
    }

    /// Clears the collected trace data.
    pub fn clear(&mut self) {
        self.activations.clear();
        self.fact_triggers.clear();
    }
}

/// An activation with its justification (explanation for why it fired).
#[derive(Debug, Clone)]
pub struct ActivationWithJustification {
    /// The rule that was activated
    pub rule_id: String,
    /// Human-readable rule name
    pub rule_name: String,
    /// Facts that matched to trigger this activation
    pub facts_matched: Vec<String>,
    /// Explanation of why this activation occurred
    pub justification: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{ComparisonOp, RuleBuilder};

    #[test]
    fn test_analyze_data_driven_rule() {
        let selector = ChainingSelector::new();
        let rule = RuleBuilder::new("r1", "Rule 1")
            .condition(Condition::Pattern {
                fact_type: "transaction".to_string(),
                field: "amount".to_string(),
                value: Value::Integer(100),
            })
            .build();

        assert_eq!(
            selector.analyze_rule_structure(&rule),
            InferencePattern::DataDriven
        );
    }

    #[test]
    fn test_analyze_goal_driven_rule() {
        let selector = ChainingSelector::new();
        let rule = RuleBuilder::new("r1", "Query")
            .condition(Condition::Pattern {
                fact_type: "query_customer_eligibility".to_string(),
                field: "type".to_string(),
                value: Value::String("premium".to_string()),
            })
            .build();

        assert_eq!(
            selector.analyze_rule_structure(&rule),
            InferencePattern::GoalDriven
        );
    }

    #[test]
    fn test_select_forward_only() {
        let selector = ChainingSelector::new();
        let rules = vec![RuleBuilder::new("r1", "Rule 1")
            .condition(Condition::True)
            .build()];

        assert_eq!(selector.select(&rules, None), ChainingMode::ForwardOnly);
    }

    #[test]
    fn test_select_backward_only_with_query() {
        let selector = ChainingSelector::new();
        let rules = vec![RuleBuilder::new("r1", "Rule 1")
            .condition(Condition::True)
            .build()];

        assert_eq!(
            selector.select(&rules, Some("query_eligibility")),
            ChainingMode::BackwardOnly
        );
    }

    #[test]
    fn test_select_automatic_mixed() {
        let selector = ChainingSelector::new();
        // Create rules that would be classified differently
        let rule1 = RuleBuilder::new("r1", "Data driven")
            .condition(Condition::Pattern {
                fact_type: "transaction".to_string(),
                field: "amount".to_string(),
                value: Value::Integer(100),
            })
            .build();

        assert_eq!(selector.select(&[rule1], None), ChainingMode::ForwardOnly);
    }

    #[test]
    fn test_trace_collector() {
        let mut collector = InferenceTraceCollector::new();
        collector.record_activation(
            "r1",
            "High Value Customer",
            vec!["customer-1", "customer-2"],
            "Both facts matched condition",
        );
        collector.record_fact_trigger("fact-id-1: type=premium");

        assert_eq!(collector.activations().len(), 1);
        assert_eq!(collector.fact_triggers().len(), 1);
    }
}
