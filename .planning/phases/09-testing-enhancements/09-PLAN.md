# Phase 9: Testing Framework Enhancements - Plan

**Date:** 2026-06-07  
**Phase Number:** 9  
**Milestone:** v2.0 (continuation)  
**Status:** Ready for Execution

## Goal

Complete the testing ecosystem by implementing filesystem integration for snapshot baselines, HTML report generation, and CLI tooling.

---

## Plan 1: Snapshot Baseline Management

**Goal:** Implement file-based storage and comparison of working memory snapshots

### Files to Create
- `engine/src/testing/snapshot.rs` - New module with SnapshotManager

### Implementation Steps

1. **Create SnapshotManager struct**
   ```rust
   pub struct SnapshotManager {
       base_dir: PathBuf,
       mode: SnapshotMode,  // Read, Write, Update
   }
   ```

2. **Implement methods:**
   - `new(base_dir)` - Initialize with baseline directory
   - `load_baseline(test_name)` - Load JSON snapshot from disk
   - `save_baseline(test_name, snapshot)` - Save current state as baseline
   - `update_baseline(test_name, snapshot)` - Overwrite existing baseline
   - `assert_matches_baseline(test_name, current)` - Compare and assert

3. **Add to testing module re-exports:**
   ```rust
   pub use snapshot::{SnapshotManager, SnapshotMode};
   ```

### Test Cases
- `test_snapshot_manager_creates_directory`
- `test_snapshot_manager_saves_and_loads`
- `test_snapshot_manager_compares_correctly`
- `test_snapshot_manager_update_mode`

---

## Plan 2: JSON Serialization Helpers

**Goal:** Ensure consistent JSON formatting for snapshots

### Files to Create
- `engine/src/testing/json.rs` - New module

### Implementation Steps

1. **Create helper functions:**
   ```rust
   pub fn serialize_working_memory(snapshot: &WorkingMemorySnapshot) -> String;
   pub fn deserialize_working_memory(json: &str) -> Result<WorkingMemorySnapshot, serde_json::Error>;
   ```

2. **Ensure deterministic output:**
   - Sort keys in all HashMaps
   - Use compact JSON for comparison

3. **Add to testing module re-exports**

### Test Cases
- `test_serialize_deterministic_order`
- `test_roundtrip_serialization`

---

## Plan 3: HTML Report Generator

**Goal:** Generate self-contained HTML reports with embedded CSS

### Files to Create
- `engine/src/testing/report/html.rs` - New module

### Implementation Steps

1. **Create report structs:**
   ```rust
   pub struct TestReport {
       tests: Vec<TestResult>,
       summary: ReportSummary,
   }
   
   pub struct ReportSummary {
       total: usize,
       passed: usize,
       failed: usize,
       duration_ms: u64,
   }
   ```

2. **Implement HTML generation:**
   - `to_html(self) -> String` method
   - Embedded CSS via `<style>` tag
   - Tables for test results and summary

3. **Add coverage metrics display**

### Test Cases
- `test_report_generates_valid_html`
- `test_report_includes_summary_stats`
- `test_report_includes_test_details`

---

## Plan 4: CLI Flag Integration

**Goal:** Add snapshot management flags to test runner

### Files to Modify
- `engine/src/testing/mod.rs` - Add CLI parsing and SnapshotMode enum

### Implementation Steps

1. **Add SnapshotMode enum:**
   ```rust
   pub enum SnapshotMode {
       Read,    // Compare against baselines (default)
       Update,  // Update baselines when different
       Skip,    // Don't compare or save
   }
   ```

2. **Parse CLI arguments:**
   - `--update-snapshots` → Set mode to Update
   - `--baseline-dir <dir>` → Set base directory

3. **Update TestSession to accept SnapshotMode**

### Test Cases
- `test_snapshot_mode_parsing`
- `test_update_mode_saves_baseline`

---

## Plan 5: Integration Tests

**Goal:** Verify filesystem operations work end-to-end

### Files to Create
- `engine/tests/snapshot_integration.rs` - New integration test file

### Implementation Steps

1. **Test fixtures setup**
   ```rust
   #[fixture]
   fn temp_snapshot_dir() -> TempDir {
       // Create temp directory for tests
   }
   ```

2. **Integration tests:**
   - Save baseline, verify file exists
   - Load baseline, compare with working memory
   - Update mode overwrites existing baseline

3. **Add to Cargo.toml test targets** if needed

### Test Cases
- `test_snapshot_filesystem_roundtrip`
- `test_update_mode_behavior`
- `test_baseline_directory_creation`

---

## Implementation Order

1. **Week 1:**
   - Plan 2 (JSON helpers) - foundation for snapshots
   - Plan 1 (SnapshotManager) - core functionality

2. **Week 2:**
   - Plan 4 (CLI flags) - user-facing feature
   - Plan 3 (HTML reports) - optional enhancement

3. **Week 3:**
   - Plan 5 (Integration tests) - verify everything works

---

## Success Criteria

| Criterion | Verification |
|-----------|-------------|
| SnapshotManager saves to filesystem | File exists at expected path |
| Baseline comparison works | Tests fail on mismatch, pass on match |
| `--update-snapshots` flag works | Baselines updated when flag present |
| HTML report generates valid output | Browser can open and render |
| Integration tests pass | All 5 new test modules pass |

---

## Backward Compatibility

All changes are additive:
- New functions in testing module (re-exports)
- New files in source tree
- Existing tests continue to work unchanged
