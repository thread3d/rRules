# Phase 3: REST API Layer - Context (Updated)

**Gathered:** 2026-05-24  
**Updated:** 2026-06-07  
**Status:** Ready for completion

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
- **API-D1:** REST for CRUD operations, GraphQL for rule execution and exploration
- **API-D2:** URL path versioning (/v1/rules) - most common and straightforward
- **API-D3:** Session-based authentication via cookies tied to user login from Phase 4

### Rule Execution Endpoint (D-05)
- **EXEC-F1:** Flexible data model payload - Supports both facts object and session_id; auto-detects mode
- **EXEC-R1:** Full trace output response - Includes execution_id, fired_rules, facts_after, inference_trace for debugging
- **EXEC-A1:** Webhook callbacks for async jobs

### CRUD Operations (D-05)
- **CRUD-M1:** Standard REST methods - POST /rules (create), GET /rules (list), PUT /rules/{id} (update), DELETE /rules/{id}
- **CRUD-B1:** Conditional bulk operations
- **CRUD-F1:** Advanced filtering

### Docker/K8s Deployment (D-06) - UPDATED
- **DEP-I1:** Distroless base image - No shell/pkg manager; highest security, smallest footprint
- **DEP-H1:** Separate /health/live and /health/ready endpoints - K8s-style health checks
- **DEP-N1:** Nginx Ingress controller for production
- **K8S-A1:** All three deployment approaches: basic YAML manifests, Helm chart, Kustomize overlay

### Hot Deployment (UPDATED)
- **HOT-A1:** API endpoint trigger - POST /v1/rules/reload endpoint manually triggers rule reloading
- **HOT-C1:** Reload preserves existing sessions and working memory state

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
The API crate structure is already established:
```
src/api/
├── Cargo.toml              # actix-web dependency configured
├── lib.rs                  # Root module exports (models, v1)
├── main.rs                 # Entry point with actix_web setup
└── v1/                     # Versioned API group
    ├── mod.rs              # Route modules
    ├── rules.rs            # Rule CRUD endpoints (stub implementations)
    └── execution.rs        # Execution endpoint with DTOs
```

### Established Patterns
- Actix-web 4.4 for async HTTP handling
- Serde for JSON serialization/deserialization
- UUID v4 for request IDs

### Integration Points
Phase 3 API must integrate with:
- Phase 1 Engine - Rule execution via execute_forward/execute_backward functions
- Phase 2 Web UI - CORS configuration, authentication session validation
- Hybrid Git + PostgreSQL storage (Git for versioning, DB for metadata)
- Future Phase 4 - RBAC role checking on endpoints

## Deferred Ideas

### Reviewed Todos
None — no todos were cross-referenced or reviewed in this update.

### General
(None — discussion stayed within phase scope)

---
*Phase: 3-REST API Layer (Continuation)*  
*Context updated: 2026-06-07*
