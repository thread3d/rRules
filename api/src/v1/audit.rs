//! Audit trail API endpoints - Phase 20-04

use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use uuid::Uuid;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Audit action types
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
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
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub enum ResourceType {
    Rule,
    Category,
    Tag,
    User,
    Approval,
}

/// A single audit event
#[derive(Debug, Clone, serde::Serialize)]
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
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuditListResponse {
    pub total_count: usize,
    pub events: Vec<AuditEvent>,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

/// Query parameters for audit events
#[derive(Debug, Clone, serde::Deserialize)]
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
type AuditStore = Arc<Mutex<HashMap<Uuid, StoredAuditEvent>>>;

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
            }.to_string(),
            resource_type: match event.resource_type {
                ResourceType::Rule => "rule",
                ResourceType::Category => "category",
                ResourceType::Tag => "tag",
                ResourceType::User => "user",
                ResourceType::Approval => "approval",
            }.to_string(),
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
pub async fn query_audit_events(
    query: web::Query<AuditQuery>,
    store: web::Data<AuditStore>,
) -> impl Responder {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).max(1).min(100);

    let store_guard = store.lock().unwrap();

    // Filter events based on query parameters
    let mut events: Vec<&StoredAuditEvent> = store_guard.values().collect();

    if let Some(ref resource_type) = query.resource_type {
        let filter_type = match resource_type.as_str() {
            "rule" => ResourceType::Rule,
            "category" => ResourceType::Category,
            "tag" => ResourceType::Tag,
            "user" => ResourceType::User,
            "approval" => ResourceType::Approval,
            _ => return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid resource_type filter"
            })),
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
            _ => return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid action filter"
            })),
        };
        events.retain(|e| e.action == filter_action);
    }

    if let Some(ref user_id) = query.user_id {
        events.retain(|e| e.user_id.as_ref() == Some(user_id));
    }

    // Sort by timestamp descending (most recent first)
    events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let total_count = events.len();
    let total_pages = ((total_count + page_size as usize - 1) / page_size as usize)
        .try_into()
        .unwrap_or(0);

    // Apply pagination
    let start_idx = (page - 1) as usize * page_size as usize;
    let end_idx = (start_idx + page_size as usize).min(total_count);

    let paginated_events: Vec<AuditEvent> = if start_idx < total_count {
        events[start_idx..end_idx].iter().map(|e| AuditEvent::from(*e)).collect()
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

/// Get audit event by ID - GET /v1/audit/{id}
pub async fn get_audit_event(
    path: web::Path<String>,
    store: web::Data<AuditStore>,
) -> impl Responder {
    let id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid audit event ID format"
            }));
        }
    };

    let store_guard = store.lock().unwrap();

    match store_guard.get(&id) {
        Some(event) => HttpResponse::Ok().json(AuditEvent::from(event)),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Audit event not found"
        })),
    }
}

/// Export audit log to CSV - GET /v1/audit/export
pub async fn export_audit_log(store: web::Data<AuditStore>) -> impl Responder {
    let store_guard = store.lock().unwrap();

    // Build CSV content
    let mut csv = "id,timestamp,user_id,action,resource_type,resource_id,resource_name,details\n".to_string();

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
            event.resource_name.clone().unwrap_or_else(|| "".to_string()),
            details_escaped
        ));
    }

    HttpResponse::Ok()
        .content_type("text/csv")
        .insert_header(("Content-Disposition", "attachment; filename=\"audit_log.csv\""))
        .body(csv)
}

/// Record a new audit event
pub fn record_audit_event(
    store: &AuditStore,
    action: AuditAction,
    resource_type: ResourceType,
    resource_id: String,
    resource_name: Option<String>,
    details: serde_json::Value,
    user_id: Option<String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
) {
    let mut store_guard = store.lock().unwrap();
    let event_id = Uuid::new_v4();

    let event = StoredAuditEvent {
        id: event_id,
        timestamp: Utc::now().to_rfc3339(),
        user_id,
        action,
        resource_type,
        resource_id,
        resource_name,
        details,
        ip_address,
        user_agent,
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
