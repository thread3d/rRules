---
phase: 22
slug: close-v4-0-frontend-integration-gaps-wire-orphaned-ui-to-bac
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-02
---

# Phase 22 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from `22-RESEARCH.md` § Validation Architecture. This is a wiring/closure phase:
> feedback latency is dominated by `cargo test` (backend) and `npm run build` / `tsc --noEmit` (frontend).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Backend framework** | Rust `cargo test` (unit + integration, actix-web test utils) — 232-test baseline must not regress |
| **Backend config** | `api/Cargo.toml` |
| **Frontend framework** | vitest **NOT confirmed** in `frontend/vite.config.ts` — Wave 0 gap; frontend gate is `tsc --noEmit` + `npm run build` until decided |
| **Frontend config** | `frontend/vite.config.ts` (no vitest config seen — Wave 0 must confirm/install or accept build-only) |
| **Quick run command** | `cargo test -p api --lib && (cd frontend && npx tsc --noEmit)` |
| **Full suite command** | `cargo test --workspace && cargo clippy --workspace --all-targets && (cd frontend && npm run build)` |
| **Estimated runtime** | ~120–180 seconds full (backend ~232 tests + frontend build) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p api --lib` (affected-module fast path) + `cd frontend && npx tsc --noEmit` (typecheck). Frontend has no confirmed vitest setup — typecheck + build is the fast gate.
- **After every plan wave:** Run the full suite command above. Backend 232-test baseline must stay green; `cargo clippy` must stay at 0 warnings; frontend `npm run build` must stay green.
- **Before `/gsd-verify-work`:** Full suite green, PLUS property-style checks (see Property Checks below).
- **Max feedback latency:** ~180 seconds (full suite). Per-task quick gate ~30–60 seconds.

---

## Per-Task Verification Map

> Tasks are not yet numbered (planner assigns IDs). This map is populated from the RESEARCH § Phase Requirements → Test Map; the planner must keep every task's `<verify>` aligned with a row here. Wave 0 = files that do not exist yet and must be created.

| Task ID | Plan (cluster) | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|-----------------|------|-------------|------------|------------------|-----------|-------------------|-------------|--------|
| 22-01-T2 | 01 API path alignment | 1 | FRONTEND-02, MON-02 | — | N/A | smoke/grep | `(cd frontend && npm run build) && ! grep -rn "/api/v1" frontend/src/api/client.ts frontend/src/lib/auth-context.tsx` | ❌ W0 (path edits) | ⬜ pending |
| 22-01-T3 | 01 API path alignment | 1 | FRONTEND-02, MON-02, MON-03, MON-04 | — | N/A | build + integration | `cargo build -p api && cargo test -p api --test metrics_integration && grep -n "configure_metrics_dashboards" api/src/main.rs` | ✅ existing (metrics.rs split) | ⬜ pending |
| 22-02-T1 | 02 Auth + JWT | 2 | GOV-01 | — | `/v1/auth/login` returns signed JWT; `/v1/auth/me` resolves user | integration | `cargo test -p api --test auth_endpoints` | ❌ W0 | ⬜ pending |
| 22-02-T2 | 02 Auth + JWT | 2 | GOV-01 | T-22-01 (alg-confusion), T-22-02 (missing auth) | `requires_auth` rejects missing/invalid/wrong-secret/alg:none JWT; accepts valid HS256; protected `/v1/*` returns 401 without token | integration | `cargo test -p api --test auth_protection` | ❌ W0 | ⬜ pending |
| 22-03-T2 | 03 Router codegen + mounts | 2 | FRONTEND-01 | — | N/A | smoke (build) | `(cd frontend && npx tsc --noEmit && npm run build) && grep -n "tanstackRouter\|AuthProvider\|routeTree.gen" frontend/src/main.tsx frontend/vite.config.ts` | ❌ W0 (codegen + route files) | ⬜ pending |
| 22-03-T3 | 03 Router codegen + mounts | 2 | FRONTEND-01, MON-03/04, GOV-02 | — | N/A | smoke (build + file check) | `(cd frontend && npm run build) && for f in ...; do test -f ... || { echo MISSING; exit 1; }; done && grep -c createFileRoute frontend/src/routeTree.gen.ts` | ❌ W0 (route files) | ⬜ pending |
| 22-03-T4 | 03 Router codegen + dashboard wiring | 2 | MON-03, MON-04 | — | Dashboards fetch `/v1/metrics/{dead-rules,coverage}` via VITE_API_URL (not `/api/v1`); backend→frontend adapters present | smoke/grep | `(cd frontend && npm run build) && ! grep -rn "/api/v1" frontend/src/features/monitoring/` | ✅ existing (dashboards) | ⬜ pending |
| 22-04-T1 | 04 Metrics WS | 2 | MON-02 | — | WS `/v1/metrics/ws` upgrades and streams a frame | integration | `cargo test -p api --test metrics_ws` | ❌ W0 (WS handler) | ⬜ pending |
| 22-04-T2 | 04 Metrics WS | 2 | MON-02 | — | `useEngineMetrics` targets `/v1/monitoring/engine-state` (not `/api/v1/metrics`); single-object→array adapter | smoke/grep | `(cd frontend && npm run build) && ! grep -rn "/api/v1/metrics" frontend/src/` | ❌ W0 | ⬜ pending |
| 22-05-T1 | 05 Governance UI | 3 | GOV-03 | — | AuditLogViewer calls `/v1/audit`; export returns CSV/JSON; adapter present | integration + smoke | `cargo test -p api --test audit && (cd frontend && npm run build)` | ✅ existing + ❌ W0 wiring | ⬜ pending |
| 22-05-T2 | 05 Governance UI | 3 | GOV-02 | — | ApprovalQueue mounted; `/approvals/pending` returns list; loading/error/empty | integration + smoke | `cargo test -p api --test approvals && (cd frontend && npm run build)` | ✅ existing + ❌ W0 mount | ⬜ pending |
| 22-06-T1 | 06 Branch mgmt + backend fix | 3 | GOV-04 | T-22-04 (branch name injection) | BranchManager creates (with `name`), lists, merges; 409 surfaces `conflicts[]`; sanitize guard | integration | `cargo test -p api --test branches` (incl. create-with-name + sanitize) | ✅ existing + ❌ W0 backend fix | ⬜ pending |
| 22-06-T2 | 06 Branch mgmt + backend fix | 3 | GOV-04, FRONTEND-01 | — | BranchManager CRUD + merge conflict UI + side-by-side diff; adapter present | smoke/grep | `(cd frontend && npm run build) && ! grep -rn "/api/v1" frontend/src/features/versioning/` | ❌ W0 (wiring) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

> Files that do not exist yet and must be created before/with the tasks that depend on them.

- [ ] Frontend test framework decision: confirm vitest present in `frontend/vite.config.ts` / `package.json`. If absent, planner decides whether to add vitest (recommended for component smoke) or rely on `tsc --noEmit` + `npm run build` + manual/E2E. This is a **planner decision**, not a blocker.
- [ ] `api/tests/auth_endpoints.rs` (or extend existing test module) — covers GOV-01 login/me/refresh
- [ ] `api/tests/auth_protection.rs` — covers GOV-01 `requires_auth` 401 on protected routes
- [ ] `api/tests/metrics_ws.rs` — covers MON-02 WS upgrade + broadcast frame
- [ ] `api/tests/branches.rs` — add create-with-name test (covers GOV-04 backend fix for the `create_branch` hardcode)
- [ ] Frontend route files under `frontend/src/routes/` — `rules/new.tsx`, `rules/$id.edit.tsx`, `monitoring/*`, `governance/{approvals,audit,branches,rbac}.tsx` (covers FRONTEND-01, MON-02/03/04, GOV-02/03/04 mounting)
- [ ] `@tanstack/router-plugin` installed + `@tanstack/react-router` upgraded to current `1.x` (Pitfall 5 — version gap) — install + verify generated `routeTree.gen.ts` compiles

*Existing infrastructure (cargo test, frontend build) covers the backend-integration portions of GOV-02/03/04; the new work is frontend wiring + the auth/WS/create-branch backend pieces.*

---

## Property Checks (phase gate — non-inferable, hold-out / E2E)

> These are the property-style / E2E checks the planner must lift into `must_haves.truths` (covered edges) or note as held-out (backstop). They prove the wiring holds as a whole, not per-task.

- [ ] **P-1 Route completeness:** every route in the generated `routeTree.gen.ts` resolves to a mounted component (no orphaned route files, no missing component). *Backstop — needs a build-time / AST check.*
- [ ] **P-2 Path alignment complete:** `grep -rn "/api/v1" frontend/src/` returns nothing; all axios/fetch paths go through `VITE_API_URL` (default `/v1`). *Covered — grep assertion. Owned by 22-03 (fixes the last /api/v1 files — the monitoring dashboards); 22-01 fixes client.ts+auth-context.tsx, 22-04 fixes useEngineMetrics.ts. Verified at phase gate after Wave 2.*
- [ ] **P-3 Auth enforcement:** a request to any protected `/v1/*` route without a JWT returns 401; a JWT with a wrong-signature secret is rejected; a valid JWT is accepted. *Covered — integration tests.*
- [ ] **P-4 WS liveness:** a client connecting to `/v1/metrics/ws` completes the WS handshake and receives at least one frame broadcast from `metrics_tx`. *Backstop — integration test with a live WS client.*
- [ ] **P-5 Branch create honors name:** two sequential `POST /v1/branches` with different `name` values create two distinct branches (regression guard for the `create_branch` hardcode). *Covered — integration test.*
- [ ] **P-6 Contract adapters:** governance UIs render real backend fields (no `undefined` after mock→real swap) — audit, branches, metrics each map backend shape → frontend shape. *Backstop — needs a runtime smoke / type-narrowing test.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Login flow end-to-end in browser (dev) | GOV-01 | Requires running backend + frontend + browser session; demo credentials | Start backend (`cargo run`) + frontend (`npm run dev`); navigate to `/`, log in as a demo user; confirm redirect + `usePermission` gates UI by role |
| WS dashboard updates live | MON-02 | Real-time observation of broadcast | Open `/monitoring/engine`, confirm WS connects (devtools network → `/v1/metrics/ws` 101), confirm no immediate `onerror` |
| Branch merge conflict-resolution UI | GOV-04 | Interactive conflict accept/abort | Create divergent branches, trigger merge that 409s, confirm conflict UI shows `conflicts[]` and resolve/abort works |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies listed
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (auth tests, metrics_ws test, branches create-name test, route files, router-plugin install)
- [x] No watch-mode flags
- [x] Feedback latency < 180s full; < 60s quick
- [x] `nyquist_compliant: true` set in frontmatter (task IDs finalized and Per-Task Map populated)

**Approval:** ready (planner aligned task IDs to this map; blockers 1-3 and warnings 1-3 addressed in revision)