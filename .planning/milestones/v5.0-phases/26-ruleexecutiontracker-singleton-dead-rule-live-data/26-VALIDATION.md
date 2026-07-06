---
phase: 26
slug: ruleexecutiontracker-singleton-dead-rule-live-data
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-03
---

# Phase 26 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: `26-RESEARCH.md` §"Validation Architecture".

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `serial_test` (workspace dev-dep) + `actix_web::test` for HTTP integration |
| **Config file** | `Cargo.toml` (workspace) — per-crate `cargo test -p <crate>` |
| **Quick run command** | `cargo test -p engine --lib monitoring::detection && cargo test -p api --test execute_integration` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p engine --lib monitoring::detection && cargo test -p api --test execute_integration`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd-verify-work`:** Full workspace suite must be green
- **Max feedback latency:** ~45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 26-01-01 | 01 | 1 | MON-06 | — | N/A | unit | `cargo test -p engine --lib monitoring::detection` | ✅ (existing, rewrite to singleton) | ⬜ pending |
| 26-01-02 | 01 | 1 | MON-06 | — | N/A | unit | `cargo test -p engine --lib monitoring::detection::tests::test_once_per_rule` | ❌ W0 | ⬜ pending |
| 26-02-01 | 02 | 1 | MON-06 (SC-1) | — | N/A | integration (e2e) | `cargo test -p api --test execute_integration keystone_dead_rules -- --serial` | ❌ W0 | ⬜ pending |
| 26-02-01 | 02 | 1 | MON-06 (SC-2) | — | N/A | integration (e2e) | `cargo test -p api --test execute_integration keystone_coverage -- --serial` | ❌ W0 | ⬜ pending |
| 26-02-01 | 02 | 1 | MON-06 (SC-3) | — | N/A | integration (e2e) | `cargo test -p api --test execute_integration keystone_three_surfaces_agree -- --serial` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `api/tests/execute_integration.rs` — add `keystone_dead_rules`, `keystone_coverage`, `keystone_three_surfaces_agree` tests (SC-1/SC-2/SC-3) — RED first
- [ ] `engine/src/monitoring/detection.rs` tests — rewrite the 6 existing tests to singleton API + `#[serial]` + `clear()`; add `test_once_per_rule`
- [ ] `api/tests/metrics_integration.rs:91` — update the stale "always returns []" comment (optional: upgrade assertion to non-empty after execute)

*No framework install needed — `serial_test` already a workspace dev-dep.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| {none} | — | — | — |

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < ~45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending