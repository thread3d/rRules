---
phase: 25-real-reload-jobstore
verified: 2026-07-03T18:30:00Z
status: passed
score: 17/17 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification:
  previous_status: none
  previous_score: N/A
  gaps_closed: []
  gaps_remaining: []
  regressions: []
---

# Phase 25: Real reload + JobStore Verification Report

**Phase Goal:** Async reload-then-execute actually swaps the compiled rule set so subsequent executes run refreshed rules, and `GET /v1/jobs/{id}` returns real reload job status.
**Verified:** 2026-07-03T18:30:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

The phase goal is observably achieved in the codebase. The keystone SC-1 scenario (`reload_then_execute_against_refreshed_rules`) is encoded as an integration test that: edits a rule (+10 → +20), POSTs `/v1/rules/reload {async:true}` → 202 + `job_id`, polls `GET /v1/jobs/{job_id}` through `pending/running → completed`, then POSTs `/v1/execute` and asserts `facts_after.scorecard.score == 20` (edited rule) AND `engine_generation > 0`. The test passes. All 8 Wave 0 RED tests flipped GREEN (Nyquist GREEN completion). Full `cargo test -p api` returns 103 passed, 0 failed, 1 ignored (the ignored test passes with `--include-ignored`).

### Observable Truths

Must-haves merged from ROADMAP success criteria (3) + Plan 25-01 truths (6) + Plan 25-02 truths (6) + Plan 25-03 truths (8) = 17 truths (deduplicated by intent where roadmap SCs restate plan truths; the roadmap SC wording is retained as the contract).

| # | Truth | Status | Evidence |
| --- | --- | --- | --- |
| 1 | SC-1: After rule edit + `reload_rules({async:true})` returns 202, polling `GET /v1/jobs/{id}` transitions Pending→Running→Completed, and next `/v1/execute` reflects the edited rule | ✓ VERIFIED | `reload_then_execute_against_refreshed_rules` test passes (`api/tests/execute_integration.rs:606`); asserts score 20 + gen>0 after reload. Also `async_reload_job_transitions` test passes (`:723`) |
| 2 | SC-2: `POST /v1/rules/batch-execute` with N items returns `total_fired`/`succeeded`/`failed` consistent with real execution; one panicking item does not poison the others | ✓ VERIFIED | `batch_truthful_aggregation_one_failing_item` test passes (`:865`) — succeeded==1, failed==1, total_fired>=1, failing item success:false+error:Some. `batch_panicking_item_isolated` test passes with `--include-ignored` (`:1017`) — succeeded>=1, no poisoning |
| 3 | SC-3: `ExecuteResponse.engine_generation` increments after a reload so callers can detect they ran against a prior rule set | ✓ VERIFIED | `engine_generation_increments_after_reload` test passes (`:797`); `execute_single` stamps `guard.generation` into `ExecuteResponse.engine_generation` (`api/src/v1/execution.rs:708`); `reload_and_swap` bumps via `generation.wrapping_add(1)` (`:355`) |
| 4 | SharedRuleSet `{ ruleset: RuleSet, generation: u64 }` exists in `api::v1::execution` with `SharedRuleSet::new(ruleset)` constructor (generation 0) (D-02) | ✓ VERIFIED | `api/src/v1/execution.rs:83-95` — `pub struct SharedRuleSet { pub ruleset: RuleSet, pub generation: u64 }` + `impl SharedRuleSet { pub fn new(ruleset) -> Self { generation: 0 } }`. Inside existing `Arc<RwLock<...>>` (NOT a separate AtomicU64) — D-02 honored |
| 5 | `execute_single`, `execute_rules`, `batch_execute_rules` accept `web::Data<Arc<RwLock<SharedRuleSet>>>` and read `guard.ruleset` for load_ruleset + rule ids | ✓ VERIFIED | `api/src/v1/execution.rs:507`, `:727`, `:759` — all three handlers take `web::Data<Arc<std::sync::RwLock<SharedRuleSet>>>`; `execute_single` reads `guard.ruleset` at `:571-573` |
| 6 | `execute_single` stamps `guard.generation` into `ExecuteResponse.engine_generation` (L-04) — replacing the hardcoded zero | ✓ VERIFIED | `api/src/v1/execution.rs:559-575` — reads `(ruleset, generation)` atomically under read lock, copies `guard.generation` at `:573`; stamps at `:708` `engine_generation: generation`. No `engine_generation: 0u64` literal remains (grep clean) |
| 7 | `main.rs` constructs `SharedRuleSet::new(build_ruleset_from_dir(&rules_dir))` inside `Arc<RwLock<...>>` (L-01 preserved) | ✓ VERIFIED | `api/src/main.rs:90-91` — `let ruleset = SharedRuleSet::new(v1::execution::build_ruleset_from_dir(&rules_dir)); let ruleset = web::Data::new(std::sync::Arc::new(std::sync::RwLock::new(ruleset)));`. No ArcSwap dependency |
| 8 | Eight Wave 0 RED tests exist in `api/tests/execute_integration.rs` covering SC-1, SC-2, SC-3, job transitions, sync-swap, empty-dir, in-flight isolation, panicking-item isolation | ✓ VERIFIED | All 8 test functions present at lines 606, 723, 797, 865, 948, 1017, 1057, 1095. All 7 non-ignored tests PASS; the 8th (`batch_panicking_item_isolated`, `#[ignore]`d) PASSES with `--include-ignored` |
| 9 | All pre-existing Phase 23/24 integration tests remain GREEN after the SharedRuleSet type migration | ✓ VERIFIED | `cargo test -p api` → 103 passed, 0 failed, 1 ignored. No regressions in `batch_execution_integration` (5 passed), `metrics_ws` (4 passed), `execute_integration` pre-existing tests |
| 10 | `build_ruleset_from_dir` parses `.rules` JSON files via `serde_json::from_str` and returns a non-empty RuleSet when files exist (R1/D-05) | ✓ VERIFIED | `api/src/v1/execution.rs:116-189` — pub fn, iterates `read_dir`, for `.rules`/`.drl` files calls `serde_json::from_str::<Rule>` then falls back to `serde_json::from_str::<Vec<Rule>>`, calls `ruleset.add_rule(rule)`. Keystone test relies on this: pre-reload score==10 proves the +10 rule parsed |
| 11 | `build_ruleset_from_dir` is pub in `api::v1::execution`, accessible from `main.rs` and tests | ✓ VERIFIED | `api/src/v1/execution.rs:116` — `pub fn build_ruleset_from_dir(rules_dir: &str) -> RuleSet`; `main.rs:90` calls `v1::execution::build_ruleset_from_dir(&rules_dir)` |
| 12 | `build_ruleset_from_dir` warns and returns empty RuleSet on missing/empty dir or unparseable files (D-03a — no crash) | ✓ VERIFIED | `api/src/v1/execution.rs:118-124` (missing dir → warn + empty), `:126-135` (failed read_dir → warn + empty), `:158-167` (unparseable → warn + skip), `:181-187` (empty result → warn). `reload_empty_dir_warns_not_crashes` test passes (`:1057`) |
| 13 | `reload_and_swap(&shared, &rules_dir) -> Result<ReloadResult, String>` builds new RuleSet off-to-side, swaps whole `SharedRuleSet` under write lock with `generation.wrapping_add(1)` (D-04/D-02/L-01) | ✓ VERIFIED | `api/src/v1/execution.rs:339-371` — `pub async fn reload_and_swap(...)`. Builds via `build_ruleset_from_dir` BEFORE lock (`:348`); acquires write lock only for `*guard = SharedRuleSet { ruleset, generation: next_gen }` (`:359-362`); `next_gen = guard.generation.wrapping_add(1)` (`:355`); guard dropped implicitly after assignment |
| 14 | `reload_and_swap` holds the write lock only for the single whole-value assignment — no I/O or parsing under the lock (Pitfall 4) | ✓ VERIFIED | `api/src/v1/execution.rs:348` (build off-side, no lock) → `:352` (acquire write) → `:359-362` (single assignment) → implicit drop. No `read_dir`/`read_to_string`/`serde_json` calls between `:352` and `:362` |
| 15 | `POST /v1/rules/reload {async:true}` returns 202 + `job_id`; spawned task inserts Pending BEFORE spawn, transitions Running→Completed/Failed in JobStore (L-03, Pitfall 14) | ✓ VERIFIED | `api/src/v1/execution.rs:883-905` — Pending inserted into `job_store.write()` BEFORE `tokio::spawn` (`:889-902` insert block, `:907` spawn). Spawned task transitions Pending→Running (`:912-920`), then Running→Completed with `result:Some` (`:928-935`) or Running→Failed with `error:Some` (`:937-944`). Returns `HttpResponse::Accepted().json(JobResponse { job_id, status: "pending" })` (`:947-950`) |
| 16 | `GET /v1/jobs/{job_id}` returns real ReloadJob transitions pending→running→completed (no more 404) | ✓ VERIFIED | `get_job_status` handler at `api/src/v1/execution.rs:1009-1028` reads from `JobStore`. `async_reload_job_transitions` test passes — observes `pending`/`running` followed by `completed` |
| 17 | `POST /v1/rules/reload {async:false}` calls `reload_and_swap` inline and returns ReloadResponse with real rule_count (D-04 sync-also-swaps) | ✓ VERIFIED | `api/src/v1/execution.rs:952-965` — sync branch calls `reload_and_swap(shared.get_ref(), &rules_dir).await`, returns `ReloadResponse { rules_reloaded: result.rule_count, ... }`. `sync_reload_swaps_and_bumps_generation` test passes (`:1095`) |
| 18 | After reload, `/v1/execute` reflects edited rule AND `engine_generation > pre-reload generation` (SC-1/SC-3/L-04/D-01) | ✓ VERIFIED | Keystone test `reload_then_execute_against_refreshed_rules` asserts `score_after == 20` AND `gen_after > 0` (`:712-723`). `engine_generation_increments_after_reload` test passes |
| 19 | Batch JoinSet task returns `Result<ExecuteResponse, String>`; `Ok(Err)` → failed+error, `Ok(Ok)` → succeeded (D-03 — NOT zero-fired-success mask) | ✓ VERIFIED | `api/src/v1/execution.rs:800-805` — task calls `execute_single(...).map_err(|(_code, msg)| msg)` returning `Result<ExecuteResponse, String>`. Three-arm collector at `:821-847`: `Ok(Ok(resp))`→succeeded, `Ok(Err(msg))`→failed+error:Some, `Err(JoinError)`→failed. The hardcoded zero-fired placeholder is GONE (no `engine_generation: 0u64` literal remains) |
| 20 | Batch with one failing item reports `succeeded == N-1`, `failed == 1`, `total_fired` reflecting only successful items (SC-2) | ✓ VERIFIED | `batch_truthful_aggregation_one_failing_item` test passes (`:865`) — asserts succeeded==1, failed==1, total_fired>=1 for a 2-item batch with one `engine:"unknown"` |
| 21 | Panicking batch item becomes JoinError → `failed += 1` (L-06 preserved) | ✓ VERIFIED | `api/src/v1/execution.rs:841-847` — `Err(join_err) => { failed += 1; ... error: Some(join_err.to_string()) }`. `batch_panicking_item_isolated` test passes with `--include-ignored` |
| 22 | `execute_single` returns `Result<ExecuteResponse, (StatusCode, String)>` so batch task extracts error string cleanly (R4) | ✓ VERIFIED | `api/src/v1/execution.rs:511` — `-> Result<ExecuteResponse, (actix_web::http::StatusCode, String)>`. All 6 Err sites return `Err((StatusCode::BAD_REQUEST|INTERNAL_SERVER_ERROR, msg))` (`:528, :536, :545, :567, :573, :584`). `execute_rules` wrapper reconstructs HttpResponse at `:731-734` |
| 23 | `execute_single` reads `(ruleset, generation)` atomically under single read lock, clones ruleset via `load_ruleset`, copies generation, releases guard before `web::block` (L-05 per-request isolation) | ✓ VERIFIED | `api/src/v1/execution.rs:559-575` — `(all_rule_ids, generation): (Vec<String>, u64)` block: acquires `ruleset.read()`, calls `engine.load_ruleset(&guard.ruleset)`, copies `guard.generation` into `gen`, drops guard at block end. `web::block` call is after this block (L-05 honored) |
| 24 | `reload_and_swap` is ONE shared helper called by both sync (inline) and async (spawned task) reload paths (D-04 — cannot diverge) | ✓ VERIFIED | Sync branch: `:955` `reload_and_swap(shared.get_ref(), &rules_dir).await`. Async spawned task: `:925` `reload_and_swap(&shared_clone, &dir_clone).await`. Same function, both call sites |
| 25 | `reload_rules` async branch inserts Pending BEFORE `tokio::spawn` (Pitfall 14 — rapid poll sees Pending not 404) | ✓ VERIFIED | `api/src/v1/execution.rs:889-902` — `job_store.write()` insert block completes (with explicit `}` drop) BEFORE `tokio::spawn` at `:907`. Test `async_reload_job_transitions` polls and observes pending/running before completed |
| 26 | `main.rs` shared-state construction calls `v1::execution::build_ruleset_from_dir` (single source of truth for startup + reload — D-05) | ✓ VERIFIED | `api/src/main.rs:90` — `SharedRuleSet::new(v1::execution::build_ruleset_from_dir(&rules_dir))`. Local `build_ruleset_from_dir` in main.rs is removed (now in `v1::execution`) |

**Score:** 26/26 truths verified (0 present-behavior-unverified, 0 failed). Note: the table above lists 26 rows because some roadmap SCs map to multiple plan truths; deduplication by intent yields 17 distinct must-haves, all verified. The roadmap's 3 Success Criteria (SC-1/SC-2/SC-3) are all VERIFIED with passing behavioral tests.

### Required Artifacts

| Artifact | Expected | Status | Details |
| --- | --- | --- | --- |
| `api/src/v1/execution.rs` — `SharedRuleSet` struct | pub struct with ruleset + generation + new() | ✓ VERIFIED | `:83-95` — pub struct, pub fields, pub fn new |
| `api/src/v1/execution.rs` — `build_ruleset_from_dir` pub fn | Real serde_json parser, pub, D-03a warn+continue | ✓ VERIFIED | `:116-189` — pub, serde_json::from_str, warn+skip on all error paths |
| `api/src/v1/execution.rs` — `reload_and_swap` pub async fn | Off-to-side build + single whole-value swap under write lock | ✓ VERIFIED | `:339-371` — pub async fn, build before lock, `*guard = SharedRuleSet{...}` single assignment |
| `api/src/v1/execution.rs` — `reload_rules` handler rewrite | sync + async branches both calling reload_and_swap + JobStore transitions | ✓ VERIFIED | `:872-967` — async inserts Pending before spawn, transitions Running→Completed/Failed; sync calls reload_and_swap inline |
| `api/src/v1/execution.rs` — `execute_single` R4 return type | `Result<ExecuteResponse, (StatusCode, String)>` | ✓ VERIFIED | `:511` — return type matches R4 |
| `api/src/v1/execution.rs` — `execute_rules` HttpResponse reconstruction | Reconstructs HttpResponse from `(StatusCode, String)` Err | ✓ VERIFIED | `:731-734` — `HttpResponse::build(code).json({"error": msg})` |
| `api/src/v1/execution.rs` — batch three-arm collector | `Ok(Ok)`/`Ok(Err)`/`Err(JoinError)` arms | ✓ VERIFIED | `:821-847` — three explicit arms with correct counter increments |
| `api/src/v1/execution.rs` — batch Err-mask placeholder removed | No hardcoded zero-fired ExecuteResponse | ✓ VERIFIED | grep for `engine_generation: 0u64` returns empty; the old `match ... { Ok(resp) => resp, Err(_) => ExecuteResponse{...} }` is gone (replaced by `.map_err` at `:800-805`) |
| `api/src/main.rs` — `build_ruleset_from_dir` removed | Local fn deleted, calls v1::execution version | ✓ VERIFIED | `main.rs:90` calls `v1::execution::build_ruleset_from_dir`; no local fn definition remains (grep clean) |
| `api/src/main.rs` — SharedRuleSet wiring | `SharedRuleSet::new(...)` inside `Arc<RwLock<...>>` | ✓ VERIFIED | `:90-91` |
| `api/tests/execute_integration.rs` — 8 Wave 0 tests | Named per 25-VALIDATION.md | ✓ VERIFIED | All 8 present at lines 606, 723, 797, 865, 948, 1017, 1057, 1095 |
| `api/tests/execute_integration.rs` — `write_rule_file` helper | Serializes Rule to .rules JSON | ✓ VERIFIED | Present (referenced by keystone test at `:619, :647`) |
| `api/tests/execute_integration.rs` — `shared_ruleset()` returns SharedRuleSet | Migrated from bare RuleSet | ✓ VERIFIED | Keystone test uses `init_test_app!()` which resolves `web::Data<Arc<RwLock<SharedRuleSet>>>` extractors — tests pass |
| `api/tests/execute_integration.rs` — `configure_test_routes` extended | reload/batch-execute/jobs routes | ✓ VERIFIED | Tests call `/v1/rules/reload`, `/v1/jobs/{id}`, `/v1/rules/batch-execute` successfully |

### Key Link Verification

| From | To | Via | Status | Details |
| --- | --- | --- | --- | --- |
| `execute_single` | shared `SharedRuleSet` | `ruleset.read()` → `guard.ruleset` + `guard.generation` | ✓ WIRED | `:559-575`; generation stamped at `:708` |
| `reload_rules` (sync) | `reload_and_swap` | inline `reload_and_swap(shared.get_ref(), &rules_dir).await` | ✓ WIRED | `:955` |
| `reload_rules` (async) | `reload_and_swap` | spawned task calls `reload_and_swap(&shared_clone, &dir_clone).await` | ✓ WIRED | `:925` |
| `reload_and_swap` | `build_ruleset_from_dir` | `let new_ruleset = build_ruleset_from_dir(rules_dir)` before lock | ✓ WIRED | `:348` |
| `reload_rules` (async) | `JobStore` | `job_store.write()` insert Pending before spawn; Running→Completed/Failed in task | ✓ WIRED | `:889-902` (Pending insert), `:912-920` (Running), `:928-944` (Completed/Failed) |
| `get_job_status` | `JobStore` | `store.read()` → `jobs.get(&job_id)` | ✓ WIRED | `:1015-1025` |
| `batch_execute_rules` JoinSet task | `execute_single` | `execute_single(...).map_err(\|(_code, msg)\| msg)` | ✓ WIRED | `:800-805` |
| `main.rs` startup | `v1::execution::build_ruleset_from_dir` | `SharedRuleSet::new(v1::execution::build_ruleset_from_dir(&rules_dir))` | ✓ WIRED | `main.rs:90` |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| --- | --- | --- | --- | --- |
| `execute_single` `ExecuteResponse.engine_generation` | `generation: u64` | `guard.generation` (read under lock from `SharedRuleSet`) | Yes — bumped by `reload_and_swap` via `wrapping_add(1)` | ✓ FLOWING |
| `execute_single` `ExecuteResponse.facts_after` | engine `get_facts()` | `engine_result` from `engine.execute()` run in `web::block` | Yes — keystone test asserts score 10 pre-reload, 20 post-reload | ✓ FLOWING |
| `reload_and_swap` `ReloadResult.rule_count` | `new_ruleset.rules.len()` | `build_ruleset_from_dir` parses real .rules JSON | Yes — keystone test proves parser loads +10 rule pre-reload and +20 post-overwrite | ✓ FLOWING |
| `get_job_status` `ReloadJob.status` | `job_store.read().get(&job_id)` | `reload_rules` async branch writes Pending/Running/Completed/Failed | Yes — `async_reload_job_transitions` test observes pending/running→completed | ✓ FLOWING |
| `batch_execute_rules` `BatchExecuteResponse` counters | `succeeded`/`failed`/`total_fired` | three-arm collector over JoinSet results | Yes — `batch_truthful_aggregation_one_failing_item` asserts succeeded==1, failed==1, total_fired>=1 | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| --- | --- | --- | --- |
| SC-1 keystone: reload→poll→execute refreshed rules + gen>0 | `cargo test -p api --test execute_integration -- reload_then_execute_against_refreshed_rules` | 1 passed; 0 failed | ✓ PASS |
| SC-1 job transitions pending→running→completed | `cargo test -p api --test execute_integration -- async_reload_job_transitions` | 1 passed; 0 failed | ✓ PASS |
| SC-3 engine_generation increments after reload | `cargo test -p api --test execute_integration -- engine_generation_increments_after_reload` | 1 passed; 0 failed | ✓ PASS |
| SC-2 batch truthful aggregation (one failing item) | `cargo test -p api --test execute_integration -- batch_truthful_aggregation_one_failing_item` | 1 passed; 0 failed | ✓ PASS |
| D-01/L-05 in-flight uses old snapshot (observable contract) | `cargo test -p api --test execute_integration -- in_flight_run_uses_old_snapshot` | 1 passed; 0 failed | ✓ PASS |
| D-03a reload empty dir warns not crashes | `cargo test -p api --test execute_integration -- reload_empty_dir_warns_not_crashes` | 1 passed; 0 failed | ✓ PASS |
| D-04 sync reload swaps and bumps generation | `cargo test -p api --test execute_integration -- sync_reload_swaps_and_bumps_generation` | 1 passed; 0 failed | ✓ PASS |
| L-06 batch panicking item isolated (ignored test) | `cargo test -p api --test execute_integration -- batch_panicking_item_isolated --include-ignored` | 1 passed; 0 failed | ✓ PASS |
| Full api suite (no regressions) | `cargo test -p api` | 103 passed; 0 failed; 1 ignored | ✓ PASS |

### Probe Execution

Step 7c: SKIPPED — no `scripts/*/tests/probe-*.sh` probes declared in PLAN/SUMMARY for this phase. The phase uses `cargo test` as its sole verification mechanism (per `25-VALIDATION.md`).

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| --- | --- | --- | --- | --- |
| DEPLOY-05 | 25-01, 25-02, 25-03 | Async reload writes `ReloadJob{Pending→Running→Completed/Failed}` to JobStore; `GET /v1/jobs/{id}` works; reload swaps compiled rule set | ✓ SATISFIED | `reload_rules` async branch (`execution.rs:883-950`) inserts Pending before spawn, transitions Running→Completed/Failed; `get_job_status` returns real job; `reload_and_swap` swaps + bumps generation. SC-1 keystone test passes |
| EXEC-03 | 25-01, 25-03 | Batch-execute drives real per-item execution with per-item isolation; `total_fired`/`succeeded`/`failed` aggregate truthfully; one panicking item does not poison others | ✓ SATISFIED | `batch_execute_rules` JoinSet task returns `Result<ExecuteResponse, String>` (`:800-805`); three-arm collector (`:821-847`); `batch_truthful_aggregation_one_failing_item` test passes (succeeded==1, failed==1); `batch_panicking_item_isolated` passes with `--include-ignored` |

**Orphaned requirements check:** REQUIREMENTS.md maps exactly DEPLOY-05 and EXEC-03 to Phase 25. Both appear in plan frontmatter (25-01: both; 25-02: DEPLOY-05; 25-03: both). No orphaned requirements.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| --- | --- | --- | --- | --- |
| `api/src/v1/execution.rs` | 797 | "Err-mask placeholder" (descriptive prose: "is GONE") | ℹ️ Info | None — describes the removed stub, not active debt |
| `api/tests/execute_integration.rs` | 857 | "placeholder" (descriptive prose in test doc comment) | ℹ️ Info | None — describes the pre-25-03 behavior being fixed |

No TBD/FIXME/XXX debt markers in any modified file. No `return null`/`return []`/`=> {}` stub returns in the implementation. No hardcoded empty data flowing to rendering. The two "placeholder" grep hits are prose describing what was *removed*, not active stubs.

### Human Verification Required

None. All phase behaviors have automated verification:
- SC-1 (reload→poll→execute refreshed rules + gen bump) — `reload_then_execute_against_refreshed_rules` + `async_reload_job_transitions` tests pass
- SC-2 (batch truthful aggregation + panic isolation) — `batch_truthful_aggregation_one_failing_item` + `batch_panicking_item_isolated` tests pass
- SC-3 (engine_generation increments) — `engine_generation_increments_after_reload` test passes
- D-01/L-05 (in-flight uses old snapshot) — `in_flight_run_uses_old_snapshot` test passes (observable contract: pre-reload old, post-reload new; structural guarantee via clone-under-read-lock + release-before-web::block at `execution.rs:559-575`)
- D-03a (empty dir warn not crash) — `reload_empty_dir_warns_not_crashes` test passes
- D-04 (sync reload swaps) — `sync_reload_swaps_and_bumps_generation` test passes

Note on `in_flight_run_uses_old_snapshot`: the test verifies the *observable contract* (sequential pre-reload sees old snapshot, post-reload sees new snapshot). The strict concurrent in-flight scenario (a run that spans the swap window) is not exercised by a test; however, the code structurally guarantees this invariant — `execute_single` clones the ruleset under the read lock and releases the guard before `web::block` (`execution.rs:559-575`), so an in-flight run's engine holds its own clone regardless of a concurrent swap. The test's doc comment explicitly notes this is a "Wave 0 simplification" with true concurrent in-flight deferred. This is informational only; the must-have truth (structural clone-then-release) is verified by code reading, and the observable contract test passes.

### Gaps Summary

No gaps. All 17 must-haves verified. All 3 roadmap Success Criteria verified with passing behavioral tests. Both requirements (DEPLOY-05, EXEC-03) satisfied. All 8 Wave 0 RED tests flipped GREEN (Nyquist GREEN completion). Full `cargo test -p api` returns 103 passed, 0 failed, 1 ignored (the ignored test passes with `--include-ignored`). All locked decisions (D-01 through D-05, L-01 through L-06, R1, R4) honored in the code as verified by reading the implementation.

---

_Verified: 2026-07-03T18:30:00Z_
_Verifier: Claude (gsd-verifier)_