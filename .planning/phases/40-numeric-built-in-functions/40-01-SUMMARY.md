# Plan 40-01 Summary: abs() Duration Support + Named-Parameter Resolution

**Status:** Complete
**Date:** 2026-07-15

## Changes

### `engine/src/dmn/feel/builtins.rs`
- Extended `built_in_abs` with two new match arms for `Value::DaysTimeDuration` and `Value::YearsMonthsDuration` per DMN 1.5 §10.3.4.1
- Added `use crate::dmn::feel::temporal::YearsMonthsDuration` import
- Added 5 unit tests: `abs_duration_days_negative`, `abs_duration_days_positive`, `abs_duration_years_months_negative`, `abs_duration_years_months_positive`, `abs_duration_null`

### `engine/src/dmn/feel/eval.rs`
- Added `builtin_param_names()` helper mapping 23 numeric built-in function names to their expected parameter names
- Modified `FunctionInvocation` arm to resolve named parameters to positional order when `named` is `Some(...)`
- Named parameters that don't match expected names are rejected (return `Value::Null`)

## Verification
- `cargo test -p engine --lib dmn::feel::builtins::numeric` — 31 passed (was 26)
- `cargo test -p engine --lib dmn::feel::eval` — 132 passed
- `cargo check -p engine` — compiles cleanly
