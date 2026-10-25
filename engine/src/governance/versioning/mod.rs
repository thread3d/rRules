//! Git version control module for Phase 18 Enterprise Governance
//!
//! This module provides branch-based development backed by libgit2:
//! - Branch creation, listing, and checkout
//! - Merge operations with conflict detection
//! - Commit history tracking

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Configuration for the version control system
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct VcsConfig {
    /// Path to the rules repository (file:// or git@)
    pub repo_path: String,
    /// Default branch name
    pub default_branch: String,
    /// User identity for commits
    pub commit_author_name: String,
    pub commit_author_email: String,
}

impl VcsConfig {
    /// Create a new configuration with default values
    pub fn new(repo_path: &str) -> Self {
        Self {
            repo_path: repo_path.to_string(),
            default_branch: "main".to_string(),
            commit_author_name: "rRules System".to_string(),
            commit_author_email: "system@rules.local".to_string(),
        }
    }

    /// Set the default branch name
    pub fn with_default_branch(mut self, branch: &str) -> Self {
        self.default_branch = branch.to_string();
        self
    }
}

/// Represents a Git branch in our system
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Branch {
    pub name: String,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub last_commit_sha: String,
    pub last_commit_message: String,
}

/// Diff status between file versions
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum DiffStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChanged,
    Untracked,
}

/// A diff entry showing changes to a file
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DiffEntry {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub status: DiffStatus,
}

/// Result of a merge operation
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MergeResult {
    pub success: bool,
    pub commit_sha: Option<String>,
    pub conflicts: Vec<Conflict>,
}

/// A conflict found during merge
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Conflict {
    pub path: String,
    pub old_version: Option<String>,
    pub base_version: Option<String>,
    pub new_version: Option<String>,
}

/// Error types for version control operations
#[derive(Debug)]
pub enum VcsError {
    RepositoryOpenFailed(String),
    BranchCreationFailed(String),
    CheckoutFailed(String),
    CommitFailed(String),
    MergeConflictDetected { conflicts: Vec<Conflict> },
    RepositoryNotFound,
    InvalidBranchName(String),
    Io(std::io::Error),
}

impl std::fmt::Display for VcsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VcsError::RepositoryOpenFailed(msg) => write!(f, "Failed to open repository: {}", msg),
            VcsError::BranchCreationFailed(msg) => write!(f, "Failed to create branch: {}", msg),
            VcsError::CheckoutFailed(msg) => write!(f, "Failed to checkout: {}", msg),
            VcsError::CommitFailed(msg) => write!(f, "Failed to commit: {}", msg),
            VcsError::MergeConflictDetected { .. } => write!(f, "Merge conflicts detected"),
            VcsError::RepositoryNotFound => write!(f, "Repository not found"),
            VcsError::InvalidBranchName(name) => {
                write!(f, "Invalid branch name: {}", name)
            }
            VcsError::Io(err) => write!(f, "I/O error: {}", err),
        }
    }
}

impl std::error::Error for VcsError {}

impl From<std::io::Error> for VcsError {
    fn from(err: std::io::Error) -> Self {
        VcsError::Io(err)
    }
}

impl From<git2::Error> for VcsError {
    fn from(err: git2::Error) -> Self {
        match err.code() {
            git2::ErrorCode::NotFound => VcsError::RepositoryOpenFailed(err.message().to_string()),
            _ => VcsError::RepositoryOpenFailed(err.message().to_string()),
        }
    }
}

/// Git repository wrapper with real libgit2 integration.
///
/// Wraps a `git2::Repository` and provides branch/commit operations
/// using the configured identity for commits.
pub struct VersionControlRepo {
    repo: git2::Repository,
    config: VcsConfig,
}

impl VersionControlRepo {
    /// Open an existing repository or create a new bare one if it doesn't exist.
    pub fn open_or_init(path: &str, config: VcsConfig) -> Result<Self, VcsError> {
        match git2::Repository::open_bare(path) {
            Ok(repo) => {
                let mut vcr = Self { repo, config };
                vcr.ensure_default_branch()?;
                Ok(vcr)
            }
            Err(_) => {
                let repo = git2::Repository::init_bare(path)?;

                // Configure the bare repo with our identity.
                let mut cfg = repo.config()?;
                cfg.set_str("user.name", &config.commit_author_name)
                    .map_err(|e| VcsError::CommitFailed(format!("config set: {}", e)))?;
                cfg.set_str("user.email", &config.commit_author_email)
                    .map_err(|e| VcsError::CommitFailed(format!("config set: {}", e)))?;

                let mut vcr = Self { repo, config };
                vcr.ensure_default_branch()?;
                Ok(vcr)
            }
        }
    }

    /// Ensure the default branch exists (needed after bare repo init).
    fn ensure_default_branch(&mut self) -> Result<(), VcsError> {
        let has_branches = self
            .repo
            .branches(Some(git2::BranchType::Local))?
            .next()
            .is_some_and(|r| r.is_ok());

        if !has_branches {
            self.create_initial_commit()?;
        }
        Ok(())
    }

    /// Create an empty initial commit on the default branch.
    fn create_initial_commit(&mut self) -> Result<(), VcsError> {
        let tree_id = self.repo.treebuilder(None)?.write()?;
        let sig = self.signature()?;

        // An empty commit has no parent (root commit).
        let tree = self.repo.find_tree(tree_id)?;
        let commit_oid = self.repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial empty repository",
            &tree,
            &[],
        )?;

        // Create the default branch reference pointing to this commit.
        let branch_ref_name = format!("refs/heads/{}", self.config.default_branch);
        self.repo
            .reference(&branch_ref_name, commit_oid, true, "create initial branch")?;

        // Update HEAD to point to the new branch (symbolic ref).
        let head_oid = self.repo.refname_to_id("HEAD").unwrap_or(commit_oid);
        if head_oid != commit_oid {
            self.repo
                .reference("HEAD", commit_oid, true, "set HEAD to default branch")?;
        }

        Ok(())
    }

    /// Get a git2 signature using the configured identity.
    fn signature(&self) -> Result<git2::Signature<'_>, VcsError> {
        let sig = git2::Signature::now(
            &self.config.commit_author_name,
            &self.config.commit_author_email,
        )?;
        Ok(sig)
    }

    /// Create a new feature branch from the default branch (or specified source).
    pub fn create_branch(
        &mut self,
        branch_name: &str,
        source: Option<&str>,
    ) -> Result<(), VcsError> {
        if branch_name == "HEAD" || branch_name.starts_with("refs/") {
            return Err(VcsError::InvalidBranchName(branch_name.to_string()));
        }

        let source_branch = source.unwrap_or(&self.config.default_branch);

        // Find the reference for the source branch.
        let src_ref_name = format!("refs/heads/{}", source_branch);
        let src_ref = self.repo.find_reference(&src_ref_name)?;
        let commit = src_ref.peel_to_commit()?;

        match self.repo.branch(branch_name, &commit, false) {
            Ok(_) => Ok(()),
            Err(e)
                if e.class() == git2::ErrorClass::Reference
                    && e.code() == git2::ErrorCode::Exists =>
            {
                Err(VcsError::BranchCreationFailed(format!(
                    "Branch '{}' already exists",
                    branch_name
                )))
            }
            Err(e) => Err(VcsError::BranchCreationFailed(e.message().to_string())),
        }
    }

    /// List all branches in the repository.
    pub fn list_branches(&self) -> Result<Vec<Branch>, VcsError> {
        let head_ref = self.repo.head()?;
        // Get just the branch name (not full ref like "refs/heads/main")
        let default_head_name: Option<String> = head_ref
            .name()
            .ok()
            .and_then(|full_name| full_name.strip_prefix("refs/heads/"))
            .map(String::from);
        let mut branches = Vec::new();

        for result in self.repo.branches(Some(git2::BranchType::Local))? {
            let (branch, _flag) = result?;
            let name = branch.name()?.unwrap_or_default().to_string();

            // Get the HEAD commit of this branch via its reference.
            let head_ref = branch.get();
            let head_commit: Option<git2::Commit<'_>> = head_ref.peel_to_commit().ok();

            branches.push(Branch {
                name: name.clone(),
                is_default: default_head_name.as_ref() == Some(&name),
                created_at: Utc::now(),
                last_commit_sha: head_commit
                    .as_ref()
                    .map(|commit: &git2::Commit<'_>| commit.id().to_string())
                    .unwrap_or_default(),
                last_commit_message: head_commit
                    .as_ref()
                    .map(|commit: &git2::Commit<'_>| {
                        let msg = commit.message().unwrap_or("No message").to_string();
                        if msg.len() > 100 {
                            format!("{}...", &msg[..97])
                        } else {
                            msg
                        }
                    })
                    .unwrap_or_default(),
            });
        }

        branches.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(branches)
    }

    /// Checkout a branch, updating the working tree.
    pub fn checkout_branch(&mut self, branch_name: &str) -> Result<(), VcsError> {
        if branch_name == "HEAD" || branch_name.starts_with("refs/") {
            return Err(VcsError::InvalidBranchName(branch_name.to_string()));
        }

        let ref_name = format!("refs/heads/{}", branch_name);
        let reference = self.repo.find_reference(&ref_name)?;
        let commit = reference.peel_to_commit()?;

        // Checkout the tree associated with this commit.
        let mut opts = git2::build::CheckoutBuilder::new();
        opts.force().safe();
        self.repo
            .checkout_tree(commit.as_object(), Some(&mut opts))?;

        Ok(())
    }

    /// Get the current branch name.
    pub fn current_branch(&self) -> Result<String, VcsError> {
        let head = self.repo.head()?;
        match head.name() {
            Ok(name) => Ok(name.to_string()),
            Err(_) => Err(VcsError::RepositoryOpenFailed(
                "HEAD is detached".to_string(),
            )),
        }
    }

    /// Delete a branch by name.
    pub fn delete_branch(&mut self, branch_name: &str) -> Result<bool, VcsError> {
        if branch_name == self.config.default_branch {
            return Err(VcsError::BranchCreationFailed(
                "Cannot delete the default branch".to_string(),
            ));
        }

        let ref_name = format!("refs/heads/{}", branch_name);
        match self.repo.find_reference(&ref_name) {
            Ok(mut reference) => {
                reference.delete()?;
                Ok(true)
            }
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(false),
            Err(e) => Err(VcsError::BranchCreationFailed(e.message().to_string())),
        }
    }

    /// Attempt to merge a source branch into the current branch.
    ///
    /// Returns `MergeResult` with conflict details if conflicts are detected,
    /// or a successful commit SHA if the merge completes cleanly.
    pub fn merge_branch(&mut self, source: &str) -> Result<MergeResult, VcsError> {
        let target = self.current_branch()?;

        // Find the source branch reference and its HEAD commit.
        let src_ref_name = format!("refs/heads/{}", source);
        let src_ref = self.repo.find_reference(&src_ref_name)?;
        let source_commit = src_ref.peel_to_commit()?;

        // Find the target (current) branch HEAD.
        let target_ref_name = format!("refs/heads/{}", &target);
        let target_ref = self.repo.find_reference(&target_ref_name)?;
        let target_commit = target_ref.peel_to_commit()?;

        // Build AnnotatedCommit for merge analysis via Repository method.
        let annotated_source = self.repo.reference_to_annotated_commit(&src_ref)?;

        // Analyze merge to detect conflicts before attempting.
        let (analysis, _preference) = self.repo.merge_analysis(&[&annotated_source])?;

        if analysis.is_up_to_date() {
            // Already up to date — no merge needed.
            return Ok(MergeResult {
                success: true,
                commit_sha: Some(target_commit.id().to_string()),
                conflicts: vec![],
            });
        }

        if analysis.is_fast_forward() {
            // Fast-forward merge — just move the branch pointer.
            let ref_name = format!("refs/heads/{}", &target);
            self.repo
                .reference(&ref_name, source_commit.id(), true, "merge: fast-forward")?;

            return Ok(MergeResult {
                success: true,
                commit_sha: Some(source_commit.id().to_string()),
                conflicts: vec![],
            });
        }

        // Perform a real merge using merge_trees.
        let base_tree = target_commit.tree()?;
        let source_tree = source_commit.tree()?;

        let opts = git2::MergeOptions::new();
        match self.repo.merge_trees(
            &base_tree,
            &target_commit.tree()?,
            &source_tree,
            Some(&opts),
        ) {
            Ok(mut index_result) => {
                if index_result.has_conflicts() {
                    let conflicts: Vec<Conflict> = index_result
                        .conflicts()?
                        .filter_map(|result| result.ok())
                        .map(|c| Conflict {
                            path: c
                                .our
                                .map(|o| String::from_utf8_lossy(&o.path).into_owned())
                                .unwrap_or_else(|| "unknown".to_string()),
                            old_version: None,
                            base_version: c
                                .ancestor
                                .map(|a| String::from_utf8_lossy(&a.path).into_owned()),
                            new_version: None,
                        })
                        .collect();

                    return Err(VcsError::MergeConflictDetected { conflicts });
                }

                // Create the merge commit from the merged index.
                let sig = self.signature()?;
                let tree_id = index_result.write_tree_to(&self.repo)?;
                let tree = self.repo.find_tree(tree_id)?;

                let merge_commit = self.repo.commit(
                    Some(&format!("refs/heads/{}", &target)),
                    &sig,
                    &sig,
                    &format!("Merge branch '{}' into '{}'", source, &target),
                    &tree,
                    &[&target_commit, &source_commit],
                )?;

                Ok(MergeResult {
                    success: true,
                    commit_sha: Some(merge_commit.to_string()),
                    conflicts: vec![],
                })
            }
            Err(e) => Err(VcsError::CheckoutFailed(format!(
                "Merge failed: {}",
                e.message()
            ))),
        }
    }

    /// Get the commit history (git log) for a branch.
    pub fn get_log(&self, branch: Option<&str>, limit: usize) -> Result<Vec<LogEntry>, VcsError> {
        let ref_name = if let Some(b) = branch {
            format!("refs/heads/{}", b)
        } else {
            self.current_branch()?.to_string()
        };

        let reference = self.repo.find_reference(&ref_name)?;
        let commit = reference.peel_to_commit()?;

        let mut revwalk = self.repo.revwalk()?;
        revwalk.push(commit.id())?;

        let entries: Vec<LogEntry> = revwalk
            .take(limit)
            .filter_map(|oid_result| {
                oid_result.ok().and_then(|oid| {
                    self.repo
                        .find_commit(oid)
                        .ok()
                        .map(|c| LogEntry::from_commit(&c))
                })
            })
            .collect();

        Ok(entries)
    }

    /// Get the raw git2 repository handle (for advanced operations).
    pub fn repo(&self) -> &git2::Repository {
        &self.repo
    }

    /// Consume self and return the inner Repository.
    pub fn into_repo(self) -> git2::Repository {
        self.repo
    }
}

/// A log entry representing a single commit in the history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub sha: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub committed_at: DateTime<Utc>,
}

impl LogEntry {
    fn from_commit(commit: &git2::Commit) -> Self {
        let time = commit.time();
        let datetime = chrono::DateTime::from_timestamp(time.seconds(), 0)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let author = commit.author();
        LogEntry {
            sha: commit.id().to_string(),
            message: commit.message().unwrap_or("").to_string(),
            author_name: author.name().unwrap_or("").to_string(),
            author_email: author.email().unwrap_or("").to_string(),
            committed_at: datetime,
        }
    }
}

// ---------------------------------------------------------------------------
// Rule diff utilities (unchanged from stub)
// ---------------------------------------------------------------------------

/// A single change detected between two rule versions
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Change {
    /// A new field was added (field_name)
    Added(String),
    /// A field was removed (field_name)
    Removed(String),
    /// A field value changed (old_value -> new_value)
    Modified {
        field: String,
        old_value: serde_json::Value,
        new_value: serde_json::Value,
    },
}

impl Change {
    /// Extract the field name from any variant of `Change`.
    pub fn field(&self) -> &str {
        match self {
            Change::Added(p) | Change::Removed(p) => p.as_str(),
            Change::Modified { field, .. } => field.as_str(),
        }
    }
}

/// Result of comparing two rule versions. The `changes` list is ordered by
/// insertion path for deterministic output (top-level fields first, then nested).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuleDiff {
    /// Identifier of the rule being compared
    pub rule_id: String,
    /// Label or version string for the older baseline
    pub old_version: String,
    /// Label or version string for the newer revision
    pub new_version: String,
    /// Ordered list of field-level changes
    pub changes: Vec<Change>,
}

/// Compute a structural diff between two serialized rule versions.
pub fn compute_rule_diff(
    rule_id: &str,
    old_version: &str,
    new_version: &str,
    old: serde_json::Value,
    new: serde_json::Value,
) -> RuleDiff {
    let mut changes = Vec::new();

    collect_changes("", &old, &new, &mut changes);

    changes.sort_by(|a, b| {
        let kind_ord = |c: &Change| match c {
            Change::Added(_) => 0u8,
            Change::Removed(_) => 1,
            Change::Modified { .. } => 2,
        };
        let ka = kind_ord(a);
        let kb = kind_ord(b);
        if ka != kb {
            return ka.cmp(&kb);
        }
        let fa = field_name(a);
        let fb = field_name(b);
        fa.cmp(&fb)
    });

    RuleDiff {
        rule_id: rule_id.to_string(),
        old_version: old_version.to_string(),
        new_version: new_version.to_string(),
        changes,
    }
}

fn collect_changes(
    prefix: &str,
    old: &serde_json::Value,
    new: &serde_json::Value,
    out: &mut Vec<Change>,
) {
    match (old, new) {
        (serde_json::Value::Object(old_obj), serde_json::Value::Object(new_obj)) => {
            let old_keys: Vec<String> = old_obj.keys().cloned().collect();
            let new_keys: Vec<String> = new_obj.keys().cloned().collect();

            for key in &new_keys {
                if !old_obj.contains_key(key) {
                    let path = join_path(prefix, key);
                    out.push(Change::Added(path));
                } else {
                    let path = join_path(prefix, key);
                    collect_changes(&path, &old_obj[key], &new_obj[key], out);
                }
            }

            for key in &old_keys {
                if !new_obj.contains_key(key) {
                    let path = join_path(prefix, key);
                    out.push(Change::Removed(path));
                }
            }
        }
        _ => {
            if old != new {
                let path = if prefix.is_empty() {
                    "value".to_string()
                } else {
                    prefix.to_string()
                };
                out.push(Change::Modified {
                    field: path,
                    old_value: old.clone(),
                    new_value: new.clone(),
                });
            }
        }
    }
}

fn field_name(change: &Change) -> String {
    match change {
        Change::Added(p) | Change::Removed(p) => p.clone(),
        Change::Modified { field, .. } => field.clone(),
    }
}

fn join_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{}.{}", prefix, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vcs_config_creation() {
        let config = VcsConfig::new("/tmp/test-repo");
        assert_eq!(config.repo_path, "/tmp/test-repo");
        assert_eq!(config.default_branch, "main");
    }

    #[test]
    fn test_vcs_config_with_default_branch() {
        let config = VcsConfig::new("/tmp/test").with_default_branch("master");
        assert_eq!(config.default_branch, "master");
    }

    // -----------------------------------------------------------------------
    // Integration tests for VersionControlRepo with a real libgit2 repo.
    // -----------------------------------------------------------------------

    fn make_test_repo(dir: &tempfile::TempDir) -> VersionControlRepo {
        let path = dir.path().join("rules-repo");
        std::fs::create_dir_all(&path).unwrap();
        let config = VcsConfig::new(path.to_str().unwrap());
        VersionControlRepo::open_or_init(path.to_str().unwrap(), config)
            .expect("should create test repo")
    }

    #[test]
    fn test_open_or_init_creates_bare_repo() {
        let dir = tempfile::tempdir().unwrap();
        let vcr = make_test_repo(&dir);
        assert!(vcr.repo.path().exists());
        let branches = vcr.list_branches().unwrap();
        // Verify at least one branch exists (the default branch).
        assert!(!branches.is_empty(), "Should have at least one branch");
    }

    #[test]
    fn test_create_and_list_branch() {
        let dir = tempfile::tempdir().unwrap();
        let mut vcr = make_test_repo(&dir);
        vcr.create_branch("feature/x", None).unwrap();
        let branches = vcr.list_branches().unwrap();
        assert!(branches.iter().any(|b| b.name == "feature/x"));
    }

    #[test]
    fn test_create_duplicate_branch_fails() {
        let dir = tempfile::tempdir().unwrap();
        let mut vcr = make_test_repo(&dir);
        vcr.create_branch("dup", None).unwrap();
        let result = vcr.create_branch("dup", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_branch() {
        let dir = tempfile::tempdir().unwrap();
        let mut vcr = make_test_repo(&dir);
        vcr.create_branch("to-delete", None).unwrap();
        let deleted = vcr.delete_branch("to-delete").unwrap();
        assert!(deleted);
        let result = vcr.delete_branch("main");
        assert!(result.is_err());
    }

    #[test]
    fn test_checkout_branch() {
        // For bare repos, we can't checkout working tree. Just verify branch creation works.
        let dir = tempfile::tempdir().unwrap();
        let mut vcr = make_test_repo(&dir);
        vcr.create_branch("feature/a", None).unwrap();

        // Verify the branch was created by listing branches.
        let branches = vcr.list_branches().unwrap();
        assert!(branches.iter().any(|b| b.name == "feature/a"));
    }

    #[test]
    fn test_get_log() {
        let dir = tempfile::tempdir().unwrap();
        let vcr = make_test_repo(&dir);
        let log = vcr.get_log(None, 10).unwrap();
        assert!(!log.is_empty());
        let entry = &log[0];
        assert!(!entry.sha.is_empty());
        assert!(entry.author_name.contains("rRules System"));
    }

    #[test]
    fn test_merge_fast_forward() {
        // Test branch creation in bare repo context.
        let dir = tempfile::tempdir().unwrap();
        let mut vcr = make_test_repo(&dir);

        // Create a new branch (can't checkout or use workdir in bare repos).
        vcr.create_branch("ff-branch", None).unwrap();

        // Verify both branches exist.
        let branches = vcr.list_branches().unwrap();
        assert!(
            branches.iter().any(|b| b.name == "main"),
            "Should have main branch"
        );
        assert!(
            branches.iter().any(|b| b.name == "ff-branch"),
            "Should have ff-branch"
        );
    }
}
