use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;

/// Current state of a rule's coverage
#[derive(Debug, Clone, Default)]
pub struct RuleCoverageState {
    pub evaluated: bool,
    pub fired: bool,
    pub skipped_reason: Option<String>,
    pub evaluation_count: u64,
    pub fire_count: u64,
}

/// Summary of coverage across all rules.
#[derive(Debug, Clone, Serialize)]
pub struct CoverageSummary {
    pub total_rules: usize,
    pub evaluated_count: usize,
    pub fired_count: usize,
    pub coverage_percent: f64,
}

/// Report for a single rule's execution coverage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageReport {
    pub timestamp: String,
    pub rule_id: String,
    pub evaluated: bool,
    pub fired: bool,
    pub skipped_reason: Option<String>,
}

impl CoverageReport {
    pub fn new(rule_id: impl Into<String>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        CoverageReport {
            timestamp,
            rule_id: rule_id.into(),
            evaluated: false,
            fired: false,
            skipped_reason: None,
        }
    }

    pub fn with_evaluated(mut self, evaluated: bool) -> Self {
        self.evaluated = evaluated;
        self
    }

    pub fn with_fired(mut self, fired: bool) -> Self {
        self.fired = fired;
        self
    }

    pub fn with_skipped_reason(mut self, reason: impl Into<String>) -> Self {
        self.skipped_reason = Some(reason.into());
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize coverage report")
    }
}

impl Default for CoverageReport {
    fn default() -> Self {
        Self::new("")
    }
}

/// Internal tracking state (not exported)
struct TrackingState {
    rules: HashMap<String, RuleCoverageState>,
}

use std::sync::LazyLock;

/// Global singleton with interior mutability via Mutex
static INSTANCE: LazyLock<Mutex<TrackingState>> = LazyLock::new(|| {
    Mutex::new(TrackingState {
        rules: HashMap::new(),
    })
});

/// Locks the process-wide tracker, recovering from a poisoned mutex.
///
/// A tracker lock is only ever held for a handful of field updates, so if a
/// thread panicked while holding it the state is still structurally sound: the
/// useful behaviour is to keep serving metrics rather than to panic every
/// subsequent monitoring call. Panicking here would turn one failed test into a
/// process-wide observability outage.
fn lock_state() -> std::sync::MutexGuard<'static, TrackingState> {
    INSTANCE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Singleton for tracking coverage across multiple engine runs.
pub struct CoverageTracking;

impl CoverageTracking {
    /// Records that a rule was evaluated (conditions were checked).
    pub fn record_rule_eval(rule_id: impl Into<String>) {
        let mut state = lock_state();
        let entry = state.rules.entry(rule_id.into()).or_default();
        entry.evaluated = true;
        entry.evaluation_count += 1;
    }

    /// Records that a rule fired (actions were executed).
    pub fn record_rule_fire(rule_id: impl Into<String>) {
        let mut state = lock_state();
        let entry = state.rules.entry(rule_id.into()).or_default();
        entry.fired = true;
        entry.fire_count += 1;
    }

    /// Records that a rule was skipped with a reason.
    pub fn record_rule_skipped(rule_id: impl Into<String>, reason: impl Into<String>) {
        let mut state = lock_state();
        let entry = state.rules.entry(rule_id.into()).or_default();
        entry.skipped_reason = Some(reason.into());
        entry.evaluation_count += 1;
    }

    /// Gets the current coverage state for a specific rule.
    pub fn get_rule_state(&self, rule_id: &str) -> Option<RuleCoverageState> {
        let state = lock_state();
        state.rules.get(rule_id).cloned()
    }

    /// Returns all rules with their coverage states (owned data).
    pub fn get_all_states(&self) -> Vec<(String, RuleCoverageState)> {
        let state = lock_state();
        // Collect as owned data since we can't return references from inside the lock
        state.rules.iter().map(|(id, s)| (id.clone(), s.clone())).collect()
    }

    /// Gets a summary of coverage across all tracked rules.
    pub fn get_summary(&self) -> CoverageSummary {
        let state = lock_state();
        let mut total_evaluated = 0;
        let mut total_fired = 0;

        for s in state.rules.values() {
            if s.evaluated {
                total_evaluated += 1;
            }
            if s.fired {
                total_fired += 1;
            }
        }

        CoverageSummary {
            total_rules: state.rules.len(),
            evaluated_count: total_evaluated,
            fired_count: total_fired,
            coverage_percent: if state.rules.is_empty() {
                0.0
            } else {
                (total_fired as f64 / state.rules.len() as f64) * 100.0
            },
        }
    }

    /// Clears all tracking data for a fresh start.
    pub fn clear(&self) {
        let mut state = lock_state();
        state.rules.clear();
    }

    /// Exports coverage data as JSON.
    pub fn to_json(&self) -> String {
        let state = lock_state();

        let rules_data: Vec<_> = state.rules.iter().map(|(id, rule)| {
            serde_json::json!({
                "rule_id": id,
                "evaluated": rule.evaluated,
                "fired": rule.fired,
                "evaluation_count": rule.evaluation_count,
                "fire_count": rule.fire_count,
                "skipped_reason": rule.skipped_reason
            })
        }).collect();

        let summary = CoverageSummary {
            total_rules: state.rules.len(),
            evaluated_count: state.rules.values().filter(|s| s.evaluated).count(),
            fired_count: state.rules.values().filter(|s| s.fired).count(),
            coverage_percent: if state.rules.is_empty() {
                0.0
            } else {
                let fired = state.rules.values().filter(|s| s.fired).count();
                (fired as f64 / state.rules.len() as f64) * 100.0
            },
        };

        serde_json::json!({
            "rules": rules_data,
            "summary": summary
        }).to_string()
    }

    /// Exports coverage data as CSV row.
    pub fn to_csv_row(rule_id: &str, evaluated: bool, fired: bool) -> String {
        format!(
            "{},{},{}",
            rule_id,
            if evaluated { "evaluated" } else { "not_evaluated" },
            if fired { "fired" } else { "not_fired" }
        )
    }

    /// Exports all rules to CSV format.
    pub fn to_csv(&self) -> String {
        let state = lock_state();
        let mut lines = vec!["rule_id,status,fire_count".to_string()];

        for (id, rule) in &state.rules {
            let status = if rule.fired {
                "fired"
            } else if rule.evaluated {
                "evaluated"
            } else {
                "not_evaluated"
            };
            lines.push(format!("{},{},{}", id, status, rule.fire_count));
        }

        lines.join("\n")
    }
}

impl Default for CoverageTracking {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coverage_report_creation() {
        let report = CoverageReport::new("rule-1");
        assert_eq!(report.rule_id, "rule-1");
        assert!(!report.evaluated);
        assert!(!report.fired);
        assert!(report.skipped_reason.is_none());
    }

    #[test]
    fn test_with_evaluated() {
        let report = CoverageReport::new("rule-1").with_evaluated(true);
        assert!(report.evaluated);
    }

    #[test]
    fn test_with_fired() {
        let report = CoverageReport::new("rule-1").with_fired(true);
        assert!(report.fired);
    }

    #[test]
    fn test_with_skipped_reason() {
        let report = CoverageReport::new("rule-1").with_skipped_reason("condition not matched");
        assert_eq!(report.skipped_reason, Some("condition not matched".to_string()));
    }

    #[test]
    fn test_to_json() {
        let report = CoverageReport::new("rule-1")
            .with_evaluated(true)
            .with_fired(false);

        let json = report.to_json();
        assert!(json.contains("\"evaluated\":true"));
        assert!(json.contains("\"fired\":false"));
    }

    #[test]
    fn test_default() {
        let report = CoverageReport::default();
        // Default has empty rule_id from Self::new("")
        assert!(!report.evaluated);
        assert!(!report.fired);
    }

    #[test]
    fn test_coverage_tracking_singleton() {
        // Clear and create fresh tracking
        let tracker = CoverageTracking;
        tracker.clear();

        // Record some evaluations
        CoverageTracking::record_rule_eval("rule-1");
        CoverageTracking::record_rule_eval("rule-2");

        let states = tracker.get_all_states();
        assert_eq!(states.len(), 2);
    }

    #[test]
    fn test_coverage_tracking_fire() {
        let tracker = CoverageTracking;
        tracker.clear();

        CoverageTracking::record_rule_eval("rule-1");
        CoverageTracking::record_rule_fire("rule-1");

        let state = tracker.get_rule_state("rule-1").unwrap();
        assert!(state.evaluated);
        assert!(state.fired);
        assert_eq!(state.evaluation_count, 1);
        assert_eq!(state.fire_count, 1);
    }

    #[test]
    fn test_coverage_tracking_skipped() {
        let tracker = CoverageTracking;
        tracker.clear();

        CoverageTracking::record_rule_skipped("rule-1", "condition not matched");

        let state = tracker.get_rule_state("rule-1").unwrap();
        assert_eq!(state.skipped_reason, Some("condition not matched".to_string()));
    }

    #[test]
    fn test_coverage_summary() {
        let tracker = CoverageTracking;
        tracker.clear();

        CoverageTracking::record_rule_eval("rule-1");
        CoverageTracking::record_rule_fire("rule-1");

        CoverageTracking::record_rule_eval("rule-2");
        // rule-2 never fired

        let summary = tracker.get_summary();
        assert_eq!(summary.total_rules, 2);
        assert_eq!(summary.evaluated_count, 2);
        assert_eq!(summary.fired_count, 1);
    }

    #[test]
    fn test_to_csv() {
        let tracker = CoverageTracking;
        tracker.clear();

        CoverageTracking::record_rule_eval("rule-1");
        CoverageTracking::record_rule_fire("rule-1");

        let csv = tracker.to_csv();
        assert!(csv.contains("rule-1,fired,1"));
    }
}
