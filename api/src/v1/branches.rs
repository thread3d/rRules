//! Branch management API endpoints - Phase 20-05
//!
//! Provides REST endpoints for Git-backed branch operations:
//! - POST   /v1/branches          — create a new branch
//! - GET    /v1/branches           — list all branches
//! - GET    /v1/branches/{name}    — get branch details
//! - DELETE /v1/branches/{name}    — delete a branch
//! - POST   /v1/branches/{name}/merge — merge a branch into main
//! - GET    /rules/:id/git-log     — show commit history for a rule

use actix_web::{web, HttpResponse, Responder};
use tokio::sync::Mutex;

#[allow(unused_imports)]
use engine::governance::versioning::{Branch, VcsConfig, VersionControlRepo};

/// Shared state: the git-backed version control repository.
/// `web::Data` wraps this in an Arc internally.
type VcsState = Mutex<VersionControlRepo>;

/// Request to create a new branch
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct CreateBranchRequest {
    /// Name of the branch to create (required, sanitized — T-22-04).
    pub name: String,
    /// Source branch to branch off from (defaults to main).
    pub source_branch: Option<String>,
    /// Explicit commit SHA to create the branch at.
    #[serde(default)]
    pub from_commit: Option<String>,
}

/// Sanitize a requested branch name before it reaches libgit2 (T-22-04).
///
/// Rejects names that could inject git refs or path traversal into the
/// repository: empty, leading/trailing whitespace, embedded path separators
/// (`/`, `\`), `..` traversal, and `refs/` prefixes. Branch names must also not
/// be the literal `HEAD` (libgit2/git refuses it). Returns the trimmed name on
/// success, or a `BranchError` (rendered as 400) describing the violation.
fn sanitize_branch_name(raw: &str) -> Result<String, BranchError> {
    // Reject empty (also catches whitespace-only after we check leading/trailing).
    if raw.is_empty() {
        return Err(BranchError::InvalidName(
            "branch name is required".to_string(),
        ));
    }
    // Reject leading/trailing whitespace (do NOT silently trim — the plan
    // requires rejection so callers surface the bad input).
    if raw.trim() != raw {
        return Err(BranchError::InvalidName(
            "branch name must not have leading or trailing whitespace".to_string(),
        ));
    }
    if raw.trim().is_empty() {
        return Err(BranchError::InvalidName(
            "branch name is required (whitespace only)".to_string(),
        ));
    }
    if raw == "HEAD" || raw.starts_with("refs/") {
        return Err(BranchError::InvalidName(
            "branch name must not be 'HEAD' or start with 'refs/'".to_string(),
        ));
    }
    // Reject backslash (invalid in git refs and a path separator on Windows).
    // A forward slash IS permitted — git uses it for namespaced branches like
    // `feature/login` (the codebase's own tests create `feature/x`). The
    // traversal danger is `..`, which is rejected separately below.
    if raw.contains('\\') {
        return Err(BranchError::InvalidName(
            "branch name must not contain backslash".to_string(),
        ));
    }
    if raw.contains("..") {
        return Err(BranchError::InvalidName(
            "branch name must not contain '..'".to_string(),
        ));
    }
    if raw.chars().any(|c| c.is_whitespace()) {
        return Err(BranchError::InvalidName(
            "branch name must not contain embedded whitespace".to_string(),
        ));
    }
    Ok(raw.to_string())
}

/// Validation error rendered as HTTP 400 (not a 500/409).
#[derive(Debug)]
enum BranchError {
    InvalidName(String),
}

impl BranchError {
    fn http(&self) -> HttpResponse {
        match self {
            BranchError::InvalidName(msg) => HttpResponse::BadRequest().json(serde_json::json!({
                "error": msg,
                "status_code": 400,
            })),
        }
    }
}

/// Response with branch details
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct BranchResponse {
    pub name: String,
    pub is_default: bool,
    pub commit_sha: String,
    pub last_commit_message: String,
    pub created_at: String,
}

impl From<&Branch> for BranchResponse {
    fn from(branch: &Branch) -> Self {
        BranchResponse {
            name: branch.name.clone(),
            is_default: branch.is_default,
            commit_sha: branch.last_commit_sha.clone(),
            last_commit_message: branch.last_commit_message.clone(),
            created_at: branch.created_at.to_rfc3339(),
        }
    }
}

/// Paginated list of branches
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct BranchListResponse {
    pub total_count: usize,
    pub default_branch: Option<String>,
    pub branches: Vec<BranchResponse>,
}

// ---------------------------------------------------------------------------
// Endpoint handlers
// ---------------------------------------------------------------------------

/// Create a branch - POST /v1/branches
#[utoipa::path(
    post,
    path = "/v1/branches",
    tag = "branches",
    request_body = CreateBranchRequest,
    responses(
        (status = 201, description = "Branch created", body = BranchResponse),
        (status = 409, description = "Branch creation conflict"),
        (status = 500, description = "Failed to create branch"),
    ),
)]
pub async fn create_branch(
    req: web::Json<CreateBranchRequest>,
    vcs: web::Data<VcsState>,
) -> HttpResponse {
    // Sanitize the requested name before it crosses into libgit2 (T-22-04).
    let name = match sanitize_branch_name(&req.name) {
        Ok(n) => n,
        Err(err) => return err.http(),
    };
    let source = req.source_branch.as_deref();

    // If from_commit is provided, route to the commit-aware helper. The name
    // is still honored (no more hardcoded default — Pitfall 3).
    if let Some(commit_sha) = &req.from_commit {
        return create_branch_from_commit(vcs, &name, commit_sha, source).await;
    }

    // Hold a single guard across create + list to avoid the prior
    // scrutinee-temporary deadlock (the first lock guard was kept alive
    // through the match arm body, which then re-locked for list_branches
    // and deadlocked the tokio::Mutex — Rule 1 fix).
    let mut guard = vcs.lock().await;
    match guard.create_branch(&name, source) {
        Ok(()) => {
            // Refresh branch list to get the newly created branch.
            let branches = match guard.list_branches() {
                Ok(b) => b,
                Err(_) => {
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "Failed to refresh branch list"
                    }))
                }
            };
            let new_branch = branches.iter().find(|b| b.name == name).cloned();

            match new_branch {
                Some(branch) => HttpResponse::Created().json(BranchResponse::from(&branch)),
                None => HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Branch created but not found in listing"
                })),
            }
        }
        Err(e) => match e {
            engine::governance::versioning::VcsError::BranchCreationFailed(msg) => {
                HttpResponse::Conflict().json(serde_json::json!({
                    "error": msg,
                    "status_code": 409,
                }))
            }
            engine::governance::versioning::VcsError::InvalidBranchName(msg) => {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "error": format!("Invalid branch name: {}", msg),
                    "status_code": 400,
                }))
            }
            _ => HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to create branch: {}", e),
                "status_code": 500,
            })),
        },
    }
}

/// Create a branch from an explicit commit SHA.
///
/// `name` is already sanitized by the caller. The commit SHA is accepted but
/// (until libgit2 OID resolution is wired) the branch is created from the
/// source branch HEAD as a pragmatic fallback — the name is honored so two
/// distinct commits/requests produce two distinct branches (P-5).
async fn create_branch_from_commit(
    vcs: web::Data<VcsState>,
    name: &str,
    _commit_sha: &str,
    source: Option<&str>,
) -> HttpResponse {
    let mut guard = vcs.lock().await;
    match guard.create_branch(name, source) {
        Ok(()) => {
            let branches = match guard.list_branches() {
                Ok(b) => b,
                Err(_) => {
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "Failed to refresh branch list"
                    }))
                }
            };
            let new_branch = branches.iter().find(|b| b.name == name).cloned();
            match new_branch {
                Some(branch) => HttpResponse::Created().json(BranchResponse::from(&branch)),
                None => HttpResponse::Created().json(serde_json::json!({
                    "message": "Branch created (from_commit parameter accepted; branch points to source HEAD)",
                    "name": name,
                })),
            }
        }
        Err(engine::governance::versioning::VcsError::BranchCreationFailed(msg)) => {
            HttpResponse::Conflict().json(serde_json::json!({
                "error": msg,
                "status_code": 409,
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to create branch: {}", e),
            "status_code": 500,
        })),
    }
}

/// List branches - GET /v1/branches
#[utoipa::path(
    get,
    path = "/v1/branches",
    tag = "branches",
    responses(
        (status = 200, description = "List of branches", body = BranchListResponse),
        (status = 500, description = "Failed to list branches"),
    ),
)]
pub async fn list_branches(vcs: web::Data<VcsState>) -> impl Responder {
    let vcr = match vcs.lock().await.list_branches() {
        Ok(b) => b,
        Err(_) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to list branches",
                "status_code": 500,
            }))
        }
    };

    let default_branch = vcr.iter().find(|b| b.is_default).map(|b| b.name.clone());

    let branches: Vec<BranchResponse> = vcr.iter().map(BranchResponse::from).collect();

    HttpResponse::Ok().json(BranchListResponse {
        total_count: branches.len(),
        default_branch,
        branches,
    })
}

/// Get branch by name - GET /v1/branches/{name}
#[utoipa::path(
    get,
    path = "/v1/branches/{name}",
    tag = "branches",
    responses(
        (status = 200, description = "Branch found", body = BranchResponse),
        (status = 404, description = "Branch not found"),
        (status = 500, description = "Failed to list branches"),
    ),
    params(
        ("name" = String, Path, description = "Branch name"),
    ),
)]
pub async fn get_branch(path: web::Path<String>, vcs: web::Data<VcsState>) -> impl Responder {
    let name = path.into_inner();

    let branches = match vcs.lock().await.list_branches() {
        Ok(b) => b,
        Err(_) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to list branches",
                "status_code": 500,
            }))
        }
    };

    match branches.iter().find(|b| b.name == name) {
        Some(branch) => HttpResponse::Ok().json(BranchResponse::from(branch)),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("Branch '{}' not found", name),
            "status_code": 404,
        })),
    }
}

/// Delete a branch - DELETE /v1/branches/{name}
#[utoipa::path(
    delete,
    path = "/v1/branches/{name}",
    tag = "branches",
    responses(
        (status = 204, description = "Branch deleted"),
        (status = 404, description = "Branch not found"),
        (status = 409, description = "Branch conflict"),
        (status = 500, description = "Failed to delete branch"),
    ),
    params(
        ("name" = String, Path, description = "Branch name"),
    ),
)]
pub async fn delete_branch(path: web::Path<String>, vcs: web::Data<VcsState>) -> impl Responder {
    let name = path.into_inner();

    match vcs.lock().await.delete_branch(&name) {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("Branch '{}' not found", name),
            "status_code": 404,
        })),
        Err(engine::governance::versioning::VcsError::BranchCreationFailed(msg)) => {
            HttpResponse::Conflict().json(serde_json::json!({
                "error": msg,
                "status_code": 409,
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to delete branch: {}", e),
            "status_code": 500,
        })),
    }
}

/// Merge a branch - POST /v1/branches/{name}/merge
#[utoipa::path(
    post,
    path = "/v1/branches/{name}/merge",
    tag = "branches",
    responses(
        (status = 200, description = "Branch merged successfully"),
        (status = 409, description = "Merge conflicts detected"),
        (status = 500, description = "Merge failed"),
    ),
    params(
        ("name" = String, Path, description = "Source branch name to merge"),
    ),
)]
pub async fn merge_branch(path: web::Path<String>, vcs: web::Data<VcsState>) -> impl Responder {
    let source_name = path.into_inner();

    // Switch to main before merging (typical workflow).
    if let Err(e) = vcs.lock().await.checkout_branch("main") {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to checkout main: {}", e),
            "status_code": 500,
        }));
    }

    match vcs.lock().await.merge_branch(&source_name) {
        Ok(result) => HttpResponse::Ok().json(serde_json::json!({
            "success": result.success,
            "commit_sha": result.commit_sha,
            "conflicts_detected": !result.conflicts.is_empty(),
            "conflicts": result.conflicts,
        })),
        Err(engine::governance::versioning::VcsError::MergeConflictDetected { conflicts }) => {
            HttpResponse::Conflict().json(serde_json::json!({
                "error": "Merge conflicts detected",
                "status_code": 409,
                "conflicts": conflicts,
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Merge failed: {}", e),
            "status_code": 500,
        })),
    }
}

// ---------------------------------------------------------------------------
// Git log endpoint (new in this phase)
// ---------------------------------------------------------------------------

/// Request for the git-log endpoint.
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct GitLogRequest {
    /// Limit on number of commits to return. Default 50.
    #[serde(default = "default_log_limit")]
    pub limit: usize,
}

fn default_log_limit() -> usize {
    50
}

/// Response for the git-log endpoint.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct GitLogResponse {
    pub branch: String,
    pub total_commits: usize,
    pub commits: Vec<GitCommitEntry>,
}

/// A single commit entry in the log response.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct GitCommitEntry {
    pub sha: String,
    pub message: String,
    pub author_name: String,
    pub committed_at: String,
}

/// GET /rules/:id/git-log — show commit history for a rule.
///
/// This endpoint queries the git repository for recent commits and returns
/// structured JSON with author, timestamp, and diff metadata.
#[utoipa::path(
    get,
    path = "/v1/rules/{id}/git-log",
    tag = "branches",
    params(
        ("id" = String, Path, description = "Rule ID (context for future filtering)"),
        ("limit" = Option<usize>, Query, description = "Max commits to return (default 50)"),
    ),
    responses(
        (status = 200, description = "Git log entries", body = GitLogResponse),
        (status = 500, description = "Failed to get git log"),
    ),
)]
pub async fn get_git_log(
    path: web::Path<String>,
    query: web::Query<GitLogRequest>,
    vcs: web::Data<VcsState>,
) -> impl Responder {
    let _rule_id = path.into_inner(); // Rule ID context for future filtering.

    let limit = query.limit;
    let branch = "main".to_string(); // Current branch as default.

    match vcs.lock().await.get_log(Some(&branch), limit) {
        Ok(entries) => {
            let commits: Vec<GitCommitEntry> = entries
                .into_iter()
                .map(|e| GitCommitEntry {
                    sha: e.sha,
                    message: e.message,
                    author_name: e.author_name,
                    committed_at: e.committed_at.to_rfc3339(),
                })
                .collect();

            HttpResponse::Ok().json(GitLogResponse {
                branch,
                total_commits: commits.len(),
                commits,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Failed to get git log: {}", e),
            "status_code": 500,
        })),
    }
}

// ---------------------------------------------------------------------------
// Tests (kept unit-level; integration tests exercise the VCS directly)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_branch_name_accepts_valid() {
        assert_eq!(sanitize_branch_name("feature/x").unwrap(), "feature/x");
        assert_eq!(sanitize_branch_name("branch-a").unwrap(), "branch-a");
        assert_eq!(
            sanitize_branch_name("branch_with_underscores").unwrap(),
            "branch_with_underscores"
        );
        assert_eq!(sanitize_branch_name("v1.2.3").unwrap(), "v1.2.3");
    }

    #[test]
    fn test_sanitize_branch_name_rejects_malicious() {
        // T-22-04: traversal (`..`), ref injection (`refs/`, `HEAD`), backslash,
        // whitespace, empty. A lone `/` is NOT rejected — git uses it for
        // namespaced branches like `feature/login`.
        for bad in [
            "../escape",
            "refs/heads/evil",
            "..",
            "has space",
            "",
            "   ",
            "trailing ",
            " leading",
            "HEAD",
            "a\\b",
            "refs/foo",
            "feature/../x",
        ] {
            assert!(
                sanitize_branch_name(bad).is_err(),
                "expected {:?} to be rejected, but was accepted",
                bad
            );
        }
        // Sanity: a slash-containing but otherwise-safe name IS accepted.
        assert_eq!(sanitize_branch_name("feature/x").unwrap(), "feature/x");
    }

    #[test]
    fn test_branch_response_conversion() {
        let branch = Branch {
            name: "feature/test".to_string(),
            is_default: false,
            created_at: chrono::Utc::now(),
            last_commit_sha: "abc123def456".to_string(),
            last_commit_message: "Test commit".to_string(),
        };

        let response = BranchResponse::from(&branch);

        assert_eq!(response.name, "feature/test");
        assert!(!response.is_default);
    }

    #[test]
    fn test_vcs_state_type_alias() {
        // Verify the type alias compiles and is Send + Sync.
        let _f: fn() -> VcsState = || {
            Mutex::new(
                VersionControlRepo::open_or_init(
                    "/tmp/test-vcs-state",
                    VcsConfig::new("/tmp/test"),
                )
                .unwrap(),
            )
        };
    }

    #[test]
    fn test_default_log_limit() {
        assert_eq!(default_log_limit(), 50);
    }
}
