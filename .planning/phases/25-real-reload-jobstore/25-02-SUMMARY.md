---
phase: 25-real-reload-jobstore
plan: 02
subsystem: api-execute-path
tags: [rules-parser, serde-json, reload-and-swap, generation-bump, wave-1, r1-prerequisite, d-04-shared-swap]
requires:
  - "Phase 25 Plan 01 (SharedRuleSet wrapper + 8 Wave 0 RED tests)"
provides:
  - "pub fn build_ruleset_from_dir(rules_dir: &str) -> RuleSet in api::v1::execution — real .rules JSON parser (R1)"
  - "pub async fn reload_and_swap(&Arc<RwLock<SharedRuleSet>>, &str) -> Result<ReloadResult, String> — atomic swap + generation bump (D-04)"
  - "main.rs calls v1::execution::build_ruleset_from_dir (D-05 single source of truth for startup + reload)"
affects:
  - "api/src/v1/execution.rs (build_ruleset_from_dir pub fn, reload_and_swap pub async fn, Rule import)"
  - "api/src/main.rs (local build_ruleset_from_dir removed, call site updated, RuleSet import removed)"
tech-stack:
  added: []
  patterns:
    - "serde_json::from_str::<Rule> then serde_json::from_str::<Vec<Rule>> — single-rule + rule-array file shapes, all AST types derive Deserialize"
    - "D-03a warn+continue: missing dir, failed read_dir, unreadable file, unparseable content, validation failure → tracing::warn + skip, never crash"
    - "Off-to-side build + single whole-value swap under write lock (Pitfall 4: no I/O under lock, D-02: swap whole struct not in-place mutation)"
key-files:
  created: []
  modified:
    - "api/src/v1/execution.rs"
    - "api/src/main.rs"
decisions:
  - "R1: build_ruleset_from_dir now parses .rules/.drl JSON files via serde_json::from_str::<Rule> / Vec<Rule> — no longer returns empty RuleSet"
  - "D-05: build_ruleset_from_dir moved to api::v1::execution (pub) — single source of truth for startup (main.rs) and reload (reload_and_swap)"
  - "D-03a: warn + empty RuleSet on missing/empty/unparseable dir or file — never crash (preserved from Phase 23)"
  - "D-04/D-02/L-01: reload_and_swap builds off-to-side, swaps whole SharedRuleSet { ruleset, generation: old.wrapping_add(1) } in one write-lock assignment"
  - "Pitfall 4: write lock held ONLY for the single *guard = ... assignment — no file I/O or parsing under the lock"
metrics:
  duration: "~3 min"
  completed: 2026-07-03
  tasks: 2
  files: 2
status: complete
---

# Phase 25 Plan 02: Real .rules JSON Parser + reload_and_swap Summary

Implemented the real `.rules` JSON parser inside `build_ruleset_from_dir` (R1 — the critical prerequisite without which reload swaps empty-for-empty and SC-1 is impossible), and added the `reload_and_swap` helper (D-04) that both sync and async reload paths will call in Plan 25-03.

## What Was Built

### Task 1: Real .rules JSON parser in build_ruleset_from_dir (R1, D-05, D-03a)

**Commit:** `e15b62004`

- Moved `build_ruleset_from_dir` from `api/src/main.rs:44-71` into `api/src/v1/execution.rs` as `pub fn build_ruleset_from_dir(rules_dir: &str) -> RuleSet`, positioned after the `SharedRuleSet` impl block.
- Rewrote the body to actually parse `.rules` and `.drl` files: iterates `std::fs::read_dir(path)`, for each entry whose file name ends with `.rules` or `.drl`, calls `std::fs::read_to_string(entry.path())`, then attempts `serde_json::from_str::<Rule>(&content)`; if that fails, attempts `serde_json::from_str::<Vec<Rule>>(&content)` (supports both single-rule and rule-array file shapes). For each parsed Rule, calls `ruleset.add_rule(rule)`.
- **D-03a preserved (warn + continue, never crash):** on missing dir (warn + empty), failed `read_dir` (warn + empty), unreadable file (warn + skip), unparseable content (warn + skip), or rule that fails `add_rule` validation (warn + skip). One bad file does not crash startup or reload. If the resulting ruleset is empty, a warning is logged.
- Added `Rule` to the `use engine::rules::{Rule, RuleSet};` import in execution.rs (was `RuleSet` only).
- In `api/src/main.rs`: deleted the local `build_ruleset_from_dir` function and its stale doc comment (the "live parsing is Phase 25 scope" deferral log is gone). Updated the call site to `SharedRuleSet::new(v1::execution::build_ruleset_from_dir(&rules_dir))`. Removed the now-unused `use engine::rules::RuleSet;` import.
- **D-05:** this is the single function both startup (`main.rs`) and reload (`reload_and_swap` in Task 2) call, so the swapped RuleSet matches what a restart would load.

### Task 2: reload_and_swap helper — single swap implementation for sync + async (D-04, D-02, L-01, Pitfall 4)

**Commit:** `2f3bf8fd5`

- Added `pub async fn reload_and_swap(shared: &Arc<std::sync::RwLock<SharedRuleSet>>, rules_dir: &str) -> Result<ReloadResult, String>` to `api/src/v1/execution.rs`, positioned after the `ReloadResult` struct.
- Body: records `start_time = Utc::now()`; calls `build_ruleset_from_dir(rules_dir)` OFF-SIDE (no lock held during I/O + parse — Pitfall 4); captures `rule_count = new_ruleset.rules.len()`; acquires the write lock; computes `next_gen = guard.generation.wrapping_add(1)`; generates `reload_id = Uuid::new_v4()`; assigns `*guard = SharedRuleSet { ruleset: new_ruleset, generation: next_gen }` (the single whole-value swap — D-02: swap the whole struct, never in-place mutation); drops the guard (implicit); returns `Ok(ReloadResult { reload_id, rule_count, start_time, end_time: Utc::now(), success: true })`.
- Returns `Err(String)` if the write lock is poisoned.
- The function is `async` for parity with the spawned-task context in Plan 25-03; the body is sync I/O + a lock, so `.await` is a no-op but the signature matches.
- **Not yet wired to any handler** — `reload_rules` still uses the old `EngineState::reload_from_directory` / `reload_engine` path. Plan 25-03 Task 1 rewrites `reload_rules` to call `reload_and_swap`. The function is `pub` (no dead_code warning) but is dead code at runtime until 25-03.
- Uses only existing imports (`Utc`, `Uuid`, `Arc`, `SharedRuleSet`, `ReloadResult` all in scope). No new dependencies (L-01: no ArcSwap).

## Verification Results

- `cargo build -p api` — clean (exit 0)
- `cargo check -p api` — clean (exit 0, no dead_code warnings for new functions)
- `cargo test -p api --test execute_integration` — 8 passed, 7 failed, 1 ignored
  - 8 pre-existing tests GREEN (no regression)
  - 7 Wave 0 RED tests stay RED (all depend on 25-03 handler wiring — expected)
  - 1 `#[ignore]`d test (`batch_panicking_item_isolated` — needs 25-03)
- `cargo test -p api --test batch_execution_integration` — 5 passed, 0 failed (no regression)
- `cargo test -p api --test metrics_ws` — 4 passed, 0 failed (no regression)

### Wave 0 RED test status after 25-02

| # | Test | Status | Reason |
|---|------|--------|--------|
| 1 | `reload_then_execute_against_refreshed_rules` | RED | reload_rules handler still no-op (25-03) |
| 2 | `async_reload_job_transitions` | RED | JobStore never written (25-03) |
| 3 | `engine_generation_increments_after_reload` | RED | gen stays 0, handler not rewired (25-03) |
| 4 | `batch_truthful_aggregation_one_failing_item` | RED | batch Err-mask bug (25-03) |
| 5 | `in_flight_run_uses_old_snapshot` | RED | reload doesn't swap, handler not rewired (25-03) |
| 6 | `batch_panicking_item_isolated` | IGNORED | needs 25-03 deterministic panic trigger |
| 7 | `reload_empty_dir_warns_not_crashes` | RED | exercises `/v1/rules/reload` endpoint (old handler returns 500 on missing dir — 25-03 rewires to `reload_and_swap` which returns empty gracefully) |
| 8 | `sync_reload_swaps_and_bumps_generation` | RED | sync reload doesn't swap, handler not rewired (25-03) |

**Note on `reload_empty_dir_warns_not_crashes`:** The plan's acceptance criteria (line 99) explicitly state this test "may go GREEN (parser returns empty on empty dir without crashing) or remain RED if it exercises the reload endpoint (still a no-op until Plan 25-03)." This test posts to `/v1/rules/reload` with `async_reload: false`, which still calls `EngineState::reload_from_directory` (returns `Err("Directory not found")` → HTTP 500). The new `build_ruleset_from_dir` parser does handle empty/missing dirs gracefully (D-03a), but the handler has not been rewired to use it yet — that is Plan 25-03's scope. The test remains RED as the plan allows. No 25-03 over-reach.

## Deviations from Plan

None — plan executed exactly as written. Both tasks implemented per their `<action>` sections, all acceptance criteria met.

## Known Stubs

| File | Line (approx) | Description | Resolved By |
|------|---------------|-------------|-------------|
| `api/src/v1/execution.rs` | ~700 | `reload_rules` handler still uses `EngineState::reload_from_directory` / `reload_engine` (no-op reload, not calling `reload_and_swap`) | Plan 25-03 Task 1 (rewire to `reload_and_swap` + JobStore) |
| `api/src/v1/execution.rs` | ~626 | `engine_generation: 0u64` in batch Err-mask placeholder | Plan 25-03 Task 2 (D-03 truthful Err→failed aggregation) |
| `api/src/v1/execution.rs` | ~183 | `EngineState::reload_engine` no-op (acquire-and-drop write lock) | Plan 25-03 (deprecate/remove; reload operates on shared ruleset) |

These stubs are intentional and documented in the plan as deferred to 25-03. They do NOT prevent this plan's goal (real parser + reload_and_swap helper) from being achieved.

## TDD Gate Compliance

- **RED gate:** The 8 Wave 0 RED tests from Plan 25-01 serve as the RED gate. After 25-02, 7 remain RED (all depend on 25-03 handler wiring) and 1 remains `#[ignore]`d. No new RED tests were added in this plan — the Wave 0 RED scaffolds encode the SC-1/SC-2/SC-3 + edge case behaviors.
- **GREEN gate:** Partial — `build_ruleset_from_dir` is now a real parser (R1 satisfied), and `reload_and_swap` implements the atomic swap + generation bump (D-04/D-02/L-01/Pitfall 4 satisfied). The GREEN commits land fully in Plan 25-03 when the handler is wired and the 7 RED tests turn GREEN.
- **Note:** Both Task 1 (`feat`) and Task 2 (`feat`) commits build cleanly (`cargo build -p api` succeeds). The RED tests fail at runtime on substantive assertions (handler not rewired), not at compile time.

## Self-Check: PASSED

- `api/src/v1/execution.rs` — FOUND (`pub fn build_ruleset_from_dir`, `pub async fn reload_and_swap`)
- `api/src/main.rs` — FOUND (local function removed, call site uses `v1::execution::build_ruleset_from_dir`)
- Commit `e15b62004` — FOUND in git log
- Commit `2f3bf8fd5` — FOUND in git log