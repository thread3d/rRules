//! Integration tests for the `/v1/tenants` endpoints and per-tenant rule
//! isolation.
//!
//! Routes are registered WITHOUT the project-wide `requires_auth` middleware so
//! these tests stay focused on the tenant handler contract; auth protection is
//! exercised by `auth_protection.rs`.

use actix_web::{body::BoxBody, dev::ServiceResponse, test, web, App};
use serde_json::{json, Value};

use api::v1::rules::{create_rule, get_rules, init_store as init_rule_store};
use api::v1::tenants::{
    create_tenant, delete_tenant, get_tenant, init_store as init_tenant_store, list_tenants,
};

/// Mount the tenant routes under `/v1` with a fresh in-memory store.
macro_rules! init_tenant_app {
    ($store:expr) => {
        test::init_service(
            App::new()
                .app_data($store)
                .route("/v1/tenants", web::get().to(list_tenants))
                .route("/v1/tenants", web::post().to(create_tenant))
                .route("/v1/tenants/{id}", web::get().to(get_tenant))
                .route("/v1/tenants/{id}", web::delete().to(delete_tenant)),
        )
        .await
    };
}

async fn body_json(resp: ServiceResponse<BoxBody>) -> Value {
    let bytes = test::read_body(resp).await;
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        panic!(
            "invalid JSON body: {e}; body={}",
            String::from_utf8_lossy(&bytes)
        )
    })
}

#[actix_web::test]
async fn create_then_get_and_list() {
    let app = init_tenant_app!(web::Data::new(init_tenant_store()));

    let req = test::TestRequest::post()
        .uri("/v1/tenants")
        .set_json(json!({ "name": "Acme Corp" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let created = body_json(resp).await;
    let id = created["id"].as_str().expect("id in response").to_string();
    assert_eq!(created["name"], "Acme Corp");
    assert!(created["created_at"].is_string());

    let req = test::TestRequest::get()
        .uri(&format!("/v1/tenants/{id}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["id"], id);

    let req = test::TestRequest::get().uri("/v1/tenants").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let listed = body_json(resp).await;
    assert_eq!(listed["total_count"], 1);
    assert_eq!(listed["tenants"][0]["name"], "Acme Corp");
}

#[actix_web::test]
async fn duplicate_name_conflicts() {
    let app = init_tenant_app!(web::Data::new(init_tenant_store()));

    for expected in [201u16, 409] {
        let req = test::TestRequest::post()
            .uri("/v1/tenants")
            .set_json(json!({ "name": "Duplicate Org" }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), expected);
    }
}

#[actix_web::test]
async fn blank_name_is_rejected() {
    let app = init_tenant_app!(web::Data::new(init_tenant_store()));

    let req = test::TestRequest::post()
        .uri("/v1/tenants")
        .set_json(json!({ "name": "   " }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn delete_then_missing() {
    let app = init_tenant_app!(web::Data::new(init_tenant_store()));

    let req = test::TestRequest::post()
        .uri("/v1/tenants")
        .set_json(json!({ "name": "To Delete" }))
        .to_request();
    let id = body_json(test::call_service(&app, req).await).await["id"]
        .as_str()
        .expect("id")
        .to_string();

    let req = test::TestRequest::delete()
        .uri(&format!("/v1/tenants/{id}"))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), 204);

    let req = test::TestRequest::get()
        .uri(&format!("/v1/tenants/{id}"))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), 404);

    let req = test::TestRequest::delete()
        .uri(&format!("/v1/tenants/{id}"))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), 404);
}

#[actix_web::test]
async fn unknown_tenant_is_not_found() {
    let app = init_tenant_app!(web::Data::new(init_tenant_store()));

    let req = test::TestRequest::get()
        .uri("/v1/tenants/does-not-exist")
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), 404);
}

/// Rule CRUD is scoped by the `X-Tenant-ID` header: two tenants never see each
/// other's rules, and the header defaults to `"default"`.
#[actix_web::test]
async fn rules_are_isolated_per_tenant() {
    let store = init_rule_store();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(store))
            .route("/v1/rules", web::get().to(get_rules))
            .route("/v1/rules", web::post().to(create_rule)),
    )
    .await;

    for (tenant, name) in [("acme", "acme-rule"), ("globex", "globex-rule")] {
        let req = test::TestRequest::post()
            .uri("/v1/rules")
            .insert_header(("X-Tenant-ID", tenant))
            .set_json(json!({
                "name": name,
                "description": "tenant scoped",
                "category": "pricing",
                "dsl_body": "rule x when true then y := 1"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201, "create for tenant {tenant}");
    }

    for (tenant, expected, unexpected) in [
        ("acme", "acme-rule", "globex-rule"),
        ("globex", "globex-rule", "acme-rule"),
    ] {
        let req = test::TestRequest::get()
            .uri("/v1/rules")
            .insert_header(("X-Tenant-ID", tenant))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body = body_json(resp).await;
        assert_eq!(body["total_count"], 1, "tenant {tenant} sees exactly one rule");
        let names: Vec<&str> = body["rules"]
            .as_array()
            .expect("rules array")
            .iter()
            .filter_map(|r| r["name"].as_str())
            .collect();
        assert!(names.contains(&expected), "tenant {tenant} sees {expected}");
        assert!(!names.contains(&unexpected), "tenant {tenant} must not see {unexpected}");
    }
}
