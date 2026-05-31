//! Forward chaining execution with Rete algorithm integration.
//!
//! This module implements the execute_forward function for rule evaluation.

use crate::environment::{Engine, Session};
use crate::fact::Fact;
use crate::rules::Rule;
use crate::salience::agenda::{Activation, Agenda};

/// Result of a forward chaining execution.
pub struct ExecutionResult {
    /// Number of rules fired
    pub rules_fired: usize,
    /// Facts added during execution
    pub facts_added: Vec<Fact>,
    /// Facts removed during execution
    pub facts_retracted: Vec<String>,
    /// Any errors encountered
    pub errors: Vec<String>,
}

impl ExecutionResult {
    pub fn new() -> Self {
        Self {
            rules_fired: 0,
            facts_added: Vec::new(),
            facts_retracted: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn with_rule_fired(mut self) -> Self {
        self.rules_fired += 1;
        self
    }
}

/// Executes forward chaining using Rete algorithm.
///
/// This is the main entry point for forward rule execution as exported from lib.rs.
pub fn execute_forward(engine: &mut Engine, session: &Session) -> ExecutionResult {
    // Placeholder implementation - actual Rete integration would go here
    let mut result = ExecutionResult::new();

    // This would:
    // 1. Build or reuse the Rete network from compiled rules
    // 2. Propagate facts through alpha/beta memories
    // 3. Trigger rule actions when conditions match
    // 4. Handle salience-based agenda ordering

    result
}

/// Conflict resolution strategy for ordering rule firings.
pub use crate::salience::conflict_resolution::ConflictResolver;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agenda_priority() {
        let mut agenda = Agenda::new();

        // Create rules with different saliences
        let rule_high = Rule::builder("high", "High Priority").salience(100).build();
        let rule_low = Rule::builder("low", "Low Priority").salience(-50).build();
        let rule_med = Rule::builder("med", "Medium Priority").salience(50).build();

        agenda.push(Activation::new(rule_high, 100));
        agenda.push(Activation::new(rule_low, -50));
        agenda.push(Activation::new(rule_med, 50));

        // Should pop in priority order
        assert_eq!(agenda.pop().unwrap().salience, 100);
        assert_eq!(agenda.pop().unwrap().salience, 50);
        assert_eq!(agenda.pop().unwrap().salience, -50);
        assert!(agenda.is_empty());
    }

    #[test]
    fn test_conflict_resolver() {
        let resolver = ConflictResolver::default();

        // Verify the default strategy is Salience
        assert_eq!(resolver.strategy_name(), "salience");
    }
}
