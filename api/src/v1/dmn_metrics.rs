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
    let _ = registry.register(Box::new(METRIC_EVALUATE_REQUESTS_TOTAL.clone()));
    let _ = registry.register(Box::new(METRIC_EVALUATE_DURATION_SECONDS.clone()));
    let _ = registry.register(Box::new(METRIC_RULES_FIRED_TOTAL.clone()));
    registry
});

/// Metric: total decision evaluation requests.
pub static METRIC_EVALUATE_REQUESTS_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        Opts::new("rrules_dmn_evaluate_requests_total", "Total number of decision evaluation requests"),
        &["decision_id"],
    )
    .expect("metrics to be valid")
});

/// Metric: distribution of decision evaluation duration in seconds.
pub static METRIC_EVALUATE_DURATION_SECONDS: LazyLock<HistogramVec> = LazyLock::new(|| {
    HistogramVec::new(
        HistogramOpts::new(
            "rrules_dmn_evaluate_duration_seconds",
            "Distribution of decision evaluation duration in seconds",
        ),
        &["decision_id", "hit_policy"],
    )
    .expect("metrics to be valid")
});

/// Metric: total rules fired during decision evaluation.
pub static METRIC_RULES_FIRED_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        Opts::new("rrules_dmn_rules_fired_total", "Total number of rules fired during decision evaluation"),
        &["decision_id", "hit_policy"],
    )
    .expect("metrics to be valid")
});
