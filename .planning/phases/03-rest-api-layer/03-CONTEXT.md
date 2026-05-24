# Phase 3: REST API Layer - Context

**Gathered:** 2026-05-24
**Status:** Ready for planning

## Phase Boundary

Expose complete REST API for all rule operations and enable container-based deployment. This includes the full CRUD interface for rules, rule execution endpoint with flexible data models, OpenAPI documentation, Docker/Kubernetes deployment support, and hot deployment capabilities.

## Requirements (locked by Phase 2 decisions)

Phase 1 established:
- Hybrid approach: Both API and file-based DSL for rule definition
- Explicit session lifecycle: Stateful/stateless execution modes

Phase 2 established:
- Dual-pane editor with CodeMirror 6 and ag-Grid decision tables
- Guided rule builder with adaptive vocabulary
- Class-based template inheritance

This phase focuses on the HTTP API layer that exposes these capabilities.

## Implementation Decisions

### API Design Approach (D-04)
- **API-D1:** Hybrid REST/GraphQL approach - REST for CRUD operations, GraphQL for rule execution and exploration
- **API-D2:** URL path versioning (/v1/rules) - most common and straightforward for clients
- **API-D3:** Session-based authentication - Cookie-based sessions tied to user login from Phase 4

### Rule Execution Endpoint (D-05)
- **EXEC-F1:** Flexible data model payload - Supports both facts object and session_id; auto-detects mode
- **EXEC-R1:** Full trace output response - Includes execution_id, fired_rules, facts_after, inference_trace for debugging
- **EXEC-A1:** Webhook callbacks for async jobs - Async POST returns immediately, results posted to callback URL when complete

### CRUD Operations (D-05)
- **CRUD-M1:** Standard REST methods - POST /rules (create), GET /rules (list), PUT /rules/{id} (update), DELETE /rules/{id}
- **CRUD-B1:** Conditional bulk operations - POST /rules/bulk with transaction option for all-or-nothing
- **CRUD-F1:** Advanced filtering - ?filter[name]=test&filter[tag]=production for structured queries

### Docker/K8s Deployment (D-06)
- **DEP-I1:** Distroless base image - No shell/pkg manager; highest security, smallest footprint
- **DEP-H1:** Separate /health/live and /health/ready endpoints - K8s-style health checks
- **DEP-N1:** Nginx Ingress controller for production - Most common; good documentation

### Claude's Discretion
- Exact OpenAPI schema details can be finalized during plan implementation
- Specific JWT library choice left to planner/researcher (e.g., jsonwebtoken, go-jose)
- Container registry strategy (Docker Hub, ECR, GCR) deferred to deployment phase

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-Level
- `.planning/PROJECT.md` — Core value, constraints (Rust backend, React frontend), hybrid Git+DB storage
- `.planning/REQUIREMENTS.md` — Phase 3 covers API-01 through API-05

### Phase 1 Context (locked decisions)
- `.planning/phases/01-core-engine-foundation/01-CONTEXT.md` — Hybrid approach, session management

### Phase 2 Context (locked decisions)
- `.planning/phases/02-rule-authoring-interface/02-CONTEXT.md` — Dual-pane editor, ag-Grid decision tables

### Roadmap
- `.planning/ROADMAP.md` — Phase 3 details, success criteria, and plan breakdown

## Existing Code Insights

### Reusable Assets
(None yet — initial project setup; Phases 1-2 will establish patterns)

### Established Patterns
(None yet — first phases of implementation)

### Integration Points
Phase 3 API must integrate with:
- Phase 1 Engine - Rule execution via execute_forward/execute_backward functions
- Phase 2 Web UI - CORS configuration, authentication session validation
- Hybrid Git + PostgreSQL storage (Git for versioning, DB for metadata per PROJECT.md)
- Future Phase 4 - RBAC role checking on endpoints

## Specific Ideas

### API Directory Structure
```
src/api/
├── Cargo.toml              # API crate with actix-web/axum dependency
├── lib.rs                  # Root module exports
├── v1/                     # Versioned API group
│   ├── mod.rs
│   ├── rules.rs           # Rule CRUD endpoints
│   ├── execution.rs       # POST /rules/execute endpoint
│   └── health.rs          # Health check endpoints
├── middleware/
│   ├── auth.rs            # Session-based authentication
│   └── logging.rs         # Request/response logging
└── models/
    ├── rules.rs           # DTOs for rule CRUD
    └── execution.rs       # DTOs for execution requests/responses
```

### Rule Execution Request/Response
```json
// Request - flexible model (auto-detects mode)
{
  "session_id": "abc123",  // OR facts object if stateless
  "actions": ["fire"],
  "webhook_url": "https://example.com/callback"
}

// Response - full trace output
{
  "execution_id": "exec_12345",
  "fired_rules": [{"id": "r1", "name": "Rule A"}],
  "facts_after": {...},
  "inference_trace": [...]
}
```

## Deferred Ideas

### Reviewed Todos (not folded)
(None — no todos were cross-referenced or reviewed)

### General
(None — discussion stayed within phase scope)

---

*Phase: 3-REST API Layer*
*Context gathered: 2026-05-24*
