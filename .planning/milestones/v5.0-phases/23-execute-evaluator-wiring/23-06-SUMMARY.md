---
phase: 23-execute-evaluator-wiring
plan: 06
subsystem: engine
tags: [rust, treat, leaps, phreak, direct-evaluator, fixpoint, coverage, tdd, keystone, multi-engine]

# Dependency graph
requires:
  - phase: 23-01
    provides: "ExecutionEngine::load_ruleset/get_facts default-impl trait methods + EngineResult.fired_rule_ids + register_default_engines registering all four factories under cfg feature gates"
  - phase: 23-02
    provides: "Expression AST + Expression::evaluate + compare_values + Condition::evaluate (real Pattern matching + full ComparisonOp) + Action::UpdateFact.fields as HashMap<String, Expression>"
  - phase: 23-03
    provides: "ReteEngine direct evaluator — the canonical reference implementation (rules field, load_ruleset/get_facts overrides, execute_rules direct interpreter, apply_action helper, fired_set once-per-rule guard, fixpoint cap, once-per-rule coverage via HashSet) that this plan mirrors onto the other three engines"
  - phase: 23-05
    provides: "POST /v1/execute wired through EngineRegistry — the API surface that is only honest once all four engines actually fire rules (this plan fills Treat/Leaps/Phreak)"
provides:
  - "TreatEngine.rules: Vec<Rule> field populated by load_ruleset (clones rules from the shared RuleSet into the per-request engine)"
  - "LeapsEngine.rules: Vec<Rule> field populated by load_ruleset"
  - "PhreakEngine.rules: Vec<Rule> field populated by load_ruleset"
  - "TreatEngine/LeapsEngine/PhreakEngine::load_ruleset override (clones ruleset.rules into self.rules, returns Ok)"
  - "TreatEngine/LeapsEngine/PhreakEngine::get_facts override (returns self.working_facts.values().cloned().collect() — post-execution working memory)"
  - "TreatEngine/LeapsEngine/PhreakEngine::execute_rules rewritten into the real direct evaluator (D-04): delegates to the shared engine::direct_eval::direct_execute helper, returns EngineResult with fired_rule_ids populated"
  - "engine/src/engine/direct_eval.rs shared module — direct_execute(rules, working_facts) -> (fired_set, evaluated_set) + apply_action(action, working_facts); the engine-agnostic evaluate-and-apply loop factored out of ReteEngine's inlined reference, used by all three non-Rete engines to avoid 3x duplication"
  - "engine/tests/engine_execution_tests.rs — 3 serial-gated keystone integration tests (treat/leaps/phreak score+10) with per-engine fire_count==1 coverage-once-per-rule assertion (D-02)"
affects: []
# Final plan of phase 23 — no downstream phase-23 plans. Affects future
# milestone work that selects engine by name (the engine param is now
# truthful for every advertised name) and any future real-algorithm
# rollout (TREAT/LEAPS/PHREAK real algos can swap in behind the same
# load_ruleset/get_facts/execute_rules contract).

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Shared direct-evaluator helper module: engine::direct_eval::direct_execute(rules, working_facts) -> (fired_set, evaluated_set) factored out of ReteEngine's inlined reference, used by Treat/Leaps/Phreak. ReteEngine keeps its own inlined impl (the canonical reference with its own metrics integration); the other three delegate. Avoids 3x duplication without churning the tested-and-shipped ReteEngine."
    - "Per-engine execute_rules delegates to the shared helper, then performs engine-specific metrics bookkeeping (record_rule_fired per fired rule, set_execution_time_ms) and wraps fired_set into EngineResult (populating fired_rule_ids). Coverage recording is centralized in the helper (once per rule after the loop — Pitfall 6) so no engine file imports CoverageTracking anymore."
    - "execute_rules return type changed from usize to EngineResult on all three engines (mirrors 23-03's Rete change). execute() delegates directly (self.execute_rules()); run() reads .rules_fired off the result."
    - "TDD RED-then-GREEN across the two tasks: keystone tests written FIRST against the 23-01 stub engines (RED — fail at load_ruleset trait default Err), then the direct evaluator implemented (GREEN — 3/3 pass). Plan task order was implement-then-test, which would have violated the RED-gate rule; reordered to tests-first (documented as a deviation)."

key-files:
  created:
    - "engine/src/engine/direct_eval.rs — shared direct-evaluator module: direct_execute(rules, working_facts) -> (HashSet<fired>, HashSet<evaluated>) with fixpoint cap (RRULES_MAX_CHAIN_ITERATIONS, default 1000) and once-per-rule CoverageTracking recording after the loop; apply_action(action, working_facts) helper mirroring WorkingMemory CRUD on HashMap<String, Fact> (UpdateFact exact-key then fact_type_id fall-back, InsertFact/RetractFact handled, CallFunction/LogMessage no-ops)"
    - "engine/tests/engine_execution_tests.rs — 3 serial-gated keystone integration tests (treat_fires_score_plus_ten, leaps_fires_score_plus_ten, phreak_fires_score_plus_ten) with shared assert_keystone_fires helper + score_plus_10 fixture mirroring rete_execution.rs; per-engine fire_count==1 assertion (Pitfall 6 regression)"
    - ".planning/phases/23-execute-evaluator-wiring/deferred-items.md — logged pre-existing cargo build -p engine --all-features prometheus/metrics.rs compile errors (out of scope, unrelated to this plan)"
  modified:
    - "engine/src/engine/mod.rs — added `pub mod direct_eval;`"
    - "engine/src/engine/treat/mod.rs — added rules: Vec<Rule> field (init empty in new()/with_cache_size(), cleared in reset()); overrode load_ruleset (clones ruleset.rules) and get_facts (returns working_facts.values().cloned()) on impl ExecutionEngine; rewrote execute_rules into direct evaluator returning EngineResult (delegates to direct_eval::direct_execute); execute() delegates directly, run() reads .rules_fired; removed placeholder per-nagram counting loop + per-nagram record_rule_fire double-count; removed now-unused CoverageTracking import"
    - "engine/src/engine/leaps/mod.rs — same mirror as treat: rules field (new()/with_network()/reset()), load_ruleset/get_facts overrides, execute_rules direct evaluator returning EngineResult, execute/run updated, placeholder per-nagram loop + double-count removed, CoverageTracking import removed"
    - "engine/src/engine/phreak/mod.rs — same mirror: rules field (new()/reset()), load_ruleset/get_facts overrides, execute_rules direct evaluator returning EngineResult, execute/run updated, placeholder per-nagram justification-adding loop + double-count removed, CoverageTracking import removed; the inherent-method test test_justification_tracking updated to assert result.success instead of the old usize return; TMS struct left intact (real TMS wiring deferred to a future phase)"

key-decisions:
  - "Factored the engine-agnostic evaluate-and-apply loop into a shared module (engine/src/engine/direct_eval.rs) rather than mirror-pasting the Rete body 3x. Per the 23-CONTEXT.md Discretion item ('How the four evaluators share code'), the planner allowed a shared helper. ReteEngine keeps its own inlined implementation (the canonical Plan 23-03 reference, with its own metrics integration and apply_action free fn); the other three engines delegate to the shared helper. This genuinely reduces 3x duplication to one shared impl + one reference impl, without churning the tested-and-shipped ReteEngine. Documented as the chosen path among the discretion options."
  - "Reordered the plan's task sequence to honor TDD. The plan listed Task 1 (implement) before Task 2 (tdd=true tests), but writing tests AFTER the implementation would make them pass on first run — a RED-gate violation. Reordered to tests-first (RED) then implementation (GREEN), which is the only valid TDD cycle. Both tasks' acceptance criteria are still satisfied; only the commit order changed."
  - "execute_rules return type changed from usize to EngineResult on all three engines (mirrors 23-03's Rete change). execute() now delegates directly (self.execute_rules()); run() reads .rules_fired off the result. This is required so fired_rule_ids propagate to the API layer (Plan 23-05 derives rules_not_fired from them)."
  - "PhreakEngine's truth-maintenance system (PhreakNetwork::tms, Justification, DependencyGraph) is NOT touched by the direct evaluator. The old placeholder execute_rules added fake justifications per nagram; the new one does not. The TMS struct and inherent methods (is_supported, get_supported_facts, justification_count, retract_fact_with_maintenance) are preserved for a future real-algorithm phase. The test_justification_tracking unit test still passes (it asserted justification_count == 0, which the new impl also satisfies)."
  - "Pre-existing cargo build -p engine --all-features failure (prometheus::Opts API drift in engine/src/engine/metrics.rs:288-293) is OUT OF SCOPE — it reproduces on the parent commit before any 23-06 changes, touches a file this plan never edits, and is gated behind the unrelated `metrics`/`prometheus` feature. Logged to deferred-items.md, not fixed. The plan's actual scope (cargo build -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak and cargo check -p api --tests) compiles clean."

patterns-established:
  - "Pattern: shared direct-evaluator helper for the non-Rete engines. When multiple engine structs share the same working-memory shape (HashMap<String, Fact>) and rule-iteration contract, factor the loop into a free function in engine/src/engine/direct_eval.rs and have each engine's execute_rules delegate + wrap the result. This is the template a future real-algorithm rollout can swap behind."
  - "Pattern: per-engine execute_rules = shared helper + engine-specific metrics + EngineResult wrap. The helper owns coverage recording (once per rule after the loop); the engine owns metrics. No engine file imports CoverageTracking anymore — coverage is centralized."
  - "Pattern: per-engine keystone test (D-02) — every engine that registers a factory in register_default_engines must have a score+10 keystone test proving it actually fires rules. available_engines() being truthful requires per-engine proof, not just compile-clean factories."

requirements-completed: [EXEC-01, ENGINE-07]

# Coverage metadata (#1602)
coverage:
  - id: D1
    description: "TreatEngine/LeapsEngine/PhreakEngine each hold a rules: Vec<Rule> field and override load_ruleset (clones ruleset.rules) + get_facts (returns working_facts.values().cloned()) on impl ExecutionEngine (L-02 bridge, Pitfall 11 facts_after)"
    requirement: "EXEC-01"
    verification:
      - kind: unit
        ref: "grep -n 'fn load_ruleset' engine/src/engine/{treat,leaps,phreak}/mod.rs (1 match each); grep -n 'fn get_facts' (1 trait-impl match each); grep -n 'rules: Vec<crate::rules::Rule>' (1 match each); cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak exits 0"
        status: pass
    human_judgment: false
  - id: D2
    description: "TreatEngine/LeapsEngine/PhreakEngine::execute_rules rewritten into the real direct evaluator (D-04): delegates to engine::direct_eval::direct_execute, iterates self.rules, evaluates Condition against working_facts, applies actions via Expression, fixpoint under RRULES_MAX_CHAIN_ITERATIONS cap (D-07), returns EngineResult with fired_rule_ids populated"
    requirement: "EXEC-01"
    verification:
      - kind: integration
        ref: "engine/tests/engine_execution_tests.rs::{treat,leaps,phreak}_fires_score_plus_ten (keystone score+10: rules_fired>=1, score==10, fired_rule_ids contains r1); cargo test -p engine --test engine_execution_tests --features engine-rete,engine-treat,engine-leaps,engine-phreak -> 3/3 pass"
        status: pass
    human_judgment: false
  - id: D3
    description: "Shared direct-evaluator module engine/src/engine/direct_eval.rs (direct_execute + apply_action) factored out of ReteEngine's inlined reference, used by all three non-Rete engines to avoid 3x duplication (per 23-CONTEXT.md Discretion item)"
    requirement: "EXEC-01"
    verification:
      - kind: unit
        ref: "engine/src/engine/direct_eval.rs (direct_execute at :56, apply_action at :78); engine/src/engine/{treat,leaps,phreak}/mod.rs execute_rules delegates to crate::engine::direct_eval::direct_execute; cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak exits 0"
        status: pass
    human_judgment: false
  - id: D4
    description: "Once-per-rule coverage recording (L-06 / Pitfall 6 / T-23-18) mirrored across all three engines: fired/evaluated ids collected in HashSet inside the shared helper, record_rule_fire/record_rule_eval called once per rule AFTER the fixpoint loop; the per-nagram double-count bugs in the placeholder execute_rules are removed (no CoverageTracking call sites remain in treat/leaps/phreak mod.rs)"
    requirement: "EXEC-01"
    verification:
      - kind: integration
        ref: "engine/tests/engine_execution_tests.rs::assert_keystone_fires asserts CoverageTracking.get_rule_state(\"r1\").fire_count == 1 for each engine (Pitfall 6 regression); grep 'CoverageTracking::record_rule_fire\\|CoverageTracking::record_rule_eval' engine/src/engine/{treat,leaps,phreak}/mod.rs -> 0 matches (coverage centralized in direct_eval.rs after loop)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Per-engine keystone integration test (D-02) for each of treat/leaps/phreak proving the score+10 scenario fires — four keystone tests total across the phase (one per engine, including 23-03's rete_execution.rs)"
    requirement: "ENGINE-07"
    verification:
      - kind: integration
        ref: "engine/tests/engine_execution_tests.rs::{treat_fires_score_plus_ten, leaps_fires_score_plus_ten, phreak_fires_score_plus_ten} (3/3 pass, serial-gated, CoverageTracking.clear() at start); combined with engine/tests/rete_execution.rs::fires_on_matching_fact (Plan 23-03) the phase has four keystone tests"
        status: pass
    human_judgment: false

# Metrics
duration: 6min
completed: 2026-07-03
status: complete
---

# Phase 23 Plan 06: Mirror direct evaluator onto Treat/Leaps/Phreak Summary

**Rolled the real direct rule evaluator out to Treat, Leaps, and Phreak — `engine: "treat"|"leaps"|"phreak"` now actually fires rules instead of returning `rules_fired: 0`. Factored the engine-agnostic evaluate-and-apply loop into a shared `engine::direct_eval` module (3x duplication -> one shared impl + Rete's inlined reference), and pinned each engine with a serial-gated score+10 keystone test. The four-engine guarantee in `available_engines()` is honest, not merely compile-clean.**

## Performance

- **Duration:** 6 min (started 2026-07-03T07:13:58Z, completed 2026-07-03T07:20:40Z)
- **Tasks:** 2 (one TDD cycle: RED then GREEN, reordered to tests-first)
- **Files modified:** 5 source + 2 new (1 source module + 1 test file) + 1 deferred-items log = 8 total

## Accomplishments
- Added a `rules: Vec<Rule>` field to `TreatEngine`, `LeapsEngine`, and `PhreakEngine` (initialized empty in every constructor, cleared in `reset()`) and overrode the `load_ruleset` and `get_facts` trait methods on each engine's `impl ExecutionEngine` block — `load_ruleset` clones `ruleset.rules` into `self.rules` (the L-02 RuleSet->engine bridge, per-request clone of the shared snapshot), `get_facts` returns `self.working_facts.values().cloned().collect()` (post-execution working memory for `facts_after`, Pitfall 11).
- Rewrote `execute_rules` on all three engines from placeholder counting loops (iterate network nodes x nagrams, double-count coverage per nagram) into the real direct evaluator: each delegates to `engine::direct_eval::direct_execute(&self.rules, &mut self.working_facts)`, which iterates `self.rules`, evaluates `rule.condition` against `working_facts` via `Condition::evaluate`, applies matching `rule.actions` (evaluating each `UpdateFact` field's `Expression`), chains to fixpoint under `RRULES_MAX_CHAIN_ITERATIONS` (D-07, default 1000), and returns an `EngineResult` with `fired_rule_ids` populated. `execute()` delegates directly; `run()` reads `.rules_fired`.
- Factored the engine-agnostic evaluate-and-apply loop into a new shared module `engine/src/engine/direct_eval.rs` (`direct_execute` + `apply_action`), avoiding 3x duplication. `ReteEngine` keeps its own inlined implementation (the canonical Plan 23-03 reference with its own metrics integration); Treat/Leaps/Phreak delegate to the shared helper. Per the 23-CONTEXT.md Discretion item, the planner allowed this factored approach.
- Fixed Pitfall 6 (coverage double-counting) across all three engines: fired and evaluated rule ids are collected in `HashSet<String>` inside the shared helper, and `CoverageTracking::record_rule_fire`/`record_rule_eval` are called exactly once per id AFTER the fixpoint loop. The per-nagram `record_rule_fire`/`record_rule_eval` call sites in the placeholder `execute_rules` of each engine are removed — no `CoverageTracking` call sites remain in `treat/mod.rs`, `leaps/mod.rs`, or `phreak/mod.rs` (coverage is centralized in `direct_eval.rs`).
- Created `engine/tests/engine_execution_tests.rs` with 3 `#[serial_test::serial]` keystone integration tests (`treat_fires_score_plus_ten`, `leaps_fires_score_plus_ten`, `phreak_fires_score_plus_ten`) sharing an `assert_keystone_fires` helper and a `score_plus_10_rule`/`score_plus_10_ruleset` fixture identical to `rete_execution.rs`. Each test asserts `rules_fired >= 1`, `fired_rule_ids` contains `r1`, `get_facts()` shows `score == 10`, and `CoverageTracking.get_rule_state("r1").fire_count == 1` (Pitfall 6 regression). Combined with `rete_execution.rs::fires_on_matching_fact` (Plan 23-03), the phase now has four keystone tests — one per engine (D-02).
- Verified no regression: 218/218 lib tests pass with all four features; 7/7 `rete_execution` tests pass; 1/1 `coverage` test passes; `cargo check -p api --tests` passes (api transitively enables all four engine features).

## Task Commits

The two tasks were executed as one TDD cycle (RED then GREEN). The plan listed Task 1 (implement) before Task 2 (tdd=true tests), which would have written tests after the implementation and made them pass on first run — a RED-gate violation. Reordered to tests-first:

1. **Task 2 RED — per-engine keystone tests (failing against 23-01 stubs)**
   - `da05fe0d0` — test(23-06): add failing per-engine keystone tests (RED — 3/3 fail at `load_ruleset` against the trait default `Err`)
2. **Task 1 + Task 2 GREEN — mirror direct evaluator onto all three engines**
   - `fee004092` — feat(23-06): mirror direct evaluator onto treat/leaps/phreak (GREEN — 3/3 keystone tests pass; 218/218 lib + 7/7 rete_execution + 1/1 coverage regression pass; api --tests check passes)

**Plan metadata:** pending (docs: complete plan — committed after STATE.md/ROADMAP.md updates)

## Files Created/Modified
- `engine/src/engine/direct_eval.rs` (NEW) — shared direct-evaluator module: `direct_execute(rules, working_facts) -> (fired_set, evaluated_set)` with fixpoint cap and once-per-rule coverage recording; `apply_action(action, working_facts)` helper
- `engine/src/engine/mod.rs` — added `pub mod direct_eval;`
- `engine/src/engine/treat/mod.rs` — `rules` field + `load_ruleset`/`get_facts` overrides + `execute_rules` direct evaluator (returns `EngineResult`); placeholder per-nagram loop + double-count removed; `CoverageTracking` import removed
- `engine/src/engine/leaps/mod.rs` — same mirror as treat
- `engine/src/engine/phreak/mod.rs` — same mirror as treat; `test_justification_tracking` unit test updated to assert `result.success` (execute_rules now returns `EngineResult`); TMS struct preserved
- `engine/tests/engine_execution_tests.rs` (NEW) — 3 serial-gated keystone tests + shared `assert_keystone_fires` helper and `score_plus_10` fixture
- `.planning/phases/23-execute-evaluator-wiring/deferred-items.md` (NEW) — logged pre-existing `cargo build -p engine --all-features` prometheus/metrics.rs failure (out of scope)

## Decisions Made
- **Factored a shared direct-evaluator module instead of mirror-pasting the Rete body 3x.** Per the 23-CONTEXT.md Discretion item, the planner allowed a shared helper. `engine::direct_eval::direct_execute` owns the fixpoint loop + coverage recording; each non-Rete engine's `execute_rules` delegates, does its own metrics bookkeeping, and wraps into `EngineResult`. `ReteEngine` keeps its inlined Plan 23-03 reference (tested and shipped) — churning it would risk regression for no gain. Result: one shared impl + one reference impl instead of 4x duplication.
- **Reordered tasks to tests-first (TDD RED-gate).** The plan listed implement-then-test; writing tests after the implementation would make them pass on first run, violating the RED-gate rule. Reordered to RED (failing tests against 23-01 stubs) then GREEN (implementation). Both tasks' acceptance criteria still satisfied.
- **`execute_rules` return type changed from `usize` to `EngineResult` on all three engines** (mirrors 23-03's Rete change) so `fired_rule_ids` propagate to the API layer. `execute()` delegates directly; `run()` reads `.rules_fired`.
- **PhreakEngine's TMS is not touched by the direct evaluator.** The old placeholder `execute_rules` added fake justifications per nagram; the new one does not. The TMS struct and inherent methods are preserved for a future real-algorithm phase. `test_justification_tracking` still passes (it asserted `justification_count == 0`).
- **Pre-existing `--all-features` prometheus failure is out of scope.** It reproduces on the parent commit (before any 23-06 changes), touches `engine/src/engine/metrics.rs` (a file this plan never edits), and is gated behind the unrelated `metrics`/`prometheus` feature. Logged to `deferred-items.md`, not fixed. The plan's actual scope (`cargo build -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` + `cargo check -p api --tests`) compiles clean.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Reordered task sequence to honor TDD RED-gate**
- **Found during:** Task execution start (task ordering review)
- **Issue:** The plan listed Task 1 (`type="auto"`, implement the direct evaluator on 3 engines) before Task 2 (`type="auto" tdd="true"`, keystone tests). Writing tests after the implementation would make them pass on first run, which the TDD fail-fast rule treats as a RED-gate violation ("If a test passes unexpectedly during the RED phase... STOP").
- **Fix:** Reordered to tests-first: wrote `engine/tests/engine_execution_tests.rs` against the 23-01 stub engines (RED — 3/3 fail at `load_ruleset` trait default `Err`), committed as `test(23-06)`, then implemented the direct evaluator on all three engines (GREEN — 3/3 pass), committed as `feat(23-06)`. Both tasks' acceptance criteria are satisfied; only the commit order changed.
- **Files modified:** none (process deviation — same files, different order)
- **Committed in:** `da05fe0d0` (RED) + `fee004092` (GREEN)

**2. [Rule 2 - Missing Critical] Fixed PhreakEngine unit test for new `EngineResult` return type**
- **Found during:** Task 1 GREEN (phreak `mod tests` would not compile)
- **Issue:** `test_justification_tracking` called `let fired = engine.execute_rules(); assert!(fired >= 0);`. After rewriting `execute_rules` to return `EngineResult` (not `usize`), `fired >= 0` does not compile (`EngineResult` does not implement `Ord`).
- **Fix:** Updated the assertion to `let result = engine.execute_rules(); assert!(result.success);` — same intent (execute succeeds), correct for the new return type. The following `assert_eq!(engine.justification_count(), 0)` still holds (the direct evaluator does not add placeholder justifications).
- **Files modified:** engine/src/engine/phreak/mod.rs (test only)
- **Committed in:** `fee004092` (Task 1 GREEN)

**3. [Rule 3 - Blocking] Invalid `-x` flag in plan verification commands**
- **Found during:** Task verification
- **Issue:** The plan's `<verify><automated>` and `<verification>` use `cargo test ... -x`. The `-x` flag is not a valid `cargo test` argument (`error: unexpected argument '-x' found`). Same artifact as 23-01/23-02/23-03/23-05.
- **Fix:** Used the correct substring-filter form `cargo test -p engine --test engine_execution_tests --features engine-rete,engine-treat,engine-leaps,engine-phreak` (no `-x`).
- **Files modified:** none (command-form fix only)

---

**Total deviations:** 3 auto-fixed (1 blocking process/ordering, 1 critical compile fix, 1 blocking command-form). No scope creep — the delivered artifacts match the plan's `output` section exactly.

## Issues Encountered
None beyond the deviations above. The pre-existing `cargo build -p engine --all-features` prometheus/metrics.rs failure was discovered and logged to `deferred-items.md` (out of scope, not a 23-06 issue).

## Threat Model Mitigation Status
- **T-23-17 (DoS, treat/leaps/phreak forward-chaining, HIGH):** MITIGATED — `RRULES_MAX_CHAIN_ITERATIONS` (default 1000) fixpoint cap mirrored into all three engines via the shared `direct_eval::direct_execute` helper. Cyclic rule sets terminate at the cap (and sooner via the once-per-rule `fired_set` guard).
- **T-23-18 (Tampering, treat/leaps/phreak coverage recording, medium):** MITIGATED — fired/evaluated ids in `HashSet`, `record_rule_fire`/`record_rule_eval` once per rule AFTER the loop in the shared helper; per-nagram double-count bugs in the placeholder `execute_rules` of all three engines removed.
- **T-23-19 (Elevation of Privilege, per-request engine isolation, HIGH):** MITIGATED (unchanged from 23-05) — factories still return `Box::new(XEngine::new())` per request; `create_engine_by_name` constructs a fresh engine moved into `web::block`; no shared `&mut engine` across requests.

## User Setup Required
None — no external service configuration, no new packages installed. All work is against the existing in-tree Rust codebase. `serial_test` is already a workspace dev-dep.

## Next Phase Readiness
- Phase 23 is now complete: all 6 plans shipped. The four engines (Rete/Treat/Leaps/Phreak) all honestly fire rules through the same direct-evaluator contract. The `engine` param is truthful for every name in `available_engines()`.
- D-01 (all four engines implemented) and D-02 (four keystone tests, one per engine) are satisfied.
- The `/v1/execute` API (Plan 23-05) now drives live engine invocation for every advertised engine name — selecting `treat`/`leaps`/`phreak` no longer hits the trait default `Err` and 500s; it fires rules and returns real `facts_after`.
- A future milestone can swap real TREAT/LEAPS/PHREAK algorithms in behind the same `load_ruleset`/`get_facts`/`execute_rules` contract — the direct evaluator is the minimal honest placeholder that the pluggable-engine architecture (v3.0) always intended.
- No blockers. The pre-existing `--all-features` prometheus failure is logged in `deferred-items.md` for a future cleanup phase; it does not affect any phase-23 success criterion.

## Requirement Status Note

`EXEC-01` and `ENGINE-07` are marked complete in this plan's frontmatter per the SUMMARY template (copy from the plan's `requirements` field). At the engine level, all four engines now fire rules truthfully (keystone score+10 passes per engine), `fired_rule_ids` are populated, `get_facts()` returns post-execution working memory, and coverage records once per fired rule. The end-to-end `/v1/execute` HTTP behavior for all four engine names was already GREEN after Plan 23-05 (which wired the API through `EngineRegistry`); this plan makes the underlying engine execution truthful for the three non-Rete engines so the API surface is honest, not hollow. REQUIREMENTS.md status is left for the phase verifier to close.

---
*Phase: 23-execute-evaluator-wiring*
*Completed: 2026-07-03*

## Self-Check: PASSED

- `engine/src/engine/direct_eval.rs` exists on disk: FOUND
- `engine/tests/engine_execution_tests.rs` exists on disk: FOUND
- `.planning/phases/23-execute-evaluator-wiring/deferred-items.md` exists on disk: FOUND
- Task 2 RED commit `da05fe0d0`: FOUND
- Task 1 + Task 2 GREEN commit `fee004092`: FOUND
- `cargo test -p engine --test engine_execution_tests --features engine-rete,engine-treat,engine-leaps,engine-phreak` exits 0 — 3/3 pass (treat/leaps/phreak keystone score+10, fire_count==1)
- `cargo test -p engine --lib --features engine-rete,engine-treat,engine-leaps,engine-phreak` exits 0 — 218/218 pass
- `cargo test -p engine --features engine-rete --test rete_execution` exits 0 — 7/7 pass (no Rete regression)
- `cargo test -p engine --features engine-rete --test coverage` exits 0 — 1/1 pass (no coverage regression)
- `cargo build -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` exits 0
- `cargo check -p api --tests` exits 0
- TDD gate compliance: RED commit `da05fe0d0` (test) followed by GREEN commit `fee004092` (feat)