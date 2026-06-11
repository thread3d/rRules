# Phase 20 Research: Complete Management API & Governance Integration

**Phase Number:** 20  
**Milestone:** v4.1 - Monitoring & Metrics Platform (continued)  
**Date:** 2026-06-10  
**Status:** Research Complete

---

## Executive Summary

This phase completes the REST API layer with full CRUD operations for rules, categories, tags, and approval workflow management. It also integrates governance features (RBAC, audit trails, version control) into the API.

### Key Scope
1. **Rules CRUD** - Create, read, update, delete with proper validation
2. **Categories & Tags** - Taxonomy management for rule organization
3. **Approval Workflows** - Maker-checker pattern implementation
4. **Audit Trails** - Complete modification history tracking
5. **Version Control** - Branch and merge support for rules

### Design Philosophy
- **RESTful design** - Follow HTTP conventions (GET/POST/PUT/DELETE)
- **Versioned API** - `/v1` prefix, clear migration path for v2
- **Error handling** - Consistent error format with HTTP status codes
- **Pagination** - All list endpoints support offset/limit pagination
- **Filtering** - Query parameters for filtering (category, owner, status)
- **Sorting** - Sort by multiple fields with direction control

---

## 1. Rules Management API

### Current State
- `/v1/execute` - Partially implemented (execution only)
- `/v1/rules` - Stub implementations returning `NotImplemented`
- Missing: POST (create), PUT/PATCH (update), DELETE, bulk operations

### Required Endpoints

#### Rules CRUD
| Method | Path | Description |
|--------|------|-------------|
| GET | /v1/rules | List rules with pagination/filtering/sorting |
| GET | /v1/rules/{id} | Get specific rule details |
| POST | /v1/rules | Create new rule |
| PUT | /v1/rules/{id} | Replace existing rule |
| PATCH | /v1/rules/{id} | Partial update rule |
| DELETE | /v1/rules/{id} | Delete rule (soft delete) |
| POST | /v1/rules/{id}/undelete | Restore soft-deleted rule |

#### Rule Execution
| Method | Path | Description |
|--------|------|-------------|
| POST | /v1/execute | Execute rules with facts input |
| POST | /v1/execute/batch | Batch execution for multiple inputs |
| GET | /v1/sessions/{id} | Get session state |
| DELETE | /v1/sessions/{id} | Clear session |

### Rule Schema (Request/Response)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub tags: Vec<String>,
    pub salience: i32,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    pub status: RuleStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
    pub updated_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleStatus {
    Draft,
    Active,
    Inactive,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub expression: String,  // FEEL expression
    pub field: Option<String>,
    pub operator: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub type_: ActionKind,
    pub field: Option<String>,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionKind {
    Assert,
    Modify,
    Retract,
    Log,
}
```

### Validation Rules
1. Rule name must be unique within category
2. Conditions cannot be empty
3. Salience range: -1000 to +1000
4. Category must exist or be auto-created

---

## 2. Categories & Tags Management

### Required Endpoints
| Method | Path | Description |
|--------|------|-------------|
| GET | /v1/categories | List all categories |
| POST | /v1/categories | Create new category |
| GET | /v1/categories/{name} | Get category details |
| PUT | /v1/categories/{name} | Update category |
| DELETE | /v1/categories/{name} | Delete category |

### Category Schema
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub parent_category: Option<String>,  // For hierarchy
    pub created_at: DateTime<Utc>,
}
```

### Tag Schema
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub color: Option<String>,  // Hex color for UI display
    pub description: Option<String>,
}
```

---

## 3. Approval Workflow API (Maker-Checker Pattern)

### Required Endpoints

#### Submission
| Method | Path | Description |
|--------|------|-------------|
| POST | /v1/rules/{id}/submit | Submit rule for review |
| POST | /v1/categories/{name}/submit | Submit category changes |

#### Review
| Method | Path | Description |
|--------|------|-------------|
| GET | /v1/approvals/pending | List pending approvals (reviewer only) |
| GET | /v1/approvals/history | Approval history for a rule |
| POST | /v1/approvals/{id}/approve | Approve change |
| POST | /v1/approvals/{id}/reject | Reject change with feedback |

### Approval Workflow Schema
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalSubmission {
    pub id: String,
    pub rule_id: String,
    pub rule_name: Option<String>,
    pub changes: Vec<ChangeSummary>,
    pub submitted_by: String,
    pub status: SubmissionStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubmissionStatus {
    Pending,
    Approved,
    Rejected,
    Withdrawn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSummary {
    pub field: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}
```

### Approval Flow
1. User submits rule change → creates `ApprovalSubmission`
2. Reviewer receives notification
3. Reviewer approves or rejects
4. If approved, changes are applied to production
5. If rejected, submission returned to author

---

## 4. Audit Trail API

### Required Endpoints
| Method | Path | Description |
|--------|------|-------------|
| GET | /v1/audit | List audit events with filtering |
| GET | /v1/audit/{id} | Get specific audit event |
| GET | /v1/rules/{id}/audit | Audit trail for specific rule |

### Audit Event Schema
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub user_id: Option<String>,
    pub action: AuditAction,
    pub resource_type: ResourceType,
    pub resource_id: String,
    pub resource_name: Option<String>,
    pub details: serde_json::Value,  // Change diff
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditAction {
    Create,
    Read,
    Update,
    Delete,
    Approve,
    Reject,
    Execute,
    Export,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Rule,
    Category,
    Tag,
    User,
    Approval,
}
```

### Audit Requirements
- Store all rule modifications (create/update/delete)
- Record approval actions
- Log execution events with session info
- Preserve audit history even after soft-delete

---

## 5. Version Control API

### Required Endpoints

#### Branching
| Method | Path | Description |
|--------|------|-------------|
| GET | /v1/branches | List all branches |
| POST | /v1/branches | Create new branch |
| GET | /v1/branches/{name} | Get branch details |
| DELETE | /v1/branches/{name} | Delete branch |

#### Merge
| Method | Path | Description |
|--------|------|-------------|
| POST | /v1/branches/{name}/merge | Merge branch to target |
| GET | /v1/rules/{id}/versions | List rule versions across branches |
| POST | /v1/rules/{id}/version/{version}/restore | Restore specific version |

### Version Control Schema
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub base_branch: Option<String>,
    pub current_head: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub is_protected: bool,  // e.g., 'main' branch
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub version_id: String,
    pub rule_id: String,
    pub branch: String,
    pub parent_version: Option<String>,
    pub change_summary: String,
    pub committed_by: String,
    pub committed_at: DateTime<Utc>,
}
```

### Branching Strategy
- `main` - Production branch (protected)
- `develop` - Integration branch
- Feature branches for rule changes
- Merge requests require approval

---

## 6. Implementation Structure

### New API Modules
```
api/src/v1/
├── mod.rs              # Module declarations
├── rules.rs            # Rules CRUD (complete implementation)
├── categories.rs       # Categories management (new)
├── tags.rs             # Tags management (new)
├── approvals.rs        # Approval workflows (new)
├── audit.rs            # Audit trail endpoints (new)
├── branches.rs         # Version control (new)
├── metrics.rs          # Metrics endpoints
└── execution.rs        # Execution endpoints
```

### Models Structure
```
api/src/models/
├── mod.rs              # Module declarations
├── rules.rs            # Rule, Condition, Action types
├── categories.rs       # Category types
├── tags.rs             # Tag types
├── approvals.rs        # Approval submission types
├── audit.rs            # Audit event types
└── branches.rs         # Branch and version types
```

### Error Handling
```rust
#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

// HTTP Status codes:
// 200 OK - Success
// 201 Created - Resource created
// 204 No Content - Success, no response body
// 400 Bad Request - Validation error
// 401 Unauthorized - Missing/invalid auth
// 403 Forbidden - Insufficient permissions
// 404 Not Found - Resource doesn't exist
// 409 Conflict - Version conflict or duplicate
// 422 Unprocessable Entity - Business rule violation
// 500 Internal Server Error
```

---

## 7. Dependencies & Integration

### Engine Integration Points
- Rule validation → `engine::rules::validate_rule()`
- Execution → `EngineBuilder::build().execute()`
- Session management → `WorkingMemory` with session ID

### Storage Requirements
- Rules stored in Git-backed storage
- Audit events in append-only log (PostgreSQL or file)
- Approvals in PostgreSQL for query efficiency
- Branches tracked in Git references

---

## 8. Testing Strategy

### Unit Tests
- Validate rule schema against JSON schema
- Test error responses for invalid input
- Verify pagination works correctly

### Integration Tests
- Complete CRUD flow for rules
- Approval workflow end-to-end
- Audit event generation on mutations
- Branch creation and merge conflicts

---

## 9. Migration Path

### Phase 20 Plans
| Plan | Description | Priority |
|------|-------------|----------|
| 20-01 | Rules CRUD endpoints with full validation | P1 |
| 20-02 | Categories & tags management | P2 |
| 20-03 | Approval workflow integration | P1 |
| 20-04 | Audit trail implementation | P1 |
| 20-05 | Version control (branching/merge) | P2 |

### Success Criteria
1. All rules CRUD operations functional with validation
2. Maker-checker approval workflow operational
3. Complete audit trail for all rule modifications
4. Branch creation and merge functionality working

---

## References

1. **RESTful API Design** - Roy Fielding's dissertation
2. **Maker-Checker Pattern** - Common in financial systems for fraud prevention
3. **Audit Trail Best Practices** - SOX compliance requirements
4. **Git-based Version Control** - DVC, Git LFS patterns for large files

---

*Research completed: 2026-06-10*
