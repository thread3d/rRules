//! Approval workflow API endpoints - Phase 20-03

use actix_web::{web, HttpResponse, Responder};
use uuid::Uuid;

// In-memory approval storage (would be replaced with database in production)
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Shared state for approvals API
type ApprovalStore = Arc<Mutex<HashMap<Uuid, StoredApproval>>>;

/// Internal representation of an approval request
#[derive(Debug, Clone)]
pub struct StoredApproval {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub workflow_id: Uuid,
    pub maker_id: String,
    pub state: ApprovalState,
    pub reviewers: Vec<String>,
    pub required_approvals: i32,
    pub approved_by: Vec<String>,
    pub created_at: String,
}

/// Current state of an approval request
#[derive(Clone, Debug, serde::Serialize, PartialEq)]
pub enum ApprovalState {
    Draft,
    Submitted { submitted_at: String },
    InReview { started_at: String, current_tier: i32 },
    Approved { approved_by: Vec<String>, completed_at: String },
    Rejected { rejected_by: String, reason: Option<String>, completed_at: String },
    Escalated { escalated_to_tier: i32, escalated_at: String },
}

/// Request to submit a rule for approval
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SubmitApprovalRequest {
    pub workflow_id: Option<Uuid>,
    pub maker_id: Option<String>,
}

/// Response when submitting an approval
#[derive(Debug, Clone, serde::Serialize)]
pub struct SubmitApprovalResponse {
    pub approval_id: String,
    pub rule_id: String,
    pub status: String,
}

/// Request to approve/reject an approval request
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ApprovalActionRequest {
    pub approver_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl From<&StoredApproval> for ApprovalState {
    fn from(_approval: &StoredApproval) -> Self {
        // This is a placeholder - actual state conversion would happen
        // in the approval engine
        ApprovalState::Draft
    }
}

/// Initialize in-memory approval store
pub fn init_store() -> ApprovalStore {
    let store: HashMap<Uuid, StoredApproval> = HashMap::new();
    Arc::new(Mutex::new(store))
}

/// Submit a rule for approval - POST /v1/rules/{id}/submit
pub async fn submit_for_approval(
    path: web::Path<String>,
    req: web::Json<SubmitApprovalRequest>,
    store: web::Data<ApprovalStore>,
) -> impl Responder {
    let rule_id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid rule ID format"
            }));
        }
    };

    let workflow_id = req.workflow_id.unwrap_or(Uuid::new_v4());
    let maker_id = req.maker_id.clone().unwrap_or_else(|| "system".to_string());

    // Create approval request in draft state
    let approval = StoredApproval {
        id: Uuid::new_v4(),
        rule_id,
        workflow_id,
        maker_id,
        state: ApprovalState::Draft,
        reviewers: vec![],
        required_approvals: 1,
        approved_by: vec![],
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut store_guard = store.lock().unwrap();
    let approval_id = approval.id;
    store_guard.insert(approval_id, approval);

    HttpResponse::Created().json(SubmitApprovalResponse {
        approval_id: approval_id.to_string(),
        rule_id: rule_id.to_string(),
        status: "draft".to_string(),
    })
}

/// List pending approvals - GET /v1/approvals/pending
pub async fn list_pending_approvals(
    store: web::Data<ApprovalStore>,
) -> impl Responder {
    let store_guard = store.lock().unwrap();

    // Filter for non-completed states (Draft, Submitted, InReview)
    let pending: Vec<&StoredApproval> = store_guard
        .values()
        .filter(|a| !matches!(
            a.state,
            ApprovalState::Approved { .. } | ApprovalState::Rejected { .. }
        ))
        .collect();

    HttpResponse::Ok().json(serde_json::json!({
        "total_count": pending.len(),
        "approvals": pending.iter().map(|a| serde_json::json!({
            "id": a.id.to_string(),
            "rule_id": a.rule_id.to_string(),
            "maker_id": a.maker_id,
            "state": state_to_string(&a.state),
            "created_at": a.created_at
        })).collect::<Vec<_>>()
    }))
}

/// Approve an approval request - POST /v1/approvals/{id}/approve
pub async fn approve_request(
    path: web::Path<String>,
    req: web::Json<ApprovalActionRequest>,
    store: web::Data<ApprovalStore>,
) -> impl Responder {
    let approval_id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid approval ID format"
            }));
        }
    };

    let approver_id = req.approver_id.clone().unwrap_or_else(|| "system".to_string());
    let reason = req.reason.clone();

    let mut store_guard = store.lock().unwrap();

    match store_guard.get_mut(&approval_id) {
        Some(approval) => {
            // Transition to approved state
            approval.state = ApprovalState::Approved {
                approved_by: vec![approver_id],
                completed_at: chrono::Utc::now().to_rfc3339(),
            };
            if let Some(reason) = reason {
                approval.approved_by.push(reason);
            }

            HttpResponse::Ok().json(serde_json::json!({
                "approval_id": approval.id.to_string(),
                "status": "approved",
                "approved_by": approval.approved_by
            }))
        }
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Approval not found",
            "approval_id": approval_id.to_string()
        })),
    }
}

/// Reject an approval request - POST /v1/approvals/{id}/reject
pub async fn reject_request(
    path: web::Path<String>,
    req: web::Json<ApprovalActionRequest>,
    store: web::Data<ApprovalStore>,
) -> impl Responder {
    let approval_id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid approval ID format"
            }));
        }
    };

    let approver_id = req.approver_id.clone().unwrap_or_else(|| "system".to_string());
    let reason = req.reason.clone();

    let mut store_guard = store.lock().unwrap();

    match store_guard.get_mut(&approval_id) {
        Some(approval) => {
            approval.state = ApprovalState::Rejected {
                rejected_by: approver_id,
                reason,
                completed_at: chrono::Utc::now().to_rfc3339(),
            };

            HttpResponse::Ok().json(serde_json::json!({
                "approval_id": approval.id.to_string(),
                "status": "rejected"
            }))
        }
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Approval not found",
            "approval_id": approval_id.to_string()
        })),
    }
}

fn state_to_string(state: &ApprovalState) -> &'static str {
    match state {
        ApprovalState::Draft => "draft",
        ApprovalState::Submitted { .. } => "submitted",
        ApprovalState::InReview { .. } => "in_review",
        ApprovalState::Approved { .. } => "approved",
        ApprovalState::Rejected { .. } => "rejected",
        ApprovalState::Escalated { .. } => "escalated",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_store() {
        let store = init_store();
        assert!(store.lock().unwrap().is_empty());
    }

    #[test]
    fn test_state_to_string() {
        assert_eq!(state_to_string(&ApprovalState::Draft), "draft");
        assert_eq!(
            state_to_string(&ApprovalState::Submitted { submitted_at: "".to_string() }),
            "submitted"
        );
        assert_eq!(
            state_to_string(&ApprovalState::Approved {
                approved_by: vec![],
                completed_at: "".to_string()
            }),
            "approved"
        );
    }
}
