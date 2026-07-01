//! Integration tests for `POST /v1/rules/batch-execute` and the `execute_single`
//! refactor (v4.0 hardening tasks A4/A5).
//!
//! The test App registers only the execution routes — the handlers take only
//! `web::Json` and need no `app_data`. Route ordering mirrors `main.rs`:
//! `/rules/batch-execute` is registered before the `/rules/{id}` wildcard so
//! the literal path wins.
//!
//! Note: `execute_rules` / `execute_single` are currently stubs — `rules_fired`
//! is always `0`. These tests assert structure and `succeeded` counts, not
//! `rules_fired > 0` (functional rule firing is a future milestone, out of scope
//! for v4.0 hardening).

use actix_web::{body::BoxBody, test, web, App, HttpResponse, Responder};
use serde_json::{json, Value};

use api::v1::execution::{batch_execute_rules, execute_rules};

/// Stand-in for the `/rules/{id}` handler, registered AFTER the static route to
/// prove the wildcard does NOT shadow `/rules/batch-execute`. A misroute would
/// return this marker instead of a batch response.
async fn wildcard_marker(_path: web::Path<String>) -> impl Responder {
    HttpResponse::Ok().json(json!({ "hit": "wildcard", "rules_fired": -1 }))
}

fn configure_test_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            // Static route FIRST (mirrors main.rs ordering).
            .route("/rules/batch-execute", web::post().to(batch_execute_rules))
            .route("/execute", web::post().to(execute_rules))
            // Wildcard AFTER — must not shadow the static path above.
            .route("/rules/{id}", web::get().to(wildcard_marker)),
    );
}

/// Reads a success response body as JSON, panicking with a useful message on
/// non-2xx status or invalid JSON.
async fn json_body(resp: actix_web::dev::ServiceResponse<BoxBody>) -> Value {
    assert!(
        resp.status().is_success(),
        "expected success, got {}",
        resp.status()
    );
    let bytes = test::read_body(resp).await;
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        panic!(
            "invalid JSON body: {e}; body={}",
            String::from_utf8_lossy(&bytes)
        )
    })
}

#[actix_web::test]
async fn batch_execute_runs_two_requests_successfully() {
    let app = test::init_service(App::new().configure(configure_test_routes)).await;

    let req = test::TestRequest::post()
        .uri("/v1/rules/batch-execute")
        .set_json(json!({
            "requests": [
                {"facts": {"customer": {"tier": "Gold"}}},
                {"facts": {"customer": {"tier": "Silver"}}}
            ]
        }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert_eq!(body["succeeded"], 2, "body was: {body}");
    assert_eq!(body["failed"], 0);
    // execute_rules is a stub → rules_fired always 0, so the batch total is 0.
    assert_eq!(body["total_fired"], 0);
    let results = body["results"].as_array().expect("results is an array");
    assert_eq!(results.len(), 2);
    for r in results {
        assert_eq!(r["success"], true, "each result successful: {r}");
        assert!(r["response"]["rules_fired"].is_number(), "response present");
        assert!(r["response"]["execution_id"].is_string(), "execution_id present");
    }
}

#[actix_web::test]
async fn batch_execute_empty_requests_yields_empty_results() {
    let app = test::init_service(App::new().configure(configure_test_routes)).await;

    let req = test::TestRequest::post()
        .uri("/v1/rules/batch-execute")
        .set_json(json!({ "requests": [] }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert_eq!(body["succeeded"], 0);
    assert_eq!(body["failed"], 0);
    assert_eq!(body["total_fired"], 0);
    assert_eq!(body["results"].as_array().unwrap().len(), 0);
}

#[actix_web::test]
async fn batch_execute_respects_max_concurrency_without_deadlock() {
    // Many requests with a low concurrency cap: the Semaphore must release
    // permits as tasks finish so all complete (no deadlock).
    let app = test::init_service(App::new().configure(configure_test_routes)).await;

    let requests: Vec<Value> = (0..10)
        .map(|i| json!({ "facts": { "customer": { "tier": format!("T{i}") } } }))
        .collect();
    let req = test::TestRequest::post()
        .uri("/v1/rules/batch-execute")
        .set_json(json!({ "requests": requests, "max_concurrency": 2 }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert_eq!(body["succeeded"], 10, "all requests should complete; body: {body}");
    assert_eq!(body["failed"], 0);
    assert_eq!(body["results"].as_array().unwrap().len(), 10);
}

#[actix_web::test]
async fn batch_execute_static_path_beats_wildcard() {
    // Registering both the static path and the {id} wildcard: POSTing the
    // static path must reach the batch handler, not the wildcard marker.
    let app = test::init_service(App::new().configure(configure_test_routes)).await;

    let req = test::TestRequest::post()
        .uri("/v1/rules/batch-execute")
        .set_json(json!({ "requests": [{"facts": {}}] }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;
    // A misroute to the wildcard would yield {"hit":"wildcard","rules_fired":-1}
    // and would lack `succeeded`.
    assert!(
        body.get("succeeded").is_some(),
        "static /rules/batch-execute must win over /rules/{{id}}; body: {body}"
    );
    assert_ne!(body["hit"], "wildcard", "wildcard must not shadow static path");
}

#[actix_web::test]
async fn single_execute_still_works_after_refactor() {
    // Confirms the execute_rules → execute_single refactor didn't break the
    // single-request path.
    let app = test::init_service(App::new().configure(configure_test_routes)).await;

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "customer": { "tier": "Gold" } } }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert_eq!(body["rules_fired"], 0, "stub — rules_fired always 0");
    assert!(body["execution_id"].is_string(), "execution_id present");
    assert_eq!(body["rules_not_fired"].as_array().unwrap().len(), 0);
}