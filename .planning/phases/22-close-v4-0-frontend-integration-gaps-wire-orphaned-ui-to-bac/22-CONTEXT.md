# Phase 22: Close v4.0 frontend integration gaps - wire orphaned UI to backend - Context

**Gathered:** 2026-07-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Close the v4.0 frontend↔backend integration gaps so the 9 partial v4.0 requirements deliver end-to-end: **FRONTEND-01, FRONTEND-02, GOV-01, GOV-02, GOV-03, GOV-04, MON-02, MON-03, MON-04.**

This is a **wiring/closure phase**, not new-feature building. The backend (phases 18–21) is substantially complete; the React frontend (Phase 17) is a scaffolding skeleton — components exist but are orphaned from the router and backend. This phase wires them together. Root-cause cluster and file:line evidence live in `.planning/v4-MILESTONE-AUDIT.md` and `.planning/.continue-here.md`.

**Authoritative gap source (read first):** `.planning/v4-MILESTONE-AUDIT.md` (8 integration gaps, 10 E2E flows, requirements cross-reference) and `.planning/.continue-here.md` (gap list + file:line evidence).

**Phase is large and MUST be broken into multiple plans** (planner: recommend one plan per gap cluster — API path, auth, router, governance UI, metrics WS, dead-rules/coverage dashboards). The user deliberately chose the thorough/complete option at every fork; this is "close inline" done properly, not a stub.

</domain>

<decisions>
## Implementation Decisions

### Auth depth & enforcement (GOV-01)
- **D-22-01:** Phase 22 implements **pragmatic minimal auth**: in-memory user store seeded with the 4 predefined roles (admin, author, reviewer, viewer) and a few demo users; `/v1/auth/login`, `/v1/auth/me` (and refresh) endpoints; signed JWTs using the existing `API_SECRET_KEY` (HMAC); `requires_auth` upgraded from its current presence-only stub (`api/src/middleware/auth.rs:6` only checks `Bearer ` prefix) to actually validate the JWT and resolve the role; wrap it on mutating `/v1/*` routes with a public allowlist for login, `/health`, `/swagger-ui`, `/api-docs`. **Backend currently has ZERO auth endpoints** — they must be built (STATE.md's "P1 done" claim is false; verify during research). Passwords may be plaintext or lightly hashed for the demo; production hashing is deferred.
- **D-22-02 (enforcement granularity):** Backend enforces "authenticated vs public" for v4.0 (public allowlist + authenticated on protected routes). Finer per-route role enforcement (viewer read-only, author cannot approve, admin full) is deferred — the frontend `usePermission` already gates UI by role; full backend role gating lands in v5.0.
- **D-22-03 (split to v5.0):** The user's original preference was full production auth + Postgres. After a scope check, the user chose to **split**: full production-grade auth (Postgres user store per D-18-01, argon2/bcrypt password hashing, refresh-token rotation, per-route role enforcement) AND migration of all v4.0 in-memory stores (rules/approvals/audit/branches) to Postgres → **new milestone v5.0: Persistence & Identity**. **Action for planner/orchestrator: flag v5.0 milestone creation.** Phase 22 does NOT introduce Postgres and does NOT migrate existing stores.

### API path strategy (the foundation — do first)
- **D-22-04:** **Align frontend to `/v1`** — drop `/api` everywhere. `frontend/src/api/client.ts:6` baseURL → `/v1` (drop `/api/v1`); `frontend/src/lib/auth-context.tsx` fetch paths `/api/v1/auth/*` → `/v1/auth/*`; `.env.development` and `.env.production` `VITE_API_URL` → `/v1`-based. Retarget the vite proxy (`frontend/vite.config.ts`) from `/api` to `/v1` → `http://localhost:8080` (or add a `/v1` proxy). No backend route prefix change (backend stays `web::scope("/v1")`, `api/src/main.rs:96`).
- **D-22-05 (URL mode):** Keep `VITE_API_URL` **configurable absolute** (default points at backend `/v1`; prod can override to a separate API host like `https://api.rrules.example.com/v1`). Enables splitting SPA and API across hosts in prod; relies on the CORS already added by P2 hardening (actix-cors). Dev uses the vite proxy. The current `client.ts` uses absolute URLs (bypasses vite proxy, cross-origin) while `auth-context.tsx` uses relative `fetch` — make these consistent (both via the configured client / `VITE_API_URL`).

### Router strategy
- **D-22-06:** **Proper TanStack file-based routing with codegen** — add `@tanstack/router-plugin` to vite, make route files under `frontend/src/routes/` the source of truth, auto-generate the route tree so it cannot drift again. The current `frontend/src/routes/_layout.gen.ts` is hand-maintained and stale (only `/` and `/rules` registered despite `monitoring.tsx` existing). Replace the hand-maintained gen file with generated output.
- **D-22-07 (mount all orphaned components):** Register routes for every orphaned component: `/monitoring` (+ dashboard sub-routes for DeadRuleReport, RuleCoverage, EngineMetrics), `/governance/approvals`, `/governance/audit`, `/governance/branches`, `/governance/rbac`, and rule authoring routes (D-22-08). Mirror the existing `frontend/src/features/{governance,versioning,monitoring,rules}` structure.
- **D-22-08 (rule authoring access):** **Dedicated edit routes** — `/rules/new` (create) and `/rules/:id/edit` (edit), rendering `RuleEditor` with `DecisionTable` and `HitPolicySelector` as sub-components/tabs. Deep-linkable, back/forward works. Closes FRONTEND-01's "no route, no save flow" gap. Not a modal/drawer.
- **D-22-09 (AuthProvider mount):** Mount `AuthProvider` in `frontend/src/main.tsx:19-26` so `useAuth()` / `usePermission()` stop throwing on render (currently crashes `/rules` and governance hooks).

### Real-time metrics (MON-02)
- **D-22-10:** **Implement real WebSocket streaming now** — register `configure_metrics_ws` in `api/src/main.rs` (currently defined at `api/src/websocket.rs:30-33` but never called; handler is a stub), flesh out the handler to broadcast from the existing `metrics_tx` broadcast channel (`api/src/main.rs:30`), and wire `useEngineMetrics.ts` WS path. Fix the frontend fetch path (`useEngineMetrics.ts:111,142` fetches nonexistent `/api/v1/metrics` → point at `/v1/monitoring/engine-state` + WS). Mount the dashboard in the router. Honors D-19-02 (WebSocket streaming decision). Data is empty while `execute_rules` is a stub (accepted tech debt) — the stream legitimately pushes empty state until a future execution milestone.

### Governance UI depth (GOV-02, GOV-03, GOV-04, FRONTEND-01)
- **D-22-11:** **Full functional wiring + conflict resolution + side-by-side diff.** AuditLogViewer (`frontend/src/features/governance/audit/AuditLogViewer.tsx`) calls `/v1/audit` for list and `/v1/audit/export` for CSV/JSON export (closes GOV-03 export — currently hardcoded mock at lines 27-38). BranchManager (`frontend/src/features/versioning/BranchManager.tsx`) calls `/v1/branches` CRUD and `/v1/branches/{name}/merge` (closes GOV-04 basic merge — currently commented stubs at lines 8-10, 70-83, 113-116), AND implements the interactive conflict-resolution UI + side-by-side diff view (GOV-04 "merge with conflict resolution" + FRONTEND-01 "side-by-side version comparison" — both explicitly required by acceptance criteria, flagged partial by audit). ApprovalQueue mounted and calls `/approvals/*` (closes GOV-02 — contract already matches `approvals.rs:153-155`, only unmounted + 404). Real fetch, loading, error, and empty states throughout.

### Branch
- **D-22-12:** Phase 22 gets its own git branch `gsd/phase-22-frontend-wiring` off the current `gsd/phase-19-monitoring-metrics` (or `main`). **Pending user confirmation at plan time** (user accepted the recommendation by silence; flag in plan). The prior monitoring branch stays untouched.

### Claude's Discretion
- JWT token lifetimes and exact demo-user credentials/roles for the in-memory store (planner picks sensible defaults).
- Exact vite proxy retarget mechanics (new `/v1` proxy entry vs retarget `/api` with rewrite) — achieve "frontend targets `/v1`" any clean way.
- Route file layout/naming under TanStack file-based conventions.
- Polling interval and WS reconnect/backoff specifics in `useEngineMetrics`.
- Whether the metrics WS endpoint lives at `/v1/monitoring/ws` or `/metrics/ws` (audit notes both `configure_metrics_ws` and stale frontend default `ws://localhost:8080/metrics/ws`); pick one and align both sides.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Audit / gap source (PRIMARY — read first)
- `.planning/v4-MILESTONE-AUDIT.md` — the 8 integration gaps, 10 E2E flows, requirements cross-reference, tech-debt list. Authoritative evidence.
- `.planning/.continue-here.md` — gap list with file:line evidence + next actions.
- `.planning/HANDOFF.json` — structured handoff (A1–A7 hardening done, B1–B2 audit done, C1 frontend phase next).

### Project / requirements
- `.planning/PROJECT.md` — Core value, constraints (Rust backend, React/TS frontend, hybrid Git+DB)
- `.planning/REQUIREMENTS.md` §FRONTEND-01/02, §GOV-01–04, §MON-01–04, §API-04, §DEPLOY-03/04 — acceptance criteria for every requirement this phase closes
- `.planning/ROADMAP.md` §Phase 22 — phase goal (currently TBD; planner fills from this CONTEXT)

### Prior phase context (decisions carried forward)
- `.planning/phases/18-enterprise-governance/18-CONTEXT.md` — D-18-01 (RBAC in Postgres, deferred to v5.0), D-18-02 (webhooks), D-18-03 (audit event stream), D-18-04 (direct Git integration)
- `.planning/phases/19-monitoring-metrics/19-CONTEXT.md` — D-19-01 (Prometheus+JSON), D-19-02 (WebSocket dashboard streaming — honored by D-22-10), D-19-03 (dead-rule threshold), D-19-04 (full trace coverage)
- `.planning/phases/21-deployment-ci-cd/21-CONTEXT.md` — deployment shape (single container serves SPA via `fs::Files` at root — relevant to same-origin vs separate-host URL decision D-22-05)

### Code (the files to wire — scout-confirmed)
- `api/src/main.rs` — route registration; `web::scope("/v1")` at line 96 (no `/api`); `configure_metrics_ws` NOT called; `requires_auth` NOT wrapped; `metrics_tx` channel at line 30; SPA fallback `fs::Files::new("/")` at line 156
- `api/src/middleware/auth.rs:6` — `requires_auth` presence-only stub to upgrade
- `frontend/src/api/client.ts:6` — baseURL `/api/v1` (absolute) to fix
- `frontend/src/lib/auth-context.tsx` — `AuthProvider` to mount; fetch paths `/api/v1/auth/*` to fix; calls login/me endpoints that don't exist yet (build them)
- `frontend/src/main.tsx:19-26` — mount `AuthProvider`
- `frontend/src/routes/_layout.gen.ts` — stale hand-maintained route tree (only `/`, `/rules`); replace with codegen
- `frontend/src/routes/monitoring.tsx` — exists but orphaned
- `frontend/src/hooks/useEngineMetrics.ts:111,142` — wrong fetch path; WS path to wire
- `frontend/src/features/governance/audit/AuditLogViewer.tsx:27-38` — mock events to replace
- `frontend/src/features/versioning/BranchManager.tsx` — commented stubs to implement
- `frontend/vite.config.ts` — proxy `/api` → backend, no rewrite; retarget to `/v1`
- `frontend/.env.development`, `frontend/.env.production` — `VITE_API_URL` `/api/v1` → `/v1`-based

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `frontend/src/hooks/useEngineMetrics.ts` — already has WS + HTTP polling lifecycle; needs path fixes, not a rewrite
- `frontend/src/features/monitoring/dashboards/` — `EngineMetricsDashboard.tsx`, `DeadRuleReportDashboard.tsx`, `RuleCoverageDashboard.tsx` exist; orphaned, need mounting + endpoint wiring
- `frontend/src/features/governance/` (approval/audit/rbac) + `frontend/src/features/versioning/` — components exist; need mounting + real fetch wiring
- `frontend/src/lib/auth-context.tsx` + `frontend/src/hooks/use-permission.ts` — auth/permission machinery exists; needs `AuthProvider` mounted + backend endpoints to back it
- Backend `/v1/monitoring/engine-state`, `/v1/audit`, `/v1/audit/export`, `/v1/branches` (+`/merge`), `/v1/approvals/*` — all registered and reachable; the frontend just never calls them correctly
- `metrics_tx` broadcast channel (`api/src/main.rs:30`) — already created "for future WebSocket use"; D-22-10 consumes it

### Established Patterns
- **In-memory stores via `init_store()`** — the whole v4.0 backend uses in-memory stores (rules, categories, tags, approvals, audit, jobs). The pragmatic auth store (D-22-01) follows this pattern; Postgres is deliberately NOT introduced (deferred to v5.0).
- **Actix Web routing** — `/v1` scope, async handlers; `configure_*` functions for route groups (e.g. `configure_monitoring`, `configure_metrics`).
- **Hybrid Git + DB storage** (PROJECT.md) — Git for rule versioning (already wired via `VersionControlRepo` in `main.rs`), DB for runtime metadata (deferred).
- **TanStack Router + Query + Zustand** — frontend stack; router codegen (D-22-06) is the intended TanStack pattern.

### Integration Points
- `main.tsx` → mount `AuthProvider` above `RouterProvider`
- `_layout.gen.ts` → replaced by codegen output from route files
- `client.ts` + `auth-context.tsx` → `/v1` target via `VITE_API_URL`
- `api/src/main.rs` → register `configure_metrics_ws`; wrap `requires_auth`; add `/v1/auth/*` routes
- `api/src/middleware/auth.rs` → upgrade `requires_auth` to validate JWT + resolve role

</code_context>

<specifics>
## Specific Ideas

- The user consistently chose the **thorough/complete** option at every decision fork (full auth → then split; codegen router; WS now; full governance + conflict resolution). Intent: v4.0 closes for real, not as a stub. The planner should treat "fully close each requirement's acceptance criteria" as the bar, and break this large phase into multiple plans (one per gap cluster) rather than collapse it.
- Pragmatic auth should seed recognizable demo users for each predefined role so RBAC can be demonstrated end-to-end (admin/author/reviewer/viewer). Planner picks exact credentials.
- `API_SECRET_KEY` already exists as a Helm secret placeholder (Phase 21) — reuse it for JWT signing.

</specifics>

<deferred>
## Deferred Ideas

### To v5.0: Persistence & Identity (NEW MILESTONE — flag for creation)
- Full production auth: Postgres user/role/permission store (D-18-01), argon2/bcrypt password hashing, refresh-token rotation, per-route role enforcement (D-22-02).
- Migrate all v4.0 in-memory stores (rules, categories, tags, approvals, audit, branches) to PostgreSQL (D-22-03 — user chose to split this out).
- Real-time RBAC sync via WebSocket subscriptions (from 18-CONTEXT deferred).
- Multi-tenant governance isolation (from 18-CONTEXT deferred).

### Accepted tech debt (NOT in scope — do not re-litigate)
- `execute_rules` is a stub (`api/src/v1/execution.rs:312`, `rules_fired: 0`) — separate future execution milestone. Downstream: metrics/dead-rules/coverage not auto-populated; `dead_rules_endpoint` returns `[]` (fresh tracker). Phase 22 wires dashboards to endpoints; endpoints legitimately return empty until execution is wired.
- Verification debt: phases 20/21 have no formal VERIFICATION.md (20-SUMMARY `status:complete`; 21-UAT 4/4). Optional to add during closure.
- `frontend/src/api/rules.ts:48,60` call unregistered backend routes (`PATCH /rules/{id}/status`, `GET /rules/search`) — latent; wire or remove during this phase if reached, else leave documented.

</deferred>

---

*Phase: 22-Close v4.0 frontend integration gaps*
*Context gathered: 2026-07-01*