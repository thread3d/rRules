# Phase 9: Testing Framework Enhancements - Research

**Date:** 2026-06-07  
**Phase Number:** 9  
**Milestone:** v2.0 (continuation)

## Executive Summary

Phase 8 completed the core testing infrastructure with utilities for:
- Per-test engine isolation (`TestSession`)
- Working memory fixtures (`FixtureBuilder`)
- Forward chaining verification (`ForwardChainingTester`)
- Constraint enforcement verification (`ConstraintVerifier`)
- Working memory snapshots (`WorkingMemorySnapshot`) with JSON comparison
- Rule coverage tracking (`RuleCoverage`)

**Phase 9 Goal:** Complete the testing ecosystem with filesystem integration for snapshot baselines, HTML report generation, and CLI tooling.

## Deferred from Phase 8

From `08-CONTEXT.md`:

| Item | Status |
|------|--------|
| Snapshot baseline directory structure | To implement in Phase 9 |
| `--update-snapshots` CLI flag | To implement in Phase 9 |
| HTML report generation (optional) | Defer to Phase 9 |
| Integration tests for snapshot management | To implement in Phase 9 |

## Phase 9 Scope

### 1. Snapshot Baseline Management

**Requirement:** Store and compare working memory snapshots against baselines.

**Proposed Implementation:**

```
tests/
├── fixtures/          # Static test data
│   └── (existing)
├── snapshots/         # NEW: Baseline snapshots per test module
│   ├── <module>/
│   │   ├── <test_name>.json
│   │   └── ...
│   └── updates/       # NEW: Pending snapshot updates
└── integration/
```

**API Design:**

```rust
// In engine/src/testing/snapshot.rs (new module)
pub struct SnapshotManager {
    base_dir: PathBuf,
}

impl SnapshotManager {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self;
    
    // Load a baseline snapshot
    pub fn load_baseline(&self, test_name: &str) -> Option<WorkingMemorySnapshot>;
    
    // Save current working memory as new baseline
    pub fn save_baseline(&mut self, test_name: &str, snapshot: &WorkingMemorySnapshot) -> Result<(), SnapshotError>;
    
    // Compare current state against baseline
    pub fn assert_matches_baseline(
        &self,
        test_name: &str,
        current: &WorkingMemorySnapshot,
    ) -> TestResult<()>;
    
    // Update baseline (for --update-snapshots flag)
    pub fn update_baseline(&mut self, test_name: &str, snapshot: &WorkingMemorySnapshot) -> Result<(), SnapshotError>;
}
```

**File Format:** JSON with consistent ordering (sorted keys)

### 2. HTML Report Generation

**Requirement:** Generate human-readable test reports without external dependencies.

**Approach:** Use `std::fmt` to generate self-contained HTML (CSS embedded via `<style>` tag).

**HTML Structure:**

```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Test Report</title>
    <style>/* inline CSS */</style>
</head>
<body>
    <h1>Test Results</h1>
    <div class="summary">...</div>
    <div class="test-results">...</div>
</body>
</html>
```

**Coverage Metrics:**
- Pass/fail counts
- Execution time per test
- Snapshot comparison results
- Rule coverage percentage

### 3. CLI Integration

**Requirement:** Add snapshot management flags to the test runner.

**Proposed CLI Flags:**

```
cargo test -- --help
...
Testing options:
    --update-snapshots      Update baseline snapshots instead of failing
    --report <format>       Generate report (html|json)
    --report-path <path>    Output path for reports
    --baseline-dir <dir>    Directory for snapshot baselines
```

**Implementation Strategy:**
- Add flags to test harness in `engine/src/testing/mod.rs`
- Parse args in test binary
- Pass mode to `SnapshotManager`

### 4. Integration Tests

**New Test Categories:**

```rust
// engine/tests/snapshot_integration.rs (new)
mod snapshot_filesystem;
mod snapshot_update_mode;
mod report_generation;

// engine/tests/coverage_integration.rs (new)
mod coverage_csv_export;
mod coverage_threshold_assertion;
```

## Implementation Tasks

| Task | Files to Create/Modify |
|------|----------------------|
| 1. SnapshotManager struct | `engine/src/testing/snapshot.rs` (new) |
| 2. Baseline directory setup | `tests/fixtures/snapshots/` (mkdir) |
| 3. JSON serialization helpers | `engine/src/testing/json.rs` (new) |
| 4. HTML report generator | `engine/src/testing/report/html.rs` (new) |
| 5. CLI flag parsing | `engine/src/testing/mod.rs` (modify) |
| 6. Integration tests | `engine/tests/snapshot_integration.rs` (new) |

## Constraints

1. **No External Dependencies**: Use only `std`, `serde`, `uuid`
2. **Backward Compatible**: Existing tests should pass without changes
3. **Optional HTML**: Report generation should be behind a feature flag or optional compilation

## Success Criteria

| Criterion | Test |
|-----------|------|
| Snapshots can be saved to filesystem | Test creates baseline.json, verifies file exists |
| Baseline comparison works | Test compares against stored snapshot |
| `--update-snapshots` flag works | Test runs with flag, updates baseline |
| HTML report generates | Test produces valid HTML output |
| Coverage CSV export works | Test outputs CSV with correct format |

## Alternatives Considered

1. **External Dependencies**: Could use `tempfile`, `pretty-html`, etc., but adds crate dependencies
2. **Binary Snapshot Format**: Could use bincode for smaller files, but JSON is human-readable
3. **Git-based Baselines**: Could store baselines in git LFS, but adds complexity

## Related Phases

- Phase 7: Core Testing Infrastructure (completed)
- Phase 8: Advanced Testing Features (completed)
- Phase 9: This phase - filesystem integration and reporting
- Phase 10+: BDD test runner integration (deferred to v2.1)
