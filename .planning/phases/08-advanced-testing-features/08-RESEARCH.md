# Phase 8: Advanced Testing Features - Research

**Date:** 2026-06-07  
**Milestone:** v2.0 - Testing Framework Foundation  

## Stack Review

### Rust Testing Ecosystem

| Crate | Purpose | Status |
|-------|---------|--------|
| criterion | Benchmarking | Already in workspace |
| mockall | Mocking | Already in workspace |
| serial_test | Parallel isolation | Already in workspace |
| tempdir | Temporary files | Not used yet |

### Snapshot Testing Patterns

**Rust ecosystem options:**
- `insta` - Popular snapshot testing crate (not in use)
- `assert_fs` - File system assertions
- `serial_test` for parallel-safe test fixtures

**Recommendation:** Continue with custom snapshot implementation using existing serde infrastructure. The WorkingMemorySnapshot types already provide JSON serialization.

### Coverage Tools

| Tool | Purpose | Integration |
|------|---------|-------------|
| cargo-coverage | LLVM coverage | External |
| tarpaulin | Code coverage | External |

**Recommendation:** Use internal RuleCoverage for rule-level metrics (already implemented). External tools cover code coverage, not rule coverage.

### HTML Report Generation

**Options:**
1. **dodr** - Rust static site generator (adds dependency)
2. **askama + actix-web** - Full web stack (heavy)
3. **simple template rendering** - Minimal approach for Phase 8

**Recommendation:** Implement minimal HTML report in Phase 8 using built-in formatting, defer advanced features to Phase 9.

## Code Review Findings

### Testing Module (`engine/src/testing/mod.rs`)

**Strengths:**
- Well-organized with clear separation of concerns
- Comprehensive test coverage for core types
- Deterministic mode implementation solid

**Gaps to Address in Phase 8:**

1. **ConstraintVerifier**
   - Only checks no_loop, not lock_on_active
   - No integration with actual rule execution

2. **Snapshot comparison**
   - Missing diff generation
   - No partial matching for non-deterministic fields

3. **Coverage reporting**
   - Missing CSV export
   - No threshold assertions

## Proposed Implementation Plan

| Task | Estimate | Priority |
|------|----------|----------|
| ConstraintVerifier enhancement | 2h | High |
| Snapshot comparison helpers | 1.5h | High |
| Coverage reporting enhancements | 1.5h | Medium |
| Snapshot management utilities | 2h | Medium |
| HTML report generation | 4h | Low (defer) |

**Total Estimated Effort:** ~11 hours for Phase 8 (excluding HTML reports)
