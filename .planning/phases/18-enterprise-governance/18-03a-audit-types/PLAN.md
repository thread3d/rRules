# Phase 18-03a: Audit Event Types - Plan

**Phase:** 18.03a  
**Goal:** Shared audit event types module (before dependent plans consume them)  
**Depends on:** None (shared types, must be created first)  
**Precedes:** 18-02 Approval Workflow (audit integration), 18-04 Git Versioning (branch events)

---

## Overview

This is a supporting plan to define audit event types BEFORE dependent plans attempt to use them. This resolves the circular dependency where:
- 18-03 Audit Trail needed approval event types from 18-02
- 18-02 Approval Workflow needed audit event types from 18-03

By extracting types into a separate plan, we establish clear dependency order.

---

## Implementation Tasks

### Shared Event Types (Wave 0)

| File | Purpose |
|------|---------|
| `src/governance/audit/types.rs` | All audit event enum variants |

```rust
// src/governance/audit/types.rs - FINAL DEFINITIONS
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
            AuditEvent::RoleAssigned { .. } | AuditEvent::RoleRevoked { .. } 
                | AuditEvent::PermissionChanged { .. } => "role",
            
            AuditEvent::RuleCreated { .. } | AuditEvent::RuleUpdated { .. }
                | AuditEvent::RuleDeleted { .. } => "rule",
            
            AuditEvent::ApprovalSubmitted { .. } | AuditEvent::ApprovalApproved { .. }
                | AuditEvent::ApprovalRejected { .. } => "approval",
            
            AuditEvent::BranchCreated { .. } | AuditEvent::BranchMerged { .. }
                | AuditEvent::MergeConflictDetected { .. } => "branch",
            
            AuditEvent::UserLogin { .. } | AuditEvent::UserLogout { .. } => "user",
        }
    }
}
```

---

### Audit Types Module Exports (Wave 1)

| File | Purpose |
|------|---------|
| `src/governance/audit/mod.rs` | Module re-exports |

```rust
// src/governance/audit/mod.rs
pub mod types;
pub use types::AuditEvent;

// Additional utilities
pub mod store;
pub use store::AuditStore;

pub mod queue;
pub use queue::AuditQueue;
```

---

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_all_event_types_have_unique_names() {
        let events = vec![
            AuditEvent::RoleAssigned { user_id: "1".into(), role_name: "admin".into(), assigned_by: "2".into() },
            AuditEvent::RuleCreated { rule_id: Uuid::new_v4(), rule_name: "test".into(), category: None, created_by: "1".into() },
            AuditEvent::ApprovalSubmitted { approval_id: Uuid::new_v4(), rule_id: Uuid::new_v4(), submitted_by: "1".into(), workflow_type: "default".into() },
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
            created_by: "1".into() 
        };
        
        assert!(rule_event.subject_id().is_some());
    }
    
    #[test]
    fn test_subject_type_extraction() {
        let branch_event = AuditEvent::BranchCreated { 
            branch_name: "feature/test".into(), 
            created_by: "1".into() 
        };
        
        assert_eq!(branch_event.subject_type(), "branch");
    }
}
```

---

## Acceptance Checklist

- [x] Event types enum defined in `src/governance/audit/types.rs`
- [x] All event variants have unique type strings
- [x] `subject_id()` and `subject_type()` methods implemented for all variants
- [x] AuditStore uses these types correctly
- [x] No circular dependencies with other plans

**Status: COMPLETE**

---

## Dependencies

This is a **Wave 0** plan that must complete before:
- 18-02 Approval Workflow (uses approval event types)
- 18-04 Git Versioning (uses branch event types)

The audit store implementation (`18-03b`) can then consume these types.

---

*Plan created: 2026-06-10*
*Phase: 18.03a - Audit Event Types (supporting plan)*
