//! Metrics collection and reporting for the rRules API server

use actix_web::{web, HttpResponse, Responder};
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry, TextEncoder,
};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use engine::monitoring::coverage::CoverageTracking;
use engine::monitoring::detection::RuleExecutionTracker;

use crate::v1::sessions::SessionManager;

/// Metrics snapshot for real-time monitoring.
///
/// Renamed from `EngineState` (L-05 / Pitfall 10) to disambiguate from
/// `api::v1::execution::EngineState` (the per-request engine holder) before
/// Phase 23 Plan 05 adds execute-path logic that imports both types.
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub engine_name: String,
    pub rules_fired: u64,
    pub facts_inserted: u64,
    pub active_sessions: usize,
    pub memory_bytes: u64,
    pub last_updated: String,
}

/// Broadcast channel for real-time updates
pub type MetricsTx = Arc<tokio::sync::broadcast::Sender<MetricsSnapshot>>;

/// Create a new metrics broadcast sender
pub fn create_metrics_channel(
    capacity: usize,
) -> (MetricsTx, tokio::sync::broadcast::Receiver<MetricsSnapshot>) {
    let (tx, rx) = tokio::sync::broadcast::channel(capacity);
    (Arc::new(tx), rx)
}

/// Phase 24 (Plan 24-02, Deliverable 1 / Open Q4): per-request execution
/// counters for POST /v1/execute, exposed to Prometheus and the WebSocket
/// broadcast surface.
///
/// Holds three labeled Prometheus metrics (`requests_total`,
/// `rules_fired_total`, `duration_seconds` — all carrying the `engine`
/// label) plus three `AtomicU64` "snapshot" mirrors used by the WebSocket
/// `MetricsSnapshot` path (Plan 24-03). The metrics live on a **dedicated**
/// `prometheus::Registry::new()` — NOT the process-global
/// `default_registry()` — so the api layer's metric names cannot collide
/// with the engine crate's registered metrics (Pitfall 24-A:
/// `registry.register` panics on duplicate name registration; a separate
/// registry is the only isolation mechanism the `prometheus` 0.13 crate
/// provides).
///
/// Plan 24-02 scope: the struct is constructed and wired as `web::Data` in
/// `main.rs`, but no handler reads or increments it yet. Plan 24-03 wires
/// `execute_single` and `metrics_endpoint` to these counters, turning the
/// Wave 0 RED tests GREEN.
#[derive(Debug)]
pub struct ExecutionCounters {
    /// `rrules_execute_requests_total{engine=...}` — total POST /v1/execute
    /// requests (incremented once per request in Plan 24-03).
    pub requests_total: IntCounterVec,
    /// `rrules_execute_rules_fired_total{engine=...}` — total rules fired
    /// by POST /v1/execute (incremented by `engine_result.rules_fired` in
    /// Plan 24-03).
    pub rules_fired_total: IntCounterVec,
    /// `rrules_execute_duration_seconds{engine=...}` — POST /v1/execute
    /// duration histogram. Buckets: `exponential_buckets(0.001, 2.0, 12)`
    /// (1ms, 2ms, 4ms ... ~4s) per Open Q1.
    pub duration_seconds: HistogramVec,
    /// Snapshot mirror of `requests_total` for the WebSocket
    /// `MetricsSnapshot` broadcast path (Plan 24-03 / SC-2).
    pub snapshot_requests: AtomicU64,
    /// Snapshot mirror of `rules_fired_total` for the WebSocket
    /// `MetricsSnapshot` broadcast path.
    pub snapshot_rules_fired: AtomicU64,
    /// Snapshot mirror of facts inserted (populated in Plan 24-03 from
    /// `engine_result.facts_inserted` when available).
    pub snapshot_facts_inserted: AtomicU64,
    /// Dedicated api-layer Prometheus registry. Pitfall 24-A: must NOT be
    /// `prometheus::default_registry()` — the engine crate registers its
    /// own metrics there, and `register()` panics on duplicate names.
    pub registry: Registry,
}

impl ExecutionCounters {
    /// Construct the counters with a dedicated `Registry::new()` and the
    /// three labeled metrics registered on it.
    ///
    /// The counter metric names are exactly `rrules_execute_requests_total`,
    /// `rrules_execute_rules_fired_total`, `rrules_execute_duration_seconds`
    /// (matching ROADMAP SC-1 expected strings), built via
    /// `prometheus::Opts::new(name, help)` — NOT the broken struct-literal
    /// `Opts { ... }` form at `engine/src/engine/metrics.rs:288` (which
    /// builds an invalid `Opts` by populating private fields directly). The
    /// histogram uses `exponential_buckets(0.001, 2.0, 12)` (Open Q1).
    pub fn new() -> Self {
        let registry = Registry::new();

        let requests_total = IntCounterVec::new(
            Opts::new(
                "rrules_execute_requests_total",
                "Total number of POST /v1/execute requests",
            ),
            &["engine"],
        )
        .expect("construct rrules_execute_requests_total counter");
        registry
            .register(Box::new(requests_total.clone()))
            .expect("register rrules_execute_requests_total");

        let rules_fired_total = IntCounterVec::new(
            Opts::new(
                "rrules_execute_rules_fired_total",
                "Total number of rules fired by POST /v1/execute",
            ),
            &["engine"],
        )
        .expect("construct rrules_execute_rules_fired_total counter");
        registry
            .register(Box::new(rules_fired_total.clone()))
            .expect("register rrules_execute_rules_fired_total");

        let duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "rrules_execute_duration_seconds",
                "POST /v1/execute duration in seconds",
            )
            .buckets(
                prometheus::exponential_buckets(0.001, 2.0, 12)
                    .expect("exponential_buckets(0.001, 2.0, 12) must produce 12 bounds"),
            ),
            &["engine"],
        )
        .expect("construct rrules_execute_duration_seconds histogram");
        registry
            .register(Box::new(duration_seconds.clone()))
            .expect("register rrules_execute_duration_seconds");

        // Pre-initialize the "rete" label combination on each counter so the
        // metric families are always present in `registry.gather()` output —
        // even before any POST /v1/execute happens. Without this,
        // `Registry::gather()` prunes families whose `Metric` list is empty
        // (prometheus 0.13 registry.rs:126), and a cold /metrics scrape
        // returns an empty body instead of the `# HELP` / `# TYPE` lines a
        // Prometheus scraper expects. "rete" is the default engine
        // (`EngineSelector::with_defaults()`); other engine labels
        // (treat/leaps/phreak) are created lazily on first
        // `with_label_values` call in `execute_single`.
        requests_total.with_label_values(&["rete"]);
        rules_fired_total.with_label_values(&["rete"]);
        duration_seconds.with_label_values(&["rete"]);

        Self {
            requests_total,
            rules_fired_total,
            duration_seconds,
            snapshot_requests: AtomicU64::new(0),
            snapshot_rules_fired: AtomicU64::new(0),
            snapshot_facts_inserted: AtomicU64::new(0),
            registry,
        }
    }
}

impl Default for ExecutionCounters {
    /// Delegates to [`ExecutionCounters::new`] so test fixtures can build a
    /// default instance via `web::Data::new(ExecutionCounters::default())`
    /// (Open Q7 — test injection).
    fn default() -> Self {
        Self::new()
    }
}

/// HTML template for metrics page
fn metrics_html(
    engine_name: &str,
    rules_fired: u64,
    memory_bytes: u64,
    active_sessions: usize,
) -> String {
    let memory_mb = format!("{:.2}", memory_bytes as f64 / 1_048_576.0);
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");

    format!(
        r#"
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Metrics - rRules</title>
  <style>
    * {{ margin: 0; padding: 0; box-sizing: border-box; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
      min-height: 100vh;
      padding: 40px 20px;
    }}
    .container {{
      max-width: 800px;
      margin: 0 auto;
    }}
    h1 {{
      color: white;
      text-align: center;
      margin-bottom: 30px;
      font-size: 2.5rem;
    }}
    .metrics-grid {{
      display: grid;
      gap: 20px;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    }}
    .metric-card {{
      background: white;
      padding: 24px;
      border-radius: 16px;
      box-shadow: 0 8px 32px rgba(0,0,0,0.2);
      text-align: center;
    }}
    .metric-label {{
      color: #666;
      font-size: 0.9rem;
      margin-bottom: 12px;
      text-transform: uppercase;
      letter-spacing: 1px;
      font-weight: 600;
    }}
    .metric-value {{
      color: #333;
      font-size: 2rem;
      font-weight: 700;
    }}
    .metric-unit {{
      color: #999;
      font-size: 0.85rem;
    }}
    .section-title {{
      color: white;
      margin-top: 40px;
      margin-bottom: 20px;
      font-size: 1.5rem;
    }}
    .rules-list {{
      background: white;
      border-radius: 16px;
      padding: 24px;
      box-shadow: 0 8px 32px rgba(0,0,0,0.2);
    }}
    .status-bar {{
      margin-top: 30px;
      background: white;
      padding: 24px;
      border-radius: 16px;
      box-shadow: 0 8px 32px rgba(0,0,0,0.2);
    }}
    .status-row {{
      display: flex;
      justify-content: space-between;
      margin-bottom: 12px;
    }}
    .status-label {{ color: #666; font-size: 0.9rem; }}
    .status-value {{ color: #333; font-weight: 500; }}
    .timestamp {{
      text-align: center;
      margin-top: 40px;
      color: rgba(255,255,255,0.7);
      font-size: 0.85rem;
    }}
  </style>
</head>
<body>
  <div class="container">
    <h1>Engine Metrics</h1>
    <div class="metrics-grid">
      <div class="metric-card">
        <div class="metric-label">Rules Fired</div>
        <div class="metric-value">{}</div>
        <div class="metric-unit">total</div>
      </div>
      <div class="metric-card">
        <div class="metric-label">Memory Usage</div>
        <div class="metric-value">{}</div>
        <div class="metric-unit">MB</div>
      </div>
      <div class="metric-card">
        <div class="metric-label">Active Sessions</div>
        <div class="metric-value">{}</div>
        <div class="metric-unit">sessions</div>
      </div>
    </div>
    <h2 class="section-title">System Status</h2>
    <div class="status-bar">
      <div class="status-row"><span class="status-label">Engine Name</span><span class="status-value">{}</span></div>
      <div class="status-row"><span class="status-label">Status</span><span class="status-value">Running</span></div>
    </div>
    <p class="timestamp">Last updated: {} | rRules v9.0.75</p>
  </div>
</body>
</html>
"#,
        rules_fired, memory_mb, active_sessions, engine_name, now
    )
}

/// HTTP handler for metrics endpoint - GET /metrics
#[utoipa::path(
    get,
    path = "/metrics",
    tag = "metrics",
    responses(
        (status = 200, description = "Metrics page (HTML), JSON, or Prometheus text format depending on Accept header"),
    ),
)]
pub async fn metrics_endpoint(
    req: actix_web::HttpRequest,
    counters: web::Data<ExecutionCounters>,
    // v5.0 audit WARNING 2: read the live session count for the HTML + JSON
    // arms so they agree with the Prometheus text arm (which gathers the
    // rrules_sessions_active gauge from counters.registry) and the WS
    // broadcast (execution.rs uses session_manager.len()). Previously these
    // two arms hardcoded active_sessions: 0.
    //
    // `Option<...>` so the handler degrades to 0 in minimal test apps that
    // don't install a SessionManager (e.g. the auth allowlist + standalone
    // metrics tests). Production `main.rs` always installs it (app_data at
    // main.rs:224) → `Some` → live count; the
    // `metrics_json_active_sessions_live` regression test proves that path.
    session_manager: Option<web::Data<Arc<SessionManager>>>,
) -> impl Responder {
    let accept = req
        .headers()
        .get("Accept")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    match accept {
        _ if accept.contains("text/html") || accept.contains("*/*") => {
            // Phase 24 (Plan 24-03): read live rules_fired from the AtomicU64
            // mirror. v5.0 audit WARNING 2: active_sessions now read from the
            // live SessionManager (matches the Prometheus text + WS arms);
            // memory_bytes stays 0 (Open Q3 — no memory metric wired yet).
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(metrics_html(
                    "default",
                    counters.snapshot_rules_fired.load(Ordering::Relaxed),
                    0,
                    session_manager.as_ref().map_or(0, |sm| sm.len()),
                ))
        }
        _ if accept.contains("application/json") => {
            // Phase 24 (Plan 24-03): read live counter values from the AtomicU64
            // mirrors. The engine_name stays "default" for the JSON arm (the
            // per-execute engine_name goes through the WS broadcast, not this
            // generic scrape). v5.0 audit WARNING 2: active_sessions now read
            // from the live SessionManager (matches the Prometheus text + WS
            // arms); memory_bytes stays 0 (Open Q3).
            HttpResponse::Ok()
                .content_type("application/json")
                .json(serde_json::json!({
                    "engine_name": "default",
                    "rules_fired": counters.snapshot_rules_fired.load(Ordering::Relaxed),
                    "facts_inserted": counters.snapshot_facts_inserted.load(Ordering::Relaxed),
                    "active_sessions": session_manager.as_ref().map_or(0, |sm| sm.len()),
                    "memory_bytes": 0
                }))
        }
        _ => {
            // Phase 24 (Plan 24-03 / Deliverable 4 / Don't Hand-Roll): encode
            // the dedicated registry via prometheus::TextEncoder — NOT manual
            // format!() string-building. Reads from counters.registry (the
            // dedicated Registry::new()), NOT prometheus::default_registry()
            // (Pitfall 24-A). The TextEncoder output contains # HELP and # TYPE
            // lines for each registered metric family
            // (rrules_execute_requests_total, rrules_execute_rules_fired_total,
            // rrules_execute_duration_seconds + its _bucket/_sum/_count series),
            // with the engine="rete" label populated after a POST /v1/execute.
            let metric_families = counters.registry.gather();
            let mut buffer = Vec::new();
            let encoder = TextEncoder::new();
            encoder.encode(&metric_families, &mut buffer).expect(
                "prometheus TextEncoder encode is infallible for valid gathered metric families",
            );
            let text = String::from_utf8(buffer).unwrap_or_default();
            HttpResponse::Ok()
                .content_type("text/plain; version=0.0.4")
                .body(text)
        }
    }
}

/// Dead rules endpoint - GET /v1/metrics/dead-rules
#[utoipa::path(
    get,
    path = "/v1/metrics/dead-rules",
    tag = "metrics",
    responses(
        (status = 200, description = "List of dead rule IDs", body = Vec<String>),
    ),
)]
pub async fn dead_rules_endpoint() -> impl Responder {
    // Read dead rules from the process-wide RuleExecutionTracker singleton
    // (Plan 26-02 / D-03, L-03, Pitfall 3). The singleton is populated by
    // the recording call sites in `direct_eval.rs` (Treat/Leaps/Phreak) and
    // `rete/mod.rs` (Rete) — `record_evaluation` / `record_fire` are called
    // adjacent to the existing `CoverageTracking` calls at the post-fixpoint
    // recording block. `find_dead_rules()` returns owned `Vec<DeadRuleInfo>`
    // (the rule_ids that were evaluated at least once but never fired).
    let dead_rules = RuleExecutionTracker::find_dead_rules();

    // Map to Vec<String> of rule IDs (move info.rule_id — no clone needed).
    let rule_ids: Vec<String> = dead_rules.into_iter().map(|info| info.rule_id).collect();
    HttpResponse::Ok().json(rule_ids)
}

/// Coverage endpoint - GET /v1/metrics/coverage
#[utoipa::path(
    get,
    path = "/v1/metrics/coverage",
    tag = "metrics",
    responses(
        (status = 200, description = "Coverage data as JSON"),
    ),
)]
pub async fn coverage_endpoint() -> impl Responder {
    // Get the singleton instance and serialize coverage data to JSON
    let tracker = CoverageTracking;
    let json_data = tracker.to_json();

    HttpResponse::Ok()
        .content_type("application/json")
        .body(json_data)
}

/// Initialize the Prometheus metrics route at root — GET /metrics.
///
/// Only the Prometheus `/metrics` endpoint is registered here so it stays at
/// the root path (REQUIREMENTS.md: "HTTP endpoint `/metrics` serves
/// Prometheus-formatted metrics"). The dashboard endpoints
/// (`/metrics/dead-rules`, `/metrics/coverage`) are registered inside the
/// `/v1` scope by [`configure_metrics_dashboards`], where they resolve to
/// `/v1/metrics/dead-rules` and `/v1/metrics/coverage` (MON-03/MON-04).
pub fn configure_metrics(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/metrics").route("", web::get().to(metrics_endpoint)));
}

/// Initialize the dashboard metric routes for the `/v1` scope (MON-03/MON-04).
///
/// Registers `/metrics/dead-rules` and `/metrics/coverage`, which resolve to
/// `/v1/metrics/dead-rules` and `/v1/metrics/coverage` when this config is
/// registered inside the `web::scope("/v1")` block in `main.rs`. The Prometheus
/// `/metrics` endpoint is intentionally NOT registered here — it stays at root
/// via [`configure_metrics`] so the root Prometheus scrape contract and the
/// `metrics_prometheus_text_format` test keep working unchanged.
pub fn configure_metrics_dashboards(cfg: &mut web::ServiceConfig) {
    cfg.route("/metrics/dead-rules", web::get().to(dead_rules_endpoint))
        .route("/metrics/coverage", web::get().to(coverage_endpoint));
}
