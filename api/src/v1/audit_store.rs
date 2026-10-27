//! Where the audit trail lives.
//!
//! The trail is the record an auditor reads: who changed what, when, and with what result. It was a
//! `HashMap` behind a `Mutex`, which means it was also nothing after a restart — a poor property for
//! evidence. This module makes the storage a choice: in memory (the default, unchanged) or PostgreSQL
//! (`AUDIT_PERSISTENCE=postgres`, `--features postgres`).
//!
//! Recording is async because a database write is; filtering is pushed into SQL when the backend is a
//! database and done in the process when it is not, so the two answer the same question rather than
//! one of them loading everything.

use crate::v1::audit::{AuditAction, ResourceType, StoredAuditEvent};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// The filter a reader narrows the trail by. `None` means "every".
#[derive(Debug, Default, Clone)]
pub struct AuditFilter {
    pub resource_type: Option<ResourceType>,
    pub action: Option<AuditAction>,
    pub user_id: Option<String>,
}

impl AuditFilter {
    fn matches(&self, event: &StoredAuditEvent) -> bool {
        if let Some(resource_type) = &self.resource_type {
            if &event.resource_type != resource_type {
                return false;
            }
        }
        if let Some(action) = &self.action {
            if &event.action != action {
                return false;
            }
        }
        if let Some(user_id) = &self.user_id {
            if event.user_id.as_ref() != Some(user_id) {
                return false;
            }
        }
        true
    }
}

/// The audit trail's storage.
pub enum AuditBackend {
    /// A process-lifetime trail: fast, and gone on restart.
    Memory(Arc<Mutex<HashMap<Uuid, StoredAuditEvent>>>),
    /// A PostgreSQL table, which outlives the process.
    #[cfg(feature = "postgres")]
    Postgres(crate::v1::audit_postgres::PostgresAuditTrail),
}

impl AuditBackend {
    /// The in-memory trail, which is what an unconfigured server has always used.
    pub fn in_memory() -> Self {
        Self::Memory(Arc::new(Mutex::new(HashMap::new())))
    }

    /// The backend named by `AUDIT_PERSISTENCE`: `memory` (the default) or `postgres`.
    ///
    /// # Errors
    /// Returns an error when `postgres` is requested but the feature is not compiled, when
    /// `DATABASE_URL` is unset, when the connection fails, or when the variable names an unknown
    /// backend. Falling back silently would leave an auditor reading an empty trail while the
    /// deployment believed otherwise.
    pub async fn from_environment() -> Result<Self, String> {
        let requested = std::env::var("AUDIT_PERSISTENCE").unwrap_or_else(|_| "memory".to_string());
        match requested.as_str() {
            "memory" | "" => Ok(Self::in_memory()),
            "postgres" => {
                #[cfg(feature = "postgres")]
                {
                    let trail = crate::v1::audit_postgres::PostgresAuditTrail::from_env()
                        .await
                        .map_err(|error| {
                            format!("AUDIT_PERSISTENCE=postgres, but the trail could not connect: {error}")
                        })?;
                    Ok(Self::Postgres(trail))
                }
                #[cfg(not(feature = "postgres"))]
                {
                    Err("AUDIT_PERSISTENCE=postgres, but this build has no `postgres` feature; \
                         rebuild with --features postgres rather than silently keeping the trail in memory"
                        .to_string())
                }
            }
            other => Err(format!(
                "AUDIT_PERSISTENCE={other} is not a trail this build implements; use `memory` or `postgres`"
            )),
        }
    }

    /// The connection pool, for tests that assert on the rows directly.
    #[cfg(feature = "postgres")]
    pub fn pool_for_tests(&self) -> &sqlx::PgPool {
        match self {
            Self::Postgres(trail) => trail.pool(),
            // A memory trail has no pool; the caller should not have asked.
            Self::Memory(_) => panic!("this trail is in memory"),
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

    /// Append one event. The trail is append-only: nothing here updates or deletes.
    pub async fn append(&self, event: StoredAuditEvent) -> Result<(), String> {
        match self {
            Self::Memory(store) => {
                let mut guard = store
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                guard.insert(event.id, event);
                Ok(())
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(trail) => trail.append(&event).await,
        }
    }

    /// How many events match `filter`.
    pub async fn count(&self, filter: &AuditFilter) -> Result<usize, String> {
        match self {
            Self::Memory(store) => {
                let guard = store
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                Ok(guard.values().filter(|event| filter.matches(event)).count())
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(trail) => trail.count(filter).await,
        }
    }

    /// A page of matching events, newest first.
    pub async fn page(
        &self,
        filter: &AuditFilter,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<StoredAuditEvent>, String> {
        match self {
            Self::Memory(store) => {
                let guard = store
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let mut matching: Vec<StoredAuditEvent> = guard
                    .values()
                    .filter(|event| filter.matches(event))
                    .cloned()
                    .collect();
                // Newest first, and stable when timestamps tie (the id breaks it).
                matching.sort_by(|left, right| {
                    right
                        .timestamp
                        .cmp(&left.timestamp)
                        .then_with(|| right.id.cmp(&left.id))
                });
                Ok(matching.into_iter().skip(offset).take(limit).collect())
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(trail) => trail.page(filter, offset, limit).await,
        }
    }
}

/// The text form the database stores for an action.
#[cfg(feature = "postgres")]
pub(crate) fn action_text(action: &AuditAction) -> &'static str {
    match action {
        AuditAction::Create => "create",
        AuditAction::Read => "read",
        AuditAction::Update => "update",
        AuditAction::Delete => "delete",
        AuditAction::Approve => "approve",
        AuditAction::Reject => "reject",
        AuditAction::Execute => "execute",
        AuditAction::Export => "export",
    }
}

/// The text form the database stores for a resource type.
#[cfg(feature = "postgres")]
pub(crate) fn resource_text(resource: &ResourceType) -> &'static str {
    match resource {
        ResourceType::Rule => "rule",
        ResourceType::Category => "category",
        ResourceType::Tag => "tag",
        ResourceType::User => "user",
        ResourceType::Approval => "approval",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn event(
        action: AuditAction,
        resource_type: ResourceType,
        user: Option<&str>,
    ) -> StoredAuditEvent {
        StoredAuditEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now().to_rfc3339(),
            user_id: user.map(str::to_string),
            action,
            resource_type,
            resource_id: "abc".to_string(),
            resource_name: Some("a rule".to_string()),
            details: serde_json::json!({}),
            ip_address: None,
            user_agent: None,
        }
    }

    #[actix_web::test]
    async fn the_memory_trail_filters_and_pages() {
        let backend = AuditBackend::in_memory();
        for index in 0..5 {
            backend
                .append(event(
                    AuditAction::Create,
                    ResourceType::Rule,
                    Some("alice"),
                ))
                .await
                .expect("appended");
            if index % 2 == 0 {
                backend
                    .append(event(AuditAction::Delete, ResourceType::Rule, Some("bob")))
                    .await
                    .expect("appended");
            }
        }

        let everything = AuditFilter::default();
        assert_eq!(backend.count(&everything).await.expect("counted"), 8);

        let deletions = AuditFilter {
            action: Some(AuditAction::Delete),
            ..Default::default()
        };
        assert_eq!(backend.count(&deletions).await.expect("counted"), 3);

        let by_user = AuditFilter {
            user_id: Some("alice".to_string()),
            ..Default::default()
        };
        assert_eq!(backend.count(&by_user).await.expect("counted"), 5);

        let first_page = backend.page(&everything, 0, 3).await.expect("paged");
        assert_eq!(first_page.len(), 3);
        let second_page = backend.page(&everything, 3, 3).await.expect("paged");
        assert_eq!(second_page.len(), 3);
        assert!(
            first_page
                .iter()
                .all(|event| !second_page.iter().any(|other| other.id == event.id)),
            "pages do not overlap"
        );
    }

    #[actix_web::test]
    async fn the_default_backend_is_the_memory_trail() {
        assert_eq!(AuditBackend::in_memory().kind(), "memory");
    }
}
