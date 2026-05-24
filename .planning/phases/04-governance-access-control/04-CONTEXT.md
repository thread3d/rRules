# Phase 4: Governance & Access Control - Context

**Gathered:** 2026-05-24
**Status:** Ready for planning

## Phase Boundary

Add enterprise-grade governance capabilities including access control, approval workflows, and full audit trails. This includes RBAC with predefined roles, maker-checker approval workflows, rule ownership assignment, comprehensive audit trail for all modifications, and version control system with branching and merging support.

## Requirements (locked by Phase 3 decisions)

Phase 1 established:
- Hybrid approach: Both API and file-based DSL for rule definition

Phase 2 established:
- Dual-pane editor with CodeMirror 6 and ag-Grid decision tables
- Guided rule builder with adaptive vocabulary

Phase 3 established:
- REST/GraphQL hybrid API with versioning (/v1/)
- Session-based authentication (cookie-based)
- Full trace execution output with webhook callbacks

This phase focuses on governance capabilities layered on top of the execution API.

## Implementation Decisions

### RBAC Roles and Permissions (D-04)
- **RBAC-A1:** Hybrid approach - Roles for broad access patterns (admin, author, reviewer) plus fine-grained permissions for specific overrides
- **Role Structure:**
  - `admin`: Full system access (all CRUD + approval + configuration)
  - `author`: Create/edit rules, submit for approval; cannot approve own changes
  - `reviewer`: Read rules and approve submissions; cannot create/edit directly

### Maker-Checker Workflow (D-04)
- **MC-W1:** Multi-tier approval - Multiple reviewers at different levels based on rule criticality
- Approval hierarchy:
  - Low-risk rules: Single reviewer approval
  - Medium-risk rules: Two reviewers (one must be senior author)
  - High-risk rules: Three reviewers with voting (majority required)

### Audit Trail Requirements (D-05)
- **AUD-T1:** Full activity log - Everything tracked including rule execution, approval actions, config changes
- Events to track:
  - Rule CRUD operations (create, update, delete, undelete)
  - Approval workflow events (submitted, approved, rejected, escalated)
  - Session access and rule execution
  - Configuration changes and system events

### Version Control Features (D-05)
- **VC-B1:** GitFlow style branching - Main = production, develop = staging, feature branches for changes
- Branch strategy:
  - `main`: Production-ready rules; protected from direct pushes
  - `develop`: Staging environment with latest features
  - `feature/*`: Temporary branches for rule development
  - Merge process: PR review → approval → squash merge to develop

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-Level
- `.planning/PROJECT.md` — Core value, constraints (Rust backend, React frontend), hybrid Git+DB storage

### Phase 1 Context (locked decisions)
- `.planning/phases/01-core-engine-foundation/01-CONTEXT.md` — Hybrid approach, session management

### Phase 2 Context (locked decisions)
- `.planning/phases/02-rule-authoring-interface/02-CONTEXT.md` — Dual-pane editor, ag-Grid decision tables

### Phase 3 Context (locked decisions)
- `.planning/phases/03-rest-api-layer/03-CONTEXT.md` — REST/GraphQL API with session auth

### Roadmap
- `.planning/ROADMAP.md` — Phase 4 details, success criteria, and plan breakdown

## Existing Code Insights

### Reusable Assets
(None yet — initial project setup; Phases 1-3 will establish patterns)

### Established Patterns
(None yet — first phases of implementation)

### Integration Points
Phase 4 must integrate with:
- Phase 1 Engine - Rule execution (now subject to approval)
- Phase 2 Web UI - Show approval status, reviewer interface
- Phase 3 REST API - New endpoints for approvals, audit logs
- Hybrid Git + PostgreSQL storage (Git for versioning, DB for metadata per PROJECT.md)

## Specific Ideas

### Governance Directory Structure
```
src/governance/
├── Cargo.toml
├── lib.rs
├── rbac/                # Role-based access control
│   ├── mod.rs
│   ├── roles.rs        # Admin, Author, Reviewer definitions
│   └── permissions.rs  # Permission enums and checks
├── approval/            # Maker-checker workflow
│   ├── mod.rs
│   ├── workflows.rs    # Multi-tier approval logic
│   └── notifications.rs # Approver notifications
├── audit/               # Audit trail system
│   ├── mod.rs
│   ├── events.rs       # Event types and serialization
│   └── storage.rs      # PostgreSQL audit log
└── versioning/          # GitFlow version control
    ├── mod.rs
    ├── branches.rs     # Branch strategy
    └── merging.rs      # Merge process with approval checks
```

## Deferred Ideas

### Reviewed Todos (not folded)
(None — no todos were cross-referenced or reviewed)

### General
(None — discussion stayed within phase scope)

---

*Phase: 4-Governance & Access Control*
*Context gathered: 2026-05-24*
