//! Integration tests for `POST /v1/rules/batch-execute` and the `execute_single`
//! refactor (v4.0 hardening tasks A4/A5).
//!
//! The test App installs the same `app_data` stack Plan 23-05 wired into
//! `main.rs` — `EngineRegistry` (`register_default_engines`),
//! `EngineSelector::with_defaults()`, and a shared `Arc<RwLock<RuleSet>>` —
//! because `execute_rules` / `batch_execute_rules` now extract those handles
//! (Plan 23-05 rewrote them off the pre-23-05 stub). Route ordering mirrors
//! `main.rs`: `/rules/batch-execute` is registered before the `/rules/{id}`
//! wildcard so the literal path wins.
//!
//! The shared `RuleSet` is intentionally EMPTY. These tests assert batch
//! structure, `succeeded` counts, and concurrency — not `rules_fired > 0`
//! (functional rule firing is covered by `execute_integration.rs`). With no
//! rules loaded, the now-real engine fires nothing, so `rules_fired == 0` and
//! `total_fired == 0` hold as they did under the pre-23-05 stub.

// Requirements covered by this suite: API-006

use std::sync::{Arc, RwLock};

use actix_web::{body::BoxBody, test, web, App, HttpResponse, Responder};
use serde_json::{json, Value};

use api::metrics::{create_metrics_channel, ExecutionCounters, MetricsTx};
use api::v1::execution::{batch_execute_rules, execute_rules, SharedRuleSet};
use api::v1::sessions::{read_session_config, SessionManager};
use engine::engine::registry::EngineRegistry;
use engine::engine::selector::EngineSelector;
use engine::rules::RuleSet;

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

/// Build a real `EngineRegistry` with all four default factories registered
/// (Plan 23-01 D-01: `available_engines()` truthfully returns
/// `rete`/`treat`/`leaps`/`phreak`). Plan 23-05's `execute_rules` /
/// `batch_execute_rules` extract `web::Data<EngineRegistry>`, so the test App
/// must install it or the handlers return 500 on the missing extractor.
fn shared_registry() -> web::Data<EngineRegistry> {
    let mut registry = EngineRegistry::new();
    registry.register_default_engines();
    web::Data::new(registry)
}

/// Build an `EngineSelector::with_defaults()` (post-construction default
/// engine is `"rete"`). Used only for per-request default resolution when a
/// request omits `engine` (ENGINE-08); `select_engine(&mut self)` is NOT
/// called per request (Pitfall 7).
fn shared_selector() -> web::Data<EngineSelector> {
    web::Data::new(EngineSelector::with_defaults())
}

/// Build a shared EMPTY `RuleSet`. These tests assert batch structure,
/// `succeeded` counts, and concurrency — not `rules_fired > 0` — so the
/// ruleset carries no rules: the now-real engine (Plan 23-05) fires nothing
/// and the pre-23-05 stub-era `rules_fired == 0` / `total_fired == 0`
/// assertions stay valid.
fn shared_ruleset() -> web::Data<Arc<RwLock<SharedRuleSet>>> {
    web::Data::new(Arc::new(RwLock::new(SharedRuleSet::new(RuleSet::new()))))
}

/// Build a shared `ExecutionCounters` (Plan 24-03). `execute_rules` and
/// `batch_execute_rules` now extract `web::Data<ExecutionCounters>`, so the
/// test App must install it or the handlers return 500 on the missing
/// extractor.
fn shared_counters() -> web::Data<ExecutionCounters> {
    web::Data::new(ExecutionCounters::default())
}

/// Build a shared `MetricsTx` (broadcast sender) for the test App. Plan 24-03
/// changed `execute_rules` / `batch_execute_rules` to extract
/// `web::Data<MetricsTx>` and broadcast a `MetricsSnapshot` after each
/// execute; installing it keeps the test App forward-compatible. The receiver
/// is dropped here; no test in this file reads from the broadcast channel.
fn shared_metrics_tx() -> web::Data<MetricsTx> {
    let (tx, _rx) = create_metrics_channel(16);
    web::Data::new(tx)
}

/// Phase 27 (SESS-01): shared SessionManager for the test App —
/// `execute_rules` / `batch_execute_rules` now extract
/// `web::Data<Arc<SessionManager>>` as a required param, so the test App
/// must install it or the handlers return 500 on the missing extractor.
fn shared_session_manager() -> web::Data<std::sync::Arc<SessionManager>> {
    web::Data::new(std::sync::Arc::new(SessionManager::new(
        read_session_config(),
        prometheus::IntGauge::new("rrules_sessions_active_batch", "test").unwrap(),
    )))
}

/// Initialize the test service with the full Plan 23-05 + Plan 24-03 +
/// Plan 27-01 app_data stack (registry + selector + shared ruleset +
/// counters + metrics_tx + session_manager). Mirrors `execute_integration.rs`.
/// Inlined per-test via macro because `test::init_service` yields an
/// unnameable `impl Service` type.
macro_rules! init_test_app {
    () => {
        test::init_service(
            App::new()
                .app_data(shared_registry())
                .app_data(shared_selector())
                .app_data(shared_ruleset())
                .app_data(shared_counters())
                .app_data(shared_metrics_tx())
                .app_data(shared_session_manager())
                .configure(configure_test_routes),
        )
        .await
    };
}

#[actix_web::test]
async fn batch_execute_runs_two_requests_successfully() {
    let app = init_test_app!();

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
    // empty ruleset → no rules fire, so the batch total is 0.
    assert_eq!(body["total_fired"], 0);
    let results = body["results"].as_array().expect("results is an array");
    assert_eq!(results.len(), 2);
    for r in results {
        assert_eq!(r["success"], true, "each result successful: {r}");
        assert!(r["response"]["rules_fired"].is_number(), "response present");
        assert!(
            r["response"]["execution_id"].is_string(),
            "execution_id present"
        );
    }
}

#[actix_web::test]
async fn batch_execute_empty_requests_yields_empty_results() {
    let app = init_test_app!();

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
    let app = init_test_app!();

    let requests: Vec<Value> = (0..10)
        .map(|i| json!({ "facts": { "customer": { "tier": format!("T{i}") } } }))
        .collect();
    let req = test::TestRequest::post()
        .uri("/v1/rules/batch-execute")
        .set_json(json!({ "requests": requests, "max_concurrency": 2 }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert_eq!(
        body["succeeded"], 10,
        "all requests should complete; body: {body}"
    );
    assert_eq!(body["failed"], 0);
    assert_eq!(body["results"].as_array().unwrap().len(), 10);
}

#[actix_web::test]
async fn batch_execute_static_path_beats_wildcard() {
    // Registering both the static path and the {id} wildcard: POSTing the
    // static path must reach the batch handler, not the wildcard marker.
    let app = init_test_app!();

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
    assert_ne!(
        body["hit"], "wildcard",
        "wildcard must not shadow static path"
    );
}

#[actix_web::test]
async fn single_execute_still_works_after_refactor() {
    // Confirms the execute_rules → execute_single refactor didn't break the
    // single-request path.
    let app = init_test_app!();

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "customer": { "tier": "Gold" } } }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert_eq!(body["rules_fired"], 0, "empty ruleset — no rules fire");
    assert!(body["execution_id"].is_string(), "execution_id present");
    assert_eq!(body["rules_not_fired"].as_array().unwrap().len(), 0);
}
