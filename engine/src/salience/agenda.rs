//! Rule agenda management with priority-based firing order.
//!
//! This module implements the Agenda struct that maintains activations (pending rule firings)
//! and returns them in salience-based priority order.

use crate::rules::Rule;
use std::time::SystemTime;

/// A single rule activation on the agenda.
#[derive(Debug, Clone)]
pub struct Activation {
    /// The rule to fire
    pub rule: Rule,
    /// Salience/priority for ordering (higher = first)
    pub salience: i32,
    /// Timestamp for tie-breaking (earlier = first when equal salience)
    pub timestamp: SystemTime,
    /// Facts that matched to trigger this activation
    pub fact_matches: Vec<String>,
}

impl Activation {
    /// Creates a new activation.
    pub fn new(rule: Rule, salience: i32) -> Self {
        Self {
            rule,
            salience,
            timestamp: SystemTime::now(),
            fact_matches: Vec::new(),
        }
    }

    /// Creates a new activation with specific facts matched.
    pub fn with_matches(mut self, facts: Vec<String>) -> Self {
        self.fact_matches = facts;
        self
    }
}

/// Agenda for managing rule activations with salience-based priority.
///
/// Uses a sorted VecDeque internally to maintain priority order.
#[derive(Debug, Default)]
pub struct Agenda {
    /// Activations stored in sorted order (highest salience first)
    activations: Vec<Activation>,
}

impl Agenda {
    /// Creates a new empty agenda.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an activation to the agenda with proper priority ordering.
    pub fn push(&mut self, activation: Activation) {
        // Insert in sorted order based on salience (descending), timestamp (ascending for ties)
        let pos = self.activations.iter().position(|a| {
            (activation.salience > a.salience)
                || (activation.salience == a.salience && activation.timestamp < a.timestamp)
        });

        match pos {
            Some(p) => self.activations.insert(p, activation),
            None => self.activations.push(activation),
        }
    }

    /// Removes and returns the highest priority activation, or None if empty.
    pub fn pop(&mut self) -> Option<Activation> {
        // Pop from front (highest priority in our sorted order)
        if self.activations.is_empty() {
            None
        } else {
            Some(self.activations.remove(0))
        }
    }

    /// Returns a reference to the highest priority activation without removing it.
    pub fn peek(&self) -> Option<&Activation> {
        self.activations.first()
    }

    /// Returns true if the agenda is empty.
    pub fn is_empty(&self) -> bool {
        self.activations.is_empty()
    }

    /// Returns the number of activations in the agenda.
    pub fn len(&self) -> usize {
        self.activations.len()
    }

    /// Removes a specific activation by rule ID (used for no-loop enforcement).
    pub fn remove_activation(&mut self, rule_id: &str) -> bool {
        let original_len = self.activations.len();
        self.activations.retain(|a| a.rule.id != rule_id);
        self.activations.len() < original_len
    }

    /// Returns all activations in priority order (highest first, for debugging).
    pub fn all_activations(&self) -> Vec<&Activation> {
        self.activations.iter().collect()
    }

    /// Clears all activations from the agenda.
    pub fn clear(&mut self) {
        self.activations.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RuleBuilder;
    use crate::salience::conflict_resolution::{ConflictResolver, ConflictStrategy};

    #[test]
    fn test_agenda_priority_ordering() {
        let mut agenda = Agenda::new();

        // Create activations with different saliences
        let rule_low = RuleBuilder::new("low", "Low Priority")
            .salience(-50)
            .build();
        let rule_high = RuleBuilder::new("high", "High Priority")
            .salience(100)
            .build();
        let rule_med = RuleBuilder::new("med", "Medium Priority")
            .salience(50)
            .build();

        agenda.push(Activation::new(rule_low.clone(), -50));
        agenda.push(Activation::new(rule_high.clone(), 100));
        agenda.push(Activation::new(rule_med.clone(), 50));

        // Should pop in salience order (highest first)
        assert_eq!(agenda.pop().unwrap().rule.id, "high");
        assert_eq!(agenda.pop().unwrap().rule.id, "med");
        assert_eq!(agenda.pop().unwrap().rule.id, "low");
        assert!(agenda.is_empty());
    }

    #[test]
    fn test_agenda_tie_breaking() {
        let mut agenda = Agenda::new();

        // Create rules with same salience
        let rule_a = RuleBuilder::new("r1", "Alpha").salience(10).build();
        let rule_b = RuleBuilder::new("r2", "Beta").salience(10).build();
        let rule_c = RuleBuilder::new("r3", "Gamma").salience(10).build();

        agenda.push(Activation::new(rule_a.clone(), 10));
        std::thread::sleep(std::time::Duration::from_micros(10));
        agenda.push(Activation::new(rule_b.clone(), 10));
        std::thread::sleep(std::time::Duration::from_micros(10));
        agenda.push(Activation::new(rule_c.clone(), 10));

        // With same salience, earlier timestamp should come first (alpha, beta, gamma)
        assert_eq!(agenda.pop().unwrap().rule.id, "r1");
        assert_eq!(agenda.pop().unwrap().rule.id, "r2");
        assert_eq!(agenda.pop().unwrap().rule.id, "r3");
    }

    #[test]
    fn test_agenda_remove_activation() {
        let mut agenda = Agenda::new();

        let rule1 = RuleBuilder::new("r1", "Rule 1").salience(10).build();
        let rule2 = RuleBuilder::new("r2", "Rule 2").salience(20).build();

        agenda.push(Activation::new(rule1.clone(), 10));
        agenda.push(Activation::new(rule2.clone(), 20));

        assert_eq!(agenda.len(), 2);

        // Remove rule1
        assert!(agenda.remove_activation("r1"));
        assert_eq!(agenda.len(), 1);
        assert!(!agenda.remove_activation("nonexistent"));

        // Only rule2 should remain (highest priority)
        let activation = agenda.pop().unwrap();
        assert_eq!(activation.rule.id, "r2");
    }

    #[test]
    fn test_conflict_resolver_strategies() {
        let activations = vec![
            Activation::new(RuleBuilder::new("a", "Alpha").salience(10).build(), 10)
                .with_matches(vec!["m1".to_string()]),
            Activation::new(RuleBuilder::new("b", "Beta").salience(10).build(), 10)
                .with_matches(vec!["m1".to_string(), "m2".to_string(), "m3".to_string()]),
            Activation::new(RuleBuilder::new("c", "Gamma").salience(10).build(), 10)
                .with_matches(vec!["m1".to_string(), "m2".to_string()]),
        ];

        // Depth first: fewer matches first
        let resolver = ConflictResolver::new(ConflictStrategy::DepthFirst);
        let sorted = resolver.sort(&activations);
        assert_eq!(sorted[0].rule.id, "a"); // 1 match
        assert_eq!(sorted[1].rule.id, "c"); // 2 matches
        assert_eq!(sorted[2].rule.id, "b"); // 3 matches

        // Breadth first: more matches first
        let resolver = ConflictResolver::new(ConflictStrategy::BreadthFirst);
        let sorted = resolver.sort(&activations);
        assert_eq!(sorted[0].rule.id, "b");
        assert_eq!(sorted[1].rule.id, "c");
        assert_eq!(sorted[2].rule.id, "a");

        // Lexicographic: alphabetical
        let resolver = ConflictResolver::new(ConflictStrategy::Lexicographic);
        let sorted = resolver.sort(&activations);
        assert_eq!(sorted[0].rule.id, "a"); // Alpha
        assert_eq!(sorted[1].rule.id, "b"); // Beta
        assert_eq!(sorted[2].rule.id, "c"); // Gamma
    }
}
