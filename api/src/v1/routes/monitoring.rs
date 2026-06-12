//! Monitoring and metrics routes for the API

use actix_web::{web, HttpResponse, Responder};
use serde::Serialize;

/// Response struct for engine state
#[derive(Debug, Clone, Serialize)]
pub struct EngineStateResponse {
    pub engine_name: String,
    pub rules_fired: u64,
    pub facts_inserted: u64,
    pub active_sessions: usize,
    pub memory_bytes: u64,
    pub alpha_memory_size: u64,
    pub beta_memory_size: u64,
    pub network_traversals: u64,
    pub execution_time_ms: f64,
    pub last_updated: String,
}

/// Get engine state - GET /v1/monitoring/engine-state
pub async fn get_engine_state() -> impl Responder {
    HttpResponse::Ok().json(EngineStateResponse {
        engine_name: "default".to_string(),
        rules_fired: 0,
        facts_inserted: 0,
        active_sessions: 0,
        memory_bytes: 0,
        alpha_memory_size: 0,
        beta_memory_size: 0,
        network_traversals: 0,
        execution_time_ms: 0.0,
        last_updated: chrono::Utc::now().to_rfc3339(),
    })
}

/// Health check for monitoring - GET /v1/monitoring/health
pub async fn monitoring_health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "rrules-monitoring"
    }))
}

/// Configuration for monitoring endpoints
pub fn configure_monitoring(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/monitoring")
            .route("/engine-state", web::get().to(get_engine_state))
            .route("/health", web::get().to(monitoring_health)),
    );
}
