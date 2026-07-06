---
phase: 23-execute-evaluator-wiring
plan: 03
subsystem: engine
tags: [rust, rete, direct-evaluator, fixpoint, coverage, tdd, keystone]

# Dependency graph
requires:
  - phase: 23-01
    provides: "ExecutionEngine::load_ruleset/get_facts default-impl trait methods + EngineResult.fired_rule_ids field + compiling engine-rete feature surface"
  - phase: 23-02
    provides: "Expression AST + Expression::evaluate + compare_values + rewritten Condition::evaluate (real Pattern matching + full ComparisonOp) + Action::UpdateFact.fields as HashMap<String, Expression>"
provides:
  - "ReteEngine.rules: Vec<Rule> field populated by load_ruleset (clones rules from the shared RuleSet into the per-request engine)"
  - "ReteEngine::load_ruleset override (clones ruleset.rules into self.rules, returns Ok)"
  - "ReteEngine::get_facts override (returns self.working_facts.values().cloned().collect() — post-execution working memory)"
  - "ReteEngine::execute_rules rewritten into a real direct interpreter (D-04): iterates self.rules, evaluates rule.condition against working_facts via Condition::evaluate, applies matching rule.actions (evaluating Expression per UpdateFact field via Expression::evaluate), chains to fixpoint under RRULES_MAX_CHAIN_ITERATIONS cap (D-07), returns EngineResult with fired_rule_ids populated"
  - "apply_action(action, working_facts) helper mirroring WorkingMemory CRUD on HashMap<FactId, Fact>; UpdateFact locates by exact FactId key then fact_type_id fall-back; CallFunction/LogMessage no-ops in Phase 23"
  - "Once-per-rule coverage recording (L-06 / Pitfall 6): fired/evaluated ids collected in HashSet, record_rule_fire/record_rule_eval called once per rule AFTER the fixpoint loop (not inside any inner loop); stale per-nagram record in join_propagation removed"
  - "RRULES_MAX_CHAIN_ITERATIONS env var (default 1000) — the D-07 cycle guard"
  - "engine/tests/rete_execution.rs integration test file (7 tests: keystone score+10, get_facts_after_execute, non-matching, fixpoint chain, cyclic termination, coverage once-per-rule regression, eval-but-not-fire)"
affects: [23-04, 23-05, 23-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Direct interpreter bypassing the placeholder Rete network: iterate self.rules, evaluate Condition::evaluate against working_facts.values(), apply actions — the network types stay untouched (a future enhancement can swap a real network compiler in behind the same load_ruleset/get_facts contract)"
    - "Index-based loop iteration + actions.clone() to release the immutable borrow of self.rules before mutably borrowing self.working_facts (Rust borrow-checker pattern for struct-field cross-borrows)"
    - "Once-per-rule coverage via HashSet<fired_id> + HashSet<evaluated_id> collected during the fixpoint loop, then record_rule_fire/record_rule_eval iterated AFTER the loop — the Pitfall 6 fix (no recording inside any inner loop)"
    - "Fixpoint-with-cap forward chaining: repeat passes until no rule newly fires; each rule fires at most once per execute() (fired_set guard); RRULES_MAX_CHAIN_ITERATIONS is the ultimate safety net"
    - "TDD RED/GREEN per task: failing trait-override tests (defaults return Err/empty) → implement overrides; failing integration tests against placeholder execute_rules → rewrite into real evaluator"
    - "serial_test::serial on ALL tests in a file that touches a process-global singleton (CoverageTracking) — non-serial tests run concurrently with serial ones and pollute the singleton; reset via CoverageTracking.clear() at test start"

key-files:
  created:
    - "engine/tests/rete_execution.rs — 7 serial-gated integration tests proving the keystone score+10 scenario, fixpoint chaining, cyclic termination, and once-per-rule coverage (Pitfall 6 regression)"
  modified:
    - "engine/src/engine/rete/mod.rs — added rules: Vec<Rule> field (init empty in new()/with_network(), cleared in reset()); overrode load_ruleset (clones ruleset.rules) and get_facts (returns working_facts.values().cloned()) on impl ExecutionEngine; rewrote execute_rules into the direct evaluator + fixpoint cap + once-per-rule coverage; added apply_action free fn; updated execute/run for the EngineResult return type; removed the stale per-nagram record_rule_eval in join_propagation; added 4 load_ruleset/get_facts unit tests in mod tests"

key-decisions:
  - "Action::UpdateFact.id is matched as a fact_type_id (first fact of that type) with an exact-FactId-key fast path. The plan's behavior uses id: \"scorecard\" (a fact type), so the fact_type fall-back is the common path; the exact-key path supports callers that pass a Uuid string. Documented as the keystone-scenario interpretation."
  - "Each rule fires at most once per execute() call (fired_set guard). This is the once-per-rule semantic that bounds the fixpoint loop at |rules| passes and makes coverage truthful. The cyclic-rule test therefore terminates at pass 1 (fired once, guard stops re-firing) rather than 'at exactly the max-iterations cap' — the once-per-rule guard is stronger and terminates sooner, which is the correct, safer behavior. The cap remains the ultimate safety net for pathological rule sets. Documented as a deviation from the plan's 'terminates at exactly the cap' wording."
  - "execute_rules return type changed from usize to EngineResult so fired_rule_ids propagate. execute() now delegates directly (self.execute_rules()); run() reads .rules_fired off the result."
  - "All 7 integration tests marked #[serial_test::serial] with CoverageTracking.clear() at start. The original single-serial coverage test failed (fire_count == 2) because non-serial sibling tests firing the same rule id ran concurrently and polluted the singleton. serial_test only serializes serial tests among themselves, so every test touching the singleton must carry the attribute."

patterns-established:
  - "Pattern: direct-evaluator loop (iterate rules, evaluate condition, apply actions, fixpoint to cap) as the minimal honest rule engine — bypasses placeholder network types without breaking the load_ruleset/get_facts contract a real network compiler can later implement behind."
  - "Pattern: once-per-rule coverage via post-loop HashSet iteration — never record inside the per-rule inner loop (Pitfall 6)."
  - "Pattern: serial_test::serial on ALL tests in a file that calls execute() (which records into the global CoverageTracking singleton), plus clear() at start — the isolation contract for singleton-touching integration tests."

requirements-completed: [EXEC-01, EXEC-02, MON-05]

# Coverage metadata (#1602) — per-deliverable traceability.
coverage:
  - id: D1
    description: "ReteEngine.rules: Vec<Rule> field + load_ruleset override (clones ruleset.rules) + get_facts override (returns working_facts.values().cloned()) (L-02 bridge)"
    requirement: "EXEC-01"
    verification:
      - kind: unit
        ref: "engine/src/engine/rete/mod.rs:210 (rules field), :527 (load_ruleset), :534 (get_facts); cargo test -p engine --features engine-rete --lib rete::tests (9/9 pass incl. load_ruleset_stores_rule_into_engine_rules, get_facts_returns_fact_after_assert_fact)"
        status: pass
    human_judgment: false
  - id: D2
    description: "ReteEngine::execute_rules rewritten into a real direct evaluator (D-04): iterates self.rules, evaluates rule.condition against working_facts via Condition::evaluate, applies matching rule.actions (evaluating Expression per UpdateFact field), chains to fixpoint under RRULES_MAX_CHAIN_ITERATIONS cap (D-07), returns EngineResult with fired_rule_ids populated"
    requirement: "EXEC-01"
    verification:
      - kind: unit
        ref: "engine/tests/rete_execution.rs::fires_on_matching_fact (keystone score+10: rules_fired >= 1, fired_rule_ids contains r1, get_facts shows score=10); ::fixpoint_chain_fires_both_rules (A enables B, both fire, score=20); ::cyclic_rule_set_terminates_without_panic (terminates, rules_fired==1)"
        status: pass
    human_judgment: false
  - id: D3
    description: "facts_after from post-execution working memory (Pitfall 11): get_facts returns the action-mutated working_facts, not echoed request input"
    requirement: "EXEC-02"
    verification:
      - kind: unit
        ref: "engine/tests/rete_execution.rs::get_facts_after_execute_shows_incremented_score (score==10 incremented, not 0 echoed)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Once-per-rule coverage recording (L-06 / Pitfall 6): fired/evaluated ids collected in HashSet, record_rule_fire/record_rule_eval called once per rule AFTER the fixpoint loop; stale per-nagram record in join_propagation removed"
    requirement: "MON-05"
    verification:
      - kind: unit
        ref: "engine/tests/rete_execution.rs::coverage_records_fire_count_one_for_single_fire (fire_count == 1, serial); ::coverage_records_eval_but_not_fire_for_non_matching_rule (evaluated==true, fired==false); grep record_rule_fire at rete/mod.rs:397 (after fixpoint loop, iterating fired_set — not inside inner loop)"
        status: pass
    human_judgment: false
  - id: D5
    description: "apply_action helper mirrors WorkingMemory CRUD on HashMap<FactId, Fact>; UpdateFact locates by exact FactId key then fact_type_id fall-back and evaluates each field's Expression against the working-memory snapshot; InsertFact/RetractFact/CallFunction/LogMessage handled"
    requirement: "EXEC-01"
    verification:
      - kind: unit
        ref: "engine/src/engine/rete/mod.rs:421 (fn apply_action); keystone + chain tests exercise UpdateFact with computed Expression"
        status: pass
    human_judgment: false

# Metrics
duration: 7min
completed: 2026-07-03
status: complete
---

# Phase 23 Plan 03: ReteEngine direct rule evaluator (keystone) Summary

**Rewrote the placeholder `ReteEngine::execute_rules` into a real direct evaluator that actually fires rules — the keystone of Phase 23. A `score := score + 10` rule now yields `rules_fired >= 1` and `get_facts()` showing `score == 10`. Coverage records once per fired rule (Pitfall 6 fixed), forward chaining iterates to fixpoint under `RRULES_MAX_CHAIN_ITERATIONS`, and `EngineResult.fired_rule_ids` is populated.**

## Performance

- **Duration:** 7 min (started 2026-07-03T06:41:21Z, completed 2026-07-03T06:48:49Z)
- **Tasks:** 2 (both TDD: RED -> GREEN)
- **Files modified:** 2 (1 source + 1 new test file)

## Accomplishments
- Added a `rules: Vec<Rule>` field to `ReteEngine` (initialized empty in `new()`/`with_network()`, cleared in `reset()`) and overrode the `load_ruleset` and `get_facts` trait methods: `load_ruleset` clones `ruleset.rules` into `self.rules` (the L-02 RuleSet->engine bridge, per-request clone of the shared snapshot), and `get_facts` returns `self.working_facts.values().cloned().collect()` (post-execution working memory for `facts_after`, Pitfall 11).
- Rewrote `execute_rules` from a counting placeholder (iterate action_nodes x join_nodes x nagrams, increment a counter, double-count coverage per nagram) into a real direct interpreter (D-04): iterate `self.rules`, evaluate each `rule.condition` against `self.working_facts` via `Condition::evaluate`, apply matching `rule.actions` to `working_facts` (evaluating each `UpdateFact` field's `Expression` via `Expression::evaluate`), chain to fixpoint under the `RRULES_MAX_CHAIN_ITERATIONS` cap (D-07, default 1000), and return an `EngineResult` with `fired_rule_ids` populated.
- Implemented `apply_action(action, working_facts)` mirroring `WorkingMemory`'s CRUD on `HashMap<FactId, Fact>`: `UpdateFact` locates the target by exact `FactId` key first then falls back to the first fact of the named `fact_type_id` (the keystone uses `id: "scorecard"`, a fact type), evaluates each field's `Expression` against a working-memory snapshot, and writes the computed `Value` into the fact's payload; `InsertFact`/`RetractFact` handled; `CallFunction`/`LogMessage` are no-ops in Phase 23.
- Fixed Pitfall 6 (coverage double-counting): fired and evaluated rule ids are collected in `HashSet<String>` during the fixpoint loop, and `CoverageTracking::record_rule_fire`/`record_rule_eval` are called exactly once per id AFTER the loop (not inside any inner loop). Removed the stale per-nagram `record_rule_eval` call in `join_propagation` (it always recorded against the first action node — a placeholder heuristic that the direct evaluator now owns correctly).
- Created `engine/tests/rete_execution.rs` with 7 serial-gated integration tests: the keystone `score := score + 10` scenario (rules_fired >= 1, score=10, fired_rule_ids contains r1), `get_facts_after_execute` shows the incremented score (Pitfall 11), non-matching condition fires zero, a two-rule fixpoint chain (A enables B, both fire, score=20), cyclic-rule termination without panic, and the Pitfall 6 regression (fire_count == 1 after one execute, evaluated-but-not-fired for non-matching).

## Task Commits

Each task followed the TDD RED -> GREEN cycle:

1. **Task 1: Add rules field + load_ruleset/get_facts impls on ReteEngine (L-02)**
   - `3266368d8` — test(23-03): add failing tests for load_ruleset/get_facts (RED — 2/3 fail against default Err/empty impls)
   - `c409677b1` — feat(23-03): implement rules field + load_ruleset/get_facts on ReteEngine (GREEN — 9/9 rete::tests pass)
2. **Task 2: Rewrite execute_rules into a direct evaluator with fixpoint cap and once-per-rule coverage (D-04, D-07, L-06)**
   - `087e581df` — test(23-03): add failing rete_execution integration tests (RED — 6/7 fail against placeholder execute_rules)
   - `781d544f2` — feat(23-03): rewrite execute_rules into real direct evaluator (GREEN — 7/7 pass after serial isolation fix)

**Plan metadata:** pending (docs: complete plan — committed after STATE.md/ROADMAP.md updates)

## Files Created/Modified
- `engine/src/engine/rete/mod.rs` — added `rules: Vec<Rule>` field (init empty in new()/with_network(), cleared in reset()); overrode `load_ruleset` (clones `ruleset.rules`) and `get_facts` (returns `working_facts.values().cloned()`) on `impl ExecutionEngine`; rewrote `execute_rules` into the direct evaluator + fixpoint cap + once-per-rule coverage (returns `EngineResult` with `fired_rule_ids`); added `apply_action` free fn; updated `execute`/`run` for the new return type; removed the stale per-nagram `record_rule_eval` in `join_propagation`; added 4 `load_ruleset`/`get_facts` unit tests in `mod tests`
- `engine/tests/rete_execution.rs` — new integration test file, 7 `#[serial_test::serial]` tests covering the keystone scenario, get_facts_after_execute, non-matching, fixpoint chain, cyclic termination, and the Pitfall 6 coverage regression

## Decisions Made
- **`Action::UpdateFact.id` is matched as a `fact_type_id` (first fact of that type) with an exact-FactId-key fast path.** The plan's behavior specifies `id: "scorecard"` (a fact type), so the fact_type fall-back is the common path; the exact-key path supports callers that pass a Uuid string. The plan's action text said "locate the fact in working_facts by id (the FactId)" which contradicts the behavior's `id: "scorecard"` — the behavior wins (it's the keystone scenario), and the dual-lookup covers both interpretations.
- **Each rule fires at most once per `execute()` call (fired_set guard).** This is the once-per-rule semantic that bounds the fixpoint loop at |rules| passes and makes coverage truthful. The cyclic-rule test therefore terminates at pass 1 (fired once, guard stops re-firing) rather than "at exactly the max-iterations cap" — the guard is stronger and terminates sooner, which is correct and safer. The cap remains the ultimate safety net.
- **`execute_rules` return type changed from `usize` to `EngineResult`** so `fired_rule_ids` propagate. `execute()` delegates directly; `run()` reads `.rules_fired` off the result.
- **All 7 integration tests marked `#[serial_test::serial]` with `CoverageTracking.clear()` at start.** The original single-serial coverage test failed (fire_count == 2) because non-serial sibling tests firing the same rule id ran concurrently and polluted the singleton. `serial_test` only serializes serial tests among themselves, so every test touching the singleton must carry the attribute.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Test isolation for the CoverageTracking singleton**
- **Found during:** Task 2 GREEN (coverage regression test failed with fire_count == 2)
- **Issue:** The plan specified `#[serial_test::serial]` only on the coverage regression test. But every test in the file calls `execute()`, which records into the process-global `CoverageTracking` singleton. `serial_test` only serializes serial tests among themselves — non-serial tests run concurrently with serial ones and pollute the singleton (the keystone test firing `r1` ran alongside the coverage test, producing fire_count == 2).
- **Fix:** Marked ALL 7 tests `#[serial_test::serial]` and added a `reset_coverage()` helper (`CoverageTracking.clear()`) called at the start of each test.
- **Files modified:** engine/tests/rete_execution.rs
- **Verification:** `cargo test -p engine --features engine-rete --test rete_execution` -> 7/7 pass, fire_count == 1.
- **Committed in:** `781d544f2` (Task 2 GREEN)

**2. [Rule 2 - Missing Critical] Removed stale per-nagram `record_rule_eval` in `join_propagation`**
- **Found during:** Task 2 implementation
- **Issue:** `join_propagation` (called from `assert_fact`) still called `CoverageTracking::record_rule_eval` against an arbitrary action node (a placeholder heuristic). With the direct evaluator now owning once-per-rule eval recording after the fixpoint loop, this stale call would double-record evals and pollute the singleton on every `assert_fact`.
- **Fix:** Removed the `record_rule_eval` call from `join_propagation` (left a comment noting the direct evaluator now owns recording). The network-propagation path is otherwise untouched.
- **Files modified:** engine/src/engine/rete/mod.rs
- **Committed in:** `781d544f2` (Task 2 GREEN)

**3. [Rule 3 - Blocking] Invalid `-x` flag in plan verification commands**
- **Found during:** Task verification
- **Issue:** The plan's `<verify><automated>` and `<verification>` use `cargo test ... -x`. The `-x` flag is not a valid `cargo test` argument (`error: unexpected argument '-x' found`). Same artifact as 23-01 and 23-02.
- **Fix:** Used the correct substring-filter form `cargo test -p engine --features engine-rete --test rete_execution` (no `-x`).
- **Files modified:** none (command-form fix only)

---

**Total deviations:** 3 auto-fixed (2 critical correctness, 1 blocking command-form). No scope creep — the delivered artifacts match the plan's `output` section exactly.

## Issues Encountered
None beyond the deviations above. The borrow-checker concern (iterating `self.rules` while mutating `self.working_facts`) was resolved with index-based iteration + an `actions.clone()` per firing (actions are small; cloning is O(actions) and acceptable). A cross-field borrow approach would also work but the clone is clearer.

## User Setup Required
None — no external service configuration, no new packages installed. All work is against the existing in-tree Rust codebase. `serial_test` is already a workspace dev-dep (`engine/Cargo.toml:21`).

## Next Phase Readiness
- The ReteEngine now actually fires rules. The keystone `score := score + 10` scenario passes at the engine level: `rules_fired >= 1`, `get_facts()` shows `score == 10`, `fired_rule_ids` populated, coverage recorded once per fired rule.
- **Plan 23-04** can extend the direct evaluator / Expression surface if scoped (the core D-04/D-05/D-06 build is done across 23-02 + 23-03).
- **Plan 23-05** can wire `POST /v1/execute` through the registry: `create_engine_by_name` -> `load_ruleset` (shared `Arc<RwLock<RuleSet>>`) -> `insert_fact` per request fact -> `execute()` -> serialize `facts_after` from `get_facts()` via the reverse `engine_value_to_json`. The `engine` param validates against `available_engines()` (truthful since 23-01). `rules_not_fired` = ruleset rule ids minus `EngineResult.fired_rule_ids`.
- **Plan 23-06** can mirror the direct evaluator + `load_ruleset`/`get_facts` onto `TreatEngine`/`LeapsEngine`/`PhreakEngine` (D-01/D-02) — the evaluator loop is engine-agnostic (operates on `self.rules` + `self.working_facts`), so a shared helper or per-engine mirror both work.
- No blockers. The `load_ruleset`/`get_facts`/`execute_rules`/`apply_action` contracts are stable for downstream plans.

## Requirement Status Note

`EXEC-01`, `EXEC-02`, and `MON-05` are marked complete in this plan's frontmatter per the SUMMARY template (copy from the plan's `requirements` field). The engine-level keystone scenario passes (rules fire truthfully, facts_after reflects mutations, coverage records once per fired rule). The full requirements — truthful `rules_fired`/`facts_after`/coverage from the live `/v1/execute` endpoint — are only end-to-end satisfied when Plan 23-05's API integration tests pass against the wired HTTP handler. REQUIREMENTS.md status is left for the phase verifier to close after 23-05 lands.

---
*Phase: 23-execute-evaluator-wiring*
*Completed: 2026-07-03*

## Self-Check: PASSED

- `engine/src/engine/rete/mod.rs` exists on disk: FOUND
- `engine/tests/rete_execution.rs` exists on disk: FOUND
- Task 1 RED commit `3266368d8`: FOUND
- Task 1 GREEN commit `c409677b1`: FOUND
- Task 2 RED commit `087e581df`: FOUND
- Task 2 GREEN commit `781d544f2`: FOUND
- `cargo test -p engine --features engine-rete --lib rete::tests` exits 0 — 9/9 pass
- `cargo test -p engine --features engine-rete --test rete_execution` exits 0 — 7/7 pass (keystone score+10, fixpoint chain, cyclic termination, Pitfall 6 coverage regression)
- `cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` exits 0
- `cargo check -p api` exits 0
- TDD gate compliance: each task has a `test(...)` RED commit followed by a `feat(...)` GREEN commit