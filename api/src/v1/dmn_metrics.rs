//! DMN-specific Prometheus metrics (Phase 33).
//!
//! Simple metric collectors for decision evaluation tracking.

use std::sync::LazyLock;

use prometheus::{
    HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry,
};

/// Dedicated Prometheus registry for DMN metrics (separate from the main engine registry).
pub static DMN_METRICS_REGISTRY: LazyLock<Registry> = LazyLock::new(|| {
    let registry = Registry::new();
    // Register all metric collectors.
    let _ = registry.register(Box::new(METRIC_EVALUATIONS_TOTAL.clone()));
    let _ = registry.register(Box::new(METRIC_DECISION_DURATION_SECONDS.clone()));
    let _ = registry.register(Box::new(METRIC_HIT_POLICY_VIOLATIONS_TOTAL.clone()));
    registry
});

/// Metric: total decision evaluation requests (ROADMAP canonical name).
pub static METRIC_EVALUATIONS_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        Opts::new("rrules_dmn_evaluations_total", "Total number of decision evaluations"),
        &["decision_id"],
    )
    .expect("metrics to be valid")
});

/// Metric: distribution of decision evaluation duration in seconds (ROADMAP canonical name).
pub static METRIC_DECISION_DURATION_SECONDS: LazyLock<HistogramVec> = LazyLock::new(|| {
    HistogramVec::new(
        HistogramOpts::new(
            "rrules_dmn_decision_duration_seconds",
            "Distribution of decision evaluation duration in seconds",
        ),
        &["decision_id", "hit_policy"],
    )
    .expect("metrics to be valid")
});

/// Metric: total hit policy violations during decision evaluation (ROADMAP canonical name).
pub static METRIC_HIT_POLICY_VIOLATIONS_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        Opts::new("rrules_dmn_hit_policy_violations_total", "Total number of hit policy violations during decision evaluation"),
        &["decision_id", "hit_policy"],
    )
    .expect("metrics to be valid")
});
