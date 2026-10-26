//! Audit trail API endpoints - Phase 20-04

use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use uuid::Uuid;

use actix_web::HttpMessage;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Audit action types
#[derive(Debug, Clone, serde::Serialize, PartialEq, utoipa::ToSchema)]
pub enum AuditAction {
    Create,
    Read,
    Update,
    Delete,
    Approve,
    Reject,
    Execute,
    Export,
}

/// Resource types that can be audited
#[derive(Debug, Clone, serde::Serialize, PartialEq, utoipa::ToSchema)]
pub enum ResourceType {
    Rule,
    Category,
    Tag,
    User,
    Approval,
}

/// A single audit event
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: String,
    pub user_id: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub resource_name: Option<String>,
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Paginated audit event list
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct AuditListResponse {
    pub total_count: usize,
    pub events: Vec<AuditEvent>,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

/// Query parameters for audit events
#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct AuditQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    #[serde(rename = "resource_type")]
    pub resource_type: Option<String>,
    #[serde(rename = "action")]
    pub action: Option<String>,
    pub user_id: Option<String>,
    pub since: Option<String>,
}

/// Shared state for audit API
pub type AuditStore = Arc<Mutex<HashMap<Uuid, StoredAuditEvent>>>;

/// Internal representation of an audit event
#[derive(Debug, Clone)]
pub struct StoredAuditEvent {
    pub id: Uuid,
    pub timestamp: String,
    pub user_id: Option<String>,
    pub action: AuditAction,
    pub resource_type: ResourceType,
    pub resource_id: String,
    pub resource_name: Option<String>,
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

impl From<&StoredAuditEvent> for AuditEvent {
    fn from(event: &StoredAuditEvent) -> Self {
        AuditEvent {
            id: event.id.to_string(),
            timestamp: event.timestamp.clone(),
            user_id: event.user_id.clone(),
            action: match event.action {
                AuditAction::Create => "create",
                AuditAction::Read => "read",
                AuditAction::Update => "update",
                AuditAction::Delete => "delete",
                AuditAction::Approve => "approve",
                AuditAction::Reject => "reject",
                AuditAction::Execute => "execute",
                AuditAction::Export => "export",
            }
            .to_string(),
            resource_type: match event.resource_type {
                ResourceType::Rule => "rule",
                ResourceType::Category => "category",
                ResourceType::Tag => "tag",
                ResourceType::User => "user",
                ResourceType::Approval => "approval",
            }
            .to_string(),
            resource_id: event.resource_id.clone(),
            resource_name: event.resource_name.clone(),
            details: event.details.clone(),
            ip_address: event.ip_address.clone(),
            user_agent: event.user_agent.clone(),
        }
    }
}

/// Initialize in-memory audit store
pub fn init_store() -> AuditStore {
    let store: HashMap<Uuid, StoredAuditEvent> = HashMap::new();
    Arc::new(Mutex::new(store))
}

/// Query audit events - GET /v1/audit
#[utoipa::path(
    get,
    path = "/v1/audit",
    tag = "audit",
    params(
        ("page" = Option<u32>, Query, description = "Page number (1-indexed)"),
        ("page_size" = Option<u32>, Query, description = "Items per page (max 100)"),
        ("resource_type" = Option<String>, Query, description = "Filter by resource type (rule, category, tag, user, approval)"),
        ("action" = Option<String>, Query, description = "Filter by action (create, read, update, delete, approve, reject, execute, export)"),
        ("user_id" = Option<String>, Query, description = "Filter by user ID"),
        ("since" = Option<String>, Query, description = "Filter events since timestamp (RFC 3339)"),
    ),
    responses(
        (status = 200, description = "Paginated audit events", body = AuditListResponse),
        (status = 400, description = "Invalid filter value"),
    ),
)]
pub async fn query_audit_events(
    query: web::Query<AuditQuery>,
    store: web::Data<AuditStore>,
) -> impl Responder {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 100);

    let store_guard = store
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    // Filter events based on query parameters
    let mut events: Vec<&StoredAuditEvent> = store_guard.values().collect();

    if let Some(ref resource_type) = query.resource_type {
        let filter_type = match resource_type.as_str() {
            "rule" => ResourceType::Rule,
            "category" => ResourceType::Category,
            "tag" => ResourceType::Tag,
            "user" => ResourceType::User,
            "approval" => ResourceType::Approval,
            _ => {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Invalid resource_type filter"
                }))
            }
        };
        events.retain(|e| e.resource_type == filter_type);
    }

    if let Some(ref action) = query.action {
        let filter_action = match action.as_str() {
            "create" => AuditAction::Create,
            "read" => AuditAction::Read,
            "update" => AuditAction::Update,
            "delete" => AuditAction::Delete,
            "approve" => AuditAction::Approve,
            "reject" => AuditAction::Reject,
            "execute" => AuditAction::Execute,
            "export" => AuditAction::Export,
            _ => {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Invalid action filter"
                }))
            }
        };
        events.retain(|e| e.action == filter_action);
    }

    if let Some(ref user_id) = query.user_id {
        events.retain(|e| e.user_id.as_ref() == Some(user_id));
    }

    // Sort by timestamp descending (most recent first)
    events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let total_count = events.len();
    let total_pages = total_count
        .div_ceil(page_size as usize)
        .try_into()
        .unwrap_or(0);

    // Apply pagination
    let start_idx = (page - 1) as usize * page_size as usize;
    let end_idx = (start_idx + page_size as usize).min(total_count);

    let paginated_events: Vec<AuditEvent> = if start_idx < total_count {
        events[start_idx..end_idx]
            .iter()
            .map(|e| AuditEvent::from(*e))
            .collect()
    } else {
        vec![]
    };

    HttpResponse::Ok().json(AuditListResponse {
        total_count,
        events: paginated_events,
        page,
        page_size,
        total_pages,
    })
}

/// Export audit log to CSV - GET /v1/audit/export
#[utoipa::path(
    get,
    path = "/v1/audit/export",
    tag = "audit",
    responses(
        (status = 200, description = "Audit log exported as CSV", content_type = "text/csv"),
    ),
)]
pub async fn export_audit_log(store: web::Data<AuditStore>) -> impl Responder {
    let store_guard = store
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    // Build CSV content
    let mut csv =
        "id,timestamp,user_id,action,resource_type,resource_id,resource_name,details\n".to_string();

    for event in store_guard.values() {
        let details_json = serde_json::to_string(&event.details).unwrap_or_default();
        let details_escaped = details_json.replace('"', "\"\"");

        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            event.id,
            event.timestamp,
            event.user_id.clone().unwrap_or_else(|| "".to_string()),
            match event.action {
                AuditAction::Create => "create",
                AuditAction::Read => "read",
                AuditAction::Update => "update",
                AuditAction::Delete => "delete",
                AuditAction::Approve => "approve",
                AuditAction::Reject => "reject",
                AuditAction::Execute => "execute",
                AuditAction::Export => "export",
            },
            match event.resource_type {
                ResourceType::Rule => "rule",
                ResourceType::Category => "category",
                ResourceType::Tag => "tag",
                ResourceType::User => "user",
                ResourceType::Approval => "approval",
            },
            event.resource_id,
            event
                .resource_name
                .clone()
                .unwrap_or_else(|| "".to_string()),
            details_escaped
        ));
    }

    HttpResponse::Ok()
        .content_type("text/csv")
        .insert_header((
            "Content-Disposition",
            "attachment; filename=\"audit_log.csv\"",
        ))
        .body(csv)
}

/// Record a new audit event
#[allow(clippy::too_many_arguments)]
/// Record an event for a request that just changed something.
///
/// The actor comes from the `Claims` the auth middleware put in the request extensions, the
/// address from the peer connection, the agent from the header — the three things an audit trail
/// needs and that a handler would otherwise have to unpack at every call site.
pub fn record_for_request(
    store: &AuditStore,
    request: &actix_web::HttpRequest,
    action: AuditAction,
    resource_type: ResourceType,
    resource_id: String,
    resource_name: Option<String>,
    details: serde_json::Value,
) {
    // `extensions()` comes from `HttpMessage`; the auth middleware put the `Claims` there.
    let user_id = request
        .extensions()
        .get::<crate::auth::Claims>()
        .map(|claims| claims.sub.clone());
    let ip_address = request.peer_addr().map(|addr| addr.ip().to_string());
    let user_agent = request
        .headers()
        .get(actix_web::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    record_audit_event(
        store,
        NewAuditEvent {
            action,
            resource_type,
            resource_id,
            resource_name,
            details,
            user_id,
            ip_address,
            user_agent,
        },
    );
}

pub struct NewAuditEvent {
    pub action: AuditAction,
    pub resource_type: ResourceType,
    pub resource_id: String,
    pub resource_name: Option<String>,
    pub details: serde_json::Value,
    pub user_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Append an event to the trail.
///
/// Takes a struct rather than nine positional arguments: at a call site, two `Option<String>`s in a
/// row (`user_id`, `ip_address`) are indistinguishable, and swapping them would silently write the
/// wrong one. Callers describing a request should use [`record_for_request`].
pub fn record_audit_event(store: &AuditStore, new: NewAuditEvent) {
    let mut store_guard = store
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let event_id = Uuid::new_v4();

    let event = StoredAuditEvent {
        id: event_id,
        timestamp: Utc::now().to_rfc3339(),
        user_id: new.user_id,
        action: new.action,
        resource_type: new.resource_type,
        resource_id: new.resource_id,
        resource_name: new.resource_name,
        details: new.details,
        ip_address: new.ip_address,
        user_agent: new.user_agent,
    };

    store_guard.insert(event_id, event);
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
    fn test_audit_event_conversion() {
        let event = StoredAuditEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now().to_rfc3339(),
            user_id: Some("user123".to_string()),
            action: AuditAction::Create,
            resource_type: ResourceType::Rule,
            resource_id: "rule-123".to_string(),
            resource_name: Some("Test Rule".to_string()),
            details: serde_json::json!({"field": "value"}),
            ip_address: Some("127.0.0.1".to_string()),
            user_agent: Some("test-agent".to_string()),
        };

        let api_event = AuditEvent::from(&event);

        assert_eq!(api_event.user_id, Some("user123".to_string()));
        assert_eq!(api_event.action, "create");
        assert_eq!(api_event.resource_type, "rule");
    }
}
