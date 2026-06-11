# Phase 18-02: Maker-Checker Approval Workflow - Plan

**Phase:** 18.02  
**Goal:** Production-ready approval workflow with webhooks as primary triggers  
**Depends on:** Phase 18-01 (RBAC infrastructure)  

---

## Overview

Implement maker-checker pattern for rule changes:
- **Workflow states:** draft → submitted → in_review → approved/rejected
- **Multi-tier support:** Sequential or parallel approvers
- **Primary trigger mechanism:** Webhooks receive external events and create approval requests
- **REST API:** Read-only status queries only (no creation)
- **Time-based escalation:** Auto-escalate stale approvals

**Note:** This plan follows D-18-02 which requires webhooks as primary triggers. The REST API is for query/status only.

---

## Success Criteria (What Must Be TRUE)

1. Rule changes require submission before production deployment
2. Reviewers receive notification for pending approvals
3. Approved changes are deployed to production automatically
4. Rejected changes return to author with feedback
5. Time-based escalation prevents workflow deadlock

---

## Implementation Tasks

### Database Schema (Wave 0)

| File | Purpose |
|------|---------|
| `migrations/18_02_approval_schema.sql` | Approval workflows and requests tables |

```sql
-- approval_workflows: Template definitions for approval processes
CREATE TABLE approval_workflows (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    tiers JSONB NOT NULL,  -- [{reviewers: [user_ids], required_approvals: 1}, ...]
    auto_escape_hours INTEGER DEFAULT 48,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- approval_requests: Individual workflow instances
CREATE TABLE approval_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id INTEGER REFERENCES approval_workflows(id),
    rule_id UUID NOT NULL,        -- Rule being modified
    maker_id INTEGER NOT NULL REFERENCES users(id),  -- Submitter
    state VARCHAR(50) NOT NULL CHECK (state IN ('draft', 'submitted', 'in_review', 'approved', 'rejected', 'escalated')),
    current_tier INTEGER DEFAULT 1,
    reviewers JSONB,              -- Current tier reviewers array
    required_approvals INTEGER DEFAULT 1,
    approved_by INTEGER[],        -- Array of user IDs who approved
    rejected_by INTEGER,          -- User ID who rejected (if rejected)
    rejection_reason TEXT,
    submitted_at TIMESTAMP WITH TIME ZONE,
    started_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- approval_webhooks: Webhook configuration for external notifications
CREATE TABLE approval_webhooks (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    url TEXT NOT NULL,
    events JSONB NOT NULL DEFAULT '["submitted", "approved", "rejected"]',  -- Event types to trigger on
    secret TEXT,                  -- Webhook signature verification
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

---

### Domain Module (Wave 1)

| File | Purpose |
|------|---------|
| `src/governance/approval/mod.rs` | Module exports and workflow engine |
| `src/governance/approval/engine.rs` | State machine implementation |
| `src/governance/approval/webhooks.rs` | Webhook dispatch logic |
| `src/governance/approval/notifications.rs` | Email/Slack integration |

```rust
// src/governance/approval/types.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ApprovalState {
    Draft,
    Submitted { submitted_at: DateTime<Utc> },
    InReview { started_at: DateTime<Utc>, current_tier: i32 },
    Approved { approved_by: Vec<String>, completed_at: DateTime<Utc> },
    Rejected { rejected_by: String, reason: String, completed_at: DateTime<Utc> },
    Escalated { escalated_to_tier: i32, escalated_at: DateTime<Utc> },
}

#[derive(Clone, Debug)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub workflow_id: i32,
    pub rule_id: Uuid,
    pub maker_id: String,
    pub state: ApprovalState,
    pub reviewers: Vec<String>,      // Current tier reviewers
    pub required_approvals: i32,     // approvals needed in current tier
    pub tiers: Vec<TierConfig>,      // All tiers for the workflow
}

#[derive(Clone, Debug)]
pub struct TierConfig {
    pub reviewers: Vec<String>,
    pub required_approvals: i32,
}

// Workflow engine operations
pub trait ApprovalEngine {
    fn submit(&mut self, approval_id: Uuid) -> Result<(), ApprovalError>;
    fn review(&mut self, approval_id: Uuid, reviewer_id: String, approve: bool, reason: Option<String>) -> Result<(), ApprovalError>;
    fn escalate(&mut self, approval_id: Uuid) -> Result<(), ApprovalError>;
}
```

---

### Webhook Dispatcher (Wave 1)

| File | Purpose |
|------|---------|
| `src/governance/approval/webhooks.rs` | Webhook dispatch with retry logic |

```rust
// src/governance/approval/webhooks.rs - PRIMARY TRIGGER HANDLER
use reqwest::Client;
use serde_json::{json, Value};
use uuid::Uuid;

pub struct WebhookHandler {
    client: Client,
}

impl WebhookHandler {
    pub fn new() -> Self {
        Self { 
            client: Client::new(),
        }
    }

    /// PRIMARY ENTRY POINT: Handle webhook event and create approval request
    /// This is the main path for approval workflow - NOT REST API
    pub async fn handle_incoming_event(&self, payload: &Value) -> Result<Uuid, WebhookError> {
        // Parse the incoming webhook event
        let rule_id = payload.get("rule_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or(WebhookError::MissingField("rule_id"))?;
        
        let maker_id = payload.get("maker_id")
            .and_then(|v| v.as_str())
            .ok_or(WebhookError::MissingField("maker_id"))?;

        // Validate webhook signature if secret is configured
        self.validate_signature(payload)?;

        // Create the approval request in database
        // This triggers the workflow state machine
        let approval_request = ApprovalRequest {
            id: Uuid::new_v4(),
            workflow_id: 1,  // Default workflow - could be configurable
            rule_id,
            maker_id: maker_id.to_string(),
            state: ApprovalState::Submitted { submitted_at: chrono::Utc::now() },
            reviewers: vec![],  // Populated from workflow definition
            required_approvals: 1,
            tiers: vec![],
        };

        // Store in database (implementation would use sqlx)
        self.store_approval(&approval_request).await?;

        Ok(approval_request.id)
    }

    /// REST API endpoints - READ ONLY (status queries only)
    /// These should NOT create new workflows, just query existing ones
    pub async fn get_status(&self, approval_id: Uuid) -> Result<ApprovalStatus, WebhookError> {
        // Query database for current state
        unimplemented!("REST endpoint implementation - read-only")
    }

    fn validate_signature(&self, payload: &Value) -> Result<(), WebhookError> {
        // Validate webhook signature from request headers
        Ok(())
    }

    async fn store_approval(&self, approval: &ApprovalRequest) -> Result<(), sqlx::Error> {
        // Store in database using sqlx
        unimplemented!("Database storage implementation")
    }
}

#[derive(Clone)]
pub struct ApprovalStatus {
    pub id: Uuid,
    pub state: String,
    pub current_tier: i32,
}

#[derive(Debug)]
pub enum WebhookError {
    MissingField(&'static str),
    InvalidSignature,
    BadResponse(reqwest::StatusCode),
    DatabaseError(sqlx::Error),
}

---

### API Endpoints (Wave 2) - READ ONLY

**Important:** REST API is for QUERY/STATUS only. Webhooks are the primary trigger mechanism.

| Endpoint | Method | Role Required | Purpose |
|----------|--------|---------------|---------|
| `/api/v1/approvals` | GET | author/reviewer/admin | List requests (query status) |
| `/api/v1/approvals/{id}` | GET | author/reviewer/admin | Get request status (query only) |
| `/api/v1/approvals/webhooks` | GET | admin | List webhook configurations |

**Webhook Entry Point (PRIMARY):**

| Webhook URL | Method | Purpose |
|-------------|--------|---------|
| `/webhooks/approval/create` | POST | External trigger - creates approval in DB |

```rust
// Example: Webhook handler - PRIMARY TRIGGER PATH
#[post("/webhooks/approval/create")]
async fn create_approval_from_webhook(
    payload: web::Json<serde_json::Value>,
    webhook_handler: web::Data<Arc<Mutex<WebhookHandler>>>,
) -> Result<web::Json<AppResponse>, AppError> {
    let approval_id = webhook_handler.lock().await
        .handle_incoming_event(&payload.into_inner())
        .map_err(AppError::from)?;
    
    Ok(web::Json(AppResponse { 
        success: true,
        approval_id: approval_id.to_string()
    }))
}

// Example: Status query endpoint (read-only)
#[get("/approvals/{id}")]
async fn get_approval_status(
    path: web::Path<Uuid>,
    webhook_handler: web::Data<Arc<Mutex<WebhookHandler>>>,
) -> Result<web::Json<AppResponse>, AppError> {
    let approval_id = path.into_inner();
    
    let status = webhook_handler.lock().await.get_status(approval_id).map_err(AppError::from)?;
    
    Ok(web::Json(status))
}

// Example: Review endpoint - REST API can still be used for manual review
#[put("/approvals/{id}/review")]
async fn review_approval(
    path: web::Path<Uuid>,
    review_data: web::Json<ReviewRequest>,
    claims: JwtClaims<RbacClaims>,
    webhook_handler: web::Data<Arc<Mutex<WebhookHandler>>>,
) -> Result<web::Json<AppResponse>, AppError> {
    let approval_id = path.into_inner();
    let data = review_data.into_inner();
    
    // Check reviewer permission
    if !claims.has_permission("approval", "review") {
        return Err(AppError::Forbidden);
    }
    
    Ok(web::Json(AppResponse { success: true }))
}
```

---

### Escalation Worker (Wave 2)

Background worker that checks for stale approvals and auto-escalates:

```rust
// src/governance/approval/escalation.rs
use chrono::{Duration, Utc};
use sqlx::PgPool;

pub struct EscalationWorker {
    pool: PgPool,
    max_stale_hours: i64,
}

impl EscalationWorker {
    pub fn new(pool: PgPool, max_stale_hours: i64) -> Self {
        Self { pool, max_stale_hours }
    }

    pub async fn check_stale_approvals(&self) -> Result<Vec<Uuid>, sqlx::Error> {
        let cutoff = Utc::now() - Duration::hours(self.max_stale_hours);
        
        let stale: Vec<Uuid> = sqlx::query!(
            r#"SELECT id FROM approval_requests
               WHERE state = 'in_review' AND started_at < $1"#,
            cutoff
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| row.id)
        .collect();
        
        // Auto-escalate each stale approval
        for id in &stale {
            sqlx::query!(
                r#"UPDATE approval_requests 
                   SET state = 'escalated', current_tier = current_tier + 1 
                   WHERE id = $1"#,
                id
            )
            .execute(&self.pool)
            .await?;
        }
        
        Ok(stale)
    }

    pub async fn run_periodically(&self, interval: std::time::Duration) {
        loop {
            tokio::time::sleep(interval).await;
            if let Err(e) = self.check_stale_approvals().await {
                log::error!("Escalation check failed: {}", e);
            }
        }
    }
}
```

---

### Frontend: Approval Queue (Wave 3)

| File | Purpose |
|------|---------|
| `frontend/src/features/governance/approval/ApprovalQueue.tsx` | Reviewer approval queue |
| `frontend/src/features/governance/approval/ApprovalWorkflow.tsx` | Submitter workflow UI |
| `frontend/src/features/governance/approval/WebhookConfig.tsx` | Webhook management |

```typescript
// frontend/src/features/governance/approval/ApprovalQueue.tsx
export function ApprovalQueue() {
  const { user } = useAuth();
  const [requests, setRequests] = useState<ApprovalRequest[]>([]);
  
  useEffect(() => {
    if (user?.role === 'reviewer' || user?.role === 'admin') {
      fetch('/api/v1/approvals?status=pending').then(r => r.json()).then(setRequests);
    }
  }, [user]);
  
  const handleReview = async (id: string, approve: boolean, reason?: string) => {
    await fetch(`/api/v1/approvals/${id}/review`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ approve, reason })
    });
    setRequests(prev => prev.filter(r => r.id !== id));
  };
  
  return (
    <div className="approval-queue">
      {requests.map(request => (
        <ApprovalCard 
          key={request.id}
          request={request}
          onApprove={() => handleReview(request.id, true)}
          onReject={() => handleReview(request.id, false)}
        />
      ))}
    </div>
  );
}

interface ApprovalRequest {
  id: string;
  rule_name: string;
  maker_name: string;
  submitted_at: string;
  tier: number;
}
```

---

### Frontend: Submit Approval (Wave 3)

```typescript
// frontend/src/features/governance/approval/ApprovalWorkflow.tsx
export function SubmitForApproval({ ruleId, onClose }: { ruleId: string, onClose: () => void }) {
  const [submitting, setSubmitting] = useState(false);
  
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    
    try {
      await fetch('/api/v1/approvals', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ rule_id: ruleId })
      });
      onClose();
    } finally {
      setSubmitting(false);
    }
  };
  
  return (
    <Modal title="Submit Rule for Approval">
      <p>This will require reviewer approval before changes deploy to production.</p>
      <button onClick={handleSubmit} disabled={submitting}>
        {submitting ? 'Submitting...' : 'Submit'}
      </button>
    </Modal>
  );
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
    fn test_submit_transitions_to_in_review() {
        let mut engine = ApprovalEngine::new();
        let id = Uuid::new_v4();
        
        // Initial state: draft
        assert!(matches!(engine.get_state(id), ApprovalState::Draft));
        
        engine.submit(id).unwrap();
        
        // After submit: in_review with first tier reviewers
        match engine.get_state(id) {
            ApprovalState::InReview { current_tier, .. } => {
                assert_eq!(current_tier, 1);
            }
            _ => panic!("Wrong state after submit"),
        }
    }
    
    #[test]
    fn test_approval_requires_all_reviewers() {
        let mut engine = ApprovalEngine::new();
        let id = Uuid::new_v4();
        
        // Setup: 3 reviewers, 2 required approvals
        engine.add_tier(vec!["r1", "r2", "r3"].into_iter().map(|s| s.to_string()).collect(), 2);
        
        engine.submit(id).unwrap();
        
        // One approval - not enough
        engine.review(id, "r1".to_string(), true, None).unwrap();
        assert!(matches!(engine.get_state(id), ApprovalState::InReview { .. }));
        
        // Second approval - enough to approve
        engine.review(id, "r2".to_string(), true, None).unwrap();
        assert!(matches!(engine.get_state(id), ApprovalState::Approved { .. }));
    }
}
```

### Integration Tests

| Test | Endpoint/Component | Pass Condition |
|------|-------------------|----------------|
| APW-02-01 | POST `/api/v1/approvals` (author) | Returns 201, state = 'submitted' |
| APW-02-02 | PUT `/api/v1/approvals/{id}/review` (viewer) | Returns 403 Forbidden |
| APW-02-03 | Webhook delivery (mock server) | HTTP 200 response received |

---

## Acceptance Checklist

- [ ] Approval workflow database schema created
- [ ] Engine module compiles with `cargo check --lib governance::approval`
- [ ] State transitions are correct (draft → submitted → in_review → approved/rejected)
- [ ] Webhook dispatcher sends events on state changes
- [ ] Review endpoint accepts approve/reject with optional reason
- [ ] Escalation worker runs and updates stale approvals
- [ ] Frontend approval queue shows pending requests
- [ ] Frontend submit workflow creates approval request

---

## Dependencies

### Cargo.toml additions
```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1.37", features = ["time", "macros"] }
futures-util = "0.3"
serde_json = "1.0"

# For cron-like scheduling (optional)
tokio-cron-scheduler = "0.9"
```

---

## Notes

- **Escalation default:** 48 hours before auto-escalation
- **Approval threshold:** Configurable per workflow template
- **Webhook retries:** 3 attempts with exponential backoff
- **Future enhancement:** Add approval comments/thread for discussion
- **Future enhancement:** Support parallel tier approvals (all must approve vs any)

---

*Plan created: 2026-06-10*
*Phase: 18.02 - Maker-Checker Approval Workflow*
