//! Snapshot baseline management for testing framework
//!
//! This module provides file-based storage and comparison of working memory snapshots,
//! enabling snapshot-based verification with filesystem persistence.

use crate::testing::WorkingMemorySnapshot;
use serde_json;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Snapshot mode for controlling baseline behavior
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapshotMode {
    /// Compare against baselines (default)
    Read,
    /// Update baselines when different
    Update,
    /// Don't compare or save
    Skip,
}

impl Default for SnapshotMode {
    fn default() -> Self {
        SnapshotMode::Read
    }
}

/// Error type for snapshot operations
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("Failed to read snapshot file: {0}")]
    ReadError(String),
    #[error("Failed to write snapshot file: {0}")]
    WriteError(String),
    #[error("Invalid JSON in snapshot file: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Snapshot not found: {0}")]
    NotFound(String),
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
}

/// Result type for snapshot operations
pub type SnapshotResult<T> = Result<T, SnapshotError>;

/// Manages snapshot baselines with filesystem storage and comparison
///
/// The `SnapshotManager` provides utilities to:
/// - Save working memory snapshots to files
/// - Load baseline snapshots from disk
/// - Compare current state against baselines
/// - Update baselines when needed
///
/// # Example
/// ```ignore
/// use engine::testing::{SnapshotManager, SnapshotMode};
/// use std::collections::HashMap;
///
/// let temp_dir = std::env::temp_dir().join("test_snapshots");
/// let manager = SnapshotManager::new(&temp_dir);
///
/// // Load a baseline and compare
/// if let Ok(baseline) = manager.load_baseline("my_test") {
///     // Compare with current state
/// }
///
/// // Save or update baselines - requires &WorkingMemorySnapshot
/// // manager.save_baseline("my_test", &snapshot)?;
/// ```
pub struct SnapshotManager {
    /// Base directory for snapshot storage (public for testing)
    pub base_dir: PathBuf,
    /// Current snapshot mode (read, update, skip)
    mode: SnapshotMode,
}

impl SnapshotManager {
    /// Create a new snapshot manager with the given base directory
    ///
    /// The manager will use `SnapshotMode::Read` by default.
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            mode: SnapshotMode::Read,
        }
    }

    /// Create a new snapshot manager with the given base directory and mode
    pub fn new_with_mode(base_dir: impl AsRef<Path>, mode: SnapshotMode) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            mode,
        }
    }

    /// Get the current snapshot mode
    pub fn mode(&self) -> SnapshotMode {
        self.mode
    }

    /// Set the snapshot mode for subsequent operations
    pub fn set_mode(&mut self, mode: SnapshotMode) {
        self.mode = mode;
    }

    /// Get the path to a snapshot file for the given test name
    fn snapshot_path(&self, test_name: impl AsRef<str>) -> PathBuf {
        self.base_dir.join(format!("{}.snapshot.json", test_name.as_ref()))
    }

    /// Ensure the base directory exists, creating it if necessary
    fn ensure_base_dir_exists(&self) -> SnapshotResult<()> {
        fs::create_dir_all(&self.base_dir)
            .map_err(|e| SnapshotError::WriteError(format!(
                "Failed to create directory '{}': {}",
                self.base_dir.display(),
                e
            )))?;
        Ok(())
    }

    /// Load a baseline snapshot from disk
    ///
    /// Returns the snapshot if it exists, or an error if not found.
    pub fn load_baseline(&self, test_name: impl AsRef<str>) -> SnapshotResult<WorkingMemorySnapshot> {
        let path = self.snapshot_path(test_name);

        if !path.exists() {
            return Err(SnapshotError::NotFound(format!(
                "Baseline snapshot not found: {}",
                path.display()
            )));
        }

        let json = fs::read_to_string(&path)
            .map_err(|e| SnapshotError::ReadError(format!(
                "Failed to read '{}': {}",
                path.display(),
                e
            )))?;

        match WorkingMemorySnapshot::from_json(&json) {
            Ok(snapshot) => Ok(snapshot),
            Err(e) => Err(SnapshotError::JsonError(e)),
        }
    }

    /// Save a snapshot as the new baseline
    ///
    /// Creates the directory if it doesn't exist.
    pub fn save_baseline(
        &self,
        test_name: impl AsRef<str>,
        snapshot: &WorkingMemorySnapshot,
    ) -> SnapshotResult<()> {
        self.ensure_base_dir_exists()?;

        let path = self.snapshot_path(test_name);
        let json = serde_json::to_string_pretty(snapshot)
            .map_err(|e| SnapshotError::WriteError(format!(
                "Failed to serialize snapshot: {}",
                e
            )))?;

        fs::write(&path, &json)
            .map_err(|e| SnapshotError::WriteError(format!(
                "Failed to write '{}': {}",
                path.display(),
                e
            )))?;

        Ok(())
    }

    /// Update an existing baseline with a new snapshot
    ///
    /// This is an alias for `save_baseline` - both will overwrite the file.
    pub fn update_baseline(
        &self,
        test_name: impl AsRef<str>,
        snapshot: &WorkingMemorySnapshot,
    ) -> SnapshotResult<()> {
        self.save_baseline(test_name, snapshot)
    }

    /// Compare current state against baseline and take action based on mode
    ///
    /// - `Read`: Returns error if different from baseline
    /// - `Update`: Updates baseline and returns success if different
    /// - `Skip`: Always returns success without comparison
    pub fn assert_matches_baseline(
        &self,
        test_name: impl AsRef<str>,
        current: &WorkingMemorySnapshot,
    ) -> SnapshotResult<()> {
        let name = test_name.as_ref().to_string();

        match self.mode {
            SnapshotMode::Skip => Ok(()),

            SnapshotMode::Read => {
                let baseline = self.load_baseline(&name)?;
                if baseline.compare(current).is_empty() {
                    Ok(())
                } else {
                    let diff = baseline.compare(current);
                    Err(SnapshotError::WriteError(format!(
                        "Snapshot mismatch:\n{}",
                        diff.describe()
                    )))
                }
            }

            SnapshotMode::Update => {
                // Load existing snapshot to compare
                match self.load_baseline(&name) {
                    Ok(baseline) => {
                        if baseline.compare(current).is_empty() {
                            // No changes needed
                            Ok(())
                        } else {
                            // Update the baseline
                            self.update_baseline(&name, current)
                        }
                    }
                    Err(SnapshotError::NotFound(_)) => {
                        // No existing baseline - create it
                        self.save_baseline(&name, current)
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

    /// Check if a baseline snapshot exists for the given test name
    pub fn has_baseline(&self, test_name: impl AsRef<str>) -> bool {
        self.snapshot_path(test_name).exists()
    }

    /// Delete a baseline snapshot file
    ///
    /// Returns Ok(true) if the file was deleted, Ok(false) if it didn't exist.
    pub fn delete_baseline(&self, test_name: impl AsRef<str>) -> SnapshotResult<bool> {
        let path = self.snapshot_path(test_name);

        if !path.exists() {
            return Ok(false);
        }

        fs::remove_file(&path)
            .map_err(|e| SnapshotError::WriteError(format!(
                "Failed to delete '{}': {}",
                path.display(),
                e
            )))?;

        Ok(true)
    }
}

/// Test helper for creating snapshot managers in tests
///
/// Usage:
/// ```no_run
/// use engine::testing::{FixtureBuilder, SnapshotManager};
///
/// let temp_dir = std::env::temp_dir().join("test_snapshots");
/// let manager = SnapshotManager::new(&temp_dir);
/// ```
pub fn create_snapshot_manager(base_dir: impl AsRef<Path>) -> SnapshotManager {
    SnapshotManager::new(base_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ValueSnapshot;
    use std::collections::HashMap;

    #[test]
    fn test_snapshot_manager_creates_directory() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

        // Manager should not error on creation even if directory doesn't exist
        let manager = SnapshotManager::new(temp_dir.path());

        // Directory should be created on first use
        assert!(manager.base_dir.exists());
    }

    #[test]
    fn test_snapshot_manager_saves_and_loads() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let manager = SnapshotManager::new(temp_dir.path());

        // Create a test snapshot
        let mut payload = HashMap::new();
        payload.insert("name".to_string(), ValueSnapshot::String("test".to_string()));
        payload.insert("value".to_string(), ValueSnapshot::Integer(42));

        let snapshot = WorkingMemorySnapshot {
            facts: vec![
                crate::testing::FactSnapshot {
                    id: "fact1".to_string(),
                    fact_type: "item".to_string(),
                    payload,
                }
            ],
        };

        // Save baseline
        manager.save_baseline("test_save_load", &snapshot).expect("Failed to save");

        // Verify file exists
        let snapshot_path = temp_dir.path().join("test_save_load.snapshot.json");
        assert!(snapshot_path.exists());

        // Load and verify
        let loaded = manager.load_baseline("test_save_load").expect("Failed to load");

        assert_eq!(loaded.facts.len(), 1);
        assert_eq!(loaded.facts[0].fact_type, "item");
    }

    #[test]
    fn test_snapshot_manager_compares_correctly() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let mut manager = SnapshotManager::new(temp_dir.path());

        // Create and save a baseline
        let baseline_payload = HashMap::from([
            ("name".to_string(), ValueSnapshot::String("original".to_string())),
        ]);

        let snapshot = WorkingMemorySnapshot {
            facts: vec![crate::testing::FactSnapshot {
                id: "fact1".to_string(),
                fact_type: "item".to_string(),
                payload: baseline_payload.clone(),
            }],
        };

        manager
            .save_baseline("test_compare", &snapshot)
            .expect("Failed to save");

        // Same content should match
        let same_snapshot = WorkingMemorySnapshot {
            facts: vec![crate::testing::FactSnapshot {
                id: "fact2".to_string(),  // Different ID but same content
                fact_type: "item".to_string(),
                payload: baseline_payload.clone(),
            }],
        };

        assert!(manager.assert_matches_baseline("test_compare", &same_snapshot).is_ok());

        // Different content should fail in Read mode
        let different_payload = HashMap::from([
            ("name".to_string(), ValueSnapshot::String("different".to_string())),
        ]);

        let different_snapshot = WorkingMemorySnapshot {
            facts: vec![crate::testing::FactSnapshot {
                id: "fact3".to_string(),
                fact_type: "item".to_string(),
                payload: different_payload,
            }],
        };

        assert!(manager.assert_matches_baseline("test_compare", &different_snapshot).is_err());
    }

    #[test]
    fn test_snapshot_manager_update_mode() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let mut manager = SnapshotManager::new_with_mode(temp_dir.path(), SnapshotMode::Update);

        // Create and save initial baseline
        let initial_payload = HashMap::from([
            ("name".to_string(), ValueSnapshot::String("initial".to_string())),
        ]);

        let initial_snapshot = WorkingMemorySnapshot {
            facts: vec![crate::testing::FactSnapshot {
                id: "fact1".to_string(),
                fact_type: "item".to_string(),
                payload: initial_payload.clone(),
            }],
        };

        manager
            .save_baseline("test_update", &initial_snapshot)
            .expect("Failed to save");

        // Update with different content
        let updated_payload = HashMap::from([
            ("name".to_string(), ValueSnapshot::String("updated".to_string())),
        ]);

        let updated_snapshot = WorkingMemorySnapshot {
            facts: vec![crate::testing::FactSnapshot {
                id: "fact2".to_string(),
                fact_type: "item".to_string(),
                payload: updated_payload.clone(),
            }],
        };

        // Update should succeed (and update the baseline)
        assert!(manager
            .assert_matches_baseline("test_update", &updated_snapshot)
            .is_ok());

        // Verify baseline was updated by loading it back
        let loaded = manager.load_baseline("test_update").expect("Failed to load");

        assert_eq!(
            loaded.facts[0].payload.get("name"),
            Some(&ValueSnapshot::String("updated".to_string()))
        );
    }

    #[test]
    fn test_snapshot_manager_skip_mode() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let mut manager = SnapshotManager::new_with_mode(temp_dir.path(), SnapshotMode::Skip);

        // Different content should not fail in Skip mode
        let snapshot = WorkingMemorySnapshot {
            facts: vec![crate::testing::FactSnapshot {
                id: "fact1".to_string(),
                fact_type: "item".to_string(),
                payload: HashMap::from([
                    ("name".to_string(), ValueSnapshot::String("test".to_string())),
                ]),
            }],
        };

        assert!(manager.assert_matches_baseline("nonexistent", &snapshot).is_ok());
    }

    #[test]
    fn test_snapshot_manager_has_baseline() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let manager = SnapshotManager::new(temp_dir.path());

        // No baseline initially
        assert!(!manager.has_baseline("nonexistent"));

        // Create a snapshot
        let snapshot = WorkingMemorySnapshot {
            facts: vec![crate::testing::FactSnapshot {
                id: "fact1".to_string(),
                fact_type: "item".to_string(),
                payload: HashMap::new(),
            }],
        };

        manager.save_baseline("test_exists", &snapshot).expect("Failed to save");

        assert!(manager.has_baseline("test_exists"));
    }

    #[test]
    fn test_snapshot_manager_delete_baseline() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let manager = SnapshotManager::new(temp_dir.path());

        // Create a snapshot
        let snapshot = WorkingMemorySnapshot {
            facts: vec![crate::testing::FactSnapshot {
                id: "fact1".to_string(),
                fact_type: "item".to_string(),
                payload: HashMap::new(),
            }],
        };

        manager.save_baseline("test_delete", &snapshot).expect("Failed to save");

        // Verify it exists
        assert!(manager.has_baseline("test_delete"));

        // Delete it
        let deleted = manager.delete_baseline("test_delete").expect("Delete failed");
        assert!(deleted);

        // Verify it's gone
        assert!(!manager.has_baseline("test_delete"));
    }

    #[test]
    fn test_snapshot_manager_nonexistent_baseline() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let manager = SnapshotManager::new(temp_dir.path());

        // Loading nonexistent baseline should error
        let result = manager.load_baseline("nonexistent");

        assert!(matches!(result, Err(SnapshotError::NotFound(_))));
    }
}
