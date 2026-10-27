//! Where the tenant registry lives.
//!
//! Tenants are the organisations that own rules; the registry is what makes `X-Tenant-ID` meaningful.
//! It was a `MemoryTenantStore` behind a `Mutex`, so a restart forgot every tenant and the rules
//! scoped to them. This makes the storage a choice: in memory (the default, unchanged) or PostgreSQL
//! (`TENANT_PERSISTENCE=postgres`, `--features postgres`).
//!
//! The synchronous core in `engine::governance::tenant::MemoryTenantStore` stays as it is — it is the
//! in-memory implementation, and its `create`/`get`/`list`/`delete` are what the database variant must
//! answer identically. Both are reached through this enum, so a handler never learns which is behind it.

use engine::governance::tenant::{MemoryTenantStore, Tenant, TenantStoreError};
use std::sync::{Arc, Mutex};

/// The tenant registry's storage.
pub enum TenantBackend {
    /// A process-lifetime registry: fast, and gone on restart.
    Memory(Arc<Mutex<MemoryTenantStore>>),
    /// A PostgreSQL table, which outlives the process.
    #[cfg(feature = "postgres")]
    Postgres(crate::v1::tenant_postgres::PostgresTenantStore),
}

impl TenantBackend {
    /// The in-memory registry, which is what an unconfigured server has always used.
    pub fn in_memory() -> Self {
        Self::Memory(Arc::new(Mutex::new(MemoryTenantStore::new())))
    }

    /// The backend named by `TENANT_PERSISTENCE`: `memory` (the default) or `postgres`.
    ///
    /// # Errors
    /// Returns an error when `postgres` is requested but the feature is not compiled, when
    /// `DATABASE_URL` is unset, when the connection fails, or when the variable names an unknown
    /// backend — rather than silently starting with an empty registry a deployment believes is
    /// persisted.
    pub async fn from_environment() -> Result<Self, String> {
        let requested =
            std::env::var("TENANT_PERSISTENCE").unwrap_or_else(|_| "memory".to_string());
        match requested.as_str() {
            "memory" | "" => Ok(Self::in_memory()),
            "postgres" => {
                #[cfg(feature = "postgres")]
                {
                    let store = crate::v1::tenant_postgres::PostgresTenantStore::from_env()
                        .await
                        .map_err(|error| {
                            format!("TENANT_PERSISTENCE=postgres, but the registry could not connect: {error}")
                        })?;
                    Ok(Self::Postgres(store))
                }
                #[cfg(not(feature = "postgres"))]
                {
                    Err("TENANT_PERSISTENCE=postgres, but this build has no `postgres` feature; \
                         rebuild with --features postgres rather than silently starting empty"
                        .to_string())
                }
            }
            other => Err(format!(
                "TENANT_PERSISTENCE={other} is not a registry this build implements; use `memory` or `postgres`"
            )),
        }
    }

    /// The backend's name, for logs.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Memory(_) => "memory",
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => "postgres",
        }
    }

    /// Register a tenant. Rejects blank and duplicate names exactly as the memory store does.
    pub async fn create(&self, name: &str) -> Result<Tenant, TenantStoreError> {
        match self {
            Self::Memory(store) => {
                let mut guard = store
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                guard.create(name)
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => store.create(name).await,
        }
    }

    /// One tenant by id.
    pub async fn get(&self, id: &str) -> Result<Option<Tenant>, TenantStoreError> {
        match self {
            Self::Memory(store) => {
                let guard = store
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                Ok(guard.get(id).cloned())
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => store.get(id).await,
        }
    }

    /// Every tenant, newest first.
    pub async fn list(&self) -> Result<Vec<Tenant>, TenantStoreError> {
        match self {
            Self::Memory(store) => {
                let guard = store
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                Ok(guard.list())
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => store.list().await,
        }
    }

    /// Remove a tenant. `true` when it existed.
    pub async fn delete(&self, id: &str) -> Result<bool, TenantStoreError> {
        match self {
            Self::Memory(store) => {
                let mut guard = store
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                Ok(guard.delete(id))
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(store) => store.delete(id).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn the_memory_registry_keeps_the_store_semantics() {
        let backend = TenantBackend::in_memory();
        assert_eq!(backend.kind(), "memory");

        let acme = backend.create("Acme").await.expect("created");
        assert_eq!(acme.name, "Acme");
        assert!(
            matches!(
                backend.create("Acme").await,
                Err(TenantStoreError::DuplicateName(_))
            ),
            "a duplicate name is rejected"
        );
        assert!(
            matches!(
                backend.create("   ").await,
                Err(TenantStoreError::EmptyName)
            ),
            "a blank name is rejected"
        );

        assert_eq!(
            backend
                .get(&acme.id)
                .await
                .expect("looked up")
                .map(|tenant| tenant.name),
            Some("Acme".to_string())
        );
        assert_eq!(backend.list().await.expect("listed").len(), 1);
        assert!(
            backend.delete(&acme.id).await.expect("deleted"),
            "the first delete removes it"
        );
        assert!(
            !backend.delete(&acme.id).await.expect("deleted"),
            "the second reports it was gone"
        );
        assert!(backend.get(&acme.id).await.expect("looked up").is_none());
    }
}
