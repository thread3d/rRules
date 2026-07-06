---
phase: 27-stateful-sessions
plan: 01
subsystem: api
tags: [stateful-sessions, dashmap, ttl, lru, prometheus, actix-web, tokio, web-block, session-management]

# Dependency graph
requires:
  - phase: 23-real-engine-wiring
    provides: EngineRegistry + per-request Box<dyn ExecutionEngine> surface (the contract a stateful session wraps)
  - phase: 24-metrics-monitoring
    provides: MetricsSnapshot.active_sessions field (hardcoded 0) + dedicated Prometheus Registry (Pitfall 24-A)
  - phase: 25-ruleset-snapshots
    provides: SharedRuleSet{ruleset, generation} atomic snapshot + reload_and_swap generation bump
provides:
  - SessionManager (Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>>) with lookup_or_create, sweep_expired, dispose_by_alias, reset_by_alias
  - execute_sessionful path in execute_single (session_id branch with Pitfall 7 web::block !Send guard scope)
  - ExecuteResponse.session_id: Option<String> (server UUID v4 returned to caller; client id as idempotency alias)
  - Background tokio::time::interval sweep task (run_session_sweep) evicting TTL/max-lifetime/LRU sessions every 60s
  - rrules_sessions_active Prometheus IntGauge on the dedicated Registry (NOT default_registry — Pitfall 24-A)
  - DELETE /v1/execute/session/{id} (dispose) + POST /v1/execute/session/{id}/reset (clear WM, keep engine) lifecycle routes
  - MetricsSnapshot.active_sessions populated from SessionManager::len() (replaces hardcoded 0)
  - Four #[serial_test::serial] SC integration tests (SC-1 accumulate, SC-2 TTL, SC-3 LRU+gauge, SC-4 lifecycle)
affects: [phase-28-frontend-session-ui, phase-28-inference-trace, v5.0-live-execution]

# Tech tracking
tech-stack:
  added: [dashmap 6.2.1 (pinned per ROADMAP — NOT 7.0.0-rc2)]
  patterns:
    - "Per-session Box<dyn ExecutionEngine> held across calls (D-01 — wraps the Phase-23 surface, NOT v1.0 environment::Session stub)"
    - "Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>> sharded registry (D-02 — no global write lock on hot path)"
    - "Server UUID v4 primary key + client session_id as idempotency alias (D-04 — second DashMap<String, Uuid>)"
    - "TTL + LRU + max-count + background sweep + Prometheus gauge shipped TOGETHER (D-03 — OOM is silent)"
    - "Pitfall 7 web::block !Send guard scope: MutexGuard in its own block scope, std::mem::replace engine out, drop guard at block end BEFORE .await"
    - "Dedicated prometheus::Registry (Pitfall 24-A — register() panics on duplicate names on default_registry)"
    - "#[serial_test::serial] + SessionManager::clear() for test isolation (D-09)"
    - "reset_session rebuilds a FRESH engine (ReteEngine::reset clears working_facts AND rules — rete/mod.rs:525-532)"

key-files:
  created:
    - api/src/v1/sessions.rs
  modified:
    - api/Cargo.toml
    - api/src/v1/mod.rs
    - api/src/v1/execution.rs
    - api/src/main.rs
    - api/tests/execute_integration.rs
    - api/tests/batch_execution_integration.rs
    - api/tests/metrics_ws.rs

key-decisions:
  - "D-01: StatefulSession wraps per-session Box<dyn ExecutionEngine> (Phase-23 surface), NOT v1.0 environment::Session stub"
  - "D-02: Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>> sharded registry (DashMap 6.2.1 pinned)"
  - "D-03: TTL + LRU + max-count + background sweep + Prometheus gauge ship TOGETHER (Pitfall 5 — OOM is silent)"
  - "D-04: Server UUID v4 primary key + client session_id as idempotency alias (second DashMap<String, Uuid>)"
  - "D-05: Lifecycle routes DELETE /execute/session/{id} + POST /execute/session/{id}/reset"
  - "D-06: Engine fixed at session creation (load_ruleset once, reuse across calls — no shared lock on hot resume path)"
  - "D-07: engine_generation captured at creation and stamped into every sessionful ExecuteResponse"
  - "D-08: ExecuteResponse.session_id: Option<String> with #[serde(skip_serializing_if)] — stateless path stays unchanged"
  - "D-09: #[serial_test::serial] + SessionManager::clear() for SC test isolation"
  - "D-10: Background tokio::time::interval sweep task (mirrors start_file_watcher pattern)"
  - "D-11: Per-session std::sync::Mutex (engine &mut self is !Sync); engine.execute() runs in web::block"
  - "D-12: web::Data<Arc<SessionManager>> shared-state wiring across actix workers"

patterns-established:
  - "Pitfall 7 web::block !Send guard scope: { let mut guard = entry.lock()...; std::mem::replace(&mut guard.engine, placeholder) } — guard drops at block end BEFORE web::block(.await)"
  - "Per-session engine extraction pattern: replace engine out with a placeholder factory-fresh engine, drop MutexGuard, move real engine into web::block, re-lock to write back + bump last_used"
  - "Accumulating rule condition (ComparisonOp::Ge score >= 0) for SC-1 score:20 keystone — fires on every call when scorecard fact exists"

requirements-completed: [SESS-01]

# Coverage metadata — drives DETERMINISTIC UAT routing in verify-work.
coverage:
  - id: D1
    description: "POST /v1/execute with session_id accumulates working memory across calls (SC-1: score:10 then score:20 on second call with facts={})"
    requirement: SESS-01
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs#stateful_session_accumulates_wm"
        status: pass
  - id: D2
    description: "Session untouched for >idle TTL is evicted; touched session persists (SC-2)"
    requirement: SESS-01
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs#session_ttl_evicts_idle"
        status: pass
  - id: D3
    description: "LRU eviction when count exceeds MAX_SESSIONS; rrules_sessions_active gauge reflects capped live count (SC-3)"
    requirement: SESS-01
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs#lru_evicts_oldest_and_gauge_capped"
        status: pass
  - id: D4
    description: "DELETE /execute/session/{id} disposes; POST /execute/session/{id}/reset clears WM without disposing (SC-4)"
    requirement: SESS-01
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs#session_lifecycle_delete_reset"
        status: pass
  - id: D5
    description: "Background sweep task (run_session_sweep) spawned at startup with tokio::time::interval(sweep_interval)"
    requirement: SESS-01
    verification:
      - kind: unit
        ref: "api/src/main.rs:179 — tokio::spawn(run_session_sweep(sweep_manager))"
        status: pass
  - id: D6
    description: "rrules_sessions_active Prometheus gauge on dedicated Registry; MetricsSnapshot.active_sessions populated from SessionManager::len()"
    requirement: SESS-01
    verification:
      - kind: unit
        ref: "api/src/main.rs:113-121 gauge construction + registration; api/src/v1/execution.rs active_sessions: session_manager.len()"
        status: pass
    human_judgment: false

# Metrics
duration: approx. 90min (across 3 sessions, interrupted by context limits)
completed: 2026-07-06
status: complete
---

# Phase 27 Plan 01: Stateful Sessions Summary

**SessionManager with DashMap-sharded per-session Box<dyn ExecutionEngine>, TTL+LRU+max-count+sweep+gauge shipped together — four SC tests GREEN (accumulate, TTL, LRU+gauge, lifecycle).**

## Performance

- **Duration:** approx. 90min (across 3 sessions, interrupted by context limits)
- **Started:** 2026-07-04
- **Completed:** 2026-07-06T08:43Z
- **Tasks:** 3/3
- **Files modified:** 8 (1 created, 7 modified)

## Accomplishments

- **SC-1 (accumulation):** `POST /v1/execute` with `session_id='s1'` + `facts={scorecard:{score:0}}` fires r1 → `facts_after.scorecard.score == 10`; a second call with the SAME `session_id` and `facts={}` fires r1 again against accumulated WM → `score == 20` (impossible in the stateless path). The stateless one-shot path (no `session_id`) is unchanged.
- **SC-2 (TTL):** A session untouched for >idle TTL is evicted by the background sweep; a subsequent call with that `session_id` creates a FRESH session returning `score:10`. A session touched within TTL persists and returns `score:20`.
- **SC-3 (LRU + gauge cap):** When session count exceeds `MAX_SESSIONS`, the LRU entry (oldest `last_used`) is evicted; `rrules_sessions_active` gauge reads `MAX_SESSIONS`, not `MAX_SESSIONS+1`.
- **SC-4 (lifecycle):** `DELETE /v1/execute/session/{id}` disposes the session (next call creates fresh empty WM). `POST /v1/execute/session/{id}/reset` clears WM WITHOUT disposing (next call resumes with empty WM but same engine/generation → `score:10`, not `score:20`).
- **Bounded resource use:** TTL (900s idle) + max lifetime (86400s) + max-count (1024) + background sweep (60s) + `rrules_sessions_active` Prometheus gauge — all shipped TOGETHER (D-03 / Pitfall 5).
- **`MetricsSnapshot.active_sessions`** populated from `SessionManager::len()` (replaces the Phase-24 hardcoded `0`).
- **No regressions:** api 110 passed / 0 failed / 1 ignored; engine 242 passed / 0 failed / 3 ignored; execute_integration 22 passed (incl. 4 new SC tests).

## New + Modified Symbols

### api/src/v1/sessions.rs (CREATED)
- `pub struct SessionConfig { idle_ttl: Duration, max_lifetime: Duration, max_sessions: usize, sweep_interval: Duration }`
- `pub fn read_session_config() -> SessionConfig` — reads `RRULES_SESSION_IDLE_TTL_SECS` / `RRULES_SESSION_MAX_LIFETIME_SECS` / `RRULES_MAX_SESSIONS` / `RRULES_SESSION_SWEEP_INTERVAL_SECS` env vars with D-03 defaults
- `struct StatefulSessionEntry { engine: Box<dyn ExecutionEngine>, engine_name: String, engine_generation: u64, created_at: Instant, last_used: Instant, server_id: Uuid, client_alias: Option<String> }`
- `pub struct SessionManager { sessions: Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>>, aliases: Arc<DashMap<String, Uuid>>, config: SessionConfig, active_gauge: IntGauge }`
- `impl SessionManager` — `new`, `len`, `lookup_or_create` (D-04/D-06: server UUID primary key + client alias idempotency; engine fixed at creation), `sweep_expired` (D-03: TTL/max-lifetime/LRU), `dispose_by_alias`, `reset_by_alias` (rebuilds fresh engine via `registry.create_engine_by_name` + `load_ruleset` — Open Q#1: `ReteEngine::reset` clears rules too), `remove_by_uuid`, `clear` (test isolation)
- `pub async fn run_session_sweep(manager: Arc<SessionManager>)` — tokio::time::interval loop (D-10)
- `pub async fn dispose_session(path, session_manager) -> impl Responder` — DELETE /execute/session/{id}
- `pub async fn reset_session(path, session_manager, registry, ruleset) -> impl Responder` — POST /execute/session/{id}/reset

### api/src/v1/execution.rs (MODIFIED)
- `ExecuteResponse` — added `session_id: Option<String>` with `#[serde(skip_serializing_if = "Option::is_none")]` (D-08)
- `execute_single` — added `session_manager: web::Data<Arc<SessionManager>>` param; branches to `execute_sessionful` when `req.session_id.is_some()`; stateless path sets `session_id: None` and `active_sessions: session_manager.len()` (replaces `0`)
- `execute_sessionful` (NEW) — `lookup_or_create` per-session engine; Pitfall 7 web::block pattern (MutexGuard in own block, `std::mem::replace` engine out with placeholder, drop guard at block end, move engine into `web::block`, re-lock to write back + bump `last_used`); stamps `engine_generation` + `session_id` into response
- `execute_rules` — added `session_manager` param, threads to `execute_single`
- `batch_execute_rules` — added `session_manager` param, clones into JoinSet tasks
- `execute_with_session` — REMOVED (D-08 — replaced by real `execute_sessionful`)

### api/src/main.rs (MODIFIED)
- Import `SessionManager`, `read_session_config`, `run_session_sweep`
- Construct `SessionManager` with `rrules_sessions_active` IntGauge registered on `counters.registry` (dedicated Registry — Pitfall 24-A)
- `tokio::spawn(run_session_sweep(sweep_manager))` after file watcher (D-10)
- `.app_data(web::Data::new(session_manager.clone()))` in HttpServer closure (D-12)
- Routes `/execute/session/{id}` (DELETE) + `/execute/session/{id}/reset` (POST) in /v1 scope (D-05)

### api/src/v1/mod.rs (MODIFIED)
- Added `pub mod sessions;`

### api/Cargo.toml (MODIFIED)
- Added `dashmap = "6.2.1"` (D-02 — pinned per ROADMAP, NOT 7.0.0-rc2)

### api/tests/execute_integration.rs (MODIFIED)
- Added `score_plus_10_accumulating_rule()` (ComparisonOp::Ge `score >= 0` — Rule 1 fix) + `shared_accumulating_ruleset()`
- Added `init_test_app_with_manager!` macro (custom SessionManager for SC-2/SC-3 config-specific tests)
- Added `SHARED_SESSION_MANAGER` OnceLock + `build_session_gauge` (fresh Registry per test file — `rrules_sessions_active_test`)
- Updated `reset_test_env()` to call `shared_session_manager_arc().clear()`
- Updated `configure_test_routes` with lifecycle routes
- Updated `init_test_app!` / `init_two_rule_app!` with `.app_data(shared_session_manager())`
- Appended four `#[serial_test::serial]` SC tests

### api/tests/batch_execution_integration.rs + api/tests/metrics_ws.rs (MODIFIED — Rule 3)
- Added `SessionManager` import + `shared_session_manager()` helper + `.app_data(...)` injection (required extractor missing after signature change)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] score_plus_10_rule condition prevents SC-1 score:20 keystone**
- **Found during:** Task 1 (RED scaffold)
- **Issue:** The plan's SC-1 expects `score:20` on the second same-session call, but the existing `score_plus_10_rule` has condition `score == 0` (ComparisonOp::Eq) which only matches on the FIRST call (after increment, score is 10, so the rule does not fire again).
- **Fix:** Created `score_plus_10_accumulating_rule()` with condition `score >= 0` (ComparisonOp::Ge) — fires on every call when a scorecard fact exists. Determined that `facts={}` on subsequent calls is the deterministic pattern (avoids multiple scorecard facts with non-deterministic HashMap iteration order in `facts_after` merge).
- **Files modified:** api/tests/execute_integration.rs
- **Commit:** 43f1d95d2

**2. [Rule 3 - Blocking] (StatusCode, String) does not implement ResponseError**
- **Found during:** Task 2 (GREEN sessions.rs)
- **Issue:** `dispose_session` and `reset_session` handlers returned `Result<HttpResponse, (StatusCode, String)>` which cannot be used as actix route handler return types (`(StatusCode, String)` does not implement `ResponseError`).
- **Fix:** Changed return type to `impl actix_web::Responder` with `HttpResponse` built directly (matching the `execute_rules` pattern at execution.rs:733-745).
- **Files modified:** api/src/v1/sessions.rs
- **Commit:** 39a15f72e

**3. [Rule 3 - Blocking] MutexGuard is !Send, cannot be held across .await**
- **Found during:** Task 3 (GREEN wiring)
- **Issue:** `engine.execute()` requires `&mut self`; the web::block closure needs to own the engine. Even with an explicit `drop(guard)`, the compiler could not prove the guard was dropped before the `.await` on `web::block`.
- **Fix:** Put the MutexGuard in its own block scope: `{ let mut guard = entry_arc.lock()...; engine_name = guard.engine_name.clone(); let placeholder = registry.create_engine_by_name(...); std::mem::replace(&mut guard.engine, placeholder) }` — the guard drops at the block end BEFORE `web::block(.await)`. The returned engine moves into `web::block`; after it completes, re-lock to write the engine back + bump `last_used`.
- **Files modified:** api/src/v1/execution.rs
- **Commit:** 6c22b6fe9

**4. [Rule 3 - Blocking] Other test files missing session_manager app_data**
- **Found during:** Task 3 (GREEN wiring)
- **Issue:** `batch_execution_integration.rs` and `metrics_ws.rs` use `execute_rules`/`batch_execute_rules` which now require a `session_manager` extractor — without the app_data injection the handlers return 500 on the missing extractor.
- **Fix:** Added `SessionManager` imports, `shared_session_manager()` helpers (with distinct gauge names `rrules_sessions_active_batch` / `rrules_sessions_active_ws` to avoid Prometheus duplicate-registration panics), and `.app_data(...)` injection to both test files.
- **Files modified:** api/tests/batch_execution_integration.rs, api/tests/metrics_ws.rs
- **Commit:** 6c22b6fe9

## Open Question Resolved

**Open Q#1 (reset_session semantics):** `ReteEngine::reset()` (rete/mod.rs:525-532) clears BOTH `working_facts` AND `rules`. A `reset_session` that calls `engine.reset()` would leave the session with no rules loaded — the next call would fire nothing. Resolution: `reset_by_alias` rebuilds a FRESH engine via `registry.create_engine_by_name` + `load_ruleset` (re-reading the current shared ruleset snapshot), preserving the engine_name and engine_generation. This satisfies SC-4 (next call returns `score:10`, not `score:20` — empty WM but rules present).

## TDD Gate Compliance

- **RED gate (Task 1):** `test(27-01): add Wave 0 RED scaffolds for SESS-01 SC-1..SC-4` — commit 43f1d95d2. Four SC tests written and confirmed failing before implementation.
- **GREEN gate (Task 2 + Task 3):** `feat(27-01): implement SessionManager...` (39a15f72e) + `feat(27-01): wire execute_single sessionful branch...` (6c22b6fe9). All four SC tests now pass.
- **REFACTOR gate:** Not needed — no cleanup-only changes required.

## Test Results

- `cargo test -p api --test execute_integration`: **22 passed, 0 failed, 1 ignored** (4 new SC tests pass)
- `cargo test -p api`: **110 passed, 0 failed, 1 ignored** (no regressions)
- `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak`: **242 passed, 0 failed, 3 ignored**

## Known Stubs

None — all surfaces wired to real engine invocation. The v1.0 `engine::environment::Session` stub (environment.rs:152) is documented (D-01) but NOT used by the live execute path and NOT deleted (out of scope — other v1.0 code references it).

## Threat Flags

None — no security-relevant surface introduced beyond what the plan's threat model covered. The lifecycle routes inherit the Phase-22 JWT middleware wrap (registered on the /v1 scope); the per-session Mutex prevents cross-session WM contamination; the DashMap sharding prevents global-lock contention.

## Self-Check: PASSED

- All 8 created/modified files exist on disk.
- All 3 task commits (43f1d95d2, 39a15f72e, 6c22b6fe9) exist in git log.
- All 4 SC tests pass GREEN; api 110/0/1, engine 242/0/3.