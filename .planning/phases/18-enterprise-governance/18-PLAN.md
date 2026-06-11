# Phase 18: Enterprise Governance - Plan

**Phase:** 18  
**Goal:** Production-ready enterprise governance (RBAC, approval workflows, audit trails, Git versioning)  
**Milestone:** v4.0 - Production Platform  
**Status:** **PLANNING COMPLETE**  

---

## Overview

Phase 18 delivers the complete enterprise governance stack for rRules BRMS:
- **18-01 RBAC**: Role-based access control with PostgreSQL storage
- **18-02 Approval Workflow**: Maker-checker pattern with multi-tier support
- **18-03 Audit Trail**: Event-sourced immutable logging
- **18-04 Git Versioning**: Branch-based development with conflict detection

This phase builds on Phase 17's frontend foundation to provide production-grade governance features required for enterprise adoption.

---

## Success Criteria (What Must Be TRUE)

1. Users have distinct roles (admin/author/reviewer/viewer) with different API access levels
2. Rule changes require submission and approval before production deployment
3. Every system activity is recorded in an immutable audit trail
4. Rules can be developed in branches and merged to main with conflict detection
5. Frontend UI reflects user permissions and hides unauthorized features

---

## Phase Plans Summary

| Sub-Plan | Status | Description |
|----------|--------|-------------|
| 18-01 RBAC Implementation | **IN PROGRESS** | Role-based access control with JWT middleware - Backend complete, frontend pending |
| 18-02 Approval Workflow | **TODO** | Maker-checker workflow with webhooks as primary triggers |
| 18-03a Audit Event Types | **COMPLETE** | Shared audit event types (Wave 0b) |
| 18-03 Audit Trail | **IN PROGRESS** | Event-sourced immutable logging using shared types - Backend store complete, queries pending |
| 18-04 Git Versioning | **TODO** | Branch-based version control integration |

**Dependencies:**
- Phase 17 (Frontend Foundation) - Required for UI components
- 18-01 must complete before 18-02/18-03/18-04 (RBAC foundation)
- **18-03a Audit Event Types must complete BEFORE 18-02 and 18-04** (shared types)

---

## Implementation Wave Breakdown

### Wave 0: Database Schema (Shared)
All plans require initial database migrations:

| Migration | Purpose |
|-----------|---------|
| `18_01_rbac_schema.sql` | roles, permissions, role_assignments tables |
| `18_02_approval_schema.sql` | approval_workflows, approval_requests tables |
| `18_03_audit_schema.sql` | audit_events append-only table |
| `18_04_git_schema.sql` | Optional: branches tracking table |

**Note:** These migrations can run in any order as they're independent.

### Wave 0b: Shared Audit Event Types (NEW - MUST COME FIRST)
Define audit event types BEFORE dependent plans consume them:

| Plan | File | Purpose |
|------|------|---------|
| **18-03a** | `src/governance/audit/types.rs` | AuditEvent enum with ALL variants |

This resolves circular dependencies where both approval workflow and Git versioning need to reference audit event types.

---

### Wave 1: Backend Domain Modules
Each plan has its own domain module:

| Plan | Module Directory | Key Files |
|------|------------------|-----------|
| 18-01 | `src/governance/rbac/` | mod.rs, roles.rs, permissions.rs, store.rs |
| 18-03a | `src/governance/audit/types.rs` | Shared AuditEvent enum |
| 18-02 | `src/governance/approval/` | mod.rs, engine.rs, webhooks.rs, notifications.rs |
| 18-03 | `src/governance/audit/` | store.rs, queue.rs (uses types from 18-03a) |
| 18-04 | `src/governance/versioning/` | mod.rs, repository.rs, merge.rs |

**Note:** 18-02 depends on 18-01's RBAC middleware for permission checks.
**Note:** 18-03 uses types from 18-03a (shared module).
**Note:** 18-04 uses types from 18-03a for branch audit events.

---

### Wave 2: API Endpoints

| Plan | Endpoints |
|------|-----------|
| 18-01 RBAC | `/api/v1/rbac/roles`, `/api/v1/rbac/users/{id}/roles` |
| 18-02 Approval | `/webhooks/approval/create` (primary), `/api/v1/approvals` (query) |
| 18-03 Audit | `/api/v1/audit/events`, `/api/v1/audit/export` |
| 18-04 Git | `/api/v1/branches`, `/api/v1/branches/{name}/merge` |

### Wave 2b: Audit Integration Points (NEW)

**Critical:** Add `audit.append_event()` calls in these locations:

| Location | Event Type | Trigger |
|----------|------------|---------|
| Rule CRUD endpoints | `rule.created`, `rule.updated`, `rule.deleted` | Rule modifications |
| Approval workflow state changes | `approval.submitted`, `approval.approved`, `approval.rejected` | Workflow transitions |
| RBAC role assignments | `role.assigned`, `role.revoked` | Role changes |

**Example pattern (applies to all plans):**

```rust
// After successful rule update:
audit_store.append_event(&AuditEvent::RuleUpdated {
    rule_id,
    rule_name: rule.name.clone(),
    diff: json_patch,  // JSON diff of changes
    updated_by: claims.sub.clone(),
}).await?;
```

---

### Wave 3: Frontend Components
React components mirroring backend structure:

| Plan | Frontend Directory | Key Components |
|------|-------------------|----------------|
| 18-01 RBAC | `features/governance/rbac/` | RoleManager, ProtectedRoute, usePermission |
| 18-02 Approval | `features/governance/approval/` | ApprovalQueue, SubmitForApproval, WebhookConfig |
| 18-03 Audit | `features/governance/audit/` | AuditLogViewer, ExportDialog |
| 18-04 Git | `features/versioning/` | BranchManager, ConflictViewer |

---

## Technical Stack

### Backend
| Component | Library | Purpose |
|-----------|---------|---------|
| Authentication | actix-jwt v0.3.0 | JWT token validation |
| RBAC | Custom middleware | Role-based authorization |
| Git Integration | git2 v0.21.0 | libgit2 bindings |
| Audit Store | evented crate | Event sourcing abstraction |
| Kafka (optional) | rdkafka v0.36 | Event streaming |

### Frontend
| Component | Library | Status |
|-----------|---------|--------|
| Routing | @tanstack/react-router | Phase 17 dependency |
| Icons | lucide-react | Already in package.json |
| HTTP Client | axios | Already configured |

---

## Testing Strategy

### Test Coverage Per Plan

| Plan | Unit Tests | Integration Tests | Benchmark Tests |
|------|------------|-------------------|-----------------|
| 18-01 RBAC | ✅ middleware checks, role queries | API endpoint coverage | N/A |
| 18-02 Approval | ✅ state machine transitions | Webhook dispatch, escalation | None needed |
| 18-03 Audit | ✅ event serialization | Query filters, export formats | Batch write performance |
| 18-04 Git | ✅ branch creation, merge | Conflict detection, clone operations | Repository operations |

### Test Commands
```bash
# Run all governance tests
cargo test -- governance::

# Run specific plan tests
cargo test governance::rbac::
cargo test governance::approval::engine::tests
cargo test governance::audit::types

# Benchmarks (if added)
cargo bench -- governance::audit::queue
```

---

## Migration Strategy

### Phase 18 Rollout Order (MUST BE EXACT)

1. **Database Migrations** (run together before code deployment)
   - Run all migrations: `18_01_rbac_schema.sql`, `18_02_approval_schema.sql`, `18_03_audit_schema.sql`
   - Verify table creation and constraints

2. **Shared Types** (MUST be first backend module)
   - Build and deploy 18-03a audit event types
   - All other modules depend on this

3. **Backend Services** (deploy in dependency order)
   - Deploy RBAC module first (foundation for others)
   - Deploy Approval workflow (depends on RBAC + shared types)
   - Deploy Git versioning (uses shared types for branch events)

4. **Frontend Components** (roll out as features)
   - Enable route guards based on RBAC
   - Add approval queue UI
   - Enable audit log viewer for admins

---

## Rollback Plan

### Database Rollback
```sql
-- Drop all Phase 18 tables (in reverse dependency order)
DROP TABLE IF EXISTS audit_events CASCADE;
DROP TABLE IF EXISTS approval_webhooks CASCADE;
DROP TABLE IF EXISTS approval_requests CASCADE;
DROP TABLE IF EXISTS approval_workflows CASCADE;
DROP TABLE IF EXISTS role_assignments CASCADE;
DROP TABLE IF EXISTS permissions CASCADE;
DROP TABLE IF EXISTS roles CASCADE;
```

### Code Rollback
- Git branches: Use `git revert` or tag release before Phase 18
- Database migrations: Use `sqlx migrate revert` if using sqlx CLI

---

## Known Limitations & Future Enhancements

### Phase 18 (Current)
- No multi-tenant isolation (single organization only)
- Webhook delivery has retry logic but no persistence for failures
- Git operations block on repository access (no async pooling)

### Post-Phase 18 (Future)
- WebSocket subscriptions for real-time RBAC updates
- Multi-tenant governance with organization-scoped roles
- Advanced permission modeling (RBAC + ABAC hybrid)
- Smart branch merge recommendations based on change similarity

---

## Acceptance Checklist (Pre-Delivery)

### Phase 18 - All Plans Complete
- [ ] All sub-plan PLAN.md files are created and reviewed
- [ ] Database migrations run successfully in development
- [ ] Backend modules compile: `cargo check --all`
- [ ] Unit tests pass for all governance modules
- [ ] Integration tests cover critical user workflows
- [ ] Frontend components render with proper permission checks

### Phase 18 - Quality Gates Passed
- [ ] Security audit: No hardcoded secrets, proper authz checks
- [ ] Performance review: No N+1 queries in API endpoints
- [ ] Documentation: API docs and admin guides updated

---

## Stakeholder Sign-off

| Role | Name | Signature |
|------|------|-----------|
| Product Owner | | |
| Tech Lead | | |

---

*Phase Plan Created: 2026-06-10*
*Phase Number: 18*
*Milestone: v4.0 - Production Platform*
