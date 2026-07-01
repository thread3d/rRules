//! Integration tests for snapshot filesystem operations
//!
//! These tests verify that the SnapshotManager correctly saves, loads,
//! and compares working memory snapshots to/from the filesystem.

use engine::testing::{FixtureBuilder, SnapshotMode, SnapshotManager};
use std::collections::HashMap;

/// Create a temporary directory for snapshot testing using tempfile crate
fn create_temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

#[test]
fn test_snapshot_manager_creates_directory() {
    let temp_dir = create_temp_dir();
    let manager = SnapshotManager::new(temp_dir.path());

    // The base_dir should exist after manager creation
    assert!(manager.base_dir.exists());
}

#[test]
fn test_snapshot_filesystem_roundtrip() {
    let temp_dir = create_temp_dir();
    let manager = SnapshotManager::new(temp_dir.path());

    // Create a snapshot with some facts using the proper API
    let fixture = FixtureBuilder::new()
        .with_fact("person", HashMap::from([
            ("name".to_string(), engine::data_types::Value::String("Alice".to_string())),
            ("age".to_string(), engine::data_types::Value::Integer(30)),
        ]))
        .with_fact("account", HashMap::from([
            ("status".to_string(), engine::data_types::Value::String("active".to_string())),
        ]))
        .build();

    let snapshot = engine::testing::WorkingMemorySnapshot::from_working_memory(&fixture);

    // Save the snapshot
    manager
        .save_baseline("test_roundtrip", &snapshot)
        .expect("Failed to save snapshot");

    // Verify file exists
    let snapshot_path = temp_dir.path().join("test_roundtrip.snapshot.json");
    assert!(snapshot_path.exists(), "Snapshot file should exist");

    // Load the snapshot back
    let loaded = manager
        .load_baseline("test_roundtrip")
        .expect("Failed to load snapshot");

    // Verify content matches (fact IDs will differ, but types and payloads should match)
    assert_eq!(loaded.facts.len(), snapshot.facts.len());

    // Check that both facts are present by fact type
    let loaded_types: Vec<&str> = loaded.facts.iter().map(|f| f.fact_type.as_str()).collect();
    assert!(loaded_types.contains(&"person"));
    assert!(loaded_types.contains(&"account"));
}

#[test]
fn test_snapshot_manager_update_mode() {
    let temp_dir = create_temp_dir();
    let manager = SnapshotManager::new_with_mode(temp_dir.path(), SnapshotMode::Update);

    // Initial snapshot
    let initial_fixture = FixtureBuilder::new()
        .with_fact("config", HashMap::from([
            ("version".to_string(), engine::data_types::Value::String("1".to_string())),
        ]))
        .build();

    let initial_snapshot =
        engine::testing::WorkingMemorySnapshot::from_working_memory(&initial_fixture);

    manager
        .save_baseline("test_update", &initial_snapshot)
        .expect("Failed to save initial snapshot");

    // Updated snapshot with different content
    let updated_fixture = FixtureBuilder::new()
        .with_fact("config", HashMap::from([
            ("version".to_string(), engine::data_types::Value::String("2".to_string())),
        ]))
        .build();

    let updated_snapshot =
        engine::testing::WorkingMemorySnapshot::from_working_memory(&updated_fixture);

    // In Update mode, assert_matches_baseline should update if different
    manager
        .assert_matches_baseline("test_update", &updated_snapshot)
        .expect("Update mode should succeed for different snapshot");

    // Verify the baseline was actually updated by loading it back
    let loaded = manager
        .load_baseline("test_update")
        .expect("Failed to load updated snapshot");

    // Check that version is now "2" by checking the payload
    let config_fact = loaded.facts.iter().find(|f| f.fact_type == "config").unwrap();
    let payload = &config_fact.payload;
    assert_eq!(payload.get("version"), Some(&engine::testing::ValueSnapshot::String("2".to_string())));
}

#[test]
fn test_snapshot_baseline_directory_creation() {
    let temp_dir = create_temp_dir();

    // Create a subdirectory path that doesn't exist
    let sub_dir = temp_dir.path().join("snapshots").join("baseline");

    let manager = SnapshotManager::new(&sub_dir);

    // The directory should be created when we save (handled inside save_baseline)
    let fixture = FixtureBuilder::new()
        .with_fact("test", HashMap::new())
        .build();

    let snapshot =
        engine::testing::WorkingMemorySnapshot::from_working_memory(&fixture);

    manager
        .save_baseline("test_dir_create", &snapshot)
        .expect("Failed to save");

    // Verify the full path was created
    assert!(sub_dir.exists());
}

#[test]
fn test_snapshot_manager_delete_baseline() {
    let temp_dir = create_temp_dir();
    let manager = SnapshotManager::new(temp_dir.path());

    // Create and save a snapshot
    let fixture = FixtureBuilder::new()
        .with_fact("item", HashMap::new())
        .build();

    let snapshot =
        engine::testing::WorkingMemorySnapshot::from_working_memory(&fixture);

    manager
        .save_baseline("test_delete", &snapshot)
        .expect("Failed to save");

    // Verify it exists
    assert!(manager.has_baseline("test_delete"));

    // Delete the baseline
    let deleted = manager
        .delete_baseline("test_delete")
        .expect("Delete failed");
    assert!(deleted, "Should return true when file was deleted");

    // Verify it's gone
    assert!(!manager.has_baseline("test_delete"));

    // Deleting again should return false (file didn't exist)
    let not_deleted = manager.delete_baseline("test_delete").unwrap();
    assert!(!not_deleted);
}

#[test]
fn test_snapshot_manager_has_baseline() {
    let temp_dir = create_temp_dir();
    let manager = SnapshotManager::new(temp_dir.path());

    // Initially no baseline
    assert!(!manager.has_baseline("nonexistent"));

    // Create and save a snapshot
    let fixture = FixtureBuilder::new()
        .with_fact("test", HashMap::new())
        .build();

    let snapshot =
        engine::testing::WorkingMemorySnapshot::from_working_memory(&fixture);

    manager
        .save_baseline("test_exists", &snapshot)
        .expect("Failed to save");

    // Now it should exist
    assert!(manager.has_baseline("test_exists"));
}

#[test]
fn test_snapshot_mode_parsing() {
    // Verify SnapshotMode enum values are distinct
    assert_eq!(SnapshotMode::Read, SnapshotMode::Read);
    assert_ne!(SnapshotMode::Read, SnapshotMode::Update);
    assert_ne!(SnapshotMode::Read, SnapshotMode::Skip);

    // Verify default is Read mode
    assert_eq!(SnapshotMode::default(), SnapshotMode::Read);
}

#[test]
fn test_update_mode_saves_baseline() {
    let temp_dir = create_temp_dir();
    let manager = SnapshotManager::new_with_mode(temp_dir.path(), SnapshotMode::Update);

    // Create a snapshot with specific content
    let fixture = FixtureBuilder::new()
        .with_fact("record", HashMap::from([
            ("value".to_string(), engine::data_types::Value::Integer(42)),
        ]))
        .build();

    let snapshot =
        engine::testing::WorkingMemorySnapshot::from_working_memory(&fixture);

    // In Update mode, assert_matches_baseline should save if no baseline exists
    manager
        .assert_matches_baseline("test_update_save", &snapshot)
        .expect("Update mode should succeed for new baseline");

    // Verify the file was created
    let snapshot_path = temp_dir.path().join("test_update_save.snapshot.json");
    assert!(snapshot_path.exists());

    // Load and verify content
    let loaded = manager
        .load_baseline("test_update_save")
        .expect("Failed to load");
    assert_eq!(loaded.facts.len(), 1);
}

#[test]
fn test_snapshot_manager_read_mode_rejects_mismatch() {
    let temp_dir = create_temp_dir();
    let manager = SnapshotManager::new_with_mode(temp_dir.path(), SnapshotMode::Read);

    // Initial snapshot
    let initial_fixture = FixtureBuilder::new()
        .with_fact("config", HashMap::from([
            ("version".to_string(), engine::data_types::Value::String("1".to_string())),
        ]))
        .build();

    let initial_snapshot =
        engine::testing::WorkingMemorySnapshot::from_working_memory(&initial_fixture);

    manager
        .save_baseline("test_read_mismatch", &initial_snapshot)
        .expect("Failed to save initial snapshot");

    // Different content - should fail in Read mode
    let different_fixture = FixtureBuilder::new()
        .with_fact("config", HashMap::from([
            ("version".to_string(), engine::data_types::Value::String("99".to_string())),
        ]))
        .build();

    let different_snapshot =
        engine::testing::WorkingMemorySnapshot::from_working_memory(&different_fixture);

    // In Read mode, mismatch should return an error
    assert!(manager.assert_matches_baseline("test_read_mismatch", &different_snapshot).is_err());
}

#[test]
fn test_snapshot_manager_skip_mode_always_succeeds() {
    let temp_dir = create_temp_dir();
    let manager = SnapshotManager::new_with_mode(temp_dir.path(), SnapshotMode::Skip);

    // Even if baseline doesn't exist, Skip mode should succeed
    let fixture = FixtureBuilder::new()
        .with_fact("test", HashMap::new())
        .build();

    let snapshot =
        engine::testing::WorkingMemorySnapshot::from_working_memory(&fixture);

    assert!(manager.assert_matches_baseline("nonexistent", &snapshot).is_ok());
}
