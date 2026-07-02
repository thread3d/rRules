---
phase: 23
slug: execute-evaluator-wiring
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-02
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
| TBD | TBD | TBD | EXEC-01 | — | Per-request `Box<dyn ExecutionEngine>` via `spawn_blocking`; no shared `&mut` engine (Pitfall 1) | integration | `cargo test -p api --test execute_integration -- execute_fires_real_rule -x` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | EXEC-02 | — | N/A | integration | `cargo test -p api --test execute_integration -- facts_after_not_echoed -x` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | ENGINE-07 | — | Unknown `engine` name → HTTP 400 (input validation) | integration | `cargo test -p api --test execute_integration -- engine_param_validation -x` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | ENGINE-08 | — | N/A | integration | `cargo test -p api --test execute_integration -- default_engine_resolution -x` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | MON-05 | — | Coverage recorded once per fired rule per execution (no double-count, Pitfall 6) | unit + integration (serial) | `cargo test -p engine --test coverage -- recordonce_per_rule -x` and `cargo test -p api --test execute_integration -- coverage_auto_populated -x` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | (bridge) | — | N/A | unit | `cargo test -p engine --test rete_execution -- fires_on_matching_fact -x` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | (bridge) | — | N/A | unit | `cargo test -p engine --test rete_execution -- get_facts_after_execute -x` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | (compile) | — | N/A | build | `cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` | n/a | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*
*Task ID / Plan / Wave columns are populated from PLAN.md tasks during planning/execution. Threat Ref linked from each plan's `<threat_model>` block.*

---

## Wave 0 Requirements

- [ ] `api/tests/execute_integration.rs` — stubs for EXEC-01/02, ENGINE-07/08, MON-05 integration (POST /v1/execute against a `score := score + 10` rule → `rules_fired >= 1`, `facts_after.score` incremented; GET /v1/metrics/coverage shows `fired == true`).
- [ ] `engine/tests/rete_execution.rs` — stubs for `load_ruleset`/`execute`/`get_facts` unit behavior.
- [ ] `engine/tests/coverage.rs` (or extend) — `serial_test`-gated stub for `record_rule_fire` called exactly once per fired rule per execution (Pitfall 6 regression).
- [ ] Fixture: a ruleset with one rule `r1` matching `fact_type: "scorecard", field: "score"` with an action that increments `score` by 10 — loaded by the integration test (temp rules dir or constructed in-test via `RuleSet::add_rule`). *Note: requires the `Expression` action-model extension (D-06) to represent `score := score + 10`.*

*Existing `cargo test` infrastructure covers the framework; Wave 0 adds the phase-specific test files listed above.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification (per `23-RESEARCH.md` Validation Architecture).*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending