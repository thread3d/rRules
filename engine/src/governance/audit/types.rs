//! Shared audit event types for Phase 18 Enterprise Governance
//!
//! This module defines all audit event variants used across the governance system:
//! - RBAC events (role assignments, permission changes)
//! - Rule lifecycle events (creation, updates, deletion)
//! - Approval workflow events (submission, approval, rejection)
//! - Git versioning events (branch creation, merge, conflicts)
//! - System events (user login/logout)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// All audit events for rRules governance system
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AuditEvent {
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

    // Rule lifecycle events (from Phase 3/17)
    RuleCreated {
        rule_id: Uuid,
        rule_name: String,
        category: Option<String>,
        created_by: String,
    },

    RuleUpdated {
        rule_id: Uuid,
        rule_name: String,
        diff: serde_json::Value,
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

    // Git versioning events
    BranchCreated {
        branch_name: String,
        created_by: String,
    },

    BranchMerged {
        source_branch: String,
        target_branch: String,
        commit_sha: String,
        created_by: String,
    },

    MergeConflictDetected {
        source_branch: String,
        target_branch: String,
        conflict_count: i32,
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
    /// Returns the event type string for this event
    pub fn event_type(&self) -> &'static str {
        match self {
            // RBAC
            AuditEvent::RoleAssigned { .. } => "role.assigned",
            AuditEvent::RoleRevoked { .. } => "role.revoked",
            AuditEvent::PermissionChanged { .. } => "permission.changed",

            // Rule lifecycle
            AuditEvent::RuleCreated { .. } => "rule.created",
            AuditEvent::RuleUpdated { .. } => "rule.updated",
            AuditEvent::RuleDeleted { .. } => "rule.deleted",

            // Approval workflow
            AuditEvent::ApprovalSubmitted { .. } => "approval.submitted",
            AuditEvent::ApprovalApproved { .. } => "approval.approved",
            AuditEvent::ApprovalRejected { .. } => "approval.rejected",

            // Git versioning
            AuditEvent::BranchCreated { .. } => "branch.created",
            AuditEvent::BranchMerged { .. } => "branch.merged",
            AuditEvent::MergeConflictDetected { .. } => "merge.conflict",

            // System events
            AuditEvent::UserLogin { .. } => "user.login",
            AuditEvent::UserLogout { .. } => "user.logout",
        }
    }

    /// Returns the occurred timestamp (currentlyUtc::now())
    pub fn occurred_at(&self) -> DateTime<Utc> {
        Utc::now()
    }

    /// Returns the subject ID if this event has one
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

    /// Returns the subject type for this event
    pub fn subject_type(&self) -> &'static str {
        match self {
            AuditEvent::RoleAssigned { .. }
            | AuditEvent::RoleRevoked { .. }
            | AuditEvent::PermissionChanged { .. } => "role",

            AuditEvent::RuleCreated { .. }
            | AuditEvent::RuleUpdated { .. }
            | AuditEvent::RuleDeleted { .. } => "rule",

            AuditEvent::ApprovalSubmitted { .. }
            | AuditEvent::ApprovalApproved { .. }
            | AuditEvent::ApprovalRejected { .. } => "approval",

            AuditEvent::BranchCreated { .. }
            | AuditEvent::BranchMerged { .. }
            | AuditEvent::MergeConflictDetected { .. } => "branch",

            AuditEvent::UserLogin { .. } | AuditEvent::UserLogout { .. } => "user",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_event_types_have_unique_names() {
        let events = vec![
            AuditEvent::RoleAssigned {
                user_id: "1".into(),
                role_name: "admin".into(),
                assigned_by: "2".into(),
            },
            AuditEvent::RuleCreated {
                rule_id: Uuid::new_v4(),
                rule_name: "test".into(),
                category: None,
                created_by: "1".into(),
            },
            AuditEvent::ApprovalSubmitted {
                approval_id: Uuid::new_v4(),
                rule_id: Uuid::new_v4(),
                submitted_by: "1".into(),
                workflow_type: "default".into(),
            },
        ];

        let types: Vec<_> = events.iter().map(|e| e.event_type()).collect();
        let unique_types: std::collections::HashSet<_> = types.iter().collect();

        assert_eq!(types.len(), unique_types.len(), "Event type names must be unique");
    }

    #[test]
    fn test_subject_id_extraction() {
        let rule_event = AuditEvent::RuleCreated {
            rule_id: Uuid::new_v4(),
            rule_name: "test".into(),
            category: None,
            created_by: "1".into(),
        };

        assert!(rule_event.subject_id().is_some());
    }

    #[test]
    fn test_subject_type_extraction() {
        let branch_event = AuditEvent::BranchCreated {
            branch_name: "feature/test".into(),
            created_by: "1".into(),
        };

        assert_eq!(branch_event.subject_type(), "branch");
    }

    #[test]
    fn test_rule_updated_event() {
        let event = AuditEvent::RuleUpdated {
            rule_id: Uuid::new_v4(),
            rule_name: "Test Rule".into(),
            diff: serde_json::json!({"op": "replace", "path": "/name"}),
            updated_by: "user123".into(),
        };

        assert_eq!(event.event_type(), "rule.updated");
        assert_eq!(event.subject_type(), "rule");
    }

    #[test]
    fn test_approval_rejected_event() {
        let event = AuditEvent::ApprovalRejected {
            approval_id: Uuid::new_v4(),
            rejected_by: "reviewer1".into(),
            reason: Some("Invalid syntax".into()),
        };

        assert_eq!(event.event_type(), "approval.rejected");
        assert_eq!(event.subject_type(), "approval");
    }
}
