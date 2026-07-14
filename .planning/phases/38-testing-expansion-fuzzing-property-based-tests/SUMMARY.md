# Phase 38: Testing Expansion + Fuzzing + Property-based Tests

## Status: COMPLETE (Tasks T1, T2, T3)

**Created:** 2026-07-13  
**Updated:** 2026-07-14  
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
| T3 | Fuzzing for decision-table evaluation | COMPLETE |

### T3: Decision-Table Fuzzing (COMPLETE)

**File:** `engine/fuzz/fuzz_targets/decision_table.rs`

Unlike a fixed-table smoke test, this target builds a **fuzz-shaped** table from
the input byte stream (rule count, FEEL cell text, scope bindings all vary) and
evaluates it across **all 7 hit policies** (UNIQUE, ANY, PRIORITY, FIRST, RULE
ORDER, OUTPUT ORDER, COLLECT) and **every COLLECT aggregator** (SUM, MIN, MAX,
COUNT, plus the no-aggregator form). The contract under fuzz is *no panics* —
the evaluator must return `Ok`/`Err`, never panic.

- Structurally consistent tables (input/output entry counts match clauses)
- Curated FEEL-fragment palettes exercise the numeric, string, boolean, and
  null branches; PRIORITY/OUTPUT ORDER get a valid output enumeration to rank
- Shared `FeelAstCache` per invocation mirrors the production hot path

**Smoke run:** `cargo +nightly fuzz run decision_table -- -runs=20000` →
20,000 executions, **0 crashes**, coverage 1088 / ft 2851.

### Fuzz harness restructure (T1 + T3)

The fuzz crate was moved to the canonical cargo-fuzz layout so `cargo +nightly
fuzz run` actually finds and builds the targets (the prior `Cargo.fuzz.toml`
layout was non-functional and never built):
- `engine/Cargo.fuzz.toml` → `engine/fuzz/Cargo.toml` (`[package.metadata]
  cargo-fuzz = true`, standalone `[workspace]`, `engine = { path = ".." }`)
- `make fuzz` / `make fuzz-build` Makefile targets
- `.github/workflows/fuzz.yml` — nightly 03:00 UTC fuzz gate + `fuzz`-label PR
  trigger, uploading crash artifacts on failure

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

1. **Run fuzzers long-term:** `make fuzz` (or `make fuzz TARGET=decision_table`) locally; the nightly CI gate (`fuzz.yml`, 03:00 UTC) runs each target for 5 min.
2. **Expand targets:** add a `feel_evaluator` target fuzzing `eval::evaluate` directly, and a `dmn_round_trip` target fuzzing import→export IR-equality (Phase 35 DMN-07).
3. **Triage corpus:** committed corpus can be minimized with `cargo +nightly fuzz tmin`.
