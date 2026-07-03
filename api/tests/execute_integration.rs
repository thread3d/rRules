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
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use actix_web::{body::BoxBody, test, web, App};
use serde_json::{json, Value};

use api::metrics::{
    configure_metrics, coverage_endpoint, create_metrics_channel, ExecutionCounters, MetricsTx,
};
use api::v1::execution::{
    batch_execute_rules, execute_rules, get_job_status, init_job_store, list_jobs, reload_rules,
    EngineState, JobStore, SharedRuleSet,
};
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

/// Build the edited keystone rule: `r1` with condition `score == 0` and action
/// `score := score + 20` — the +20 variant used by the SC-1 keystone scenario
/// (edit +10 → +20, reload, execute sees the new delta). Mirrors
/// `score_plus_10_rule()` with `Literal(Integer(20))`.
fn score_plus_20_rule() -> Rule {
    let mut fields = HashMap::new();
    fields.insert(
        "score".to_string(),
        Expression::Add(
            Box::new(Expression::FieldRef {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
            }),
            Box::new(Expression::Literal(EngineValue::Integer(20))),
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

/// Serialize `rule` via `serde_json::to_string_pretty` and write it to
/// `{dir}/{name}.rules`, returning the path. The `Rule` AST derives
/// `Serialize` (`engine/src/rules.rs:15`), so this is a one-liner. Used by
/// the SC-1 keystone test to write r1 (+10) and then overwrite it with +20
/// before calling `POST /v1/rules/reload`.
fn write_rule_file(dir: &std::path::Path, name: &str, rule: &Rule) -> PathBuf {
    let path = dir.join(format!("{name}.rules"));
    let json = serde_json::to_string_pretty(rule)
        .expect("serialize rule to JSON");
    std::fs::write(&path, json)
        .unwrap_or_else(|e| panic!("write_rule_file: failed to write {}: {e}", path.display()));
    path
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

/// Build a shared `JobStore` (Plan 25-01 Task 2). `reload_rules` (post-25-03
/// rewrite) and `get_job_status` / `list_jobs` extract `web::Data<JobStore>`,
/// so the test App must install it. The current `reload_rules` handler still
/// extracts `web::Data<EngineState>` (pre-25-03); installing both keeps the
/// test App forward-compatible so the RED reload tests fail on substantive
/// assertions (job 404 / no swap / gen 0) rather than on a 500 missing-
/// extractor error.
fn shared_job_store() -> web::Data<JobStore> {
    web::Data::new(init_job_store())
}

/// Build a shared `EngineState` for the current (pre-25-03) `reload_rules`
/// handler which extracts `web::Data<EngineState>`. Installing it lets the
/// reload endpoint return 202/200 so the RED reload tests fail on the
/// substantive assertion (no swap, no generation bump, job 404) rather than
/// on a 500 missing-extractor error. Harmless for pre-existing tests (actix
/// silently ignores unused `app_data`).
fn shared_engine_state() -> web::Data<EngineState> {
    web::Data::new(EngineState::new())
}

// =====================================================================
// Test App wiring
// =====================================================================

/// Register the routes Plan 23-05 + Plan 25-01 need: `POST /v1/execute`,
/// `GET /v1/metrics/coverage`, `POST /v1/rules/reload`, `POST /v1/rules/
/// batch-execute`, `GET /v1/jobs/{job_id}`, `GET /v1/jobs`. The `/v1` scope
/// mirrors `main.rs` so routes resolve to their canonical `/v1/...` paths.
/// Route ordering mirrors `main.rs`: `/rules/batch-execute` and `/rules/reload`
/// are registered before any `/rules/{id}` wildcard so the literal paths win.
fn configure_test_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/execute", web::post().to(execute_rules))
            .route("/metrics/coverage", web::get().to(coverage_endpoint))
            // Phase 25 (Plan 25-01 Task 2): reload + batch + jobs routes.
            .route("/rules/batch-execute", web::post().to(batch_execute_rules))
            .route("/rules/reload", web::post().to(reload_rules))
            .route("/jobs/{job_id}", web::get().to(get_job_status))
            .route("/jobs", web::get().to(list_jobs)),
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
                .app_data(shared_job_store())
                .app_data(shared_engine_state())
                .configure(configure_test_routes)
                .configure(configure_metrics),
        )
        .await
    };
}

/// Reset the `CoverageTracking` singleton and pin `RRULES_DEFAULT_ENGINE=rete`
/// so `default_engine_resolution` is deterministic. Called at the start of
/// every serial test. Also resets `RULE_STORAGE_PATH` to the default so
/// reload tests that mutate it don't leak into siblings.
fn reset_test_env() {
    CoverageTracking.clear();
    // SAFETY on env vars: tests run serially (#[serial_test::serial]); no
    // concurrent test reads/writes these vars. Plan 23-05's handler reads
    // RRULES_DEFAULT_ENGINE once per request; Plan 25's reload handler reads
    // RULE_STORAGE_PATH once per reload.
    std::env::set_var("RRULES_DEFAULT_ENGINE", "rete");
    std::env::set_var("RULE_STORAGE_PATH", "./rules");
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

// =====================================================================
// Phase 25 Wave 0 RED scaffolds (Plan 25-01 Task 2)
//
// These eight scaffolds encode the Phase 25 success criteria (SC-1 reload-
// then-execute-against-refreshed, SC-2 batch truthful aggregation, SC-3
// generation increment) and supporting edge cases (async job transitions,
// in-flight snapshot isolation, panicking-item isolation, empty-dir
// graceful handling, sync reload swap). They compile against the final
// SharedRuleSet shape (Plan 25-01 Task 1) and FAIL on their substantive
// assertions (RED) because:
//   - reload_rules is still a no-op (EngineState.reload_engine acquires-
//     and-drops the write lock, never recompiles/swaps — Pitfall 12 / L-02)
//   - the async spawned task never writes to the JobStore (Pitfall 14 /
//     L-03) → GET /v1/jobs/{id} returns 404
//   - generation never bumps (L-04 — reload_and_swap lands in Plan 25-02)
//   - batch_execute_rules masks execute_single Err as a zero-fired success
//     (D-03 — the truthful Err→failed aggregation lands in Plan 25-03)
//
// Plans 25-02 (real .rules parser + reload_and_swap) and 25-03 (handler
// wiring + batch aggregation) turn these GREEN. Mirrors the Phase 23-04 /
// 24-01 Wave 0 RED→GREEN + Nyquist pattern.
// =====================================================================

/// Create a unique temp directory for a reload test and return its path.
/// Uses `uuid::Uuid::new_v4()` so serial tests don't collide. The caller is
/// responsible for cleanup (best-effort `fs::remove_dir_all` on drop).
fn create_test_rules_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rrules-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir)
        .unwrap_or_else(|e| panic!("create_test_rules_dir: failed to create {}: {e}", dir.display()));
    dir
}

// ---------------------------------------------------------------------
// SC-1 / DEPLOY-05: reload_then_execute_against_refreshed_rules
// ---------------------------------------------------------------------

/// **SC-1 keystone scenario:** write r1 (+10) to a temp rules dir, execute
/// sees `score == 10` / `gen == 0`; overwrite r1 (+20); POST
/// `/v1/rules/reload {async:true}` returns 202 with `job_id`; poll
/// `GET /v1/jobs/{job_id}` reaches `completed`; the next execute sees
/// `score == 20` / `gen == 1` (the edited rule + bumped generation).
///
/// RED today because `reload_rules` is a no-op (L-02: `reload_engine`
/// acquires-and-drops the write lock, never recompiles/swaps), the spawned
/// task never writes to the JobStore (L-03 / Pitfall 14 → job 404), and
/// generation never bumps (L-04 — `reload_and_swap` lands in Plan 25-02).
/// Plan 25-02 + 25-03 turn this GREEN.
#[serial_test::serial]
#[actix_web::test]
async fn reload_then_execute_against_refreshed_rules() {
    reset_test_env();
    let dir = create_test_rules_dir();
    std::env::set_var("RULE_STORAGE_PATH", dir.to_str().unwrap());

    // Write r1 (+10) to the rules dir.
    write_rule_file(&dir, "r1", &score_plus_10_rule());

    let app = init_test_app!();

    // 1. Execute against the +10 rule → score 10, gen 0.
    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;
    let score_before = body["facts_after"]["scorecard"]["score"].as_i64();
    let gen_before = body["engine_generation"].as_u64();
    assert_eq!(
        score_before,
        Some(10),
        "SC-1: pre-reload execute should see score 10 (+10 rule); got {:?}; body: {body}",
        score_before
    );
    assert_eq!(
        gen_before,
        Some(0),
        "SC-1 / L-04: pre-reload generation should be 0; got {:?}; body: {body}",
        gen_before
    );

    // 2. Overwrite r1 with the +20 rule.
    write_rule_file(&dir, "r1", &score_plus_20_rule());

    // 3. POST /v1/rules/reload {async:true} → 202 with job_id.
    let reload_req = test::TestRequest::post()
        .uri("/v1/rules/reload")
        .set_json(json!({ "async_reload": true }))
        .to_request();
    let reload_resp = test::call_service(&app, reload_req).await;
    assert_eq!(
        reload_resp.status().as_u16(),
        202,
        "SC-1: async reload should return 202, got {}",
        reload_resp.status()
    );
    let reload_body: Value = json_body(reload_resp).await;
    let job_id = reload_body["job_id"]
        .as_str()
        .expect("SC-1: 202 response should contain job_id")
        .to_string();

    // 4. Poll GET /v1/jobs/{job_id} until completed (or timeout).
    let mut observed_statuses: Vec<String> = Vec::new();
    for _ in 0..50 {
        let poll_req = test::TestRequest::get()
            .uri(&format!("/v1/jobs/{job_id}"))
            .to_request();
        let poll_resp = test::call_service(&app, poll_req).await;
        if poll_resp.status() == actix_web::http::StatusCode::NOT_FOUND {
            observed_statuses.push("not_found".to_string());
        } else if poll_resp.status().is_success() {
            let poll_body = json_body(poll_resp).await;
            if let Some(status) = poll_body["status"].as_str() {
                observed_statuses.push(status.to_string());
                if status == "completed" {
                    break;
                }
            }
        } else {
            observed_statuses.push(format!("status_{}", poll_resp.status().as_u16()));
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        observed_statuses.iter().any(|s| s == "completed"),
        "SC-1: expected job to reach 'completed'; observed statuses: {:?}",
        observed_statuses
    );

    // 5. Execute again → should see score 20 (edited rule) and gen > 0.
    let req2 = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let body2 = json_body(test::call_service(&app, req2).await).await;
    let score_after = body2["facts_after"]["scorecard"]["score"].as_i64();
    let gen_after = body2["engine_generation"].as_u64();
    assert_eq!(
        score_after,
        Some(20),
        "SC-1: post-reload execute should see score 20 (+20 rule); got {:?}; body: {body2}",
        score_after
    );
    assert!(
        gen_after.unwrap_or(0) > 0,
        "SC-1 / L-04: post-reload generation should be > 0; got {:?}; body: {body2}",
        gen_after
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// SC-1 / DEPLOY-05: async_reload_job_transitions
// ---------------------------------------------------------------------

/// POST `/v1/rules/reload {async:true}` returns 202; polling
/// `GET /v1/jobs/{job_id}` observes `pending` or `running` followed by
/// `completed` (L-03: the spawned reload task writes transitions into the
/// JobStore). RED today because the spawned task calls
/// `engine_state.reload_engine` (a no-op) and never writes to the
/// `JobStore` — `GET /v1/jobs/{id}` returns 404 for every poll
/// (Pitfall 14). Plan 25-03 wires the spawned task to write
/// `Pending → Running → Completed` transitions.
#[serial_test::serial]
#[actix_web::test]
async fn async_reload_job_transitions() {
    reset_test_env();
    let dir = create_test_rules_dir();
    std::env::set_var("RULE_STORAGE_PATH", dir.to_str().unwrap());
    write_rule_file(&dir, "r1", &score_plus_10_rule());

    let app = init_test_app!();

    let reload_req = test::TestRequest::post()
        .uri("/v1/rules/reload")
        .set_json(json!({ "async_reload": true }))
        .to_request();
    let reload_resp = test::call_service(&app, reload_req).await;
    assert_eq!(
        reload_resp.status().as_u16(),
        202,
        "SC-1: async reload should return 202, got {}",
        reload_resp.status()
    );
    let reload_body: Value = json_body(reload_resp).await;
    let job_id = reload_body["job_id"]
        .as_str()
        .expect("SC-1: 202 response should contain job_id")
        .to_string();

    // Poll for transitions. RED: all polls return 404 (not_found) because
    // the spawned task never writes to the JobStore.
    let mut statuses: Vec<String> = Vec::new();
    for _ in 0..50 {
        let poll_req = test::TestRequest::get()
            .uri(&format!("/v1/jobs/{job_id}"))
            .to_request();
        let poll_resp = test::call_service(&app, poll_req).await;
        if poll_resp.status() == actix_web::http::StatusCode::NOT_FOUND {
            statuses.push("not_found".to_string());
        } else if poll_resp.status().is_success() {
            let poll_body = json_body(poll_resp).await;
            if let Some(status) = poll_body["status"].as_str() {
                statuses.push(status.to_string());
                if status == "completed" {
                    break;
                }
            }
        } else {
            statuses.push(format!("status_{}", poll_resp.status().as_u16()));
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        statuses.iter().any(|s| s == "pending" || s == "running"),
        "SC-1: expected to observe 'pending' or 'running' transition; got {:?}",
        statuses
    );
    assert!(
        statuses.iter().any(|s| s == "completed"),
        "SC-1: expected to observe 'completed' transition; got {:?}",
        statuses
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// SC-3 / L-04: engine_generation_increments_after_reload
// ---------------------------------------------------------------------

/// Execute returns `engine_generation == 0`; after a reload, a subsequent
/// execute returns `engine_generation > 0` (L-04: generation is stamped
/// from the shared snapshot and incremented on each successful reload).
/// RED today because `reload_rules` never swaps the `SharedRuleSet`, so
/// `generation` stays 0. Plan 25-02's `reload_and_swap` increments it
/// under the write lock.
#[serial_test::serial]
#[actix_web::test]
async fn engine_generation_increments_after_reload() {
    reset_test_env();
    let dir = create_test_rules_dir();
    std::env::set_var("RULE_STORAGE_PATH", dir.to_str().unwrap());
    write_rule_file(&dir, "r1", &score_plus_10_rule());

    let app = init_test_app!();

    // Pre-reload execute → gen 0.
    let req1 = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let body1 = json_body(test::call_service(&app, req1).await).await;
    let gen_before = body1["engine_generation"].as_u64();
    assert_eq!(
        gen_before,
        Some(0),
        "SC-3 / L-04: pre-reload generation should be 0; got {:?}; body: {body1}",
        gen_before
    );

    // Sync reload.
    let reload_req = test::TestRequest::post()
        .uri("/v1/rules/reload")
        .set_json(json!({ "async_reload": false }))
        .to_request();
    let reload_resp = test::call_service(&app, reload_req).await;
    assert!(
        reload_resp.status().is_success(),
        "SC-3: sync reload should return 200, got {}",
        reload_resp.status()
    );

    // Post-reload execute → gen > 0. RED: gen stays 0.
    let req2 = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let body2 = json_body(test::call_service(&app, req2).await).await;
    let gen_after = body2["engine_generation"].as_u64();
    assert!(
        gen_after.unwrap_or(0) > 0,
        "SC-3 / L-04: post-reload generation should be > 0; got {:?}; body: {body2}",
        gen_after
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// SC-2 / EXEC-03: batch_truthful_aggregation_one_failing_item
// ---------------------------------------------------------------------

/// POST `/v1/rules/batch-execute` with 2 items — one valid (`engine`
/// omitted → default rete), one with `engine: "unknown"` (→ `execute_single`
/// returns `Err` / HTTP 400). The truthful aggregate (D-03 / SC-2) is
/// `succeeded == 1`, `failed == 1`, the failing item has `success: false`
/// and `error: Some(...)`, and `total_fired >= 1` (only the successful
/// item's fired rules count). RED today because `batch_execute_rules`
/// converts `execute_single`'s `Err` into a placeholder
/// `ExecuteResponse { rules_fired: 0, … }` and counts it as `succeeded`
/// with `success: true` (`execution.rs:618-628`) → `succeeded == 2`,
/// `failed == 0`. Plan 25-03 Task 2 changes the JoinSet task to return
/// `Result<ExecuteResponse, String>` and the collector to branch
/// `Err → failed / success:false / error:Some` (D-03).
#[serial_test::serial]
#[actix_web::test]
async fn batch_truthful_aggregation_one_failing_item() {
    reset_test_env();
    let app = init_test_app!();

    let batch_req = test::TestRequest::post()
        .uri("/v1/rules/batch-execute")
        .set_json(json!({
            "requests": [
                { "facts": { "scorecard": { "score": 0 } } },
                { "facts": { "scorecard": { "score": 0 } }, "engine": "unknown" }
            ]
        }))
        .to_request();
    let body = json_body(test::call_service(&app, batch_req).await).await;

    let succeeded = body["succeeded"].as_u64().unwrap_or(0);
    let failed = body["failed"].as_u64().unwrap_or(0);
    let total_fired = body["total_fired"].as_u64().unwrap_or(0);
    let results = body["results"]
        .as_array()
        .expect("SC-2: batch response should contain 'results' array");

    assert_eq!(
        succeeded, 1,
        "SC-2 / EXEC-03: succeeded should be 1 (one valid item); got {succeeded}; body: {body}"
    );
    assert_eq!(
        failed, 1,
        "SC-2 / EXEC-03: failed should be 1 (one unknown-engine item); got {failed}; body: {body}"
    );
    assert!(
        total_fired >= 1,
        "SC-2 / EXEC-03: total_fired should reflect the successful item's fired rules; got {total_fired}; body: {body}"
    );

    // The failing item should have success:false + error:Some. JoinSet
    // returns tasks in completion order (NOT submission order), so the
    // unknown-engine item — which returns Err instantly without engine
    // construction or web::block — typically completes before the valid
    // item. Find the failing/successful items by their `success` flag
    // rather than assuming a fixed index (Rule 1 fix: the original
    // `results[1]` assumption broke under completion-order delivery).
    let failing = results
        .iter()
        .find(|r| r["success"].as_bool() == Some(false))
        .expect("SC-2 / D-03: expected one failing item in results; body: {body}");
    assert_eq!(
        failing["success"].as_bool(),
        Some(false),
        "SC-2 / D-03: failing item should have success:false; body: {body}"
    );
    assert!(
        failing["error"].is_string(),
        "SC-2 / D-03: failing item should have error:Some(...); body: {body}"
    );

    let ok_item = results
        .iter()
        .find(|r| r["success"].as_bool() == Some(true))
        .expect("SC-2: expected one successful item in results; body: {body}");
    assert_eq!(
        ok_item["success"].as_bool(),
        Some(true),
        "SC-2: successful item should have success:true; body: {body}"
    );
}

// ---------------------------------------------------------------------
// L-05 / D-01: in_flight_run_uses_old_snapshot
// ---------------------------------------------------------------------

/// A run started before a reload completes against the OLD snapshot
/// (generation-accept, D-01: swap immediately, tag with generation; in-
/// flight runs finish against their already-cloned snapshot — L-05).
///
/// Wave 0 simplification: this scaffold verifies the observable contract —
/// a pre-reload execute sees `gen == 0` / `score == 10`, and a post-reload
/// execute sees `gen > 0` / `score == 20` (the new rule). True concurrent
/// in-flight isolation (a run that spans the swap window) is verified in
/// Plan 25-03's handler-level tests. RED today because reload doesn't swap
/// — the post-reload execute still sees `score == 10` / `gen == 0`.
#[serial_test::serial]
#[actix_web::test]
async fn in_flight_run_uses_old_snapshot() {
    reset_test_env();
    let dir = create_test_rules_dir();
    std::env::set_var("RULE_STORAGE_PATH", dir.to_str().unwrap());
    write_rule_file(&dir, "r1", &score_plus_10_rule());

    let app = init_test_app!();

    // Pre-reload run → old snapshot: score 10, gen 0.
    let req1 = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let body1 = json_body(test::call_service(&app, req1).await).await;
    assert_eq!(
        body1["facts_after"]["scorecard"]["score"].as_i64(),
        Some(10),
        "L-05 / D-01: pre-reload run should see old snapshot (score 10); body: {body1}"
    );
    assert_eq!(
        body1["engine_generation"].as_u64(),
        Some(0),
        "L-05 / D-01: pre-reload run should see gen 0; body: {body1}"
    );

    // Overwrite r1 (+20) and reload.
    write_rule_file(&dir, "r1", &score_plus_20_rule());
    let reload_req = test::TestRequest::post()
        .uri("/v1/rules/reload")
        .set_json(json!({ "async_reload": false }))
        .to_request();
    let _ = test::call_service(&app, reload_req).await;

    // Post-reload run → new snapshot: score 20, gen > 0.
    // RED: reload doesn't swap → score stays 10, gen stays 0.
    let req2 = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let body2 = json_body(test::call_service(&app, req2).await).await;
    assert_eq!(
        body2["facts_after"]["scorecard"]["score"].as_i64(),
        Some(20),
        "L-05 / D-01: post-reload run should see new snapshot (score 20); body: {body2}"
    );
    assert!(
        body2["engine_generation"].as_u64().unwrap_or(0) > 0,
        "L-05 / D-01: post-reload run should see gen > 0; body: {body2}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// SC-2: batch_panicking_item_isolated
// ---------------------------------------------------------------------

/// A batch with one panicking item must not poison the others (SC-2 / L-06).
/// The current `batch_execute_rules` collector already handles
/// `JoinError` (panic) → `failed += 1` (`execution.rs:648-655`), so the
/// isolation contract holds. However, a deterministic panic trigger
/// requires Plan 25-03's per-item error propagation refactor (the JoinSet
/// task returns `Result<ExecuteResponse, String>`), so this scaffold is
/// `#[ignore]`d until 25-03 lands a test fixture that deterministically
/// panics one item. Plan 25-03 removes the `#[ignore]` and turns this
/// GREEN.
#[serial_test::serial]
#[actix_web::test]
#[ignore = "Plan 25-03: deterministic panic trigger requires the Err→failed refactor"]
async fn batch_panicking_item_isolated() {
    reset_test_env();
    let app = init_test_app!();

    // Submit a batch of 2 valid items. The real SC-2 panic-isolation test
    // (25-03) will replace one item with a deterministic panic trigger.
    let batch_req = test::TestRequest::post()
        .uri("/v1/rules/batch-execute")
        .set_json(json!({
            "requests": [
                { "facts": { "scorecard": { "score": 0 } } },
                { "facts": { "scorecard": { "score": 0 } } }
            ]
        }))
        .to_request();
    let body = json_body(test::call_service(&app, batch_req).await).await;

    // At least one item should succeed (no poisoning). When 25-03 wires a
    // deterministic panic trigger, exactly one item panics → failed == 1
    // and the other succeeds → succeeded == 1.
    let succeeded = body["succeeded"].as_u64().unwrap_or(0);
    assert!(
        succeeded >= 1,
        "SC-2: at least one batch item should succeed (no poisoning); body: {body}"
    );
}

// ---------------------------------------------------------------------
// D-03a: reload_empty_dir_warns_not_crashes
// ---------------------------------------------------------------------

/// Calling reload on an empty/missing rules directory should warn and
/// continue with an empty ruleset (D-03a), NOT crash with a 500. RED today
/// because `EngineState::reload_from_directory` returns `Err` for a missing
/// directory (`execution.rs:119-121`), which the sync reload handler
/// surfaces as HTTP 500. Plan 25-02's real `build_ruleset_from_dir` parser
/// handles empty/missing dirs gracefully (warn + empty `RuleSet`) and
/// `reload_and_swap` swaps the empty ruleset without error.
#[serial_test::serial]
#[actix_web::test]
async fn reload_empty_dir_warns_not_crashes() {
    reset_test_env();
    // Point RULE_STORAGE_PATH at a nonexistent directory.
    let dir = std::env::temp_dir().join(format!("rrules-empty-{}", uuid::Uuid::new_v4()));
    std::env::set_var("RULE_STORAGE_PATH", dir.to_str().unwrap());

    let app = init_test_app!();

    let reload_req = test::TestRequest::post()
        .uri("/v1/rules/reload")
        .set_json(json!({ "async_reload": false }))
        .to_request();
    let resp = test::call_service(&app, reload_req).await;

    // D-03a: reload on a missing dir should NOT return 500. RED: current
    // reload_from_directory returns Err → 500.
    assert_ne!(
        resp.status().as_u16(),
        500,
        "D-03a: reload on empty/missing dir should not crash (500); got {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------
// DEPLOY-05 / D-04: sync_reload_swaps_and_bumps_generation
// ---------------------------------------------------------------------

/// POST `/v1/rules/reload {async:false}` returns 200, and a subsequent
/// execute sees the refreshed rules + bumped generation (D-04: BOTH sync
/// and async reload swap via the shared `reload_and_swap` helper). RED
/// today because the sync reload path calls
/// `engine_state.reload_from_directory` which only updates the
/// `rule_files` map — it does NOT swap the shared `SharedRuleSet` or bump
/// generation (L-02 / D-04). Plan 25-02's `reload_and_swap` makes both
/// paths swap.
#[serial_test::serial]
#[actix_web::test]
async fn sync_reload_swaps_and_bumps_generation() {
    reset_test_env();
    let dir = create_test_rules_dir();
    std::env::set_var("RULE_STORAGE_PATH", dir.to_str().unwrap());
    write_rule_file(&dir, "r1", &score_plus_10_rule());

    let app = init_test_app!();

    // Pre-reload execute → score 10, gen 0.
    let req1 = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let body1 = json_body(test::call_service(&app, req1).await).await;
    assert_eq!(
        body1["facts_after"]["scorecard"]["score"].as_i64(),
        Some(10),
        "D-04: pre-reload execute should see score 10; body: {body1}"
    );

    // Overwrite r1 (+20) and sync reload.
    write_rule_file(&dir, "r1", &score_plus_20_rule());
    let reload_req = test::TestRequest::post()
        .uri("/v1/rules/reload")
        .set_json(json!({ "async_reload": false }))
        .to_request();
    let reload_resp = test::call_service(&app, reload_req).await;
    assert!(
        reload_resp.status().is_success(),
        "D-04: sync reload should return 200, got {}",
        reload_resp.status()
    );

    // Post-reload execute → score 20, gen > 0.
    // RED: sync reload doesn't swap → score stays 10, gen stays 0.
    let req2 = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let body2 = json_body(test::call_service(&app, req2).await).await;
    assert_eq!(
        body2["facts_after"]["scorecard"]["score"].as_i64(),
        Some(20),
        "D-04: post-reload execute should see score 20 (swapped ruleset); body: {body2}"
    );
    assert!(
        body2["engine_generation"].as_u64().unwrap_or(0) > 0,
        "D-04: post-reload execute should see gen > 0 (bumped); body: {body2}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}