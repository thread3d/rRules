//! The audit trail in PostgreSQL.
//!
//! Append-only by contract: `append` inserts, and nothing here updates or deletes a row. Filtering
//! happens in SQL so a long trail does not have to be loaded to answer a question about it.

use crate::v1::audit::{AuditAction, ResourceType, StoredAuditEvent};
use crate::v1::audit_store::{action_text, resource_text, AuditFilter};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// The DDL this trail applies on connect.
const SCHEMA: &str = include_str!("../../../migrations/21_02_audit_events.sql");

/// An audit trail backed by PostgreSQL.
pub struct PostgresAuditTrail {
    pool: sqlx::PgPool,
}

impl PostgresAuditTrail {
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

    pub async fn append(&self, event: &StoredAuditEvent) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO audit_events \
                 (id, occurred_at, user_id, action, resource_type, resource_id, resource_name, details, ip_address, user_agent) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(event.id)
        .bind(DateTime::parse_from_rfc3339(&event.timestamp).map(|at| at.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()))
        .bind(&event.user_id)
        .bind(action_text(&event.action))
        .bind(resource_text(&event.resource_type))
        .bind(&event.resource_id)
        .bind(&event.resource_name)
        .bind(&event.details)
        .bind(&event.ip_address)
        .bind(&event.user_agent)
        .execute(&self.pool)
        .await
        .map_err(|error| format!("could not append the event: {error}"))?;
        Ok(())
    }

    pub async fn count(&self, filter: &AuditFilter) -> Result<usize, String> {
        // `QueryBuilder` rather than a formatted string: sqlx refuses a dynamic SQL string outright,
        // and the filter values are always bound, never interpolated.
        let mut builder = sqlx::QueryBuilder::new("SELECT COUNT(*) FROM audit_events WHERE TRUE");
        Self::apply_filter(&mut builder, filter);
        let count: i64 = builder
            .build_query_scalar::<i64>()
            .fetch_one(&self.pool)
            .await
            .map_err(|error| format!("could not count the trail: {error}"))?;
        Ok(count.max(0) as usize)
    }

    pub async fn page(
        &self,
        filter: &AuditFilter,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<StoredAuditEvent>, String> {
        let mut builder = sqlx::QueryBuilder::new(
            "SELECT id, occurred_at, user_id, action, resource_type, resource_id, resource_name, \
                    details, ip_address, user_agent FROM audit_events WHERE TRUE",
        );
        Self::apply_filter(&mut builder, filter);
        builder.push(" ORDER BY occurred_at DESC, id DESC LIMIT ");
        builder.push_bind(limit as i64);
        builder.push(" OFFSET ");
        builder.push_bind(offset as i64);

        let rows = builder
            .build_query_as::<AuditRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| format!("could not read the trail: {error}"))?;

        Ok(rows.into_iter().map(StoredAuditEvent::from).collect())
    }

    /// The three optional predicates, in one place so `count` and `page` cannot disagree.
    fn apply_filter(builder: &mut sqlx::QueryBuilder<sqlx::Postgres>, filter: &AuditFilter) {
        if let Some(resource_type) = &filter.resource_type {
            builder
                .push(" AND resource_type = ")
                .push_bind(resource_text(resource_type));
        }
        if let Some(action) = &filter.action {
            builder
                .push(" AND action = ")
                .push_bind(action_text(action));
        }
        if let Some(user_id) = &filter.user_id {
            builder.push(" AND user_id = ").push_bind(user_id);
        }
    }
}

/// One row of the trail, before it becomes a [`StoredAuditEvent`].
type AuditRow = (
    Uuid,
    DateTime<Utc>,
    Option<String>,
    String,
    String,
    String,
    Option<String>,
    serde_json::Value,
    Option<String>,
    Option<String>,
);

impl From<AuditRow> for StoredAuditEvent {
    fn from(row: AuditRow) -> Self {
        let (
            id,
            occurred_at,
            user_id,
            action,
            resource_type,
            resource_id,
            resource_name,
            details,
            ip_address,
            user_agent,
        ) = row;
        Self {
            id,
            timestamp: occurred_at.to_rfc3339(),
            user_id,
            action: parse_action(&action),
            resource_type: parse_resource(&resource_type),
            resource_id,
            resource_name,
            details,
            ip_address,
            user_agent,
        }
    }
}

fn parse_action(text: &str) -> AuditAction {
    match text {
        "read" => AuditAction::Read,
        "update" => AuditAction::Update,
        "delete" => AuditAction::Delete,
        "approve" => AuditAction::Approve,
        "reject" => AuditAction::Reject,
        "execute" => AuditAction::Execute,
        "export" => AuditAction::Export,
        _ => AuditAction::Create,
    }
}

fn parse_resource(text: &str) -> ResourceType {
    match text {
        "category" => ResourceType::Category,
        "tag" => ResourceType::Tag,
        "user" => ResourceType::User,
        "approval" => ResourceType::Approval,
        _ => ResourceType::Rule,
    }
}
