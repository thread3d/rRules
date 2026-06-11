//! Branch management API endpoints - Phase 20-05

use actix_web::{web, HttpResponse, Responder};
use uuid::Uuid;

// In-memory branch storage (would be replaced with Git integration in production)
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Shared state for branches API
type BranchStore = Arc<Mutex<HashMap<String, StoredBranch>>>;

/// Represents a Git branch in our system
#[derive(Debug, Clone)]
pub struct StoredBranch {
    pub name: String,
    pub is_default: bool,
    pub commit_sha: String,
    pub last_commit_message: String,
    pub created_at: String,
}

/// Request to create a new branch
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateBranchRequest {
    pub source_branch: Option<String>,
    pub from_commit: Option<String>,
}

/// Response with branch details
#[derive(Debug, Clone, serde::Serialize)]
pub struct BranchResponse {
    pub name: String,
    pub is_default: bool,
    pub commit_sha: String,
    pub last_commit_message: String,
    pub created_at: String,
}

/// Paginated list of branches
#[derive(Debug, Clone, serde::Serialize)]
pub struct BranchListResponse {
    pub total_count: usize,
    pub default_branch: Option<String>,
    pub branches: Vec<BranchResponse>,
}

impl From<&StoredBranch> for BranchResponse {
    fn from(branch: &StoredBranch) -> Self {
        BranchResponse {
            name: branch.name.clone(),
            is_default: branch.is_default,
            commit_sha: branch.commit_sha.clone(),
            last_commit_message: branch.last_commit_message.clone(),
            created_at: branch.created_at.clone(),
        }
    }
}

/// Initialize in-memory branch store with default branch
pub fn init_store() -> BranchStore {
    let mut store: HashMap<String, StoredBranch> = HashMap::new();

    // Create default branch
    let now = chrono::Utc::now().to_rfc3339();
    store.insert(
        "main".to_string(),
        StoredBranch {
            name: "main".to_string(),
            is_default: true,
            commit_sha: "initial-commit-sha".to_string(),
            last_commit_message: "Initial commit".to_string(),
            created_at: now,
        },
    );

    Arc::new(Mutex::new(store))
}

/// Create a branch - POST /v1/branches
pub async fn create_branch(
    req: web::Json<CreateBranchRequest>,
    store: web::Data<BranchStore>,
) -> impl Responder {
    let source_branch = req.source_branch.clone().unwrap_or_else(|| "main".to_string());

    // In a real implementation, this would check if the source branch exists
    // and use its HEAD commit as the from_commit

    let now = chrono::Utc::now().to_rfc3339();
    let branch_name = format!("feature/{}", Uuid::new_v4().to_string()[..8].to_lowercase());

    let branch = StoredBranch {
        name: branch_name.clone(),
        is_default: false,
        commit_sha: source_branch.clone(), // Would be actual commit SHA in production
        last_commit_message: "Branch created".to_string(),
        created_at: now,
    };

    let mut store_guard = store.lock().unwrap();

    if store_guard.contains_key(&branch.name) {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Branch with this name already exists"
        }));
    }

    let branch_name = branch.name.clone();
    store_guard.insert(branch_name.clone(), branch);

    HttpResponse::Created().json(BranchResponse::from(
        store_guard.get(&branch_name).unwrap(),
    ))
}

/// List branches - GET /v1/branches
pub async fn list_branches(store: web::Data<BranchStore>) -> impl Responder {
    let store_guard = store.lock().unwrap();

    let default_branch = store_guard
        .values()
        .find(|b| b.is_default)
        .map(|b| b.name.clone());

    let branches: Vec<BranchResponse> = store_guard.values().map(BranchResponse::from).collect();

    HttpResponse::Ok().json(BranchListResponse {
        total_count: branches.len(),
        default_branch,
        branches,
    })
}

/// Get branch by name - GET /v1/branches/{name}
pub async fn get_branch(
    path: web::Path<String>,
    store: web::Data<BranchStore>,
) -> impl Responder {
    let name = path.into_inner();

    let store_guard = store.lock().unwrap();

    match store_guard.get(&name) {
        Some(branch) => HttpResponse::Ok().json(BranchResponse::from(branch)),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Branch not found"
        })),
    }
}

/// Delete a branch - DELETE /v1/branches/{name}
pub async fn delete_branch(
    path: web::Path<String>,
    store: web::Data<BranchStore>,
) -> impl Responder {
    let name = path.into_inner();

    let mut store_guard = store.lock().unwrap();

    // Prevent deleting the default branch
    if store_guard
        .values()
        .any(|b| b.name == name && b.is_default)
    {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Cannot delete the default branch"
        }));
    }

    match store_guard.remove(&name) {
        Some(_) => HttpResponse::NoContent().finish(),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Branch not found"
        })),
    }
}

/// Merge a branch - POST /v1/branches/{name}/merge
pub async fn merge_branch(
    path: web::Path<String>,
    store: web::Data<BranchStore>,
) -> impl Responder {
    let source_name = path.into_inner();

    // In production, this would:
    // 1. Check if source branch exists
    // 2. Attempt to merge with target (typically main)
    // 3. Detect conflicts and report them

    let store_guard = store.lock().unwrap();

    // Find the source branch
    let source_branch = match store_guard.get(&source_name) {
        Some(b) => b,
        None => return HttpResponse::NotFound().json(serde_json::json!({
            "error": "Source branch not found"
        })),
    };

    // Simulate merge (would update commit SHA in production)
    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "source_branch": source_name,
        "commit_sha": source_branch.commit_sha,
        "conflicts_detected": false
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_store() {
        let store = init_store();
        let guard = store.lock().unwrap();

        assert_eq!(guard.len(), 1);
        assert!(guard.contains_key("main"));
        assert!(guard.get("main").unwrap().is_default);
    }

    #[test]
    fn test_branch_response_conversion() {
        let branch = StoredBranch {
            name: "feature/test".to_string(),
            is_default: false,
            commit_sha: "abc123".to_string(),
            last_commit_message: "Test commit".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let response = BranchResponse::from(&branch);

        assert_eq!(response.name, "feature/test");
        assert!(!response.is_default);
    }
}
