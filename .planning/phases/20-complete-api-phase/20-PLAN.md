# Phase 20: Complete Management API & Governance Integration

**Phase Number:** 20  
**Milestone:** v4.1 - Monitoring & Metrics Platform (continued)  
**Target Completion:** 2026-06-25  
**Research Document:** `20-RESEARCH.md`

---

## Goal

Complete the REST API layer with full CRUD operations for rules, categories, tags, and governance features including approval workflows, audit trails, and version control.

---

## Requirements Addressed

| Requirement | Priority |
|-------------|----------|
| API-01: Complete Rule Management API | P1 |
| GOV-02: Maker-Checker Approval Workflow | P1 |
| GOV-03: Full Audit Trail | P1 |
| GOV-04: Version Control & Branching | P2 |

---

## Success Criteria (what must be TRUE)

1. All rules CRUD endpoints return proper HTTP status codes and error responses
2. Maker-checker approval workflow can submit, approve, and reject rule changes
3. Every API mutation generates an audit event with full context
4. Branch creation, merging, and conflict resolution functional

---

## Plans

### Phase 20-01: Rules CRUD Implementation (P1)

**Goal:** Complete REST endpoints for rule lifecycle management.

**Plans:**
- 20-01-01: Implement GET /v1/rules with pagination, filtering, sorting
- 20-01-02: Implement POST /v1/rules with validation and default values
- 20-01-03: Implement PUT/PATCH /v1/rules/{id} for updates
- 20-01-04: Implement DELETE /v1/rules/{id} (soft delete)
- 20-01-05: Add rule validation engine integration

**Files:**
- `api/src/v1/rules.rs` - Complete CRUD implementation
- `api/src/models/rules.rs` - Request/Response types
- `api/src/lib.rs` - Export new models

**Acceptance Criteria:**
- Rules can be listed with offset/limit pagination
- Rules filterable by category, status, tags
- Rules sortable by name, salience, created_at
- Validation prevents invalid rule definitions

---

### Phase 20-02: Categories & Tags Management (P2)

**Goal:** Enable organization of rules through categories and tags.

**Plans:**
- 20-02-01: Implement categories CRUD endpoints
- 20-02-02: Implement tag management (create, list, assign)
- 20-02-03: Add category hierarchy support

**Files:**
- `api/src/v1/categories.rs` - Category endpoints
- `api/src/v1/tags.rs` - Tag endpoints
- `api/src/models/categories.rs` - Category types
- `api/src/models/tags.rs` - Tag types

**Acceptance Criteria:**
- Categories can be created with optional parent for hierarchy
- Tags support color coding and description
- Rules can be filtered by category/tag

---

### Phase 20-03: Approval Workflow Integration (P1)

**Goal:** Implement maker-checker approval workflow.

**Plans:**
- 20-03-01: Design approval submission schema
- 20-03-02: Implement POST /v1/rules/{id}/submit
- 20-03-03: Implement GET /v1/approvals/pending (reviewer view)
- 20-03-04: Implement POST /v1/approvals/{id}/approve
- 20-03-05: Implement POST /v1/approvals/{id}/reject

**Files:**
- `api/src/v1/approvals.rs` - Approval workflow endpoints
- `api/src/models/approvals.rs` - Submission/response types
- `engine/src/governance/approval.rs` - Engine integration

**Acceptance Criteria:**
- Rule submissions create approval record with diff
- Reviewers can approve/reject with comments
- Approved changes apply to production

---

### Phase 20-04: Audit Trail Implementation (P1)

**Goal:** Record all API mutations for compliance.

**Plans:**
- 20-04-01: Design audit event schema
- 20-04-02: Implement middleware/logger for event capture
- 20-04-03: Implement GET /v1/audit with filtering
- 20-04-04: Store audit events in PostgreSQL

**Files:**
- `api/src/v1/audit.rs` - Audit endpoints
- `api/src/models/audit.rs` - Audit event types
- `engine/src/governance/audit.rs` - Audit storage

**Acceptance Criteria:**
- All rule modifications logged with user, timestamp, diff
- Approval actions recorded
- Events queryable by resource type/user/time range

---

### Phase 20-05: Version Control (P2)

**Goal:** Branch and merge support for rule development.

**Plans:**
- 20-05-01: Implement branch CRUD endpoints
- 20-05-02: Implement merge logic with conflict detection
- 20-05-03: Add version history endpoint

**Files:**
- `api/src/v1/branches.rs` - Branch management
- `api/src/models/branches.rs` - Branch types
- Git integration for actual branching

**Acceptance Criteria:**
- Branches can be created from any commit
- Merge with conflict detection and resolution
- Version history tracked per rule

---

### Phase 20-06: API Documentation & Testing (P1)

**Goal:** Complete documentation and test coverage.

**Plans:**
- 20-06-01: Update OpenAPI/Swagger spec for all new endpoints
- 20-06-02: Write integration tests for CRUD flows
- 20-06-03: Write approval workflow E2E tests

**Files:**
- `api/openapi/v1.yaml` - API specification
- `tests/integration/rules_test.rs`
- `tests/integration/approvals_test.rs`

---

## Dependencies

| Phase | Status |
|-------|--------|
| Phase 19: Monitoring & Metrics | Should be complete (metrics endpoints already exist) |
| Phase 18: Enterprise Governance | Should be mostly complete (approval workflow exists in engine) |

---

## Success Metrics

- **API Response Time:** < 100ms for CRUD operations
- **Audit Event Write:** < 50ms latency
- **Coverage:** All new endpoints have integration tests
- **Documentation:** OpenAPI spec includes all parameters and examples

---

## Notes

- Soft-delete rules (mark as archived) rather than hard delete
- Approval workflow should support batch approvals for efficiency
- Audit events should be written asynchronously to not block API responses
- Branches use Git references under `refs/rules/` namespace

---

## Implementation Checklist

### Phase 20-01: Rules CRUD
- [ ] GET /v1/rules with pagination/filtering/sorting
- [ ] POST /v1/rules with validation
- [ ] PUT/PATCH /v1/rules/{id}
- [ ] DELETE /v1/rules/{id} (soft delete)
- [ ] Validation engine integration

### Phase 20-02: Categories & Tags
- [ ] Categories CRUD endpoints
- [ ] Tag management endpoints

### Phase 20-03: Approval Workflow
- [ ] Submission endpoint
- [ ] Review/approve/reject endpoints
- [ ] Notification integration (email/Slack)

### Phase 20-04: Audit Trail
- [ ] Event schema and storage
- [ ] Query endpoints

### Phase 20-05: Version Control
- [ ] Branch creation/list/delete
- [ ] Merge with conflict detection
- [ ] Version history

---

*Phase planned: 2026-06-10*
