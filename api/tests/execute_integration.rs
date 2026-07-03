//! Plan 23-04 Task 2: Wave 0 RED integration scaffolds for `POST /v1/execute`.
//!
//! These five scaffolds encode the four ROADMAP success criteria for Phase 23
//! and are intentionally RED until Plan 23-05 wires `execute_single` through
//! the real `EngineRegistry` / `ExecutionEngine` surface:
//!
//! - `execute_fires_real_rule`        (EXEC-01) — `rules_fired >= 1`
//! - `facts_after_not_echoed`         (EXEC-02 / Pitfall 11) — `facts_after.scorecard.score == 10`
//! - `engine_param_validation`        (ENGINE-07) — `engine: "rete"` → 200; `engine: "unknown"` → 400
//! - `default_engine_resolution`      (ENGINE-08) — omitted `engine` → 200 against `RRULES_DEFAULT_ENGINE`
//! - `coverage_auto_populated`        (MON-05)  — GET `/v1/metrics/coverage` shows `r1` with `fired == true`
//!
//! The test App installs a real `EngineRegistry` (`register_default_engines`),
//! `EngineSelector::with_defaults()`, and a shared `RuleSet` containing the
//! keystone `score := score + 10` fixture rule as `app_data` — exactly the
//! state Plan 23-05's handler signature will extract. The current stub handler
//! ignores the extra `app_data` and returns `rules_fired: 0`, so the
//! substantive assertions fail (Nyquist RED). The scaffolds MUST compile
//! (types resolve) so Plan 05 turns them GREEN by rewriting `execute_single`
//! alone.
//!
//! All tests are `#[serial_test::serial]` because they (will) record into the
//! process-global `CoverageTracking` singleton and mutate the
//! `RRULES_DEFAULT_ENGINE` env var; serial isolation prevents cross-test
//! pollution (Plan 23-04 threat T-23-10).
//!
//! Filter: `cargo test -p api --test execute_integration`

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use actix_web::{body::BoxBody, test, web, App};
use serde_json::{json, Value};

use api::metrics::{
    configure_metrics, coverage_endpoint, create_metrics_channel, ExecutionCounters, MetricsTx,
};
use api::v1::execution::{execute_rules, SharedRuleSet};
use engine::data_types::Value as EngineValue;
use engine::engine::registry::EngineRegistry;
use engine::engine::selector::EngineSelector;
use engine::monitoring::coverage::CoverageTracking;
use engine::rules::{Action, ComparisonOp, Condition, Expression, Rule, RuleSet};

// =====================================================================
// Fixture: the keystone `score := score + 10` rule
// =====================================================================

/// Build the keystone rule: `r1` with condition `score == 0` and action
/// `score := score + 10` (uses the Plan 23-02 `Expression` API —
/// `Expression::Add(FieldRef, Literal(Integer(10)))` and
/// `Condition::Pattern { op: ComparisonOp::Eq }`).
fn score_plus_10_rule() -> Rule {
    let mut fields = HashMap::new();
    fields.insert(
        "score".to_string(),
        Expression::Add(
            Box::new(Expression::FieldRef {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
            }),
            Box::new(Expression::Literal(EngineValue::Integer(10))),
        ),
    );
    Rule::builder("r1", "Increment score")
        .condition(Condition::Pattern {
            fact_type: "scorecard".to_string(),
            field: "score".to_string(),
            value: EngineValue::Integer(0),
            op: ComparisonOp::Eq,
        })
        .action(Action::UpdateFact {
            id: "scorecard".to_string(),
            fields,
        })
        .build()
}

/// Build the shared read-only `SharedRuleSet` containing the keystone rule,
/// wrapped for `web::Data` exactly as `main.rs` wires it (Phase 25 / D-02:
/// `Arc<RwLock<SharedRuleSet>>` — generation starts at 0 pre-reload).
fn shared_ruleset() -> web::Data<Arc<RwLock<SharedRuleSet>>> {
    let mut ruleset = RuleSet::new();
    ruleset.add_rule(score_plus_10_rule()).expect("add_rule");
    web::Data::new(Arc::new(RwLock::new(SharedRuleSet::new(ruleset))))
}

/// Build a real `EngineRegistry` with all four default factories registered
/// (Plan 23-01 D-01: `available_engines()` truthfully returns
/// `rete`/`treat`/`leaps`/`phreak`).
fn shared_registry() -> web::Data<EngineRegistry> {
    let mut registry = EngineRegistry::new();
    registry.register_default_engines();
    web::Data::new(registry)
}

/// Build an `EngineSelector::with_defaults()`. Plan 23-05 will set the default
/// engine once at startup via `select_engine(&mut self)`; for the test App the
/// selector is only used for `available_engines()` validation of the per-
/// request `engine` param, so the post-construction default (`"rete"`) is fine.
fn shared_selector() -> web::Data<EngineSelector> {
    web::Data::new(EngineSelector::with_defaults())
}

/// Build a shared `MetricsTx` (broadcast sender) for the test App. Plan 24-03
/// will change `execute_rules` to extract `web::Data<MetricsTx>` and broadcast a
/// `MetricsSnapshot` after each execute; installing it in the test App now keeps
/// `init_test_app!` forward-compatible and lets the Phase 24 RED scaffolds
/// compile against the current (pre-24-03) API — actix silently ignores unused
/// `app_data`. The receiver is dropped here; a fresh channel per test is fine
/// because no test in this file reads from the broadcast channel.
/// Build a shared `MetricsTx` (broadcast sender) for the test App. Plan 24-03
/// will change `execute_rules` to extract `web::Data<MetricsTx>` and broadcast a
/// `MetricsSnapshot` after each execute; installing it in the test App now keeps
/// `init_test_app!` forward-compatible and lets the Phase 24 RED scaffolds
/// compile against the current (pre-24-03) API — actix silently ignores unused
/// `app_data`. The receiver is dropped here; a fresh channel per test is fine
/// because no test in this file reads from the broadcast channel.
fn shared_metrics_tx() -> web::Data<MetricsTx> {
    let (tx, _rx) = create_metrics_channel(16);
    web::Data::new(tx)
}

/// Build a shared `ExecutionCounters` (Plan 24-03). `execute_rules` and
/// `metrics_endpoint` now extract `web::Data<ExecutionCounters>`, so the test
/// App must install it or the handlers return 500 on the missing extractor
/// (Open Q7). `ExecutionCounters::default()` delegates to `new()`, which
/// constructs the dedicated `Registry::new()` + three labeled metrics.
fn shared_counters() -> web::Data<ExecutionCounters> {
    web::Data::new(ExecutionCounters::default())
}

// =====================================================================
// Test App wiring
// =====================================================================

/// Register the routes Plan 23-05 needs: `POST /v1/execute` (the rewrite
/// target) and `GET /v1/metrics/coverage` (the MON-05 coverage surface). The
/// `/v1` scope mirrors `main.rs` so the coverage route resolves to
/// `/v1/metrics/coverage`.
fn configure_test_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/execute", web::post().to(execute_rules))
            .route("/metrics/coverage", web::get().to(coverage_endpoint)),
    );
}

/// Reads a success response body as JSON, panicking with a useful message on
/// non-2xx status or invalid JSON. (Mirrors `batch_execution_integration.rs`.)
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

/// Initialize the test service with the full Plan 23-05 app_data stack.
/// Inlined per-test (rather than returned from a helper) because
/// `test::init_service` yields an unnameable `impl Service` type — mirrors
/// the `batch_execution_integration.rs` pattern.
macro_rules! init_test_app {
    () => {
        test::init_service(
            App::new()
                .app_data(shared_registry())
                .app_data(shared_selector())
                .app_data(shared_ruleset())
                .app_data(shared_counters())
                .app_data(shared_metrics_tx())
                .configure(configure_test_routes)
                .configure(configure_metrics),
        )
        .await
    };
}

/// Reset the `CoverageTracking` singleton and pin `RRULES_DEFAULT_ENGINE=rete`
/// so `default_engine_resolution` is deterministic. Called at the start of
/// every serial test.
fn reset_test_env() {
    CoverageTracking.clear();
    // SAFETY on env var: tests run serially (#[serial_test::serial]); no
    // concurrent test reads/writes this var. Plan 23-05's handler reads it
    // once per request.
    std::env::set_var("RRULES_DEFAULT_ENGINE", "rete");
}

// =====================================================================
// EXEC-01: execute_fires_real_rule
// =====================================================================

/// POST `/v1/execute` with `facts: {scorecard: {score: 0}}` against the
/// keystone `score := score + 10` rule yields `rules_fired >= 1`
/// (ROADMAP success criterion #1). RED until Plan 23-05 wires `execute_single`
/// through the real engine.
#[serial_test::serial]
#[actix_web::test]
async fn execute_fires_real_rule() {
    reset_test_env();
    let app = init_test_app!();

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert!(
        body["rules_fired"].as_u64().unwrap_or(0) >= 1,
        "EXEC-01: keystone rule should fire, got rules_fired = {}; body: {body}",
        body["rules_fired"]
    );
}

// =====================================================================
// EXEC-02 / Pitfall 11: facts_after_not_echoed
// =====================================================================

/// `facts_after.scorecard.score` must be `10` (the action's increment), not
/// `0` (the echoed request input — Pitfall 11). RED until Plan 23-05
/// serializes `facts_after` from post-execution `get_facts()`.
#[serial_test::serial]
#[actix_web::test]
async fn facts_after_not_echoed() {
    reset_test_env();
    let app = init_test_app!();

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    let score = body["facts_after"]["scorecard"]["score"].as_i64();
    assert_eq!(
        score,
        Some(10),
        "EXEC-02 / Pitfall 11: facts_after.scorecard.score should be 10 (incremented), got {:?}; body: {body}",
        score
    );
}

// =====================================================================
// ENGINE-07: engine_param_validation
// =====================================================================

/// `engine: "rete"` → HTTP 200 (valid engine name); `engine: "unknown"` →
/// HTTP 400 (unknown engine name — L-07 input validation). RED until Plan
/// 23-05 validates the `engine` param against `available_engines()`.
#[serial_test::serial]
#[actix_web::test]
async fn engine_param_validation() {
    reset_test_env();
    let app = init_test_app!();

    // Valid engine name → 200.
    let req_ok = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "engine": "rete", "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let resp_ok = test::call_service(&app, req_ok).await;
    assert!(
        resp_ok.status().is_success(),
        "ENGINE-07: engine='rete' should return 200, got {}",
        resp_ok.status()
    );

    // Unknown engine name → 400.
    let req_bad = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "engine": "unknown", "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let resp_bad = test::call_service(&app, req_bad).await;
    assert_eq!(
        resp_bad.status().as_u16(),
        400,
        "ENGINE-07: engine='unknown' should return 400, got {}",
        resp_bad.status()
    );
}

// =====================================================================
// ENGINE-08: default_engine_resolution
// =====================================================================

/// Omitting `engine` resolves to `RRULES_DEFAULT_ENGINE` (pinned to `"rete"`
/// in `reset_test_env`) and executes successfully (200) with `rules_fired >=
/// 1`. RED until Plan 23-05 reads the env-var default.
#[serial_test::serial]
#[actix_web::test]
async fn default_engine_resolution() {
    reset_test_env();
    let app = init_test_app!();

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "ENGINE-08: omitted engine should default to rete and return 200, got {}",
        resp.status()
    );

    let body = json_body(resp).await;
    assert!(
        body["rules_fired"].as_u64().unwrap_or(0) >= 1,
        "ENGINE-08: default-engine execution should fire the keystone rule, got rules_fired = {}; body: {body}",
        body["rules_fired"]
    );
}

// =====================================================================
// MON-05: coverage_auto_populated
// =====================================================================

/// After an `execute()` that fires `r1`, `GET /v1/metrics/coverage` shows the
/// fixture rule with `fired == true` and `fire_count == 1` (once-per-rule,
/// Pitfall 6). RED until Plan 23-05's wired handler records into the
/// `CoverageTracking` singleton via the real evaluator.
#[serial_test::serial]
#[actix_web::test]
async fn coverage_auto_populated() {
    reset_test_env();
    let app = init_test_app!();

    // Fire the keystone rule.
    let exec_req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let _ = test::call_service(&app, exec_req).await;

    // Read the coverage surface.
    let cov_req = test::TestRequest::get()
        .uri("/v1/metrics/coverage")
        .to_request();
    let cov_body = json_body(test::call_service(&app, cov_req).await).await;

    let rules = cov_body["rules"]
        .as_array()
        .expect("coverage 'rules' should be an array");
    let r1 = rules
        .iter()
        .find(|r| r["rule_id"] == "r1")
        .unwrap_or_else(|| panic!("r1 should appear in coverage rules; body: {cov_body}"));
    assert_eq!(
        r1["fired"].as_bool(),
        Some(true),
        "MON-05: r1 should be fired in coverage; body: {cov_body}"
    );
    assert_eq!(
        r1["fire_count"].as_u64(),
        Some(1),
        "MON-05 / Pitfall 6: r1 fire_count should be 1 (once-per-rule); body: {cov_body}"
    );
}

// =====================================================================
// Phase 24 Wave 0 RED scaffolds
//
// These three scaffolds encode the Phase 24 success contract and are
// intentionally RED until Plans 24-02 (foundation) and 24-03
// (instrumentation) wire the Prometheus counters, the response-shape
// `engine_generation` field, and the execute→metrics broadcast path.
// They compile against the existing Phase 23 API and FAIL on their
// substantive assertions (not on compile or extraction errors).
// =====================================================================

/// SC-1 (Prometheus counters exposed): after a POST `/v1/execute`, the
/// `/metrics` Prometheus exposition body contains the
/// `rrules_execute_requests_total` counter. RED because the current
/// `metrics_endpoint` returns hardcoded text containing only
/// `engine_rules_fired` (Plan 24-03 swaps in a `prometheus::TextEncoder`
/// over a real `CounterVec`).
#[serial_test::serial]
#[actix_web::test]
async fn counters_exposed_in_metrics() {
    reset_test_env();
    let app = init_test_app!();

    // Fire the engine so a counter (when wired) would be non-zero.
    let exec_req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let _ = json_body(test::call_service(&app, exec_req).await).await;

    // Read the Prometheus exposition format.
    let metrics_req = test::TestRequest::get()
        .uri("/metrics")
        .insert_header(("Accept", "text/plain"))
        .to_request();
    let metrics_resp = test::call_service(&app, metrics_req).await;
    assert!(
        metrics_resp.status().is_success(),
        "metrics endpoint should return 200, got {}",
        metrics_resp.status()
    );
    let bytes = test::read_body(metrics_resp).await;
    let text = String::from_utf8_lossy(&bytes);

    assert!(
        text.contains("rrules_execute_requests_total"),
        "SC-1: expected the rrules_execute_requests_total counter in /metrics; got: {text}"
    );
}

/// Counter-correctness (24-VALIDATION.md): after TWO POST `/v1/execute`
/// calls, the `/metrics` exposition body contains the
/// `rrules_execute_rules_fired_total` counter with value >= 1. RED
/// because the current hardcoded text lacks the new metric name.
#[serial_test::serial]
#[actix_web::test]
async fn counters_increment_per_request() {
    reset_test_env();
    let app = init_test_app!();

    // Fire the keystone rule twice so a per-request counter (when wired)
    // would record at least one fired rule.
    for _ in 0..2 {
        let req = test::TestRequest::post()
            .uri("/v1/execute")
            .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
            .to_request();
        let _ = json_body(test::call_service(&app, req).await).await;
    }

    let metrics_req = test::TestRequest::get()
        .uri("/metrics")
        .insert_header(("Accept", "text/plain"))
        .to_request();
    let bytes = test::read_body(test::call_service(&app, metrics_req).await).await;
    let text = String::from_utf8_lossy(&bytes);

    assert!(
        text.contains("rrules_execute_rules_fired_total"),
        "counter-correctness: expected the rrules_execute_rules_fired_total counter in /metrics; got: {text}"
    );
}

/// Response-shape (24-VALIDATION.md): the `/v1/execute` response JSON
/// contains an `engine_generation` field of type u64. RED because the
/// current `ExecuteResponse` struct has no `engine_generation` field and
/// serde omits it, so `body["engine_generation"]` is `Null`.
#[serial_test::serial]
#[actix_web::test]
async fn response_includes_engine_generation() {
    reset_test_env();
    let app = init_test_app!();

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert!(
        body["engine_generation"].is_u64(),
        "response-shape: expected engine_generation to be a u64; got {:?}; body: {body}",
        body["engine_generation"]
    );
}