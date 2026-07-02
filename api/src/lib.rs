//! API crate - REST API layer for rRules Business Rules Management System
//!
//! Provides HTTP endpoints for rule execution, CRUD operations,
//! and governance features.

pub mod models;
pub mod metrics;
pub mod v1;
pub mod websocket;
// Phase 22: pragmatic auth (D-22-01). In-memory user store + JWT handlers.
pub mod auth;
// Phase 22: request middleware (JWT validation). Declared here so integration
// tests can reach `api::middleware::auth`.
pub mod middleware;
