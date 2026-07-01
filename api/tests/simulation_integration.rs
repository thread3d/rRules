//! Integration tests for `POST /v1/simulation` (What-If analysis) — v4.0
//! hardening task A5.
//!
//! The test App mirrors production routing by registering the real
//! `configure_v1_simulation` inside the `/v1` scope. The `run_simulation`
//! handler takes only `web::Json` and needs no `app_data`.
//!
//! Note: the simulation runner executes against a freshly built (empty) engine,
//! so no rules fire. These tests assert response structure and summary counts,
//! not `rules_fired > 0`.

use actix_web::{body::BoxBody, dev::ServiceResponse, test, web, App};
use serde_json::{json, Value};

use api::v1::configure_v1_simulation;

async fn json_body(resp: ServiceResponse<BoxBody>) -> Value {
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
async fn simulation_run_valid_returns_results() {
    let app = test::init_service(App::new().service(web::scope("/v1").configure(configure_v1_simulation)))
        .await;

    let req = test::TestRequest::post()
        .uri("/v1/simulation")
        .set_json(json!({
            "name": "whatif-basic",
            "fact_sets": [[{"id": "customer", "fields": {"tier": "Gold"}}]]
        }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert_eq!(body["name"], "whatif-basic");
    assert!(body["test_cases"].is_array(), "test_cases present: {body}");
    assert!(
        !body["test_cases"].as_array().unwrap().is_empty(),
        "non-empty test_cases: {body}"
    );
    assert_eq!(body["summary"]["total_fact_sets"], 1);
    // No proposed_changes → only the base variant.
    assert_eq!(body["summary"]["total_variants"], 1);
}

#[actix_web::test]
async fn simulation_run_empty_fact_sets_rejected_400() {
    let app = test::init_service(App::new().service(web::scope("/v1").configure(configure_v1_simulation)))
        .await;

    let req = test::TestRequest::post()
        .uri("/v1/simulation")
        .set_json(json!({ "name": "bad", "fact_sets": [] }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400, "empty fact_sets must be rejected");
}

#[actix_web::test]
async fn simulation_run_with_one_proposed_change_has_two_variants() {
    let app = test::init_service(App::new().service(web::scope("/v1").configure(configure_v1_simulation)))
        .await;

    let req = test::TestRequest::post()
        .uri("/v1/simulation")
        .set_json(json!({
            "name": "whatif-2variants",
            "proposed_changes": [
                {"RemoveRule": {"rule_id": "nonexistent-rule"}}
            ],
            "fact_sets": [[{"id": "customer", "fields": {"tier": "Gold"}}]]
        }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    // total_variants = 1 (base) + proposed_changes.len(). The handler computes
    // this from the request, independent of whether the change applied.
    assert_eq!(
        body["summary"]["total_variants"], 2,
        "base + 1 proposed change → 2 variants; body: {body}"
    );
    assert_eq!(body["summary"]["total_fact_sets"], 1);
}