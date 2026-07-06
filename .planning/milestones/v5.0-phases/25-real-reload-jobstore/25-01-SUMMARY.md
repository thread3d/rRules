---
phase: 25-real-reload-jobstore
plan: 01
subsystem: api-execute-path
tags: [shared-ruleset, generation-tagging, wave-0-red, shared-rule-set, type-migration]
requires:
  - "Phase 23 (D-03 shared Arc<RwLock<RuleSet>> scaffold)"
  - "Phase 24 (ExecuteResponse.engine_generation field)"
provides:
  - "pub struct SharedRuleSet { pub ruleset: RuleSet, pub generation: u64 } (D-02)"
  - "Migrated execute_single/execute_rules/batch_execute_rules signatures to Arc<RwLock<SharedRuleSet>>"
  - "execute_single stamps guard.generation into ExecuteResponse.engine_generation (L-04)"
  - "main.rs constructs SharedRuleSet::new(build_ruleset_from_dir(...)) inside Arc<RwLock> (L-01)"
  - "pub type JobStore (made pub for test access)"
  - "8 Wave 0 RED integration test scaffolds for SC-1/SC-2/SC-3 + edge cases"
  - "write_rule_file / shared_job_store / shared_engine_state test helpers"
  - "Extended configure_test_routes with reload/batch-execute/jobs routes"
affects:
  - "api/src/v1/execution.rs (SharedRuleSet struct, handler signatures, generation stamping, JobStore pub)"
  - "api/src/main.rs (SharedRuleSet import, shared state construction)"
  - "api/tests/execute_integration.rs (shared_ruleset helper, init_test_app macro, configure_test_routes, 8 RED tests)"
  - "api/tests/batch_execution_integration.rs (shared_ruleset helper migrated to SharedRuleSet)"
  - "api/tests/metrics_ws.rs (shared_ruleset helper migrated to SharedRuleSet)"
tech-stack:
  added: []
  patterns:
    - "SharedRuleSet wrapper inside existing Arc<RwLock> — generation read atomically with ruleset under one read lock (D-02)"
    - "Per-request isolation preserved — clone ruleset + copy u64 generation under read lock, release before web::block (L-05)"
    - "Wave 0 RED→GREEN Nyquist pattern — 8 tests compile against final SharedRuleSet shape, fail on substantive assertions"
key-files:
  created: []
  modified:
    - "api/src/v1/execution.rs"
    - "api/src/main.rs"
    - "api/tests/execute_integration.rs"
    - "api/tests/batch_execution_integration.rs"
    - "api/tests/metrics_ws.rs"
decisions:
  - "D-02: SharedRuleSet { ruleset, generation: u64 } inside existing Arc<RwLock> — NOT a separate AtomicU64"
  - "L-04: execute_single stamps guard.generation (always 0 pre-reload) into ExecuteResponse.engine_generation"
  - "L-05: read lock window unchanged — clone ruleset via load_ruleset + copy u64 generation, release before web::block"
  - "L-01: Arc<RwLock<SharedRuleSet>> pattern preserved — no ArcSwap dependency"
  - "JobStore type alias made pub so test helpers can install it for reload/jobs route wiring"
  - "batch_panicking_item_isolated #[ignore]d — deterministic panic trigger requires Plan 25-03's Err→failed refactor"
  - "Installed shared_engine_state() in init_test_app! so RED reload tests fail on substantive assertions (job 404, no swap, gen 0) not 500 missing-extractor"
metrics:
  duration: "~20 min"
  completed: 2026-07-03
  tasks: 2
  files: 5
status: complete
---

# Phase 25 Plan 01: SharedRuleSet Wrapper + Wave 0 RED Tests Summary

Introduced the `SharedRuleSet { ruleset, generation }` wrapper (D-02) inside the existing `Arc<RwLock<...>>`, migrated all three execute-path handler signatures, stamped `engine_generation` from the shared snapshot (L-04), and landed 8 Wave 0 RED integration test scaffolds encoding SC-1/SC-2/SC-3 and edge cases.

## What Was Built

### Task 1: SharedRuleSet struct + execute-path handler signature migration

**Commit:** `09aa102cd`

- Added `pub struct SharedRuleSet { pub ruleset: RuleSet, pub generation: u64 }` with `#[derive(Debug, Clone)]` and `impl SharedRuleSet { pub fn new(ruleset: RuleSet) -> Self }` (generation starts at 0) to `api/src/v1/execution.rs`, positioned after `ExecuteResponse`.
- Migrated `execute_single`, `execute_rules`, and `batch_execute_rules` parameter types from `web::Data<Arc<std::sync::RwLock<RuleSet>>>` to `web::Data<Arc<std::sync::RwLock<SharedRuleSet>>>`.
- Updated `execute_single`'s read-lock block: reads `guard.ruleset` for `load_ruleset` + rule ids AND copies `guard.generation` (a `u64`) atomically under the single read lock, then releases before `web::block` (L-05 per-request isolation preserved).
- Replaced the hardcoded `engine_generation: 0u64` at the `Ok(ExecuteResponse)` construction with `engine_generation: generation` (L-04). The batch Err-mask placeholder at `:626` remains `0u64` (Plan 25-03 scope).
- Made `JobStore` type alias `pub` so test helpers can install it.
- Updated `api/src/main.rs` to import `SharedRuleSet` and wrap `build_ruleset_from_dir` result in `SharedRuleSet::new(...)` inside the existing `Arc<RwLock<...>>` (L-01 preserved — no ArcSwap).
- Updated `shared_ruleset()` helpers in `execute_integration.rs`, `batch_execution_integration.rs`, and `metrics_ws.rs` to build `SharedRuleSet::new(ruleset)`.

### Task 2: Eight Wave 0 RED test scaffolds + helpers + extended test routes

**Commit:** `2065ec7be`

- **`write_rule_file(dir, name, rule) -> PathBuf`** — serializes a `Rule` to JSON via `serde_json::to_string_pretty` and writes to `{dir}/{name}.rules`.
- **`score_plus_20_rule()`** — the +20 keystone variant for the SC-1 edit scenario.
- **`shared_job_store()`** — returns `web::Data<JobStore>` via `init_job_store()`.
- **`shared_engine_state()`** — returns `web::Data<EngineState>` so the current `reload_rules` handler (pre-25-03) resolves its extractor.
- **Extended `configure_test_routes`** with `/rules/batch-execute` (POST), `/rules/reload` (POST), `/jobs/{job_id}` (GET), `/jobs` (GET).
- **Updated `init_test_app!`** to install `shared_job_store()` + `shared_engine_state()`.
- **Updated `reset_test_env`** to reset `RULE_STORAGE_PATH` to `"./rules"` (prevents env var leak between serial tests).
- **`create_test_rules_dir()`** — creates a unique temp dir via `uuid::Uuid::new_v4()`.

**Eight RED test scaffolds:**

| # | Test | SC/Req | RED Reason |
|---|------|--------|------------|
| 1 | `reload_then_execute_against_refreshed_rules` | SC-1 / DEPLOY-05 | reload no-op, job 404s, score stays 10, gen stays 0 |
| 2 | `async_reload_job_transitions` | SC-1 / DEPLOY-05 | JobStore never written (all polls 404) |
| 3 | `engine_generation_increments_after_reload` | SC-3 / L-04 | gen stays 0 after reload |
| 4 | `batch_truthful_aggregation_one_failing_item` | EXEC-03 / SC-2 | Err masked as success (succeeded==2, not 1) |
| 5 | `in_flight_run_uses_old_snapshot` | L-05 / D-01 | reload doesn't swap (score stays 10, gen stays 0) |
| 6 | `batch_panicking_item_isolated` | SC-2 | `#[ignore]`d — needs 25-03 deterministic panic trigger |
| 7 | `reload_empty_dir_warns_not_crashes` | D-03a | missing dir → 500 (not graceful) |
| 8 | `sync_reload_swaps_and_bumps_generation` | DEPLOY-05 / D-04 | sync reload doesn't swap (score stays 10, gen stays 0) |

## Verification Results

- `cargo build -p api` — clean (exit 0)
- `cargo check -p api` — clean (exit 0)
- `cargo test -p api --test execute_integration --no-run` — compiles (exit 0)
- `cargo test -p api --test execute_integration` — 8 pre-existing tests GREEN, 7 new tests RED (assertion failures), 1 `#[ignore]`d
- `cargo test -p api --test batch_execution_integration` — 5 tests GREEN (no regression from SharedRuleSet migration)
- `cargo test -p api --test metrics_ws` — 4 tests GREEN (no regression from SharedRuleSet migration)

All 7 RED tests fail on substantive assertions (job 404, gen stays 0, score stays 10, succeeded==2, 500 on empty dir) — NOT on compile or extraction errors. The `#[ignore]`d test compiles and is scaffolded for Plan 25-03.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated `shared_ruleset()` in `batch_execution_integration.rs` and `metrics_ws.rs`**
- **Found during:** Task 1 (type migration)
- **Issue:** The plan only mentions updating `shared_ruleset()` in `execute_integration.rs`, but `batch_execution_integration.rs:88` and `metrics_ws.rs:101` also have `shared_ruleset()` returning `web::Data<Arc<RwLock<RuleSet>>>` and call `execute_rules` / `batch_execute_rules` whose signatures changed to `SharedRuleSet`. Without updating these, `cargo build` would fail.
- **Fix:** Updated both helpers to wrap `SharedRuleSet::new(ruleset)` — same pattern as `execute_integration.rs`.
- **Files modified:** `api/tests/batch_execution_integration.rs`, `api/tests/metrics_ws.rs`
- **Commit:** `09aa102cd`

**2. [Rule 2 - Missing functionality] Added `shared_engine_state()` helper to `init_test_app!`**
- **Found during:** Task 2 (RED test scaffolding)
- **Issue:** The plan says to install `web::Data<JobStore>` in `init_test_app!` but the current `reload_rules` handler extracts `web::Data<EngineState>` (not `JobStore` — that's 25-03's rewrite). Without `EngineState` installed, POST `/v1/rules/reload` returns 500 (missing extractor), causing the RED tests to fail on extraction errors rather than substantive assertions.
- **Fix:** Added `shared_engine_state()` helper and installed it in `init_test_app!` alongside `shared_job_store()`. This makes the RED tests fail meaningfully (reload returns 202/200 but job 404s / no swap / gen 0) rather than on a 500 missing-extractor error. Harmless for pre-existing tests (actix silently ignores unused `app_data`).
- **Files modified:** `api/tests/execute_integration.rs`
- **Commit:** `2065ec7be`

**3. [Rule 1 - Bug] Inlined job polling loop instead of generic helper function**
- **Found during:** Task 2 (test compilation)
- **Issue:** The `poll_job_statuses` helper function couldn't accept the `impl Service` type returned by `test::init_service` — the trait bounds `S: Service<ServiceRequest, Response = ServiceResponse<B>>` were not satisfiable because `init_service` returns a service for `actix_http::Request` (not `ServiceRequest`) with an unnameable body type.
- **Fix:** Inlined the polling loop directly in the 2 tests that need it (`reload_then_execute_against_refreshed_rules` and `async_reload_job_transitions`), using `test::call_service` which is generic over the request type and works in the test body scope.
- **Files modified:** `api/tests/execute_integration.rs`
- **Commit:** `2065ec7be`

## Known Stubs

| File | Line | Description | Resolved By |
|------|------|-------------|-------------|
| `api/src/v1/execution.rs` | ~626 | `engine_generation: 0u64` in batch Err-mask placeholder | Plan 25-03 Task 2 (D-03 truthful Err→failed aggregation) |
| `api/src/main.rs` | 44-71 | `build_ruleset_from_dir` returns empty `RuleSet` | Plan 25-02 Task 1 (real .rules JSON parser) |
| `api/src/v1/execution.rs` | 679-715 | `reload_rules` uses `EngineState` (no-op reload) | Plan 25-03 (rewire to SharedRuleSet + JobStore + reload_and_swap) |

These stubs are intentional and documented in the plan as deferred to 25-02/25-03. They do NOT prevent this plan's goal (SharedRuleSet type migration + Wave 0 RED scaffolds) from being achieved.

## TDD Gate Compliance

- **RED gate commit:** `2065ec7be` (`test(25-01): add 8 Wave 0 RED integration tests`) — 7 tests fail on assertions, 1 `#[ignore]`d.
- **GREEN gate commit:** N/A — this plan is Wave 0 (type migration + RED scaffolds only). The GREEN commits land in Plans 25-02 (real parser + reload_and_swap) and 25-03 (handler wiring + batch aggregation).
- **Note:** Both Task 1 (`feat`) and Task 2 (`test`) commits build cleanly (`cargo build -p api` and `cargo test --no-run` succeed). The RED tests fail at runtime, not at compile time.

## Self-Check: PASSED

- `api/src/v1/execution.rs` — FOUND (SharedRuleSet struct, impl, migrated signatures, generation stamping)
- `api/src/main.rs` — FOUND (SharedRuleSet import, construction)
- `api/tests/execute_integration.rs` — FOUND (8 new tests, write_rule_file, shared_job_store, extended routes)
- `api/tests/batch_execution_integration.rs` — FOUND (SharedRuleSet migration)
- `api/tests/metrics_ws.rs` — FOUND (SharedRuleSet migration)
- Commit `09aa102cd` — FOUND in git log
- Commit `2065ec7be` — FOUND in git log