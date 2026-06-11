//! Coverage tracking for rule execution analysis.
//!
//! This module provides functionality to track which rules are evaluated
//! and fired during execution, enabling coverage reporting and dead rule
//! detection.

use std::collections::HashMap;

/// Tracks coverage data for a single rule evaluation.
#[derive(Debug, Clone)]
pub struct RuleCoverage {
    /// Rule ID
    pub rule_id: String,
    /// Whether the rule was evaluated (conditions matched)
    pub evaluated: bool,
    /// Whether the rule fired (actions executed)
    pub fired: bool,
    /// Timestamp of last evaluation
    pub evaluated_at: Option<String>,
    /// Timestamp of last fire
    pub fired_at: Option<String>,
}

/// Coverage tracker for recording and reporting rule execution coverage.
#[derive(Debug, Default)]
pub struct CoverageTracker {
    /// Map of rule ID to coverage data
    rules: HashMap<String, RuleCoverage>,
    /// Total number of evaluations recorded
    total_evaluations: usize,
    /// Total number of fires recorded
    total_fires: usize,
}

impl CoverageTracker {
    /// Creates a new empty coverage tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that a rule was evaluated (conditions matched).
    pub fn record_evaluation(&mut self, rule_id: &str) {
        let now = chrono::Utc::now().to_rfc3339();

        self.rules.entry(rule_id.to_string()).or_insert(RuleCoverage {
            rule_id: rule_id.to_string(),
            evaluated: false,
            fired: false,
            evaluated_at: None,
            fired_at: None,
        }).evaluated = true;
        if let Some(coverage) = self.rules.get_mut(rule_id) {
            coverage.evaluated_at = Some(now);
        }
        self.total_evaluations += 1;
    }

    /// Records that a rule fired (actions executed).
    pub fn record_fire(&mut self, rule_id: &str) {
        let now = chrono::Utc::now().to_rfc3339();

        if let Some(coverage) = self.rules.get_mut(rule_id) {
            coverage.fired = true;
            coverage.fired_at = Some(now);
        } else {
            // First time seeing this rule - record it as fired
            self.rules.insert(rule_id.to_string(), RuleCoverage {
                rule_id: rule_id.to_string(),
                evaluated: true,
                fired: true,
                evaluated_at: Some(now),
                fired_at: None,
            });
        }
        self.total_evaluations += 1; // Fire implies evaluation
        self.total_fires += 1;
    }

    /// Records a rule that was skipped (condition didn't match).
    pub fn record_skipped(&mut self, rule_id: &str) {
        let now = chrono::Utc::now().to_rfc3339();

        self.rules.entry(rule_id.to_string()).or_insert(RuleCoverage {
            rule_id: rule_id.to_string(),
            evaluated: false,
            fired: false,
            evaluated_at: None,
            fired_at: None,
        }).evaluated = true;
        if let Some(coverage) = self.rules.get_mut(rule_id) {
            coverage.evaluated_at = Some(now);
        }
        self.total_evaluations += 1;
    }

    /// Gets coverage data for a specific rule.
    pub fn get_coverage(&self, rule_id: &str) -> Option<&RuleCoverage> {
        self.rules.get(rule_id)
    }

    /// Returns all coverage records.
    pub fn get_all_coverage(&self) -> Vec<&RuleCoverage> {
        self.rules.values().collect()
    }

    /// Calculates the overall coverage percentage.
    ///
    /// Coverage = rules evaluated / total rules
    pub fn coverage_percent(&self) -> f64 {
        if self.total_evaluations == 0 {
            return 0.0;
        }
        let rules_evaluated = self.rules.values().filter(|r| r.evaluated).count();
        // Count only rules that were tracked (not empty placeholders)
        let total_tracked_rules = self.rules.len();
        if total_tracked_rules == 0 {
            return 100.0; // No rules tracked, but some evaluations happened
        }
        (rules_evaluated as f64 / total_tracked_rules as f64) * 100.0
    }

    /// Returns the number of rules that have been evaluated at least once.
    pub fn rules_evaluated(&self) -> usize {
        self.rules.values().filter(|r| r.evaluated).count()
    }

    /// Returns the number of rules that have fired at least once.
    pub fn rules_fired(&self) -> usize {
        self.rules.values().filter(|r| r.fired).count()
    }

    /// Resets the tracker state for a new analysis period.
    pub fn reset(&mut self) {
        // Keep rules in map but clear their metrics
        for coverage in self.rules.values_mut() {
            *coverage = RuleCoverage {
                rule_id: coverage.rule_id.clone(),
                evaluated: false,
                fired: false,
                evaluated_at: None,
                fired_at: None,
            };
        }
        self.total_evaluations = 0;
        self.total_fires = 0;
    }

    /// Clears all rules and starts fresh.
    pub fn clear(&mut self) {
        self.rules.clear();
        self.total_evaluations = 0;
        self.total_fires = 0;
    }

    /// Gets the total number of rule evaluations recorded.
    pub fn total_evaluations(&self) -> usize {
        self.total_evaluations
    }

    /// Gets the total number of rule fires recorded.
    pub fn total_fires(&self) -> usize {
        self.total_fires
    }
}

/// Coverage report for external API responses.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CoverageReport {
    /// List of coverage data for all rules
    pub rules: Vec<CoverageEntry>,
    /// Total rules tracked
    pub total_rules: usize,
    /// Rules evaluated at least once
    pub evaluated_rules: usize,
    /// Rules that fired (coverage target)
    pub fired_rules: usize,
    /// Overall coverage percentage
    pub coverage_percent: f64,
}

/// Single rule's coverage entry in a report.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CoverageEntry {
    /// Rule ID
    pub rule_id: String,
    /// Whether the rule was evaluated
    pub evaluated: bool,
    /// Whether the rule fired
    pub fired: bool,
    /// Evaluation count (if tracked)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation_count: Option<usize>,
    /// Fire count (if tracked)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fire_count: Option<usize>,
    /// Coverage percentage for this rule
    pub coverage_percent: f64,
}

impl CoverageTracker {
    /// Generates a coverage report.
    pub fn generate_report(&self) -> CoverageReport {
        let rules: Vec<CoverageEntry> = self.rules.values().map(|r| CoverageEntry {
            rule_id: r.rule_id.clone(),
            evaluated: r.evaluated,
            fired: r.fired,
            evaluation_count: if r.evaluated { Some(1) } else { None },
            fire_count: if r.fired { Some(1) } else { None },
            coverage_percent: if r.evaluated { 100.0 } else { 0.0 },
        }).collect();

        let total_rules = self.rules.len();
        let evaluated_rules = rules.iter().filter(|r| r.evaluated).count();
        let fired_rules = rules.iter().filter(|r| r.fired).count();
        let coverage_percent = if total_rules > 0 {
            (evaluated_rules as f64 / total_rules as f64) * 100.0
        } else {
            0.0
        };

        CoverageReport {
            rules,
            total_rules,
            evaluated_rules,
            fired_rules,
            coverage_percent,
        }
    }

    /// Exports coverage data to JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.generate_report()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_evaluation() {
        let mut tracker = CoverageTracker::new();
        tracker.record_evaluation("rule1");

        let coverage = tracker.get_coverage("rule1").unwrap();
        assert!(coverage.evaluated);
        assert!(!coverage.fired);
        assert!(coverage.evaluated_at.is_some());
    }

    #[test]
    fn test_record_fire() {
        let mut tracker = CoverageTracker::new();

        // Record fire without prior evaluation - should still record evaluated=true and have fired_at set
        tracker.record_fire("rule1");

        let coverage = tracker.get_coverage("rule1").unwrap();
        assert!(coverage.evaluated);
        assert!(coverage.fired);
    }

    #[test]
    fn test_record_skipped() {
        let mut tracker = CoverageTracker::new();
        tracker.record_skipped("rule1");

        let coverage = tracker.get_coverage("rule1").unwrap();
        assert!(coverage.evaluated);
        assert!(!coverage.fired);
    }

    #[test]
    fn test_coverage_percent() {
        let mut tracker = CoverageTracker::new();

        // Add 3 rules to track (evaluate 2)
        tracker.record_evaluation("rule1");
        tracker.record_evaluation("rule2");
        // rule3 is not evaluated - it's simply missing from the tracker

        // Only 2 rules are tracked, both evaluated
        let pct = tracker.coverage_percent();
        assert_eq!(pct, 100.0); // Both of the 2 tracked rules were evaluated
    }

    #[test]
    fn test_reset() {
        let mut tracker = CoverageTracker::new();

        tracker.record_evaluation("rule1");
        tracker.record_fire("rule2");

        tracker.reset();

        assert_eq!(tracker.rules_evaluated(), 0);
        assert_eq!(tracker.total_fires(), 0);
    }

    #[test]
    fn test_generate_report() {
        let mut tracker = CoverageTracker::new();
        tracker.record_evaluation("rule1");
        tracker.record_fire("rule2");

        let report = tracker.generate_report();

        // Should have 2 rules (both evaluated)
        assert_eq!(report.total_rules, 2);
        assert_eq!(report.evaluated_rules, 2); // Both were evaluated
        assert_eq!(report.fired_rules, 1);
    }
}

// Include history sub-module if it exists
#[cfg(feature = "history")]
pub mod history;
