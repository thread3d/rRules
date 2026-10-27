//! Version 1 API endpoints - routes are configured in the main app

use actix_web::web;

pub mod approvals;
pub mod audit;
#[cfg(feature = "postgres")]
pub mod audit_postgres;
pub mod audit_store;
// Auth is implemented at `crate::auth` (handlers) + `crate::middleware::auth`
// (JWT validation) — see api/src/lib.rs, not a v1 submodule.
pub mod branches;
pub mod categories;
pub mod decisions;
pub mod dmn_metrics;
pub mod dmn_metrics_ws;
pub mod dmn_store;
pub mod execution;
pub mod rules;
pub mod sessions;
pub mod simulation;
pub mod tags;
// Multi-tenancy: tenant (organization/workspace) CRUD. Rule CRUD is already
// scoped by the `X-Tenant-ID` header; this module manages the tenants
// themselves.
#[cfg(feature = "postgres")]
pub mod tenant_postgres;
pub mod tenant_store;
pub mod tenants;

// Export common types for use in main.rs
pub use execution::{ReloadRequest, ReloadResponse};

// Re-export route modules
#[allow(unused)]
pub mod routes {
    pub mod monitoring;
}

/// Configure v1/simulation routes inside the /v1 scope.
pub fn configure_v1_simulation(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/simulation")
            .route("", web::post().to(simulation::run_simulation))
            .route("/", web::post().to(simulation::run_simulation)),
    );
}

/// Configure v1/decisions routes inside the /v1 scope (Phase 33).
pub fn configure_v1_decisions(cfg: &mut web::ServiceConfig) {
    decisions::configure_v1_decisions(cfg);
}

/// Configure DMN metrics + WebSocket endpoints inside the `/v1` scope (Plan 33-01 Task 3).
pub fn configure_dmn_metrics(cfg: &mut web::ServiceConfig) {
    cfg.configure(dmn_metrics::configure_dmn_metrics);
    cfg.configure(dmn_metrics_ws::configure_dmn_metrics_ws);
}
