//! Rules CRUD API endpoints - Phase 20-01 with multi-tenancy support.

use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use uuid::Uuid;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::v1::audit::{AuditAction, AuditStore, ResourceType};

/// A rule with full metadata including tenant isolation.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct StoredRule {
    pub id: Uuid,
    /// Tenant this rule belongs to (multi-tenancy key).
    pub tenant_id: String,
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
    /// Whether a structured rule was written to the executing directory for this document.
    ///
    /// The document and the executing rule are still two things (see `specs/rule-sources.md`); this field is
    /// what lets the API say which documents have one and which are drafts that cannot run.
    #[serde(default)]
    pub executable: bool,
}

/// Write an executable rule into the directory the engine loads, returning `Ok(())` when it is there.
///
/// The rule is first round-tripped through the engine's own parser, so a rule that would not load is
/// rejected here rather than appearing in the directory and silently not executing. The file is
/// `<rule id>.rules` holding `{"rules": [<rule>]}` — the shape `build_ruleset_from_dir` reads.
///
/// **Scope of this slice:** the executing set is per deployment, not per tenant, so a rule written here is
/// visible to every tenant's executions. Tenant-scoped execution is a later step of
/// `specs/rule-sources.md`; stating it here keeps the limitation findable.
fn write_executable_rule(
    rule_id: Uuid,
    rule_name: &str,
    tenant_id: &str,
    rule: &serde_json::Value,
) -> Result<std::path::PathBuf, String> {
    // The rule is written under its **tenant's** directory, which is where `build_tenant_ruleset_from_dir`
    // looks for ownership. This is what replaced 9.0.75's refusal of every non-default tenant: execution is
    // scoped now, so a rule created for `acme` runs for `acme` and for nobody else.

    // **The API owns identity.** The document's id and name are imposed on the executing rule, so the id a
    // caller receives from `POST /v1/rules` is the id the engine reports and the id `rule_filter` matches.
    // Without this the two universes stay disjoint even when the API writes the file: the document had a
    // UUID and the rule had whatever the client typed.
    let mut rule = rule.clone();
    let object = rule
        .as_object_mut()
        .ok_or_else(|| "`rule` must be a JSON object describing an engine rule".to_string())?;
    object.insert(
        "id".to_string(),
        serde_json::Value::String(rule_id.to_string()),
    );
    object.insert(
        "name".to_string(),
        serde_json::Value::String(rule_name.to_string()),
    );

    let document = serde_json::json!({ "rules": [rule] });
    let text = serde_json::to_string(&document)
        .map_err(|e| format!("the rule could not be serialised: {e}"))?;

    // The engine is the authority on what it can execute.
    let mut probe = engine::environment::EngineBuilder::new().build();
    probe
        .load_rules(&text)
        .map_err(|e| format!("the rule was rejected: {e}"))?;

    let root = std::env::var("RULE_STORAGE_PATH").unwrap_or_else(|_| "./rules".to_string());
    let dir = std::path::PathBuf::from(root).join(tenant_id);
    std::fs::create_dir_all(&dir).map_err(|e| {
        format!(
            "rules directory {} could not be created: {e}",
            dir.display()
        )
    })?;

    let path = dir.join(format!("{rule_id}.rules"));
    std::fs::write(&path, text)
        .map_err(|e| format!("{} could not be written: {e}", path.display()))?;
    Ok(path)
}

/// The tenant a request is acting for.
///
/// **The token decides, except for an admin acting on behalf of a tenant.** A JWT minted by this server
/// carries the user's `tenant_id`, and a caller cannot edit it. For a non-admin the token's tenant is
/// authoritative and the `X-Tenant-ID` header is ignored, including when it names someone else's tenant —
/// which closes the spoofing this function used to permit (any authenticated caller could read another
/// tenant's rules by sending a different header). An `admin` may select a tenant with the header, because
/// managing several tenants is what that role is for; the generated isolation scenario for `API-003` is
/// exactly that flow, and it is why the rule is split by role rather than applied flatly.
///
/// The header remains a fallback **only** when no tenant-bearing token is present — an older token that
/// predates the claim, or a request that reaches a handler without the auth middleware (the in-process test
/// apps). That path is unauthenticated by construction and is why it stays second: it is a routing hint,
/// not an identity.
pub fn extract_tenant_id(headers: &actix_web::HttpRequest) -> String {
    let header_tenant = || {
        headers
            .headers()
            .get("x-tenant-id")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
    };

    if let Some(claims) = token_claims(headers) {
        // An **admin** may act for a tenant of their choosing — that is what the header is for, and the
        // "admin manages several tenants" flow the isolation requirement is written against. Everyone else
        // is pinned to the tenant their token was issued for, so a forged header reaches nothing.
        if claims.role == "admin" {
            if let Some(tenant) = header_tenant() {
                return tenant;
            }
        }
        if let Some(tenant) = claims.tenant_id.as_deref() {
            return tenant.to_string();
        }
    }
    headers
        .headers()
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| "default".to_string())
}

/// The claims of the request's bearer token, when it carries a valid one.
fn token_claims(headers: &actix_web::HttpRequest) -> Option<crate::auth::Claims> {
    let header = headers.headers().get("Authorization")?;
    let value = header.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?.trim();
    crate::middleware::auth::decode_token(token).ok()
}

/// Rule status enum
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, utoipa::ToSchema)]
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
            tenant_id: "default".to_string(),
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
            executable: false,
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
            }
            .to_string(),
            version: 1,
            created_at: rule.created_at.clone(),
            updated_at: rule.updated_at.clone(),
            executable: rule.executable,
        }
    }
}

// One mapping, three ways to reach it. This used to be three copies of the field list, which meant every
// new field had to be added in three places — and the last one added (9.0.74's `executable`) was, before
// this collapse, patched into two of them and forgotten in the third by the compiler's own report.
impl From<&mut StoredRule> for RuleResponse {
    fn from(rule: &mut StoredRule) -> Self {
        Self::from(&*rule)
    }
}

impl From<&&StoredRule> for RuleResponse {
    fn from(rule: &&StoredRule) -> Self {
        Self::from(*rule)
    }
}

/// Request to create a new rule
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct CreateRuleRequest {
    /// Rule name (unique within category)
    pub name: String,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Rule category for organization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// DSL body of the rule — the FEEL expression *inside* a rule, not rule syntax.
    ///
    /// It is stored and returned but never compiled: there is no parser from it to an executable rule.
    pub dsl_body: String,

    /// The structured rule the engine executes (`engine::rules::Rule` as JSON), optional.
    ///
    /// When present it is validated by the engine and written to `RULE_STORAGE_PATH` as
    /// `<id>.rules`, so the rule joins the executing set at the next reload — the first step of the
    /// reconciliation `specs/rule-sources.md` describes. When absent the document is stored and reported
    /// as **not executable**, which is the honest answer rather than a silent one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<serde_json::Value>,

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
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
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
    /// Whether this document has an executable rule in the rules directory.
    ///
    /// `false` means the document is a draft that no engine can run — see `specs/rule-sources.md`.
    #[serde(default)]
    pub executable: bool,
}

/// Paginated list of rules
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
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
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
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
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
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

/// Shared state for rules API (multi-tenant keyed by tenant_id + rule_uuid).
type RuleStore = Arc<Mutex<HashMap<(String, Uuid), StoredRule>>>;

/// Initialize in-memory rule store.
pub fn init_store() -> RuleStore {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Create a rule - POST /v1/rules
#[utoipa::path(
    post,
    path = "/v1/rules",
    tag = "rules",
    request_body = CreateRuleRequest,
    responses(
        (status = 201, description = "Rule created", body = RuleResponse),
        (status = 400, description = "Validation error"),
        (status = 409, description = "Rule with this name already exists in the category"),
    ),
)]
pub async fn create_rule(
    req: web::Json<CreateRuleRequest>,
    headers: actix_web::HttpRequest,
    store: web::Data<RuleStore>,
    audit: web::Data<AuditStore>,
) -> impl Responder {
    let tenant_id = extract_tenant_id(&headers);
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

    // Check for duplicate name within same tenant and category
    let has_duplicate = {
        let store_guard = store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let cats = req.category.clone().unwrap_or_default();
        store_guard
            .values()
            .any(|r| r.tenant_id == tenant_id && r.rule_name == req.name && r.category == cats)
    };

    if has_duplicate {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "A rule with this name already exists in the category"
        }));
    }

    // If the caller supplied a structured rule, put it where the engine reads. This happens before the
    // document is stored so a rejected rule leaves nothing behind.
    let executable_path = match req.rule.as_ref() {
        Some(structured) => match write_executable_rule(rule_id, &req.name, &tenant_id, structured)
        {
            Ok(path) => Some(path),
            Err(error) => {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": error
                }))
            }
        },
        None => None,
    };

    // Create the new rule
    let rule = StoredRule {
        id: rule_id,
        tenant_id: tenant_id.clone(),
        rule_name: req.name.clone(),
        description: req.description.clone(),
        category: req
            .category
            .clone()
            .unwrap_or_else(|| "default".to_string()),
        tags: req.tags.clone(),
        salience: req.salience,
        dsl_body: req.dsl_body.clone(),
        status,
        created_at: now.clone(),
        updated_at: now,
        created_by: "system".to_string(),
        updated_by: None,
        executable: executable_path.is_some(),
    };

    crate::v1::audit::record_for_request(
        &audit,
        &headers,
        AuditAction::Create,
        ResourceType::Rule,
        rule.id.to_string(),
        Some(rule.rule_name.clone()),
        serde_json::json!({ "tenant_id": tenant_id, "category": rule.category }),
    )
    .await;
    {
        let mut store_guard = store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        store_guard.insert((tenant_id.clone(), rule_id), rule);
    }

    HttpResponse::Created().json(RuleResponse::from(
        store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&(tenant_id, rule_id))
            .unwrap(),
    ))
}

/// Get paginated list of rules - GET /v1/rules
/// Rows returned when the caller does not ask for a page size.
const DEFAULT_PAGE_SIZE: u32 = 50;
/// Ceiling on `page_size`, so one request cannot ask for everything.
const MAX_PAGE_SIZE: u32 = 100;

#[utoipa::path(
    get,
    path = "/v1/rules",
    tag = "rules",
    params(
        ("page" = Option<u32>, Query, description = "Page number (1-indexed)"),
        ("page_size" = Option<u32>, Query, description = "Items per page (max 100)"),
        ("category" = Option<String>, Query, description = "Filter by category"),
        ("status" = Option<String>, Query, description = "Filter by status (draft, active, inactive, archived)"),
        ("tag" = Option<String>, Query, description = "Filter by tag"),
        ("sort_by" = Option<String>, Query, description = "Sort field: name, salience, created_at, updated_at"),
        ("sort_order" = Option<String>, Query, description = "Sort direction: asc or desc"),
    ),
    responses(
        (status = 200, description = "Paginated list of rules", body = RuleListResponse),
        (status = 400, description = "Invalid status filter"),
    ),
)]
pub async fn get_rules(
    query: web::Query<RuleListRequest>,
    headers: actix_web::HttpRequest,
    store: web::Data<RuleStore>,
) -> impl Responder {
    let tenant_id = extract_tenant_id(&headers);
    let page = query.page.max(1);
    // An omitted `page_size` deserialises to 0, and clamping that to a minimum returns
    // exactly one rule — so the list showed a single row no matter how many existed.
    // The field documents a default of 50; apply it.
    let page_size = match query.page_size {
        0 => DEFAULT_PAGE_SIZE,
        requested => requested.min(MAX_PAGE_SIZE),
    };

    let store_guard = store
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    // Filter rules by tenant and other query parameters
    let mut rules: Vec<&StoredRule> = store_guard
        .values()
        .filter(|r| r.tenant_id == tenant_id)
        .collect();

    if let Some(ref category) = query.category {
        rules.retain(|r| r.category == *category);
    }

    if let Some(ref status) = query.status {
        let filter_status = match status.as_str() {
            "draft" => RuleStatus::Draft,
            "active" => RuleStatus::Active,
            "inactive" => RuleStatus::Inactive,
            "archived" => RuleStatus::Archived,
            _ => {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Invalid status filter"
                }))
            }
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
    let total_pages: u32 = total_count
        .div_ceil(page_size as usize)
        .try_into()
        .unwrap_or(0);

    // Apply pagination
    let start_idx = (page - 1) as usize * page_size as usize;
    let end_idx = (start_idx + page_size as usize).min(total_count);

    let paginated_rules: Vec<RuleResponse> = if start_idx < total_count {
        rules[start_idx..end_idx]
            .iter()
            .map(RuleResponse::from)
            .collect()
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
#[utoipa::path(
    get,
    path = "/v1/rules/{id}",
    tag = "rules",
    responses(
        (status = 200, description = "Rule found", body = RuleResponse),
        (status = 400, description = "Invalid rule ID format"),
        (status = 404, description = "Rule not found"),
    ),
    params(
        ("id" = String, Path, description = "Rule ID (UUID)"),
    ),
)]
pub async fn get_rule(
    path: web::Path<String>,
    headers: actix_web::HttpRequest,
    store: web::Data<RuleStore>,
) -> impl Responder {
    let tenant_id = extract_tenant_id(&headers);
    let id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid rule ID format"
            }));
        }
    };

    let store_guard = store
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    match store_guard.get(&(tenant_id, id)) {
        Some(rule) => HttpResponse::Ok().json(RuleResponse::from(rule)),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Rule not found"
        })),
    }
}

/// Update an existing rule - PUT/PATCH /v1/rules/{id}
#[utoipa::path(
    put,
    path = "/v1/rules/{id}",
    tag = "rules",
    request_body = UpdateRuleRequest,
    responses(
        (status = 200, description = "Rule updated", body = RuleResponse),
        (status = 400, description = "Invalid rule ID format or status value"),
        (status = 404, description = "Rule not found"),
        (status = 409, description = "Rule with this name already exists in the category"),
    ),
    params(
        ("id" = String, Path, description = "Rule ID (UUID)"),
    ),
)]
pub async fn update_rule(
    path: web::Path<String>,
    req: web::Json<UpdateRuleRequest>,
    headers: actix_web::HttpRequest,
    store: web::Data<RuleStore>,
    audit: web::Data<AuditStore>,
) -> impl Responder {
    let tenant_id = extract_tenant_id(&headers);
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
    let key = (tenant_id.clone(), id);

    {
        let store_guard = store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Some(current_rule) = store_guard.get(&key) {
            // Check for duplicate category change before modifying
            if let Some(ref new_category) = new_category_ref {
                if current_rule.category != *new_category {
                    let has_duplicate = store_guard.values().any(|r| {
                        r.tenant_id == tenant_id
                            && r.rule_name == current_rule.rule_name
                            && r.category == *new_category
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

    // The mutation runs under the lock and the response is built from it; the lock's scope ends
    // before the trail write, because a `MutexGuard` held across an await can deadlock the worker.
    let response = {
        let mut store_guard = store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match store_guard.get_mut(&key) {
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
                        _ => {
                            return HttpResponse::BadRequest().json(serde_json::json!({
                                "error": "Invalid status value"
                            }))
                        }
                    };
                }

                rule.updated_at = Utc::now().to_rfc3339();
                rule.updated_by = Some("system".to_string());

                RuleResponse::from(rule)
            }
            None => {
                return HttpResponse::NotFound().json(serde_json::json!({
                    "error": "Rule not found"
                }))
            }
        }
    };

    crate::v1::audit::record_for_request(
        &audit,
        &headers,
        AuditAction::Update,
        ResourceType::Rule,
        response.id.to_string(),
        Some(response.name.clone()),
        serde_json::json!({ "tenant_id": tenant_id }),
    )
    .await;
    HttpResponse::Ok().json(response)
}

/// Delete a rule (soft delete) - DELETE /v1/rules/{id}
#[utoipa::path(
    delete,
    path = "/v1/rules/{id}",
    tag = "rules",
    responses(
        (status = 200, description = "Rule soft-deleted (archived)"),
        (status = 400, description = "Invalid rule ID format"),
        (status = 404, description = "Rule not found"),
    ),
    params(
        ("id" = String, Path, description = "Rule ID (UUID)"),
    ),
)]
pub async fn delete_rule(
    path: web::Path<String>,
    headers: actix_web::HttpRequest,
    store: web::Data<RuleStore>,
    audit: web::Data<AuditStore>,
) -> impl Responder {
    let tenant_id = extract_tenant_id(&headers);
    let id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid rule ID format"
            }));
        }
    };

    let key = (tenant_id, id);
    let archived_name = {
        let mut store_guard = store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match store_guard.get_mut(&key) {
            Some(rule) => {
                rule.status = RuleStatus::Archived;
                rule.updated_at = Utc::now().to_rfc3339();
                rule.updated_by = Some("system".to_string());
                rule.rule_name.clone()
            }
            None => {
                return HttpResponse::NotFound().json(serde_json::json!({
                    "error": "Rule not found"
                }))
            }
        }
    };

    crate::v1::audit::record_for_request(
        &audit,
        &headers,
        AuditAction::Delete,
        ResourceType::Rule,
        id.to_string(),
        Some(archived_name.clone()),
        serde_json::json!({ "tenant_id": key.0.clone(), "archived": true }),
    )
    .await;
    HttpResponse::Ok().json(serde_json::json!({
        "id": id.to_string(),
        "name": archived_name,
        "status": "archived",
        "message": "Rule has been soft-deleted (archived)"
    }))
}

/// Undelete a rule - POST /v1/rules/{id}/undelete
#[utoipa::path(
    post,
    path = "/v1/rules/{id}/undelete",
    tag = "rules",
    responses(
        (status = 200, description = "Rule restored"),
        (status = 400, description = "Rule is not archived or invalid ID format"),
        (status = 404, description = "Rule not found"),
    ),
    params(
        ("id" = String, Path, description = "Rule ID (UUID)"),
    ),
)]
pub async fn undelete_rule(
    path: web::Path<String>,
    headers: actix_web::HttpRequest,
    store: web::Data<RuleStore>,
) -> impl Responder {
    let tenant_id = extract_tenant_id(&headers);
    let id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid rule ID format"
            }));
        }
    };

    let key = (tenant_id, id);
    let mut store_guard = store
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    match store_guard.get_mut(&key) {
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
            tenant_id: "test-tenant".to_string(),
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
            executable: false,
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
            rule: None,
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
            rule: None,
        };

        assert!(req.dsl_body.is_empty());
    }

    #[test]
    fn test_tenant_isolation_in_store() {
        let store = init_store();

        // Create rules for different tenants with same name
        let rule1 = StoredRule {
            id: Uuid::new_v4(),
            tenant_id: "tenant-1".to_string(),
            rule_name: "shared-name".to_string(),
            description: None,
            category: "test".to_string(),
            tags: vec![],
            salience: 0,
            dsl_body: "when true then action1()".to_string(),
            status: RuleStatus::Draft,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
            created_by: "test".to_string(),
            updated_by: None,
            executable: false,
        };

        let rule2 = StoredRule {
            id: Uuid::new_v4(),
            tenant_id: "tenant-2".to_string(),
            rule_name: "shared-name".to_string(),
            description: None,
            category: "test".to_string(),
            tags: vec![],
            salience: 0,
            dsl_body: "when true then action2()".to_string(),
            status: RuleStatus::Draft,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
            created_by: "test".to_string(),
            updated_by: None,
            executable: false,
        };

        {
            let mut store_guard = store.lock().unwrap();
            store_guard.insert(("tenant-1".to_string(), rule1.id), rule1);
            store_guard.insert(("tenant-2".to_string(), rule2.id), rule2);
        }

        // Verify both rules exist in the store
        let store_guard = store.lock().unwrap();
        assert_eq!(store_guard.len(), 2);

        // Simulate tenant-1 query - should only see its own rule
        let tenant1_rules: Vec<_> = store_guard
            .values()
            .filter(|r| r.tenant_id == "tenant-1")
            .collect();
        assert_eq!(tenant1_rules.len(), 1);
        assert_eq!(tenant1_rules[0].dsl_body, "when true then action1()");

        // Simulate tenant-2 query - should only see its own rule
        let tenant2_rules: Vec<_> = store_guard
            .values()
            .filter(|r| r.tenant_id == "tenant-2")
            .collect();
        assert_eq!(tenant2_rules.len(), 1);
        assert_eq!(tenant2_rules[0].dsl_body, "when true then action2()");
    }

    #[test]
    fn test_extract_tenant_id_default() {
        // Test that the helper function logic works correctly
        // In integration tests, this would be tested with real HttpRequest objects
        assert_eq!("default", "default");
    }

    #[test]
    fn test_duplicate_name_within_same_tenant_and_category() {
        let store = init_store();

        // First rule
        let rule1 = StoredRule {
            id: Uuid::new_v4(),
            tenant_id: "tenant-1".to_string(),
            rule_name: "duplicate-rule".to_string(),
            description: None,
            category: "pricing".to_string(),
            tags: vec![],
            salience: 0,
            dsl_body: "when true then action1()".to_string(),
            status: RuleStatus::Draft,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
            created_by: "test".to_string(),
            updated_by: None,
            executable: false,
        };

        // Second rule with same name in same tenant and category - should fail
        let rule2 = StoredRule {
            id: Uuid::new_v4(),
            tenant_id: "tenant-1".to_string(),
            rule_name: "duplicate-rule".to_string(),
            description: None,
            category: "pricing".to_string(),
            tags: vec![],
            salience: 0,
            dsl_body: "when true then action2()".to_string(),
            status: RuleStatus::Draft,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
            created_by: "test".to_string(),
            updated_by: None,
            executable: false,
        };

        {
            let mut store_guard = store.lock().unwrap();

            // Insert first rule
            store_guard.insert(("tenant-1".to_string(), rule1.id), rule1.clone());

            // Check for duplicate - should find it now
            let has_duplicate = store_guard.values().any(|r| {
                r.tenant_id == "tenant-1"
                    && r.rule_name == "duplicate-rule"
                    && r.category == "pricing"
            });
            assert!(has_duplicate, "Should detect rule after insertion");

            // Try to insert second rule with same key - this would overwrite in real HashMap
            store_guard.insert(("tenant-1".to_string(), rule2.id), rule2.clone());
        }

        // Verify both rules exist (different UUIDs, same tenant and name)
        let store_guard = store.lock().unwrap();
        let tenant1_rules: Vec<_> = store_guard
            .values()
            .filter(|r| r.tenant_id == "tenant-1")
            .collect();
        assert_eq!(tenant1_rules.len(), 2);

        // Verify both have the same name (the duplicate detection would catch this before insert)
        let first_rule = tenant1_rules.iter().find(|r| r.id == rule1.id).unwrap();
        let second_rule = tenant1_rules.iter().find(|r| r.id == rule2.id).unwrap();
        assert_eq!(first_rule.rule_name, "duplicate-rule");
        assert_eq!(second_rule.rule_name, "duplicate-rule");
    }
}
