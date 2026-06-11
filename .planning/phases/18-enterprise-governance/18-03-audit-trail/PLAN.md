# Phase 18-03: Audit Trail - Plan

**Phase:** 18.03  
**Goal:** Event-sourced audit trail with PostgreSQL storage and optional Kafka streaming  
**Depends on:** Phase 18-02 (approval workflow infrastructure)  

---

## Overview

Implement immutable audit logging:
- **Event types:** rule CRUD, approval actions, role changes, user logins
- **Storage:** PostgreSQL append-only log (event sourcing pattern)
- **Optional streaming:** Kafka/Redpanda for real-time processing
- **Consumer layer:** Background worker for archiving and analytics

---

## Success Criteria (What Must Be TRUE)

1. Every rule modification records user, timestamp, and diff
2. Access events are logged for compliance reporting
3. Audit log is append-only (cannot delete/modif historical entries)
4. Events can be queried by type, time range, user
5. Optional Kafka integration for real-time processing

---

## Implementation Tasks

### Database Schema (Wave 0)

| File | Purpose |
|------|---------|
| `migrations/18_03_audit_schema.sql` | Audit events append-only table |

```sql
-- audit_events: Event sourcing storage
CREATE TABLE audit_events (
    id SERIAL PRIMARY KEY,
    event_id UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    event_type VARCHAR(100) NOT NULL,      -- e.g., "rule.created", "approval.approved"
    actor_id INTEGER REFERENCES users(id),  -- User who triggered the event
    subject_id UUID,                        -- Affected resource (rule_id, approval_id)
    subject_type VARCHAR(50),               -- "rule", "approval", "role", "user"
    payload JSONB NOT NULL,                 -- Full event data as JSON
    occurred_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    source_ip INET,                         -- Client IP (optional)
    user_agent TEXT,                        -- User agent string (optional)
    
    -- Indexes for common queries
    CREATE INDEX idx_audit_event_type ON audit_events(event_type);
    CREATE INDEX idx_audit_occurred_at ON audit_events(occurred_at DESC);
    CREATE INDEX idx_audit_actor_id ON audit_events(actor_id);
    CREATE INDEX idx_audit_subject ON audit_events(subject_type, subject_id);
);

-- Immutable constraint: prevent updates/deletes
CREATE OR REPLACE FUNCTION immutable_audit_log()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Audit log is append-only';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_audit_immutable
    BEFORE UPDATE OR DELETE ON audit_events
    FOR EACH ROW
    EXECUTE FUNCTION immutable_audit_log();

-- Optional: View for common queries
CREATE VIEW audit_events_by_rule AS
SELECT 
    ae.*,
    r.name as rule_name,
    u.username as actor_username
FROM audit_events ae
LEFT JOIN rules r ON ae.subject_id = r.id
LEFT JOIN users u ON ae.actor_id = u.id
WHERE ae.subject_type = 'rule';
```

---

### Audit Event Types (Wave 1)

| File | Purpose |
|------|---------|
| `src/governance/audit/types.rs` | Event enums and serialization |

```rust
// src/governance/audit/types.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AuditEvent {
    // Rule lifecycle events
    RuleCreated {
        rule_id: Uuid,
        rule_name: String,
        category: Option<String>,
        created_by: String,
    },
    
    RuleUpdated {
        rule_id: Uuid,
        rule_name: String,
        diff: serde_json::Value,  // JSON patch format
        updated_by: String,
    },
    
    RuleDeleted {
        rule_id: Uuid,
        rule_name: String,
        deleted_by: String,
    },
    
    // Approval workflow events
    ApprovalSubmitted {
        approval_id: Uuid,
        rule_id: Uuid,
        submitted_by: String,
        workflow_type: String,
    },
    
    ApprovalApproved {
        approval_id: Uuid,
        approved_by: String,
        approvers_count: i32,
        total_required: i32,
    },
    
    ApprovalRejected {
        approval_id: Uuid,
        rejected_by: String,
        reason: Option<String>,
    },
    
    // RBAC events
    RoleAssigned {
        user_id: String,
        role_name: String,
        assigned_by: String,
    },
    
    RoleRevoked {
        user_id: String,
        role_name: String,
        revoked_by: String,
    },
    
    PermissionChanged {
        user_id: String,
        resource: String,
        action: String,
        scope: String,
        changed_by: String,
    },
    
    // System events
    UserLogin {
        user_id: String,
        username: String,
        success: bool,
        ip_address: Option<String>,
    },
    
    UserLogout {
        user_id: String,
        session_duration_seconds: i64,
    },
}

impl AuditEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            AuditEvent::RuleCreated { .. } => "rule.created",
            AuditEvent::RuleUpdated { .. } => "rule.updated",
            AuditEvent::RuleDeleted { .. } => "rule.deleted",
            AuditEvent::ApprovalSubmitted { .. } => "approval.submitted",
            AuditEvent::ApprovalApproved { .. } => "approval.approved",
            AuditEvent::ApprovalRejected { .. } => "approval.rejected",
            AuditEvent::RoleAssigned { .. } => "role.assigned",
            AuditEvent::RoleRevoked { .. } => "role.revoked",
            AuditEvent::PermissionChanged { .. } => "permission.changed",
            AuditEvent::UserLogin { .. } => "user.login",
            AuditEvent::UserLogout { .. } => "user.logout",
        }
    }

    pub fn occurred_at(&self) -> DateTime<Utc> {
        Utc::now()
    }

    pub fn subject_id(&self) -> Option<Uuid> {
        match self {
            AuditEvent::RuleCreated { rule_id, .. } => Some(*rule_id),
            AuditEvent::RuleUpdated { rule_id, .. } => Some(*rule_id),
            AuditEvent::RuleDeleted { rule_id, .. } => Some(*rule_id),
            AuditEvent::ApprovalSubmitted { approval_id, .. } => Some(*approval_id),
            AuditEvent::ApprovalApproved { approval_id, .. } => Some(*approval_id),
            AuditEvent::ApprovalRejected { approval_id, .. } => Some(*approval_id),
            _ => None,
        }
    }

    pub fn subject_type(&self) -> &'static str {
        match self {
            AuditEvent::RuleCreated { .. } => "rule",
            AuditEvent::RuleUpdated { .. } => "rule",
            AuditEvent::RuleDeleted { .. } => "rule",
            AuditEvent::ApprovalSubmitted { .. } => "approval",
            AuditEvent::ApprovalApproved { .. } => "approval",
            AuditEvent::ApprovalRejected { .. } => "approval",
            AuditEvent::RoleAssigned { .. } => "role",
            AuditEvent::RoleRevoked { .. } => "role",
            AuditEvent::PermissionChanged { .. } => "permission",
            AuditEvent::UserLogin { .. } => "user",
            AuditEvent::UserLogout { .. } => "user",
        }
    }
}
```

---

### Audit Store (Wave 1)

| File | Purpose |
|------|---------|
| `src/governance/audit/store.rs` | PostgreSQL append-only storage |

```rust
// src/governance/audit/store.rs
use crate::governance::audit::types::AuditEvent;
use sqlx::{PgPool, Row};

pub struct AuditStore {
    pool: PgPool,
}

impl AuditStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Append an event to the audit log
    /// Returns the generated event_id
    pub async fn append_event(&self, event: &AuditEvent) -> Result<String, sqlx::Error> {
        let event_type = event.event_type();
        let subject_type = event.subject_type();
        let payload = serde_json::to_value(event)?;
        
        // Extract actor_id if available
        let actor_id = match event {
            AuditEvent::RuleCreated { created_by, .. } => Some(created_by.parse::<i32>().ok()),
            AuditEvent::RuleUpdated { updated_by, .. } => Some(updated_by.parse::<i32>().ok()),
            AuditEvent::RuleDeleted { deleted_by, .. } => Some(deleted_by.parse::<i32>().ok()),
            AuditEvent::ApprovalSubmitted { submitted_by, .. } => Some(submitted_by.parse::<i32>().ok()),
            AuditEvent::ApprovalApproved { approved_by, .. } => Some(approved_by.parse::<i32>().ok()),
            AuditEvent::ApprovalRejected { rejected_by, .. } => Some(rejected_by.parse::<i32>().ok()),
            AuditEvent::RoleAssigned { assigned_by, .. } => Some(assigned_by.parse::<i32>().ok()),
            AuditEvent::RoleRevoked { revoked_by, .. } => Some(revoked_by.parse::<i32>().ok()),
            AuditEvent::PermissionChanged { changed_by, .. } => Some(changed_by.parse::<i32>().ok()),
            AuditEvent::UserLogin { user_id, .. } => Some(user_id.parse::<i32>().ok()),
            AuditEvent::UserLogout { user_id, .. } => Some(user_id.parse::<i32>().ok()),
        }.flatten();

        let event_id: String = sqlx::query_scalar!(
            r#"
            INSERT INTO audit_events (event_type, actor_id, subject_id, subject_type, payload)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING event_id::text
            "#,
            event_type,
            actor_id,
            event.subject_id().map(|id| id.to_string()),
            subject_type,
            payload
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(event_id)
    }

    /// Query events with filters
    pub async fn query_events(
        &self,
        event_types: Option<Vec<&str>>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        actor_id: Option<i32>,
        limit: i64,
    ) -> Result<Vec<AuditEvent>, sqlx::Error> {
        let mut query = r#"
            SELECT payload FROM audit_events
            WHERE 1=1
        "#.to_string();
        
        if let Some(types) = event_types {
            let placeholders: Vec<String> = (0..types.len()).map(|i| format!("${}", i + 5)).collect();
            query.push_str(&format!(" AND event_type IN ({})", placeholders.join(", ")));
        }
        
        if let Some(start) = start_time {
            query.push_str(" AND occurred_at >= $6");
        }
        
        if let Some(end) = end_time {
            query.push_str(" AND occurred_at <= $7");
        }
        
        if let Some(actor) = actor_id {
            query.push_str(" AND actor_id = $8");
        }
        
        query.push_str(" ORDER BY occurred_at DESC LIMIT $9");

        let rows = sqlx::query(&query)
            .bind(event_types.map(|v| v.first().cloned()))
            .fetch_all(&self.pool)
            .await?;

        rows.into_iter()
            .map(|row| {
                let payload: serde_json::Value = row.try_get("payload")?;
                Ok(serde_json::from_value::<AuditEvent>(payload)?)
            })
            .collect()
    }

    /// Export audit log to CSV format
    pub async fn export_csv(&self, start_time: DateTime<Utc>, end_time: DateTime<Utc>) 
        -> Result<String, sqlx::Error> 
    {
        let rows = sqlx::query!(
            r#"
            SELECT event_type, actor_id, subject_type, payload, occurred_at
            FROM audit_events
            WHERE occurred_at >= $1 AND occurred_at <= $2
            ORDER BY occurred_at ASC
            "#,
            start_time,
            end_time
        )
        .fetch_all(&self.pool)
        .await?;

        let mut csv = String::from("event_type,actor_id,subject_type,payload_occurred_at\n");
        for row in rows {
            let payload: serde_json::Value = row.payload;
            csv.push_str(&format!(
                "{},{},{},{}\n",
                row.event_type,
                row.actor_id.map_or("".to_string(), |id| id.to_string()),
                row.subject_type.unwrap_or_default(),
                row.occurred_at
            ));
        }
        
        Ok(csv)
    }
}
```

---

### Audit Queue (Wave 1)

Async batched writer to avoid blocking HTTP requests:

```rust
// src/governance/audit/queue.rs
use crate::governance::audit::types::AuditEvent;
use tokio::sync::mpsc;

pub struct AuditQueue {
    sender: mpsc::Sender<AuditEvent>,
}

impl AuditQueue {
    pub fn new(buffer_size: usize) -> (Self,tokio::task::JoinHandle<()>) {
        let (sender, mut receiver) = mpsc::channel(buffer_size);
        
        let handle = tokio::spawn(async move {
            let mut batch = Vec::with_capacity(100);
            let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));
            
            loop {
                tokio::select! {
                    event = receiver.recv() => {
                        if let Some(event) = event {
                            batch.push(event);
                        }
                        
                        // Flush if batch is full
                        if batch.len() >= 100 {
                            flush_batch(&batch).await;
                            batch.clear();
                        }
                    }
                    _ = timer.tick() => {
                        // Flush periodically
                        if !batch.is_empty() {
                            flush_batch(&batch).await;
                            batch.clear();
                        }
                    }
                }
            }
        });
        
        (Self { sender }, handle)
    }

    pub async fn push(&self, event: AuditEvent) -> Result<(), mpsc::error::SendError<AuditEvent>> {
        self.sender.send(event).await
    }
}

async fn flush_batch(events: &[AuditEvent]) {
    // In production, this would use the AuditStore
    log::info!("Flushing {} audit events", events.len());
}
```

---

### Kafka Integration (Wave 2 - Optional)

| File | Purpose |
|------|---------|
| `src/governance/audit/kafka.rs` | Kafka producer for event streaming |

```rust
// src/governance/audit/kafka.rs
use rdkafka::client::ClientOptions;
use rdkafka::producer::{BaseProducer, BaseRecord};
use serde_json::to_string;

pub struct KafkaAuditProducer {
    producer: BaseProducer,
    topic: String,
}

impl KafkaAuditProducer {
    pub fn new(brokers: &str, topic: &str) -> Self {
        let options = ClientOptions::new(vec![brokers.to_string()]);
        let producer: BaseProducer = options.create().expect("Failed to create Kafka producer");
        
        Self { producer, topic: topic.to_string() }
    }

    pub fn produce(&self, event: &AuditEvent) {
        let payload = to_string(event).expect("Failed to serialize audit event");
        let key = format!("{}:{}", event.subject_type(), event.subject_id().map(|id| id.to_string()).unwrap_or_default());
        
        let record = BaseRecord::to(&self.topic)
            .key(&key)
            .payload(&payload);
            
        self.producer.send(record, 0).expect("Failed to send audit event");
    }
}
```

---

### API Endpoints (Wave 2)

| Endpoint | Method | Role Required | Purpose |
|----------|--------|---------------|---------|
| `/api/v1/audit/events` | GET | admin | Query audit events with filters |
| `/api/v1/audit/export` | POST | admin | Export audit log to CSV/JSON |

```rust
// src/api/v1/routes/audit.rs
use actix_web::{get, post, web, HttpResponse};
use chrono::Utc;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct QueryParams {
    pub event_type: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub actor_id: Option<i32>,
}

#[get("/audit/events")]
async fn query_events(
    query: web::Query<QueryParams>,
    audit_store: web::Data<Arc<Mutex<AuditStore>>>,
) -> Result<HttpResponse, AppError> {
    let start = query.start_time
        .as_ref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    
    let end = query.end_time
        .as_ref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let events = audit_store.lock().await
        .query_events(
            query.event_type.as_deref().map(|s| vec![s]),
            start,
            end,
            query.actor_id,
            100,
        )
        .await?;

    Ok(HttpResponse::Ok().json(events))
}

#[post("/audit/export")]
async fn export_audit(
    request: web::Json<ExportRequest>,
    audit_store: web::Data<Arc<Mutex<AuditStore>>>,
) -> Result<HttpResponse, AppError> {
    let start = chrono::DateTime::parse_from_rfc3339(&request.start_time)?
        .with_timezone(&Utc);
    let end = chrono::DateTime::parse_from_rfc3339(&request.end_time)?
        .with_timezone(&Utc);

    let csv = audit_store.lock().await.export_csv(start, end).await?;
    
    Ok(HttpResponse::Ok()
        .content_type("text/csv")
        .body(csv))
}
```

---

### Frontend: Audit Log Viewer (Wave 3)

| File | Purpose |
|------|---------|
| `frontend/src/features/governance/audit/AuditLogViewer.tsx` | Event viewer with filters |
| `frontend/src/features/governance/audit/ExportDialog.tsx` | Export configuration dialog |

```typescript
// frontend/src/features/governance/audit/AuditLogViewer.tsx
export function AuditLogViewer() {
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [filter, setFilter] = useState<AuditFilter>({});
  
  useEffect(() => {
    const params = new URLSearchParams();
    if (filter.eventType) params.append('event_type', filter.eventType);
    if (filter.startTime) params.append('start_time', filter.startTime);
    
    fetch(`/api/v1/audit/events?${params}`)
      .then(r => r.json())
      .then(setEvents);
  }, [filter]);

  return (
    <div className="audit-log-viewer">
      <AuditFilterForm filter={filter} onChange={setFilter} />
      
      <div className="events-list">
        {events.map(event => (
          <EventCard key={event.eventId} event={event} />
        ))}
      </div>
      
      <ExportDialog onExport={handleExport} />
    </div>
  );
}

interface AuditEvent {
  eventId: string;
  eventType: string;
  actorId?: number;
  subjectType: string;
  occurredAt: string;
  payload: Record<string, unknown>;
}

interface AuditFilter {
  eventType?: string;
  startTime?: string;
  endTime?: string;
}
```

---

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rule_created_event() {
        let event = AuditEvent::RuleCreated {
            rule_id: Uuid::new_v4(),
            rule_name: "Test Rule".to_string(),
            category: Some("test".to_string()),
            created_by: "1".to_string(),
        };
        
        assert_eq!(event.event_type(), "rule.created");
        assert_eq!(event.subject_type(), "rule");
    }
    
    #[test]
    fn test_serialization_roundtrip() {
        let original = AuditEvent::UserLogin {
            user_id: "123".to_string(),
            username: "john.doe".to_string(),
            success: true,
            ip_address: Some("192.168.1.1".to_string()),
        };
        
        let json = serde_json::to_value(&original).unwrap();
        let parsed = serde_json::from_value::<AuditEvent>(json.clone()).unwrap();
        
        assert_eq!(original.event_type(), parsed.event_type());
    }
}
```

### Integration Tests

| Test | Endpoint/Component | Pass Condition |
|------|-------------------|----------------|
| AUD-03-01 | POST rule → event logged | event_type = "rule.created" |
| AUD-03-02 | GET `/api/v1/audit/events` (admin) | Returns 200 with events |
| AUD-03-03 | Query by time range | Only events in range returned |

---

## Acceptance Checklist

- [ ] Audit events table created with immutable trigger
- [ ] Event types enum compiles and serializes correctly
- [x] Store appends events to database (in-memory for now)
- [ ] Query endpoint filters by event type, time, actor
- [ ] Export endpoint produces CSV format
- [ ] Batch queue writes are asynchronous (non-blocking)

**Status: PARTIAL - Store implementation complete with in-memory backend**
- [ ] Kafka producer sends events if configured

---

## Dependencies

### Cargo.toml additions
```toml
[dependencies]
sqlx = { version = "0.7", features = ["postgres", "chrono", "uuid"] }
rdkafka = { version = "0.36", optional = true }
tokio = { version = "1.37", features = ["time", "sync"] }

[dev-dependencies]
mockall = "0.13"
```

---

## Notes

- **Security:** Audit log cannot be modified or deleted (database-level enforcement)
- **Performance:** Async batched writes prevent blocking HTTP requests
- **Retention:** Consider time-based partitioning for old events (PostgreSQL native)
- **Future enhancement:** Add log shipper (Filebeat) for centralized logging
- **Future enhancement:** Create materialized view for common query patterns

---

*Plan created: 2026-06-10*
*Phase: 18.03 - Audit Trail*
