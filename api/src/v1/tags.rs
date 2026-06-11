//! Tags API endpoints - Phase 20-02

use actix_web::{web, HttpResponse, Responder};
use uuid::Uuid;

// In-memory tag and rule-tag storage (would be replaced with database in production)
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// Shared state for tags API
type TagStore = Arc<Mutex<HashMap<String, StoredTag>>>;
type RuleTagStore = Arc<Mutex<HashSet<(Uuid, String)>>>;

/// A tag with metadata
#[derive(Debug, Clone)]
pub struct StoredTag {
    pub name: String,
    pub color: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
}

/// Request to create a new tag
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Response with tag details
#[derive(Debug, Clone, serde::Serialize)]
pub struct TagResponse {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: String,
}

/// Request to assign/remove tag from a rule
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RuleTagActionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>, // For removal
}

impl From<&StoredTag> for TagResponse {
    fn from(tag: &StoredTag) -> Self {
        TagResponse {
            name: tag.name.clone(),
            color: tag.color.clone(),
            description: tag.description.clone(),
            created_at: tag.created_at.clone(),
        }
    }
}

/// Initialize in-memory stores
pub fn init_stores() -> (TagStore, RuleTagStore) {
    let tag_store: HashMap<String, StoredTag> = HashMap::new();
    let rule_tag_store: HashSet<(Uuid, String)> = HashSet::new();
    (
        Arc::new(Mutex::new(tag_store)),
        Arc::new(Mutex::new(rule_tag_store)),
    )
}

/// Create a tag - POST /v1/tags
pub async fn create_tag(
    req: web::Json<CreateTagRequest>,
    store: web::Data<TagStore>,
) -> impl Responder {
    let name = req.name.clone();
    let now = chrono::Utc::now().to_rfc3339();

    let mut store_guard = store.lock().unwrap();

    // Check for duplicate
    if store_guard.contains_key(&name) {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Tag with this name already exists"
        }));
    }

    let tag = StoredTag {
        name: name.clone(),
        color: req.color.clone(),
        description: req.description.clone(),
        created_at: now,
    };

    store_guard.insert(tag.name.clone(), tag);

    HttpResponse::Created().json(TagResponse::from(store_guard.get(&name).unwrap()))
}

/// List tags - GET /v1/tags
pub async fn list_tags(store: web::Data<TagStore>) -> impl Responder {
    let store_guard = store.lock().unwrap();
    let tags: Vec<TagResponse> = store_guard.values().map(TagResponse::from).collect();

    HttpResponse::Ok().json(serde_json::json!({
        "total_count": tags.len(),
        "tags": tags
    }))
}

/// Get tag by name - GET /v1/tags/{name}
pub async fn get_tag(
    path: web::Path<String>,
    store: web::Data<TagStore>,
) -> impl Responder {
    let name = path.into_inner();

    let store_guard = store.lock().unwrap();

    match store_guard.get(&name) {
        Some(tag) => HttpResponse::Ok().json(TagResponse::from(tag)),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Tag not found"
        })),
    }
}

/// Update a tag - PUT/PATCH /v1/tags/{name}
pub async fn update_tag(
    path: web::Path<String>,
    req: web::Json<CreateTagRequest>,
    store: web::Data<TagStore>,
) -> impl Responder {
    let old_name = path.into_inner();
    let new_name = req.name.clone();

    let mut store_guard = store.lock().unwrap();

    match store_guard.remove(&old_name) {
        Some(_) => {
            // Check for duplicate new name
            if store_guard.contains_key(&new_name) && old_name != new_name {
                let now = chrono::Utc::now().to_rfc3339();
                store_guard.insert(
                    old_name.clone(),
                    StoredTag {
                        name: old_name,
                        color: None,
                        description: None,
                        created_at: now,
                    },
                );
                return HttpResponse::Conflict().json(serde_json::json!({
                    "error": "Tag with this name already exists"
                }));
            }

            let now = chrono::Utc::now().to_rfc3339();
            let tag = StoredTag {
                name: new_name.clone(),
                color: req.color.clone(),
                description: req.description.clone(),
                created_at: now,
            };

            store_guard.insert(tag.name.clone(), tag);

            HttpResponse::Ok().json(TagResponse::from(store_guard.get(&new_name).unwrap()))
        }
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Tag not found"
        })),
    }
}

/// Delete a tag - DELETE /v1/tags/{name}
pub async fn delete_tag(
    path: web::Path<String>,
    store: web::Data<TagStore>,
) -> impl Responder {
    let name = path.into_inner();

    let mut store_guard = store.lock().unwrap();

    match store_guard.remove(&name) {
        Some(_) => HttpResponse::NoContent().finish(),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Tag not found"
        })),
    }
}

/// Assign a tag to a rule - POST /v1/rules/{id}/tags
pub async fn assign_tag_to_rule(
    path: web::Path<String>,
    req: web::Json<CreateTagRequest>,
    store: web::Data<TagStore>,
    rule_tag_store: web::Data<RuleTagStore>,
) -> impl Responder {
    let rule_id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid rule ID format"
            }));
        }
    };

    let name = req.name.clone();
    let now = chrono::Utc::now().to_rfc3339();

    // Create tag if it doesn't exist
    {
        let mut store_guard = store.lock().unwrap();
        if !store_guard.contains_key(&name) {
            let tag = StoredTag {
                name: name.clone(),
                color: req.color.clone(),
                description: req.description.clone(),
                created_at: now,
            };
            store_guard.insert(tag.name.clone(), tag);
        }
    }

    // Assign to rule
    let mut rule_tag_guard = rule_tag_store.lock().unwrap();
    rule_tag_guard.insert((rule_id, name.clone()));

    HttpResponse::Created().json(serde_json::json!({
        "rule_id": rule_id.to_string(),
        "tag_name": name,
        "action": "assigned"
    }))
}

/// Remove a tag from a rule - DELETE /v1/rules/{id}/tags/{tag_name}
pub async fn remove_tag_from_rule(
    path: web::Path<(String, String)>,
    _store: web::Data<TagStore>,
    rule_tag_store: web::Data<RuleTagStore>,
) -> impl Responder {
    let (rule_id_str, tag_name) = path.into_inner();
    let rule_id = match Uuid::parse_str(&rule_id_str) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid rule ID format"
            }));
        }
    };

    let mut rule_tag_guard = rule_tag_store.lock().unwrap();
    if rule_tag_guard.remove(&(rule_id, tag_name.clone())) {
        HttpResponse::Ok().json(serde_json::json!({
            "rule_id": rule_id.to_string(),
            "tag_name": tag_name,
            "action": "removed"
        }))
    } else {
        HttpResponse::NotFound().json(serde_json::json!({
            "error": "Tag not assigned to rule"
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_stores() {
        let (tag_store, _) = init_stores();
        assert!(tag_store.lock().unwrap().is_empty());
    }

    #[test]
    fn test_tag_response_conversion() {
        let tag = StoredTag {
            name: "bugfix".to_string(),
            color: Some("#ff0000".to_string()),
            description: Some("For bug fix rules".to_string()),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let response = TagResponse::from(&tag);

        assert_eq!(response.name, "bugfix");
        assert_eq!(response.color, Some("#ff0000".to_string()));
    }
}
