---
phase: 31-feel-parser-evaluator-built-in-function-library
plan: 09
subsystem: dmn::feel::cache + dmn parser wiring
tags: [dmn-18, feel-cache, dashmap, ast-cache, pre-parse, tck-harness]
dependency_graph:
  requires:
    - phase: 31-08
      provides: parse_expression / parse_simple_unary_tests entry points + FeelExpression AST type (Phase 29 IR)
    - phase: 31-02
      provides: evaluate(&FeelExpression, &Scope) type-level no-re-parse contract
  provides:
    - FeelAstCache = DashMap<String, Arc<FeelExpression>> with canonical keying (entry-point kind + trimmed content)
    - IR loader pre-parses all decision-table FEEL cells into the cache at load time
    - TCK FEEL test harness scaffold (loader + runner + 15-case vendored subset)
  affects: [phase-32 decision-table engine, phase-35 full TCK passage]
tech_stack:
  added: []
  patterns: [DashMap sidecar cache pattern, parse-once-per-key via or_insert_with, IR walk for cache population, cell_key canonical hashing (Pitfall 8)]
key_files:
  created:
    - engine/tests/feel_cache_wiring.rs — IR loader pre-parse integration test + perf property verification
    - engine/tests/feel_tck_subset.rs — TCK FEEL harness scaffold with loader + runner
    - engine/tests/fixtures/tck/feel/feel_grammar_subset.json — 15-case vendored FEEL subset
  modified:
    - engine/src/dmn/feel/cache.rs — real FeelAstCache implementation (was skeleton from 31-01)
    - engine/src/dmn/feel/mod.rs — re-export FeelAstCache + ParseEntry
    - engine/src/dmn/parser.rs — parse_dmn_with_cache + IR walk helper for cache population
decisions:
  - "Cache is a sidecar (DmnModel, FeelAstCache) returned from load, not mutated into DmnModel — avoids breaking Phase 29 IR shape"
  - "cell_key = format!(\"{:?}:{}\", entry, src.trim()) — Pitfall 8 disambiguation via Debug derive on ParseEntry enum"
  - "Open Question A6 resolved: Phase 29 Decision carries only decision_table; non-table boxed expressions parsed on demand by Phase 33"
  - "TCK subset uses JSON format (not XML) for hand-curated vectors — simpler loader, easier to expand in Phase 35"
metrics:
  duration_minutes: 20
  completed_date: 2026-07-08
status: complete

---

# Phase 31 Plan 09: FEEL AST Cache + IR Loader Wiring + TCK Harness Scaffold Summary

## One-liner

Implement the `FeelAstCache = DashMap<String, Arc<FeelExpression>>` concurrent AST cache with canonical (entry-point-kind, trimmed-content) keying, wire it into the Phase 29 DMN IR loader to pre-parse all decision-table cells at load time, and scaffold the OMG TCK FEEL test harness with a 15-case vendored subset.

## Implementation Summary

### Task 1: FeelAstCache (DashMap<String, Arc<FeelExpression>>) — 8 unit tests pass

**Core types:**
- `ParseEntry` enum (Expression / SimpleUnaryTests) with Debug, Clone, Copy, PartialEq, Eq derives
- `FeelAstCache` struct wrapping `DashMap<String, Arc<FeelExpression>>`
- `cell_key(src: &str, entry: ParseEntry) -> String` — canonical key: format!("{:?}:{}", entry, src.trim())

**Key behaviors verified by unit tests:**
- Same (entry, content) returns same Arc via pointer identity (`Arc::as_ptr`)
- Different entry points for same text yield different Arcs (Pitfall 8)
- CDATA flag does not affect cache key (content is post-CDATA-strip)
- Malformed expressions return `FeelExpression::Err` nodes, never panic (DoS mitigation)
- Cache bounded by distinct pairs — re-requesting same keys doesn't grow it
- `.trim()` normalizes whitespace in keys

**Re-export:** `feel::{FeelAstCache, ParseEntry}` added to `engine/src/dmn/feel/mod.rs`.

### Task 2: IR Loader Wiring + Integration Test — 6 integration tests pass

**IR loader extension (`engine/src/dmn/parser.rs`):**
- New public function `parse_dmn_with_cache(input) -> Result<(DmnModel, FeelAstCache), DmnParseError>`
- Internal helper `walk_ir_and_populate_cache` walks every FEEL-bearing field:
  - `InputClause.input_expression` → ParseEntry::Expression
  - `RuleEntry.text` (input_entries) → ParseEntry::SimpleUnaryTests
  - `OutputClause.default_output_entry` → ParseEntry::Expression
  - `AllowedValues::UnaryTests` → ParseEntry::SimpleUnaryTests
  - Annotation entries → ParseEntry::Expression

**Integration test (`engine/tests/feel_cache_wiring.rs`) — 6 tests:**
- `parse_dmn_with_cache_populates_all_cell_types` — cache non-empty after load
- `entry_point_kind_correct_per_cell_type` — Pitfall 8 verified (same text, different entry → different Arc)
- `cache_len_matches_distinct_pairs` — asserts ≥10 distinct parsed cells in fixture
- `perf_property_30_cells_no_reparsing` — parse-once guarantee under repeated get_or_parse calls on large fixture (6 inputs x 5 outputs x 2 rules = 60 cells)
- `evaluate_takes_feel_expression_not_str` — type-level contract via source grep
- `feel_path_never_routed_through_rete` — FEEL isolated from Rete (no cross-imports)

### Task 3: TCK FEEL Test Harness Scaffold — 2 tests pass

**Harness (`engine/tests/feel_tck_subset.rs`):**
- `load_tck_subset() -> Vec<TckFeelCase>` — reads all `.json` files from `tests/fixtures/tck/feel/`
- `run_one_case(&TckFeelCase) -> Result<(), String>` — parse_expression → evaluate → assert result matches expected
- SCAFFOLD marker documenting full TCK passage deferred to Phase 35

**Vendored subset (`engine/tests/fixtures/tck/feel/feel_grammar_subset.json`):**
- 15 test cases covering: arithmetic on decimals, three-valued logic (and/or), comparison equality/inequality/greater-than, boxed list, boxed context, string literal, null literal, parenthesized expression

## Test Results

| Test Suite | Status | Details |
|---|---|---|
| `cargo test -p engine --lib dmn::feel::cache` | PASS | 8/8 unit tests pass |
| `cargo test -p engine --test feel_cache_wiring` | PASS | 6/6 integration tests pass |
| `cargo test -p engine --test feel_tck_subset` | PASS | 2/2 tests (15 cases green) |
| `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` | PASS* | Zero regression; pre-existing `retract_integration.rs` failures unrelated to this plan |

*\*4 pre-existing failures in `engine/tests/retract_integration.rs` (untracked file, not modified by this plan) — these are outside scope.*

## Deviations from Plan

None - plan executed exactly as written. All acceptance criteria met.

## Key Files Created/Modified

- `engine/src/dmn/feel/cache.rs` — Real FeelAstCache implementation (was skeleton from 31-01). DashMap<String, Arc<FeelExpression>> with canonical keying and get_or_parse parse-once-per-key behavior.
- `engine/src/dmn/parser.rs` — Added `parse_dmn_with_cache`, internal IR walk helper for cache population at load time.
- `engine/tests/feel_cache_wiring.rs` — 6 integration tests verifying cache wiring, entry-point correctness (Pitfall 8), perf property, type-level contract, and FEEL/Rete isolation.
- `engine/tests/feel_tck_subset.rs` — TCK harness scaffold with loader + runner pattern for OMG DMN TCK FEEL vectors.
- `engine/tests/fixtures/tck/feel/feel_grammar_subset.json` — 15-case vendored FEEL subset covering arithmetic, logic, comparison, boxed expressions, literals.

## Task Commits

1. **Task 1: FeelAstCache implementation** - `eb80b7c0f` (feat)
2. **Task 2: IR loader cache wiring + integration test** - `38881be10` (feat)
3. **Task 3: TCK FEEL harness scaffold** - `c0a327156` (test)

## DMN-18 Performance Property Verified

A 30-cell table evaluated N times performs ~30 parses at load time, not 30×N. The type-level contract (`evaluate(&FeelExpression, &Scope)` with no `&str` overload) structurally guarantees zero re-parse during evaluation. The cache's parse-once-per-key guarantee is verified by the perf integration test which calls `get_or_parse` on all large-fixture cells and asserts `cache.len()` stays constant.

---
*Phase: 31-feel-parser-evaluator-built-in-function-library*
*Completed: 2026-07-08*
