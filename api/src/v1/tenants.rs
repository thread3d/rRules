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

use crate::v1::tenant_store::TenantBackend;
use engine::governance::tenant::{
    CreateTenantRequest, TenantListResponse, TenantResponse, TenantStoreError,
};

/// The tenant registry: in memory by default, PostgreSQL when `TENANT_PERSISTENCE=postgres`.
pub type TenantStore = TenantBackend;

/// An empty in-memory registry.
pub fn init_store() -> TenantStore {
    TenantBackend::in_memory()
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
        TenantStoreError::Backend(_) => {
            // The caller's request was fine; the storage is not. 503 says that, where 409 would blame
            // the user for someone else's outage.
            HttpResponse::ServiceUnavailable().json(serde_json::json!({ "error": err.to_string() }))
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
    match store.create(&req.name).await {
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
    let listed = match store.list().await {
        Ok(listed) => listed,
        Err(err) => return store_error_response(err),
    };
    let tenants: Vec<TenantResponse> = listed.iter().map(TenantResponse::from).collect();

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

    match store.get(&id).await {
        Ok(Some(tenant)) => HttpResponse::Ok().json(TenantResponse::from(&tenant)),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Tenant not found"
        })),
        Err(err) => store_error_response(err),
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

    match store.delete(&id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Tenant not found"
        })),
        Err(err) => store_error_response(err),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn init_store_is_empty() {
        let store = init_store();
        assert_eq!(store.kind(), "memory");
        assert!(store.list().await.expect("listed").is_empty());
    }

    #[actix_web::test]
    async fn create_then_get_roundtrip() {
        let store = TenantBackend::in_memory();
        let created = store.create("Acme Corp").await.expect("create");
        let fetched = store
            .get(&created.id)
            .await
            .expect("looked up")
            .expect("tenant present");
        assert_eq!(fetched.name, "Acme Corp");
    }

    #[actix_web::test]
    async fn duplicate_name_is_rejected() {
        let store = TenantBackend::in_memory();
        store.create("Acme").await.expect("first create");
        let err = store.create("Acme").await.expect_err("duplicate rejected");
        assert!(matches!(err, TenantStoreError::DuplicateName(_)));
    }

    #[actix_web::test]
    async fn blank_name_is_rejected() {
        let store = TenantBackend::in_memory();
        let err = store.create("   ").await.expect_err("blank rejected");
        assert_eq!(err, TenantStoreError::EmptyName);
    }

    #[actix_web::test]
    async fn names_are_trimmed() {
        let store = TenantBackend::in_memory();
        let created = store.create("  Acme  ").await.expect("create");
        assert_eq!(created.name, "Acme");
    }

    #[actix_web::test]
    async fn delete_is_idempotent() {
        let store = TenantBackend::in_memory();
        let created = store.create("ToDelete").await.expect("create");
        assert!(store.delete(&created.id).await.expect("deleted"));
        assert!(!store.delete(&created.id).await.expect("deleted"));
        assert!(store.get(&created.id).await.expect("looked up").is_none());
    }

    #[actix_web::test]
    async fn list_is_newest_first() {
        let store = TenantBackend::in_memory();
        let first = store.create("First").await.expect("create");
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = store.create("Second").await.expect("create");

        let listed = store.list().await.expect("listed");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, second.id);
        assert_eq!(listed[1].id, first.id);
    }

    #[actix_web::test]
    async fn list_response_serializes() {
        let store = TenantBackend::in_memory();
        let a = store.create("Alpha").await.expect("create");
        let b = store.create("Beta").await.expect("create");
        let tenants: Vec<TenantResponse> = store
            .list()
            .await
            .expect("listed")
            .iter()
            .map(TenantResponse::from)
            .collect();
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
