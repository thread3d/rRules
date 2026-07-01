//! Categories API endpoints - Phase 20-02

use actix_web::{web, HttpResponse, Responder};
use uuid::Uuid;

// In-memory category storage (would be replaced with database in production)
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Shared state for categories API
type CategoryStore = Arc<Mutex<HashMap<Uuid, StoredCategory>>>;

/// A category with optional parent for hierarchy support
#[derive(Debug, Clone)]
pub struct StoredCategory {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub created_at: String,
}

/// Request to create a new category
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct CreateCategoryRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
}

/// Response with category details
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct CategoryResponse {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub created_at: String,
}

/// Paginated list of categories
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct CategoryListResponse {
    pub total_count: usize,
    pub categories: Vec<CategoryResponse>,
}

impl From<&StoredCategory> for CategoryResponse {
    fn from(category: &StoredCategory) -> Self {
        CategoryResponse {
            id: category.id.to_string(),
            name: category.name.clone(),
            description: category.description.clone(),
            parent_id: category.parent_id.map(|id| id.to_string()),
            created_at: category.created_at.clone(),
        }
    }
}

/// Initialize in-memory category store
pub fn init_store() -> CategoryStore {
    let store: HashMap<Uuid, StoredCategory> = HashMap::new();
    Arc::new(Mutex::new(store))
}

/// Create a category - POST /v1/categories
#[utoipa::path(
    post,
    path = "/v1/categories",
    tag = "categories",
    request_body = CreateCategoryRequest,
    responses(
        (status = 201, description = "Category created", body = CategoryResponse),
        (status = 409, description = "Category with this name already exists"),
    ),
)]
pub async fn create_category(
    req: web::Json<CreateCategoryRequest>,
    store: web::Data<CategoryStore>,
) -> impl Responder {
    let id = Uuid::new_v4();
    let category = StoredCategory {
        id,
        name: req.name.clone(),
        description: req.description.clone(),
        parent_id: req.parent_id,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut store_guard = store.lock().unwrap();

    // Check for duplicate name (case-insensitive)
    if store_guard.values().any(|c| c.name.to_lowercase() == req.name.to_lowercase()) {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Category with this name already exists"
        }));
    }

    store_guard.insert(id, category);

    HttpResponse::Created().json(CategoryResponse::from(
        store_guard.get(&id).unwrap(),
    ))
}

/// List categories (with hierarchy) - GET /v1/categories
#[utoipa::path(
    get,
    path = "/v1/categories",
    tag = "categories",
    responses(
        (status = 200, description = "List of categories", body = CategoryListResponse),
    ),
)]
pub async fn list_categories(store: web::Data<CategoryStore>) -> impl Responder {
    let store_guard = store.lock().unwrap();
    let categories: Vec<CategoryResponse> = store_guard.values().map(CategoryResponse::from).collect();

    HttpResponse::Ok().json(CategoryListResponse {
        total_count: categories.len(),
        categories,
    })
}

/// Get category by ID - GET /v1/categories/{id}
#[utoipa::path(
    get,
    path = "/v1/categories/{id}",
    tag = "categories",
    responses(
        (status = 200, description = "Category found", body = CategoryResponse),
        (status = 400, description = "Invalid category ID format"),
        (status = 404, description = "Category not found"),
    ),
    params(
        ("id" = String, Path, description = "Category ID (UUID)"),
    ),
)]
pub async fn get_category(
    path: web::Path<String>,
    store: web::Data<CategoryStore>,
) -> impl Responder {
    let id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid category ID format"
            }));
        }
    };

    let store_guard = store.lock().unwrap();

    match store_guard.get(&id) {
        Some(category) => HttpResponse::Ok().json(CategoryResponse::from(category)),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Category not found"
        })),
    }
}

/// Update a category - PUT/PATCH /v1/categories/{id}
#[utoipa::path(
    put,
    path = "/v1/categories/{id}",
    tag = "categories",
    request_body = CreateCategoryRequest,
    responses(
        (status = 200, description = "Category updated", body = CategoryResponse),
        (status = 400, description = "Invalid category ID format"),
        (status = 404, description = "Category not found"),
        (status = 409, description = "Category with this name already exists"),
    ),
    params(
        ("id" = String, Path, description = "Category ID (UUID)"),
    ),
)]
pub async fn update_category(
    path: web::Path<String>,
    req: web::Json<CreateCategoryRequest>,
    store: web::Data<CategoryStore>,
) -> impl Responder {
    let id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid category ID format"
            }));
        }
    };

    let mut store_guard = store.lock().unwrap();

    // Check for duplicate name before any mutations
    let has_duplicate = store_guard
        .values()
        .any(|c| c.id != id && c.name.to_lowercase() == req.name.to_lowercase());

    if has_duplicate {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Category with this name already exists"
        }));
    }

    match store_guard.get_mut(&id) {
        Some(category) => {
            let now = chrono::Utc::now().to_rfc3339();
            category.name = req.name.clone();
            category.description = req.description.clone();
            category.parent_id = req.parent_id;
            category.created_at = now;

            HttpResponse::Ok().json(CategoryResponse::from(&*category))
        }
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Category not found"
        })),
    }
}

/// Delete a category - DELETE /v1/categories/{id}
#[utoipa::path(
    delete,
    path = "/v1/categories/{id}",
    tag = "categories",
    responses(
        (status = 204, description = "Category deleted"),
        (status = 400, description = "Invalid category ID format"),
        (status = 404, description = "Category not found"),
        (status = 409, description = "Cannot delete category with child categories"),
    ),
    params(
        ("id" = String, Path, description = "Category ID (UUID)"),
    ),
)]
pub async fn delete_category(
    path: web::Path<String>,
    store: web::Data<CategoryStore>,
) -> impl Responder {
    let id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid category ID format"
            }));
        }
    };

    let mut store_guard = store.lock().unwrap();

    // Check if any subcategories reference this as parent
    if store_guard.values().any(|c| c.parent_id == Some(id)) {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Cannot delete category with child categories"
        }));
    }

    match store_guard.remove(&id) {
        Some(_) => HttpResponse::NoContent().finish(),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Category not found"
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_store() {
        let store = init_store();
        assert!(store.lock().unwrap().is_empty());
    }

    #[test]
    fn test_category_response_conversion() {
        let category = StoredCategory {
            id: Uuid::new_v4(),
            name: "Test Category".to_string(),
            description: Some("A test category".to_string()),
            parent_id: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let response = CategoryResponse::from(&category);

        assert_eq!(response.name, "Test Category");
        assert_eq!(response.description, Some("A test category".to_string()));
    }
}
