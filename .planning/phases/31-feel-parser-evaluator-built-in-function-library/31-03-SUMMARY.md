---
phase: 31-feel-parser-evaluator-built-in-function-library
plan: 03
subsystem: dmn/feel
tags: [feel, operators, arithmetic, string-concat, date-time, duration, coercion, dmn, dmn-13, decimal128, dst-correct]
requires:
  - phase: 31-01
    provides: FeelExpression AST, parser, lexer, FeelRange carrier
  - phase: 31-02
    provides: evaluate dispatch skeleton, private cmp module (feel_equals/feel_compare), Scope
  - phase: 30-03
    provides: add_months_zoned, add_days_time_zoned (DST-correct temporal helpers)
provides:
  - feel-arithmetic-operators
  - feel-string-concat
  - feel-in-range-membership
  - feel-date-time-arithmetic
  - feel-duration-operators
  - feel-coercion-module
  - feel-pow-right-associative
affects: [31-04, 31-05, 31-06, 31-07, 31-08, 31-09]

tech-stack:
  added: []
  patterns:
    - decimal128-numeric-binop-no-f64
    - null-on-div-by-zero-overflow-no-result-no-panic
    - no-implicit-number-string-conversion-operator-coercion
    - date-time-arithmetic-delegates-to-phase30-temporal-helpers
    - checked-add-months-for-last-day-of-month-semantics
    - proptest-macro-gated-with-cfg-test-for-dev-dependency
    - tdd-red-green-per-task

key-files:
  created: []
  modified:
    - engine/src/dmn/feel/eval.rs
    - engine/src/dmn/feel/coercion.rs
    - engine/src/dmn/feel/parser.rs

key-decisions:
  - "coercion.rs: coerce_to_number (Number/Integer → Decimal; String/Bool/Null → None — NO implicit number↔string per Pitfall 6), coerce_to_string (String only), coerce_to_bool (Bool only)"
  - "eval_add dispatch order: null-check → datetime_add → string-concat → numeric. Date-time arithmetic tried before string-concat so Date+YMD→Date works; Number+String falls through to numeric→Null"
  - "eval_pow uses rust_decimal::MathematicalOps::checked_powd — handles integer exponents exactly, non-integer via exp/ln, overflow→None→Null"
  - "Between arm fixed: ge = o != Ordering::Less, le = o != Ordering::Greater (feel_compare(a,b) returns ordering of a vs b, so a>=b ↔ o!=Less)"
  - "Parser lexer fix: second `.` of `..` followed by digit no longer consumed as leading-dot decimal (prev-byte check)"
  - "parse_list_or_range + LParen arm detect Range from parse_postfix's greedy `..` handling and fix inclusivity from brackets"
  - "proptest::proptest! macro invocations gated with #[cfg(test)] — proptest is a dev-dependency unavailable in non-test builds"
  - "Date-Date→DaysTimeDuration (not YearsMonthsDuration); years_months_between deferred to 31-08 date-time builtins"

patterns-established:
  - "numeric_binop helper: coerce_to_number both operands, apply Decimal op, None→Null (div-by-zero, overflow, type-mismatch all map to Null)"
  - "datetime_add/datetime_sub: match on (Value, Value) pairs, delegate to temporal helpers for DST-correct computation, None for type-mismatch → Null"
  - "eval_add/eval_sub: null-check → datetime dispatch → string-concat (Add only) → numeric fallback"
  - "add_months_to_date: chrono checked_add_months/checked_sub_months (Jan 31 + 1M → Feb 28 spec-correct)"

requirements-completed: [DMN-13]

coverage:
  - id: D1
    description: "Arithmetic operators (+,-,*,/,**) via Decimal128 with div-by-zero/overflow→Null; 2**3**2=512 (right-assoc); no f64"
    requirement: DMN-13
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#arithmetic_add"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#arithmetic_sub"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#arithmetic_mul"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#arithmetic_div"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#arithmetic_div_decimal"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#arithmetic_pow_right_associative_512"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#arithmetic_pow_2_to_10"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#arithmetic_unary_neg"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#arithmetic_unary_neg_integer_value"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#numeric_edge_div_by_zero_integer"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#numeric_edge_div_by_zero_decimal"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#numeric_edge_null_operand"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#numeric_edge_type_mismatch_yields_null"
        status: pass
      - kind: property
        ref: "engine/src/dmn/feel/eval.rs#arithmetic_div_identity"
        status: pass
    human_judgment: false
  - id: D2
    description: "String concatenation via + on two Strings; Number+String→Null (no implicit conversion)"
    requirement: DMN-13
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#string_concat_two_strings"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#string_concat_number_plus_string_yields_null"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#string_concat_null_operand"
        status: pass
    human_judgment: false
  - id: D3
    description: "Between desugaring (a>=b and a<=c) and In range-membership with inclusivity"
    requirement: DMN-13
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#between_in_range_true"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#between_out_of_range_false"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#in_range_inclusive_true"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#in_range_inclusive_boundary"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#in_range_exclusive_false"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#in_range_outside_false"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#in_range_null_operand_yields_null"
        status: pass
    human_judgment: false
  - id: D4
    description: "Date-time arithmetic matrix: Date+YMD→Date, DateTimeTz+DTD→DateTimeTz (IANA preserved, DST-correct), Time+DTD→Time, Date-Date→DTD, DTD+DTD→DTD, YMD+YMD→YMD"
    requirement: DMN-13
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#datetime_arith_date_plus_ymd"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#datetime_arith_datetime_plus_dtd_iana_preserved"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#datetime_arith_date_minus_date"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#datetime_arith_dtd_plus_dtd"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#datetime_arith_ymd_plus_ymd"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#datetime_arith_null_plus_duration"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#datetime_arith_type_mismatch_yields_null"
        status: pass
      - kind: property
        ref: "engine/src/dmn/feel/eval.rs#datetime_arith_identity_dtd_zero"
        status: pass
    human_judgment: false
  - id: D5
    description: "coercion.rs: explicit FEEL coercions (coerce_to_number, coerce_to_string, coerce_to_bool) — NO implicit number↔string"
    requirement: DMN-13
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/coercion.rs#coerce_to_number_number"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/coercion.rs#coerce_to_number_integer_promotes"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/coercion.rs#coerce_to_number_string_is_none_no_implicit"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/coercion.rs#coerce_to_number_bool_is_none"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/coercion.rs#coerce_to_number_null_is_none"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/coercion.rs#coerce_to_string_string"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/coercion.rs#coerce_to_string_number_is_none_no_implicit"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/coercion.rs#coerce_to_string_null_is_none"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/coercion.rs#coerce_to_bool_bool"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/coercion.rs#coerce_to_bool_number_is_none"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/coercion.rs#coerce_to_bool_null_is_none"
        status: pass
    human_judgment: false

duration: ~20min
completed: 2026-07-07
status: complete
---

# Phase 31 Plan 03: FEEL Operators + coercion.rs Summary

Arithmetic (+,-,*,/,**) via Decimal128 with div-by-zero/overflow→Null, string concatenation via `+` (no implicit number↔string), `between`/`in` range-membership desugaring, date-time arithmetic matrix (Date+YMD→Date, DateTimeTz+DTD→DateTimeTz with IANA zone preservation, duration+duration), and the `coercion.rs` explicit FEEL coercions module — all reusing Phase 30's DST-correct temporal helpers and the 31-02 cmp module.

## Performance

- **Duration:** ~20 min
- **Tasks:** 2
- **Files modified:** 3 (eval.rs, coercion.rs, parser.rs)

## Accomplishments
- `coercion.rs`: three explicit FEEL coercions — `coerce_to_number` (Number/Integer→Decimal; String/Bool/Null→None, NO implicit number↔string per Pitfall 6), `coerce_to_string` (String→Some; else None), `coerce_to_bool` (Bool→Some; else None). 10 unit tests.
- Arithmetic operators (+,-,*,/,**) via `numeric_binop` helper: coerce both operands to Decimal, apply op, None→Value::Null (div-by-zero, overflow, type-mismatch). `eval_pow` uses `rust_decimal::MathematicalOps::checked_powd` (integer exponents exact, non-integer via exp/ln, overflow→Null). Right-associativity is in the AST shape from 31-01's parser; eval walks the tree so `2**3**2 = 2**9 = 512`.
- String concatenation via `+`: `eval_add` dispatches string-concat when both operands are `Value::String`. `Number+String` falls through to numeric→Null (coerce_to_number(String)=None). No implicit number↔string conversion.
- `between` desugaring: `a between b and c` → `a>=b and a<=c` via `feel_compare` + `and_3vl`. Fixed a Rule 1 bug from 31-02 where ge/le comparisons were swapped.
- `in` range-membership: `eval_in` checks `feel_compare(value, start)` and `feel_compare(value, end)` against inclusivity flags via `and_3vl`. Null operand → Null. Full unary-test `in` semantics deferred to 31-04.
- Date-time arithmetic matrix: `datetime_add`/`datetime_sub` handle Date+YMD→Date (checked_add_months: Jan 31+1M→Feb 28), DateTimeTz+DTD→DateTimeTz (via `add_days_time_zoned` for IANA — DST gap→Null, overlap→earliest), DateTimeTz+YMD→DateTimeTz (via `add_months_zoned`), Time+DTD→Time, Date-Date→DTD, DateTimeTz-DateTimeTz→DTD, Time-Time→DTD, DTD+DTD→DTD, YMD+YMD→YMD. All null operands → Null. Type-mismatched pairs → Null.
- IANA zone preservation: `@"2021-01-01T10:00:00@Australia/Melbourne" + @"P1D"` produces `DateTimeTz(Iana(...))` with the Melbourne zone retained (Pitfall 3 mitigation).
- Parser fixes: lexer no longer consumes second `.` of `..` as leading-dot decimal when followed by digit; `parse_list_or_range` and `LParen` arm detect Range from `parse_postfix`'s greedy `..` handling and fix inclusivity from brackets.
- `proptest::proptest!` macro invocations gated with `#[cfg(test)]` (dev-dependency unavailable in non-test builds).
- 42 new tests (10 coercion + 24 Task 1 + 8 Task 2); 352 lib tests pass; 377 with engine-rete features (zero Rete regression).

## Task Commits

Each task was committed atomically (TDD RED → GREEN per task):

1. **Task 1: Arithmetic + string-concat + in-range + coercion.rs (tdd)** - `27a0134ea` (test: RED) → `ff0db6c89` (feat: GREEN)
2. **Task 2: Date-time arithmetic + duration operators (tdd)** - `33f2e69b2` (test: RED) → `d5b83d318` (feat: GREEN)

## Files Created/Modified
- `engine/src/dmn/feel/eval.rs` - Real arithmetic BinOp arms (Add/Sub/Mul/Div/Pow via numeric_binop + Decimal128), eval_add (datetime→string-concat→numeric dispatch), eval_sub (datetime→numeric), eval_in (range membership), datetime_add/datetime_sub helpers, add_months_to_date/add_months_to_naive_dt helpers, Between arm ge/le fix, proptest cfg-test gating. 32 new tests (24 Task 1 + 8 Task 2).
- `engine/src/dmn/feel/coercion.rs` - Filled the 31-01 doc-only skeleton: coerce_to_number/coerce_to_string/coerce_to_bool with 10 unit tests. NO implicit number↔string (Pitfall 6).
- `engine/src/dmn/feel/parser.rs` - Lexer fix (second `.` of `..` followed by digit no longer consumed as leading-dot decimal), parse_list_or_range Range detection + inclusivity fix, LParen arm Range detection + start_inclusive=false for `(`.

## Decisions Made
- **coercion.rs design** — Three free functions (not methods on Value): `coerce_to_number`, `coerce_to_string`, `coerce_to_bool`. Each returns `Option<T>` (None for type mismatch → Null at call-site). NO implicit number↔string — the explicit `string()`/`number()` built-ins in 31-07 are the only such path.
- **eval_add dispatch order** — null-check → datetime_add → string-concat → numeric. Date-time is tried first so `Date+YMD→Date` works. String-concat is tried before numeric so `"a"+"b"→"ab"` works. `Number+String` falls through string-concat (coerce_to_string(Number)=None) then numeric (coerce_to_number(String)=None) → Null.
- **eval_pow via checked_powd** — `rust_decimal::MathematicalOps::checked_powd(&self, exp: Decimal) -> Option<Decimal>` handles integer exponents exactly (checked_powu/checked_powi), non-integer via exp/ln, overflow→None. Right-associativity is in the AST, not eval.
- **Date-Date→DaysTimeDuration** — The DMN spec allows both Date-Date→DTD and Date-Date→YMD (via `years and months duration(a,b)`). The arithmetic `-` operator produces DTD; the YMD form is a built-in function (31-08). `years_months_between` helper deferred to 31-08.
- **Parser range-inclusivity fix** — `parse_postfix`'s `..` handling greedily produces `Range{start_inclusive: true, end_inclusive: true}` without bracket context. `parse_list_or_range` (for `[a..b]`) and the `LParen` arm (for `(a..b)`) detect the Range and fix inclusivity from the actual brackets. This is a Rule 3 blocking fix — the `in range` tests required correct `[..]` and `(..)` parsing.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Between arm ge/le comparisons were swapped**
- **Found during:** Task 1 GREEN
- **Issue:** The 31-02 Between arm used `o != Ordering::Greater` for ge and `o != Ordering::Less` for le. Since `feel_compare(a, b)` returns the ordering of `a` vs `b`, `a >= b` ↔ `o != Less` (not `o != Greater`), and `a <= b` ↔ `o != Greater` (not `o != Less`).
- **Fix:** Changed ge to `o != Ordering::Less` and le to `o != Ordering::Greater`.
- **Files modified:** engine/src/dmn/feel/eval.rs
- **Commit:** `ff0db6c89`

**2. [Rule 3 - Blocking] Lexer consumed second `.` of `..` as leading-dot decimal**
- **Found during:** Task 1 GREEN (debugging `5 in [1..10]` returning Null)
- **Issue:** In `lex_one`, when `b == b'.'` and the next char is a digit, the lexer called `lex_number` (leading-dot decimal). For `1..10`, the second `.` is followed by `1` (digit), so it was consumed as `.10` → `Number(0.10)` instead of `Dot` + `Number(10)`. This blocked all `in [a..b]` tests.
- **Fix:** Added `!(self.cursor > 0 && self.src[self.cursor - 1] == b'.')` check — the second `.` of `..` (preceded by `.`) is emitted as `Dot`, not consumed as a number.
- **Files modified:** engine/src/dmn/feel/parser.rs
- **Commit:** `ff0db6c89`

**3. [Rule 3 - Blocking] parse_list_or_range wrapped Range in List**
- **Found during:** Task 1 GREEN
- **Issue:** `parse_postfix` greedily consumed `..` as a range operator (for standalone `a..b` in `for`/`some`/`every`), producing `Range{...}`. Then `parse_list_or_range` didn't recognize `first` as already being a Range and wrapped it in `List([Range{...}])`. The evaluator's `List` arm is stubbed → `Value::Null`, so `eval_in` received Null instead of `Value::Range`.
- **Fix:** `parse_list_or_range` now checks if `first` is a `FeelExpression::Range` and returns it directly with `end_inclusive` fixed from the closing bracket (`]` → true, `)` → false).
- **Files modified:** engine/src/dmn/feel/parser.rs
- **Commit:** `ff0db6c89`

**4. [Rule 3 - Blocking] LParen arm didn't fix start_inclusive for `(a..b)` ranges**
- **Found during:** Task 1 GREEN
- **Issue:** `parse_postfix` always set `start_inclusive: true`, but `(` means exclusive start. The `LParen` arm in `parse_primary` didn't detect Range inner expressions.
- **Fix:** `LParen` arm now detects `FeelExpression::Range` inner, sets `start_inclusive: false`, and fixes `end_inclusive` from the closing bracket (`)` → false, `]` → true).
- **Files modified:** engine/src/dmn/feel/parser.rs
- **Commit:** `ff0db6c89`

**5. [Rule 1 - Bug] proptest::proptest! macro at module level broke non-test builds**
- **Found during:** Task 2 GREEN (verification: `cargo build` failed)
- **Issue:** `proptest::proptest!` macro invocations at module level (not inside `#[cfg(test)] mod tests`) required the `proptest` crate during macro expansion, which is a dev-dependency unavailable in non-test builds.
- **Fix:** Added `#[cfg(test)]` attribute to both `proptest::proptest!` invocations — the macro is only expanded in test builds, and generated test functions remain at module level (filter substrings still match).
- **Files modified:** engine/src/dmn/feel/eval.rs
- **Commit:** `d5b83d318`

---

**Total deviations:** 5 auto-fixed (1 bug + 3 blocking + 1 bug)
**Impact on plan:** All fixes are correctness/blocking requirements for the plan's operator tests. The parser fixes (deviations 2-4) are in 31-01's territory (parser/lexer) but were blocking 31-03's `in range` tests — Rule 3 auto-fix. No scope creep — the corrected behavior is what the plan's tests assert.

## Issues Encountered
- **Parser range-syntax debugging (Task 1 GREEN):** The `5 in [1..10]` E2E test initially returned `Null`. Debugging revealed a three-layer parser bug: (1) lexer consumed second `.` of `..` as leading-dot decimal, (2) `parse_list_or_range` wrapped Range in List, (3) `LParen` arm didn't fix inclusivity. All three fixed as Rule 3 blocking issues.
- **proptest dev-dependency (Task 2 GREEN):** `cargo build` (non-test) failed because `proptest::proptest!` macro at module level referenced the `proptest` crate (dev-dependency). Fixed by gating with `#[cfg(test)]`. The existing proptest tests in `temporal.rs`/`types.rs` are inside `#[cfg(test)] mod tests` blocks and don't have this issue.

## Known Stubs

The full unary-test `in` semantics (positive unary test, comma-list, `-` not form) are deferred to 31-04. The current `eval_in` handles only `Value::Range` RHS; any other RHS returns `Value::Null` with a `TODO(31-04)` marker. This does NOT prevent the plan's goal — the plan explicitly states "the full unary-test `in` semantics land in 31-04."

## TDD Gate Compliance

- **RED gate:** `test(31-03):` commits exist for both tasks — `27a0134ea` (Task 1 RED: 14 failing tests for arithmetic/string-concat/numeric-edge/in-range) and `33f2e69b2` (Task 2 RED: 6 failing tests for date-time arithmetic).
- **GREEN gate:** `feat(31-03):` commits exist after each RED — `ff0db6c89` (Task 1 GREEN: all 24 Task 1 tests pass) and `d5b83d318` (Task 2 GREEN: all 8 Task 2 tests pass).
- All RED tests were verified to fail before GREEN implementation. All GREEN tests pass.

## Four Prohibitions Verification

1. **Never reuse Rust ==/Ord for FEEL equality** — Arithmetic operators use `coerce_to_number` + Decimal operators (`+`, `-`, `*`, `checked_div`, `checked_powd`), not Rust `==`/`Ord`. The `between`/`in` operators reuse `feel_compare` from the 31-02 private `cmp` module. No new `PartialEq`/`PartialOrd`/`partial_cmp` added outside `cmp`.
2. **Never use f64** — All arithmetic uses `rust_decimal::Decimal`. `grep -rn 'f64' engine/src/dmn/feel/eval.rs engine/src/dmn/feel/coercion.rs engine/src/dmn/feel/temporal.rs | grep -v '//'` returns nothing.
3. **No implicit number↔string conversion** — `coerce_to_number(String) → None`, `coerce_to_string(Number) → None`. `1 + "2"` → `eval_add` → `coerce_to_string(1)=None` (no concat) → `coerce_to_number("2")=None` (no numeric) → `Value::Null`.
4. **FEEL path never routed through EngineRegistry/engine/src/engine/** — `eval.rs` imports: `FeelExpression` (`super::ast`), `Value` (`crate::data_types`), `FeelRange` (`super::types`), `feel_numeric_eq` (`super::numeric`), `parse_at_literal` (`super::temporal`), `add_months_zoned`/`add_days_time_zoned` (`super::temporal`), `coerce_to_number`/`coerce_to_string` (`super::coercion`), `Months`/`TimeZone` (`chrono`). No `engine::engine` import.

## Self-Check: PASSED

- All 4 task commits verified present in git log (`27a0134ea`, `ff0db6c89`, `33f2e69b2`, `d5b83d318`).
- `engine/src/dmn/feel/eval.rs`, `engine/src/dmn/feel/coercion.rs`, `engine/src/dmn/feel/parser.rs` verified present on disk.
- 352 lib tests pass (0 failures); 377 lib tests pass with engine-rete features (zero Rete regression).
- `cargo build -p engine` succeeds (proptest gating fix verified).

## Next Phase Readiness
- The operator set is complete. Downstream plans extend the evaluator:
  - 31-04: full unary-test `in` semantics (positive unary test, comma-list, `not(...)` form) + `instance of` + filter/quantified predicates
  - 31-05: boxed expressions (If/For/Quantified/Path/Filter/List/Relation/Context)
  - 31-07: built-in functions (reuse `coercion.rs` coercions; `string()`/`number()` are the only number↔string path)
  - 31-08: date-time built-ins (reuse `add_months_zoned`/`add_days_time_zoned` + `years_months_between`)

---
*Phase: 31-feel-parser-evaluator-built-in-function-library*
*Completed: 2026-07-07*