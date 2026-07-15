# Phase 40: Numeric Built-in Functions - Context

**Gathered:** 2026-07-15
**Status:** Ready for planning
**Mode:** Smart discuss (autonomous, batch table proposals)

<domain>
## Phase Boundary

Phase 40 implements the missing numeric FEEL built-in function features needed to raise TCK evaluator coverage from 21/51 to 51/51 vectors. It delivers:

- **`abs()` duration support** — extend `built_in_abs` to handle `Value::DaysTimeDuration` and `Value::YearsMonthsDuration` per DMN 1.5 §10.3.4.1, using the existing `abs()` on the inner duration value.
- **`number(from, grouping, decimal)` — 3-arg form** — implement the full 3-argument `number()` function with locale-independent parsing: remove grouping separators then parse with decimal separator as the radix point.
- **Evaluator dispatch verification** — verify the evaluator's built-in function dispatch (name lookup in `builtin_registry()`) works correctly for all numeric functions; handle named parameters by positional fallback.
- **Edge case handling** — verify empty-list SUM/MIN/MAX → null is properly routed through the evaluator layer; add unit tests for each function.

**Depends on:** Phase 31 (FEEL evaluator core + built-in function registry), Phase 30 (FEEL value types + Decimal128 numeric domain)

**Requirements:** FEEL-01, FEEL-02, FEEL-03, FEEL-04, FEEL-05, FEEL-06, FEEL-07, FEEL-08, FEEL-09, FEEL-10

</domain>

<decisions>
## Implementation Decisions

### abs() Duration Support
- Extend `built_in_abs` to handle `Value::DaysTimeDuration` and `Value::YearsMonthsDuration` per DMN 1.5 §10.3.4.1
- Use the existing `abs()` on the inner duration value (DaysTimeDuration uses `TimeDelta` which can be negative; YearsMonthsDuration has a `months` field)

### number(from, grouping, decimal) — 3-arg Form
- Implement the full 3-argument `number()` function
- Parse the string by removing grouping separators then parsing with decimal separator as the radix point (locale-independent per DMN spec)

### Evaluator Dispatch Fixes
- Verify evaluator dispatch works for all numeric functions via name lookup in `builtin_registry()`
- Handle named parameters by positional fallback (e.g., `abs(n:-1)` → positional)

### Edge Cases and Test Strategy
- Add unit tests for each function covering normal, edge, and null cases
- Run TCK evaluator layer to verify 14 numeric vectors pass

### Claude's Discretion
- Exact implementation details for duration abs (how to negate `TimeDelta` and `YearsMonthsDuration`)
- Internal helper function structure for `number()` 3-arg parsing
- Whether to add integration tests beyond unit tests and TCK sweep

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`engine/src/dmn/feel/builtins.rs`** — Contains all existing built-in function implementations including `built_in_abs`, `built_in_number`, `built_in_sqrt`, `built_in_exp`, `built_in_log`, `built_in_even`, `built_in_odd`, `built_in_modulo`, `built_in_median`, `built_in_mode`, `built_in_stddev`, `built_in_product`, `built_in_sum`, `built_in_min`, `built_in_max`, `built_in_mean`, `built_in_floor`, `built_in_ceiling`, `built_in_decimal`, and rounding functions. All use `rust_decimal::Decimal` for numeric operations.
- **`engine/src/dmn/feel/numeric.rs`** — `feel_numeric_eq` helper for null-propagating FEEL numeric equality.
- **`engine/src/dmn/feel/temporal.rs`** — Temporal types including `TimeValue`, `DateTimeValue`, `YearsMonthsDuration`, `DaysTimeDuration` (chrono::TimeDelta).
- **`engine/src/dmn/feel/eval.rs`** — FEEL evaluator with built-in function dispatch at line 473-479: looks up callee name in `builtin_registry()` and calls the function pointer.

### Established Patterns

- **`BuiltinFn = fn(&[Value]) -> Value`** — function pointer type for all built-ins
- **`null_unary` / `null_binary` / `null_ternary`** — helper wrappers for null-propagating operations
- **`builtin_registry()`** — `HashMap<&'static str, BuiltinFn>` populated via `reg!()` macro
- **Decimal128 via `rust_decimal::Decimal`** — never `f64` in the FEEL production path
- **Rich spec-cited doc comments** with §10.3.x citations and pitfall references

### Integration Points

- **Evaluator dispatch** at `eval.rs:473-479` — the built-in registry lookup is the integration point for all numeric functions
- **TCK sweep** at `engine/tests/tck_sweep.rs` — the evaluator layer test that must pass for all 14 numeric vectors
- **`engine/tests/feel_tck_numeric.rs`** — existing numeric TCK integration test (Phase 30 scope, limited to basic arithmetic)

</code_context>

<specifics>
## Specific Ideas

- The `abs()` function currently handles `Value::Number` and `Value::Integer` only. TCK tests also expect `abs(duration("-P1D"))` → `P1D` and `abs(duration("-P1Y"))` → `P1Y`. The implementation should match on `Value::DaysTimeDuration` and `Value::YearsMonthsDuration` and return the absolute value.
- The `number()` function currently handles only the 1-arg form. The 3-arg form `number(from, grouping, decimal)` needs to: (1) remove all occurrences of the `grouping` character from the `from` string, (2) replace the `decimal` character with `.`, (3) parse as Decimal.
- The evaluator dispatch at `eval.rs:473-479` already handles built-in lookup by name. Named parameters (e.g., `abs(n:-1)`) should fall back to positional matching when the parameter name doesn't match the built-in's expected parameter name.

</specifics>

<deferred>
## Deferred Ideas

- `abs()` for date/time types — not required by TCK numeric vectors; defer unless TCK demands
- `number()` with scientific notation parsing — not in current TCK scope
- Full clippy cleanup of builtins.rs — defer to a tech-debt phase

</deferred>
