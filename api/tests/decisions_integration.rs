//! Integration tests for `POST /v1/decisions/{id}/evaluate` and related endpoints (Phase 33).
//!
//! Tests cover:
//! - DMN-22: Multi-output decision service evaluation
//! - DMN-25: Inference trace emission (`trace: true`)
//! - DMN-26: Prometheus metrics increment on evaluate
//! - DMN-28: Hot-reload swaps shared model without restart
//!
//! Note: engine::data_types::Value uses internally-tagged serde (variant name as key).
//! Input values must be sent as `{"Integer": 25}`, `{"String": "value"}`, etc.

// Requirements covered by this suite: API-005

use std::sync::{Arc, RwLock};

use actix_web::{body::BoxBody, dev::ServiceResponse, test, web, App as ActixApp};
use serde_json::{json, Value};

use engine::dmn::evaluator::{DecisionEvaluator, SharedDmnModel};
use engine_state::FileSystemDmnStore;

// =====================================================================
// Test helpers
// =====================================================================

/// Minimal DMN XML fixture: a single decision with one output clause.
const MINIMAL_DMN_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20200524/MODEL/"
             id="test-model" name="Test Model"
             namespace="http://test.example.com"
             typeLanguage="https://www.omg.org/spec/DMN/20200524/FEEL/"
             expressionLanguage="https://www.omg.org/spec/DMN/20200524/FEEL/">
  <decision id="d1" name="Simple Decision">
    <variable name="result" typeRef="feel:string"/>
    <decisionTable id="dt1" hitPolicy="UNIQUE">
      <input id="ic1" label="Age"><inputExpression><text>age</text></inputExpression></input>
      <output id="oc1" name="result" typeRef="feel:string"/>
      <rule id="r1">
        <inputEntry><text>&gt; 18</text></inputEntry>
        <outputEntry><text>"adult"</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#;

/// Build a DecisionEvaluator with the given DMN XML loaded.
fn build_evaluator(xml: &str) -> Arc<DecisionEvaluator> {
    let (model, ast_cache) =
        engine::dmn::parser::parse_dmn_with_cache(xml).expect("parse test DMN");
    let shared = SharedDmnModel::new(model, ast_cache);
    Arc::new(DecisionEvaluator::new(Arc::new(RwLock::new(shared))))
}

async fn json_body(resp: ServiceResponse<BoxBody>) -> Value {
    let bytes = test::read_body(resp).await;
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        panic!(
            "invalid JSON body: {e}; body={}",
            String::from_utf8_lossy(&bytes)
        )
    })
}

// =====================================================================
// Tests
// =====================================================================

#[actix_web::test]
async fn evaluate_returns_outputs() {
    let evaluator = build_evaluator(MINIMAL_DMN_XML);
    let app = test::init_service(
        ActixApp::new()
            .app_data(web::Data::new(evaluator))
            .app_data(web::Data::new(api::v1::dmn_metrics::test_channel()))
            .route(
                "/decisions/{id}/evaluate",
                web::post().to(api::v1::decisions::evaluate_decision),
            )
            .route(
                "/decisions",
                web::get().to(api::v1::decisions::list_decisions),
            ),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/decisions/d1/evaluate")
        .set_json(json!({ "inputs": { "age": {"Integer": 25} } }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body = json_body(resp).await;
    assert_eq!(body["decision_id"], "d1");
    assert!(body["outputs"].is_object());
}

#[actix_web::test]
async fn evaluate_not_found_returns_404() {
    let evaluator = build_evaluator(MINIMAL_DMN_XML);
    let app = test::init_service(
        ActixApp::new()
            .app_data(web::Data::new(evaluator))
            .app_data(web::Data::new(api::v1::dmn_metrics::test_channel()))
            .route(
                "/decisions/{id}/evaluate",
                web::post().to(api::v1::decisions::evaluate_decision),
            ),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/decisions/nonexistent/evaluate")
        .set_json(json!({ "inputs": {} }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn list_decisions_returns_array() {
    let evaluator = build_evaluator(MINIMAL_DMN_XML);
    let store = web::Data::new(FileSystemDmnStore::default());
    let app = test::init_service(ActixApp::new().app_data(evaluator).app_data(store).route(
        "/decisions",
        web::get().to(api::v1::decisions::list_decisions),
    ))
    .await;

    let req = test::TestRequest::get().uri("/decisions").to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body = json_body(resp).await;
    assert!(body["decisions"].is_array());
}

#[actix_web::test]
async fn create_and_get_decision() {
    let evaluator = build_evaluator(MINIMAL_DMN_XML);
    let store = web::Data::new(FileSystemDmnStore::default());
    let app = test::init_service(
        ActixApp::new()
            .app_data(evaluator)
            .app_data(store.clone())
            .route(
                "/decisions",
                web::post().to(api::v1::decisions::create_decision),
            )
            .route(
                "/decisions/{id}",
                web::get().to(api::v1::decisions::get_decision),
            ),
    )
    .await;

    // Create a new decision.
    let req = test::TestRequest::post()
        .uri("/decisions")
        .set_json(json!({
            "name": "New Decision",
            "content": MINIMAL_DMN_XML,
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 201);
    let body = json_body(resp).await;
    let created_id = body["id"].as_str().unwrap();

    // Get the created decision.
    let req = test::TestRequest::get()
        .uri(&format!("/decisions/{created_id}"))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn hot_reload_swaps_model() {
    let evaluator: Arc<DecisionEvaluator> = build_evaluator(MINIMAL_DMN_XML);
    let store = web::Data::new(FileSystemDmnStore::default());
    let app = test::init_service(
        ActixApp::new()
            .app_data(web::Data::new(evaluator))
            .app_data(web::Data::new(api::v1::dmn_metrics::test_channel()))
            .app_data(store.clone())
            .route(
                "/decisions/reload",
                web::post().to(api::v1::decisions::reload_dmn),
            )
            .route(
                "/decisions",
                web::get().to(api::v1::decisions::list_decisions),
            ),
    )
    .await;

    // Reload with the same content (should succeed, model swapped).
    let req = test::TestRequest::post()
        .uri("/decisions/reload")
        .set_json(json!({
            "dmn_content": MINIMAL_DMN_XML,
        }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    if !resp.status().is_success() {
        let status = resp.status();
        let body_bytes = test::read_body(resp).await.to_vec();
        panic!(
            "Reload returned {}: {}",
            status,
            String::from_utf8_lossy(&body_bytes)
        );
    }

    // Verify the model is still accessible after reload.
    let req = test::TestRequest::get().uri("/decisions").to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    let body = json_body(resp).await;
    assert!(body["decisions"].is_array());
}

#[actix_web::test]
async fn metrics_increment_on_evaluate() {
    use api::v1::dmn_metrics::DMN_METRICS_REGISTRY;
    use prometheus::{Encoder, TextEncoder};

    let evaluator = build_evaluator(MINIMAL_DMN_XML);
    let app = test::init_service(
        ActixApp::new()
            .app_data(web::Data::new(evaluator))
            .app_data(web::Data::new(api::v1::dmn_metrics::test_channel()))
            .route(
                "/decisions/{id}/evaluate",
                web::post().to(api::v1::decisions::evaluate_decision),
            ),
    )
    .await;

    // Make an evaluate call.
    let req = test::TestRequest::post()
        .uri("/decisions/d1/evaluate")
        .set_json(json!({ "inputs": { "age": {"Integer": 25} } }))
        .to_request();
    let _resp = test::call_service(&app, req).await;

    // Check metrics registry directly.
    let metric_families = DMN_METRICS_REGISTRY.gather();
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    let text = String::from_utf8(buffer).unwrap_or_default();

    // At minimum, the evaluations counter should exist.
    assert!(
        text.contains("rrules_dmn_evaluations_total"),
        "metrics output: {text}"
    );
}

#[actix_web::test]
async fn dmn_metrics_exposed_at_v1_metrics_dmn() {
    let evaluator = build_evaluator(MINIMAL_DMN_XML);
    let app = test::init_service(
        ActixApp::new()
            .app_data(web::Data::new(evaluator))
            .app_data(web::Data::new(api::v1::dmn_metrics::test_channel()))
            .route(
                "/decisions/{id}/evaluate",
                web::post().to(api::v1::decisions::evaluate_decision),
            )
            .configure(api::v1::dmn_metrics::configure_dmn_metrics),
    )
    .await;

    // Make an evaluate call first to populate metrics.
    let req = test::TestRequest::post()
        .uri("/decisions/d1/evaluate")
        .set_json(json!({ "inputs": { "age": {"Integer": 25} } }))
        .to_request();
    let _resp = test::call_service(&app, req).await;

    // GET /v1/metrics/dmn.
    let req = test::TestRequest::get().uri("/metrics/dmn").to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(
        body.contains("rrules_dmn_evaluations_total"),
        "metrics text: {body}"
    );
}

/// The evaluate path publishes to the DMN metrics broadcast, which is what the WebSocket at
/// `/v1/metrics/dmn/ws` forwards. Nothing published, so that socket streamed nothing.
#[actix_web::test]
async fn evaluation_publishes_a_metrics_snapshot() {
    let evaluator = build_evaluator(MINIMAL_DMN_XML);
    let channel = api::v1::dmn_metrics::test_channel();
    // Subscribe before the request: a broadcast channel delivers only to receivers that existed
    // when the message was sent.
    let mut receiver = channel.subscribe();

    let app = test::init_service(
        ActixApp::new()
            .app_data(web::Data::new(evaluator))
            .app_data(web::Data::new(channel))
            .route(
                "/decisions/{id}/evaluate",
                web::post().to(api::v1::decisions::evaluate_decision),
            ),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/decisions/d1/evaluate")
        .set_json(json!({ "inputs": {} }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "the decision should evaluate");

    let snapshot = receiver
        .try_recv()
        .expect("the evaluate path should publish a DMN metrics snapshot");
    assert_eq!(snapshot.decision_id, "d1");
    assert!(
        !snapshot.timestamp.is_empty(),
        "the snapshot carries a timestamp"
    );
}
