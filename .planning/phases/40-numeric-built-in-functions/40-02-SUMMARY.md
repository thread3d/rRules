# Plan 40-02 Summary: number(from, grouping, decimal) 3-arg Form

**Status:** Complete
**Date:** 2026-07-15

## Changes

### `engine/src/dmn/feel/builtins.rs`
- Updated `built_in_number` to dispatch to 3-arg form when `args.len() >= 3`
- Added `built_in_number_3arg` helper implementing DMN 1.5 §10.3.4.1 algorithm:
  - Validates `from` is a non-null string
  - Validates `grouping` separator is one of `{',', '.', ' '}` or null
  - Validates `decimal` separator is one of `{',', '.'}` or null
  - Rejects when grouping and decimal are equal (both non-null)
  - Removes grouping separators, replaces decimal separator with `.`, parses as Decimal
- Added 11 unit tests covering European/US formats, null separators, invalid separators, space grouping, parse failures, and 1-arg backward compatibility

## Verification
- `cargo test -p engine --lib dmn::feel::builtins::numeric` — 42 passed (was 31)
- All 10 new number() 3-arg tests pass
- Existing 1-arg form unchanged
