//! API crate - REST API layer for rRules Business Rules Management System
//!
//! Provides HTTP endpoints for rule execution, CRUD operations,
//! and governance features.

pub mod metrics;
pub mod v1;
pub mod websocket;
// Phase 22: pragmatic auth (D-22-01). In-memory user store + JWT handlers.
pub mod auth;
// Phase 22: request middleware (JWT validation). Declared here so integration
// tests can reach `api::middleware::auth`.
pub mod middleware;

// Modules the binary also needs. They were declared in `main.rs` as well, which compiled
// the same sources into two crates: the binary's `metrics::ExecutionCounters` was a
// different type from the library's, so a test could pass against one and the server run
// the other. The library owns them; the binary imports them.
pub mod health;
pub mod openapi;
pub mod spa;

/// The application itself: state, and the builder `main` and the tests both call.
pub mod app;
