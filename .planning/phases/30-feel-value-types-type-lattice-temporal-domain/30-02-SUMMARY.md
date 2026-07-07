---
phase: 30-feel-value-types-type-lattice-temporal-domain
plan: 02
subsystem: engine
tags: [feel, dmn, rust_decimal, decimal128, numeric, tck, conformance]

# Dependency graph
requires:
  - phase: 30-feel-value-types-type-lattice-temporal-domain
    plan: 01
    provides: engine/src/dmn/feel/numeric.rs stub, Value::Number(rust_decimal::Decimal) variant, vendored TCK subset under engine/tests/fixtures/tck/, A1 precision-audit gate (rust_decimal locked)
provides:
  - engine/src/dmn/feel/numeric.rs — feel_numeric_eq(&Value, &Value) -> Option<bool> (null-propagating FEEL numeric equality; Number/Number + Number/Integer cross-type; no number↔string)
  - 0.1 + 0.2 = 0.3 headline DMN-08 proof in rust_decimal::Decimal (test_decimal_add)
  - engine/tests/feel_tck_numeric.rs — integration test running 17 vendored 0100-arithmetic numeric vectors against Value::Number via feel_numeric_eq, with a 13-category SKIP_LIST audit trail
affects: [30-03-temporal, 30-04-lattice-regression, 31-feel-parser-evaluator]

# Tech tracking
tech-stack:
  added: []
  patterns: [TDD RED stub→GREEN on feel_numeric_eq, minimal binary-arithmetic evaluator for TCK value-domain proof, quick-xml NsReader streaming parse of TCK DMN+test XML, documented conformance-gap SKIP_LIST audit trail]

key-files:
  created:
    - engine/tests/feel_tck_numeric.rs
  modified:
    - engine/src/dmn/feel/numeric.rs

key-decisions:
  - "D-30-02a: feel_numeric_eq returns Option<bool> (not bool) so the Phase 31 three-valued-logic operators consume the None (null-propagated) result directly without a re-wrap"
  - "D-30-02b: Number↔Integer cross-type equality via Decimal::from(i) (spec §10.3.2.2 numeric domain treats integers and numbers as one domain); Number↔String returns None (NO implicit conversion — Phase 31 explicit string()/number() built-in, Pitfall 6)"
  - "D-30-02c: eval_numeric_expr is a deliberately minimal LHS OP RHS evaluator for +,-,*,/ on decimal literals — it is NOT the Phase 31 FEEL evaluator; it exists only to prove the Value::Number value domain against the TCK arithmetic vectors"
  - "D-30-02d: Division-by-zero vectors (100/0) are skipped with a documented SKIP_LIST reason, NOT mapped to null — the Err→null mapping is Phase 31 (research Security Domain, T-30-02-02 accept)"
  - "D-30-02e: The 1141-1144 round-up/down/half-up/half-down built-ins are entirely Phase 31 (DMN-17, Pitfall 2); the test only confirms the value-domain equality 5.5==5.5 the TCK 017_a expectation reduces to (research A3)"
  - "D-30-02f: The 1100-feel-decimal-function vectors all use the decimal(n,scale) built-in (Phase 31 DMN-17) — the entire directory is in the SKIP_LIST; no value-domain assertion is possible without implementing the rounding built-in"

patterns-established:
  - "FEEL numeric equality is null-propagating (Option<bool>) and decimal-based (rust_decimal::Decimal::PartialEq, never f64)"
  - "TCK conformance tests hand-parse vendored XML with quick-xml NsReader (no external TCK runner crate); parse failures panic with file+case-id for visible conformance gaps"
  - "Out-of-scope TCK vectors are documented in a SKIP_LIST constant with category + reason — the conformance gap is explicit, never silent"

requirements-completed: [DMN-08]

# Coverage metadata
coverage:
  - id: N1
    description: "0.1 + 0.2 = 0.3 holds in rust_decimal::Decimal via Value::Number (DMN-08 headline proof)"
    requirement: "DMN-08"
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/numeric.rs#test_decimal_add"
        status: pass
      - kind: automated
        ref: "cargo test -p engine --lib dmn::feel::numeric::test_decimal_add"
        status: pass
    human_judgment: false
  - id: N2
    description: "feel_numeric_eq: null propagation, Number/Number, Number/Integer cross-type, no Number↔String"
    requirement: "DMN-08"
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/numeric.rs#test_feel_numeric_eq_number + test_feel_numeric_eq_cross_type + test_feel_numeric_eq_null + test_float_not_equal"
        status: pass
      - kind: automated
        ref: "cargo test -p engine --lib dmn::feel::numeric:: (5 passed)"
        status: pass
    human_judgment: false
  - id: N3
    description: "Vendored TCK 0100-arithmetic numeric vectors pass against Value::Number via feel_numeric_eq with a documented SKIP_LIST"
    requirement: "DMN-08"
    verification:
      - kind: integration
        ref: "engine/tests/feel_tck_numeric.rs#feel_tck_numeric_vectors (17 in-scope vectors passed)"
        status: pass
      - kind: integration
        ref: "engine/tests/feel_tck_numeric.rs#feel_tck_scale_boundary_value_domain (1141 017_a 5.5==5.5)"
        status: pass
      - kind: integration
        ref: "engine/tests/feel_tck_numeric.rs#feel_tck_skip_list_documented (13 categories)"
        status: pass
      - kind: automated
        ref: "cargo test -p engine --test feel_tck_numeric (3 passed)"
        status: pass
    human_judgment: false
  - id: N4
    description: "Full feature-gated engine regression green after the numeric module (no regression from Plan 02)"
    requirement: "DMN-08"
    verification:
      - kind: automated
        ref: "cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak (0 failed)"
        status: pass
    human_judgment: false

# Metrics
duration: 10min
completed: 2026-07-07
status: complete
---

# Phase 30 Plan 02: FEEL Decimal128 Numeric Domain Summary

**FEEL numeric equality (`feel_numeric_eq`) with null propagation and Number↔Integer cross-type promotion on `rust_decimal::Decimal`, the headline `0.1 + 0.2 = 0.3` DMN-08 proof, and 17 vendored OMG TCK 0100-arithmetic numeric vectors passing against `Value::Number` with a 13-category conformance-gap SKIP_LIST**

## Performance

- **Duration:** ~10 min
- **Tasks:** 2
- **Files modified:** 2 (1 source + 1 integration test)

## Accomplishments
- Implemented `feel_numeric_eq(&Value, &Value) -> Option<bool>` in `engine/src/dmn/feel/numeric.rs` — the FEEL numeric-domain equality helper. Null propagates (`None`); `Number`↔`Number` compares via `rust_decimal::Decimal::PartialEq` (decimal, not f64); `Number`↔`Integer` promotes the integer via `Decimal::from(i)` per spec §10.3.2.2; all other type pairings (String, Bool, Float, Date, …) return `None` — NO `number↔string` implicit conversion (Phase 31 explicit built-in, Pitfall 6).
- Proved the DMN-08 headline: `dec!(0.1) + dec!(0.2) == dec!(0.3)` holds in `rust_decimal::Decimal` (`test_decimal_add`), with a negative-contrast `test_float_not_equal` showing `0.1_f64 + 0.2_f64 != 0.3_f64` (the binary-float problem FEEL avoids).
- Built `engine/tests/feel_tck_numeric.rs` — an integration test that hand-parses the vendored OMG DMN TCK `0100-arithmetic` DMN + test-case XML with `quick-xml::NsReader`, evaluates the 17 in-scope decimal `+`/`-`/`*`/`/` vectors via a minimal `eval_numeric_expr` helper, and asserts `feel_numeric_eq(actual, expected) == Some(true)` for each. All 17 pass.
- Added a 13-category `SKIP_LIST` audit trail documenting every out-of-scope TCK vector category (exponentiation `**`, duration/temporal/string/list/context/null/bool operands, division-by-zero, the `decimal()` built-in, the `round up/down/half-up/half-down` built-ins) with the Phase 31 / Plan 03 reason — the conformance gap is explicit, never silent.
- Confirmed the 1141 `round up(5.5, 6176)` TCK 017_a expectation reduces to the value-domain equality `5.5 == 5.5` under `feel_numeric_eq` (research A3) — the rounding built-in itself is Phase 31 DMN-17 (Pitfall 2).
- Full feature-gated regression `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` green (0 failures) — zero regression from the numeric module.

## Task Commits

Each task followed the TDD RED→GREEN cycle (plan per-task `tdd="true"`):

1. **Task 1 (30-02-01): FEEL numeric equality + 0.1+0.2=0.3 proof** —
   - `e67e6b147` (test) RED: failing numeric tests (feel_numeric_eq stubbed → None)
   - `444875a82` (feat) GREEN: implement feel_numeric_eq (5 unit tests green)
2. **Task 2 (30-02-02): Vendored TCK numeric conformance vectors** —
   - `44c64bc39` (test) RED: failing TCK integration test (eval_numeric_expr stubbed → 0 vectors)
   - `592b793a4` (feat) GREEN: implement eval_numeric_expr (17 vectors pass)

## Files Created/Modified
- `engine/src/dmn/feel/numeric.rs` — `feel_numeric_eq` implementation + 5 unit tests (test_decimal_add, test_float_not_equal, test_feel_numeric_eq_number, test_feel_numeric_eq_cross_type, test_feel_numeric_eq_null)
- `engine/tests/feel_tck_numeric.rs` — integration test: `parse_dmn_expressions`, `parse_test_cases`, `eval_numeric_expr` helpers + 3 tests (feel_tck_numeric_vectors, feel_tck_scale_boundary_value_domain, feel_tck_skip_list_documented) + `SKIP_LIST` + `TCK_PROVENANCE`

## Decisions Made
- D-30-02a: `feel_numeric_eq` returns `Option<bool>` (not `bool`) so the Phase 31 three-valued-logic operators consume the `None` (null-propagated) result directly without a re-wrap.
- D-30-02b: `Number↔Integer` cross-type equality via `Decimal::from(i)` (spec §10.3.2.2 numeric domain); `Number↔String` returns `None` — NO implicit conversion (Phase 31 explicit `string()`/`number()` built-in, Pitfall 6).
- D-30-02c: `eval_numeric_expr` is a deliberately minimal `LHS OP RHS` evaluator for `+,-,*,/` on decimal literals — NOT the Phase 31 FEEL evaluator; it exists only to prove the `Value::Number` value domain against the TCK arithmetic vectors.
- D-30-02d: Division-by-zero vectors (`100/0`) are skipped with a documented `SKIP_LIST` reason, NOT mapped to null — the `Err→null` mapping is Phase 31 (research Security Domain, T-30-02-02 accept disposition).
- D-30-02e: The 1141-1144 round-up/down/half-up/half-down built-ins are entirely Phase 31 (DMN-17, Pitfall 2); the test only confirms the value-domain equality `5.5==5.5` the TCK 017_a expectation reduces to (research A3).
- D-30-02f: The 1100-feel-decimal-function vectors all use the `decimal(n, scale)` built-in (Phase 31 DMN-17) — the entire directory is in the `SKIP_LIST`; no value-domain assertion is possible without implementing the rounding built-in.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] quick-xml BytesText API: `.unescape()` → `.decode()`**
- **Found during:** Task 2 (RED)
- **Issue:** The plan said to hand-parse TCK XML with quick-xml; the initial implementation used `BytesText::unescape()` which does not exist in quick-xml 0.40 (the method is `.decode()`). The parser.rs reference uses `t.decode()` for `Event::Text`.
- **Fix:** Replaced both `t.unescape().unwrap_or_default()` calls with `t.decode().unwrap_or_default()`.
- **Files modified:** engine/tests/feel_tck_numeric.rs
- **Verification:** `cargo test -p engine --test feel_tck_numeric` compiles and passes.
- **Committed in:** 44c64bc39 (RED) / 592b793a4 (GREEN)

**2. [Rule 1 - Bug] SKIP_LIST smoke-check joined only categories, not reasons**
- **Found during:** Task 2 (RED)
- **Issue:** The `feel_tck_skip_list_documented` test built its `joined` string from only the category (first tuple element), so the `assert!(joined.contains("exponentiation"))` check failed — "exponentiation" appears in the REASON, not the category. This was a test-logic bug causing a non-stub RED failure.
- **Fix:** Changed `map(|(c, _)| c)` to `map(|(c, r)| format!("{c}: {r}"))` so both category and reason substrings are visible to the `contains` checks.
- **Files modified:** engine/tests/feel_tck_numeric.rs
- **Verification:** `feel_tck_skip_list_documented` passes; RED failure isolated to the stub (clean RED gate).
- **Committed in:** 44c64bc39 (RED)

---

**Total deviations:** 2 auto-fixed (both Rule 1 test-logic bugs)
**Impact on plan:** Both fixes were test-harness implementation details (quick-xml API surface, assertion substring scope). No scope creep — the deliverables (feel_numeric_eq semantics, the 0.1+0.2=0.3 proof, the 17 passing TCK vectors, the SKIP_LIST audit trail) match the plan exactly.

## Issues Encountered
- `cargo test -p engine` (no features) fails to compile feature-gated integration tests — this is the pre-existing baseline condition documented in Plan 01 (engine algorithm modules are feature-gated). The regression check uses `--features engine-rete,engine-treat,engine-leaps,engine-phreak`, which is green. The `feel_tck_numeric` integration test does NOT need the engine features and passes standalone.
- The two pre-existing `unused_imports` warnings in `engine/src/chaining/mode_selector.rs` and another lib file are unrelated to this plan (pre-existing, out of scope per deviation rules).

## User Setup Required
None — no new dependencies. `rust_decimal` 1.42 (with `serde`, `macros`, `maths` features) and `quick-xml` 0.40 were already in `engine/Cargo.toml` from earlier phases.

## Next Phase Readiness
- The FEEL numeric value domain is locked: `Value::Number(rust_decimal::Decimal)` with `feel_numeric_eq` for equality. Phase 31's evaluator can build `+`/`-`/`*`/`/`/`**` operators on this foundation, mapping division-by-zero and overflow `Err` → `Value::Null` (research Security Domain).
- The `decimal()`, `round up/down/half-up/half-down`, `floor`, `ceiling`, `abs`, `number()`, `string()` built-ins are Phase 31 (DMN-13/DMN-17). The `SKIP_LIST` records exactly which TCK vector categories await them.
- Plan 03 (temporal) can proceed independently — it modifies `temporal.rs`, not `numeric.rs`.
- Plan 04 (lattice + regression) can proceed independently — it modifies `types.rs` and audits Rete-path wildcards.

## TDD Gate Compliance
Both tasks followed the RED→GREEN cycle (plan per-task `tdd="true"`). Git log confirms:
1. `test(30-02): ...` commits (RED) — `e67e6b147`, `44c64bc39`
2. `feat(30-02): ...` commits (GREEN) — `444875a82`, `592b793a4`

RED gates failed as expected (stubbed `feel_numeric_eq` → 2 tests fail; stubbed `eval_numeric_expr` → 0 vectors evaluated). GREEN gates pass. No refactor commits needed.

## Self-Check: PASSED

**Files created/modified (verified present):**
- FOUND: engine/src/dmn/feel/numeric.rs
- FOUND: engine/tests/feel_tck_numeric.rs

**Commits (verified in git log):**
- FOUND: e67e6b147 (test 30-02 RED Task 1)
- FOUND: 444875a82 (feat 30-02 GREEN Task 1)
- FOUND: 44c64bc39 (test 30-02 RED Task 2)
- FOUND: 592b793a4 (feat 30-02 GREEN Task 2)

---
*Phase: 30-feel-value-types-type-lattice-temporal-domain*
*Completed: 2026-07-07*