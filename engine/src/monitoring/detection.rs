//! Dead rule detection for identifying rules that never fire.
//!
//! This module provides functionality to analyze rule execution data
//! and identify rules that were defined but never executed during
//! a monitoring period.

use std::collections::HashMap;

/// Represents the state of a rule regarding its execution activity.
#[derive(Debug, Clone, PartialEq)]
pub enum RuleActivity {
    /// Rule has fired at least once
    Active,
    /// Rule has not fired during the analysis window
    Dead,
    /// Rule was never evaluated (not matched by any facts)
    Inactive,
}

/// Result of dead rule detection for a single rule.
#[derive(Debug, Clone)]
pub struct DeadRuleInfo {
    /// Rule ID
    pub rule_id: String,
    /// Rule name
    pub rule_name: String,
    /// Last known activity timestamp
    pub last_fired_at: Option<String>,
    /// Number of times evaluated (matched conditions)
    pub evaluation_count: usize,
    /// Number of times fired
    pub fire_count: usize,
    /// Category the rule belongs to
    pub category: Option<String>,
}

impl DeadRuleInfo {
    /// Creates a new dead rule info entry.
    pub fn new(rule_id: impl Into<String>, rule_name: impl Into<String>) -> Self {
        Self {
            rule_id: rule_id.into(),
            rule_name: rule_name.into(),
            last_fired_at: None,
            evaluation_count: 0,
            fire_count: 0,
            category: None,
        }
    }

    /// Marks the rule as active (has fired).
    pub fn set_active(&mut self, timestamp: impl Into<String>) {
        self.last_fired_at = Some(timestamp.into());
        self.fire_count += 1;
    }

    /// Records a condition evaluation.
    pub fn record_evaluation(&mut self) {
        self.evaluation_count += 1;
    }
}

/// Collector for tracking rule execution across multiple runs.
#[derive(Debug, Default)]
pub struct RuleExecutionTracker {
    /// Map of rule ID to rule info
    rules: HashMap<String, DeadRuleInfo>,
    /// Total evaluation count
    total_evaluations: usize,
    /// Total fire count
    total_fires: usize,
}

impl RuleExecutionTracker {
    /// Creates a new empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that a rule was evaluated (conditions were checked).
    pub fn record_evaluation(&mut self, rule_id: impl Into<String>, rule_name: impl Into<String>) {
        let rule_id = rule_id.into();
        let rule_name = rule_name.into();

        self.rules
            .entry(rule_id.clone())
            .or_insert_with(|| DeadRuleInfo::new(&rule_id, &rule_name))
            .record_evaluation();
        self.total_evaluations += 1;
    }

    /// Records that a rule fired.
    pub fn record_fire(
        &mut self,
        rule_id: impl Into<String>,
        rule_name: impl Into<String>,
        timestamp: impl Into<String>,
    ) {
        let rule_id = rule_id.into();
        let rule_name = rule_name.into();
        let timestamp = timestamp.into();

        if let Some(info) = self.rules.get_mut(&rule_id) {
            info.fire_count += 1;
            info.last_fired_at = Some(timestamp);
        } else {
            // First time seeing this rule - mark as active (no evaluation yet)
            let mut info = DeadRuleInfo::new(&rule_id, &rule_name);
            info.fire_count = 1;  // Directly set fire count without evaluation
            info.last_fired_at = Some(timestamp);
            self.rules.insert(rule_id, info);
        }
        self.total_fires += 1;
    }

    /// Records a rule's category.
    pub fn set_rule_category(&mut self, rule_id: impl Into<String>, category: impl Into<String>) {
        let rule_id = rule_id.into();
        if let Some(info) = self.rules.get_mut(&rule_id) {
            info.category = Some(category.into());
        }
    }

    /// Gets information about a specific rule.
    pub fn get_rule_info(&self, rule_id: &str) -> Option<&DeadRuleInfo> {
        self.rules.get(rule_id)
    }

    /// Returns all rules with their activity status.
    pub fn get_all_rules(&self) -> Vec<&DeadRuleInfo> {
        self.rules.values().collect()
    }

    /// Identifies dead rules based on the evaluation window.
    ///
    /// A rule is considered dead if it was never fired during the analysis period,
    /// regardless of whether it was evaluated.
    pub fn find_dead_rules(&self) -> Vec<&DeadRuleInfo> {
        self.rules
            .values()
            .filter(|info| info.fire_count == 0)
            .collect()
    }

    /// Identifies rules that are both dead and have never been evaluated.
    pub fn find_truly_inactive_rules(&self) -> Vec<&DeadRuleInfo> {
        self.rules
            .values()
            .filter(|info| info.evaluation_count == 0 && info.fire_count == 0)
            .collect()
    }

    /// Gets the total number of rules being tracked.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Gets the total evaluation count across all rules.
    pub fn total_evaluations(&self) -> usize {
        self.total_evaluations
    }

    /// Gets the total fire count across all rules.
    pub fn total_fires(&self) -> usize {
        self.total_fires
    }

    /// Calculates the rule activation rate (fired / evaluated).
    pub fn activation_rate(&self) -> f64 {
        if self.total_evaluations == 0 {
            return 0.0;
        }
        self.total_fires as f64 / self.total_evaluations as f64
    }

    /// Resets the tracker state for a new analysis period.
    pub fn reset(&mut self) {
        // Keep rules in map but clear their metrics
        for info in self.rules.values_mut() {
            *info = DeadRuleInfo::new(&info.rule_id, &info.rule_name);
        }
        self.total_evaluations = 0;
        self.total_fires = 0;
    }

    /// Resets the tracker completely (clears all rules).
    pub fn clear(&mut self) {
        self.rules.clear();
        self.total_evaluations = 0;
        self.total_fires = 0;
    }
}

/// Dead rule detection results.
#[derive(Debug)]
pub struct DeadRuleReport {
    /// List of dead rules
    pub dead_rules: Vec<DeadRuleInfo>,
    /// Total rules analyzed
    pub total_rules: usize,
    /// Dead rule percentage
    pub dead_percentage: f64,
    /// Recommendations for each dead rule
    pub recommendations: HashMap<String, String>,
}

impl DeadRuleReport {
    /// Creates a new report from the tracker data.
    pub fn generate(tracker: &RuleExecutionTracker) -> Self {
        let dead_rules = tracker.find_dead_rules();
        let total_rules = tracker.rule_count();

        let dead_percentage = if total_rules > 0 {
            (dead_rules.len() as f64 / total_rules as f64) * 100.0
        } else {
            0.0
        };

        // Generate recommendations for each dead rule
        let mut recommendations = HashMap::new();
        for info in &dead_rules {
            recommendations.insert(
                info.rule_id.clone(),
                Self::generate_recommendation(info),
            );
        }

        Self {
            dead_rules: dead_rules.into_iter().cloned().collect(),
            total_rules,
            dead_percentage,
            recommendations,
        }
    }

    /// Generates a recommendation for a dead rule.
    fn generate_recommendation(info: &DeadRuleInfo) -> String {
        if info.evaluation_count == 0 {
            format!(
                "Rule '{}' has never been evaluated. Consider reviewing the conditions or ensuring matching facts are being inserted.",
                info.rule_name
            )
        } else {
            format!(
                "Rule '{}' was evaluated {} time(s) but never fired. Check condition logic for correctness.",
                info.rule_name, info.evaluation_count
            )
        }
    }

    /// Returns true if there are no dead rules.
    pub fn is_clean(&self) -> bool {
        self.dead_rules.is_empty()
    }

    /// Returns a summary string of the report.
    pub fn summary(&self) -> String {
        if self.dead_rules.is_empty() {
            "No dead rules detected".to_string()
        } else {
            format!(
                "{} dead rule(s) found ({:.1}% of {} rules)",
                self.dead_rules.len(),
                self.dead_percentage,
                self.total_rules
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_records_evaluations() {
        let mut tracker = RuleExecutionTracker::new();
        tracker.record_evaluation("rule1", "Test Rule");
        tracker.record_evaluation("rule2", "Another Rule");

        assert_eq!(tracker.total_evaluations(), 2);
        assert_eq!(tracker.get_rule_info("rule1").unwrap().evaluation_count, 1);
    }

    #[test]
    fn test_tracker_records_fires() {
        let mut tracker = RuleExecutionTracker::new();
        tracker.record_fire("rule1", "Test Rule", "2024-01-01T00:00:00Z".to_string());
        tracker.record_fire("rule2", "Another Rule", "2024-01-01T00:00:01Z".to_string());

        assert_eq!(tracker.total_fires(), 2);
        let info = tracker.get_rule_info("rule1").unwrap();
        assert_eq!(&info.last_fired_at, &Some("2024-01-01T00:00:00Z".to_string()));
    }

    #[test]
    fn test_find_dead_rules() {
        let mut tracker = RuleExecutionTracker::new();
        tracker.record_fire("rule1", "Active Rule", "now");
        tracker.record_evaluation("rule2", "Dead Rule");
        tracker.record_evaluation("rule3", "Also Dead");

        let dead = tracker.find_dead_rules();
        assert_eq!(dead.len(), 2);
        let rule_ids: Vec<&str> = dead.iter().map(|r| r.rule_id.as_str()).collect();
        assert!(rule_ids.contains(&"rule2"));
        assert!(rule_ids.contains(&"rule3"));
    }

    #[test]
    fn test_find_truly_inactive_rules() {
        let mut tracker = RuleExecutionTracker::new();
        // Add a rule with 0 evaluations and 0 fires by directly manipulating internal state
        tracker.rules.insert(
            "rule1".to_string(),
            DeadRuleInfo {
                rule_id: "rule1".to_string(),
                rule_name: "Evaluated Only".to_string(),
                last_fired_at: None,
                evaluation_count: 0,
                fire_count: 0,
                category: None,
            },
        );
        // Fire rule2 which also records an evaluation (fire implies evaluation)
        tracker.record_fire("rule2", "Active", "now");

        let inactive = tracker.find_truly_inactive_rules();
        assert_eq!(inactive.len(), 1);
        assert_eq!(inactive[0].rule_id, "rule1");
    }

    #[test]
    fn test_report_generation() {
        let mut tracker = RuleExecutionTracker::new();
        for i in 0..5 {
            tracker.record_evaluation(&format!("rule{}", i), &format!("Rule {}", i));
        }
        // Make rule2 active
        tracker.record_fire("rule2", "Active Rule", "now");

        let report = DeadRuleReport::generate(&tracker);

        assert_eq!(report.total_rules, 5);
        assert_eq!(report.dead_rules.len(), 4);
        assert!((report.dead_percentage - 80.0).abs() < 0.1);
    }

    #[test]
    fn test_report_summary() {
        let tracker = RuleExecutionTracker::new();
        let report = DeadRuleReport::generate(&tracker);

        assert!(report.summary().contains("No dead rules"));
    }
}
