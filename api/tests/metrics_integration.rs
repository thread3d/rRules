//! Integration tests for the metrics endpoints (v4.0 hardening task A5).
//!
//! The metrics handlers take no `app_data`. The Prometheus `/metrics` endpoint
//! stays at root (`configure_metrics`); the dashboard endpoints
//! `/metrics/dead-rules` and `/metrics/coverage` live inside the `/v1` scope
//! (`configure_metrics_dashboards`), resolving to `/v1/metrics/dead-rules` and
//! `/v1/metrics/coverage` (MON-03/MON-04). Prometheus tests register only
//! `configure_metrics` and hit `/metrics`; dashboard tests mirror `main.rs` by
//! registering both configs (`configure_metrics` at root + `/v1` scope with
//! `configure_metrics_dashboards`) and hit `/v1/metrics/*`. The coverage test
//! pre-populates the global `CoverageTracking` singleton before the request to
//! prove the endpoint reads real tracked data — this is the functional proof
//! that Phase 19's coverage gap closure works end-to-end (the endpoint reads
//! from the tracker rather than a hardcoded empty array).

// Requirements covered by this suite: API-007

use actix_web::{body::BoxBody, test, web, App};
use serde_json::Value;

use api::metrics::{configure_metrics, configure_metrics_dashboards, ExecutionCounters};
use engine::monitoring::coverage::CoverageTracking;

/// Unique rule id for the coverage test. The `CoverageTracking` singleton
/// persists across tests in one process, so a unique id avoids cross-test
/// interference.
const COVERAGE_TEST_RULE: &str = "coverage-int-test-rule-9F2A";

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
async fn metrics_prometheus_text_format() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(ExecutionCounters::default()))
            .configure(configure_metrics),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/metrics")
        .insert_header(("Accept", "text/plain"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "status: {}", resp.status());
    let bytes = test::read_body(resp).await;
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("# HELP"),
        "expected Prometheus exposition format; got: {text}"
    );
    assert!(
        text.contains("rrules_execute_requests_total"),
        "expected the rrules_execute_requests_total metric; got: {text}"
    );
}

#[actix_web::test]
async fn metrics_json_engine_state() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(ExecutionCounters::default()))
            .configure(configure_metrics),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/metrics")
        .insert_header(("Accept", "application/json"))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert_eq!(body["engine_name"], "default");
    assert!(body["rules_fired"].is_number(), "rules_fired numeric: {body}");
    assert!(body["memory_bytes"].is_number(), "memory_bytes numeric: {body}");
}

#[actix_web::test]
async fn dead_rules_returns_valid_json_array() {
    // dead_rules_endpoint reads from the process-wide RuleExecutionTracker
    // singleton (Plan 26-02 / D-03). The singleton is populated by the
    // recording call sites in direct_eval.rs and rete/mod.rs; this test does
    // not execute any rules, so the singleton is empty and the endpoint
    // returns []. We assert valid structure (a JSON array) — non-emptiness
    // after an execute is proven by the keystone_dead_rules e2e in
    // execute_integration.rs (SC-1).
    //
    // The dashboard routes live inside the /v1 scope (configure_metrics_dashboards),
    // so the test App mirrors main.rs: configure_metrics (root Prometheus) +
    // /v1 scope with configure_metrics_dashboards. The request URI is therefore
    // /v1/metrics/dead-rules.
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(ExecutionCounters::default()))
            .configure(configure_metrics)
            .service(web::scope("/v1").configure(configure_metrics_dashboards)),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v1/metrics/dead-rules")
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert!(body.is_array(), "expected a JSON array; got: {body}");
}

#[actix_web::test]
async fn coverage_endpoint_reads_tracked_data() {
    // Pre-populate the global singleton BEFORE the request. The coverage
    // endpoint reads the same singleton, so the recorded rule must surface in
    // the response — proving the endpoint reads real tracked data.
    //
    // The coverage route lives inside the /v1 scope (configure_metrics_dashboards),
    // so the test App mirrors main.rs and the request URI is /v1/metrics/coverage.
    CoverageTracking::record_rule_eval(COVERAGE_TEST_RULE);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(ExecutionCounters::default()))
            .configure(configure_metrics)
            .service(web::scope("/v1").configure(configure_metrics_dashboards)),
    )
    .await;
    let req = test::TestRequest::get()
        .uri("/v1/metrics/coverage")
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    let rules = body["rules"].as_array().expect("rules is an array");
    let found = rules
        .iter()
        .any(|r| r["rule_id"] == COVERAGE_TEST_RULE && r["evaluated"] == true);
    assert!(
        found,
        "coverage endpoint must surface the recorded rule {COVERAGE_TEST_RULE}; body: {body}"
    );
    // The summary must reflect at least one evaluated rule.
    assert!(
        body["summary"]["evaluated_count"].as_u64().is_some_and(|c| c >= 1),
        "summary.evaluated_count must be >= 1; body: {body}"
    );
}