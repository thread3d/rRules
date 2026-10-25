//! Tenant management API endpoints — multi-tenancy.
//!
//! Provides CRUD operations for tenant (organisation/workspace) management:
//! - POST   /v1/tenants      — create a new tenant
//! - GET    /v1/tenants      — list all tenants (newest first)
//! - GET    /v1/tenants/{id} — get a specific tenant
//! - DELETE /v1/tenants/{id} — delete a tenant
//!
//! Rule CRUD is already scoped per tenant by the `X-Tenant-ID` header; this
//! module manages the tenants themselves. The store is
//! [`engine::governance::tenant::MemoryTenantStore`] behind a `std::sync::Mutex`
//! so handlers can use its synchronous methods without holding a lock across an
//! `.await`.

use actix_web::{web, HttpResponse, Responder};
use std::sync::{Arc, Mutex};

use engine::governance::tenant::{
    CreateTenantRequest, MemoryTenantStore, TenantListResponse, TenantResponse, TenantStoreError,
};

/// Shared in-memory tenant store.
pub type TenantStore = Arc<Mutex<MemoryTenantStore>>;

/// Initialize an empty in-memory tenant store.
pub fn init_store() -> TenantStore {
    Arc::new(Mutex::new(MemoryTenantStore::new()))
}

/// Map a store error to the matching HTTP status.
fn store_error_response(err: TenantStoreError) -> HttpResponse {
    match err {
        TenantStoreError::DuplicateName(_) => {
            HttpResponse::Conflict().json(serde_json::json!({ "error": err.to_string() }))
        }
        TenantStoreError::EmptyName => {
            HttpResponse::BadRequest().json(serde_json::json!({ "error": err.to_string() }))
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Create a new tenant — POST /v1/tenants.
#[utoipa::path(
    post,
    path = "/v1/tenants",
    tag = "tenants",
    request_body = CreateTenantRequest,
    responses(
        (status = 201, description = "Tenant created", body = TenantResponse),
        (status = 400, description = "Blank name"),
        (status = 409, description = "Tenant name already exists"),
    ),
)]
pub async fn create_tenant(
    req: web::Json<CreateTenantRequest>,
    store: web::Data<TenantStore>,
) -> impl Responder {
    let mut guard = match store.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "Resource temporarily unavailable"
            }));
        }
    };

    match guard.create(&req.name) {
        Ok(tenant) => HttpResponse::Created().json(TenantResponse::from(&tenant)),
        Err(err) => store_error_response(err),
    }
}

/// List all tenants — GET /v1/tenants.
#[utoipa::path(
    get,
    path = "/v1/tenants",
    tag = "tenants",
    responses(
        (status = 200, description = "All tenants, newest first", body = TenantListResponse),
    ),
)]
pub async fn list_tenants(store: web::Data<TenantStore>) -> impl Responder {
    let guard = match store.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": "Resource temporarily unavailable"
            }));
        }
    };

    let tenants: Vec<TenantResponse> = guard.list().iter().map(TenantResponse::from).collect();

    HttpResponse::Ok().json(TenantListResponse {
        total_count: tenants.len(),
        tenants,
    })
}

/// Get a tenant by ID — GET /v1/tenants/{id}.
#[utoipa::path(
    get,
    path = "/v1/tenants/{id}",
    tag = "tenants",
    params(("id" = String, Path, description = "Tenant id")),
    responses(
        (status = 200, description = "Tenant found", body = TenantResponse),
        (status = 404, description = "Unknown tenant"),
    ),
)]
pub async fn get_tenant(path: web::Path<String>, store: web::Data<TenantStore>) -> impl Responder {
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
#[utoipa::path(
    delete,
    path = "/v1/tenants/{id}",
    tag = "tenants",
    params(("id" = String, Path, description = "Tenant id")),
    responses(
        (status = 204, description = "Tenant deleted"),
        (status = 404, description = "Unknown tenant"),
    ),
)]
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

    if guard.delete(&id) {
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
    fn init_store_is_empty() {
        let store = init_store();
        assert!(store.lock().unwrap().list().is_empty());
    }

    #[test]
    fn create_then_get_roundtrip() {
        let mut store = MemoryTenantStore::new();
        let created = store.create("Acme Corp").expect("create");
        let fetched = store.get(&created.id).expect("tenant present");
        assert_eq!(fetched.name, "Acme Corp");
    }

    #[test]
    fn duplicate_name_is_rejected() {
        let mut store = MemoryTenantStore::new();
        store.create("Acme").expect("first create");
        let err = store.create("Acme").expect_err("duplicate rejected");
        assert!(matches!(err, TenantStoreError::DuplicateName(_)));
    }

    #[test]
    fn blank_name_is_rejected() {
        let mut store = MemoryTenantStore::new();
        let err = store.create("   ").expect_err("blank rejected");
        assert_eq!(err, TenantStoreError::EmptyName);
    }

    #[test]
    fn names_are_trimmed() {
        let mut store = MemoryTenantStore::new();
        let created = store.create("  Acme  ").expect("create");
        assert_eq!(created.name, "Acme");
    }

    #[test]
    fn delete_is_idempotent() {
        let mut store = MemoryTenantStore::new();
        let created = store.create("ToDelete").expect("create");
        assert!(store.delete(&created.id));
        assert!(!store.delete(&created.id));
        assert!(store.get(&created.id).is_none());
    }

    #[test]
    fn list_is_newest_first() {
        let mut store = MemoryTenantStore::new();
        let first = store.create("First").expect("create");
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = store.create("Second").expect("create");

        let listed = store.list();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, second.id);
        assert_eq!(listed[1].id, first.id);
    }

    #[test]
    fn list_response_serializes() {
        let mut store = MemoryTenantStore::new();
        let a = store.create("Alpha").expect("create");
        let b = store.create("Beta").expect("create");
        let tenants: Vec<TenantResponse> = store.list().iter().map(TenantResponse::from).collect();
        let list = TenantListResponse {
            total_count: tenants.len(),
            tenants,
        };

        let json = serde_json::to_string(&list).expect("serialize");
        assert!(json.contains(&a.name));
        assert!(json.contains(&b.name));
        assert!(json.contains("\"total_count\":2"));
    }

    #[test]
    fn create_request_deserializes() {
        let req: CreateTenantRequest =
            serde_json::from_str(r#"{"name": "TestOrg"}"#).expect("deserialize");
        assert_eq!(req.name, "TestOrg");
    }
}
