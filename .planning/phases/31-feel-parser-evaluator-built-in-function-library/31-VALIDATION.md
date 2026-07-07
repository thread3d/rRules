---
phase: 31
slug: feel-parser-evaluator-built-in-function-library
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-07
---

# Phase 31 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from `31-RESEARCH.md` § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `proptest 1.11.0` (workspace dev-dep) + snapshot/golden tests |
| **Config file** | `engine/Cargo.toml` `[dev-dependencies]` (proptest already present from Phase 30-04) |
| **Quick run command** | `cargo test -p engine --lib dmn::feel::` |
| **Full suite command** | `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` |
| **Estimated runtime** | ~30–60 seconds (lib FEEL subset); full suite ~120–180s |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p engine --lib dmn::feel::`
- **After every plan wave:** Run `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak`
- **Before `/gsd-verify-work`:** Full suite must be green + DMN-12 truth-table unit tests green + vendored TCK subset green + Rete regression green
- **Max feedback latency:** ~60 seconds (lib subset); ~180 seconds (full suite)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 31-01-01 | 01 | 1 | DMN-11 | — | N/A | unit + property | `cargo test -p engine --lib dmn::feel::parser` | ❌ W0 | ⬜ pending |
| 31-01-02 | 01 | 1 | DMN-11 | — | N/A | unit | `cargo test -p engine --lib dmn::feel::parser::error_recovery` | ❌ W0 | ⬜ pending |
| 31-02-01 | 02 | 1 | DMN-12 | — | N/A | unit (MANDATORY) | `cargo test -p engine --lib dmn::feel::eval::dmn12_` | ❌ W0 | ⬜ pending |
| 31-02-02 | 02 | 1 | DMN-12 | — | N/A | unit + property | `cargo test -p engine --lib dmn::feel::eval::truth_tables` | ❌ W0 | ⬜ pending |
| 31-02-03 | 02 | 1 | DMN-12 | — | No `==`/`Ord` reuse in feel::eval | smoke | `cargo test -p engine --lib dmn::feel::eval::no_rust_eq` | ❌ W0 | ⬜ pending |
| 31-03-01 | 03 | 2 | DMN-13 | — | N/A | unit + property | `cargo test -p engine --lib dmn::feel::eval::arithmetic` | ❌ W0 | ⬜ pending |
| 31-03-02 | 03 | 2 | DMN-13 | — | N/A | unit + property | `cargo test -p engine --lib dmn::feel::eval::datetime_arith` | ❌ W0 | ⬜ pending |
| 31-03-03 | 03 | 2 | DMN-13 | — | Div-by-zero / overflow → null | unit | `cargo test -p engine --lib dmn::feel::eval::numeric_edge` | ❌ W0 | ⬜ pending |
| 31-03-04 | 03 | 2 | DMN-13 | — | `1 + "2"` → null | unit | `cargo test -p engine --lib dmn::feel::eval::string_concat` | ❌ W0 | ⬜ pending |
| 31-04-01 | 04 | 2 | DMN-14 | — | N/A | unit + property | `cargo test -p engine --lib dmn::feel::eval::control` | ❌ W0 | ⬜ pending |
| 31-05-01 | 05 | 3 | DMN-15 | — | N/A | unit + snapshot | `cargo test -p engine --lib dmn::feel::eval::boxed` | ❌ W0 | ⬜ pending |
| 31-05-02 | 05 | 3 | DMN-15 | — | Cycle detection | unit + property | `cargo test -p engine --lib dmn::feel::eval::context_topo` | ❌ W0 | ⬜ pending |
| 31-06-01 | 06 | 3 | DMN-16 | — | `external` rejected → null + warning | unit | `cargo test -p engine --lib dmn::feel::eval::external_rejected` | ❌ W0 | ⬜ pending |
| 31-06-02 | 06 | 3 | DMN-16 | — | N/A | unit | `cargo test -p engine --lib dmn::feel::eval::lambdas` | ❌ W0 | ⬜ pending |
| 31-07-01 | 07 | 4 | DMN-17 (part 1) | — | N/A | unit + property | `cargo test -p engine --lib dmn::feel::builtins::conversion dmn::feel::builtins::string dmn::feel::builtins::list dmn::feel::builtins::numeric` | ❌ W0 | ⬜ pending |
| 31-08-01 | 08 | 4 | DMN-17 (part 2) | — | N/A | unit | `cargo test -p engine --lib dmn::feel::builtins::datetime dmn::feel::builtins::context dmn::feel::builtins::boolean dmn::feel::builtins::range dmn::feel::builtins::equality` | ❌ W0 | ⬜ pending |
| 31-08-02 | 08 | 4 | DMN-17 | — | `is` never returns null (Pitfall 6) | unit | `cargo test -p engine --lib dmn::feel::builtins::is_never_null` | ❌ W0 | ⬜ pending |
| 31-09-01 | 09 | 4 | DMN-18 | — | 30-cell × 10k = ~30 parses not 300k | bench | `cargo test -p engine --lib dmn::feel::cache::perf -- --nocapture` | ❌ W0 | ⬜ pending |
| 31-09-02 | 09 | 4 | DMN-18 | — | Evaluator takes `&FeelExpression`, never `&str` | compile | `cargo build -p engine` (no `evaluate(&str,...)` overload) | ❌ W0 | ⬜ pending |
| 31-09-03 | 09 | 4 | DMN-18 | — | IR loader pre-parses all cells | integration | `cargo test -p engine --test feel_cache_wiring` | ❌ W0 | ⬜ pending |
| 31-REG | — | — | Regression | — | Rete path unchanged | regression | `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` | ✅ (existing) | ⬜ pending |
| 31-TCK | — | — | TCK subset | — | N/A | integration | `cargo test -p engine --test feel_tck_subset` | ❌ W0 (full TCK → Phase 35) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `cargo add dashmap regex --package engine` — two dependency installs (verified legitimate in research)
- [ ] `engine/src/dmn/feel/ast.rs` — `FeelExpression` enum + `Span` + `QuantKind` (DMN-11)
- [ ] `engine/src/dmn/feel/parser.rs` — Lexer + recursive-descent/Pratt parser, dual entry points, error recovery (DMN-11)
- [ ] `engine/src/dmn/feel/eval.rs` — `evaluate(&FeelExpression, &Scope)`, private `cmp` module, `Scope` type (DMN-12/13/14/15)
- [ ] `engine/src/dmn/feel/builtins.rs` — `BuiltinFn` type + `HashMap<&str, BuiltinFn>` registry skeleton (DMN-17)
- [ ] `engine/src/dmn/feel/coercion.rs` — explicit FEEL operator/builtin coercions (DMN-13)
- [ ] `engine/src/dmn/feel/cache.rs` (or fold into `parser.rs`) — `FeelAstCache = DashMap<String, Arc<FeelExpression>>` (DMN-18)
- [ ] `engine/src/dmn/feel/types.rs` — extend `FeelFunction` with `body`/`captured`/`param_names` (DMN-16)
- [ ] `engine/tests/fixtures/tck/feel/` — vendored OMG TCK FEEL subset (loader + runner scaffold)
- [ ] `engine/tests/feel_tck_subset.rs` — TCK harness scaffold (full passage deferred to Phase 35)
- [ ] `engine/tests/feel_cache_wiring.rs` — IR loader pre-parse integration test

*All infrastructure is new in Wave 0 across the 9 plans — no existing FEEL parser/evaluator exists in `engine/src/dmn/feel/` (Phase 30 delivered only `types.rs`/`numeric.rs`/`temporal.rs` value-domain foundation).*

---

## Manual-Only Verifications

*All phase behaviors have automated verification. The DMN-12 three-valued-logic truth tables (`"a"=1`→null, `null<3`→null, `null=null`→true, `not(null)`→null) are MANDATORY automated unit tests, not manual checks.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 180s (full suite)
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending