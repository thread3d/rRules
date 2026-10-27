//! The tenant registry in PostgreSQL.
//!
//! Answers exactly what `MemoryTenantStore` answers — trimmed names, rejecting blanks and duplicates,
//! newest first, `true` from the first delete and `false` from the next — because the two are
//! interchangeable behind `TenantBackend` and a deployment should not discover the difference in
//! production.
//!
//! The table is the one `migrations/20_01_tenant_schema.sql` already described; the statement below is
//! applied idempotently on connect so the store owns its schema.

use chrono::{DateTime, Utc};
use engine::governance::tenant::{Tenant, TenantStoreError};

/// The DDL this store applies on connect.
const SCHEMA: &str = include_str!("../../../migrations/20_01_tenant_schema.sql");

/// A tenant registry backed by PostgreSQL.
pub struct PostgresTenantStore {
    pool: sqlx::PgPool,
}

impl PostgresTenantStore {
    /// Connect to `url` and make sure the table exists.
    pub async fn connect(url: &str) -> Result<Self, String> {
        let pool = sqlx::PgPool::connect(url)
            .await
            .map_err(|error| format!("could not connect: {error}"))?;
        sqlx::raw_sql(SCHEMA)
            .execute(&pool)
            .await
            .map_err(|error| format!("could not apply the schema: {error}"))?;
        Ok(Self { pool })
    }

    /// Connect using `DATABASE_URL`.
    pub async fn from_env() -> Result<Self, String> {
        let url =
            std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set".to_string())?;
        Self::connect(&url).await
    }

    /// The pool, for callers that need to run their own queries against the same database.
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    pub async fn create(&self, name: &str) -> Result<Tenant, TenantStoreError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(TenantStoreError::EmptyName);
        }

        // `id` is a UUID string, as `Tenant::new` generates; the table's `VARCHAR(36)` holds it.
        let tenant = Tenant::new(name);
        let inserted: Option<(String, String, DateTime<Utc>)> = sqlx::query_as(
            "INSERT INTO tenants (id, name, created_at) VALUES ($1, $2, $3) \
             ON CONFLICT (name) DO NOTHING \
             RETURNING id, name, created_at",
        )
        .bind(&tenant.id)
        .bind(&tenant.name)
        .bind(tenant.created_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(database_error)?;

        match inserted {
            Some((id, name, created_at)) => Ok(Tenant {
                id,
                name,
                created_at,
            }),
            // The only conflict the table can raise is the unique name.
            None => Err(TenantStoreError::DuplicateName(name.to_string())),
        }
    }

    pub async fn get(&self, id: &str) -> Result<Option<Tenant>, TenantStoreError> {
        let row: Option<(String, String, DateTime<Utc>)> =
            sqlx::query_as("SELECT id, name, created_at FROM tenants WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(database_error)?;
        Ok(row.map(|(id, name, created_at)| Tenant {
            id,
            name,
            created_at,
        }))
    }

    pub async fn list(&self) -> Result<Vec<Tenant>, TenantStoreError> {
        let rows: Vec<(String, String, DateTime<Utc>)> =
            sqlx::query_as("SELECT id, name, created_at FROM tenants ORDER BY created_at DESC")
                .fetch_all(&self.pool)
                .await
                .map_err(database_error)?;
        Ok(rows
            .into_iter()
            .map(|(id, name, created_at)| Tenant {
                id,
                name,
                created_at,
            })
            .collect())
    }

    pub async fn delete(&self, id: &str) -> Result<bool, TenantStoreError> {
        let deleted: Option<(String,)> =
            sqlx::query_as("DELETE FROM tenants WHERE id = $1 RETURNING id")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(database_error)?;
        Ok(deleted.is_some())
    }
}

/// A database failure: the storage is unavailable, which the HTTP layer answers with 503.
fn database_error(error: sqlx::Error) -> TenantStoreError {
    TenantStoreError::Backend(error.to_string())
}
