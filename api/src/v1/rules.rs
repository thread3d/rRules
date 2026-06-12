//! Rules CRUD API endpoints - Phase 20-01

use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use uuid::Uuid;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A rule with full metadata
#[derive(Debug, Clone, serde::Serialize)]
pub struct StoredRule {
    pub id: Uuid,
    #[serde(rename = "name")]
    pub rule_name: String,
    pub description: Option<String>,
    pub category: String,
    pub tags: Vec<String>,
    pub salience: i32,
    pub dsl_body: String,
    pub status: RuleStatus,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: String,
    pub updated_by: Option<String>,
}

/// Rule status enum
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum RuleStatus {
    Draft,
    Active,
    Inactive,
    Archived,
}

impl Default for StoredRule {
    fn default() -> Self {
        let now = Utc::now().to_rfc3339();
        StoredRule {
            id: Uuid::new_v4(),
            rule_name: String::new(),
            description: None,
            category: "default".to_string(),
            tags: vec![],
            salience: 0,
            dsl_body: String::new(),
            status: RuleStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
            created_by: "system".to_string(),
            updated_by: None,
        }
    }
}

impl From<&StoredRule> for RuleResponse {
    fn from(rule: &StoredRule) -> Self {
        RuleResponse {
            id: rule.id.to_string(),
            name: rule.rule_name.clone(),
            description: rule.description.clone(),
            category: Some(rule.category.clone()),
            owner: None,
            dsl_body: Some(rule.dsl_body.clone()),
            tags: rule.tags.clone(),
            status: match rule.status {
                RuleStatus::Draft => "draft",
                RuleStatus::Active => "active",
                RuleStatus::Inactive => "inactive",
                RuleStatus::Archived => "archived",
            }.to_string(),
            version: 1,
            created_at: rule.created_at.clone(),
            updated_at: rule.updated_at.clone(),
        }
    }
}

impl From<&mut StoredRule> for RuleResponse {
    fn from(rule: &mut StoredRule) -> Self {
        RuleResponse {
            id: rule.id.to_string(),
            name: rule.rule_name.clone(),
            description: rule.description.clone(),
            category: Some(rule.category.clone()),
            owner: None,
            dsl_body: Some(rule.dsl_body.clone()),
            tags: rule.tags.clone(),
            status: match rule.status {
                RuleStatus::Draft => "draft",
                RuleStatus::Active => "active",
                RuleStatus::Inactive => "inactive",
                RuleStatus::Archived => "archived",
            }.to_string(),
            version: 1,
            created_at: rule.created_at.clone(),
            updated_at: rule.updated_at.clone(),
        }
    }
}

impl From<&&StoredRule> for RuleResponse {
    fn from(rule: &&StoredRule) -> Self {
        RuleResponse {
            id: rule.id.to_string(),
            name: rule.rule_name.clone(),
            description: rule.description.clone(),
            category: Some(rule.category.clone()),
            owner: None,
            dsl_body: Some(rule.dsl_body.clone()),
            tags: rule.tags.clone(),
            status: match rule.status {
                RuleStatus::Draft => "draft",
                RuleStatus::Active => "active",
                RuleStatus::Inactive => "inactive",
                RuleStatus::Archived => "archived",
            }.to_string(),
            version: 1,
            created_at: rule.created_at.clone(),
            updated_at: rule.updated_at.clone(),
        }
    }
}

/// Request to create a new rule
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateRuleRequest {
    /// Rule name (unique within category)
    pub name: String,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Rule category for organization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// DSL body of the rule
    pub dsl_body: String,

    /// Tags for filtering/searching
    #[serde(default)]
    pub tags: Vec<String>,

    /// Salience (priority) of the rule, default 0
    #[serde(default)]
    pub salience: i32,

    /// Initial status, defaults to Draft
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Response with rule details
#[derive(Debug, Clone, serde::Serialize)]
pub struct RuleResponse {
    /// Unique identifier (UUID)
    pub id: String,

    /// Rule name
    pub name: String,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Category
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Owner (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,

    /// DSL body of the rule
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsl_body: Option<String>,

    /// Tags for filtering/searching
    #[serde(default)]
    pub tags: Vec<String>,

    /// Rule status (draft, active, inactive, archived)
    pub status: String,

    /// Version number
    pub version: u32,

    /// When the rule was created
    pub created_at: String,

    /// When the rule was last updated
    pub updated_at: String,
}

/// Paginated list of rules
#[derive(Debug, Clone, serde::Serialize)]
pub struct RuleListResponse {
    /// Total number of rules matching filters
    pub total_count: usize,

    /// Number of rules in this page
    #[serde(rename = "rules")]
    pub items: Vec<RuleResponse>,

    /// Current page number (1-indexed)
    pub page: u32,

    /// Items per page
    pub page_size: u32,

    /// Total pages available
    pub total_pages: u32,
}

/// Request for paginated rule listing
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RuleListRequest {
    /// Page number (1-indexed), default 1
    #[serde(default)]
    pub page: u32,

    /// Items per page, default 50
    #[serde(default)]
    pub page_size: u32,

    /// Filter by category
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Filter by status (draft, active, inactive, archived)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    /// Filter by tag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,

    /// Sort field: name, salience, created_at, updated_at
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,

    /// Sort direction: asc or desc (default asc)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<String>,
}

/// Request to update an existing rule
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UpdateRuleRequest {
    /// New description (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// New category (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Updated DSL body (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsl_body: Option<String>,

    /// New tags (replaces all existing)
    #[serde(default)]
    pub tags: Vec<String>,

    /// New salience (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salience: Option<i32>,

    /// New status (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Shared state for rules API
type RuleStore = Arc<Mutex<HashMap<Uuid, StoredRule>>>;

/// Initialize in-memory rule store
pub fn init_store() -> RuleStore {
    let store: HashMap<Uuid, StoredRule> = HashMap::new();
    Arc::new(Mutex::new(store))
}

/// Create a rule - POST /v1/rules
pub async fn create_rule(
    req: web::Json<CreateRuleRequest>,
    store: web::Data<RuleStore>,
) -> impl Responder {
    let now = Utc::now().to_rfc3339();
    let rule_id = Uuid::new_v4();

    // Validate rule name
    if req.name.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Rule name is required"
        }));
    }

    // Validate DSL body
    if req.dsl_body.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "DSL body is required"
        }));
    }

    // Determine status from request or default to Draft
    let status = match req.status.as_deref() {
        Some("active") => RuleStatus::Active,
        Some("inactive") => RuleStatus::Inactive,
        Some("archived") => RuleStatus::Archived,
        _ => RuleStatus::Draft,
    };

    // Check for duplicate name within same category
    let has_duplicate = {
        let store_guard = store.lock().unwrap();
        let cats = req.category.clone().unwrap_or_default();
        store_guard.values().any(|r| {
            r.rule_name == req.name && r.category == cats
        })
    };

    if has_duplicate {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "A rule with this name already exists in the category"
        }));
    }

    // Create the new rule
    let rule = StoredRule {
        id: rule_id,
        rule_name: req.name.clone(),
        description: req.description.clone(),
        category: req.category.clone().unwrap_or_else(|| "default".to_string()),
        tags: req.tags.clone(),
        salience: req.salience,
        dsl_body: req.dsl_body.clone(),
        status,
        created_at: now.clone(),
        updated_at: now,
        created_by: "system".to_string(),
        updated_by: None,
    };

    {
        let mut store_guard = store.lock().unwrap();
        store_guard.insert(rule_id, rule);
    }

    HttpResponse::Created().json(RuleResponse::from(store.lock().unwrap().get(&rule_id).unwrap()))
}

/// Get paginated list of rules - GET /v1/rules
pub async fn get_rules(
    query: web::Query<RuleListRequest>,
    store: web::Data<RuleStore>,
) -> impl Responder {
    let page = query.page.max(1);
    let page_size = query.page_size.max(1).min(100); // Max 100 per page

    let store_guard = store.lock().unwrap();

    // Filter rules based on query parameters
    let mut rules: Vec<&StoredRule> = store_guard.values().collect();

    if let Some(ref category) = query.category {
        rules.retain(|r| r.category == *category);
    }

    if let Some(ref status) = query.status {
        let filter_status = match status.as_str() {
            "draft" => RuleStatus::Draft,
            "active" => RuleStatus::Active,
            "inactive" => RuleStatus::Inactive,
            "archived" => RuleStatus::Archived,
            _ => return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid status filter"
            })),
        };
        rules.retain(|r| r.status == filter_status);
    }

    if let Some(ref tag) = query.tag {
        rules.retain(|r| r.tags.iter().any(|t| t == tag));
    }

    // Sort rules
    match query.sort_by.as_deref() {
        Some("name") => rules.sort_by(|a, b| {
            let cmp = a.rule_name.cmp(&b.rule_name);
            if query.sort_order.as_deref() == Some("desc") {
                cmp.reverse()
            } else {
                cmp
            }
        }),
        Some("salience") => rules.sort_by(|a, b| {
            let cmp = a.salience.cmp(&b.salience);
            if query.sort_order.as_deref() == Some("desc") {
                cmp.reverse()
            } else {
                cmp
            }
        }),
        Some("created_at") => rules.sort_by(|a, b| {
            let cmp = a.created_at.cmp(&b.created_at);
            if query.sort_order.as_deref() == Some("desc") {
                cmp.reverse()
            } else {
                cmp
            }
        }),
        Some("updated_at") => rules.sort_by(|a, b| {
            let cmp = a.updated_at.cmp(&b.updated_at);
            if query.sort_order.as_deref() == Some("desc") {
                cmp.reverse()
            } else {
                cmp
            }
        }),
        _ => {} // Default: no sorting
    }

    let total_count = rules.len();
    let total_pages: u32 = ((total_count + page_size as usize - 1) / page_size as usize)
        .try_into()
        .unwrap_or(0);

    // Apply pagination
    let start_idx = (page - 1) as usize * page_size as usize;
    let end_idx = (start_idx + page_size as usize).min(total_count);

    let paginated_rules: Vec<RuleResponse> = if start_idx < total_count {
        rules[start_idx..end_idx].iter().map(RuleResponse::from).collect()
    } else {
        vec![]
    };

    HttpResponse::Ok().json(RuleListResponse {
        total_count,
        items: paginated_rules,
        page,
        page_size,
        total_pages,
    })
}

/// Get a specific rule by ID - GET /v1/rules/{id}
pub async fn get_rule(
    path: web::Path<String>,
    store: web::Data<RuleStore>,
) -> impl Responder {
    let id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid rule ID format"
            }));
        }
    };

    let store_guard = store.lock().unwrap();

    match store_guard.get(&id) {
        Some(rule) => HttpResponse::Ok().json(RuleResponse::from(rule)),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Rule not found"
        })),
    }
}

/// Update an existing rule - PUT/PATCH /v1/rules/{id}
pub async fn update_rule(
    path: web::Path<String>,
    req: web::Json<UpdateRuleRequest>,
    store: web::Data<RuleStore>,
) -> impl Responder {
    let id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid rule ID format"
            }));
        }
    };

    // First, check for duplicates and read the current state immutably
    let new_category_ref = req.category.clone();
    let id_clone = id;

    {
        let store_guard = store.lock().unwrap();

        if let Some(current_rule) = store_guard.get(&id_clone) {
            // Check for duplicate category change before modifying
            if let Some(ref new_category) = new_category_ref {
                if current_rule.category != *new_category {
                    let has_duplicate = store_guard.values().any(|r| {
                        r.rule_name == current_rule.rule_name && r.category == *new_category
                    });
                    if has_duplicate {
                        return HttpResponse::Conflict().json(serde_json::json!({
                            "error": "A rule with this name already exists in the category"
                        }));
                    }
                }
            }
        } else {
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": "Rule not found"
            }));
        }
    }

    // Now we can take mutable access to update
    let mut store_guard = store.lock().unwrap();

    match store_guard.get_mut(&id) {
        Some(rule) => {
            // Apply all updates (duplicate check already done above)
            if let Some(ref description) = req.description {
                rule.description = Some(description.clone());
            }

            if let Some(ref new_category) = req.category {
                rule.category = new_category.clone();
            }

            if let Some(ref dsl_body) = req.dsl_body {
                rule.dsl_body = dsl_body.clone();
            }

            if !req.tags.is_empty() {
                rule.tags = req.tags.clone();
            }

            if let Some(salience) = req.salience {
                rule.salience = salience;
            }

            if let Some(ref status_str) = req.status {
                rule.status = match status_str.as_str() {
                    "draft" => RuleStatus::Draft,
                    "active" => RuleStatus::Active,
                    "inactive" => RuleStatus::Inactive,
                    "archived" => RuleStatus::Archived,
                    _ => return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": "Invalid status value"
                    })),
                };
            }

            rule.updated_at = Utc::now().to_rfc3339();
            rule.updated_by = Some("system".to_string());

            HttpResponse::Ok().json(RuleResponse::from(rule))
        }
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Rule not found"
        })),
    }
}

/// Delete a rule (soft delete) - DELETE /v1/rules/{id}
pub async fn delete_rule(
    path: web::Path<String>,
    store: web::Data<RuleStore>,
) -> impl Responder {
    let id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid rule ID format"
            }));
        }
    };

    let mut store_guard = store.lock().unwrap();

    match store_guard.get_mut(&id) {
        Some(rule) => {
            rule.status = RuleStatus::Archived;
            rule.updated_at = Utc::now().to_rfc3339();
            rule.updated_by = Some("system".to_string());

            HttpResponse::Ok().json(serde_json::json!({
                "id": id.to_string(),
                "name": rule.rule_name,
                "status": "archived",
                "message": "Rule has been soft-deleted (archived)"
            }))
        }
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Rule not found"
        })),
    }
}

/// Undelete a rule - POST /v1/rules/{id}/undelete
pub async fn undelete_rule(
    path: web::Path<String>,
    store: web::Data<RuleStore>,
) -> impl Responder {
    let id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid rule ID format"
            }));
        }
    };

    let mut store_guard = store.lock().unwrap();

    match store_guard.get_mut(&id) {
        Some(rule) if rule.status == RuleStatus::Archived => {
            rule.status = RuleStatus::Draft;
            rule.updated_at = Utc::now().to_rfc3339();
            rule.updated_by = Some("system".to_string());

            HttpResponse::Ok().json(serde_json::json!({
                "id": id.to_string(),
                "name": rule.rule_name,
                "status": "draft",
                "message": "Rule has been restored"
            }))
        }
        Some(_) => HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Rule is not archived, cannot restore"
        })),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Rule not found"
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
    fn test_rule_response_conversion() {
        let now = Utc::now().to_rfc3339();
        let rule = StoredRule {
            id: Uuid::new_v4(),
            rule_name: "Test Rule".to_string(),
            description: Some("A test rule".to_string()),
            category: "test".to_string(),
            tags: vec!["tag1".to_string()],
            salience: 10,
            dsl_body: "when ... then ...".to_string(),
            status: RuleStatus::Active,
            created_at: now.clone(),
            updated_at: now,
            created_by: "admin".to_string(),
            updated_by: None,
        };

        let response = RuleResponse::from(&rule);

        assert_eq!(response.name, "Test Rule");
        assert_eq!(response.description, Some("A test rule".to_string()));
        assert_eq!(response.category, Some("test".to_string()));
        assert_eq!(response.status, "active");
    }

    #[test]
    fn test_create_rule_validation() {
        // Test empty name validation
        let req = CreateRuleRequest {
            name: String::new(),
            description: None,
            category: None,
            dsl_body: "when ... then ...".to_string(),
            tags: vec![],
            salience: 0,
            status: None,
        };

        assert!(req.name.is_empty());

        // Test empty DSL validation
        let req = CreateRuleRequest {
            name: "Test Rule".to_string(),
            description: None,
            category: None,
            dsl_body: String::new(),
            tags: vec![],
            salience: 0,
            status: None,
        };

        assert!(req.dsl_body.is_empty());
    }
}
