---
phase: 23-execute-evaluator-wiring
plan: 04
subsystem: api
tags: [rust, rename, metrics, tdd, red-scaffold, wave-0, pitfall-10, pitfall-6]

# Dependency graph
requires:
  - phase: 23-01
    provides: "EngineRegistry::register_default_engines (all four factories) + EngineSelector::with_defaults + engine features enabled on api crate"
  - phase: 23-02
    provides: "Expression AST + Condition::Pattern { op: ComparisonOp } + Action::UpdateFact.fields as HashMap<String, Expression>"
  - phase: 23-03
    provides: "ReteEngine direct evaluator that actually fires rules (keystone score+10 passes at engine level) + once-per-rule coverage recording (Pitfall 6 fixed)"
provides:
  - "api/src/metrics.rs::MetricsSnapshot (renamed from EngineState) + MetricsTx alias + create_metrics_channel return types use MetricsSnapshot — no metrics::EngineState symbol remains (Pitfall 10 resolved before Plan 23-05 adds execute logic)"
  - "engine/tests/coverage.rs — serial-gated record_once_per_rule regression test asserting CoverageTracking fire_count == 1 after one execute() that fires r1 (Pitfall 6 regression contract; currently GREEN because Plan 23-03 landed)"
  - "api/tests/execute_integration.rs — five #[serial_test::serial] RED scaffolds (execute_fires_real_rule, facts_after_not_echoed, engine_param_validation, default_engine_resolution, coverage_auto_populated) encoding the four ROADMAP success criteria; the test App installs real EngineRegistry + EngineSelector::with_defaults() + shared Arc<RwLock<RuleSet>> with the keystone score := score + 10 fixture as app_data, exactly the state Plan 23-05's handler signature will extract"
affects: [23-05, 23-06]

# Tech tracking
tech-stack:
  added:
    - "serial_test (api crate dev-dep, workspace version 3) — CoverageTracking singleton isolation for integration tests (threat T-23-10)"
  patterns:
    - "Pure rename disambiguation: when two types in different modules share a name (metrics::EngineState vs execution::EngineState), rename the one that will share an import path first, before adding logic that imports both — update the struct, the type alias, the function signature, and all doc comments that reference the old name."
    - "RED scaffold pattern for Nyquist feedback: install the full app_data stack (registry, selector, ruleset) the future handler will extract, register the real routes, and write the substantive assertions now — the current stub ignores the extra app_data and fails the assertions, so the scaffold compiles today and turns GREEN the moment the handler is rewired (no test edits needed in Plan 05)."
    - "Macro-over-async-helper for actix test init: test::init_service returns an unnameable impl Service type, so a macro that expands to the init_service(...).await call site lets each test keep a one-line `let app = init_test_app!();` without fighting the type system (mirrors the batch_execution_integration.rs inline pattern)."
    - "serial_test::serial on ALL tests in a file that touches the process-global CoverageTracking singleton OR mutates env vars (RRULES_DEFAULT_ENGINE) — serial isolation prevents cross-test pollution (threat T-23-10)."

key-files:
  created:
    - "engine/tests/coverage.rs — serial-gated record_once_per_rule regression test (Pitfall 6 contract); GREEN against Plan 23-03's evaluator"
    - "api/tests/execute_integration.rs — five RED scaffolds covering EXEC-01/EXEC-02/ENGINE-07/ENGINE-08/MON-05"
  modified:
    - "api/src/metrics.rs — renamed EngineState struct to MetricsSnapshot; updated MetricsTx type alias and create_metrics_channel return type; expanded doc comment noting the L-05/Pitfall 10 rationale"
    - "api/src/main.rs — updated WS broadcast comment (line 210) to reference MetricsSnapshot"
    - "api/src/websocket.rs — updated all doc comments referencing the broadcast type from EngineState to MetricsSnapshot (code uses the MetricsTx alias, no type-name change needed in code)"
    - "api/src/middleware/auth.rs — updated public-prefixes doc comment to reference MetricsSnapshot"
    - "api/Cargo.toml — added serial_test = { workspace = true } to [dev-dependencies] for singleton isolation (threat T-23-10)"

key-decisions:
  - "The metrics EngineState rename is a pure rename (no logic/field/method change). execution.rs::EngineState (the per-request engine holder at execution.rs:75) is intentionally NOT renamed — it is a different struct, and the whole point of the rename is to disambiguate the two before Plan 23-05 imports both."
  - "engine/tests/coverage.rs::record_once_per_rule is GREEN, not RED. The plan text said it would be RED until Plan 23-03 landed; Plan 23-03 HAS landed (rules fire truthfully, once-per-rule coverage fixed), so this scaffold passes today. It remains a valid regression contract — any future change that re-introduces per-nagram inner-loop recording would turn it RED. The acceptance criteria only required the scaffold to compile, not a RED state."
  - "All five api/tests/execute_integration.rs scaffolds are RED on substantive assertions (rules_fired=0, facts echoed, no engine validation, no coverage population). They compile and run; they fail only on the assertions that encode the ROADMAP success criteria. This is the expected Nyquist RED state — Plan 23-05 turns them GREEN by rewriting execute_single alone (no test edits required)."
  - "Added serial_test as an api crate dev-dep (workspace version 3). The plan's threat model (T-23-10) mandates #[serial_test::serial] for CoverageTracking singleton isolation, but serial_test was only an engine dev-dep before this plan. Adding it to api is a Rule 2 auto-fix (missing critical test isolation per the threat model)."

patterns-established:
  - "Pattern: RED scaffold with forward-compatible app_data — install the app_data the future handler will extract, write the assertions now, accept RED against the current stub. Plan 05 rewires the handler and the scaffolds go GREEN with zero test edits."
  - "Pattern: macro for actix test::init_service to avoid the unnameable impl Service return type."

requirements-completed: [MON-05]

# Coverage metadata (#1602) — per-deliverable traceability.
coverage:
  - id: D1
    description: "Rename metrics::EngineState -> MetricsSnapshot (L-05 / Pitfall 10): struct + MetricsTx alias + create_metrics_channel return type + all doc-comment references; execution.rs::EngineState untouched"
    requirement: "MON-05"
    verification:
      - kind: build
        ref: "api/src/metrics.rs:16 (struct MetricsSnapshot), :26 (MetricsTx alias), :29 (create_metrics_channel); grep -rn 'metrics::EngineState' api/src returns zero matches; cargo check -p api exits 0"
        status: pass
    human_judgment: false
  - id: D2
    description: "engine/tests/coverage.rs serial-gated record_once_per_rule regression test (Pitfall 6 contract): asserts CoverageTracking::get_rule_state('r1').fire_count == 1 after one execute() that fires r1"
    requirement: "MON-05"
    verification:
      - kind: unit
        ref: "engine/tests/coverage.rs::record_once_per_rule; cargo test -p engine --features engine-rete --test coverage -> 1/1 pass (GREEN — Plan 23-03 evaluator fires rules truthfully)"
        status: pass
    human_judgment: false
  - id: D3
    description: "api/tests/execute_integration.rs five RED scaffolds: execute_fires_real_rule (EXEC-01), facts_after_not_echoed (EXEC-02 / Pitfall 11), engine_param_validation (ENGINE-07), default_engine_resolution (ENGINE-08), coverage_auto_populated (MON-05)"
    requirement: "MON-05"
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs; cargo test -p api --test execute_integration --no-run exits 0 (compiles); cargo test -p api --test execute_integration -> 0 passed, 5 failed (RED — expected, Plan 23-05 wires the handler to turn them GREEN)"
        status: pass
    human_judgment: false

# Metrics
duration: 4min
completed: 2026-07-03
status: complete
---

# Phase 23 Plan 04: Rename MetricsSnapshot + Wave 0 RED test scaffolds Summary

**Renamed `metrics::EngineState` → `MetricsSnapshot` (Pitfall 10) to disambiguate from `execution::EngineState` before Plan 23-05 adds execute-path logic, and landed the Wave 0 test scaffolds: a serial-gated `record_once_per_rule` coverage regression test (GREEN — Plan 23-03's evaluator fires rules truthfully) and five RED integration scaffolds in `api/tests/execute_integration.rs` encoding the four ROADMAP success criteria (EXEC-01/02, ENGINE-07/08, MON-05), installed against the full Plan 23-05 app_data stack so they go GREEN the moment `execute_single` is rewired.**

## Performance

- **Duration:** 4 min (started 2026-07-03T06:52:41Z, completed 2026-07-03T06:56:39Z)
- **Tasks:** 2 (1 refactor + 1 test-scaffold)
- **Files modified:** 6 (4 source-rename + 2 new test files + 1 Cargo.toml + Cargo.lock)

## Accomplishments
- Renamed `api/src/metrics.rs::EngineState` to `MetricsSnapshot` (L-05 / Pitfall 10), updating the `MetricsTx` type alias (`Arc<tokio::sync::broadcast::Sender<MetricsSnapshot>>`) and the `create_metrics_channel` return type. Updated all doc-comment references across `metrics.rs`, `main.rs` (WS broadcast comment), `websocket.rs` (8 comment sites — code uses the `MetricsTx` alias so no code change was needed there), and `middleware/auth.rs` (public-prefixes comment). The `execution.rs::EngineState` (the per-request engine holder, line 75) is intentionally untouched — the rename's purpose is to disambiguate the two types before Plan 23-05 imports both.
- Created `engine/tests/coverage.rs` with a `#[serial_test::serial]` `record_once_per_rule` regression test that asserts `CoverageTracking::get_rule_state("r1").fire_count == 1` after one `execute()` that fires the keystone `score := score + 10` rule (the Pitfall 6 regression contract). This test is GREEN because Plan 23-03's direct evaluator already fires rules truthfully and records once per rule; it guards against any future regression that re-introduces per-nagram inner-loop recording.
- Created `api/tests/execute_integration.rs` with five `#[serial_test::serial]` RED scaffolds: `execute_fires_real_rule` (EXEC-01 — `rules_fired >= 1`), `facts_after_not_echoed` (EXEC-02 / Pitfall 11 — `facts_after.scorecard.score == 10`), `engine_param_validation` (ENGINE-07 — `engine: "rete"` → 200, `engine: "unknown"` → 400), `default_engine_resolution` (ENGINE-08 — omitted `engine` → 200 against `RRULES_DEFAULT_ENGINE=rete`), and `coverage_auto_populated` (MON-05 — GET `/v1/metrics/coverage` shows `r1` with `fired == true`, `fire_count == 1`). The test App installs a real `EngineRegistry` (`register_default_engines`), `EngineSelector::with_defaults()`, and a shared `Arc<RwLock<RuleSet>>` containing the keystone fixture rule as `app_data` — exactly the state Plan 23-05's handler signature will extract. The current stub handler ignores the extra `app_data` and returns `rules_fired: 0` with echoed `facts_after`, so all five fail on substantive assertions (Nyquist RED) while compiling cleanly.
- Added `serial_test = { workspace = true }` to `api/Cargo.toml` `[dev-dependencies]` (Rule 2 auto-fix — threat T-23-10 mandates `#[serial_test::serial]` for `CoverageTracking` singleton isolation, but `serial_test` was previously only an engine dev-dep).

## Task Commits

1. **Task 1: Rename metrics EngineState → MetricsSnapshot (L-05 / Pitfall 10)**
   - `03bec3e1d` — refactor(23-04): rename metrics::EngineState to MetricsSnapshot (L-05 / Pitfall 10)
2. **Task 2: Land Wave 0 test scaffolds (engine/tests/coverage.rs + api/tests/execute_integration.rs)**
   - `13daa9405` — test(23-04): land Wave 0 RED scaffolds for execute + coverage

**Plan metadata:** pending (docs: complete plan — committed after STATE.md/ROADMAP.md updates)

## Files Created/Modified
- `api/src/metrics.rs` — renamed `EngineState` struct to `MetricsSnapshot`; updated `MetricsTx` alias and `create_metrics_channel` return type; expanded doc comment with L-05/Pitfall 10 rationale
- `api/src/main.rs` — updated WS broadcast comment (line 210) from `EngineState` to `MetricsSnapshot`
- `api/src/websocket.rs` — updated 8 doc-comment references from `EngineState` to `MetricsSnapshot` (code unchanged — uses `MetricsTx` alias)
- `api/src/middleware/auth.rs` — updated public-prefixes doc comment to `MetricsSnapshot`
- `engine/tests/coverage.rs` — new integration test file; `record_once_per_rule` serial-gated regression test (GREEN)
- `api/tests/execute_integration.rs` — new integration test file; five RED scaffolds covering EXEC-01/02, ENGINE-07/08, MON-05
- `api/Cargo.toml` — added `serial_test = { workspace = true }` dev-dep
- `Cargo.lock` — serial_test dep resolution for api crate

## Decisions Made
- **The metrics rename is a pure rename.** No logic, field, or method changes. `execution.rs::EngineState` (line 75, the per-request engine holder) is intentionally NOT renamed — it is a different struct, and the rename's purpose is to disambiguate the two before Plan 23-05 imports both. All doc comments that referenced the metrics broadcast type were updated to `MetricsSnapshot` for consistency.
- **`engine/tests/coverage.rs::record_once_per_rule` is GREEN, not RED.** The plan text said it would be RED until Plan 23-03 landed; Plan 23-03 HAS landed (rules fire truthfully, once-per-rule coverage fixed), so this scaffold passes today. It remains a valid regression contract. The acceptance criteria only required the scaffold to compile (`cargo test --no-run` exits 0), not a RED state.
- **All five api scaffolds are RED on substantive assertions.** They compile and run; they fail only on the assertions encoding the ROADMAP success criteria. This is the expected Nyquist RED state — Plan 23-05 turns them GREEN by rewriting `execute_single` alone (zero test edits required).
- **Added `serial_test` as an api crate dev-dep.** Threat T-23-10 mandates `#[serial_test::serial]` for `CoverageTracking` singleton isolation; `serial_test` was only an engine dev-dep before this plan. Rule 2 auto-fix (missing critical test isolation per threat model).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added `serial_test` dev-dep to api crate**
- **Found during:** Task 2 implementation
- **Issue:** The plan's threat model (T-23-10) mandates `#[serial_test::serial]` on all tests touching the `CoverageTracking` singleton, but `serial_test` was only declared as a dev-dependency in `engine/Cargo.toml`, not `api/Cargo.toml`. The api integration scaffolds could not use the attribute without adding the dep.
- **Fix:** Added `serial_test = { workspace = true }` to `api/Cargo.toml` `[dev-dependencies]` (workspace version 3, already declared in the root `Cargo.toml`).
- **Files modified:** api/Cargo.toml, Cargo.lock
- **Verification:** `cargo test -p api --test execute_integration --no-run` exits 0; all five scaffolds carry `#[serial_test::serial]`.
- **Committed in:** `13daa9405` (Task 2)

**2. [Rule 3 - Blocking] Plan verify command omits required `--features engine-rete` flag**
- **Found during:** Task 2 verification
- **Issue:** The plan's `<verify><automated>` for the engine coverage test runs `cargo test -p engine --test coverage --no-run` without the `engine-rete` feature flag. `ReteEngine` is gated behind `#[cfg(feature = "engine-rete")]` (`engine/src/engine/mod.rs:31`), so the test fails to compile with `unresolved import engine::engine::rete`. Same artifact as Plans 23-01/23-02/23-03 (the `-x` flag / missing feature flags in plan verify commands).
- **Fix:** Used `cargo test -p engine --features engine-rete --test coverage --no-run` (and the run variant with the same feature flag) — matches the `rete_execution.rs` test invocation convention established in Plan 23-03.
- **Files modified:** none (command-form fix only)

---

**Total deviations:** 2 auto-fixed (1 critical test isolation, 1 blocking command-form). No scope creep — the delivered artifacts match the plan's `output` section exactly.

## Issues Encountered
None beyond the deviations above. The `test::init_service` return type is an unnameable `impl Service`, so a helper function returning the initialized app was infeasible; resolved with a `init_test_app!()` macro that expands to the `init_service(...).await` call site (mirrors the inline pattern in `batch_execution_integration.rs`).

## User Setup Required
None — no external service configuration, no new runtime packages. `serial_test` was already a workspace dependency; this plan only added it to the api crate's dev-deps.

## Next Phase Readiness
- The `metrics::EngineState` name collision is resolved. Plan 23-05 can now add execute-path logic that imports both `MetricsSnapshot` (metrics broadcast) and `execution::EngineState` (per-request holder) without ambiguity.
- The five RED scaffolds in `api/tests/execute_integration.rs` are Plan 23-05's immediate feedback loop: rewrite `execute_single` to extract `web::Data<EngineRegistry>`, `web::Data<EngineSelector>`, `web::Data<Arc<RwLock<RuleSet>>>`, resolve/validate the `engine` param, `load_ruleset`, `insert_fact` per request fact, `execute()`, serialize `facts_after` from `get_facts()`, and the scaffolds go GREEN with zero test edits. The app_data stack is already installed in the test App.
- The `record_once_per_rule` regression test guards Pitfall 6 going forward — any change that re-introduces inner-loop coverage recording turns it RED.
- No blockers. The `MetricsSnapshot` rename + scaffold contracts are stable for Plan 23-05.

## Requirement Status Note

`MON-05` is marked complete in this plan's frontmatter per the SUMMARY template (copy from the plan's `requirements` field). The engine-level once-per-rule coverage contract is established and GREEN (`record_once_per_rule`). The full MON-05 requirement — truthful `rules_fired`/`facts_after`/coverage from the live `/v1/execute` endpoint — is only end-to-end satisfied when Plan 23-05's API integration tests pass against the wired HTTP handler. REQUIREMENTS.md status is left for the phase verifier to close after 23-05 lands.

---
*Phase: 23-execute-evaluator-wiring*
*Completed: 2026-07-03*

## Self-Check: PASSED

- `api/src/metrics.rs` exists on disk with `struct MetricsSnapshot`: FOUND
- `engine/tests/coverage.rs` exists on disk: FOUND
- `api/tests/execute_integration.rs` exists on disk: FOUND
- Task 1 commit `03bec3e1d`: FOUND
- Task 2 commit `13daa9405`: FOUND
- `grep -rn 'metrics::EngineState' api/src` returns zero matches: OK
- `grep -n 'struct MetricsSnapshot' api/src/metrics.rs` returns line 16: OK
- `grep -n 'pub struct EngineState' api/src/v1/execution.rs` returns line 75 (untouched): OK
- `cargo check -p api` exits 0: OK
- `cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` exits 0: OK
- `cargo test -p engine --features engine-rete --test coverage` -> 1/1 pass (`record_once_per_rule` GREEN)
- `cargo test -p api --test execute_integration --no-run` exits 0 (scaffolds compile); run -> 0 passed, 5 failed (RED — expected, Plan 23-05 turns GREEN)
- Post-commit deletion check: no tracked files deleted in either task commit