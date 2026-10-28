//! Integration tests for `POST /v1/simulation` (What-If analysis) — v4.0
//! hardening task A5.
//!
//! The test App mirrors production routing by registering the real
//! `configure_v1_simulation` inside the `/v1` scope. The `run_simulation`
//! handler takes only `web::Json` and needs no `app_data`.
//!
//! The request carries the rules to simulate (`rules`), so a run with rules reports which of them
//! fired. Until 9.0.67 the handler built an engine with no rules *and* the runner discarded the facts
//! it built, so every variant answered `success: true`, zero fired rules and no differences — an
//! answer to a question nobody had asked. The last three tests here are what makes the feature real.

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
    let app = test::init_service(
        App::new().service(web::scope("/v1").configure(configure_v1_simulation)),
    )
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
    let app = test::init_service(
        App::new().service(web::scope("/v1").configure(configure_v1_simulation)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/simulation")
        .set_json(json!({ "name": "bad", "fact_sets": [] }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status().as_u16(),
        400,
        "empty fact_sets must be rejected"
    );
}

#[actix_web::test]
async fn simulation_run_with_one_proposed_change_has_two_variants() {
    let app = test::init_service(
        App::new().service(web::scope("/v1").configure(configure_v1_simulation)),
    )
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

/// The rules a what-if run is a what-if of.
///
/// Until 9.0.67 the endpoint built a fresh engine with **no rules** and the runner discarded the facts
/// it built, so every variant reported `success: true`, zero fired rules and no differences. These two
/// tests are the difference between "the response has the right shape" and "the simulation simulates".
fn discount_rules() -> serde_json::Value {
    json!({
        "rules": [{
            "id": "gold-discount",
            "name": "Gold customers get a discount",
            "salience": 0,
            "no_loop": false,
            "lock_on_active": false,
            "condition": {"Pattern": {"fact_type": "customer", "field": "tier",
                                      "value": {"String": "Gold"}, "op": "Eq"}},
            "actions": [{"LogMessage": {"level": "Info", "message": "gold discount applied"}}]
        }]
    })
}

async fn run_simulation(body: Value) -> Value {
    let app = test::init_service(
        App::new().service(web::scope("/v1").configure(configure_v1_simulation)),
    )
    .await;
    let req = test::TestRequest::post()
        .uri("/v1/simulation")
        .set_json(body)
        .to_request();
    json_body(test::call_service(&app, req).await).await
}

#[actix_web::test]
async fn a_simulation_with_rules_reports_the_rule_that_fired() {
    let body = run_simulation(json!({
        "name": "gold-fires",
        "rules": discount_rules(),
        "fact_sets": [[{"id": "customer", "fields": {"tier": "Gold"}}]]
    }))
    .await;

    let variants = body["test_cases"][0]["variants"]
        .as_array()
        .expect("variants");
    let base = &variants[0];
    assert_eq!(base["variant_label"], "base");
    assert_eq!(
        base["rules_fired"],
        json!(["gold-discount"]),
        "the rule the facts satisfy must be reported as fired: {body}"
    );
}

#[actix_web::test]
async fn a_disabled_rule_makes_the_two_variants_differ() {
    let body = run_simulation(json!({
        "name": "disable-changes-the-answer",
        "rules": discount_rules(),
        "proposed_changes": [{"DisableRule": {"rule_id": "gold-discount"}}],
        "fact_sets": [[{"id": "customer", "fields": {"tier": "Gold"}}]]
    }))
    .await;

    let variants = body["test_cases"][0]["variants"]
        .as_array()
        .expect("variants");
    assert_eq!(variants.len(), 2, "base plus one variant: {body}");
    assert_eq!(
        variants[0]["rules_fired"],
        json!(["gold-discount"]),
        "base fires"
    );
    assert_eq!(
        variants[1]["rules_fired"],
        json!([]),
        "the disabled rule cannot fire: {body}"
    );
    assert!(
        !body["test_cases"][0]["differences"]
            .as_array()
            .expect("differences")
            .is_empty(),
        "a variant that behaves differently must produce a difference: {body}"
    );
}

#[actix_web::test]
async fn rules_that_do_not_parse_are_a_400() {
    let app = test::init_service(
        App::new().service(web::scope("/v1").configure(configure_v1_simulation)),
    )
    .await;
    let req = test::TestRequest::post()
        .uri("/v1/simulation")
        .set_json(json!({
            "name": "bad-rules",
            "rules": {"not": "a rule set"},
            "fact_sets": [[{"id": "customer", "fields": {}}]]
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400, "unparseable rules are rejected");
}
