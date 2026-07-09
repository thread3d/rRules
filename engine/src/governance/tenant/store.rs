//! PostgreSQL-backed tenant store for multi-tenancy support.
//!
//! This module provides a trait-based interface for tenant CRUD operations
//! with an in-memory implementation and a placeholder SQLx backend.

#![allow(async_fn_in_trait)]

use crate::governance::tenant::types::Tenant;
use chrono::{DateTime, Utc};

/// Trait for tenant storage backends.
pub trait TenantStoreBackend {
    /// Create a new tenant. Returns the created tenant with generated id and timestamp.
    async fn create_tenant(&mut self, name: &str) -> Result<Tenant, Box<dyn std::error::Error + Send + Sync>>;

    /// Retrieve a tenant by ID. Returns `None` if not found.
    async fn get_tenant(&self, id: &str) -> Result<Option<Tenant>, Box<dyn std::error::Error + Send + Sync>>;

    /// List all tenants, ordered by creation time descending.
    async fn list_tenants(&self) -> Result<Vec<Tenant>, Box<dyn std::error::Error + Send + Sync>>;

    /// Delete a tenant by ID. Returns `true` if the tenant was found and deleted.
    async fn delete_tenant(&mut self, id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;
}

/// PostgreSQL-backed tenant store using sqlx.
pub struct SqlxTenantStore {
    pool: Option<sqlx::PgPool>,
}

impl SqlxTenantStore {
    /// Create a new tenant store with the given database connection pool.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool: Some(pool) }
    }

    /// Get reference to the pool (for advanced usage).
    pub fn pool(&self) -> Option<&sqlx::PgPool> {
        self.pool.as_ref()
    }
}

impl Default for SqlxTenantStore {
    fn default() -> Self {
        Self::new_from_env().unwrap_or_else(|_| Self {
            pool: None,
        })
    }
}

impl SqlxTenantStore {
    /// Create a new store from DATABASE_URL environment variable.
    pub fn new_from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let _url = std::env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL not set")?;

        // This would be: sqlx::PgPool::connect(&url).await
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "SQLX tenant store requires DATABASE_URL and runtime setup",
        )))
    }
}

impl TenantStoreBackend for SqlxTenantStore {
    async fn create_tenant(&mut self, _name: &str) -> Result<Tenant, Box<dyn std::error::Error + Send + Sync>> {
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "SqlxTenantStore requires sqlx with DATABASE_URL and runtime setup",
        )))
    }

    async fn get_tenant(&self, _id: &str) -> Result<Option<Tenant>, Box<dyn std::error::Error + Send + Sync>> {
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "SqlxTenantStore requires sqlx with DATABASE_URL and runtime setup",
        )))
    }

    async fn list_tenants(&self) -> Result<Vec<Tenant>, Box<dyn std::error::Error + Send + Sync>> {
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "SqlxTenantStore requires sqlx with DATABASE_URL and runtime setup",
        )))
    }

    async fn delete_tenant(&mut self, _id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "SqlxTenantStore requires sqlx with DATABASE_URL and runtime setup",
        )))
    }
}

/// In-memory tenant store for testing/dev (no database required).
pub struct MemoryTenantStore {
    tenants: Vec<Tenant>,
}

impl MemoryTenantStore {
    /// Create a new in-memory tenant store.
    pub fn new() -> Self {
        Self { tenants: Vec::new() }
    }

    /// Get all stored tenants (unsorted).
    pub fn tenants(&self) -> &[Tenant] {
        &self.tenants
    }

    /// Clear all stored tenants.
    pub fn clear(&mut self) {
        self.tenants.clear();
    }
}

impl Default for MemoryTenantStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TenantStoreBackend for MemoryTenantStore {
    async fn create_tenant(
        &mut self,
        name: &str,
    ) -> Result<Tenant, Box<dyn std::error::Error + Send + Sync>> {
        // Check for duplicate names.
        if self.tenants.iter().any(|t| t.name == name) {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("Tenant with name '{name}' already exists"),
            )));
        }

        let tenant = Tenant::with_id(uuid::Uuid::new_v4().to_string(), name.to_string());
        self.tenants.push(tenant.clone());
        Ok(tenant)
    }

    async fn get_tenant(
        &self,
        id: &str,
    ) -> Result<Option<Tenant>, Box<dyn std::error::Error + Send + Sync>> {
        let tenant = self.tenants.iter().find(|t| t.id == id).cloned();
        Ok(tenant)
    }

    async fn list_tenants(&self) -> Result<Vec<Tenant>, Box<dyn std::error::Error + Send + Sync>> {
        let mut tenants = self.tenants.clone();
        tenants.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(tenants)
    }

    async fn delete_tenant(
        &mut self,
        id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let initial_len = self.tenants.len();
        self.tenants.retain(|t| t.id != id);
        Ok(self.tenants.len() < initial_len)
    }
}

/// Type alias for the tenant store (defaults to in-memory).
pub type TenantStore = MemoryTenantStore;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_store_create_and_get() {
        let mut store = MemoryTenantStore::new();

        let tenant = store.create_tenant("Acme Corp").await.unwrap();
        assert_eq!(tenant.name, "Acme Corp");
        assert!(!tenant.id.is_empty());

        let fetched = store.get_tenant(&tenant.id).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Acme Corp");
    }

    #[tokio::test]
    async fn test_memory_store_duplicate_name() {
        let mut store = MemoryTenantStore::new();

        store.create_tenant("Acme").await.unwrap();
        let result = store.create_tenant("Acme").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_memory_store_list() {
        let mut store = MemoryTenantStore::new();

        store.create_tenant("First").await.unwrap();
        store.create_tenant("Second").await.unwrap();

        let tenants = store.list_tenants().await.unwrap();
        assert_eq!(tenants.len(), 2);
    }

    #[tokio::test]
    async fn test_memory_store_delete() {
        let mut store = MemoryTenantStore::new();

        let tenant = store.create_tenant("ToDelete").await.unwrap();
        assert!(store.delete_tenant(&tenant.id).await.unwrap());
        assert!(!store.delete_tenant(&tenant.id).await.unwrap()); // already gone

        assert!(store.get_tenant(&tenant.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_memory_store_get_nonexistent() {
        let store = MemoryTenantStore::new();
        let result = store.get_tenant("nonexistent").await.unwrap();
        assert!(result.is_none());
    }
}
