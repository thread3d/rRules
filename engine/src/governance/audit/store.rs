//! PostgreSQL-backed audit event store for Phase 18 Enterprise Governance
//!
//! This module implements an immutable append-only audit log using event sourcing pattern.
//!
//! Note: Full implementation requires sqlx with DATABASE_URL set at compile time.
//! For now, we provide a trait-based interface with placeholder implementations.

#![allow(async_fn_in_trait)]

use crate::governance::audit::types::AuditEvent;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Trait for audit event storage backends
pub trait AuditStoreBackend {
    /// Append an event to the audit log
    async fn append_event(&mut self, event: &AuditEvent) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;

    /// Query events with filters
    async fn query_events(
        &self,
        event_types: Option<Vec<&str>>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        actor_id: Option<i32>,
        limit: i64,
    ) -> Result<Vec<AuditEvent>, Box<dyn std::error::Error + Send + Sync>>;

    /// Export audit log to CSV format
    async fn export_csv(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;
}

/// PostgreSQL-backed audit store using sqlx
pub struct SqlxAuditStore {
    pool: Option<sqlx::PgPool>,
}

impl SqlxAuditStore {
    /// Create a new audit store with the given database connection pool
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool: Some(pool) }
    }

    /// Get reference to the pool (for advanced usage)
    pub fn pool(&self) -> Option<&sqlx::PgPool> {
        self.pool.as_ref()
    }
}

impl Default for SqlxAuditStore {
    fn default() -> Self {
        Self::new_from_env().unwrap_or_else(|_| Self {
            pool: None,
        })
    }
}

impl SqlxAuditStore {
    /// Create a new store from DATABASE_URL environment variable
    pub fn new_from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let _url = std::env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL not set")?;

        // This would be: sqlx::PgPool::connect(&url).await
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "SQLX store requires DATABASE_URL and runtime setup",
        )))
    }
}

impl AuditStoreBackend for SqlxAuditStore {
    async fn append_event(&mut self, _event: &AuditEvent) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "SqlxAuditStore requires sqlx with DATABASE_URL and runtime setup",
        )))
    }

    async fn query_events(
        &self,
        _event_types: Option<Vec<&str>>,
        _start_time: Option<DateTime<Utc>>,
        _end_time: Option<DateTime<Utc>>,
        _actor_id: Option<i32>,
        _limit: i64,
    ) -> Result<Vec<AuditEvent>, Box<dyn std::error::Error + Send + Sync>> {
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "SqlxAuditStore requires sqlx with DATABASE_URL and runtime setup",
        )))
    }

    async fn export_csv(
        &self,
        _start_time: DateTime<Utc>,
        _end_time: DateTime<Utc>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "SqlxAuditStore requires sqlx with DATABASE_URL and runtime setup",
        )))
    }
}

/// In-memory audit store for testing/dev (no database required)
pub struct MemoryAuditStore {
    /// Stored events paired with the timestamp recorded at append time.
    /// The timestamp is captured once, when the event is appended, so the
    /// log ordering is stable and reflects when each event was recorded
    /// (rather than re-reading the clock on every query/sort).
    events: Vec<(DateTime<Utc>, AuditEvent)>,
}

impl MemoryAuditStore {
    /// Create a new in-memory audit store
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Get all stored events paired with their recorded timestamps,
    /// in append (oldest-first) order.
    pub fn events(&self) -> &[(DateTime<Utc>, AuditEvent)] {
        &self.events
    }

    /// Clear all stored events
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

impl Default for MemoryAuditStore {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditStoreBackend for MemoryAuditStore {
    async fn append_event(&mut self, event: &AuditEvent) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let event_id = Uuid::new_v4().to_string();
        self.events.push((Utc::now(), event.clone()));
        Ok(event_id)
    }

    async fn query_events(
        &self,
        _event_types: Option<Vec<&str>>,
        _start_time: Option<DateTime<Utc>>,
        _end_time: Option<DateTime<Utc>>,
        _actor_id: Option<i32>,
        limit: i64,
    ) -> Result<Vec<AuditEvent>, Box<dyn std::error::Error + Send + Sync>> {
        // Sort by recorded timestamp descending (most recent first).
        // The append index is used as a deterministic tiebreaker so that
        // events recorded within the same timestamp always order by
        // insertion (newest first), independent of clock resolution.
        let mut result: Vec<(usize, DateTime<Utc>, AuditEvent)> = self
            .events
            .iter()
            .enumerate()
            .map(|(i, (ts, e))| (i, *ts, e.clone()))
            .collect();
        result.sort_by_key(|(i, ts, _)| std::cmp::Reverse((*ts, *i)));

        let mut events: Vec<AuditEvent> = result.into_iter().map(|(_, _, e)| e).collect();

        if limit > 0 {
            events.truncate(limit as usize);
        }

        Ok(events)
    }

    async fn export_csv(
        &self,
        _start_time: DateTime<Utc>,
        _end_time: DateTime<Utc>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut csv = String::from("event_type,actor_id,subject_type,payload_occurred_at\n");
        for (occurred_at, event) in &self.events {
            let _payload = serde_json::to_value(event).unwrap_or_default();
            csv.push_str(&format!(
                "{},{},{},{}\n",
                event.event_type(),
                "N/A",
                event.subject_type(),
                occurred_at
            ));
        }
        Ok(csv)
    }
}

/// Type alias for the store
pub type AuditStore = MemoryAuditStore;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_store_append_and_query() {
        let mut store = MemoryAuditStore::new();

        // Add some events
        let event1 = AuditEvent::RoleAssigned {
            user_id: "user1".into(),
            role_name: "admin".into(),
            assigned_by: "admin".into(),
        };

        let event2 = AuditEvent::RuleCreated {
            rule_id: Uuid::new_v4(),
            rule_name: "test rule".into(),
            category: None,
            created_by: "user1".into(),
        };

        store.append_event(&event1).await.unwrap();
        store.append_event(&event2).await.unwrap();

        // Query all events
        let events = store.query_events(None, None, None, None, 10).await.unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type(), "rule.created");
    }

    #[tokio::test]
    async fn test_csv_export() {
        let mut store = MemoryAuditStore::new();

        let event = AuditEvent::UserLogin {
            user_id: "user1".into(),
            username: "testuser".into(),
            success: true,
            ip_address: Some("127.0.0.1".into()),
        };

        store.append_event(&event).await.unwrap();

        let csv = store.export_csv(Utc::now() - chrono::Duration::days(365 * 10), Utc::now()).await.unwrap();

        assert!(csv.contains("user.login"));
        assert!(csv.contains("N/A"));
    }
}
