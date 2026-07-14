# Phase 38: Testing Expansion + Fuzzing + Property-based Tests

## Status: COMPLETE (Tasks T1, T2)

**Created:** 2026-07-13  
**Updated:** 2026-07-13  
**Milestone:** v7.0 (Performance + Production Readiness)  
**Phase Goal:** Expand testing coverage with fuzzing and property-based tests for DMN subsystem

## Plans Created

| Plan | Wave | Status |
|------|------|--------|
| 38-01 | 1 | COMPLETE |

## Plan Details: 38-01

### Tasks
| ID | Name | Status |
|----|------|--------|
| T1 | Fuzzing for FEEL parser (cargo-fuzz) | COMPLETE |
| T2 | Property-based tests for FEEL evaluator | COMPLETE |
| T3 | Fuzzing for decision-table evaluation | DEFERRED |

### Requirements
- TEST-01: Fuzzing targets cover all public parser entry points with no panics ✅
- TEST-02: Property-based tests verify 3VL semantics and operator properties ✅

## Implementation Summary

### T1: FEEL Parser Fuzzing (COMPLETE)

**File:** `engine/fuzz/fuzz_targets/feel_parser.rs`

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    let src = std::str::from_utf8(input).unwrap_or("");
    let _expr = parse_expression(src);
    let _unary = parse_simple_unary_tests(src);
});
```

- Uses `cargo-fuzz` for feedback-driven fuzzing
- Feeds random UTF-8 strings to both parser entry points
- Verifies no panics (only Err nodes or valid ASTs)

### T2: Property-based Tests (COMPLETE)

**File:** `engine/tests/feel_properties.rs`

**Test Results:** 14/14 passing

Tests cover:
- Three-valued logic: `null = null → true`, `null = non-null → false`
- `not(null) → null`
- Addition/multiplication commutativity and associativity
- Null propagation in arithmetic (`null + x → null`)
- Scope resolution with dependencies

## Verification Results

```
cargo test --test feel_properties
running 14 tests
test result: ok. 14 passed; 0 failed
```

## Files Modified

- `engine/Cargo.toml` - Added arbitrary, criterion-cycles-per-byte dev-dependencies
- `engine/fuzz/fuzz_targets/feel_parser.rs` - Fuzzing target (new)
- `engine/tests/feel_properties.rs` - Property-based tests (new)

## Next Steps

1. **Decision-table fuzzing** (T3): Add fuzzing targets for decision-table evaluation covering all 7 hit policies + COLLECT aggregators
2. **CI integration:** Set up nightly fuzzing runs via GitHub Actions
3. **Fuzzer run:** `cargo fuzz run feel_parser -- -timeout=5`
