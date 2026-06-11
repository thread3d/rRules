# Phase 21: Deployment & CI/CD - Context

**Gathered:** 2026-06-11  
**Status:** Ready for Planning

## Phase Boundary

Deliver production deployment infrastructure for the rRules BRMS:
- **DEPLOY-03:** Optimized Docker image with multi-stage builds, layer caching
- **DEPLOY-04:** Complete Kubernetes manifests (Deployment, Service, ConfigMap, HPA)
- **API-05:** Hot reload endpoint for rule updates without container restart

This phase builds upon the completed API layer (Phase 20) and infrastructure foundation to provide production-ready deployment tooling.

## Implementation Decisions

### Base Image Strategy (D-21-01)
- **Decision:** Use Alpine Linux with musl static linking
- **Rationale:** Balance of small image size (~50-80MB), shell access for debugging, and compatibility
- **Implementation:** 
  - Build stage: `rust:1.79-alpine` with musl-tools
  - Runtime stage: `alpine:3.19`
  - Static linking via `musl-utils`

### Kubernetes Deployment Approach (D-21-02)
- **Decision:** Provide both plain YAML manifests AND Helm charts
- **Rationale:** 
  - Plain YAML for reference, single-environment deployments
  - Helm for multi-environment (dev/staging/prod) with values customization
- **Implementation:**
  - `k8s/` directory contains all plain YAML manifests
  - `k8s/helm/` directory contains complete Helm chart with values templates

### Hot Reload Behavior (D-21-03)
- **Decision:** Async reload with job tracking
- **Rationale:** 
  - Rule set recompilation may take seconds for large rule bases
  - Immediate API response prevents client timeouts
  - Job status endpoint allows monitoring progress
- **Implementation:**
  - POST /v1/rules/reload returns job ID immediately
  - GET /v1/rules/reload/{job_id} returns status (pending/running/completed/failed)
  - Completion response includes rules reloaded count and timing

### Docker Optimization Priority (D-21-04)
- **Decision:** Minimize runtime image size as top priority
- **Rationale:** 
  - Faster container pulls reduce deployment time
  - Smaller attack surface improves security posture
  - Reduced storage costs in registry
- **Implementation:**
  - Multi-stage build with distroless/runtime stages
  - Static linking to eliminate libc dependency
  - Remove build tools from final image

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-Level
- `.planning/PROJECT.md` — Core value, constraints (Rust backend), hybrid Git+DB storage
- `.planning/REQUIREMENTS.md` — Requirements DEPLOY-03, DEPLOY-04, API-05

### Phase Context (previous governance implementation)
- `.planning/phases/18-enterprise-governance/18-CONTEXT.md` — Existing K8s patterns, audit trail integration
- `.planning/phases/20-complete-api-phase/` — API structure for hot reload endpoint

### Roadmap
- `.planning/ROADMAP.md` §Phase 21 — Phase goal and success criteria
- `.planning/REQUIREMENTS.md` §DEPLOY-03, DEPLOY-04, API-05 — Acceptance criteria

## Existing Code Insights

### Reusable Assets
- `k8s/configmap.yaml` — ConfigMap already exists with environment variables
- `k8s/deployment.yaml` — Deployment with liveness/readiness probes
- `k8s/service-account.yaml` — RBAC role and binding for minimal permissions
- `k8s/hpa.yaml` — HorizontalPodAutoscaler with CPU/memory metrics
- `api/src/v1/rules.rs` — Rules CRUD endpoints to extend with reload operation

### Established Patterns
- **Hybrid Git + PostgreSQL storage** (PROJECT.md): Git for versioning rules as code, DB for runtime metadata
- **React + TypeScript frontend** (Phase 17): Component-based architecture with TanStack Router
- **Rust backend with Actix Web** (Phase 3): REST API structure already established

### Integration Points
- Phase 20 API (completed) — Add reload endpoint to rules.rs module
- Dockerfile (existing) — Modify for multi-stage build optimization
- k8s/ manifests (existing) — Create Helm chart templates

## Deferred Ideas

### Reviewed Todos (not folded)
(None — no todos were cross-referenced for Phase 21)

### Future Phases / Milestones
- GitOps integration with ArgoCD or Flux — deferred to v5.0
- Certificate management via cert-manager — deferred to production deployment phase
- Service mesh integration (Istio/Linkerd) — deferred to advanced networking phase

---

*Phase: 21-Deployment & CI/CD*
*Context gathered: 2026-06-11*
