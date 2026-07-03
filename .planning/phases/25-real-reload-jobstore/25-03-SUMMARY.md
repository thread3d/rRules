---
phase: 25-real-reload-jobstore
plan: 03
subsystem: api-execute-path
tags: [reload-handler, jobstore-transitions, batch-truthful-aggregation, execute-single-refactor, wave-2, keystone, nyquist-green]
requires:
  - "Phase 25 Plan 01 (SharedRuleSet wrapper + 8 Wave 0 RED tests)"
  - "Phase 25 Plan 02 (real .rules JSON parser + reload_and_swap helper)"
provides:
  - "Rewritten reload_rules handler accepting web::Data<Arc<RwLock<SharedRuleSet>>> + web::Data<JobStore> with sync + async branches both calling reload_and_swap"
  - "Async reload: Pending inserted BEFORE tokio::spawn (Pitfall 14), spawned task transitions Running→Completed/Failed in JobStore (L-03)"
  - "Sync reload: calls reload_and_swap inline (D-04 — both paths swap via one shared helper)"
  - "execute_single return type refactored to Result<ExecuteResponse, (StatusCode, String)> (R4)"
  - "execute_rules reconstructs HttpResponse from (StatusCode, String) Err"
  - "batch_execute_rules JoinSet task returns Result<ExecuteResponse, String>; three-arm collector Ok(Ok)/Ok(Err)/Err(JoinError) (D-03)"
  - "Removed: batch Err-mask placeholder (hardcoded zero-fired ExecuteResponse) and engine_state no-op from reload_rules (Pitfall 12)"
  - "All 7 Wave 0 RED tests flipped GREEN (Nyquist GREEN completion)"
affects:
  - "api/src/v1/execution.rs (reload_rules rewrite, execute_single R4 refactor, execute_rules wrapper, batch three-arm collector)"
  - "api/tests/execute_integration.rs (batch_truthful_aggregation_one_failing_item order-assumption fix)"
tech-stack:
  added: []
  patterns:
    - "Pitfall 14 mitigation: insert Pending row into JobStore synchronously BEFORE tokio::spawn so rapid polls see Pending, not 404"
    - "D-04 single-swap-helper: both sync and async reload branches call reload_and_swap — they cannot diverge"
    - "D-03 three-arm collector: Ok(Ok(resp))→succeeded, Ok(Err(msg))→failed+error, Err(JoinError)→failed (panic→JoinError→failed preserved)"
    - "R4 (StatusCode, String) Err: batch task maps Err((_, msg))→Err(msg) without consuming an HttpResponse body"
key-files:
  created: []
  modified:
    - "api/src/v1/execution.rs"
    - "api/tests/execute_integration.rs"
decisions:
  - "L-02: reload_rules no longer calls EngineState.reload_engine (no-op) — calls reload_and_swap on the shared SharedRuleSet (real build-then-swap)"
  - "L-03: async branch inserts Pending into JobStore before spawn; spawned task writes Running→Completed/Failed transitions"
  - "D-01: generation-accept — swap happens immediately under write lock; in-flight executes finish against their cloned snapshot (L-05 already clones under read lock + releases before execute)"
  - "D-04: sync reload ALSO calls reload_and_swap inline (one shared helper for both paths — a sync reload that didn't swap would be a lie)"
  - "D-03: execute_single Err is a per-item FAILURE (failed += 1, success:false, error:Some), NOT a masked zero-fired success"
  - "R4: execute_single returns Result<ExecuteResponse, (StatusCode, String)> so the batch task extracts the error string cleanly"
  - "L-06: panic→JoinError→failed path preserved (already correct, unchanged)"
metrics:
  duration: "~5 min"
  completed: 2026-07-03
  tasks: 2
  files: 2
status: complete
---

# Phase 25 Plan 03: Reload Handler Wiring + Batch Truthful Aggregation Summary

Wave 2 keystone plan — wired the `reload_rules` handler to `reload_and_swap` with sync + async branches and JobStore transitions (L-02, L-03, D-01, D-04, Pitfall 12/14), refactored `execute_single`'s return type to `Result<ExecuteResponse, (StatusCode, String)>` (R4), and fixed batch truthful aggregation with a three-arm collector (D-03, SC-2). All 7 Wave 0 RED tests flipped GREEN — Nyquist GREEN completion achieved.

## What Was Built

### Task 1: Rewrite reload_rules handler — sync + async via reload_and_swap + JobStore transitions

**Commit:** `cb96ee050`

- Replaced the `reload_rules` handler signature: removed `engine_state: web::Data<EngineState>`, added `shared: web::Data<Arc<std::sync::RwLock<SharedRuleSet>>>` + `job_store: web::Data<JobStore>` (Pitfall 12 — the legacy `EngineState::reload_engine` no-op is no longer called by `reload_rules`).
- **Async branch** (`req.async_reload == true`): generates `job_id`, inserts a `ReloadJob { status: ReloadStatus::Pending, .. }` into `job_store.write()` BEFORE `tokio::spawn` (Pitfall 14 — a rapid poll of `GET /v1/jobs/{job_id}` sees Pending, not 404). Clones `shared` + `job_store` (Arc clones) + `rules_dir` into the spawned task. The spawned task transitions Pending→Running, calls `reload_and_swap(&shared_clone, &dir).await`, then transitions Running→Completed (with `result: Some(ReloadResult)`) or Running→Failed (with `error: Some(e)`). Returns `HttpResponse::Accepted().json(JobResponse { job_id, status: "pending" })`.
- **Sync branch** (`req.async_reload == false`): calls `reload_and_swap(shared.get_ref(), &rules_dir).await` inline (D-04 — sync also swaps; one shared helper for both paths). On `Ok(result)` returns `ReloadResponse { reload_id, rules_reloaded: result.rule_count, status }`; on `Err(e)` returns 500 with the error string.
- Both paths swap the whole `SharedRuleSet` + bump generation via `reload_and_swap` (L-02, L-04, D-01). `GET /v1/jobs/{job_id}` now returns real Pending/Running/Completed/Failed transitions (L-03, SC-1).

**Tests flipped GREEN by Task 1:** `reload_then_execute_against_refreshed_rules` (SC-1 keystone), `async_reload_job_transitions` (SC-1 job transitions), `engine_generation_increments_after_reload` (SC-3), `sync_reload_swaps_and_bumps_generation` (D-04), `in_flight_run_uses_old_snapshot` (D-01), `reload_empty_dir_warns_not_crashes` (D-03a — `reload_and_swap` warns+empty on missing dir, returns Ok with 0 rules, NOT 500).

### Task 2: execute_single R4 refactor + batch three-arm collector (D-03, SC-2, L-06)

**Commit:** `10bbde654`

- **R4:** Refactored `execute_single` return type from `Result<ExecuteResponse, HttpResponse>` to `Result<ExecuteResponse, (actix_web::http::StatusCode, String)>`. Converted all 6 `Err(HttpResponse::BadRequest().json(...))` / `Err(HttpResponse::InternalServerError().json(...))` returns to `Err((StatusCode::BAD_REQUEST, msg))` / `Err((StatusCode::INTERNAL_SERVER_ERROR, msg))` — extracting the inner format string as the `msg` (unknown engine, engine creation failed, ruleset lock poisoned, load_ruleset failed, insert_fact failed, engine execution panicked).
- Updated `execute_rules` wrapper to reconstruct the HttpResponse: `match execute_single(...).await { Ok(resp) => HttpResponse::Ok().json(resp), Err((code, msg)) => HttpResponse::build(code).json(serde_json::json!({ "error": msg })) }` — preserves the existing 400/500 response shapes for the single-execute path.
- **D-03:** Rewrote the `batch_execute_rules` JoinSet task to return `Result<ExecuteResponse, String>` via `execute_single(...).await.map_err(|(_code, msg)| msg)` — the task calls execute_single, `Ok(resp)` stays `Ok(resp)`, `Err((_, msg))` becomes `Err(msg)`. REMOVED the entire Err-mask placeholder branch (the `match ... { Ok(resp) => resp, Err(_) => ExecuteResponse { ... hardcoded zero ... } }` with `engine_generation: 0u64`).
- **Three-arm collector:** `Ok(Ok(resp))` → `total_fired += resp.rules_fired; succeeded += 1; success:true, response:Some(resp), error:None`; `Ok(Err(msg))` → `failed += 1; success:false, response:None, error:Some(msg)` (the D-03 fix — Err is a per-item FAILURE, not a zero-fired success); `Err(join_err)` → `failed += 1; success:false, response:None, error:Some(join_err.to_string())` (L-06 — panic→JoinError→failed, preserved).

**Tests flipped GREEN by Task 2:** `batch_truthful_aggregation_one_failing_item` (SC-2 — succeeded==1, failed==1, total_fired>=1, failing item has success:false + error:Some).

## Verification Results

- `cargo build -p api` — clean (exit 0)
- `cargo test -p api` — 103 passed, 0 failed, 1 ignored (`batch_panicking_item_isolated` — intentionally `#[ignore]`d; passes when run with `--include-ignored`)
- `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` — 241 passed, 0 failed, 3 ignored (pre-existing)
- `cargo test -p api --test execute_integration` — 15 passed, 0 failed, 1 ignored (all 7 Wave 0 RED tests flipped GREEN + 8 pre-existing tests still GREEN)

### Wave 0 RED test status after 25-03 (Nyquist GREEN completion)

| # | Test | Status | How it turned GREEN |
|---|------|--------|---------------------|
| 1 | `reload_then_execute_against_refreshed_rules` | GREEN | reload_rules async branch inserts Pending→spawn→Running→Completed; post-reload execute sees score 20 + gen > 0 (SC-1 keystone) |
| 2 | `async_reload_job_transitions` | GREEN | JobStore receives Pending (before spawn) + Running + Completed transitions (L-03, Pitfall 14) |
| 3 | `engine_generation_increments_after_reload` | GREEN | reload_and_swap bumps generation; sync reload path stamps new gen into ExecuteResponse (SC-3, D-04) |
| 4 | `batch_truthful_aggregation_one_failing_item` | GREEN | three-arm collector: unknown-engine Err → failed += 1, success:false, error:Some (SC-2, D-03) |
| 5 | `in_flight_run_uses_old_snapshot` | GREEN | sync reload swaps SharedRuleSet; post-reload execute sees new snapshot (score 20, gen > 0) (D-01) |
| 6 | `batch_panicking_item_isolated` | IGNORED | `#[ignore]`d — deterministic panic trigger requires a test fixture that panics inside the JoinSet task (out of scope; the L-06 panic→JoinError→failed collector arm is preserved and correct). Passes with `--include-ignored` (2 valid items → succeeded >= 1). |
| 7 | `reload_empty_dir_warns_not_crashes` | GREEN | reload_and_swap calls build_ruleset_from_dir which warns+empty on missing dir, returns Ok(0 rules) → 200, NOT 500 (D-03a) |
| 8 | `sync_reload_swaps_and_bumps_generation` | GREEN | sync branch calls reload_and_swap inline; post-reload execute sees score 20 + gen > 0 (D-04) |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed test order assumption in `batch_truthful_aggregation_one_failing_item`**
- **Found during:** Task 2 (running the batch test after the three-arm collector refactor)
- **Issue:** The Wave 0 test asserted `let failing = &results[1]` and `let ok_item = &results[0]` — assuming JoinSet returns tasks in submission order. In reality, `join_next()` returns tasks in **completion order**. The unknown-engine item (instant Err — no engine construction, no web::block) completes before the valid item, so the failing item ends up at `results[0]`, not `results[1]`. After the D-03 refactor (Err → failed, no masking), the test's hardcoded index assumption caused `assert_eq!(failing["success"].as_bool(), Some(false))` to fail because `results[1]` was the SUCCESS item.
- **Fix:** Changed the test to find the failing/successful items by their `success` flag (`results.iter().find(|r| r["success"].as_bool() == Some(false))`) rather than assuming a fixed index. This is robust to completion-order delivery and aligns with the test's intent (one succeeds, one fails, failing has success:false + error:Some). The test's substantive assertions (succeeded==1, failed==1, total_fired>=1, failing success:false + error:Some, ok success:true) are unchanged.
- **Files modified:** `api/tests/execute_integration.rs`
- **Commit:** `10bbde654`

### Documentation Discrepancies (not code deviations)

**1. Plan acceptance criterion for `web::Data<JobStore>` count**
- **Plan says:** `grep -c 'web::Data<JobStore>' api/src/v1/execution.rs` returns 1 (reload_rules handler param)
- **Actual:** 3 — `reload_rules` (added by this plan) + `get_job_status` (pre-existing, line 903) + `list_jobs` (pre-existing, line 926). The plan's criterion didn't account for the two pre-existing handlers that already extract `web::Data<JobStore>`. The code is correct; the criterion is inaccurate.

**2. Plan acceptance criterion `-x` flag**
- **Plan says:** `cargo test -p api --test execute_integration -x`
- **Actual:** `-x` is not a valid `cargo test` flag. The intended flag is `--include-ignored` (to run `#[ignore]`d tests). All non-ignored tests pass; the one `#[ignore]`d test (`batch_panicking_item_isolated`) passes when run with `--include-ignored`. The Nyquist GREEN completion (0 failed) is achieved.

## Known Stubs

None — all stubs tracked in Plan 25-01 and 25-02 are resolved by this plan:

| File | Line (approx) | Description | Resolved By |
|------|---------------|-------------|-------------|
| `api/src/v1/execution.rs` | ~700 (was) | `reload_rules` handler used `EngineState` (no-op reload) | **RESOLVED** — Task 1 rewired to `reload_and_swap` + JobStore |
| `api/src/v1/execution.rs` | ~626 (was) | `engine_generation: 0u64` in batch Err-mask placeholder | **RESOLVED** — Task 2 removed the placeholder (D-03 three-arm collector) |
| `api/src/v1/execution.rs` | ~183 (was) | `EngineState::reload_engine` no-op called from reload_rules | **RESOLVED** — reload_rules no longer calls it (Pitfall 12 closed). The `EngineState` struct + `reload_engine` method remain as dead code (deletion at discretion per plan; no handler reads it). |

## Threat Flags

None — no new security-relevant surface introduced beyond what the plan's `<threat_model>` already covers. T-25-04 (error string disclosure — accept), T-25-05 (JobStore in-memory — accept), T-25-06 (spawned reload task DoS — mitigate via single whole-value swap), T-25-SC (no new packages — mitigate). All dispositions honored.

## TDD Gate Compliance

- **RED gate:** The 8 Wave 0 RED tests from Plan 25-01 serve as the RED gate. After 25-02, 7 remained RED (all depended on 25-03 handler wiring) and 1 remained `#[ignore]`d.
- **GREEN gate:** `10bbde654` (`fix(25-03): batch truthful aggregation...`) — all 7 RED tests flipped GREEN, 1 `#[ignore]`d test passes with `--include-ignored`. Full `cargo test -p api` is GREEN (0 failed). Nyquist GREEN completion achieved.
- **Note:** Both Task 1 (`feat`) and Task 2 (`fix`) commits build cleanly (`cargo build -p api` succeeds). The RED tests failed at runtime on substantive assertions (handler not rewired, batch Err masked as success), not at compile time.

## Self-Check: PASSED

- `api/src/v1/execution.rs` — FOUND (rewritten reload_rules with sync+async+JobStore transitions, refactored execute_single return type, three-arm batch collector)
- `api/tests/execute_integration.rs` — FOUND (batch_truthful_aggregation_one_failing_item order-assumption fix)
- Commit `cb96ee050` — FOUND in git log (Task 1)
- Commit `10bbde654` — FOUND in git log (Task 2)
- `cargo test -p api` — 103 passed, 0 failed, 1 ignored (Nyquist GREEN)
- `cargo test -p engine --features all` — 241 passed, 0 failed, 3 ignored (no regression)