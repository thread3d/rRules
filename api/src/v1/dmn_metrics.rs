//! DMN-specific Prometheus metrics + WebSocket broadcast (Phase 33).
//!
//! Exposes metric families on a dedicated registry, and provides a real-time
//! WebSocket broadcast surface for decision evaluation events.

use std::sync::Arc;
use std::sync::LazyLock;

use actix_web::{HttpResponse, Responder};
use prometheus::{Encoder, Registry, TextEncoder};
use serde::Serialize;

// Re-export Phase 35 extended metrics from engine (avoids duplicate registration).
pub use engine::dmn::evaluator::{METRIC_DRD_CHAIN_DEPTH, METRIC_OUTPUT_EVALUATIONS_TOTAL};

/// Original Phase 33 metrics (defined here since they're API-scoped).
use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, Opts};

pub static METRIC_EVALUATIONS_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        Opts::new(
            "rrules_dmn_evaluations_total",
            "Total number of decision evaluations",
        ),
        &["decision_id"],
    )
    .expect("metrics to be valid")
});

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

pub static METRIC_HIT_POLICY_VIOLATIONS_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        Opts::new(
            "rrules_dmn_hit_policy_violations_total",
            "Total number of hit policy violations during decision evaluation",
        ),
        &["decision_id", "hit_policy"],
    )
    .expect("metrics to be valid")
});

// =========================================================================
// Dedicated Prometheus registry (Pitfall 24-A: never use default_registry)
// =========================================================================

/// Dedicated Prometheus registry for DMN metrics.
pub static DMN_METRICS_REGISTRY: LazyLock<Registry> = LazyLock::new(|| {
    let registry = Registry::new();
    // Phase 33 metrics.
    let _ = registry.register(Box::new(METRIC_EVALUATIONS_TOTAL.clone()));
    let _ = registry.register(Box::new(METRIC_DECISION_DURATION_SECONDS.clone()));
    let _ = registry.register(Box::new(METRIC_HIT_POLICY_VIOLATIONS_TOTAL.clone()));
    // Phase 35 extended metrics (re-exported from engine).
    let _ = registry.register(Box::new(METRIC_OUTPUT_EVALUATIONS_TOTAL.clone()));
    let _ = registry.register(Box::new(METRIC_DRD_CHAIN_DEPTH.clone()));
    registry
});

// =========================================================================
// Real-time broadcast channel (DmnMetricsSnapshot)
// =========================================================================

/// Snapshot broadcast after each successful DMN evaluation. Fields mirror the
/// v5.0 engine `MetricsSnapshot` pattern (Phase 22 / Plan 22-04).
#[derive(Debug, Clone, Serialize)]
pub struct DmnMetricsSnapshot {
    /// Decision ID that was evaluated.
    pub decision_id: String,
    /// Number of rules fired during evaluation.
    pub rules_fired: u64,
    /// Evaluation duration in milliseconds.
    pub duration_ms: f64,
    /// ISO-8601 timestamp when the snapshot was created.
    pub timestamp: String,
}

/// A broadcaster with no subscribers, for tests that mount a handler directly.
///
/// The evaluate handler publishes a snapshot; a test that builds its own `App` must supply the
/// sender, exactly as the real application does.
pub fn test_channel() -> DmnMetricsTx {
    create_dmn_metrics_channel(16).0
}

/// Broadcast channel type alias for DMN metrics snapshots.
pub type DmnMetricsTx = Arc<tokio::sync::broadcast::Sender<DmnMetricsSnapshot>>;

/// Create a new DMN metrics broadcast sender + receiver pair (capacity 16).
pub fn create_dmn_metrics_channel(
    capacity: usize,
) -> (
    DmnMetricsTx,
    tokio::sync::broadcast::Receiver<DmnMetricsSnapshot>,
) {
    let (tx, rx) = tokio::sync::broadcast::channel(capacity);
    (Arc::new(tx), rx)
}

// =========================================================================
// HTTP endpoint — Prometheus text format
// =========================================================================

/// Serve DMN metrics in Prometheus text format — `GET /v1/metrics/dmn`.
///
/// Uses the dedicated `DMN_METRICS_REGISTRY` (NOT `default_registry`) to avoid
/// Pitfall 24-A duplicate metric name panics. The output contains `# HELP` and
/// `# TYPE` lines for every registered metric family, matching the Prometheus
/// exposition format that scrapers expect.
#[utoipa::path(
    get,
    path = "/v1/metrics/dmn",
    responses((status = 200, description = "DMN metrics in Prometheus exposition format", body = String, content_type = "text/plain")),
    tag = "metrics",
)]
pub async fn dmn_metrics_endpoint() -> impl Responder {
    let metric_families = DMN_METRICS_REGISTRY.gather();
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .expect("prometheus TextEncoder encode is infallible for valid gathered metric families");
    let text = String::from_utf8(buffer).unwrap_or_default();
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(text)
}

// =========================================================================
// Route configuration
// =========================================================================

/// Configure DMN metrics routes inside the `/v1` scope (Plan 33-01 Task 3).
pub fn configure_dmn_metrics(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.route(
        "/metrics/dmn",
        actix_web::web::get().to(dmn_metrics_endpoint),
    );
}
