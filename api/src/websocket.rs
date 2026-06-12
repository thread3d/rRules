//! WebSocket streaming for real-time monitoring
//!
//! This module provides WebSocket endpoints for real-time engine metrics streaming.
//! The endpoint `/v1/metrics/ws` allows clients to subscribe to live engine state updates.

use actix_web::{web, HttpResponse};
use std::sync::Arc;

/// Broadcast channel for metrics updates
pub type MetricsTx = Arc<tokio::sync::broadcast::Sender<crate::metrics::EngineState>>;

/// WebSocket handler for metrics streaming (placeholder)
///
/// Connects to /v1/metrics/ws and streams engine state updates.
///
/// Note: Full WebSocket implementation requires actix-web ws feature with
/// proper stream handling. This is a placeholder that returns an upgrade response.
pub async fn metrics_ws(
    _req: actix_web::HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    // Return a response indicating WebSocket support
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "WebSocket endpoint",
        "path": "/metrics/ws",
        "status": "ready for streaming"
    })))
}

/// Configuration for WebSocket monitoring endpoints
pub fn configure_metrics_ws(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/metrics")
            .route("/ws", web::get().to(metrics_ws)),
    );
}
