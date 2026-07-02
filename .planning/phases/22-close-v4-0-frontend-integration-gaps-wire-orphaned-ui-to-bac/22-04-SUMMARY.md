---
phase: 22-close-v4-0-frontend-integration-gaps-wire-orphaned-ui-to-bac
plan: "04"
subsystem: api
tags: [websocket, actix-web-actors, realtime, metrics, monitoring, mon-02, frontend-wiring, broadcast]

# Dependency graph
requires:
  - phase: 22-close-v4-0-frontend-integration-gaps-wire-orphaned-ui-to-bac
    provides: "Plan 22-01: /v1 scope alignment + VITE_API_URL + CORS so the WS route and the frontend hook resolve to the right paths; Plan 22-02: requires_auth wrap on /v1 (the WS endpoint sits behind it and is public-via-allowlist)"
provides:
  - "Real WebSocket upgrade at /v1/metrics/ws broadcasting metrics::EngineState from the metrics_tx broadcast channel (replaces the JSON-stub handler — RESEARCH § Pitfall 6)"
  - "MetricsWsSession actor (actix-web-actors::ws) subscribing to metrics_tx and forwarding every broadcast EngineState as a JSON text frame"
  - "Frontend useEngineMetrics hook retargeted to /v1/monitoring/engine-state + /v1/metrics/ws with the single-object→array adapter (Pitfall 4 fixed)"
  - "/v1/metrics/ws added to the requires_auth public allowlist so a browser new WebSocket() (which cannot send a Bearer token) can connect to the read-only stream"
affects: [22-03 (mounts the monitoring dashboard that consumes this hook; MON-02 closed end-to-end with 22-03's mount), 22-05, 22-06, v5.0 (execute_single will push real EngineState to metrics_tx — currently a stub, D-22-03)]

# Tech tracking
tech-stack:
  added: ["actix-web-actors 4.3.1 (ws::start + WebsocketContext)", "actix 0.13 (Actor/StreamHandler/AsyncContext traits — declared directly so use actix::Actor resolves)", "actix-test 0.1 (dev-dep; TestServer for the WS integration test)"]
  patterns:
    - "Pattern: actor-based WS broadcast session — MetricsWsSession subscribes to a tokio broadcast::Sender in started() and polls try_recv() on a short run_interval, forwarding each frame as ctx.text(json). Broadcast::Receiver is !Clone, so subscribe-per-session is the correct shape (each connected client gets its own receiver)."
    - "Pattern: public-allowlist bypass for read-only WS — a browser WebSocket handshake cannot attach a Bearer Authorization header, so a read-only broadcast endpoint behind a /v1 auth wrap MUST be added to the middleware allowlist (not made to require a token)."
    - "Pattern: single-object→array adapter at the fetch boundary — when a backend returns one object and a frontend hook expects an array, adapt at the hook (not the backend) so the UI contract stays stable."

key-files:
  created:
    - api/tests/metrics_ws.rs
  modified:
    - api/src/websocket.rs
    - api/src/main.rs
    - api/src/middleware/auth.rs
    - api/Cargo.toml
    - frontend/src/hooks/useEngineMetrics.ts
    - Cargo.lock

key-decisions:
  - "Polling over push for the actor: MetricsWsSession uses ctx.run_interval(50ms) + rx.try_recv() to drain buffered broadcasts, rather than a tokio task + actix Message/Handler push. Polling avoids defining a Message type + Handler impl and is near-real-time (50ms) for a monitoring dashboard; the broadcast channel buffers frames between ticks."
  - "Flat route for configure_metrics_ws: registers /metrics/ws as a flat route inside /v1 (matching configure_metrics_dashboards) rather than a nested web::scope(\"/metrics\"), so the dashboard's existing flat /metrics/dead-rules + /metrics/coverage routes and the WS route share one /v1/metrics/* prefix without a scope/flat registration overlap."
  - "/v1/metrics/ws is public via the auth allowlist (Rule 2 auto-fix): a browser new WebSocket() cannot send a Bearer token, and the stream is read-only (threat model: broadcasts EngineState, no user input). Without this entry the /v1 auth wrap from 22-02 would 401 the WS upgrade."
  - "buildMetricsWsUrl handles both VITE_API_URL shapes: same-origin path prefix ('/v1' → ws(s)://location.host/v1/metrics/ws) and absolute separate-host URL ('https://api.../v1' → wss://api.../v1/metrics/ws) so D-22-05's separate-host prod mode is real."
  - "Did not modify REQUIREMENTS.md: MON-02 is already [x] (stub) in the tracking section; 22-04 upgrades it to functional, and the orchestrator owns post-wave requirement reconciliation (precedent set by 22-01)."

patterns-established:
  - "Pattern: actix-web-actors ws::start session subscribing to a broadcast channel via run_interval + try_recv — the canonical shape for a read-only real-time broadcast endpoint in this codebase."
  - "Pattern: WS endpoints behind /v1 auth are allowlist-bypassed (not token-gated) when the stream is read-only and the client is a browser."

requirements-completed: [MON-02]

# Coverage metadata (#1602) — one entry per shipped deliverable.
coverage:
  - id: D1
    description: "Real WebSocket upgrade at /v1/metrics/ws broadcasting metrics::EngineState from metrics_tx (replaces the JSON-stub handler — Pitfall 6)"
    requirement: MON-02
    verification:
      - kind: integration
        ref: "cargo test -p api --test metrics_ws — 3/3 pass (101 handshake, broadcast→EngineState frame, old JSON stub gone)"
        status: pass
      - kind: automated
        ref: "grep -n 'ws::start' api/src/websocket.rs → present; grep old 'HttpResponse::Ok().json(serde_json::json!({\"message\": \"WebSocket endpoint\"' stub → absent; grep 'configure_metrics_ws' in main.rs /v1 scope → present"
        status: pass
    human_judgment: false
  - id: D2
    description: "Frontend useEngineMetrics hook retargeted to /v1/monitoring/engine-state + /v1/metrics/ws with the single-object→array adapter (Pitfall 4)"
    requirement: MON-02
    verification:
      - kind: automated
        ref: "grep -rn '/api/v1/metrics' frontend/src/hooks/useEngineMetrics.ts → none; grep 'monitoring/engine-state' → present; grep '/metrics/ws' → present; adapter (adaptEngineStateResponse + EngineStateResponse) present"
        status: pass
      - kind: automated
        ref: "npx tsc --noEmit — useEngineMetrics.ts has 0 errors (the 32 phase-wide errors are all in files owned by 22-03/22-06, see Deviations)"
        status: pass
    human_judgment: false

# Metrics
duration: ~25 min
completed: 2026-07-02
status: complete
---

# Phase 22 Plan 04: Real-Time Metrics WebSocket Summary

**Real WS upgrade at /v1/metrics/ws broadcasting EngineState (replacing the JSON-stub handler), plus the useEngineMetrics hook retargeted to /v1/monitoring/engine-state with the single-object→array adapter — closing MON-02's streaming half (the dashboard mount lands in 22-03).**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-07-02T09:35:00Z
- **Completed:** 2026-07-02T09:48:06Z
- **Tasks:** 2 (Task 1 was TDD: RED → GREEN)
- **Files:** 1 created, 6 modified (+ Cargo.lock)

## Accomplishments

- Replaced the `metrics_ws` JSON-stub handler with a real WebSocket upgrade via `actix_web_actors::ws::start` (RESEARCH § Pitfall 6). The `MetricsWsSession` actor subscribes to `metrics_tx` in `started()` and forwards every broadcast `EngineState` as a JSON text frame on a 50ms `run_interval` cadence.
- Registered `configure_metrics_ws` inside the `/v1` scope in `main.rs` so the endpoint is `/v1/metrics/ws` (it was defined but NEVER called before — D-22-10).
- Added `/v1/metrics/ws` to the `requires_auth` public allowlist so a browser `new WebSocket()` (which cannot send a Bearer token) connects to the read-only stream; the threat model confirms the boundary is read-only (no user input accepted).
- Retargeted `useEngineMetrics.ts` from the stale `/api/v1/metrics` (never a real route) to `${API_URL}/monitoring/engine-state` and `/v1/metrics/ws`; added the `adaptEngineStateResponse` single-object→array adapter (Pitfall 4 — the backend returns one `EngineStateResponse`, the hook exposes `MetricsData[]`).
- Added a 3-case WS integration test (`api/tests/metrics_ws.rs`) using a raw TCP WS client against an `actix-test` server — handshake completes with 101, a broadcast on `metrics_tx` is received as a serialized `EngineState` frame, and the old JSON stub is gone.

## Task Commits

Each task was committed atomically (HEAD-drift guard run before each commit):

1. **Task 1 (RED): add failing WS integration test + test deps** — `c63343ce7` (test)
2. **Task 1 (GREEN): real WS upgrade at /v1/metrics/ws + register in /v1 scope** — `ebd620440` (feat)
3. **Task 2: fix useEngineMetrics paths + single-object→array adapter** — `ce5694a93` (feat)

**Plan metadata:** (this commit) (docs: complete plan)

_Note: Task 1 is `tdd="true"` — the RED test (failing against the 200 JSON stub) was committed before the GREEN implementation, per the TDD gate sequence._

## Files Created/Modified

- `api/src/websocket.rs` — `MetricsWsSession` actor (`Actor` + `StreamHandler`) subscribing to `metrics_tx`; `metrics_ws` now calls `ws::start` (real upgrade, replaces JSON stub); `configure_metrics_ws` registers a flat `/metrics/ws` route
- `api/src/main.rs` — declares `mod websocket;`; registers `websocket::configure_metrics_ws` inside the `/v1` scope so the endpoint is `/v1/metrics/ws`
- `api/src/middleware/auth.rs` — adds `/v1/metrics/ws` to `PUBLIC_PREFIXES` (Rule 2 auto-fix)
- `api/Cargo.toml` — adds `actix-web-actors = "4"`, `actix = "0.13"`; dev-dep `actix-test = "0.1"`
- `api/tests/metrics_ws.rs` (new) — 3 integration tests (101 handshake, broadcast→EngineState frame, JSON stub gone) with a raw TCP WS client + `actix-test` server
- `frontend/src/hooks/useEngineMetrics.ts` — `API_URL` source of truth; `fetchEngineMetrics()` → `${API_URL}/monitoring/engine-state` + `adaptEngineStateResponse`; `buildMetricsWsUrl()` → `ws(s)://.../v1/metrics/ws`; `setLoading(false)` on WS open
- `Cargo.lock` — actix-web-actors 4.3.1 + actix 0.13.5 + actix-test 0.1.5 + transitive deps (awc, actix-http-test)

## Decisions Made

- **Polling (run_interval + try_recv) over push (Message + Handler).** The actor polls `metrics_tx` every 50ms and drains `try_recv`, rather than spawning a tokio task that pushes via an `actix::Message` + `Handler`. Polling avoids defining a message type and handler impl and is near-real-time for a monitoring dashboard; the broadcast channel buffers frames between ticks. The plan explicitly permitted actor-design discretion ("spawn a task that loops over rx.recv()" was the suggested shape; the interval-poll form is equivalent and simpler in this codebase).
- **Flat route for `configure_metrics_ws`.** Registers `/metrics/ws` as a flat route inside `/v1` (matching `configure_metrics_dashboards`) instead of a nested `web::scope("/metrics")`, so the dashboard's flat `/metrics/dead-rules` + `/metrics/coverage` and the WS route share one `/v1/metrics/*` prefix without a scope/flat registration overlap. Same resolved path (`/v1/metrics/ws`); no semantic change.
- **`/v1/metrics/ws` public via the auth allowlist (Rule 2 auto-fix).** A browser `new WebSocket()` cannot attach a Bearer Authorization header, and the stream is read-only (threat model). Without this entry the `/v1` `requires_auth` wrap added in 22-02 would 401 the upgrade. This is a 1-line addition to `middleware/auth.rs` (not in the plan's `files_modified`); documented as a deviation.
- **`buildMetricsWsUrl` handles both `VITE_API_URL` shapes.** Same-origin path (`/v1` → `ws(s)://location.host/v1/metrics/ws`) and absolute separate-host (`https://api.../v1` → `wss://api.../v1/metrics/ws`) so D-22-05's separate-host prod mode works for WS, not just HTTP.
- **Did not modify REQUIREMENTS.md.** MON-02 is already `[x]` (stub) in the tracking section; 22-04 upgrades it to functional, and the orchestrator owns post-wave requirement reconciliation (precedent set by 22-01, which also did not modify REQUIREMENTS.md).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added actix-web-actors + actix + actix-test dependencies**
- **Found during:** Task 1 (RED — the test references `actix_test::start` and the implementation needs `actix_web_actors::ws::start` + `actix::Actor`)
- **Issue:** The plan's `read_first` says "actix-web `ws` feature already enabled" and the action says `actix_web_actors::ws::start`, but `actix-web-actors` was NOT in `Cargo.lock` (the `actix-web` `ws` feature provides low-level helpers, not the actor `ws::start` / `WebsocketContext`). `actix` (the `Actor`/`StreamHandler`/`AsyncContext` traits) was also not declared directly. `actix-test` (the `TestServer` the integration test needs) was absent.
- **Fix:** Added `actix-web-actors = "4"`, `actix = "0.13"` to `[dependencies]` and `actix-test = "0.1"` to `[dev-dependencies]` in `api/Cargo.toml`. Resolved to 4.3.1 / 0.13.5 / 0.1.5. (Note: `actix-web-actors 4.3.1` is marked `+deprecated` in crates.io in favor of the newer `actix-web` extractor API, but the plan explicitly specifies `actix_web_actors::ws::start`; the crate is functional, not removed.)
- **Files modified:** api/Cargo.toml, Cargo.lock
- **Verification:** `cargo test -p api --test metrics_ws` compiles and 3/3 pass
- **Committed in:** `c63343ce7` (RED commit; deps are required for the test to compile)

**2. [Rule 2 - Missing Critical] Added /v1/metrics/ws to the requires_auth public allowlist**
- **Found during:** Task 1 (GREEN — the endpoint sits behind 22-02's `wrap(from_fn(requires_auth))` on the `/v1` scope)
- **Issue:** A browser `new WebSocket()` cannot attach a Bearer Authorization header (the WS handshake is a plain HTTP upgrade). Without an allowlist entry the middleware would 401 the upgrade and the frontend `useEngineMetrics` hook could never connect. The plan's threat model treats the WS boundary as read-only ("broadcasts read-only EngineState; no user input accepted") with no auth requirement, but the plan's `files_modified` did not list `middleware/auth.rs`.
- **Fix:** Added `"/v1/metrics/ws"` to `PUBLIC_PREFIXES` in `api/src/middleware/auth.rs` (1-line + doc-comment). The stream stays read-only; no new attack surface (T-22-SC unaffected).
- **Files modified:** api/src/middleware/auth.rs
- **Verification:** the WS integration test (which uses a raw client with no Bearer token) completes the 101 handshake; `cargo test --workspace` 257/0
- **Committed in:** `ebd620440` (GREEN commit)

### Planned-layout refinement (documented, not a blocker)

**3. Flat route for `configure_metrics_ws` instead of `web::scope("/metrics")`**
- The plan's action said "register the route as `/metrics/ws` INSIDE the `/v1` scope" and the prior code used `web::scope("/metrics").route("/ws", ...)`. I used a flat `cfg.route("/metrics/ws", ...)` to match `configure_metrics_dashboards` (which already registers flat `/metrics/dead-rules` + `/metrics/coverage` inside `/v1`) and avoid a scope/flat registration overlap under the shared `/v1/metrics/*` prefix. Same resolved path (`/v1/metrics/ws`); no semantic change.

### Out-of-scope pre-existing issue (documented, not auto-fixed)

**4. [Scope boundary] Pre-existing frontend `tsc --noEmit` / `npm run build` failures in files this plan does not touch**
- **Found during:** Task 2 (acceptance criterion `cd frontend && npx tsc --noEmit exits 0`)
- **Issue:** `npx tsc --noEmit` exits 2 with 32 errors, all in `src/monitoring/components/Chart.tsx` (29), `src/routes/monitoring.tsx` (1), `src/features/monitoring/routes.tsx` (1), and `src/features/versioning/BranchManager.tsx` (1) — files NOT modified by 22-04. `npm run build` (`tsc -b && vite build`) fails for the same reason.
- **Evidence:** Same 32 errors, same 4 files flagged in 22-01-SUMMARY at the base commit (verified there via `git stash`). My changes introduce 0 new tsc errors (`useEngineMetrics.ts` is tsc-clean; `grep useEngineMetrics` in `npm run build` output → no matches).
- **Fix:** NOT applied — out of scope. Per the scope-boundary rule, these files are owned by plan 22-03 (the monitoring dashboards: `Chart.tsx`, `monitoring/routes.tsx`, `routes/monitoring.tsx`) and 22-06 (`BranchManager.tsx` conflict-resolution rewrite), which replace this code. The phase-wide tsc/build green is the orchestrator's post-merge check, not a 22-04 deliverable.
- **Files affected:** (none modified by 22-04) `src/monitoring/components/Chart.tsx`, `src/routes/monitoring.tsx`, `src/features/monitoring/routes.tsx`, `src/features/versioning/BranchManager.tsx`
- **Verification:** the scoped deliverable (useEngineMetrics path alignment + adapter) is deterministically proven by grep + the file-local tsc-clean check
- **Committed in:** N/A (not fixed; owned by 22-03/22-06)

### Minor correctness fix (within the feature's scope)

**5. `setLoading(false)` on WS open**
- The prior hook only cleared `loading` in the HTTP-fallback path; on a successful WS connection `loading` stayed `true` forever (the UI was stuck "loading"). Added `setLoading(false)` in `ws.onopen` so the hook is functional on the primary (WS) path. One line; directly required for the feature to work as designed.

---

**Total deviations:** 2 auto-fixes (1 Rule 3 blocking dep, 1 Rule 2 missing-critical allowlist) + 1 documented layout refinement + 1 out-of-scope pre-existing issue (not fixed) + 1 minor in-scope correctness fix. No plan semantics changed.
**Impact on plan:** All auto-fixes were necessary for the WS endpoint to function (deps) and to be reachable from a browser (allowlist). The pre-existing tsc/build failures are owned by 22-03/22-06 and do not block 22-04's deliverables (proven by grep + file-local tsc-clean + the 3/3 WS integration test).

## Issues Encountered

- **`actix_test::start` is synchronous, not async.** The first test draft awaited `start(...).await`; the compiler reported "`TestServer` is not a future". Fixed by treating `start` as sync, keeping the returned `TestServer` alive in the test scope (so the listener stays open for the raw TCP client), and reading the bound address via `server.addr()`.
- **`actix::AsyncContext` must be in scope for `run_interval`.** The first implementation draft called `ctx.run_interval(...)` without importing the trait; the compiler suggested `use actix::AsyncContext;`. Added the import and explicitly typed the closure parameters (`&mut Self`, `&mut WebsocketContext<Self>`) to resolve inference.

## User Setup Required

None for this plan. The WS endpoint is read-only and public-via-allowlist; no new env vars beyond what 22-01 set (`VITE_API_URL`, `CORS_ALLOWED_ORIGINS`) and 22-02 set (`API_SECRET_KEY`). A production deployment with a separate API host should set `VITE_API_URL` (absolute, e.g. `https://api.rrules.example.com/v1`) — `buildMetricsWsUrl` derives the WS URL from it (D-22-05).

## Next Phase Readiness

- **Ready for 22-03 (dashboard mount).** The `useEngineMetrics` hook now targets the correct paths with the correct shape adapter; 22-03 mounts the monitoring dashboard that consumes it, closing MON-02 end-to-end (the stream legitimately pushes empty state while `execute_single` is a stub — accepted tech debt per D-22-03, not a 22-04 blocker).
- **Ready for 22-05/22-06.** The `/v1/metrics/ws` endpoint and the auth-allowlist pattern are in place; downstream plans are unaffected.
- **Orchestrator follow-up (D-22-03):** real `EngineState` will be pushed to `metrics_tx` once `execute_single` is implemented at a future execution milestone — the stream currently broadcasts nothing until then (the hook falls back to HTTP polling, which reads the zero-state `EngineStateResponse`).
- **No blockers** for the next wave beyond the documented pre-existing frontend type errors, which are owned by 22-03/22-06.

---
*Phase: 22-close-v4-0-frontend-integration-gaps-wire-orphaned-ui-to-bac*
*Completed: 2026-07-02*