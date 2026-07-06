---
phase: 26-ruleexecutiontracker-singleton-dead-rule-live-data
plan: 02
subsystem: monitoring
tags: [rust, monitoring, dead-rules, e2e, keystone, three-surfaces, prometheus, coverage, singleton-recording, tdd, wave0-red-green, rete, direct-eval]

# Dependency graph
requires:
  - "26-01: RuleExecutionTracker LazyLock<Mutex<TrackerState>> singleton with associated-function API (record_evaluation, record_fire, find_dead_rules, clear) + deprecated new()"
provides:
  - "Live recording into the RuleExecutionTracker singleton from BOTH evaluator call sites (direct_eval.rs for Treat/Leaps/Phreak AND rete/mod.rs for Rete — C1 correction)"
  - "dead_rules_endpoint reads live singleton data via RuleExecutionTracker::find_dead_rules() (no per-request RuleExecutionTracker::new() — Pitfall 3 fixed)"
  - "SC-1/SC-2/SC-3 keystone e2e tests pinning the dead-rules panel, coverage surface, and three-surfaces agreement contract"
  - "id→name HashMap pattern (Pitfall 4) for sourcing rule_name from rules: &[Rule] / self.rules at the recording call sites"
affects: [dead-rules-endpoint, execute_rules-live-data, coverage-tracking, prometheus-metrics]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Singleton recording at post-fixpoint call sites: build id→name HashMap<&String,&String> once before the loops, then call RuleExecutionTracker::record_evaluation / record_fire adjacent to CoverageTracking inside the existing for-id loops"
    - "C1 dual-site recording: Rete keeps its own inlined execute_rules (does NOT delegate to direct_eval.rs), so both sites must be wired independently with the same id→name pattern"
    - "Endpoint singleton read: RuleExecutionTracker::find_dead_rules() returns owned Vec<DeadRuleInfo>; map to Vec<String> via .into_iter().map(|i| i.rule_id) (move, no clone)"

key-files:
  created: []
  modified:
    - "engine/src/engine/direct_eval.rs — RuleExecutionTracker::record_evaluation + record_fire wired at post-fixpoint block (Treat/Leaps/Phreak coverage)"
    - "engine/src/engine/rete/mod.rs — RuleExecutionTracker::record_evaluation + record_fire wired at post-fixpoint block (C1 — Rete inlined execute_rules)"
    - "api/src/metrics.rs — dead_rules_endpoint rewritten to read singleton (dropped deprecated new() + #[allow]; dropped .clone() on owned info.rule_id)"
    - "api/tests/execute_integration.rs — three SC-1/SC-2/SC-3 keystone e2e tests + never_fires_rule r2 fixture + two_rule_shared_ruleset + init_two_rule_app! macro + /v1/metrics/dead-rules route registration + reset_test_env clears RuleExecutionTracker"
    - "api/tests/metrics_integration.rs — stale 'always returns []' comment corrected to reflect live singleton read"

key-decisions:
  - "C1 honored: both direct_eval.rs AND rete/mod.rs wired independently — Rete does NOT delegate to direct_eval.rs"
  - "Pitfall 4 id→name HashMap built once before the recording loops (not inside) — evaluated_set/fired_set are HashSet<String> of ids only"
  - "D-02 three-surfaces agreement by construction: all three surfaces (Prometheus rrules_execute_rules_fired_total, coverage fire_count, dead-rules exclusion) derive from the shared fired_set"
  - "C4 out-of-contract: EngineMetrics.rules_fired left as a per-engine gauge — no edits to engine/src/engine/metrics.rs or self.metrics.record_rule_fired() call sites"
  - "D-05 out-of-scope: no edits to dead_rules.rs HealthRegistry, coverage/history/mod.rs, coverage/mod.rs CoverageTracker"

patterns-established:
  - "Dual-site singleton recording with id→name lookup: let id_to_name: HashMap<&String,&String> = rules.iter().map(|r| (&r.id,&r.name)).collect(); before the for-id loops"

requirements-completed: [MON-06]

coverage:
  - id: G1
    description: "RuleExecutionTracker recording wired at both evaluator call sites (C1): direct_eval.rs:105-110 for Treat/Leaps/Phreak AND rete/mod.rs:393-398 for Rete"
    requirement: MON-06
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs#keystone_dead_rules — r2-inclusion assertion proves recording is live"
        status: pass
      - kind: integration
        ref: "api/tests/execute_integration.rs#keystone_coverage — r1.fired + r2 not fired"
        status: pass
      - kind: integration
        ref: "api/tests/execute_integration.rs#keystone_three_surfaces_agree — three surfaces agree on fire count"
        status: pass
    human_judgment: false
  - id: G2
    description: "dead_rules_endpoint reads RuleExecutionTracker::find_dead_rules() from the singleton (no RuleExecutionTracker::new())"
    requirement: MON-06
    verification:
      - kind: static
        ref: "grep api/src/metrics.rs: RuleExecutionTracker::new() returns 0 matches; RuleExecutionTracker::find_dead_rules() returns 1 match"
        status: pass
      - kind: integration
        ref: "api/tests/metrics_integration.rs#dead_rules_returns_valid_json_array — passes (valid JSON array from singleton read)"
        status: pass
    human_judgment: false

# Metrics
duration: ~25min
completed: 2026-07-03
status: complete
---

# Phase 26 Plan 02: Dead-Rule Live Data Summary

**Wired RuleExecutionTracker recording into BOTH evaluator call sites (direct_eval.rs for Treat/Leaps/Phreak AND rete/mod.rs for Rete — C1 correction), rewrote dead_rules_endpoint to read live singleton data via find_dead_rules(), and pinned SC-1/SC-2/SC-3 with three keystone e2e tests using the r2-unfired-rule trick — dead-rules panel now returns live data and all three metric surfaces agree on fire count.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-07-03
- **Completed:** 2026-07-03
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Added the SC-1/SC-2/SC-3 keystone e2e tests (Wave 0 RED) with a never-fires `r2` sentinel rule (condition `score == 999`, LogMessage no-op action) and a `two_rule_shared_ruleset` / `init_two_rule_app!` fixture. The r2-INCLUSION assertion in `keystone_dead_rules` is the RED→GREEN proof — today the singleton is empty so `find_dead_rules()` returns `[]` and `r2` is absent.
- Registered `/v1/metrics/dead-rules` in `configure_test_routes` (the existing test App only registered `/v1/metrics/coverage`).
- Extended `reset_test_env()` to clear the `RuleExecutionTracker` singleton alongside `CoverageTracking` (L-05 isolation for the second process-wide singleton).
- Wired `RuleExecutionTracker::record_evaluation` + `record_fire` at the post-fixpoint recording block in `direct_eval.rs` (covers Treat/Leaps/Phreak) with an id→name HashMap built once from `rules: &[Rule]` (Pitfall 4).
- Wired the same recording at the post-fixpoint block in `rete/mod.rs` (C1 — Rete keeps its own inlined `execute_rules` and does NOT delegate to `direct_eval.rs`), with the id→name HashMap built from `self.rules`.
- Rewrote `dead_rules_endpoint` to read `RuleExecutionTracker::find_dead_rules()` from the singleton — dropped the deprecated `RuleExecutionTracker::new()` local + `#[allow(deprecated, unused_variables)]` attribute (resolves the Plan 26-01 deprecation warning, D-04) and dropped the `.clone()` on `info.rule_id` (the `Vec<DeadRuleInfo>` is owned, so `info.rule_id` moves).
- Updated the stale "always returns []" / "honest deferred tech debt" comment in `metrics_integration.rs:91` to reflect that dead-rules now reads live singleton data.

## Task Commits

Each task was committed atomically (TDD RED → GREEN):

1. **Task 1: Wave 0 RED — add SC-1/SC-2/SC-3 keystone e2e tests** — `85e418d11` (test)
2. **Task 2: GREEN — wire recording at both evaluator sites + rewrite dead_rules_endpoint** — `4c7bf6213` (feat)

## Files Created/Modified
- `engine/src/engine/direct_eval.rs` — RuleExecutionTracker recording at post-fixpoint block (Treat/Leaps/Phreak)
- `engine/src/engine/rete/mod.rs` — RuleExecutionTracker recording at post-fixpoint block (C1 — Rete inlined execute_rules)
- `api/src/metrics.rs` — dead_rules_endpoint rewritten to read singleton (dropped deprecated new() + .clone())
- `api/tests/execute_integration.rs` — three keystone e2e tests + r2 fixture + two-rule ruleset + route registration + reset_test_env clears RuleExecutionTracker
- `api/tests/metrics_integration.rs` — stale comment corrected

## Decisions Made
- Honored C1 (load-bearing correction): wired BOTH `direct_eval.rs` AND `rete/mod.rs` independently. Rete keeps its own inlined `execute_rules` and does NOT delegate to `direct_eval.rs`, so a single recording site would leave Rete uncovered.
- Built the id→name HashMap ONCE before the recording loops (Pitfall 4) — `evaluated_set`/`fired_set` are `HashSet<String>` of ids only, and `Rule.id`/`Rule.name` are `pub` at `rules.rs:18,20`, so `rules.iter().map(|r| (&r.id, &r.name)).collect()` is the source.
- Left `EngineMetrics.rules_fired` untouched (C4 — out-of-contract for SC-3; it is a per-engine gauge, not a contract surface). The three SC-3 surfaces all derive from the shared `fired_set` (D-02), so agreement is by construction.
- Left parallel systems untouched (D-05 — `dead_rules.rs` HealthRegistry, `coverage/history/mod.rs`, `coverage/mod.rs` CoverageTracker are out of scope).

## Deviations from Plan

### Notes on RED State

**1. [Plan reconciliation] keystone_coverage and keystone_three_surfaces_agree passed GREEN at RED**
- **Found during:** Task 1 (RED verification)
- **Issue:** The plan's `<done>` criteria anticipated all three keystone tests failing at RED. In practice, `keystone_coverage` passes GREEN from Phase 23's coverage wiring (the plan acknowledged this possibility: "This may already pass from Phase 23's coverage wiring — if so, it is a regression guard"). `keystone_three_surfaces_agree` also passes at RED because the dead-rules r1-EXCLUSION assertion is trivially satisfied by `[]` (empty list excludes everything), and the Prometheus + coverage surfaces are already wired from Phases 23-24.
- **Resolution:** Not a bug — the load-bearing RED assertion (`keystone_dead_rules` r2-INCLUSION) fails as expected, proving the panel is broken before Task 2. The test suite exits non-zero at RED. The other two tests serve as regression guards (keystone_coverage) and agreement checks that become meaningful once live recording is wired (keystone_three_surfaces_agree). No code change needed; documented for transparency.
- **Files modified:** none
- **Verification:** `cargo test -p api --test execute_integration keystone` at Task 1 → 1 failed (keystone_dead_rules), 2 passed. At Task 2 → 3 passed.

### Auto-fixed Issues

None — Plan 26-01's documented Rule 3 deviation (the `RuleExecutionTracker::find_dead_rules()` associated-function form at `api/src/metrics.rs:381`) was already in place; Task 2 removed the `new()` local + `#[allow]` attribute and dropped `.clone()` per the plan's `<prior_wave_context>` instructions.

**Total deviations:** 0 auto-fixed; 1 plan-reconciliation note (RED state partial).

## Issues Encountered
None.

## User Setup Required
None — no external service configuration required. All dependencies are existing workspace deps.

## Next Phase Readiness
- The dead-rules panel now returns live data from the `RuleExecutionTracker` singleton.
- The three metric surfaces (Prometheus `rrules_execute_rules_fired_total`, `/v1/metrics/coverage` `fire_count`, `/v1/metrics/dead-rules` exclusion) agree on fire count by construction (D-02).
- The `id→name HashMap` pattern (Pitfall 4) is established for future recording-site additions.
- The keystone e2e tests pin the SC-1/SC-2/SC-3 contract — any future regression that breaks recording or the endpoint rewrite will fail these tests.

## TDD Gate Compliance
- RED gate: `test(26-02): add SC-1/SC-2/SC-3 keystone e2e (RED)` — `85e418d11` — exists in git log. `keystone_dead_rules` fails on the r2-inclusion assertion (the singleton is empty, `find_dead_rules()` returns `[]`, `r2` is absent). The test suite exits non-zero.
- GREEN gate: `feat(26-02): wire RuleExecutionTracker recording at both evaluator sites + singleton endpoint (GREEN)` — `4c7bf6213` — exists in git log after the RED commit. All 3 keystone tests pass GREEN.
- REFACTOR gate: not required — no refactor needed; the GREEN implementation is clean.

## Self-Check: PASSED

- FOUND: engine/src/engine/direct_eval.rs (recording wired)
- FOUND: engine/src/engine/rete/mod.rs (recording wired — C1)
- FOUND: api/src/metrics.rs (dead_rules_endpoint rewritten)
- FOUND: api/tests/execute_integration.rs (three keystone e2e tests)
- FOUND: api/tests/metrics_integration.rs (stale comment corrected)
- FOUND: 85e418d11 (Task 1 RED commit)
- FOUND: 4c7bf6213 (Task 2 GREEN commit)
- grep api/src/metrics.rs: `RuleExecutionTracker::new()` = 0 matches; `RuleExecutionTracker::find_dead_rules()` = 1 match
- grep both evaluator files: `RuleExecutionTracker::record_fire` present in direct_eval.rs AND rete/mod.rs
- engine/src/engine/metrics.rs NOT modified (C4 honored)

---
*Phase: 26-ruleexecutiontracker-singleton-dead-rule-live-data*
*Completed: 2026-07-03*