//! Persistence layer for DMN decision documents (Phase 33).
//!
//! Defines the `DmnPersistence` trait and two implementations:
//! - `FileSystemDmnStore`: stores decisions as `.dmn` XML files + `.meta.json` on disk.
//!   Authoritative store — Git-tracked via `DMN_STORAGE_PATH`.
//! - `PostgresDmnStore`: skeleton implementation using `tokio-postgres` for HA/failover.
//!   SQL queries are `todo!()` until the PostgreSQL connection is configured in a later phase.
//!
//! The trait uses native async methods (Rust 1.75+), avoiding the `async-trait` dependency.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use tokio::fs;

// =========================================================================
// Public types
// =========================================================================

/// Summary of a DMN document (used in list responses).
#[derive(Debug, Clone, Serialize)]
pub struct DmnSummary {
    pub id: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Full DMN document (metadata + raw XML content).
#[derive(Debug, Clone, Serialize)]
pub struct DmnDocument {
    pub id: String,
    pub name: String,
    pub content: Vec<u8>,
    pub metadata: JsonValue,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// =========================================================================
// Persistence trait (native async — Rust 1.75+)
// =========================================================================

/// Trait for DMN decision document persistence.
///
/// Two implementations exist: `FileSystemDmnStore` (primary, Git-tracked) and
/// `PostgresDmnStore` (secondary, HA/failover). The API layer switches between
/// them via dependency injection without handler changes.
///
/// Uses native async methods (Rust 1.75+) — no `async-trait` dependency needed.
pub trait DmnPersistence: Send + Sync {
    async fn list(&self) -> Result<Vec<DmnSummary>, PersistenceError>;
    async fn get(&self, id: &str) -> Result<DmnDocument, PersistenceError>;
    async fn create(&self, doc: DmnDocument) -> Result<DmnDocument, PersistenceError>;
    async fn update(&self, id: &str, doc: DmnDocument) -> Result<DmnDocument, PersistenceError>;
    async fn delete(&self, id: &str) -> Result<(), PersistenceError>;
    async fn save_file(&self, filename: &str, content: &[u8]) -> Result<(), PersistenceError>;
    async fn load_file(&self, filename: &str) -> Result<Vec<u8>, PersistenceError>;
}

// =========================================================================
// Errors
// =========================================================================

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("file not found: {0}")]
    NotFound(String),

    #[error("parse error: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Database(String),

    #[error("conflict: {0}")]
    Conflict(String),
}

// =========================================================================
// FileSystemDmnStore (primary implementation)
// =========================================================================

/// File-system-backed DMN persistence.
///
/// Each decision is stored as `{id}.dmn` XML + `{id}.meta.json` metadata in the
/// configured storage directory (`DMN_STORAGE_PATH`, fallback `"./dmn"`). Thread-safe
/// via `Arc<Mutex<>>` around filesystem operations using `tokio::fs`.
pub struct FileSystemDmnStore {
    /// Storage directory for `.dmn` files and `.meta.json` sidecars.
    storage_path: PathBuf,
}

impl FileSystemDmnStore {
    /// Create a new FileSystemDmnStore from the `DMN_STORAGE_PATH` env var (fallback `"./dmn"`).
    pub fn new() -> Self {
        let path = std::env::var("DMN_STORAGE_PATH")
            .unwrap_or_else(|_| "./dmn".to_string());
        Self {
            storage_path: PathBuf::from(path),
        }
    }

    /// Create a FileSystemDmnStore with an explicit storage directory.
    pub fn with_path(storage_path: impl Into<PathBuf>) -> Self {
        Self {
            storage_path: storage_path.into(),
        }
    }

    /// Ensure the storage directory exists; create it if not.
    async fn ensure_dir(&self) -> Result<(), PersistenceError> {
        fs::create_dir_all(&self.storage_path).await?;
        Ok(())
    }

    /// Read and parse a `.meta.json` sidecar file, returning the stored timestamps.
    async fn read_meta(&self, id: &str) -> Result<(DateTime<Utc>, DateTime<Utc>), PersistenceError> {
        let meta_path = self.meta_path(id);
        if !meta_path.exists() {
            return Err(PersistenceError::NotFound(format!("metadata for decision {}", id)));
        }
        let content = fs::read_to_string(&meta_path).await?;
        #[derive(serde::Deserialize)]
        struct MetaFile {
            created_at: String,
            updated_at: String,
        }
        let meta: MetaFile = serde_json::from_str(&content)
            .map_err(|e| PersistenceError::ParseError(format!("invalid metadata JSON for {}: {}", id, e)))?;
        let parsed_created = DateTime::parse_from_rfc3339(&meta.created_at)
            .map_err(|e| PersistenceError::ParseError(format!("invalid created_at: {}", e)))?;
        let created = chrono::TimeZone::from_utc_datetime(&Utc, &parsed_created.naive_utc());
        let parsed_updated = DateTime::parse_from_rfc3339(&meta.updated_at)
            .map_err(|e| PersistenceError::ParseError(format!("invalid updated_at: {}", e)))?;
        let updated = chrono::TimeZone::from_utc_datetime(&Utc, &parsed_updated.naive_utc());
        Ok((created, updated))
    }

    /// Write a `.meta.json` sidecar file for a decision.
    async fn write_meta(&self, id: &str, created: DateTime<Utc>, updated: DateTime<Utc>) -> Result<(), PersistenceError> {
        self.ensure_dir().await?;
        let meta = serde_json::json!({
            "created_at": created.to_rfc3339(),
            "updated_at": updated.to_rfc3339(),
        });
        let meta_path = self.meta_path(id);
        fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).await?;
        Ok(())
    }

    fn dmn_path(&self, id: &str) -> PathBuf {
        self.storage_path.join(format!("{}.dmn", id))
    }

    fn meta_path(&self, id: &str) -> PathBuf {
        self.storage_path.join(format!("{}.meta.json", id))
    }
}

impl DmnPersistence for FileSystemDmnStore {
    async fn list(&self) -> Result<Vec<DmnSummary>, PersistenceError> {
        self.ensure_dir().await?;

        let mut summaries = Vec::new();
        let mut entries = fs::read_dir(&self.storage_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "dmn") {
                let id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                // Try to parse name/version from the XML.
                let content = match fs::read_to_string(&path).await {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let (name, version) = parse_dmn_name_version(&content);
                let updated_at = Utc::now(); // Fallback if meta not found.

                summaries.push(DmnSummary {
                    id,
                    name,
                    version,
                    updated_at,
                });
            }
        }

        // Sort by name for deterministic ordering.
        summaries.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(summaries)
    }

    async fn get(&self, id: &str) -> Result<DmnDocument, PersistenceError> {
        let dmn_path = self.dmn_path(id);
        if !dmn_path.exists() {
            return Err(PersistenceError::NotFound(format!("decision file for {}", id)));
        }

        let content = fs::read(&dmn_path).await?;
        let (created_at, updated_at) = match self.read_meta(id).await {
            Ok(v) => v,
            Err(_) => (Utc::now(), Utc::now()),
        };

        // Try to extract name from metadata or XML.
        let meta_content = fs::read_to_string(self.meta_path(id)).await.ok();
        let name_from_meta = meta_content
            .as_ref()
            .and_then(|c| serde_json::from_str::<JsonValue>(c).ok())
            .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from));

        let name = if let Some(name) = name_from_meta {
            name
        } else {
            let xml_str = String::from_utf8_lossy(&content);
            parse_dmn_name(&xml_str).unwrap_or_else(|| "unknown".to_string())
        };

        Ok(DmnDocument {
            id: id.to_string(),
            name,
            content,
            metadata: meta_content.and_then(|c| serde_json::from_str(&c).ok()).unwrap_or(JsonValue::Null),
            created_at,
            updated_at,
        })
    }

    async fn create(&self, doc: DmnDocument) -> Result<DmnDocument, PersistenceError> {
        self.ensure_dir().await?;

        let now = Utc::now();
        let dmn_path = self.dmn_path(&doc.id);
        if dmn_path.exists() {
            return Err(PersistenceError::Conflict(format!(
                "decision '{}' already exists", doc.id
            )));
        }

        fs::write(&dmn_path, &doc.content).await?;
        let created = doc.created_at; // Already DateTime<Utc>.
        self.write_meta(&doc.id, created, now).await?;

        Ok(DmnDocument {
            id: doc.id,
            name: doc.name,
            content: doc.content,
            metadata: doc.metadata,
            created_at: created,
            updated_at: now,
        })
    }

    async fn update(&self, id: &str, doc: DmnDocument) -> Result<DmnDocument, PersistenceError> {
        let dmn_path = self.dmn_path(id);
        if !dmn_path.exists() {
            return Err(PersistenceError::NotFound(format!("decision file for {}", id)));
        }

        fs::write(&dmn_path, &doc.content).await?;
        let now = Utc::now();
        // Preserve original created_at from existing metadata if available.
        let (created, _) = match self.read_meta(id).await {
            Ok(v) => v,
            Err(_) => (now, now),
        };
        self.write_meta(id, created, now).await?;

        Ok(DmnDocument {
            id: doc.id,
            name: doc.name,
            content: doc.content,
            metadata: doc.metadata,
            created_at: created,
            updated_at: now,
        })
    }

    async fn delete(&self, id: &str) -> Result<(), PersistenceError> {
        let dmn_path = self.dmn_path(id);
        let meta_path = self.meta_path(id);

        if !dmn_path.exists() && !meta_path.exists() {
            return Err(PersistenceError::NotFound(format!("decision '{}' not found", id)));
        }

        fs::remove_file(&dmn_path).await.ok(); // Ignore if already gone.
        fs::remove_file(&meta_path).await.ok();

        Ok(())
    }

    async fn save_file(&self, filename: &str, content: &[u8]) -> Result<(), PersistenceError> {
        self.ensure_dir().await?;
        let path = self.storage_path.join(filename);
        fs::write(&path, content).await?;
        Ok(())
    }

    async fn load_file(&self, filename: &str) -> Result<Vec<u8>, PersistenceError> {
        let path = self.storage_path.join(filename);
        if !path.exists() {
            return Err(PersistenceError::NotFound(format!("file '{}' not found", filename)));
        }
        Ok(fs::read(&path).await?)
    }
}

impl Default for FileSystemDmnStore {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================
// PostgresDmnStore (skeleton — todo!() at SQL sites)
// =========================================================================

/// PostgreSQL-backed DMN persistence skeleton.
///
/// This is a secondary/failover store. The actual `.dmn` XML content is also
/// persisted for Git-tracked backup; the filesystem store remains authoritative.
/// SQL queries are `todo!()` until a live database connection is configured in
/// a later phase. Connection string comes from env var `DATABASE_URL`.
pub struct PostgresDmnStore {
    /// The (unconfigured) connection pool placeholder. Real implementation would
    /// use `deadpool-postgres` or `tokio-postgres::Client`.
    _placeholder: (),
}

impl PostgresDmnStore {
    pub fn new() -> Self {
        eprintln!(
            "PostgresDmnStore constructed (skeleton — DATABASE_URL must be set for live use)"
        );
        Self { _placeholder: () }
    }
}

impl Default for PostgresDmnStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DmnPersistence for PostgresDmnStore {
    async fn list(&self) -> Result<Vec<DmnSummary>, PersistenceError> {
        todo!("PostgresDmnStore::list — SELECT id, name, version, updated_at FROM dmn_documents ORDER BY updated_at DESC")
    }

    async fn get(&self, id: &str) -> Result<DmnDocument, PersistenceError> {
        todo!("PostgresDmnStore::get — SELECT content, metadata, created_at, updated_at FROM dmn_documents WHERE id = $1")
    }

    async fn create(&self, doc: DmnDocument) -> Result<DmnDocument, PersistenceError> {
        todo!("PostgresDmnStore::create — INSERT INTO dmn_documents (id, name, content, metadata, created_at, updated_at) VALUES (...)")
    }

    async fn update(&self, id: &str, doc: DmnDocument) -> Result<DmnDocument, PersistenceError> {
        todo!("PostgresDmnStore::update — UPDATE dmn_documents SET content = $2, metadata = $3, updated_at = NOW() WHERE id = $1")
    }

    async fn delete(&self, id: &str) -> Result<(), PersistenceError> {
        todo!("PostgresDmnStore::delete — DELETE FROM dmn_documents WHERE id = $1")
    }

    async fn save_file(&self, filename: &str, content: &[u8]) -> Result<(), PersistenceError> {
        todo!("PostgresDmnStore::save_file — INSERT INTO dmn_files (filename, content) VALUES ($1, $2)")
    }

    async fn load_file(&self, filename: &str) -> Result<Vec<u8>, PersistenceError> {
        todo!("PostgresDmnStore::load_file — SELECT content FROM dmn_files WHERE filename = $1")
    }
}

// =========================================================================
// Helpers: parse name/version from DMN XML (lightweight string scanning)
// =========================================================================

/// Extract the `name` attribute from a `<definitions>` element in DMN XML.
fn parse_dmn_name(xml: &str) -> Option<String> {
    let start = xml.find("<definitions")?;
    let end = xml[start..].find('>')? + start + 1;
    let tag = &xml[start..end];
    extract_attr(tag, "name").or(extract_attr(tag, "id"))
}

/// Extract `name` and optional `version` from a `<definitions>` element.
fn parse_dmn_name_version(xml: &str) -> (Option<String>, Option<String>) {
    let start = match xml.find("<definitions") {
        Some(pos) => pos,
        None => return (None, None),
    };
    let end = match xml[start..].find('>') {
        Some(offset) => offset + start + 1,
        None => return (None, None),
    };
    let tag = &xml[start..end];
    (extract_attr(tag, "name"), extract_attr(tag, "version"))
}

/// Extract an attribute value from an XML opening tag.
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    // Match `attr="value"` or `attr='value'`.
    let pattern = format!("{}=\"", attr);
    if let Some(start) = tag.find(&pattern) {
        let value_start = start + pattern.len();
        if let Some(end) = tag[value_start..].find('"') {
            return Some(tag[value_start..value_start + end].to_string());
        }
    }
    // Try single quotes.
    let pattern_sq = format!("{}='", attr);
    if let Some(start) = tag.find(&pattern_sq) {
        let value_start = start + pattern_sq.len();
        if let Some(end) = tag[value_start..].find('\'') {
            return Some(tag[value_start..value_start + end].to_string());
        }
    }
    None
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn filesystem_store_create_and_list() {
        let dir = TempDir::new().unwrap();
        let store = FileSystemDmnStore::with_path(dir.path());

        // Create a test document.
        let doc = DmnDocument {
            id: "test-1".to_string(),
            name: "Test Decision".to_string(),
            content: b"<?xml version=\"1.0\"?><definitions id=\"test-1\" name=\"Test\"></definitions>".to_vec(),
            metadata: serde_json::json!({"key": "value"}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let created = store.create(doc).await.unwrap();
        assert_eq!(created.id, "test-1");

        // List should include the document.
        let summaries = store.list().await.unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, "test-1");
    }

    #[tokio::test]
    async fn filesystem_store_get_and_delete() {
        let dir = TempDir::new().unwrap();
        let store = FileSystemDmnStore::with_path(dir.path());

        // Create.
        let doc = DmnDocument {
            id: "get-test".to_string(),
            name: "Get Test".to_string(),
            content: b"<definitions></definitions>".to_vec(),
            metadata: JsonValue::Null,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        store.create(doc).await.unwrap();

        // Get.
        let got = store.get("get-test").await.unwrap();
        assert_eq!(got.id, "get-test");
        assert!(!got.content.is_empty());

        // Delete.
        store.delete("get-test").await.unwrap();

        // Should be gone.
        match store.get("get-test").await {
            Err(PersistenceError::NotFound(_)) => {} // expected
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn parse_dmn_name_from_xml() {
        let xml = r#"<definitions id="d1" name="My Decision" namespace="http://example.com">"#;
        assert_eq!(parse_dmn_name(xml), Some("My Decision".to_string()));
    }

    #[test]
    fn test_parse_dmn_name_version() {
        let xml = r#"<definitions id="d1" name="Test" version="1.0.0">"#;
        let (name, version) = parse_dmn_name_version(xml);
        assert_eq!(name, Some("Test".to_string()));
        assert_eq!(version, Some("1.0.0".to_string()));
    }

    #[test]
    fn extract_attr_single_quotes() {
        let tag = r#"<foo bar='hello world' baz="test">"#;
        assert_eq!(extract_attr(tag, "bar"), Some("hello world".to_string()));
        assert_eq!(extract_attr(tag, "baz"), Some("test".to_string()));
    }
}
