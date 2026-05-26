//! Forward chaining execution with Rete algorithm integration.
//!
//! This module implements the execute_forward function for rule evaluation.

use crate::environment::{Engine, Session};
use crate::fact::Fact;
use crate::rules::Rule;

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
pub enum ConflictResolver {
    /// Higher salience fires first (default)
    Salience,
    /// Depth-first traversal of rule network
    DepthFirst,
    /// Breadth-first traversal
    BreadthFirst,
    /// Random order when salience is equal
    Random,
}

impl Default for ConflictResolver {
    fn default() -> Self {
        ConflictResolver::Salience
    }
}

/// Agenda for managing activations (pending rule firings).
pub struct Agenda {
    /// Pending activations sorted by priority
    activations: Vec<Activation>,
    /// Conflict resolver strategy
    resolver: ConflictResolver,
}

impl Agenda {
    pub fn new() -> Self {
        Self {
            activations: Vec::new(),
            resolver: ConflictResolver::default(),
        }
    }

    pub fn add_activation(&mut self, activation: Activation) {
        self.activations.push(activation);
        // Sort by priority based on resolver strategy
        self.sort();
    }

    pub fn pop(&mut self) -> Option<Activation> {
        self.activations.pop()
    }

    pub fn is_empty(&self) -> bool {
        self.activations.is_empty()
    }

    fn sort(&mut self) {
        // Sort by salience (descending - higher first)
        self.activations
            .sort_by(|a, b| b.salience.cmp(&a.salience));
    }
}

/// A single rule activation on the agenda.
pub struct Activation {
    /// The rule to fire
    pub rule: Rule,
    /// Salience/priority for ordering
    pub salience: i32,
    /// Fact matches that triggered this activation
    pub matches: Vec<String>,
}

impl Activation {
    pub fn new(rule: Rule, salience: i32) -> Self {
        Self {
            rule,
            salience,
            matches: Vec::new(),
        }
    }
}

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

        agenda.add_activation(Activation::new(rule_high, 100));
        agenda.add_activation(Activation::new(rule_low, -50));
        agenda.add_activation(Activation::new(rule_med, 50));

        // Should pop in priority order
        assert_eq!(agenda.pop().unwrap().salience, 100);
        assert_eq!(agenda.pop().unwrap().salience, 50);
        assert_eq!(agenda.pop().unwrap().salience, -50);
        assert!(agenda.is_empty());
    }
}
