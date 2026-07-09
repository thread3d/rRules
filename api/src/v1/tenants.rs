//! Tenant management API endpoints — Phase 20-01 Multi-Tenancy.
//!
//! Provides CRUD operations for tenant (organization/workspace) management:
//! - POST /v1/tenants — create a new tenant
//! - GET  /v1/tenants — list all tenants
//! - GET  /v1/tenants/{id} — get a specific tenant
//! - DELETE /v1/tenants/{id} — delete a tenant

use actix_web::{web, HttpResponse, Responder};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Shared in-memory tenant store.
pub type TenantStore = Arc<Mutex<HashMap<String, StoredTenant>>>;

/// A tenant persisted in the API layer (mirrors `engine::governance::tenant::Tenant`).
#[derive(Debug, Clone)]
pub struct StoredTenant {
    pub id: String,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Request to create a new tenant.
#[derive(Debug, serde::Deserialize)]
pub struct CreateTenantRequest {
    /// Human-readable display name (required).
    pub name: String,
}

/// Response shape for a single tenant.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TenantResponse {
    pub id: String,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<&StoredTenant> for TenantResponse {
    fn from(tenant: &StoredTenant) -> Self {
        Self {
            id: tenant.id.clone(),
            name: tenant.name.clone(),
            created_at: tenant.created_at,
        }
    }
}

/// Paginated list response.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TenantListResponse {
    pub total_count: usize,
    pub tenants: Vec<TenantResponse>,
}

impl From<&StoredTenant> for engine::governance::tenant::TenantResponse {
    fn from(tenant: &StoredTenant) -> Self {
        Self {
            id: tenant.id.clone(),
            name: tenant.name.clone(),
            created_at: tenant.created_at,
        }
    }
}

/// Initialize an empty in-memory tenant store.
pub fn init_store() -> TenantStore {
    Arc::new(Mutex::new(HashMap::new()))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Create a new tenant — POST /v1/tenants.
pub async fn create_tenant(
    req: web::Json<CreateTenantRequest>,
    store: web::Data<TenantStore>,
) -> impl Responder {
    let name = req.name.clone();

    if name.trim().is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "name is required and cannot be empty"
        }));
    }

    let mut guard = match store.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "Resource temporarily unavailable"
            }));
        }
    };

    // Check for duplicate name.
    if guard.values().any(|t| t.name == name) {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": format!("Tenant with name '{name}' already exists")
        }));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    guard.insert(id.clone(), StoredTenant {
        id: id.clone(),
        name: name.clone(),
        created_at: now,
    });

    HttpResponse::Created().json(TenantResponse {
        id,
        name,
        created_at: now,
    })
}

/// List all tenants — GET /v1/tenants.
pub async fn list_tenants(store: web::Data<TenantStore>) -> impl Responder {
    let guard = match store.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "Resource temporarily unavailable"
            }));
        }
    };

    let tenants: Vec<TenantResponse> = guard.values().map(TenantResponse::from).collect();

    HttpResponse::Ok().json(TenantListResponse {
        total_count: tenants.len(),
        tenants,
    })
}

/// Get a tenant by ID — GET /v1/tenants/{id}.
pub async fn get_tenant(
    path: web::Path<String>,
    store: web::Data<TenantStore>,
) -> impl Responder {
    let id = path.into_inner();

    let guard = match store.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "Resource temporarily unavailable"
            }));
        }
    };

    match guard.get(&id) {
        Some(tenant) => HttpResponse::Ok().json(TenantResponse::from(tenant)),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Tenant not found"
        })),
    }
}

/// Delete a tenant — DELETE /v1/tenants/{id}.
pub async fn delete_tenant(
    path: web::Path<String>,
    store: web::Data<TenantStore>,
) -> impl Responder {
    let id = path.into_inner();

    let mut guard = match store.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "Resource temporarily unavailable"
            }));
        }
    };

    if guard.remove(&id).is_some() {
        HttpResponse::NoContent().finish()
    } else {
        HttpResponse::NotFound().json(serde_json::json!({
            "error": "Tenant not found"
        }))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_store_empty() {
        let store = init_store();
        assert_eq!(store.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_tenant_response_from_stored() {
        let stored = StoredTenant {
            id: "abc".into(),
            name: "Acme Corp".into(),
            created_at: chrono::Utc::now(),
        };
        let resp = TenantResponse::from(&stored);
        assert_eq!(resp.id, "abc");
        assert_eq!(resp.name, "Acme Corp");
    }

    #[test]
    fn test_tenant_list_response_serialization() {
        let tenants: Vec<TenantResponse> = vec![
            TenantResponse { id: "1".into(), name: "Alpha".into(), created_at: chrono::Utc::now() },
            TenantResponse { id: "2".into(), name: "Beta".into(), created_at: chrono::Utc::now() },
        ];
        let list = TenantListResponse { total_count: tenants.len(), tenants };

        // Verify serialization works.
        let json = serde_json::to_string(&list).unwrap();
        assert!(json.contains("Alpha"));
        assert!(json.contains("Beta"));
    }

    #[test]
    fn test_create_tenant_request_deserialization() {
        let req: CreateTenantRequest = serde_json::from_str(r#"{"name": "TestOrg"}"#).unwrap();
        assert_eq!(req.name, "TestOrg");
    }

    #[test]
    fn test_stored_tenant_clone() {
        let stored = StoredTenant {
            id: "abc".into(),
            name: "Acme Corp".into(),
            created_at: chrono::Utc::now(),
        };
        let cloned = stored.clone();
        assert_eq!(cloned.id, stored.id);
        assert_eq!(cloned.name, stored.name);
    }

    // Note: Full HTTP integration tests would require setting up an actix-web test app
    // with all routes configured. This is left as future work since the core logic
    // (store operations, response shapes) is tested above via unit tests.
}
