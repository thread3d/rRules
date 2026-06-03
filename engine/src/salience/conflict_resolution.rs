//! Conflict resolution strategies for ordering rule firings.
//!
//! This module provides the ConflictResolver that implements multiple ordering strategies
//! for determining which activation should fire next when multiple rules are activated.

use crate::salience::agenda::Activation;

/// Conflict resolution strategy for ordering rule activations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConflictStrategy {
    /// Higher salience fires first (default)
    Salience,
    /// Fewer matched facts first (depth-first)
    DepthFirst,
    /// More matched facts first (breadth-first)
    BreadthFirst,
    /// Most recent activation wins
    Recency,
    /// Alphabetical rule name for determinism
    Lexicographic,
}

impl Default for ConflictStrategy {
    fn default() -> Self {
        ConflictStrategy::Salience
    }
}

/// Resolves conflicts between activated rules using a specified strategy.
#[derive(Debug, Clone)]
pub struct ConflictResolver {
    strategy: ConflictStrategy,
}

impl ConflictResolver {
    /// Creates a new conflict resolver with the specified strategy.
    pub fn new(strategy: ConflictStrategy) -> Self {
        Self { strategy }
    }

    /// Sorts activations according to the configured strategy.
    ///
    /// Returns a new sorted vector without modifying the original.
    pub fn sort<'a>(&self, activations: &'a [Activation]) -> Vec<&'a Activation> {
        let mut result: Vec<_> = activations.iter().collect();

        match self.strategy {
            ConflictStrategy::Salience => {
                // Higher salience first; earlier timestamp for ties
                result.sort_by(|a, b| {
                    b.salience
                        .cmp(&a.salience)
                        .then_with(|| a.timestamp.cmp(&b.timestamp))
                });
            }
            ConflictStrategy::DepthFirst => {
                // Fewer matched facts (deeper in inference tree) first
                result.sort_by_key(|a| a.fact_matches.len());
            }
            ConflictStrategy::BreadthFirst => {
                // More matched facts first (wider propagation)
                result.sort_by_key(|a| std::cmp::Reverse(a.fact_matches.len()));
            }
            ConflictStrategy::Recency => {
                // Most recent activation wins
                result.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            }
            ConflictStrategy::Lexicographic => {
                // Alphabetical rule name for determinism
                result.sort_by(|a, b| a.rule.name.cmp(&b.rule.name));
            }
        }

        result
    }

    /// Sorts activations in-place using the configured strategy.
    pub fn sort_in_place(&self, activations: &mut [Activation]) {
        match self.strategy {
            ConflictStrategy::Salience => {
                activations.sort_by(|a, b| {
                    b.salience
                        .cmp(&a.salience)
                        .then_with(|| a.timestamp.cmp(&b.timestamp))
                });
            }
            ConflictStrategy::DepthFirst => {
                activations.sort_by_key(|a| a.fact_matches.len());
            }
            ConflictStrategy::BreadthFirst => {
                activations.sort_by_key(|a| std::cmp::Reverse(a.fact_matches.len()));
            }
            ConflictStrategy::Recency => {
                activations.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            }
            ConflictStrategy::Lexicographic => {
                activations.sort_by(|a, b| a.rule.name.cmp(&b.rule.name));
            }
        }
    }

    /// Returns the name of the current strategy.
    pub fn strategy_name(&self) -> &'static str {
        match self.strategy {
            ConflictStrategy::Salience => "salience",
            ConflictStrategy::DepthFirst => "depth-first",
            ConflictStrategy::BreadthFirst => "breadth-first",
            ConflictStrategy::Recency => "recency",
            ConflictStrategy::Lexicographic => "lexicographic",
        }
    }
}

impl Default for ConflictResolver {
    fn default() -> Self {
        Self::new(ConflictStrategy::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RuleBuilder;

    #[test]
    fn test_salience_strategy() {
        let resolver = ConflictResolver::new(ConflictStrategy::Salience);

        let activations = vec![
            Activation::new(RuleBuilder::new("low", "Low").salience(-10).build(), -10),
            Activation::new(RuleBuilder::new("high", "High").salience(50).build(), 50),
            Activation::new(RuleBuilder::new("med", "Medium").salience(25).build(), 25),
        ];

        let sorted = resolver.sort(&activations);
        assert_eq!(sorted[0].rule.id, "high");
        assert_eq!(sorted[1].rule.id, "med");
        assert_eq!(sorted[2].rule.id, "low");
    }

    #[test]
    fn test_depth_first_strategy() {
        let resolver = ConflictResolver::new(ConflictStrategy::DepthFirst);

        let activations = vec![
            Activation::new(RuleBuilder::new("a", "A").salience(10).build(), 10)
                .with_matches(vec!["f1".to_string(), "f2".to_string()]),
            Activation::new(RuleBuilder::new("b", "B").salience(10).build(), 10)
                .with_matches(vec!["f1".to_string()]),
        ];

        let sorted = resolver.sort(&activations);
        // Depth-first: fewer matches first
        assert_eq!(sorted[0].rule.id, "b"); // 1 match
        assert_eq!(sorted[1].rule.id, "a"); // 2 matches
    }

    #[test]
    fn test_breadth_first_strategy() {
        let resolver = ConflictResolver::new(ConflictStrategy::BreadthFirst);

        let activations = vec![
            Activation::new(RuleBuilder::new("a", "A").salience(10).build(), 10)
                .with_matches(vec!["f1".to_string(), "f2".to_string()]),
            Activation::new(RuleBuilder::new("b", "B").salience(10).build(), 10)
                .with_matches(vec!["f1".to_string()]),
        ];

        let sorted = resolver.sort(&activations);
        // Breadth-first: more matches first
        assert_eq!(sorted[0].rule.id, "a"); // 2 matches
        assert_eq!(sorted[1].rule.id, "b"); // 1 match
    }

    #[test]
    fn test_lexicographic_strategy() {
        let resolver = ConflictResolver::new(ConflictStrategy::Lexicographic);

        let activations = vec![
            Activation::new(RuleBuilder::new("zebra", "Zebra").salience(10).build(), 10),
            Activation::new(RuleBuilder::new("alpha", "Alpha").salience(10).build(), 10),
            Activation::new(RuleBuilder::new("beta", "Beta").salience(10).build(), 10),
        ];

        let sorted = resolver.sort(&activations);
        assert_eq!(sorted[0].rule.id, "alpha");
        assert_eq!(sorted[1].rule.id, "beta");
        assert_eq!(sorted[2].rule.id, "zebra");
    }

    #[test]
    fn test_strategy_names() {
        assert_eq!(
            ConflictResolver::new(ConflictStrategy::Salience).strategy_name(),
            "salience"
        );
        assert_eq!(
            ConflictResolver::new(ConflictStrategy::DepthFirst).strategy_name(),
            "depth-first"
        );
        assert_eq!(
            ConflictResolver::new(ConflictStrategy::BreadthFirst).strategy_name(),
            "breadth-first"
        );
        assert_eq!(
            ConflictResolver::new(ConflictStrategy::Recency).strategy_name(),
            "recency"
        );
        assert_eq!(
            ConflictResolver::new(ConflictStrategy::Lexicographic).strategy_name(),
            "lexicographic"
        );
    }
}
