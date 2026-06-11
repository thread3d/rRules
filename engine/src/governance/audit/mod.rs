//! Audit trail module for Phase 18 Enterprise Governance
//!
//! This module provides event-sourced immutable logging:
//! - Event types (defined in `types` submodule)
//! - PostgreSQL-backed storage (`store` submodule)
//! - Async batched write queue (`queue` submodule)

pub mod types;
pub use types::AuditEvent;

pub mod store;
pub use store::AuditStore;

// pub mod queue;
// pub use queue::AuditQueue;
