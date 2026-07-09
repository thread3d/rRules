//! Tenant management module for multi-tenancy support.
//!
//! This module provides the core tenant abstraction and storage backends:
//! - `types` submodule — `Tenant`, request/response types
//! - `store` submodule — `TenantStoreBackend` trait with `MemoryTenantStore` and SQLx placeholder

pub mod types;
pub use types::{CreateTenantRequest, Tenant, TenantListResponse, TenantResponse};

pub mod store;
pub use store::TenantStore;
