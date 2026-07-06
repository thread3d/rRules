---
phase: 28
slug: frontend-test-runner
status: active
nyquist_compliant: false
wave_0_complete: true
created: 2026-07-06
updated: 2026-07-06
---

# Phase 28 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: `28-RESEARCH.md` §Validation Architecture. This phase touches BOTH backend
> (`api` + `engine` crates) and frontend (`frontend` Vitest) — two test surfaces.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Backend framework** | `cargo test` (Rust integration tests in `api/tests/`, unit tests in `engine/`) |
| **Backend config file** | `api/Cargo.toml`, `engine/Cargo.toml` — existing |
| **Frontend framework** | Vitest ^2.0.0 + `@testing-library/react` ^16.0.0 (devDeps present; **no config/setup/tests exist — Wave 0 scaffolds**) |
| **Frontend config file** | `frontend/vite.config.ts` — Wave 0 adds `test` block; `frontend/src/test/setup.ts` — NEW |
| **Backend quick run command** | `cargo test -p api --test execute_integration` |
| **Backend full suite command** | `cargo test -p api -p engine` |
| **Frontend quick run command** | `cd frontend && npm test -- --run` |
| **Frontend full suite command** | `cd frontend && npm test` |
| **Estimated runtime** | ~30 seconds (backend) + ~10 seconds (frontend after Wave 0) |

---

## Sampling Rate

- **After every task commit (backend):** Run `cargo test -p api --test execute_integration`
- **After every task commit (frontend):** Run `cd frontend && npm test -- --run`
- **After every plan wave:** Run `cargo test -p api -p engine && cd frontend && npm test`
- **Before `/gsd-verify-work`:** Full backend + frontend suites green; SC-1..SC-4 keystone e2e demonstrated
- **Max feedback latency:** ~40 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 28-01-01 | 01 | 0 | EXEC-04 | T-28-trace-leak | Trace opt-in; `data: None` (lean payload) | integration | `cargo test -p api --test execute_integration -- trace_on_populates_entries` | ❌ W0 | ⬜ pending |
| 28-01-02 | 01 | 0 | EXEC-04 | — | Trace off → `None` | integration | `cargo test -p api --test execute_integration -- trace_off_is_none` | ❌ W0 | ⬜ pending |
| 28-01-03 | 01 | 0 | EXEC-04 | — | Rete two-site trace (C1) | integration | `cargo test -p api --test execute_integration -- rete_trace_populated` | ❌ W0 | ⬜ pending |
| 28-02-01 | 02 | 0 | FRONTEND-04 / D-03 | T-28-engine-enum | `GET /v1/engines` behind `requires_auth`; sorted, server-derived | integration | `cargo test -p api --test execute_integration -- list_engines_returns_registry` | ❌ W0 | ⬜ pending |
| 28-03-01 | 03 | 0 | FRONTEND-03 | — | Run renders `rules_fired`/`rules_not_fired`/`facts_after` | unit (component) | `cd frontend && npm test -- TestRunner.test.tsx -t "renders fired rules and facts_after"` | ❌ W0 | ⬜ pending |
| 28-03-02 | 03 | 0 | FRONTEND-04 | — | Dropdown lists engines from `GET /v1/engines` | unit (component) | `cd frontend && npm test -- TestRunner.test.tsx -t "dropdown lists engines"` | ❌ W0 | ⬜ pending |
| 28-03-03 | 03 | 0 | EXEC-04 (UI) | — | Trace section hidden when `None`; shown when present | unit (component) | `cd frontend && npm test -- TestRunner.test.tsx -t "trace section hidden when null"` | ❌ W0 | ⬜ pending |
| 28-03-04 | 03 | 0 | SC-4 (D-07) | T-28-stale-result | Reload-then-Run polls `getJobStatus` then executes; `engine_generation` correlation | unit (component) | `cd frontend && npm test -- TestRunner.test.tsx -t "reload then run polls job status"` | ❌ W0 | ⬜ pending |
| 28-03-05 | 03 | 0 | EXEC-04 (latency) | T-28-trace-dos | Trace-off latency ≈ Phase 24 baseline (SC-3) | manual | manual benchmark / loose timing assertion | ❌ manual-only | ⬜ pending |
| 28-04-01 | 04 | 1 | SC-1 keystone | — | `score := score + 10` → `facts_after.scorecard.score == 10` through UI | e2e/manual | manual click-through (Playwright not set up) | ❌ manual-only | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `api/tests/execute_integration.rs` — RED scaffolds: `trace_on_populates_entries`, `trace_off_is_none`, `rete_trace_populated`, `list_engines_returns_registry` (extend existing file; reuse `init_test_app!`/`configure_test_routes` fixtures at `:231-391`).
- [ ] `frontend/vite.config.ts` — add `test` block: `{ environment: 'jsdom', globals: true, setupFiles: ['./src/test/setup.ts'] }`.
- [ ] `frontend/src/test/setup.ts` — NEW: import `@testing-library/jest-dom` (install if missing).
- [ ] `frontend/src/components/__tests__/TestRunner.test.tsx` — NEW RED scaffolds for the 4 component behaviors above.
- [ ] `frontend/src/main.tsx` — add `QueryClientProvider` (Critical Finding 2: TanStack Query installed but UNUSED — no provider) so `useQuery`/`useMutation` work in tests AND the app.
- [ ] Verify/install `jsdom` + `@testing-library/jest-dom` (`npm ls jsdom @testing-library/jest-dom`).
- [ ] No new backend framework install needed (`cargo test` already runs).

*Backend test infra exists; frontend infra must be scaffolded from zero.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Trace-off latency neutrality (SC-3) | EXEC-04 | Tight timing assertions are flaky in CI | Run `cargo test -p api --test execute_integration -- trace_off_latency_neutral` (loose bound) OR manual benchmark vs Phase-24 baseline; record the delta in SUMMARY. |
| SC-1 keystone click-through | FRONTEND-03 | Playwright/e2e not set up in this project | Start the dev stack, open the edit page for the `score := score + 10` rule, click Run, confirm `facts_after.scorecard.score == 10` renders. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 40s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending