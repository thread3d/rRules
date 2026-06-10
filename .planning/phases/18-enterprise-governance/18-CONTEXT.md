# Phase 18: Enterprise Governance - Context

**Gathered:** 2026-06-10
**Status:** Ready for planning

## Phase Boundary

Deliver production-ready enterprise governance capabilities for the rRules BRMS:
- **GOV-01:** Complete RBAC implementation with predefined roles and permissions (admin, author, reviewer, viewer)
- **GOV-02:** Maker-checker approval workflow engine with multi-tier approvals
- **GOV-03:** Full audit trail of all system activities (immutable append-only log)
- **GOV-04:** Version control system with Git-backed branching and merging

This phase builds upon Phase 17's frontend architecture to provide enterprise-grade governance features.

## Implementation Decisions

### RBAC Storage Strategy (D-18-01)
- **Decision:** Store roles and permissions in PostgreSQL database
- **Rationale:** Dynamic updates without service restart, supports runtime role assignment changes
- **Implementation:** Roles table with foreign key to users, permissions stored as JSON array or separate lookup table

### Approval Workflow Triggers (D-18-02)
- **Decision:** Webhooks/callbacks in addition to REST API triggers
- **Rationale:** Enables integration with external notification systems (Slack, email) and external approval workflows
- **Implementation:** Webhook configuration endpoint for registering callback URLs; event bus pattern for dispatching approval events

### Audit Storage Backend (D-18-03)
- **Decision:** Event stream (Kafka/Redpanda) for high-volume production deployments
- **Rationale:** Scalable, fault-tolerant, supports multiple consumers (monitoring, compliance, analytics)
- **Implementation:** Audit events published to Kafka topic; consumer services for long-term storage in PostgreSQL and archival

### Version Control Integration (D-18-04)
- **Decision:** Direct Git integration from API layer
- **Rationale:** Transparent version history, standard Git workflows, no additional service overhead
- **Implementation:** Git operations via libgit2 or git CLI; branches for development, main for production

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-Level
- `.planning/PROJECT.md` — Core value, constraints (Rust backend, React frontend), hybrid Git+DB storage

### Phase 4 Context (previous governance implementation)
- `.planning/phases/04-governance-access-control/04-CONTEXT.md` — Previous RBAC and approval workflow stubs; D-04 to D-05 decisions

### Phase 17 Context (frontend foundation)
- `.planning/phases/17-production-frontend/17-PLAN.md` — React component architecture, routing structure
- `.planning/REQUIREMENTS.md` §FRONTEND-02 — Dashboard patterns requiring governance UI elements

### Roadmap
- `.planning/ROADMAP.md` §Phase 18 — Phase goal and success criteria
- `.planning/REQUIREMENTS.md` §GOV-01 through GOV-04 — Acceptance criteria for each requirement

## Existing Code Insights

### Reusable Assets
- `frontend/src/hooks/use-permission.ts` — Permission checking hook; extend to support DB-stored permissions
- `frontend/src/lib/auth-context.tsx` — Auth context with basic role checks; update for admin/author/reviewer/viewer roles
- `frontend/src/types/auth.ts` — User type with role and permissions; extends to include DB sync fields

### Established Patterns
- **Hybrid Git + PostgreSQL storage** (PROJECT.md): Git for versioning rules as code, DB for runtime metadata
- **React + TypeScript frontend** (Phase 17): Component-based architecture with TanStack Router
- **Rust backend with Actix Web** (Phase 3): REST API structure already established

### Integration Points
- Phase 4 Governance (existing stubs) — Replace stub implementations with production logic
- Phase 17 Frontend (completed) — Add governance UI components to navigation and rule management
- Database migrations — New tables: `roles`, `role_assignments`, `approval_workflows`, `audit_events`

## Specific Ideas

### Directory Structure
```
src/governance/
├── Cargo.toml
├── lib.rs
├── rbac/                # PostgreSQL-backed RBAC
│   ├── mod.rs
│   ├── roles.rs        # Role definitions and queries
│   └── permissions.rs  # Permission enums and checks
├── approval/            # Maker-checker workflow with webhook support
│   ├── mod.rs
│   ├── engine.rs       # Workflow state machine
│   ├── webhooks.rs     # Webhook dispatch logic
│   └── notifications.rs # Email/Slack integrations
├── audit/               # Event stream integration
│   ├── mod.rs
│   ├── events.rs       # Audit event types
│   └── producer.rs     # Kafka producer
└── versioning/          # Git integration
    ├── mod.rs
    ├── repository.rs   # Git operations wrapper
    └── merge.rs        # Branch merging logic
```

### Frontend Components to Add (Phase 17 + 18)
- `frontend/src/features/governance/` directory structure mirroring Rust modules
- RBAC management UI (role assignment, permission editing)
- Approval workflow dashboard with webhook configuration
- Audit log viewer with filtering and export

## Deferred Ideas

### Reviewed Todos (not folded)
(None — no todos were cross-referenced for Phase 18)

### Future Phases / Milestones
- Real-time RBAC sync via WebSocket subscriptions — deferred to monitoring phase
- Multi-tenant governance isolation — deferred to v5.0 milestone
- Advanced rule-based permissions (e.g., "can approve rules in category X") — deferred

---

*Phase: 18-Enterprise Governance*
*Context gathered: 2026-06-10*
