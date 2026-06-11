# Phase 18-04: Git Version Control - Plan

**Phase:** 18.04  
**Goal:** Git-backed version control for rules with branch-based development and merge workflows  
**Depends on:** Phase 18-03 (audit trail infrastructure)  

---

## Overview

Implement Git-integrated versioning:
- **Repository structure:** Single repo with feature branches per rule set
- **Operations:** Branch creation, merging, conflict detection via libgit2
- **Integration:** Audit trail for all version control actions
- **Frontend:** Branch viewer and merge conflict visualization

---

## Success Criteria (What Must Be TRUE)

1. Users can create branches for rule development without affecting production
2. Feature branches merge to main with automatic conflict detection
3. View history of all changes to rules via Git log
4. Side-by-side diff view comparing rule versions
5. Merge conflicts are detected and displayed before merging

---

## Implementation Tasks

### Repository Manager (Wave 0)

| File | Purpose |
|------|---------|
| `src/governance/versioning/repository.rs` | libgit2 wrapper for repo operations |

```rust
// src/governance/versioning/types.rs
use chrono::{DateTime, Utc};
use git2::{Commit, Repository as GitRepo, Branch};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for the version control system
#[derive(Clone, Serialize, Deserialize)]
pub struct VcsConfig {
    /// Path to the rules repository (file:// or git@)
    pub repo_path: String,
    /// Default branch name
    pub default_branch: String,
    /// User identity for commits
    pub commit_author_name: String,
    pub commit_author_email: String,
}

/// Represents a Git branch in our system
#[derive(Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub last_commit_sha: String,
    pub last_commit_message: String,
}

/// Diff between two commits or trees
#[derive(Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub status: DiffStatus,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum DiffStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChanged,
    Untracked,
}

/// Merge result with conflict information
#[derive(Clone, Serialize, Deserialize)]
pub struct MergeResult {
    pub success: bool,
    pub commit_sha: Option<String>,
    pub conflicts: Vec<Conflict>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub path: String,
    pub old_version: Option<String>,
    pub base_version: Option<String>,
    pub new_version: Option<String>,
}
```

---

### Repository Operations (Wave 0)

| File | Purpose |
|------|---------|
| `src/governance/versioning/repository.rs` | Git repository operations |

```rust
// src/governance/versioning/repository.rs
use crate::governance::versioning::types::*;
use git2::{Repository, Branch, ObjectType, PushOptions, RemoteCallbacks};
use std::path::Path;

pub struct VersionControlRepo {
    repo: Repository,
    config: VcsConfig,
}

impl VersionControlRepo {
    pub fn open(path: &str, config: VcsConfig) -> Result<Self, VcsError> {
        let repo = Repository::open(path)
            .map_err(|e| VcsError::RepositoryOpenFailed(e.to_string()))?;
        
        Ok(Self { repo, config })
    }

    /// Create a new feature branch from the default branch
    pub fn create_branch(&self, branch_name: &str) -> Result<(), VcsError> {
        let default_ref = self.repo.refname_to_id(format!("refs/heads/{}", self.config.default_branch).as_str())?;
        let commit = self.repo.find_commit(default_ref)?;;
        
        self.repo.branch(branch_name, &commit, false)?;
        Ok(())
    }

    /// List all branches in the repository
    pub fn list_branches(&self) -> Result<Vec<Branch>, VcsError> {
        let mut branches = Vec::new();
        
        for branch_result in self.repo.branches(Some(git2::BranchType::Local))? {
            let (branch, _) = branch_result?;
            
            if let Some(name) = branch.name()? {
                branches.push(Branch {
                    name: name.to_string(),
                    is_default: name == self.config.default_branch,
                    created_at: Utc::now(),  // Could track creation time in a separate table
                    last_commit_sha: String::new(),  // Populate from branch target
                    last_commit_message: String::new(),
                });
            }
        }
        
        Ok(branches)
    }

    /// Checkout a branch for editing
    pub fn checkout_branch(&self, branch_name: &str) -> Result<(), VcsError> {
        let reference = self.repo.find_reference(format!("refs/heads/{}", branch_name).as_str())?;
        self.repo.checkout(reference.as_object(), Some(
            git2::build::CheckoutBuilder::new().force()
        ))?;
        
        Ok(())
    }

    /// Stage a rule file for commit
    pub fn stage_file(&self, path: &str) -> Result<(), VcsError> {
        let mut index = self.repo.index()?;
        index.add_path(Path::new(path))?;
        index.write()?;
        Ok(())
    }

    /// Commit staged changes with the given message
    pub fn commit(&self, message: &str, author_email: Option<&str>) -> Result<String, VcsError> {
        let mut index = self.repo.index()?;
        let tree_id = index.write_tree()?;
        
        let signature = self.repo.signature()?;
        let tree = self.repo.find_tree(tree_id)?;
        
        // Get parent commits
        let head = self.repo.head()?;
        let parent_commit = self.repo.find_commit(head.target().unwrap())?;
        
        // Create commit
        let commit_id = self.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent_commit],
        )?;
        
        Ok(commit_id.to_string())
    }
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
}
```

---

### Merge Operations (Wave 1)

| File | Purpose |
|------|---------|
| `src/governance/versioning/merge.rs` | Branch merging with conflict detection |

```rust
// src/governance/versioning/merge.rs
use crate::governance::versioning::repository::VersionControlRepo;
use crate::governance::versioning::types::*;
use git2::{Tree, MergeOptions};

impl VersionControlRepo {
    /// Prepare a branch for merging (check conflicts before actual merge)
    pub fn prepare_merge(&self, source_branch: &str, target_branch: &str) -> Result<MergeResult, VcsError> {
        let target_ref = self.repo.find_reference(format!("refs/heads/{}", target_branch).as_str())?;
        let target_commit = target_ref.peel_to_commit()?;
        
        let source_ref = self.repo.find_reference(format!("refs/heads/{}", source_branch).as_str())?;
        let source_commit = source_ref.peel_to_commit()?;
        
        // Find merge base
        let merge_base = self.repo.merge_base(target_commit.id(), source_commit.id())
            .map_err(|e| VcsError::MergeConflictDetected { conflicts: vec![] })?;
        
        let tree_ancestor = self.repo.find_tree(merge_base)?;
        let tree_target = target_commit.tree()?;
        let tree_source = source_commit.tree()?;
        
        // Check for merge conflicts
        let mut options = MergeOptions::new();
        match self.repo.merge_trees(&tree_ancestor, &tree_target, &tree_source, Some(&mut options)) {
            Ok(merged_tree) => {
                if merged_tree.has_conflicts() {
                    // Extract conflict information
                    let conflicts = self.extract_conflicts(&merged_tree)?;
                    
                    Ok(MergeResult {
                        success: false,
                        commit_sha: None,
                        conflicts,
                    })
                } else {
                    // No conflicts, perform the actual merge
                    self.perform_merge(source_branch, target_branch)
                }
            }
            Err(e) => Err(VcsError::MergeConflictDetected { conflicts: vec![] }),
        }
    }

    /// Perform the actual merge after conflict resolution
    pub fn perform_merge(&self, source_branch: &str, target_branch: &str) -> Result<MergeResult, VcsError> {
        let target_ref = self.repo.find_reference(format!("refs/heads/{}", target_branch).as_str())?;
        let target_commit = target_ref.peel_to_commit()?;
        
        let source_ref = self.repo.find_reference(format!("refs/heads/{}", source_branch).as_str())?;
        let source_commit = source_ref.peel_to_commit()?;
        
        // Merge trees
        let merge_base = self.repo.merge_base(target_commit.id(), source_commit.id())?;
        let tree_ancestor = self.repo.find_tree(merge_base)?;
        let tree_target = target_commit.tree()?;
        let tree_source = source_commit.tree()?;
        
        let mut options = MergeOptions::new();
        let merged_tree = self.repo.merge_trees(&tree_ancestor, &tree_target, &tree_source, Some(&mut options))?;
        
        if merged_tree.has_conflicts() {
            let conflicts = self.extract_conflicts(&merged_tree)?;
            return Ok(MergeResult {
                success: false,
                commit_sha: None,
                conflicts,
            });
        }
        
        // Create merge commit
        let signature = self.repo.signature()?;
        let target_tree = self.repo.find_tree(merged_tree.id())?;
        let merge_commit = self.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &format!("Merge {} into {}", source_branch, target_branch),
            &target_tree,
            &[&target_commit],
        )?;
        
        // Update target branch reference
        self.repo.set_head_detached(merge_commit)?;
        self.repo.branch(target_branch, &merge_commit, true)?;
        
        Ok(MergeResult {
            success: true,
            commit_sha: Some(merge_commit.to_string()),
            conflicts: vec![],
        })
    }

    /// Extract detailed conflict information from a merge tree
    fn extract_conflicts(&self, merged_tree: &Tree) -> Result<Vec<Conflict>, VcsError> {
        let mut conflicts = Vec::new();
        
        // Get the three trees for each conflict
        // (This is a simplified version - actual implementation would iterate over conflicts)
        
        Ok(conflicts)
    }
}
```

---

### Branch Manager Service (Wave 1)

| File | Purpose |
|------|---------|
| `src/governance/versioning/manager.rs` | High-level branch management API |

```rust
// src/governance/versioning/manager.rs
use crate::governance::versioning::{repository, types, merge};
use crate::governance::audit::AuditStore;
use chrono::Utc;

pub struct VersionManager {
    repo: repository::VersionControlRepo,
    audit: AuditStore,
}

impl VersionManager {
    pub fn new(repo_path: &str, config: VcsConfig, audit: AuditStore) -> Self {
        let repo = repository::VersionControlRepo::open(repo_path, config)
            .expect("Failed to open version control repository");
        
        Self { repo, audit }
    }

    /// Create a new feature branch for rule development
    pub async fn create_branch(&self, name: &str) -> Result<(), VcsError> {
        // Check if user has permission (author or admin)
        // ... permission check ...
        
        self.repo.create_branch(name)?;
        
        // Log to audit trail
        self.audit.append_event(&AuditEvent::BranchCreated {
            branch_name: name.to_string(),
            created_by: "current_user_id".to_string(),  // From JWT claims
        }).await?;
        
        Ok(())
    }

    /// Merge a feature branch into production (main)
    pub async fn merge_branch(&self, source: &str, target: &str) -> Result<MergeResult, VcsError> {
        // Check user permission to merge
        // ... permission check ...
        
        let result = self.repo.prepare_merge(source, target)?;
        
        if !result.success && !result.conflicts.is_empty() {
            // Log conflict detection
            self.audit.append_event(&AuditEvent::MergeConflictDetected {
                source_branch: source.to_string(),
                target_branch: target.to_string(),
                conflict_count: result.conflicts.len() as i32,
            }).await?;
            
            return Ok(result);
        }
        
        if result.success {
            // Log successful merge
            self.audit.append_event(&AuditEvent::BranchMerged {
                source_branch: source.to_string(),
                target_branch: target.to_string(),
                commit_sha: result.commit_sha.clone().unwrap_or_default(),
            }).await?;
        }
        
        Ok(result)
    }

    /// Get all branches with their status
    pub async fn list_all_branches(&self) -> Result<Vec<BranchInfo>, VcsError> {
        let branches = self.repo.list_branches()?;
        
        let mut result = Vec::new();
        for branch in branches {
            result.push(BranchInfo {
                name: branch.name,
                is_default: branch.is_default,
                rules_changed: 0,  // Could count changed files
                last_modified: branch.created_at,
            });
        }
        
        Ok(result)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_default: bool,
    pub rules_changed: i32,
    pub last_modified: DateTime<Utc>,
}
```

---

### API Endpoints (Wave 2)

| Endpoint | Method | Role Required | Purpose |
|----------|--------|---------------|---------|
| `/api/v1/branches` | GET | viewer | List all branches |
| `/api/v1/branches` | POST | author/admin | Create new branch |
| `/api/v1/branches/{name}/merge` | POST | admin | Merge branch to main |
| `/api/v1/rules/{rule_id}/history` | GET | viewer | View rule version history |

```rust
// src/api/v1/routes/branches.rs
use actix_web::{get, post, web, HttpResponse};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreateBranchRequest {
    pub name: String,
}

#[post("/branches")]
async fn create_branch(
    request: web::Json<CreateBranchRequest>,
    version_manager: web::Data<Arc<Mutex<VersionManager>>>,
) -> Result<HttpResponse, AppError> {
    version_manager.lock().await.create_branch(&request.name).await?;
    
    Ok(HttpResponse::Created().json(serde_json::json!({
        "success": true,
        "branch_name": request.name
    })))
}

#[derive(Deserialize)]
pub struct MergeRequest {
    pub source: String,
    pub target: String,
}

#[post("/branches/{name}/merge")]
async fn merge_branch(
    path: web::Path<String>,
    request: web::Json<MergeRequest>,
    version_manager: web::Data<Arc<Mutex<VersionManager>>>,
) -> Result<HttpResponse, AppError> {
    let branch_name = path.into_inner();
    
    // Allow merge with source=branch_name for convenience
    let result = version_manager.lock().await.merge_branch(&branch_name, &request.target).await?;
    
    if result.success {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "commit_sha": result.commit_sha
        })))
    } else {
        // Return conflict details for user resolution
        Ok(HttpResponse::Conflict().json(serde_json::json!({
            "success": false,
            "conflicts": result.conflicts
        })))
    }
}

#[get("/rules/{rule_id}/history")]
async fn get_rule_history(
    path: web::Path<Uuid>,
    version_manager: web::Data<Arc<Mutex<VersionManager>>>,
) -> Result<HttpResponse, AppError> {
    let rule_id = path.into_inner();
    
    // Query the git history for this rule file
    // This would require tracking file paths in commits
    
    Ok(HttpResponse::Ok().json(vec![]))  // Placeholder
}
```

---

### Frontend: Branch Manager (Wave 3)

| File | Purpose |
|------|---------|
| `frontend/src/features/versioning/BranchManager.tsx` | Branch list and management UI |
| `frontend/src/features/versioning/ConflictViewer.tsx` | Merge conflict visualization |

```typescript
// frontend/src/features/versioning/BranchManager.tsx
export function BranchManager() {
  const [branches, setBranches] = useState<Branch[]>([]);
  const [showCreateModal, setShowCreateModal] = useState(false);
  
  useEffect(() => {
    fetch('/api/v1/branches')
      .then(r => r.json())
      .then(setBranches);
  }, []);
  
  const handleMerge = async (branch: Branch) => {
    if (!window.confirm(`Merge ${branch.name} into main?`)) return;
    
    try {
      const response = await fetch(`/api/v1/branches/${branch.name}/merge`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ target: 'main' })
      });
      
      const result = await response.json();
      
      if (response.ok) {
        alert(`Merged successfully! Commit: ${result.commit_sha}`);
        setBranches(prev => prev.filter(b => b.name !== branch.name));
      } else if (response.status === 409) {
        // Show conflict viewer
        showConflictViewer(result.conflicts);
      }
    } catch (error) {
      console.error('Merge failed:', error);
    }
  };

  return (
    <div className="branch-manager">
      <h2>Branches</h2>
      
      {branches.map(branch => (
        <div key={branch.name} className={`branch ${branch.is_default ? 'default' : ''}`}>
          <span className="branch-name">{branch.name}</span>
          {!branch.is_default && (
            <button onClick={() => handleMerge(branch)}>Merge</button>
          )}
        </div>
      ))}
      
      <button onClick={() => setShowCreateModal(true)}>New Branch</button>
      
      {showCreateModal && (
        <CreateBranchModal onClose={() => setShowCreateModal(false)} />
      )}
    </div>
  );
}

interface Branch {
  name: string;
  is_default: boolean;
  rules_changed: number;
}

// frontend/src/features/versioning/ConflictViewer.tsx
export function ConflictViewer({ conflicts }: { conflicts: Conflict[] }) {
  return (
    <div className="conflict-viewer">
      <h3>Merge Conflicts ({conflicts.length})</h3>
      
      {conflicts.map((conflict, idx) => (
        <div key={idx} className="conflict">
          <h4>{conflict.path}</h4>
          
          <div className="diff-view">
            {/* Three-way diff view: base | old | new */}
            <DiffViewer
              base={conflict.base_version}
              left={conflict.old_version}
              right={conflict.new_version}
            />
          </div>
        </div>
      ))}
    </div>
  );
}

interface Conflict {
  path: string;
  old_version?: string;
  base_version?: string;
  new_version?: string;
}
```

---

### Frontend: Rule History (Wave 3)

```typescript
// frontend/src/features/versioning/RuleHistory.tsx
export function RuleHistory({ ruleId }: { ruleId: string }) {
  const [history, setHistory] = useState<Commit[]>([]);
  
  useEffect(() => {
    fetch(`/api/v1/rules/${ruleId}/history`)
      .then(r => r.json())
      .then(setHistory);
  }, [ruleId]);

  return (
    <div className="rule-history">
      <h3>Version History</h3>
      
      <ol className="commit-list">
        {history.map(commit => (
          <li key={commit.sha} className="commit-item">
            <span className="sha">{commit.sha.slice(0, 7)}</span>
            <span className="message">{commit.message}</span>
            <span className="author">{commit.author}</span>
            <span className="timestamp">{new Date(commit.timestamp).toLocaleString()}</span>
          </li>
        ))}
      </ol>
    </div>
  );
}

interface Commit {
  sha: string;
  message: string;
  author: string;
  timestamp: string;
}
```

---

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_branch_creation() {
        // Setup: Temp repository
        let temp_dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(temp_dir.path()).unwrap();
        
        // Create a file and commit to establish initial state
        std::fs::write(temp_dir.path().join("rules.drl"), "test").unwrap();
        
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("rules.drl")).unwrap();
        index.write().unwrap();
        
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        
        let signature = repo.signature().unwrap();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        ).unwrap();
        
        // Create branch
        let branches = repo.branches(Some(git2::BranchType::Local)).unwrap();
        assert!(branches.count() >= 1);
    }
    
    #[test]
    fn test_merge_conflict_detection() {
        // Setup: Two divergent branches
        // ... (similar setup as above)
        
        // Add conflicting change on branch A
        // Add conflicting change on branch B
        
        // Attempt merge - should detect conflicts
        let result = repo.merge_trees(&tree_ancestor, &tree_target, &tree_source, None);
        assert!(result.is_err() || result.unwrap().has_conflicts());
    }
}
```

### Integration Tests

| Test | Endpoint/Component | Pass Condition |
|------|-------------------|----------------|
| GIT-04-01 | POST `/api/v1/branches` (author) | Returns 201, branch created |
| GIT-04-02 | Merge branches with conflicts | Returns 409 with conflict details |
| GIT-04-03 | Merge clean branches | Returns 200 with commit SHA |

---

## Acceptance Checklist

- [ ] libgit2 system library installed
- [ ] Repository manager compiles with `cargo check --lib governance::versioning`
- [ ] Branch creation works via API
- [ ] Merge preparation detects conflicts before actual merge
- [ ] Successful merges update target branch reference
- [ ] Frontend branch list displays available branches
- [ ] Conflict viewer shows three-way diff for merge conflicts

---

## Dependencies

### Cargo.toml additions
```toml
[dependencies]
git2 = "0.21"
chrono = { version = "0.4", features = ["serde"] }
serde_json = "1.0"

# For test fixtures
tempfile = "3.8"
```

### System dependency (must be installed on target)
```bash
# Ubuntu/Debian
sudo apt-get install libgit2-1.9

# macOS (via Homebrew)
brew install libgit2

# Fedora/RHEL
sudo dnf install libgit2-devel
```

---

## Notes

- **Single repo strategy:** Start with one repository for all rules; consider multi-repo if scale exceeds 10K rules
- **Merge strategy:** Default to "create merge commit" (not rebase) for traceability
- **Conflict resolution:** Frontend must show three-way diff view before user can resolve
- **Security:** Git repository should be isolated from direct filesystem access; all operations via API
- **Future enhancement:** Support pull/merge request patterns with review workflow

---

*Plan created: 2026-06-10*
*Phase: 18.04 - Git Version Control*
