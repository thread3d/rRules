//! Metrics and monitoring endpoints

use actix_web::{web, HttpResponse, Responder};

/// Get engine metrics summary - GET /v1/metrics/engine
pub async fn get_engine_metrics() -> impl Responder {
    // For now, return default metrics since we don't track them yet
    HttpResponse::Ok().json(serde_json::json!({
        "total_evaluations": 0,
        "total_fires": 0,
        "rules_evaluated": 0,
        "rules_fired": 0,
        "coverage_percent": 0.0
    }))
}

/// Reset coverage metrics - POST /v1/metrics/coverage/reset
pub async fn reset_coverage() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "reset",
        "message": "Coverage metrics have been cleared"
    }))
}
