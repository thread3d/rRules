//! Health check endpoints for Kubernetes

use actix_web::{web, HttpResponse};

/// Live endpoint - returns 200 if server is running
#[utoipa::path(
    get,
    path = "/health/live",
    tag = "health",
    responses(
        (status = 200, description = "Server is live"),
    ),
)]
pub async fn live() -> HttpResponse {
    HttpResponse::Ok().finish()
}

/// Ready endpoint - returns 200 if server is ready to accept traffic
#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "health",
    responses(
        (status = 200, description = "Server is ready"),
    ),
)]
pub async fn ready() -> HttpResponse {
    // Check critical dependencies (database, rule storage)
    // For now, return ready since we use in-memory stores

    HttpResponse::Ok().finish()
}

/// Configure health endpoints
pub fn configure_health(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/health")
            .route("/live", web::get().to(live))
            .route("/ready", web::get().to(ready)),
    );
}
