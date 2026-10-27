//! PostgreSQL-backed DMN persistence.
//!
//! Opt in with the `postgres` feature: nothing compiles a database driver otherwise. The
//! `FileSystemDmnStore` remains the authoritative store — this one exists for deployments that want
//! documents in a database alongside, or instead of, `DMN_STORAGE_PATH`.
//!
//! The schema lives in `migrations/21_01_dmn_documents.sql` and is applied by [`PostgresDmnStore::connect`]
//! with `IF NOT EXISTS`, so connecting twice is harmless and an existing table is left alone.
//!
//! Every method here runs real SQL. This module replaced one whose methods were `todo!()` — it was
//! never constructed, and a store that panics if you call it is not a store.

use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;

use crate::{DmnDocument, DmnPersistence, DmnSummary, PersistenceError};

/// The DDL this store applies on connect.
const SCHEMA: &str = include_str!("../../migrations/21_01_dmn_documents.sql");

/// A DMN store backed by PostgreSQL.
pub struct PostgresDmnStore {
    pool: sqlx::PgPool,
}

impl PostgresDmnStore {
    /// Connect to `url` and make sure the schema exists.
    pub async fn connect(url: &str) -> Result<Self, PersistenceError> {
        let pool = sqlx::PgPool::connect(url)
            .await
            .map_err(|error| PersistenceError::Database(error.to_string()))?;
        let store = Self { pool };
        store.ensure_schema().await?;
        Ok(store)
    }

    /// Connect using `DATABASE_URL`.
    pub async fn from_env() -> Result<Self, PersistenceError> {
        let url = std::env::var("DATABASE_URL")
            .map_err(|_| PersistenceError::Database("DATABASE_URL is not set".to_string()))?;
        Self::connect(&url).await
    }

    /// Apply the DDL. Idempotent: every statement is `IF NOT EXISTS`.
    pub async fn ensure_schema(&self) -> Result<(), PersistenceError> {
        // Several statements in one round trip. `sqlx::query` accepts them because the schema file is
        // ours and contains no user input.
        sqlx::raw_sql(SCHEMA)
            .execute(&self.pool)
            .await
            .map_err(|error| PersistenceError::Database(error.to_string()))?;
        Ok(())
    }

    /// The pool, for callers that need to run their own queries against the same database.
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
}

/// A `.dmn` file *is* a decision document in either backend.
///
/// The filesystem store proves it: `save_file("x.dmn", …)` writes `x.dmn`, and `list()` finds it by
/// scanning `*.dmn`, so the file view and the document view are the same data. Keeping two tables
/// here without this rule would make an imported document invisible to `list` — a difference between
/// backends that no test would catch unless something imported and then listed.
fn decision_id(filename: &str) -> Option<&str> {
    filename.strip_suffix(".dmn")
}

/// A `list` row: id, name, version, updated_at.
type ListRow = (String, Option<String>, Option<String>, DateTime<Utc>);

/// A `get` row: id, name, content, metadata, created_at, updated_at.
type DocumentRow = (
    String,
    Option<String>,
    Vec<u8>,
    JsonValue,
    DateTime<Utc>,
    DateTime<Utc>,
);

impl DmnPersistence for PostgresDmnStore {
    async fn list(&self) -> Result<Vec<DmnSummary>, PersistenceError> {
        let rows: Vec<ListRow> = sqlx::query_as(
            "SELECT id, name, version, updated_at FROM dmn_documents ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| PersistenceError::Database(error.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|(id, name, version, updated_at)| DmnSummary {
                id,
                name,
                version,
                updated_at,
            })
            .collect())
    }

    async fn get(&self, id: &str) -> Result<DmnDocument, PersistenceError> {
        let row: Option<DocumentRow> = sqlx::query_as(
            "SELECT id, name, content, metadata, created_at, updated_at \
                 FROM dmn_documents WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| PersistenceError::Database(error.to_string()))?;

        match row {
            Some((id, name, content, metadata, created_at, updated_at)) => Ok(DmnDocument {
                id,
                name: name.unwrap_or_default(),
                content,
                metadata,
                created_at,
                updated_at,
            }),
            None => Err(PersistenceError::NotFound(id.to_string())),
        }
    }

    async fn create(&self, doc: DmnDocument) -> Result<DmnDocument, PersistenceError> {
        // An existing id is a conflict, not an upsert: silently overwriting a decision document is
        // how a user loses work they did not ask to lose.
        let inserted: Option<(DateTime<Utc>, DateTime<Utc>)> = sqlx::query_as(
            "INSERT INTO dmn_documents (id, name, version, content, metadata, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (id) DO NOTHING \
             RETURNING created_at, updated_at",
        )
        .bind(&doc.id)
        .bind(&doc.name)
        .bind(doc.metadata.get("version").and_then(JsonValue::as_str))
        .bind(&doc.content)
        .bind(&doc.metadata)
        .bind(doc.created_at)
        .bind(doc.updated_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| PersistenceError::Database(error.to_string()))?;

        match inserted {
            Some((created_at, updated_at)) => Ok(DmnDocument {
                created_at,
                updated_at,
                ..doc
            }),
            None => Err(PersistenceError::Conflict(format!(
                "a DMN document with id {} already exists",
                doc.id
            ))),
        }
    }

    async fn update(&self, id: &str, doc: DmnDocument) -> Result<DmnDocument, PersistenceError> {
        let updated: Option<(DateTime<Utc>,)> = sqlx::query_as(
            "UPDATE dmn_documents \
             SET content = $2, metadata = $3, name = $4, version = $5, updated_at = $6 \
             WHERE id = $1 \
             RETURNING updated_at",
        )
        .bind(id)
        .bind(&doc.content)
        .bind(&doc.metadata)
        .bind(&doc.name)
        .bind(doc.metadata.get("version").and_then(JsonValue::as_str))
        .bind(Utc::now())
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| PersistenceError::Database(error.to_string()))?;

        match updated {
            Some((updated_at,)) => {
                let stored = self.get(id).await?;
                Ok(DmnDocument {
                    updated_at,
                    ..stored
                })
            }
            None => Err(PersistenceError::NotFound(id.to_string())),
        }
    }

    async fn delete(&self, id: &str) -> Result<(), PersistenceError> {
        let deleted: Option<(String,)> =
            sqlx::query_as("DELETE FROM dmn_documents WHERE id = $1 RETURNING id")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| PersistenceError::Database(error.to_string()))?;

        match deleted {
            Some(_) => Ok(()),
            None => Err(PersistenceError::NotFound(id.to_string())),
        }
    }

    async fn save_file(&self, filename: &str, content: &[u8]) -> Result<(), PersistenceError> {
        if let Some(id) = decision_id(filename) {
            // Same data the document methods use, so an imported model is listed and deletable.
            let text = String::from_utf8_lossy(content);
            let (name, version) = crate::parse_dmn_name_version(&text);
            sqlx::query(
                "INSERT INTO dmn_documents (id, name, version, content, metadata, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, '{}'::jsonb, NOW(), NOW()) \
                 ON CONFLICT (id) DO UPDATE SET content = EXCLUDED.content, name = EXCLUDED.name, \
                     version = EXCLUDED.version, updated_at = NOW()",
            )
            .bind(id)
            .bind(name)
            .bind(version)
            .bind(content)
            .execute(&self.pool)
            .await
            .map_err(|error| PersistenceError::Database(error.to_string()))?;
            return Ok(());
        }

        sqlx::query(
            "INSERT INTO dmn_files (filename, content, updated_at) VALUES ($1, $2, NOW()) \
             ON CONFLICT (filename) DO UPDATE SET content = EXCLUDED.content, updated_at = NOW()",
        )
        .bind(filename)
        .bind(content)
        .execute(&self.pool)
        .await
        .map_err(|error| PersistenceError::Database(error.to_string()))?;
        Ok(())
    }

    async fn load_file(&self, filename: &str) -> Result<Vec<u8>, PersistenceError> {
        if let Some(id) = decision_id(filename) {
            // Read the document table first: that is where `save_file` put a `.dmn` file.
            let document: Option<(Vec<u8>,)> =
                sqlx::query_as("SELECT content FROM dmn_documents WHERE id = $1")
                    .bind(id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|error| PersistenceError::Database(error.to_string()))?;
            return match document {
                Some((content,)) => Ok(content),
                None => Err(PersistenceError::NotFound(filename.to_string())),
            };
        }

        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT content FROM dmn_files WHERE filename = $1")
                .bind(filename)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| PersistenceError::Database(error.to_string()))?;

        match row {
            Some((content,)) => Ok(content),
            None => Err(PersistenceError::NotFound(filename.to_string())),
        }
    }
}
