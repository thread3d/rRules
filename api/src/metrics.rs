//! Metrics collection and reporting for the rRules API server

use std::sync::Arc;
use actix_web::{web, HttpResponse, Responder};
use serde::Serialize;

use engine::monitoring::detection::RuleExecutionTracker;
use engine::monitoring::coverage::CoverageTracking;

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
pub fn create_metrics_channel(capacity: usize) -> (MetricsTx, tokio::sync::broadcast::Receiver<MetricsSnapshot>) {
    let (tx, rx) = tokio::sync::broadcast::channel(capacity);
    (Arc::new(tx), rx)
}


/// HTML template for metrics page
fn metrics_html(engine_name: &str, rules_fired: u64, memory_bytes: u64, active_sessions: usize) -> String {
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
    <p class="timestamp">Last updated: {} | rRules v0.1.0</p>
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
pub async fn metrics_endpoint(req: actix_web::HttpRequest) -> impl Responder {
    let accept = req.headers().get("Accept").and_then(|h| h.to_str().ok()).unwrap_or("");

    match accept {
        _ if accept.contains("text/html") || accept.contains("*/*") => {
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(metrics_html("default", 0, 0, 0))
        }
        _ if accept.contains("application/json") => {
            HttpResponse::Ok()
                .content_type("application/json")
                .json(serde_json::json!({
                    "engine_name": "default",
                    "rules_fired": 0,
                    "facts_inserted": 0,
                    "active_sessions": 0,
                    "memory_bytes": 0
                }))
        }
        _ => {
            let metrics_text = r#"# HELP engine_rules_fired Total number of rules fired
# TYPE engine_rules_fired counter
engine_rules_fired 0
# HELP engine_memory_bytes Current memory usage in bytes
# TYPE engine_memory_bytes gauge
engine_memory_bytes 0
"#;
            HttpResponse::Ok()
                .content_type("text/plain; version=0.0.4")
                .body(metrics_text)
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
    // Create a tracker instance and get dead rules
    let tracker = RuleExecutionTracker::new();
    let dead_rules = tracker.find_dead_rules();

    // Map to Vec<String> of rule IDs
    let rule_ids: Vec<String> = dead_rules.into_iter().map(|info| info.rule_id.clone()).collect();
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
    cfg.service(
        web::scope("/metrics")
            .route("", web::get().to(metrics_endpoint)),
    );
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
