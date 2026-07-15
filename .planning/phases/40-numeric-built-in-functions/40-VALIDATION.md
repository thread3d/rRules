---
phase: 40
slug: numeric-built-in-functions
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-15
---

# Phase 40 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `cargo test` |
| **Config file** | `engine/Cargo.toml` |
| **Quick run command** | `cargo test -p engine --lib dmn::feel::builtins` |
| **Full suite command** | `cargo test -p engine` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p engine --lib dmn::feel::builtins`
- **After every plan wave:** Run `cargo test -p engine`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 40-01-01 | 01 | 1 | FEEL-01 | unit | `cargo test -p engine --lib dmn::feel::builtins` | ⬜ pending |
| 40-01-02 | 01 | 1 | FEEL-03, FEEL-08 | unit | `cargo test -p engine --lib dmn::feel::builtins` | ⬜ pending |
| 40-02-01 | 02 | 2 | FEEL-07 | unit | `cargo test -p engine --lib dmn::feel::builtins` | ⬜ pending |
| 40-03-01 | 03 | 3 | FEEL-02, FEEL-04, FEEL-05, FEEL-06, FEEL-09, FEEL-10 | unit | `cargo test -p engine` | ⬜ pending |
| 40-03-02 | 03 | 3 | FEEL-10 | integration | `cargo test -p engine --test tck_sweep` | ⬜ pending |

---

## Wave 0 Requirements

- [ ] `engine/src/dmn/feel/builtins.rs` — existing test infrastructure covers all phase requirements

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| TCK evaluator layer pass | FEEL-01..FEEL-10 | Requires full DMN parse+evaluate pipeline | Run `cargo test --test tck_sweep` and verify 14 numeric vectors pass evaluator layer |

---

## Validation Sign-Off

- [ ] All tasks have automated verify
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
