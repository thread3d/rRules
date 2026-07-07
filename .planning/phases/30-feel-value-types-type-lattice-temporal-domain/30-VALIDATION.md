---
phase: 30
slug: feel-value-types-type-lattice-temporal-domain
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-07
---

# Phase 30 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from `30-RESEARCH.md` §Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `proptest` 1.11.0 (workspace dev-dep, already present) |
| **Config file** | `engine/Cargo.toml` `[dev-dependencies]` (proptest already declared) |
| **Quick run command** | `cargo test -p engine --lib data_types:: dmn::feel::` |
| **Full suite command** | `cargo test -p engine` |
| **Estimated runtime** | ~25 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p engine --lib data_types:: dmn::feel::`
- **After every plan wave:** Run `cargo test -p engine`
- **Before `/gsd-verify-work`:** Full suite green + TCK numeric subset + TCK temporal subset green
- **Max feedback latency:** 25 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 30-01-01 | 01 | 1 | DMN-08 | — / V5 | `0.1 + 0.2 = 0.3` in `rust_decimal::Decimal` | unit | `cargo test -p engine --lib dmn::feel::types::test_decimal_add` | ❌ W0 | ⬜ pending |
| 30-01-02 | 01 | 1 | DMN-08 | — | TCK numeric vectors pass (vendored subset: `0100-arithmetic`, `1100-feel-decimal-function`) | integration | `cargo test -p engine --test feel_tck_numeric` | ❌ W0 | ⬜ pending |
| 30-01-03 | 01 | 1 | DMN-08 | — | No vendored TCK value vector exceeds 29 significant digits (confirms assumption A1; if any does, switch to `bigdecimal`) | smoke (Wave-0 gate) | `cargo test -p engine --test tck_precision_audit` | ❌ W0 | ⬜ pending |
| 30-02-01 | 02 | 1 | DMN-09 | V5 | `@""` literals for all 5 temporal types parse + round-trip | property | `cargo test -p engine --lib dmn::feel::temporal::tests -- --nocapture` | ❌ W0 | ⬜ pending |
| 30-02-02 | 02 | 1 | DMN-09 | — | `DateTime<Tz> + YearsMonthsDuration` via `0100-arithmetic` Melbourne vector round-trips with IANA tz preserved | integration | `cargo test -p engine --test feel_tck_temporal` | ❌ W0 | ⬜ pending |
| 30-02-03 | 02 | 1 | DMN-09 | — | Jan 31 + 1 month → Feb 28/29 (last-day-of-month semantics) | unit | `cargo test -p engine --lib dmn::feel::temporal::test_month_edge` | ❌ W0 | ⬜ pending |
| 30-03-01 | 03 | 2 | DMN-10 | — | `conforms` is reflexive + transitive; `Null <: T <: Any` for all `T` | property | `cargo test -p engine --lib dmn::feel::types::conformance_proptest` | ❌ W0 | ⬜ pending |
| 30-03-02 | 03 | 2 | DMN-10 | — | `equivalent` is reflexive / symmetric / transitive | property | `cargo test -p engine --lib dmn::feel::types::equivalence_proptest` | ❌ W0 | ⬜ pending |
| 30-03-03 | 03 | 2 | DMN-10 | — | Exactly 3 implicit conversions (to-singleton-list, from-singleton-list, conforms-to) behave per spec | unit + property | `cargo test -p engine --lib dmn::feel::types::implicit_convert` | ❌ W0 | ⬜ pending |
| 30-03-04 | 03 | 2 | DMN-10 | — | `1 + "2"` → `null` (NO number↔string implicit conversion — that is a Phase 31 built-in) | unit | `cargo test -p engine --lib dmn::feel::types::test_no_number_string_conversion` | ❌ W0 | ⬜ pending |
| 30-04-01 | 04 | 2 | Regression | — | Rete rule-execution path unchanged: `validation.rs` / `rules.rs` / `testing/mod.rs` compile + existing tests pass | regression | `cargo test -p engine` (full suite) | ✅ (existing) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `engine/src/dmn/feel/mod.rs` — module wiring (`pub mod types; pub mod temporal;`)
- [ ] `engine/src/dmn/feel/types.rs` — `FeelType`, `conforms`, `equivalent`, `implicit_convert`, `resolve_type_ref` + property tests
- [ ] `engine/src/dmn/feel/temporal.rs` — `TimeValue`, `DateTimeValue`, `YearsMonthsDuration`, `parse_at_literal`, `format_value`, `add_months_zoned` + round-trip tests
- [ ] `engine/tests/fixtures/tck/` — vendored subset of TCK `0100-arithmetic`, `1100-feel-decimal-function`, `1141`–`1144` XML+DMN (record OMG TCK provenance/license)
- [ ] `engine/tests/feel_tck_numeric.rs` — runs vendored TCK numeric vectors against `Value::Number`
- [ ] `engine/tests/feel_tck_temporal.rs` — runs `0100-arithmetic` temporal + duration vectors
- [ ] `engine/tests/tck_precision_audit.rs` — Wave-0 gate scanning vendored TCK for >29-digit values
- [ ] `cargo add chrono-tz --package engine` — the single new dependency

*If none: "Existing infrastructure covers all phase requirements." — N/A here, all infrastructure is new in Wave 0.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| OMG TCK provenance + license recorded for vendored subset | DMN-08/09 | License/attribution is a documentation artifact, not a runtime behavior | Confirm `engine/tests/fixtures/tck/README.md` lists source repo, commit, and OMG TCK license terms |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 25s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending