# Requirements: rRules — Milestone v7.0 Performance + Production Readiness

**Created:** 2026-07-10  
**Status:** Draft (pending user approval)  
**Scope:** Benchmarks, parser/evaluator optimization, DRD parallelization, testing expansion  

---

## Phase Structure

| # | Name | Requirements | Status |
|---|------|--------------|--------|
| 36 | Benchmarks + Memory Profiling Gates | PERF-01, PERF-02, PERF-03, PERF-04 | not_started |
| 37 | Parser/Evaluator Optimization with AST Cache Warming | PERF-05, PERF-06, PERF-07 | not_started |
| 38 | DRD Parallelization with Topological Sort | PERF-08, PERF-09 | not_started |
| 39 | Testing Expansion with Fuzzing and Property-Based Tests | PERF-10, PERF-11, PERF-12 | not_started |

**Coverage:** 12/12 v7.0 requirements (PERF-01..PERF-12) mapped to exactly one phase, 0 orphans, 4/4 phases with ≥1 requirement.

---

## Phase 36: Benchmarks + Memory Profiling Gates

**Requirements:** PERF-01, PERF-02, PERF-03, PERF-04  
**Goal:** Establish performance measurement infrastructure and memory profiling gates to catch regressions early.

### PERF-01: Criterion Benchmark Suite
- `benches/dmn_parser.rs` — benchmark DMN XML parsing throughput (rows/sec)
- `benches/feel_evaluator.rs` — benchmark FEEL expression evaluation throughput
- `benches/decision_table.rs` — benchmark decision table evaluation with all hit policies
- All benchmarks use criterion 0.5 with t-test analysis, outlier detection, flame graphs

### PERF-02: Memory Profiling Infrastructure
- Integrate dhat for line-level allocation attribution in parser and evaluator hot paths
- Add heaptrack CI integration (system package) for zero-code-changes heap tracking
- Establish baseline memory profiles for parser + FEEL evaluator before optimization

### PERF-03: Allocation Reduction Targets
- Identify top 5 allocation hotspots via dhat profiling
- Set reduction targets: 20% allocations in parser, 15% in FEEL evaluator
- Measure improvement with criterion benchmarks after each optimization wave

### PERF-04: CI Performance Regression Gates
- Add `ci/perf/regression.yml` GitHub Actions workflow
- Run criterion benchmarks on every PR; compare against baseline
- Fail PR if regression >5% on any benchmark (configurable threshold)

---

## Phase 37: Parser/Evaluator Optimization with AST Cache Warming

**Requirements:** PERF-05, PERF-06, PERF-07  
**Goal:** Optimize parser and FEEL evaluator throughput using cache warming and allocation reduction.

### PERF-05: AST Cache Warming (LRU Eviction)
- Extend DashMap-based AST cache with LRU eviction policy (max 10k entries per model)
- Implement per-thread pre-warming strategy for high-concurrency workloads
- Target: eliminate re-parse on hot paths; cache hit rate >95% for repeated evaluations

### PERF-06: Per-Thread Cache Patterns
- Add `ThreadLocal<FeelAstCache>` pattern for single-request evaluation
- Benchmark ThreadLocal vs DashMap under 16+ concurrent request scenarios
- Target: 4-20x throughput improvement at high concurrency (matches DashMap advantage)

### PERF-07: Quick-XML Streaming Patterns
- Optimize parser to use quick-xml streaming patterns for zero-copy parsing where possible
- Reduce allocation in hot paths identified by PERF-03 profiling
- Target: 20% allocations reduction in parser; measurable improvement via criterion benchmarks

---

## Phase 38: DRD Parallelization with Topological Sort

**Requirements:** PERF-08, PERF-09  
**Goal:** Enable parallel evaluation of independent decision chains via topological sort + level-by-level execution.

### PERF-08: Topological Sort for DRD Graph
- Implement topological sort over DRD dependency graph (already exists in Phase 33)
- Identify independent nodes at each level that can be evaluated in parallel
- Add `resolve_drd_order_parallel()` method alongside existing sequential resolver

### PERF-09: Level-by-Level Parallel Evaluation
- For each level in topological order, spawn tokio tasks for independent decisions
- Use ThreadLocal engine pattern to avoid shared &mut engine contention
- Target: 3-4x throughput gains on independent decision chains (measured via criterion benchmarks)

---

## Phase 39: Testing Expansion with Fuzzing and Property-Based Tests

**Requirements:** PERF-10, PERF-11, PERF-12  
**Goal:** Expand test coverage with fuzzing infrastructure and property-based tests to catch boundary bugs.

### PERF-10: cargo-fuzz Infrastructure
- Add `fuzz/` directory with libFuzzer-based fuzz targets for parser and FEEL evaluator
- Use quick-xml's chunked-reader pattern as reference for fuzz target design
- Integrate with CI (OSS-Fuzz compatible) for continuous fuzzing

### PERF-11: Property-Based Tests for FEEL Parser/Evaluator
- Add `proptest` crate for property-based testing of FEEL expressions
- Properties to verify: idempotency, associativity, commutativity where applicable
- Target: catch edge cases in three-valued logic, temporal arithmetic, list operations

### PERF-12: TCK Coverage Gap Closure
- Identify gaps in vendored TCK vectors (currently 21 directories)
- Expand to cover all Level 3 FEEL built-in function families
- Document remaining coverage gaps and known divergences for future phases

---

## Success Criteria

### Performance Targets
- Criterion benchmarks established with baseline measurements
- Memory profiling shows allocation reduction targets met (PERF-03)
- DRD parallelization achieves 3-4x throughput gains on independent chains (PERF-09)

### Test Coverage
- Fuzzing infrastructure catches at least 3 boundary bugs in parser/evaluator
- Property-based tests cover all FEEL grammar rules 1–65
- TCK coverage expanded from 21 to 35+ directories (Level 3 completeness)

### Quality Gates
- CI performance regression gates prevent >5% regressions on any benchmark
- All new code has dhat profiling data showing allocation reduction
- Zero regression in existing DMN evaluation path (PERF-08, PERF-09 additive only)

---

## Scope Locks (from prior research and user decisions)

- **No dsntk dependency** — Build FEEL from scratch; stack: `quick-xml 0.40`, `rust_decimal 1.42`, `chrono-tz 0.10`
- **AST cache stays in engine/src/dmn/feel/cache.rs** — Browser can hint warm-up but cannot bypass server parse
- **DRD parallelization additive only** — Existing sequential resolver unchanged; new parallel path alongside
- **Fuzzing uses libFuzzer (cargo-fuzz)** — Not afl.rs (better coverage guidance)

---

## Requirements Coverage

| Requirement | Phase | Status | Notes |
|-------------|-------|--------|-------|
| PERF-01: Criterion benchmarks | 36 | not_started | Foundation for all performance work |
| PERF-02: Memory profiling infra | 36 | not_started | dhat + heaptrack CI integration |
| PERF-03: Allocation reduction targets | 37 | not_started | Driven by PERF-02 profiling data |
| PERF-04: CI regression gates | 36 | not_started | Prevents performance regressions |
| PERF-05: AST cache warming (LRU) | 37 | not_started | Extends Phase 31 DashMap cache |
| PERF-06: Per-thread cache patterns | 37 | not_started | ThreadLocal for high concurrency |
| PERF-07: Quick-XML streaming | 37 | not_started | Zero-copy parsing optimization |
| PERF-08: Topological sort for DRD | 38 | not_started | Builds on Phase 33 resolver |
| PERF-09: Parallel evaluation | 38 | not_started | tokio tasks for independent decisions |
| PERF-10: cargo-fuzz infra | 39 | not_started | libFuzzer-based fuzz targets |
| PERF-11: Property-based tests | 39 | not_started | proptest crate for FEEL properties |
| PERF-12: TCK coverage gaps | 39 | not_started | Expand from 21 to 35+ directories |

**Coverage:** 12/12 v7.0 requirements (PERF-01..PERF-12) mapped to exactly one phase, 0 orphans, 4/4 phases with ≥1 requirement.
