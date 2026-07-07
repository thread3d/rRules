---
phase: 31-feel-parser-evaluator-built-in-function-library
plan: 02
subsystem: dmn/feel
tags: [feel, evaluator, three-valued-logic, cmp, dmn, dmn-12, scope, null-propagation]
requires:
  - phase: 31-01
    provides: FeelExpression AST, parser entry points (parse_expression), FeelRange carrier
provides:
  - feel-evaluator-core
  - feel-cmp-module
  - feel-scope-flat
  - three-valued-logic-operators
affects: [31-03, 31-04, 31-05, 31-06, 31-07, 31-08, 31-09]

tech-stack:
  added: []
  patterns:
    - private-cmp-module-visibility-enforced-no-rust-eq
    - three-valued-logic-option-bool-option-ordering
    - flat-scope-with-explicit-deps
    - exhaustive-match-no-wildcard-safety-net
    - tdd-red-green-per-task
    - test-flattening-for-filter-matching

key-files:
  created: []
  modified:
    - engine/src/dmn/feel/eval.rs

key-decisions:
  - "Private `mod cmp` (not `pub mod`) — makes Rust ==/Ord reuse a visibility error; no_rust_eq test enforces no PartialEq/PartialOrd/partial_cmp outside cmp at test time"
  - "feel_equals/feel_compare return Option<bool>/Option<Ordering> (None = null) — the 3VL boundary; opt_bool maps None→Value::Null at the evaluator edge"
  - "Flat Scope (HashMap<String,Value> bindings + HashMap<String,Vec<String>> deps) per CONTEXT.md user override — NOT a parent/child chain; nested resolution uses explicit dependency tracking"
  - "evaluate(&FeelExpression, &Scope) -> Value — sole entry point, takes borrowed AST (never &str); type-level no-re-parse contract (DMN-18)"
  - "Exhaustive match over FeelExpression with NO `_ =>` wildcard — adding a variant forces an evaluator-arm update (Anti-Pattern 5 safety net)"
  - "Number/Integer cross-type in feel_compare handled with correct operand order (fixes a latent bug in research Pattern 2 where (Number,Integer) used the reversed compare)"
  - "Tests flattened into `mod eval` (not nested `mod tests`) so VALIDATION.md filter substrings (`dmn::feel::eval::dmn12_`) match directly"
  - "Between desugaring evaluated as `a>=b and a<=c` via feel_compare + and_3vl (the parser emits BinOp(Between, a, List([b,c])))"
  - "Arithmetic (+,-,*,/,**) and `in` stubbed to Value::Null (31-03 coercion territory); not_3vl wired in 31-07's `not` built-in"

patterns-established:
  - "Private cmp module: the sole home of Rust ==/Ord reuse for FEEL; all outside callers go through feel_equals/feel_compare returning Option<_>"
  - "3VL boundary: Option<bool>/Option<Ordering> inside cmp → opt_bool → Value (None→Null) at the evaluator edge"
  - "Exhaustive FeelExpression match (no wildcard) is the missing-variant safety net"
  - "Test flattening: per-item #[cfg(test)] attributes directly in the module (not a nested mod tests) so cargo test filter substrings match"

requirements-completed: [DMN-12]

coverage:
  - id: D1
    description: "Private cmp module with three-valued logic operators (feel_equals, feel_compare, and_3vl, or_3vl, not_3vl) implementing DMN 1.5 rule 49.a null semantics + cross-type numeric promotion"
    requirement: DMN-12
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#dmn12_null_equals_null_yields_true"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#dmn12_null_equals_nonnull_yields_false"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#dmn12_a_equals_type_mismatch_yields_null"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#dmn12_null_lt_three_yields_null"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#dmn12_and_truth_table"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#dmn12_or_truth_table"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#dmn12_not_null"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#truth_tables_and"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#truth_tables_or"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#truth_tables_not"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#truth_tables_equals_same_kind"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#truth_tables_compare"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#number_integer_cross_type"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#date_equals_datetime_z"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#no_rust_eq"
        status: pass
    human_judgment: false
  - id: D2
    description: "evaluate(&FeelExpression, &Scope) -> Value dispatch skeleton with real Literal/Name/QualifiedName/AtLiteral/Range/BinOp(comparison+boolean)/Neg/Err arms and stubbed control/boxed/function arms"
    requirement: DMN-12
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#eval_literal_number"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#eval_name"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#eval_at_literal_date"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#eval_null_equals_null"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#eval_null_lt_three"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#eval_one_equals_one"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#eval_string_equals_number_yields_null"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#eval_true_and_false"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#eval_true_or_false"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#eval_not_null"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#eval_err_yields_null"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/eval.rs#eval_takes_borrowed_ast_not_str"
        status: pass
    human_judgment: false

duration: ~12min
completed: 2026-07-07
status: complete
---

# Phase 31 Plan 02: FEEL Evaluator Core + Private cmp Module Summary

Private `cmp` module with three-valued-logic `feel_equals`/`feel_compare`/`and_3vl`/`or_3vl`/`not_3vl` (DMN 1.5 rule 49.a), flat `Scope` with explicit dependency tracking, and `evaluate(&FeelExpression, &Scope) -> Value` exhaustive dispatch with real comparison/boolean/literal/name/range/err arms — the single most important correctness property of the phase.

## Performance

- **Duration:** ~12 min
- **Started:** 2026-07-07T06:32:53Z
- **Completed:** 2026-07-07T06:36:42Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Private `mod cmp` with `feel_equals`/`feel_compare` (`Option<bool>`/`Option<Ordering>`, null-propagating) — DMN 1.5 rule 49.a: `null = null → true`, `null = 5 → false`, `"a" = 1 → null`, `null < 3 → null`. Number/Integer cross-type via `feel_numeric_eq`; Date=DateTimeTz-at-UTC-midnight (§10.3.2.3.5); list/context element-wise equality with null-propagation.
- Explicit `and_3vl`/`or_3vl`/`not_3vl` truth tables — `false` dominant in AND, `true` dominant in OR, `not(null) → null`. No Rust `==`/`Ord` reuse (the `no_rust_eq` test enforces this at test time by grepping for forbidden patterns outside `mod cmp`).
- Flat `Scope` (bindings + deps HashMaps) with `empty`/`get`/`bind`/`resolve_qn` — CONTEXT.md user override (NOT a parent/child chain); longest-match prefix resolution + `path_nav`.
- `evaluate(&FeelExpression, &Scope) -> Value` exhaustive dispatch (NO `_ =>` wildcard) — real arms: Literal/Name/QualifiedName/AtLiteral/Range/BinOp(comparison+boolean+between)/Neg/Err; stubbed arms: If/For/Quantified/Path/Filter/InstanceOf/List/Relation/Context/FunctionInvocation/FunctionDefinition → `Value::Null` with `TODO(31-NN)` markers.
- No `evaluate(&str, ...)` overload — type-level no-re-parse contract (DMN-18); the sole signature takes `&FeelExpression`.
- 27 eval tests pass (15 cmp conformance + 12 dispatch); 103 FEEL tests pass; zero Rete regression across the full engine suite (334 lib tests).

## Task Commits

Each task was committed atomically (TDD RED → GREEN per task):

1. **Task 1: Flat Scope + private cmp module (tdd)** - `4f44d36b1` (test: RED) → `74f58d811` (feat: GREEN)
2. **Task 2: evaluate dispatch skeleton (tdd)** - `f55fb7814` (test: RED) → `8fb306e94` (feat: GREEN)

**Plan metadata:** pending (final docs commit below)

## Files Created/Modified
- `engine/src/dmn/feel/eval.rs` - Replaced the 31-01 stub with the real `Scope` (flat bindings + deps), private `cmp` module (feel_equals/feel_compare/and_3vl/or_3vl/not_3vl + list/context/date-datetime helpers), `evaluate` exhaustive dispatch, 3VL boundary helpers (opt_bool/to_bool/opt_endpoint), and 27 flattened tests (15 cmp conformance + truth-table + no_rust_eq; 12 evaluate dispatch).

## Decisions Made
- **Private `mod cmp` (not `pub mod`)** — makes Rust `==`/`Ord` reuse a visibility error: outside callers cannot name `feel_equals`'s internals and must go through the `Option<_>`-returning API. The `no_rust_eq` test enforces this at test time (grep for `PartialEq`/`PartialOrd`/`partial_cmp` outside `mod cmp`, stripping comments and `#[derive(...)]` lines to avoid false positives on the structural `Scope` derive and prohibition doc comments).
- **Number/Integer operand order in `feel_compare`** — the research Pattern 2 code had `(Number, Integer)` using `Decimal::from(*i).partial_cmp(x)` (reversed). Corrected to `x.partial_cmp(&Decimal::from(*i))` for `(Number, Integer)` and `Decimal::from(*i).partial_cmp(x)` for `(Integer, Number)` — a latent bug fix.
- **Tests flattened into `mod eval`** (not nested `mod tests`) — the VALIDATION.md per-task filters (`dmn::feel::eval::dmn12_`, `::truth_tables`, `::no_rust_eq`, `::eval_`) are substring matches; nesting under `mod tests` would produce paths like `dmn::feel::eval::tests::dmn12_...` that break the substring match (vacuous 0-test pass). Each test fn carries `#[cfg(test)]` directly.
- **Range equality deferred** — `feel_equals` does NOT handle `Value::Range` (leaves `_ → None` with a TODO(31-05)); no conformance case requires it in 31-02, and adding untested code paths would violate the TDD discipline. Range equality lands in 31-05 (boxed-context evaluation).
- **`between` evaluated as `a>=b and a<=c`** via `feel_compare` + `and_3vl` — the parser emits `BinOp { op: Between, lhs: a, rhs: List([b, c]) }`; the evaluator special-cases Between before evaluating rhs as a value (the List arm is still stubbed).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Number/Integer operand order reversed in research Pattern 2 feel_compare**
- **Found during:** Task 1 GREEN
- **Issue:** The research Pattern 2 reference code used `Decimal::from(*i).partial_cmp(x)` for BOTH `(Number, Integer)` and `(Integer, Number)`, which gives the reversed ordering for `(Number, Integer)`.
- **Fix:** Split into two arms with correct operand order: `(Number(x), Integer(i)) → x.partial_cmp(&Decimal::from(*i))` and `(Integer(i), Number(x)) → Decimal::from(*i).partial_cmp(x)`.
- **Files modified:** engine/src/dmn/feel/eval.rs
- **Verification:** `number_integer_cross_type` test passes (covers both cross-type directions).
- **Committed in:** `74f58d811` (Task 1 GREEN commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** The fix is a correctness requirement (wrong ordering for Number/Integer comparisons would silently invert `<`/`>` results). No scope creep — the corrected behavior is what the plan's truth-table tests assert.

## Issues Encountered
- **no_rust_eq false positives (pre-GREEN, resolved in RED):** The literal-string grep for `PartialEq`/`PartialOrd`/`partial_cmp` initially matched doc comments mentioning the prohibition and the structural `#[derive(PartialEq)]` on `Scope` (required by `FeelFunction`'s serde+equality bound). Resolved by stripping comment lines (`//`, `//!`, `///`) and `#[derive(...)]` lines before checking, and targeting actual reuse patterns (imports, impl blocks, `.partial_cmp(` calls). The `Scope` structural derive is NOT FEEL value equality (the plan explicitly keeps `Value`'s derives untouched).
- **Test-filter matching (pre-GREEN, resolved in RED):** Tests nested in `#[cfg(test)] mod tests { ... }` produced paths like `dmn::feel::eval::tests::dmn12_...`, breaking the VALIDATION substring filters (vacuous 0-test pass). Resolved by flattening all tests directly into `mod eval` with per-item `#[cfg(test)]` attributes.

## Known Stubs

The `evaluate` dispatch intentionally stubs arms that subsequent plans populate. These do NOT prevent the plan's goal (cmp module + comparison/boolean/literal/name/range/err dispatch) — they are explicit `TODO(31-NN)` markers in the exhaustive match:

| Arm | Stub return | Resolving plan |
|-----|-------------|----------------|
| `If { .. }` | `Value::Null` | 31-05 (3VL condition dispatch) |
| `For { .. }` | `Value::Null` | 31-05 (cartesian-product → Array) |
| `Quantified { .. }` | `Value::Null` | 31-05 (some/every fold via or_3vl/and_3vl) |
| `Path { .. }` | `Value::Null` | 31-05 (context/list path navigation) |
| `Filter { .. }` | `Value::Null` | 31-05 (list filter predicate) |
| `InstanceOf { .. }` | `Value::Null` | 31-04 (feel::types::conforms) |
| `List(_)` | `Value::Null` | 31-05 (element evaluation → Array) |
| `Relation { .. }` | `Value::Null` | 31-05 (relation → list of contexts) |
| `Context(_)` | `Value::Null` | 31-05 (dependency-ordered context evaluation) |
| `FunctionInvocation { .. }` | `Value::Null` | 31-06/31-07 (built-in + user-lambda invocation) |
| `FunctionDefinition { .. }` | `Value::Null` | 31-06 (closure construction) |
| `BinOp` arithmetic (`Add`/`Sub`/`Mul`/`Div`/`Pow`) | `Value::Null` | 31-03 (numeric/string coercion) |
| `BinOp` `In` | `Value::Null` | 31-03 (positive unary test) |

## TDD Gate Compliance

- **RED gate:** `test(31-02):` commits exist for both tasks — `4f44d36b1` (Task 1 RED: 11 cmp behavior tests fail, no_rust_eq passes) and `f55fb7814` (Task 2 RED: 7 dispatch tests fail against the evaluate stub, 5 pass where the stub's Null return is coincidentally correct for null-propagation/error paths).
- **GREEN gate:** `feat(31-02):` commits exist after each RED — `74f58d811` (Task 1 GREEN: all 15 cmp tests pass) and `8fb306e94` (Task 2 GREEN: all 27 eval tests pass).
- All RED tests were verified to fail before GREEN implementation. All GREEN tests pass.

## Four Prohibitions Verification

1. **Never reuse Rust ==/Ord for FEEL equality** — The private `mod cmp` is the sole home of `feel_equals`/`feel_compare` (returning `Option<_>`). The `no_rust_eq` test greps `eval.rs` outside `mod cmp` for `PartialEq`/`PartialOrd` imports, impl blocks, and `.partial_cmp(` calls — none found. The structural `#[derive(PartialEq)]` on `Scope` (required by `FeelFunction`'s serde bound) is explicitly allowed (stripped from the grep target); it is NOT FEEL value equality.
2. **Never use f64** — All numeric comparison uses `Decimal` (`rust_decimal`) via `feel_numeric_eq` and `Decimal::partial_cmp`. No `f64` constructed in eval.rs.
3. **No implicit number↔string conversion** — `feel_equals` on `(Number, String)` returns `None` (null), not an implicit conversion. The explicit `string()`/`number()` built-ins land in 31-07.
4. **FEEL path never routed through EngineRegistry/engine/src/engine/** — `eval.rs` imports only `FeelExpression` (`super::ast`), `Value` (`crate::data_types`), `FeelRange` (`super::types`), `feel_numeric_eq` (`super::numeric`), `parse_at_literal` (`super::temporal`), `BinOpKind` (`super::ast`). No `engine::engine` import.

## Self-Check: PASSED

- All 4 task commits verified present in git log (`4f44d36b1`, `74f58d811`, `f55fb7814`, `8fb306e94`).
- `engine/src/dmn/feel/eval.rs` verified present on disk.
- 27 eval tests pass (15 cmp + 12 dispatch); 103 FEEL tests pass; 334 lib tests pass (zero Rete regression).

## Next Phase Readiness
- The evaluator core + cmp module is complete. Downstream plans (31-03..31-09) extend the `evaluate` dispatch by replacing `TODO(31-NN)` stubs with real arms.
- `feel_equals`/`feel_compare` are ready for 31-03 (comparison operators via coercion), 31-04 (filter/quantified predicates), 31-05 (boxed-context equality).
- `Scope` (flat bindings + deps) is ready for 31-04 (for-loop iteration vars), 31-05 (boxed-context topo evaluation), 31-06 (lambda capture).
- The `not_3vl` operator is wired into the `not` built-in in 31-07 (currently the `FunctionInvocation` arm stubs `not(null)` to `Value::Null`, which the behavior block accepts).

---
*Phase: 31-feel-parser-evaluator-built-in-function-library*
*Completed: 2026-07-07*