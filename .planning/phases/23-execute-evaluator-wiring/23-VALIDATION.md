---
phase: 23
slug: execute-evaluator-wiring
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-02
validated: 2026-07-03
---

# Phase 23 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: `23-RESEARCH.md` § Validation Architecture (all claims verified against live code with file:line evidence).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` via `cargo test`; `actix-test` (api dev-dep, `api/Cargo.toml:48`) for HTTP integration tests; `serial_test` (workspace dev-dep, `engine/Cargo.toml:21`) for `CoverageTracking` singleton isolation |
| **Config file** | `Cargo.toml` (workspace + `engine/` + `api/`); engine cargo features `engine-rete,engine-treat,engine-leaps,engine-phreak` |
| **Quick run command** | `cargo check -p api && cargo test -p engine --lib -x` |
| **Full suite command** | `cargo test --workspace --features engine-rete,engine-treat,engine-leaps,engine-phreak` |
| **Estimated runtime** | ~60–120 seconds (workspace suite across all four engine features) |

---

## Sampling Rate

- **After every task commit:** Run `cargo check -p api && cargo test -p engine --lib -x`
- **After every plan wave:** Run `cargo test --workspace --features engine-rete,engine-treat,engine-leaps,engine-phreak`
- **Before `/gsd-verify-work`:** Full suite must be green; all four ROADMAP success criteria (below) must pass.
- **Max feedback latency:** ~120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 23-05·T2 (scaffold 23-04·T2) | 23-05 | 3 | EXEC-01 | T-23-12 | Per-request `Box<dyn ExecutionEngine>` via `spawn_blocking`; no shared `&mut` engine (Pitfall 1) | integration | `cargo test -p api --test execute_integration -- execute_fires_real_rule -x` | ✅ | ✅ green |
| 23-05·T2 (scaffold 23-04·T2) | 23-05 | 3 | EXEC-02 | — | `facts_after` serialized from post-execution working memory, not echoed request input (Pitfall 11) | integration | `cargo test -p api --test execute_integration -- facts_after_not_echoed -x` | ✅ | ✅ green |
| 23-05·T2 (scaffold 23-04·T2) | 23-05 | 3 | ENGINE-07 | T-23-13 | Unknown `engine` name validated against `available_engines()` → HTTP 400 (input validation) | integration | `cargo test -p api --test execute_integration -- engine_param_validation -x` | ✅ | ✅ green |
| 23-05·T1/T2 (scaffold 23-04·T2) | 23-05 | 3 | ENGINE-08 | — | Omitted `engine` resolves to `RRULES_DEFAULT_ENGINE` env → `selector.current_engine()` (config default) | integration | `cargo test -p api --test execute_integration -- default_engine_resolution -x` | ✅ | ✅ green |
| 23-04·T2 + 23-05·T2 | 23-04 / 23-05 | 2 / 3 | MON-05 | — | Coverage recorded once per fired rule per execution (HashSet → single `record_rule_fire`, no double-count, Pitfall 6) | unit + integration (serial) | `cargo test -p engine --test coverage -- record_once_per_rule -x` + `cargo test -p api --test execute_integration -- coverage_auto_populated -x` | ✅ | ✅ green |
| 23-03·T2 | 23-03 | 2 | (bridge) | — | ReteEngine direct evaluator fires on matching fact (D-04) | unit | `cargo test -p engine --test rete_execution -- fires_on_matching_fact -x` | ✅ | ✅ green |
| 23-03·T1 | 23-03 | 2 | (bridge) | — | `get_facts` exposes post-execution working memory (L-02) | unit | `cargo test -p engine --test rete_execution -- get_facts_after_execute_shows_incremented_score -x` | ✅ | ✅ green |
| 23-01·T1 | 23-01 | 1 | (compile) | — | E0433 `engine::`→`crate::` import bug fixed across all four engine modules | build | `cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` | n/a | ✅ green |
| 23-06·T2 | 23-06 | 3 | (multi-engine) | — | Treat/Leaps/Phreak mirror the direct evaluator — all four engines honestly fire rules (D-01/D-02) | integration (serial) | `cargo test -p engine --test engine_execution_tests -x` (`treat`/`leaps`/`phreak_fires_score_plus_ten`) | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*
*Task ID / Plan / Wave columns populated from each PLAN.md `<task>` block. Threat Ref linked from each plan's `<threat_model>`/STRIDE register (23-05: T-23-12 DoS, T-23-13 Tampering).*

*Additional `rete_execution.rs` coverage (bonus, not requirement-mapped): `non_matching_condition_fires_zero`, `fixpoint_chain_fires_both_rules` (D-07 fixpoint cap), `cyclic_rule_set_terminates_without_panic`, `coverage_records_fire_count_one_for_single_fire`, `coverage_records_eval_but_not_fire_for_non_matching_rule` — all green.*

---

## Wave 0 Requirements

- [x] `api/tests/execute_integration.rs` — five integration tests for EXEC-01/02, ENGINE-07/08, MON-05 (POST /v1/execute against a `score := score + 10` rule → `rules_fired >= 1`, `facts_after.score` incremented; GET /v1/metrics/coverage shows `fired == true`). ✅ green — Wave 0 RED scaffolds (23-04·T2) promoted to real by 23-05·T2.
- [x] `engine/tests/rete_execution.rs` — `load_ruleset`/`execute`/`get_facts` unit behavior (7 tests). ✅ green — delivered by 23-03 (ReteEngine direct-evaluator rewrite).
- [x] `engine/tests/coverage.rs` — `serial_test`-gated `record_once_per_rule` asserting `record_rule_fire` called exactly once per fired rule per execution (Pitfall 6 regression). ✅ green — landed 23-04·T2.
- [x] Fixture: a ruleset with one rule `r1` matching `fact_type: "scorecard", field: "score"` with an action that increments `score` by 10 — constructed in-test via `RuleSet::add_rule` + the `Expression` action-model extension (D-06, 23-02). ✅ — `score_plus_10_rule()` helper used across all four engine keystone suites.

*Existing `cargo test` infrastructure covers the framework; Wave 0 adds the phase-specific test files listed above.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification (per `23-RESEARCH.md` Validation Architecture).*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 120s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-07-03 (post-execution nyquist audit — all requirements COVERED, zero gaps)

---

## Validation Audit 2026-07-03

| Metric | Count |
|--------|-------|
| Gaps found | 0 |
| Resolved | 0 |
| Escalated | 0 |
| Requirements audited | 5 (EXEC-01, EXEC-02, ENGINE-07, ENGINE-08, MON-05) |
| Requirements COVERED | 5 |
| Requirements PARTIAL | 0 |
| Requirements MISSING | 0 |

**Method:** State A audit of the plan-time draft. Cross-referenced each requirement to its test file + function name (confirmed via `grep` over `api/tests/execute_integration.rs`, `engine/tests/{rete_execution,coverage,engine_execution_tests}.rs`) and to its green run (confirmed by the `gsd-verifier` pass: `cargo test -p api --test execute_integration` 5/5, `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak --test coverage --test rete_execution --test engine_execution_tests` 11/11, `cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` exit 0). No gaps → no `gsd-nyquist-auditor` spawn required (workflow Step 3→6 short-circuit). `nyquist_compliant: true` set.