//! DMN decision tracking (Phase 35, DMN-27).
//!
//! Separate from `RuleExecutionTracker` — tracks DMN-specific telemetry:
//! - Which decisions have been evaluated (coverage tracking)
//! - Dead decision detection (decisions never invoked)
//! - Per-decision evaluation counts for the dashboard panel

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Tracks which DMN decisions have been evaluated and how many times.
/// Separate singleton from `RuleExecutionTracker` to avoid source tag contamination.
pub struct DmnDecisionTracker {
    /// decision_id → total evaluations since tracker creation
    evaluation_counts: HashMap<String, u64>,
    /// decision_id → last evaluation timestamp (RFC 3339)
    last_evaluated: HashMap<String, String>,
}

impl Default for DmnDecisionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl DmnDecisionTracker {
    pub fn new() -> Self {
        Self {
            evaluation_counts: HashMap::new(),
            last_evaluated: HashMap::new(),
        }
    }

    /// Record that a decision was evaluated. Called after each evaluate() call.
    pub fn record_evaluation(&mut self, decision_id: &str) {
        *self
            .evaluation_counts
            .entry(decision_id.to_string())
            .or_insert(0) += 1;
        self.last_evaluated
            .insert(decision_id.to_string(), chrono_now());
    }

    /// Returns all decisions that have never been evaluated (dead decisions).
    pub fn find_dead_decisions(&self, known_ids: &[String]) -> Vec<String> {
        known_ids
            .iter()
            .filter(|id| !self.evaluation_counts.contains_key(id.as_str()))
            .cloned()
            .collect()
    }

    /// Returns evaluation counts for all tracked decisions.
    pub fn evaluation_summary(&self) -> HashMap<String, u64> {
        self.evaluation_counts.clone()
    }

    /// Returns coverage info for a single decision.
    pub fn coverage_for(&self, decision_id: &str) -> Option<(u64, String)> {
        self.evaluation_counts.get(decision_id).map(|count| {
            let last = self
                .last_evaluated
                .get(decision_id)
                .cloned()
                .unwrap_or_default();
            (*count, last)
        })
    }
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}s", d.as_secs()))
        .unwrap_or_else(|_| "unknown".to_string())
}

// Singleton pattern (mirrors RuleExecutionTracker):
pub static DMN_DECISION_TRACKER: LazyLock<Mutex<DmnDecisionTracker>> =
    LazyLock::new(|| Mutex::new(DmnDecisionTracker::new()));
