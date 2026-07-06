---
phase: 25
slug: real-reload-jobstore
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-03
---

# Phase 25 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Test map sourced from `25-RESEARCH.md` § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust built-in) + `actix_web::test::TestRequest` |
| **Config file** | `api/Cargo.toml` `[package]` + `[features]` (engine-rete enabled Phase 23) |
| **Quick run command** | `cargo test -p api --test execute_integration -x` |
| **Full suite command** | `cargo test -p api && cargo test -p engine` |
| **Estimated runtime** | ~45 seconds (api) + ~20 seconds (engine) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p api --test execute_integration -x`
- **After every plan wave:** Run `cargo test -p api && cargo test -p engine`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

> Task IDs are provisional — the planner assigns final IDs. Each test below is a Wave 0 RED stub that turns GREEN when the corresponding task lands.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 25-01-01 | 01 | 0 | DEPLOY-05 / SC-1 | — | N/A | integration | `cargo test -p api --test execute_integration -- reload_then_execute_against_refreshed_rules -x` | ❌ W0 | ⬜ pending |
| 25-01-02 | 01 | 0 | DEPLOY-05 / SC-1 | — | N/A | integration | `cargo test -p api --test execute_integration -- async_reload_job_transitions -x` | ❌ W0 | ⬜ pending |
| 25-01-03 | 01 | 0 | SC-3 / L-04 | — | N/A | integration | `cargo test -p api --test execute_integration -- engine_generation_increments_after_reload -x` | ❌ W0 | ⬜ pending |
| 25-01-04 | 01 | 0 | EXEC-03 / SC-2 | — | N/A | integration | `cargo test -p api --test execute_integration -- batch_truthful_aggregation_one_failing_item -x` | ❌ W0 | ⬜ pending |
| 25-01-05 | 01 | 0 | L-05 / D-01 | — | N/A | integration | `cargo test -p api --test execute_integration -- in_flight_run_uses_old_snapshot -x` | ❌ W0 | ⬜ pending |
| 25-01-06 | 01 | 0 | SC-2 | — | N/A | integration | `cargo test -p api --test execute_integration -- batch_panicking_item_isolated -x` | ❌ W0 | ⬜ pending |
| 25-01-07 | 01 | 0 | D-03a | — | N/A | unit | `cargo test -p api --test execute_integration -- reload_empty_dir_warns_not_crashes -x` | ❌ W0 | ⬜ pending |
| 25-01-08 | 01 | 0 | DEPLOY-05 / D-04 | — | N/A | integration | `cargo test -p api --test execute_integration -- sync_reload_swaps_and_bumps_generation -x` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `api/tests/execute_integration.rs` — add the 8 RED tests above (SC-1/SC-2/SC-3 + reload + jobs + sync-swap + empty-dir). Reuse the Phase 23/24 `score_plus_10_rule` fixture; the `shared_ruleset()` helper (`execute_integration.rs:82`) must build a `SharedRuleSet` (generation 0) instead of a bare `RuleSet`.
- [ ] Test fixture helper `write_rule_file(dir, rule)` — serializes a `Rule` to a JSON `.rules` file so the keystone can edit it (`+10` → `+20`) and reload.
- [ ] No framework install — `cargo test` + `actix_web::test` already in use (`api/tests/execute_integration.rs` exists from Phase 23).

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| — | — | — | — |

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending