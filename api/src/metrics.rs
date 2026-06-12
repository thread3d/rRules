//! Metrics collection and reporting for the rRules API server

use std::sync::Arc;
use actix_web::{web, HttpResponse, Responder};
use serde::Serialize;

/// Engine state for real-time monitoring
#[derive(Debug, Clone, Serialize)]
pub struct EngineState {
    pub engine_name: String,
    pub rules_fired: u64,
    pub facts_inserted: u64,
    pub active_sessions: usize,
    pub memory_bytes: u64,
    pub last_updated: String,
}

/// Broadcast channel for real-time updates
pub type MetricsTx = Arc<tokio::sync::broadcast::Sender<EngineState>>;

/// Create a new metrics broadcast sender
pub fn create_metrics_channel(capacity: usize) -> (MetricsTx, tokio::sync::broadcast::Receiver<EngineState>) {
    let (tx, rx) = tokio::sync::broadcast::channel(capacity);
    (Arc::new(tx), rx)
}

/// Publish engine state to all subscribers
pub fn publish_engine_state(tx: &MetricsTx, state: EngineState) -> Result<usize, String> {
    Ok(tx.send(state).map_err(|e| e.to_string())?)
}

/// HTTP handler for metrics endpoint - GET /metrics
///
/// Supports both Prometheus text format and JSON based on Accept header.
pub async fn metrics_endpoint(req: actix_web::HttpRequest) -> impl Responder {
    // Check Accept header to determine response format
    let accept = req.headers().get("Accept").and_then(|h| h.to_str().ok()).unwrap_or("");

    match accept {
        _ if accept.contains("application/json") => {
            // Return JSON format
            HttpResponse::Ok()
                .content_type("application/json")
                .json(serde_json::json!({
                    "engine_name": "default",
                    "rules_fired": 0,
                    "facts_inserted": 0,
                    "active_sessions": 0,
                    "memory_bytes": 0,
                    "alpha_memory_size": 0,
                    "beta_memory_size": 0,
                    "network_traversals": 0,
                    "firing_rate_per_second": 0.0
                }))
        }
        _ => {
            // Default to Prometheus text format
            let metrics_text = r#"# HELP engine_rules_fired Total number of rules fired
# TYPE engine_rules_fired counter
engine_rules_fired 0
# HELP engine_memory_bytes Current memory usage in bytes
# TYPE engine_memory_bytes gauge
engine_memory_bytes 0
# HELP engine_engine_count Number of active engines
# TYPE engine_engine_count gauge
engine_engine_count 1
"#;

            HttpResponse::Ok()
                .content_type("text/plain; version=0.0.4")
                .body(metrics_text)
        }
    }
}

/// Dead rules endpoint - GET /metrics/dead-rules
pub async fn dead_rules_endpoint() -> impl Responder {
    use engine::monitoring::detection::RuleExecutionTracker;

    let tracker = RuleExecutionTracker::new();

    // Map to API response format
    #[derive(serde::Serialize)]
    struct DeadRuleResponse {
        rule_id: String,
        rule_name: String,
        category: Option<String>,
        evaluation_count: usize,
        fire_count: usize,
        recommendation: String,
    }

    let rules: Vec<DeadRuleResponse> = tracker
        .get_all_rules()
        .into_iter()
        .map(|rule| DeadRuleResponse {
            rule_id: rule.rule_id.clone(),
            rule_name: rule.rule_name.clone(),
            category: rule.category.clone(),
            evaluation_count: rule.evaluation_count,
            fire_count: rule.fire_count,
            recommendation: format!(
                "Rule '{}' has never been fired",
                rule.rule_name
            ),
        })
        .collect();

    HttpResponse::Ok().json(rules)
}

/// Coverage endpoint - GET /metrics/coverage
pub async fn coverage_endpoint() -> impl Responder {
    #[derive(serde::Serialize)]
    struct RuleCoverageResponse {
        rule_id: String,
        rule_name: String,
        category: Option<String>,
        evaluated_count: usize,
        fired_count: usize,
        coverage_percent: f64,
    }

    let coverages: Vec<RuleCoverageResponse> = Vec::new();

    HttpResponse::Ok().json(coverages)
}

/// Initialize metrics module - register routes
pub fn configure_metrics(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/metrics")
            .route("", web::get().to(metrics_endpoint))
            .route("/dead-rules", web::get().to(dead_rules_endpoint))
            .route("/coverage", web::get().to(coverage_endpoint)),
    );
}
