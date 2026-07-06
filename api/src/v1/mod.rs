//! Version 1 API endpoints - routes are configured in the main app

use actix_web::web;

pub mod approvals;
pub mod audit;
// pub mod auth;  // TODO: implement authentication module
pub mod branches;
pub mod categories;
pub mod execution;
pub mod metrics;
pub mod rules;
pub mod sessions;
pub mod simulation;
pub mod tags;
// pub mod tenants;  // TODO: implement multi-tenancy module

// Export common types for use in main.rs
pub use execution::{ReloadRequest, ReloadResponse};

// Re-export route modules
#[allow(unused)]
pub mod routes {
    pub mod monitoring;
}

/// Configure v1/metrics routes inside the /v1 scope.
pub fn configure_v1_metrics(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/metrics")
            .route("", web::get().to(metrics::get_engine_metrics))
            .route("/", web::get().to(metrics::get_engine_metrics))
            .route("/coverage/reset", web::post().to(metrics::reset_coverage)),
    );
}

/// Configure v1/simulation routes inside the /v1 scope.
pub fn configure_v1_simulation(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/simulation")
            .route("", web::post().to(simulation::run_simulation))
            .route("/", web::post().to(simulation::run_simulation)),
    );
}
