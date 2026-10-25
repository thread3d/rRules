---
milestone: v4.0
milestone_name: Production Platform
audited: 2026-07-01
status: gaps_found
scores:
  requirements: 4/13
  phases: 4/5
  integration: 3/12
  flows: 2/10
root_cause: "Frontend (Phase 17) is a scaffolding skeleton — components exist but are orphaned from the router and backend. A systematic baseURL mismatch (frontend /api/v1 vs backend /v1) breaks every axios flow at runtime, compounded by an incomplete router (only / and /rules registered), an unmounted AuthProvider, and three governance UIs that are stub/mock-only."
gaps:
  requirements:
    - id: "FRONTEND-01"
      status: partial
      phase: 17
      claimed_by_plans: ["17-PLAN.md"]
      completed_by_plans: ["17-01-SUMMARY.md"]
      verification_status: passed
      evidence: "RuleEditor/DecisionTable/HitPolicySelector components exist but are NEVER imported by any route or feature (orphaned). No route, no save flow. Plus the global baseURL /api/v1 vs backend /v1 mismatch → 404. Requirement (rule authoring interface with editing features) cannot be verified end-to-end."
    - id: "FRONTEND-02"
      status: partial
      phase: 17
      claimed_by_plans: ["17-PLAN.md"]
      completed_by_plans: ["17-01-SUMMARY.md", "17-02-SUMMARY"]
      verification_status: passed
      evidence: "useRules → getRules → apiClient.get('/rules') is code-wired, but runtime 404 (baseURL /api/v1 vs /v1). The /rules route ALSO crashes on render — usePermission('rule:create') calls useAuth() which throws because AuthProvider is never mounted (main.tsx:19-26). Create/Update/Delete UI unreachable (RuleEditor orphaned)."
    - id: "GOV-01"
      status: partial
      phase: 18
      claimed_by_plans: ["18-PLAN.md"]
      completed_by_plans: ["18-SUMMARY.md"]
      verification_status: passed
      evidence: "RBAC domain model exists (engine/src/governance/rbac) but is NOT enforced: api/src/middleware/auth.rs:6 defines requires_auth but it is never wrap()'ed or referenced in main.rs. Frontend sends a Bearer token (client.ts:13-24) but backend never validates it. Frontend usePermission calls useAuth() which throws (no AuthProvider). RBAC is data structures, not request gating."
    - id: "GOV-02"
      status: partial
      phase: 18
      claimed_by_plans: ["18-PLAN.md"]
      completed_by_plans: ["18-SUMMARY.md"]
      verification_status: passed
      evidence: "Backend approval workflow engine + endpoints exist and are registered (main.rs:114-117). Frontend ApprovalQueue.tsx:17 correctly calls /approvals/pending (stale path-mismatch issue #2 RESOLVED at path level) and the response contract matches (approvals.rs:153-155 returns {approvals:[...]}, ApprovalQueue reads response.data.approvals). BUT ApprovalQueue is not mounted in any route AND baseURL /api/v1 → 404. Backend satisfied; frontend flow broken."
    - id: "GOV-03"
      status: partial
      phase: 18
      claimed_by_plans: ["18-PLAN.md"]
      completed_by_plans: ["18-SUMMARY.md"]
      verification_status: passed
      evidence: "Backend audit trail exists (/v1/audit, /v1/audit/export registered main.rs:142-143). Frontend AuditLogViewer.tsx:27-38 uses HARDCODED MOCK events (const mockEvents = [...]) with comment 'would fetch from /v1/audit in production'; export button logs 'not yet implemented'. Backend has zero frontend consumers. Not mounted."
    - id: "GOV-04"
      status: partial
      phase: 18
      claimed_by_plans: ["18-PLAN.md"]
      completed_by_plans: ["18-SUMMARY.md"]
      verification_status: passed
      evidence: "Backend version control exists (/v1/branches CRUD + /merge, /v1/rules/{id}/git-log registered main.rs:134-140). Frontend BranchManager.tsx: fetch/merge/create are COMMENTED-OUT stubs (lines 8-10 useEffect commented, lines 70-83 'API call would go here' empty bodies, line 113-116 create only calls onClose). Backend has zero frontend consumers. Not mounted."
    - id: "MON-01"
      status: satisfied
      phase: 19
      claimed_by_plans: ["19-PLAN.md"]
      completed_by_plans: ["19-01-SUMMARY.md"]
      verification_status: passed
      evidence: "Prometheus /metrics endpoint registered (configure_metrics, main.rs:153), accept-header routing works (api/src/metrics.rs). Backend-internal; the Prometheus scraper is the consumer. Integration test metrics_prometheus_text_format passes."
    - id: "MON-02"
      status: partial
      phase: 19
      claimed_by_plans: ["19-PLAN.md"]
      completed_by_plans: ["19-02-SUMMARY.md"]
      verification_status: passed
      evidence: "Backend dashboard API exists (/v1/monitoring/engine-state registered, monitoring.rs:62-68). Frontend useEngineMetrics.ts:111,142 fetches /api/v1/metrics (NONEXISTENT — stale issue #3 still present). WebSocket /metrics/ws is NOT registered in main.rs (configure_metrics_ws defined at websocket.rs:30-33 but never called; handler is a stub). Dashboard component not in routeTree (_layout.gen.ts only registers / and /rules). Backend satisfied; frontend flow broken."
    - id: "MON-03"
      status: partial
      phase: 19
      claimed_by_plans: ["19-PLAN.md"]
      completed_by_plans: ["19-03-SUMMARY.md"]
      verification_status: passed
      evidence: "Backend endpoint /metrics/dead-rules reads RuleExecutionTracker (structurally closed, no hardcoded [] — api/src/metrics.rs:212). Frontend DeadRuleReportDashboard exists but is ORPHANED (not in any route/consumer). Data is empty because execute_rules is a stub (accepted tech debt). Backend structurally satisfied; frontend unwired; functional data deferred."
    - id: "MON-04"
      status: partial
      phase: 19
      claimed_by_plans: ["19-PLAN.md"]
      completed_by_plans: ["19-04-SUMMARY.md"]
      verification_status: passed
      evidence: "Backend endpoint /metrics/coverage reads CoverageTracking singleton (functionally proven by coverage_endpoint_reads_tracked_data integration test — api/src/metrics.rs:231). Frontend RuleCoverageDashboard exists but is ORPHANED. Data empty unless pre-seeded (execute_rules stub — accepted tech debt). Backend functionally satisfied; frontend unwired."
    - id: "API-04"
      status: satisfied
      phase: 20
      claimed_by_plans: ["20-PLAN.md"]
      completed_by_plans: ["20-SUMMARY.md"]
      verification_status: missing
      evidence: "All /v1/* management routes registered in main.rs (rules/categories/tags/approvals/audit/branches CRUD). 32 OpenAPI paths, 53 schemas (commit 5e383baf7); 12 api integration tests pass. Requirement is backend (Complete REST API) — satisfied. NOTE: no formal VERIFICATION.md for Phase 20 (verification debt); several routes (audit, branches) have no frontend consumers but the requirement is the backend API."
    - id: "DEPLOY-03"
      status: satisfied
      phase: 21
      claimed_by_plans: ["21-PLAN.md"]
      completed_by_plans: ["21-01-SUMMARY.md", "21-UAT.md (4/4 passed)"]
      verification_status: missing
      evidence: "Helm chart + values-dev/staging/prod + secret template exist (per 21-01-SUMMARY). UAT 4/4 passed. No formal VERIFICATION.md (UAT used instead — verification debt)."
    - id: "DEPLOY-04"
      status: satisfied
      phase: 21
      claimed_by_plans: ["21-PLAN.md"]
      completed_by_plans: ["21-01-SUMMARY.md", "21-UAT.md (4/4 passed)"]
      verification_status: missing
      evidence: "/v1/rules/reload + /v1/jobs + /v1/jobs/{job_id} registered (main.rs:103,111-112). Hot reload mechanism verified. No formal VERIFICATION.md (UAT used)."
  integration:
    - id: "integration-baseurl-prefix"
      status: broken
      from: frontend/src/api/client.ts:6
      to: backend /v1 scope (main.rs:96)
      issue: "Systematic: frontend baseURL is /api/v1 (client.ts:6, .env.development, .env.production) but backend serves /v1 with NO /api scope. Every axios request → 404 (falls through to fs::Files). Vite proxy has no rewrite. Breaks all frontend→backend flows."
      affected_reqs: ["FRONTEND-01","FRONTEND-02","GOV-02","GOV-03","GOV-04","MON-02"]
    - id: "integration-authprovider"
      status: broken
      from: frontend/src/main.tsx:19-26
      to: frontend/src/lib/auth-context.tsx:152-154
      issue: "AuthProvider never mounted; useAuth() throws 'must be used within AuthProvider'. usePermission (used by List.tsx:22 and governance hooks) crashes on render."
      affected_reqs: ["FRONTEND-02","GOV-01"]
    - id: "integration-router-incomplete"
      status: broken
      from: frontend/src/routes/_layout.gen.ts:6-20
      to: frontend route components
      issue: "routeTree only registers / and /rules. monitoring.tsx, governance, versioning, rule-editor routes never imported. ~10 components orphaned."
      affected_reqs: ["FRONTEND-01","MON-02","GOV-02","GOV-03","GOV-04"]
    - id: "integration-metrics-ws"
      status: broken
      from: frontend useEngineMetrics.ts:26
      to: backend api/src/websocket.rs:30-33
      issue: "configure_metrics_ws defined but NEVER called in main.rs; WS handler is a stub. Frontend defaults to ws://localhost:8080/metrics/ws → fails."
      affected_reqs: ["MON-02"]
    - id: "integration-metrics-fetch-path"
      status: broken
      from: frontend useEngineMetrics.ts:111,142
      to: backend /metrics + /v1/monitoring/engine-state
      issue: "Frontend fetches /api/v1/metrics (nonexistent). Backend serves root /metrics and /v1/monitoring/engine-state. Stale 2026-06-11 issue #3 still present."
      affected_reqs: ["MON-02"]
    - id: "integration-audit-frontend"
      status: broken
      from: frontend AuditLogViewer.tsx:27-38
      to: backend /v1/audit, /v1/audit/export
      issue: "Frontend uses hardcoded mock events; never calls backend; export not implemented. Not mounted."
      affected_reqs: ["GOV-03"]
    - id: "integration-branch-frontend"
      status: broken
      from: frontend BranchManager.tsx:8-10,70-83,113-116
      to: backend /v1/branches, /merge
      issue: "Frontend fetch/create/merge are commented-out stubs. Not mounted."
      affected_reqs: ["GOV-04"]
    - id: "integration-auth-middleware"
      status: broken
      from: api/src/middleware/auth.rs:6
      to: (never wrapped)
      issue: "requires_auth defined but never wrap()'ed in main.rs. No /v1/* route is protected. RBAC not enforced at any boundary."
      affected_reqs: ["GOV-01"]
    - id: "integration-frontend-rules-unregistered"
      status: broken
      from: frontend/src/api/rules.ts:48,60
      to: backend (no such routes)
      issue: "Frontend calls PATCH /rules/{id}/status and GET /rules/search — routes NOT registered in main.rs. Latent (functions unused by rendered components)."
      affected_reqs: ["FRONTEND-02"]
    - id: "integration-approvals-contract"
      status: wired
      from: frontend ApprovalQueue.tsx:17
      to: backend approvals.rs:153-155
      issue: "Contract matches (response.data.approvals). Path-level wiring correct (/approvals/pending). Broken at runtime only by global baseURL mismatch + unmounted component."
      affected_reqs: ["GOV-02"]
    - id: "integration-metrics-tracker"
      status: wired
      from: api/src/metrics.rs:212,231
      to: engine monitoring trackers
      issue: "Backend internal: dead-rules reads RuleExecutionTracker, coverage reads CoverageTracking singleton. Genuinely wired (Phase 19 gap closure). Data emptiness is accepted tech debt (execute_rules stub)."
      affected_reqs: ["MON-03","MON-04"]
    - id: "integration-monitoring-routes"
      status: wired
      from: api/src/v1/routes/monitoring.rs:62-68
      to: main.rs:149 configure_monitoring
      issue: "/v1/monitoring/engine-state, /v1/monitoring/health registered and reachable. No frontend consumer (frontend polls wrong path)."
      affected_reqs: ["MON-02"]
  flows:
    - name: "Rules CRUD"
      status: broken
      breaks_at: "useRules → getRules → /api/v1/rules (404) + /rules route crashes (no AuthProvider) + create/update UI unreachable (RuleEditor orphaned)"
      affected_reqs: ["FRONTEND-01","FRONTEND-02"]
    - name: "Approval workflow"
      status: broken
      breaks_at: "ApprovalQueue contract matches but not mounted + baseURL → 404"
      affected_reqs: ["GOV-02"]
    - name: "Audit trail query"
      status: broken
      breaks_at: "AuditLogViewer uses mock data; never calls /v1/audit; not mounted"
      affected_reqs: ["GOV-03"]
    - name: "Branch create/merge"
      status: broken
      breaks_at: "BranchManager fetch/merge/create commented-out stubs; not mounted"
      affected_reqs: ["GOV-04"]
    - name: "Metrics dashboard"
      status: broken
      breaks_at: "useEngineMetrics fetches nonexistent /api/v1/metrics; WS not registered; Dashboard not in router"
      affected_reqs: ["MON-02"]
    - name: "Rule authoring"
      status: broken
      breaks_at: "RuleEditor/DecisionTable/HitPolicySelector orphaned — never rendered; no route; no save flow"
      affected_reqs: ["FRONTEND-01"]
    - name: "Dead-rule / coverage reporting"
      status: partial
      breaks_at: "Backend endpoints read real structures; frontend dashboards orphaned; data empty (execution stub)"
      affected_reqs: ["MON-03","MON-04"]
    - name: "Rule execution"
      status: tech_debt
      breaks_at: "execute_rules stub (api/src/v1/execution.rs:312) — accepted out-of-scope deferral"
      affected_reqs: []
    - name: "Hot reload"
      status: complete
      breaks_at: "N/A — /v1/rules/reload + /v1/jobs registered (main.rs:103,111-112)"
      affected_reqs: ["DEPLOY-04"]
    - name: "Deployment configuration"
      status: complete
      breaks_at: "N/A — Helm/K8s manifests complete (Phase 21)"
      affected_reqs: ["DEPLOY-03"]
tech_debt:
  - phase: 19
    items:
      - "execute_rules is a stub (api/src/v1/execution.rs:312): builds engine into let _engine (never driven), rules_fired hardcoded 0. Downstream: metrics/dead-rules/coverage not auto-populated during execution; dead_rules_endpoint legitimately returns [] (fresh tracker per request). Making execution functional is a separate future milestone, out of scope for v4.0."
      - "WebSocket /metrics/ws: configure_metrics_ws defined (websocket.rs:30-33) but never registered in main.rs; handler is a stub. Real-time streaming not implemented."
  - phase: 20
    items:
      - "No formal VERIFICATION.md for Phase 20 (verification debt); only 20-SUMMARY.md status:complete. Backend is functionally complete (32 OpenAPI paths, integration tests pass)."
  - phase: 21
    items:
      - "No formal VERIFICATION.md for Phase 21 (verification debt); 21-UAT.md 4/4 passed used instead."
  - phase: 18
    items:
      - "RBAC domain model exists but requires_auth middleware (api/src/middleware/auth.rs:6) is never wrapped in main.rs — no /v1/* route is protected. (Also a GOV-01 integration gap.)"
  - phase: 17
    items:
      - "frontend/src/api/rules.ts:48,60 call unregistered backend routes (PATCH /rules/{id}/status, GET /rules/search) — latent (functions unused by rendered components)."
  - phase: api-layer
    items:
      - "configure_v1_metrics is dead code (never registered). configure_metrics_ws never called."
---

# Milestone v4.0 — Production Platform: Audit Report

**Date (regenerated):** 2026-07-01
**Previous audit:** 2026-06-11 (stale — predates execution of phases 18/19/20/21; this report replaces it)
**Auditor:** Claude (gsd-audit-milestone) + gsd-integration-checker subagent + manual 3-source cross-reference
**Status:** **GAPS_FOUND**

## Executive Summary

| Metric | Score |
|--------|-------|
| Requirements fully satisfied | 4/13 (31%) |
| Phases complete (backend) | 4/5 |
| Integration points working end-to-end | 3/12 |
| E2E flows complete | 2/10 |

The 2026-06-11 audit flagged gaps because phases 18/19/20 were "not yet executed" (no VERIFICATION.md). Those phases **have since been executed and are substantially complete at the backend level** — Phase 19 was re-verified to `passed` today (B1, commit `b951b41ac`), build/clippy/test are green (A7 gate), and the API has a full OpenAPI spec (commit `5e383baf7`) with 232 passing tests.

**However**, this regenerated audit surfaces a different, more accurate blocker that the prior audit (and STATE.md) missed: **Phase 17 (frontend) is a scaffolding skeleton.** The React components exist but are largely **orphaned** — never imported into the router or wired to the backend. A single systematic root cause — the frontend API client's `baseURL` of `/api/v1` vs the backend's `/v1` scope (no `/api` anywhere) — breaks every axios flow at runtime (404 → falls through to static files). This is compounded by an incomplete router (only `/` and `/rules` registered), an unmounted `AuthProvider` (so anything using permissions crashes on render), and three governance UIs that are stub/mock-only and never rendered.

**Net effect:** the backend delivers 4/13 requirements fully (MON-01, API-04, DEPLOY-03, DEPLOY-04). The other 9 (all frontend-touching: FRONTEND-01/02, GOV-01..04, MON-02/03/04) are only *partial* — their backend half is implemented but the frontend half is broken/unwired, so they cannot be verified end-to-end.

## Requirements Status

### Satisfied (4/13)

| Requirement | Phase | Evidence |
|-------------|-------|----------|
| MON-01: Prometheus Metrics Integration | 19 | `/metrics` endpoint registered, accept-header routing works; integration test passes |
| API-04: Complete Management API | 20 | All `/v1/*` CRUD routes registered; 32 OpenAPI paths, 12 integration tests pass |
| DEPLOY-03: Docker/Kubernetes Support | 21 | Helm chart + env values + secret template; UAT 4/4 passed |
| DEPLOY-04: Hot Reload Support | 21 | `/v1/rules/reload` + `/v1/jobs` registered; UAT 4/4 passed |

### Partial (9/13) — backend implemented, frontend broken/unwired

| Requirement | Phase | Why partial |
|-------------|-------|-------------|
| FRONTEND-01 | 17 | RuleEditor/DecisionTable orphaned (no route, no save flow); baseURL → 404 |
| FRONTEND-02 | 17 | useRules code-wired but runtime 404; `/rules` route crashes (no AuthProvider); CRUD UI unreachable |
| GOV-01 | 18 | RBAC model exists but NOT enforced: `requires_auth` never wrapped; AuthProvider unmounted |
| GOV-02 | 18 | Backend approval workflow + endpoints exist; ApprovalQueue contract matches but not mounted + 404 |
| GOV-03 | 18 | Backend audit trail exists; AuditLogViewer uses hardcoded mock, never calls `/v1/audit` |
| GOV-04 | 18 | Backend version control exists; BranchManager fetch/merge/create are commented-out stubs |
| MON-02 | 19 | Backend dashboard API exists; frontend fetches nonexistent `/api/v1/metrics`, WS not registered, Dashboard not in router |
| MON-03 | 19 | Backend endpoint reads tracker (structurally closed); frontend dashboard orphaned; data empty (execution stub) |
| MON-04 | 19 | Backend endpoint reads CoverageTracking (functionally proven); frontend dashboard orphaned; data empty |

### Unsatisfied (0/13)

No requirement is entirely unimplemented. Every requirement has at least backend (or infra) implementation; the gap is the frontend half.

## Integration Gaps

### Critical (systematic)

1. **API path-prefix mismatch** — frontend `baseURL: /api/v1` (`frontend/src/api/client.ts:6`, `.env.development`, `.env.production`) vs backend `web::scope("/v1")` (`api/src/main.rs:96`). No `/api` scope exists; vite proxy has no rewrite. Every axios request → 404. **Affects FRONTEND-01/02, GOV-02/03/04, MON-02.**
2. **AuthProvider never mounted** — `frontend/src/main.tsx:19-26` renders `RouterProvider` with no `AuthProvider`; `useAuth()` throws (`auth-context.tsx:152-154`); `usePermission` (used by `List.tsx:22` + governance hooks) crashes on render. **Affects FRONTEND-02, GOV-01.**
3. **Incomplete router** — `_layout.gen.ts:6-20` routeTree only registers `/` and `/rules`. ~10 components (monitoring Dashboard, ApprovalQueue, AuditLogViewer, BranchManager, RuleEditor, DecisionTable, HitPolicySelector, DeadRuleReport/RuleCoverage dashboards) are never imported → dead UI. **Affects FRONTEND-01, MON-02, GOV-02/03/04.**
4. **Metrics frontend path** — `useEngineMetrics.ts:111,142` fetches `/api/v1/metrics` (nonexistent); backend serves root `/metrics` + `/v1/monitoring/engine-state`. Stale 2026-06-11 issue #3 **still present.**
5. **WebSocket not registered** — `configure_metrics_ws` (`websocket.rs:30-33`) defined but never called in `main.rs`; handler is a stub. **Affects MON-02.**
6. **No auth enforcement** — `requires_auth` (`api/src/middleware/auth.rs:6`) never `wrap()`ed in `main.rs`. No `/v1/*` route is protected. **Affects GOV-01.**

### Stub/mock frontend components

7. **AuditLogViewer** (`AuditLogViewer.tsx:27-38`) — hardcoded mock events, never calls `/v1/audit`; export "not implemented".
8. **BranchManager** (`BranchManager.tsx:8-10,70-83,113-116`) — fetch/merge/create are commented-out stubs.

### Wired (backend-internal)

- Backend metrics → trackers (`api/src/metrics.rs:212,231`) — Phase 19 gap closure, genuine.
- Monitoring routes registered (`monitoring.rs:62-68`).
- Approvals contract matches (`approvals.rs:153-155` ↔ `ApprovalQueue.tsx:17`) — broken only by baseURL + unmounted.

## E2E Flows Status

| Flow | Status | Breaks At | REQ |
|------|--------|-----------|-----|
| Rules CRUD | BROKEN | 404 (baseURL) + route crash (AuthProvider) + orphaned editor | FRONTEND-01/02 |
| Approval workflow | BROKEN | not mounted + 404 | GOV-02 |
| Audit trail query | BROKEN | mock-only, not mounted | GOV-03 |
| Branch create/merge | BROKEN | stubbed, not mounted | GOV-04 |
| Metrics dashboard | BROKEN | wrong fetch path, WS unregistered, not in router | MON-02 |
| Rule authoring | BROKEN | editor orphaned, no route, no save | FRONTEND-01 |
| Dead-rule / coverage reporting | PARTIAL | backend wired, frontend orphaned, data empty | MON-03/04 |
| Rule execution | TECH DEBT | execute_rules stub (accepted) | — |
| Hot reload | COMPLETE | — | DEPLOY-04 |
| Deployment configuration | COMPLETE | — | DEPLOY-03 |

## Tech Debt (non-blocking, deferred)

- **execute_rules stub** (`api/src/v1/execution.rs:312`) — accepted out-of-scope deferral; downstream metrics/dead-rules/coverage not auto-populated.
- **WebSocket streaming** — `configure_metrics_ws` never registered; handler stub.
- **Verification debt** — no formal `VERIFICATION.md` for phases 20/21 (20-SUMMARY `status:complete`; 21-UAT 4/4 used instead). Phase 17/18 VERIFICATION.md are plan-level ("APPROVED FOR EXECUTION"), not execution-level.
- **Auth middleware** — `requires_auth` never wrapped (also a GOV-01 gap).
- **Unregistered frontend route calls** — `frontend/src/api/rules.ts:48,60` target nonexistent backend routes (latent).

## Nyquist Compliance

Not applicable — no `*-VALIDATION.md` files present in v4.0 phase directories (Nyquist capability not active for this milestone).

## Verification Files

| Phase | File | Status |
|-------|------|--------|
| 17 | 17-VERIFICATION.md | PASS — plan-level ("APPROVED FOR EXECUTION"); execution incomplete per integration check |
| 18 | VERIFICATION.md | PASS — plan-level ("APPROVED FOR EXECUTION"); backend executed per STATE/SUMMARY |
| 19 | VERIFICATION.md | passed — execution-level (re-verified 2026-07-01, B1, commit b951b41ac) |
| 20 | (none) | Missing — 20-SUMMARY.md `status:complete` used (verification debt) |
| 21 | (none, 21-UAT.md) | Missing VERIFICATION.md — UAT 4/4 passed (verification debt) |

## Honest Assessment vs STATE.md

STATE.md claims "Phase 17 COMPLETE — All plans implemented (validation display integration pending)". The integration check contradicts this: Phase 17's components are implemented as code but are **not wired into the application** (orphaned from router and backend). The "COMPLETE" status reflects plans/SUMMARYs existing, not end-to-end functionality. **The milestone is NOT production-ready** despite the backend being substantially complete.

## Recommendation

The v4.0 milestone should **NOT be closed as-is.** The frontend integration gap is the defining blocker. Options:

1. **Close the gaps inline** (recommended) — insert a closure phase to wire the frontend: fix the baseURL (`/api/v1` → `/v1` or add a backend `/api` scope + vite rewrite), mount `AuthProvider`, register the orphaned routes, wire the stub/mock governance UIs to their backend endpoints, register `configure_metrics_ws`. This is the work that actually delivers FRONTEND-01/02, GOV-02/03/04, MON-02 end-to-end.
2. **Re-scope v4.0** — formally defer the frontend to a v5.0 milestone (accept that v4.0 delivers the backend platform only) and complete v4.0 with the frontend explicitly out of scope.
3. **Accept as tech debt and complete** — NOT recommended; 9/13 requirements partial is not "tech debt," it's a blocker.

---

*Audit regenerated: 2026-07-01*
*Integration check: gsd-integration-checker subagent (cross-phase wiring + E2E flows)*
*Requirements cross-reference: 3-source (REQUIREMENTS.md traceability + phase VERIFICATION.md + SUMMARY.md frontmatter)*
*Phase 19 re-verification: commit b951b41ac (B1)*
*Hardening gate (A7): cargo clippy/test/check --workspace all green; 232 tests pass*