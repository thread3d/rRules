# Plan 40-03 Summary: Edge Case Unit Tests + TCK Verification

**Status:** Complete
**Date:** 2026-07-15

## Changes

### `engine/src/dmn/feel/builtins.rs`
- Added 25 edge-case unit tests for sqrt, exp, log, even, odd, modulo, product, median, mode, stddev
- Fixed `built_in_modulo` to use DMN 1.5 formula `a - b * floor(a/b)` for Decimal values (was using `a % b` which gives remainder, not Euclidean modulo)

### `engine/tests/tck_sweep.rs`
- Removed stale "sum-empty-list" known divergence — `built_in_sum`/`built_in_min`/`built_in_max` already return `Value::Null` for empty lists

## Verification
- `cargo test -p engine --lib dmn::feel::builtins::numeric` — 67 passed (was 42)
- `cargo test -p engine --test tck_sweep` — passes (all numeric vectors green)
- `cargo test -p engine --lib 'dmn::feel'` — 475 passed, 2 ignored
