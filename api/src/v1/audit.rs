//! Audit trail API endpoints - Phase 20-04

use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use uuid::Uuid;

use crate::v1::audit_store::{AuditBackend, AuditFilter};
use actix_web::HttpMessage;

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
/// The trail's storage. `AuditBackend` is in memory by default and PostgreSQL when
/// `AUDIT_PERSISTENCE=postgres`; recording is async because a database write is.
pub type AuditStore = AuditBackend;

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
    AuditBackend::in_memory()
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

    let filter = match audit_filter(&query) {
        Ok(filter) => filter,
        Err(response) => return response,
    };

    let offset = (page as usize - 1) * page_size as usize;
    let total_count = match store.count(&filter).await {
        Ok(count) => count,
        Err(error) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({ "error": error }))
        }
    };
    let events = match store.page(&filter, offset, page_size as usize).await {
        Ok(events) => events,
        Err(error) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({ "error": error }))
        }
    };

    HttpResponse::Ok().json(AuditListResponse {
        total_count,
        events: events.iter().map(AuditEvent::from).collect(),
        page,
        page_size,
        total_pages: total_count
            .div_ceil(page_size as usize)
            .try_into()
            .unwrap_or(0),
    })
}

/// The reader's filter, or the response explaining what is wrong with it.
///
/// Shared by the list and the export so the two cannot disagree about what `action=approve` means.
fn audit_filter(query: &AuditQuery) -> Result<AuditFilter, HttpResponse> {
    let resource_type = match query.resource_type.as_deref() {
        None => None,
        Some("rule") => Some(ResourceType::Rule),
        Some("category") => Some(ResourceType::Category),
        Some("tag") => Some(ResourceType::Tag),
        Some("user") => Some(ResourceType::User),
        Some("approval") => Some(ResourceType::Approval),
        Some(_) => {
            return Err(HttpResponse::BadRequest()
                .json(serde_json::json!({ "error": "Invalid resource_type filter" })))
        }
    };

    let action = match query.action.as_deref() {
        None => None,
        Some("create") => Some(AuditAction::Create),
        Some("read") => Some(AuditAction::Read),
        Some("update") => Some(AuditAction::Update),
        Some("delete") => Some(AuditAction::Delete),
        Some("approve") => Some(AuditAction::Approve),
        Some("reject") => Some(AuditAction::Reject),
        Some("execute") => Some(AuditAction::Execute),
        Some("export") => Some(AuditAction::Export),
        Some(_) => {
            return Err(HttpResponse::BadRequest()
                .json(serde_json::json!({ "error": "Invalid action filter" })))
        }
    };

    Ok(AuditFilter {
        resource_type,
        action,
        user_id: query.user_id.clone(),
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
    // An export is a full read: no filter, and a page large enough that the caller sees the trail
    // rather than the first page of it.
    let events = match store.page(&AuditFilter::default(), 0, usize::MAX).await {
        Ok(events) => events,
        Err(error) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({ "error": error }))
        }
    };

    // Build CSV content
    let mut csv =
        "id,timestamp,user_id,action,resource_type,resource_id,resource_name,details\n".to_string();

    for event in &events {
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
pub async fn record_for_request(
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
    )
    .await;
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
pub async fn record_audit_event(store: &AuditStore, new: NewAuditEvent) {
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

    // A trail that fails to record must not fail the request that was already performed: the
    // mutation happened. Losing the record silently would be worse, so it is logged loudly.
    if let Err(error) = store.append(event).await {
        tracing::error!(%error, "the audit trail could not record an event");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn test_init_store() {
        // A fresh trail is empty and stays in memory unless a deployment says otherwise.
        let store = init_store();
        assert_eq!(store.kind(), "memory");
        assert_eq!(
            store.count(&AuditFilter::default()).await.expect("counted"),
            0
        );
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
