//! Tenant storage for multi-tenancy support.
//!
//! The trait is the seam; the implementation that ships is in memory. There is no PostgreSQL
//! backend: one existed here and answered `NotConnected` to every call while pretending to be a
//! store, which is worse than not having one. `migrations/20_01_tenant_schema.sql` describes the
//! schema a real one would use.

#![allow(async_fn_in_trait)]

use crate::governance::tenant::types::Tenant;

/// Trait for tenant storage backends.
pub trait TenantStoreBackend {
    /// Create a new tenant. Returns the created tenant with generated id and timestamp.
    async fn create_tenant(
        &mut self,
        name: &str,
    ) -> Result<Tenant, Box<dyn std::error::Error + Send + Sync>>;

    /// Retrieve a tenant by ID. Returns `None` if not found.
    async fn get_tenant(
        &self,
        id: &str,
    ) -> Result<Option<Tenant>, Box<dyn std::error::Error + Send + Sync>>;

    /// List all tenants, ordered by creation time descending.
    async fn list_tenants(&self) -> Result<Vec<Tenant>, Box<dyn std::error::Error + Send + Sync>>;

    /// Delete a tenant by ID. Returns `true` if the tenant was found and deleted.
    async fn delete_tenant(
        &mut self,
        id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;
}

/// In-memory tenant store for testing/dev (no database required).
pub struct MemoryTenantStore {
    tenants: Vec<Tenant>,
}

/// Why a tenant operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TenantStoreError {
    /// Another tenant already uses this name.
    DuplicateName(String),
    /// The name was empty or whitespace-only.
    EmptyName,
}

impl std::fmt::Display for TenantStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateName(name) => {
                write!(f, "Tenant with name '{name}' already exists")
            }
            Self::EmptyName => write!(f, "name is required and cannot be empty"),
        }
    }
}

impl std::error::Error for TenantStoreError {}

impl MemoryTenantStore {
    /// Create a new in-memory tenant store.
    pub fn new() -> Self {
        Self {
            tenants: Vec::new(),
        }
    }

    /// Get all stored tenants (unsorted).
    pub fn tenants(&self) -> &[Tenant] {
        &self.tenants
    }

    /// Clear all stored tenants.
    pub fn clear(&mut self) {
        self.tenants.clear();
    }

    /// Create a tenant (synchronous core used by the async trait impl and by
    /// callers that already hold a lock, such as the HTTP layer).
    ///
    /// Rejects empty names and duplicate names.
    pub fn create(&mut self, name: &str) -> Result<Tenant, TenantStoreError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(TenantStoreError::EmptyName);
        }
        if self.tenants.iter().any(|t| t.name == name) {
            return Err(TenantStoreError::DuplicateName(name.to_string()));
        }
        let tenant = Tenant::new(name);
        self.tenants.push(tenant.clone());
        Ok(tenant)
    }

    /// Look up a tenant by id.
    pub fn get(&self, id: &str) -> Option<&Tenant> {
        self.tenants.iter().find(|t| t.id == id)
    }

    /// All tenants, newest first.
    pub fn list(&self) -> Vec<Tenant> {
        let mut tenants = self.tenants.clone();
        tenants.sort_by_key(|tenant| std::cmp::Reverse(tenant.created_at));
        tenants
    }

    /// Delete a tenant by id. Returns `true` when it existed.
    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.tenants.len();
        self.tenants.retain(|t| t.id != id);
        self.tenants.len() < before
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
        self.create(name)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn get_tenant(
        &self,
        id: &str,
    ) -> Result<Option<Tenant>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.get(id).cloned())
    }

    async fn list_tenants(&self) -> Result<Vec<Tenant>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.list())
    }

    async fn delete_tenant(
        &mut self,
        id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.delete(id))
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
