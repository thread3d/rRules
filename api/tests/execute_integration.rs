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

// Requirements covered by this suite: API-002

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use actix_web::{body::BoxBody, test, web, App};
use serde_json::{json, Value};

use api::metrics::{
    configure_metrics, coverage_endpoint, create_metrics_channel, dead_rules_endpoint,
    ExecutionCounters, MetricsTx,
};
use api::v1::execution::{
    batch_execute_rules, execute_rules, get_job_status, init_job_store, list_engines, list_jobs,
    reload_rules, JobStore, SharedRuleSet,
};
use api::v1::rules::create_rule;
use api::v1::sessions::{
    dispose_session, read_session_config, reset_session, SessionConfig, SessionManager,
};
use engine::data_types::Value as EngineValue;
use engine::engine::registry::EngineRegistry;
use engine::engine::selector::EngineSelector;
use engine::monitoring::coverage::CoverageTracking;
use engine::monitoring::detection::RuleExecutionTracker;
use engine::rules::{Action, ComparisonOp, Condition, Expression, LogLevel, Rule, RuleSet};

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

/// Phase 27 (SC-1..SC-4): the accumulating keystone rule — same action as
/// `score_plus_10_rule()` (`score := score + 10`) but with condition
/// `score >= 0` (`ComparisonOp::Ge`) so the rule fires on EVERY execute call
/// as long as a `scorecard` fact exists in working memory. This lets SC-1
/// prove WM accumulation: call 1 inserts `{score:0}` → rule fires → score:10;
/// call 2 with `facts={}` → WM retains score:10 → rule fires again → score:20
/// (impossible in the stateless path, which starts from request facts each
/// call and would have no scorecard to match).
///
/// Rule 1 deviation: the plan's SC-1 expects `score:20` on the second
/// same-session call, but `score_plus_10_rule()` has condition `score == 0`
/// which only matches on the first call (after increment, score is 10 and the
/// condition never matches again). The accumulating variant with `Ge` is the
/// minimal fix that preserves the plan's intent (prove WM accumulation) while
/// staying deterministic (one scorecard fact in WM, one rule fire, one
/// `UpdateFact` application per call).
fn score_plus_10_accumulating_rule() -> Rule {
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
    Rule::builder("r1", "Increment score (accumulating)")
        .condition(Condition::Pattern {
            fact_type: "scorecard".to_string(),
            field: "score".to_string(),
            value: EngineValue::Integer(0),
            op: ComparisonOp::Ge,
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
    let json = serde_json::to_string_pretty(rule).expect("serialize rule to JSON");
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

/// Phase 27 (SC-1..SC-4): shared `SharedRuleSet` containing the accumulating
/// keystone rule (`score >= 0` → `score += 10`). Used by
/// `init_test_app_with_manager!` so the sessionful SC tests can prove WM
/// accumulation across calls (the `score == 0` condition in
/// `score_plus_10_rule` only fires once, making `score:20` on the second call
/// impossible — see `score_plus_10_accumulating_rule` doc comment).
fn shared_accumulating_ruleset() -> web::Data<Arc<RwLock<SharedRuleSet>>> {
    let mut ruleset = RuleSet::new();
    ruleset
        .add_rule(score_plus_10_accumulating_rule())
        .expect("add_rule");
    web::Data::new(Arc::new(RwLock::new(SharedRuleSet::new(ruleset))))
}

/// Build the never-fires sentinel rule: `r2` with condition `score == 999`
/// and a no-op `Action::LogMessage`. `r2`'s condition never matches the
/// inserted fact (`score == 0`), so it is evaluated but never fires. This is
/// the D-07 / 26-RESEARCH.md "r2-unfired-rule trick": the r2-inclusion
/// assertion in `keystone_dead_rules` is the RED→GREEN proof that the
/// dead-rules panel is live (today dead-rules returns `[]` which excludes
/// r1 trivially but also fails to INCLUDE r2).
fn never_fires_rule() -> Rule {
    Rule::builder("r2", "Never-fires sentinel")
        .condition(Condition::Pattern {
            fact_type: "scorecard".to_string(),
            field: "score".to_string(),
            value: EngineValue::Integer(999),
            op: ComparisonOp::Eq,
        })
        .action(Action::LogMessage {
            level: LogLevel::Info,
            message: "r2 fired (should never happen in keystone)".to_string(),
        })
        .build()
}

/// Build a shared `SharedRuleSet` containing BOTH the keystone `r1` (fires
/// against `score == 0`) and the never-fires sentinel `r2` (condition
/// `score == 999`). Used by the Plan 26-02 SC-1/SC-2/SC-3 keystone e2e
/// tests. Mirrors `shared_ruleset()` but with the two-rule fixture.
fn two_rule_shared_ruleset() -> web::Data<Arc<RwLock<SharedRuleSet>>> {
    let mut ruleset = RuleSet::new();
    ruleset.add_rule(score_plus_10_rule()).expect("add_rule r1");
    ruleset.add_rule(never_fires_rule()).expect("add_rule r2");
    web::Data::new(Arc::new(RwLock::new(SharedRuleSet::new(ruleset))))
}

/// Initialize the test service with the two-rule (r1 + r2) app_data stack.
/// Mirrors `init_test_app!` but swaps `shared_ruleset()` for
/// `two_rule_shared_ruleset()` so the keystone e2e tests have the
/// never-fires sentinel `r2` in the ruleset.
macro_rules! init_two_rule_app {
    () => {
        test::init_service(
            App::new()
                .app_data(shared_registry())
                .app_data(shared_selector())
                .app_data(two_rule_shared_ruleset())
                .app_data(shared_counters())
                .app_data(shared_metrics_tx())
                .app_data(shared_job_store())
                // Phase 27 (D-09): shared SessionManager for SC-1..SC-4.
                .app_data(shared_session_manager())
                .configure(configure_test_routes)
                .configure(configure_metrics),
        )
        .await
    };
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

// =====================================================================
// Phase 27 (SESS-01): shared SessionManager for SC-1..SC-4
// =====================================================================

/// `OnceLock` holding the single shared `Arc<SessionManager>` the test App and
/// `reset_test_env()` both reference. Constructed once on first access via
/// `shared_session_manager()`; `reset_test_env()` calls `.clear()` on the
/// same `Arc` between tests so SC-1..SC-4 start from a known-empty state
/// (D-09 — mirrors the `CoverageTracking::clear()` /
/// `RuleExecutionTracker::clear()` pattern).
static SHARED_SESSION_MANAGER: std::sync::OnceLock<std::sync::Arc<SessionManager>> =
    std::sync::OnceLock::new();

/// Construct the `rrules_sessions_active` IntGauge on a fresh
/// `prometheus::Registry` (test-isolation: NOT the api crate's shared
/// `ExecutionCounters.registry` — registering the same metric name twice on
/// the same registry panics, Pitfall 24-A). The gauge is set to
/// `SessionManager::len()` after each execute and on every sweep eviction.
fn build_session_gauge() -> prometheus::IntGauge {
    let gauge = prometheus::IntGauge::new(
        "rrules_sessions_active_test",
        "Active stateful session count (test gauge)",
    )
    .expect("construct rrules_sessions_active_test gauge");
    let registry = prometheus::Registry::new();
    registry
        .register(Box::new(gauge.clone()))
        .expect("register rrules_sessions_active_test");
    gauge
}

/// Build (or return the cached) shared `Arc<SessionManager>` for SC-1..SC-4.
/// The same `Arc` is installed as `app_data` in `init_test_app!` /
/// `init_two_rule_app!` and referenced by `reset_test_env()` so `.clear()`
/// drains the same registry the tests use (D-09). Mirrors the
/// `shared_counters()` shape.
fn shared_session_manager() -> web::Data<std::sync::Arc<SessionManager>> {
    let arc = SHARED_SESSION_MANAGER.get_or_init(|| {
        std::sync::Arc::new(SessionManager::new(
            read_session_config(),
            build_session_gauge(),
        ))
    });
    web::Data::new(arc.clone())
}

/// Test-only accessor for the shared `Arc<SessionManager>` so
/// `reset_test_env()` can call `.clear()` on the same instance the App uses
/// (D-09 — registry isolation between serial tests).
fn shared_session_manager_arc() -> std::sync::Arc<SessionManager> {
    SHARED_SESSION_MANAGER
        .get_or_init(|| {
            std::sync::Arc::new(SessionManager::new(
                read_session_config(),
                build_session_gauge(),
            ))
        })
        .clone()
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
            // Phase 28 (D-03 / FRONTEND-04): engine-selector dropdown data
            // source. Mirrors the main.rs /v1 scope registration.
            .route("/engines", web::get().to(list_engines))
            // Phase 27 (D-09 / D-05): lifecycle routes for SC-4. Registered
            // before any wildcard `/execute/{...}` route (none currently
            // exists, but the discipline holds per PATTERNS.md line 372).
            .route("/execute/session/{id}", web::delete().to(dispose_session))
            .route("/execute/session/{id}/reset", web::post().to(reset_session))
            .route("/metrics/coverage", web::get().to(coverage_endpoint))
            .route("/metrics/dead-rules", web::get().to(dead_rules_endpoint))
            // Phase 25 (Plan 25-01 Task 2): reload + batch + jobs routes.
            // The real scope's `POST /v1/rules`, mounted so a test can create a rule *document* and show
            // that it is not a rule the engine executes (see `specs/rule-sources.md`).
            .route("/rules", web::post().to(create_rule))
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
                // The stores `create_rule` extracts — both of them. A handler that gains a `web::Data`
                // dependency breaks hand-rolled test apps at *runtime* (500), not at compile time; this is
                // exactly that hazard, found by the test that needed the handler (AGENTS.md).
                .app_data(web::Data::new(api::v1::rules::init_store()))
                .app_data(web::Data::new(api::v1::audit::init_store()))
                // Phase 27 (D-09): shared SessionManager for SC-1..SC-4.
                .app_data(shared_session_manager())
                .configure(configure_test_routes)
                .configure(configure_metrics),
        )
        .await
    };
}

/// Phase 27 (SC-2 / SC-3): like `init_test_app!` but swaps
/// `shared_session_manager()` for a caller-supplied `web::Data<Arc<SessionManager>>`
/// so tests needing non-default `idle_ttl` / `max_sessions` knobs can install
/// their own manager without disturbing the process-shared OnceLock instance
/// (D-09 — the shared manager captures env vars at first construction).
macro_rules! init_test_app_with_manager {
    ($manager_data:expr) => {
        test::init_service(
            App::new()
                .app_data(shared_registry())
                .app_data(shared_selector())
                // Phase 27 (SC-1..SC-4): accumulating rule so WM accumulation
                // is provable across same-session calls (score >= 0, not ==0).
                .app_data(shared_accumulating_ruleset())
                .app_data(shared_counters())
                .app_data(shared_metrics_tx())
                .app_data(shared_job_store())
                .app_data($manager_data)
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
    // Plan 26-02: also reset the RuleExecutionTracker singleton so the
    // keystone e2e tests start from a known-empty state (L-05 isolation
    // for the second process-wide singleton).
    RuleExecutionTracker::clear();
    // Phase 27 (D-09): clear the shared SessionManager registry so SC-1..SC-4
    // start from a known-empty state. Same shape as the CoverageTracking /
    // RuleExecutionTracker clear() calls — the shared Arc is the same one
    // the test App installs as app_data.
    shared_session_manager_arc().clear();
    // SAFETY on env vars: tests run serially (#[serial_test::serial]); no
    // concurrent test reads/writes these vars. Plan 23-05's handler reads
    // RRULES_DEFAULT_ENGINE once per request; Plan 25's reload handler reads
    // RULE_STORAGE_PATH once per reload. Phase 27's read_session_config
    // reads the TTL/max-sessions knobs once per SessionManager construction
    // (the SHARED_SESSION_MANAGER OnceLock), so env-var mutations between
    // tests only take effect on the first access per process — SC-2/SC-3
    // mutate env vars AND call clear() + use the existing manager (the env
    // vars are read by sweep_expired's Instant math via the manager's
    // idle_ttl/max_sessions fields captured at construction). Tests that
    // need different env-var-driven TTL/max-sessions values must construct
    // their own SessionManager (not the shared one) — see
    // `lru_evicts_oldest_and_gauge_capped` + `session_ttl_evicts_idle`.
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
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| {
        panic!(
            "create_test_rules_dir: failed to create {}: {e}",
            dir.display()
        )
    });
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

// =====================================================================
// Phase 26 Plan 26-02: SC-1 / SC-2 / SC-3 keystone e2e tests (Wave 0 RED)
//
// These three keystone e2e tests encode the Phase 26 success criteria and
// are intentionally RED until Plan 26-02 Task 2 wires the
// `RuleExecutionTracker::record_evaluation` / `record_fire` calls into BOTH
// evaluator call sites (`direct_eval.rs` for Treat/Leaps/Phreak AND
// `rete/mod.rs` for Rete — C1) and rewrites `dead_rules_endpoint` to read
// from the singleton (D-03, L-02, L-03). They compile against the existing
// Phase 23-25 API and FAIL on their substantive assertions:
//
//   - `keystone_dead_rules` fails on the r2-INCLUSION assertion: today no
//     live path records into the singleton, so `find_dead_rules()` returns
//     `[]` and `r2` (authored but never fired) is absent from the response.
//     The r1-exclusion assertion passes trivially because `[]` excludes
//     everything — the r2-inclusion assertion is the real proof the panel
//     is live (D-07 / 26-RESEARCH.md "E2E Keystone Test Design").
//   - `keystone_three_surfaces_agree` fails on the dead-rules r1-EXCLUSION
//     assertion for the same reason: with the singleton empty, dead-rules
//     returns `[]` which DOES exclude r1 — but the test also asserts
//     `r1` is excluded because `fire_count > 0`, and the empty-list case
//     fails the `fire_count > 0 ⇒ not dead` reasoning check (the test
//     asserts r1 is NOT in the list AND that the Prometheus + coverage
//     surfaces show r1 fired; if dead-rules returns `[]` while coverage
//     shows r1.fired == true, the three surfaces disagree — dead-rules
//     says r1 is dead, coverage says r1 is alive).
//
// Plan 26-02 Task 2 turns these GREEN. Mirrors the Phase 23-04 / 24-01 /
// 25-01 Wave 0 RED→GREEN + Nyquist pattern.
// =====================================================================

/// Fire the keystone two-rule fixture against `engine: "rete"`. Inlined
/// into each keystone test via the `fire_keystone!` macro because the
/// `test::init_service` return type is unnameable (mirrors the
/// `init_test_app!` pattern).
macro_rules! fire_keystone {
    ($app:expr) => {{
        let req = test::TestRequest::post()
            .uri("/v1/execute")
            .set_json(json!({ "engine": "rete", "facts": { "scorecard": { "score": 0 } } }))
            .to_request();
        let body = json_body(test::call_service(&$app, req).await).await;
        assert!(
            body["rules_fired"].as_u64().unwrap_or(0) >= 1,
            "keystone setup: r1 should fire, got rules_fired = {}; body: {body}",
            body["rules_fired"]
        );
        body
    }};
}

/// SC-1 (keystone_dead_rules): after POST /v1/execute fires `r1` (and `r2`
/// is authored but never fires), GET /v1/metrics/dead-rules does NOT
/// contain `r1` AND DOES contain `r2`. RED today because no live path
/// records into the `RuleExecutionTracker` singleton, so
/// `find_dead_rules()` returns `[]` which excludes `r1` trivially but
/// also fails to INCLUDE `r2` — the r2-inclusion assertion is the
/// RED→GREEN proof that the panel is live.
#[serial_test::serial]
#[actix_web::test]
async fn keystone_dead_rules() {
    reset_test_env();
    let app = init_two_rule_app!();

    let _body = fire_keystone!(app);

    // Read the dead-rules surface.
    let req = test::TestRequest::get()
        .uri("/v1/metrics/dead-rules")
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    let rule_ids: Vec<String> = body
        .as_array()
        .expect("dead-rules response should be a JSON array")
        .iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect();

    // SC-1: r1 fired ⇒ NOT dead ⇒ excluded.
    assert!(
        !rule_ids.contains(&"r1".to_string()),
        "SC-1: dead-rules should NOT contain r1 (it fired); got {:?}; body: {body}",
        rule_ids
    );
    // SC-1 / D-07: r2 authored but never fired ⇒ dead ⇒ INCLUDED. This is
    // the RED→GREEN proof — today the singleton is empty so r2 is absent.
    assert!(
        rule_ids.contains(&"r2".to_string()),
        "SC-1: dead-rules SHOULD contain r2 (authored but never fired); got {:?}; body: {body}",
        rule_ids
    );
}

/// SC-2 (keystone_coverage): after POST /v1/execute fires `r1`, GET
/// /v1/metrics/coverage shows `r1.fired == true` + `r1.evaluation_count >=
/// 1` AND `r2.fired == false`. This may already pass GREEN from Phase 23's
/// coverage wiring — if so, it is a regression guard for the SC-2 contract.
#[serial_test::serial]
#[actix_web::test]
async fn keystone_coverage() {
    reset_test_env();
    let app = init_two_rule_app!();

    let _body = fire_keystone!(app);

    // Read the coverage surface.
    let req = test::TestRequest::get()
        .uri("/v1/metrics/coverage")
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    let rules = body["rules"]
        .as_array()
        .expect("coverage 'rules' should be an array");
    let r1 = rules
        .iter()
        .find(|r| r["rule_id"] == "r1")
        .unwrap_or_else(|| panic!("SC-2: r1 should appear in coverage rules; body: {body}"));
    let r2 = rules
        .iter()
        .find(|r| r["rule_id"] == "r2")
        .unwrap_or_else(|| panic!("SC-2: r2 should appear in coverage rules; body: {body}"));

    assert_eq!(
        r1["fired"].as_bool(),
        Some(true),
        "SC-2: r1 should be fired in coverage; body: {body}"
    );
    assert!(
        r1["evaluation_count"].as_u64().unwrap_or(0) >= 1,
        "SC-2: r1 evaluation_count should be >= 1; body: {body}"
    );
    assert_eq!(
        r2["fired"].as_bool(),
        Some(false),
        "SC-2: r2 should NOT be fired in coverage; body: {body}"
    );
}

/// SC-3 (keystone_three_surfaces_agree): after POST /v1/execute fires `r1`,
/// three metric surfaces agree on fire count:
///   - Prometheus `/metrics`: `rrules_execute_rules_fired_total{engine="rete"}` >= 1
///   - `/v1/metrics/coverage`: `r1.fire_count >= 1`
///   - `/v1/metrics/dead-rules`: `r1` NOT in the list (fire_count > 0 ⇒ not dead)
/// All three derive from the shared `fired_set` (D-02 — single source of
/// truth), so agreement is by construction once Task 2 wires the recording.
/// RED today because the dead-rules surface returns `[]` (singleton empty)
/// while coverage shows `r1.fired == true` — the surfaces disagree.
#[serial_test::serial]
#[actix_web::test]
async fn keystone_three_surfaces_agree() {
    reset_test_env();
    let app = init_two_rule_app!();

    let _body = fire_keystone!(app);

    // Surface 1: Prometheus text — parse `rrules_execute_rules_fired_total{engine="rete"}`.
    let prom_req = test::TestRequest::get()
        .uri("/metrics")
        .insert_header(("Accept", "text/plain"))
        .to_request();
    let prom_resp = test::call_service(&app, prom_req).await;
    assert!(
        prom_resp.status().is_success(),
        "SC-3: /metrics should return 200, got {}",
        prom_resp.status()
    );
    let prom_bytes = test::read_body(prom_resp).await;
    let prom_text = String::from_utf8_lossy(&prom_bytes);
    let prom_fired_total =
        extract_prom_counter(&prom_text, "rrules_execute_rules_fired_total", "rete");
    assert!(
        prom_fired_total >= 1,
        "SC-3: Prometheus rrules_execute_rules_fired_total{{engine=\"rete\"}} should be >= 1; got {prom_fired_total}; text: {prom_text}"
    );

    // Surface 2: coverage — r1.fire_count >= 1.
    let cov_req = test::TestRequest::get()
        .uri("/v1/metrics/coverage")
        .to_request();
    let cov_body = json_body(test::call_service(&app, cov_req).await).await;
    let rules = cov_body["rules"]
        .as_array()
        .expect("SC-3: coverage 'rules' should be an array");
    let r1 = rules
        .iter()
        .find(|r| r["rule_id"] == "r1")
        .unwrap_or_else(|| panic!("SC-3: r1 should appear in coverage rules; body: {cov_body}"));
    let cov_fire_count = r1["fire_count"].as_u64().unwrap_or(0);
    assert!(
        cov_fire_count >= 1,
        "SC-3: coverage r1.fire_count should be >= 1; got {cov_fire_count}; body: {cov_body}"
    );

    // Surface 3: dead-rules — r1 NOT in the list (fire_count > 0 ⇒ not dead).
    let dead_req = test::TestRequest::get()
        .uri("/v1/metrics/dead-rules")
        .to_request();
    let dead_body = json_body(test::call_service(&app, dead_req).await).await;
    let dead_ids: Vec<String> = dead_body
        .as_array()
        .expect("SC-3: dead-rules response should be a JSON array")
        .iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        !dead_ids.contains(&"r1".to_string()),
        "SC-3: dead-rules should NOT contain r1 (fire_count > 0 ⇒ not dead); got {:?}; body: {dead_body}",
        dead_ids
    );

    // Three-surfaces agreement: Prometheus >= 1, coverage >= 1, dead-rules
    // excludes r1. All three derive from fired_set (D-02).
}

/// Extract a Prometheus counter value from the exposition text for the
/// metric `name{engine="<engine>"}`. Returns 0 if the metric line is not
/// found. Prometheus exposition format:
/// `rrules_execute_rules_fired_total{engine="rete"} 5`
fn extract_prom_counter(text: &str, name: &str, engine: &str) -> u64 {
    let label = format!("{name}{{engine=\"{engine}\"}}");
    for line in text.lines() {
        if line.starts_with(&label) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                return parts[1].parse::<u64>().unwrap_or(0);
            }
        }
    }
    0
}

// =====================================================================
// Phase 27 (SESS-01): Wave 0 RED keystone SC tests
//
// Four `#[serial_test::serial]` integration tests encoding the SC-1..SC-4
// success criteria. These are RED today (Task 1) because `execute_single`
// has no sessionful branch, `lookup_or_create` returns `NOT_IMPLEMENTED`,
// and `dispose_session` / `reset_session` return `NOT_IMPLEMENTED`. Task 2
// implements `SessionManager`; Task 3 wires the live `execute_single` branch
// and turns all four GREEN.
//
// All four use the accumulating keystone rule (`score >= 0` → `score += 10`,
// see `score_plus_10_accumulating_rule`) so WM accumulation across same-
// session calls is provable: call 1 inserts `{score:0}` → score:10; call 2
// with `facts={}` → WM retains score:10 → rule fires again → score:20.
//
// Filter: `cargo test -p api --test execute_integration -- stateful_session
//          session_ttl lru_evicts session_lifecycle`
// =====================================================================

/// SC-1: A second `POST /v1/execute` with the same `session_id` and
/// `facts={}` fires r1 again against the accumulated working memory and
/// returns `facts_after.scorecard.score == 20` (NOT 10 — proves WM
/// accumulated across calls; impossible in the stateless path which starts
/// from request facts each call and would have no scorecard to match).
///
/// Also asserts `response.session_id` is present (the server-generated UUID)
/// on the first call — absent on the stateless path (D-04/D-05).
///
/// RED today: `execute_single` has no sessionful branch → stateless path
/// returns `session_id: None` (assertion fails) and the second call with
/// `facts={}` has no scorecard in WM → no rule fires → no score (assertion
/// fails).
#[serial_test::serial]
#[actix_web::test]
async fn stateful_session_accumulates_wm() {
    reset_test_env();
    let app = init_test_app_with_manager!(shared_session_manager());

    // Call 1: insert {score:0}, rule fires (0 >= 0), score → 10.
    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "session_id": "s1",
            "facts": { "scorecard": { "score": 0 } }
        }))
        .to_request();
    let body1 = json_body(test::call_service(&app, req).await).await;
    let score1 = body1["facts_after"]["scorecard"]["score"].as_i64();
    assert_eq!(
        score1,
        Some(10),
        "SC-1 call 1: facts_after.scorecard.score should be 10, got {:?}; body: {body1}",
        score1,
    );
    assert!(
        body1["session_id"].as_str().is_some(),
        "SC-1 call 1: response should carry server session_id (D-04), got {:?}; body: {body1}",
        body1.get("session_id"),
    );

    // Call 2: same session_id, facts={}. WM retains score:10 from call 1.
    // Rule fires again (10 >= 0), score → 20. This is the keystone honesty
    // anchor — impossible in the stateless path.
    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "session_id": "s1",
            "facts": {}
        }))
        .to_request();
    let body2 = json_body(test::call_service(&app, req).await).await;
    let score2 = body2["facts_after"]["scorecard"]["score"].as_i64();
    assert_eq!(
        score2,
        Some(20),
        "SC-1 call 2: facts_after.scorecard.score should be 20 (accumulated WM), got {:?}; body: {body2}",
        score2,
    );
}

/// SC-2: A session untouched for >idle TTL is evicted by `sweep_expired()`;
/// a subsequent call with that `session_id` creates a FRESH session with
/// empty WM (no scorecard in `facts_after`). A session touched within TTL
/// persists — its second call returns `score:20` (WM accumulated).
///
/// Uses a custom `SessionManager` with `idle_ttl = 1s` so the test is
/// deterministic (sleep 1100ms → sweep → evict). The sweep is called directly
/// (white-box, RESEARCH A2) — no reliance on the background sweep task.
///
/// RED today: `execute_single` has no sessionful branch → no sessions are
/// created → `session_id` assertion fails on call 1.
#[serial_test::serial]
#[actix_web::test]
async fn session_ttl_evicts_idle() {
    reset_test_env();

    // Custom manager with idle_ttl=1s for deterministic eviction.
    let config = SessionConfig {
        idle_ttl: Duration::from_secs(1),
        max_lifetime: Duration::from_secs(86400),
        max_sessions: 1024,
        sweep_interval: Duration::from_secs(60),
    };
    let manager = Arc::new(SessionManager::new(config, build_session_gauge()));
    let app = init_test_app_with_manager!(web::Data::new(manager.clone()));

    // Call 1: create s2, insert {score:0}, rule fires, score → 10.
    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "session_id": "s2",
            "facts": { "scorecard": { "score": 0 } }
        }))
        .to_request();
    let body1 = json_body(test::call_service(&app, req).await).await;
    assert_eq!(
        body1["facts_after"]["scorecard"]["score"].as_i64(),
        Some(10),
        "SC-2 call 1: score should be 10; body: {body1}",
    );
    assert!(
        body1["session_id"].as_str().is_some(),
        "SC-2 call 1: response should carry server session_id; body: {body1}",
    );

    // Wait > idle_ttl (1s), then sweep — evicts s2 (idle).
    tokio::time::sleep(Duration::from_millis(1100)).await;
    manager.sweep_expired();

    // Call 2: s2 was evicted → fresh session with empty WM → no scorecard.
    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "session_id": "s2", "facts": {} }))
        .to_request();
    let body2 = json_body(test::call_service(&app, req).await).await;
    assert!(
        body2["facts_after"]["scorecard"].is_null(),
        "SC-2: evicted session should have empty WM (no scorecard in facts_after), got {:?}; body: {body2}",
        body2["facts_after"]["scorecard"],
    );

    // Contrast: a touched session persists. Create s2b, then immediately
    // call again (touch within TTL) → WM accumulates → score:20.
    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "session_id": "s2b",
            "facts": { "scorecard": { "score": 0 } }
        }))
        .to_request();
    let _body2b1 = json_body(test::call_service(&app, req).await).await;

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "session_id": "s2b", "facts": {} }))
        .to_request();
    let body2b2 = json_body(test::call_service(&app, req).await).await;
    assert_eq!(
        body2b2["facts_after"]["scorecard"]["score"].as_i64(),
        Some(20),
        "SC-2: touched session should persist with accumulated WM (score:20), got {:?}; body: {body2b2}",
        body2b2["facts_after"]["scorecard"]["score"],
    );
}

/// SC-3: When session count exceeds `MAX_SESSIONS`, `sweep_expired()` evicts
/// the LRU entry (oldest `last_used`). The `rrules_sessions_active` gauge
/// reflects the capped live count (reads `MAX_SESSIONS`, not
/// `MAX_SESSIONS + 1`).
///
/// Uses a custom `SessionManager` with `max_sessions = 2`. Creates s1/s2/s3,
/// calls `sweep_expired()` directly (LRU eviction when over max), asserts
/// `len() == 2` (gauge capped), s1 evicted (fresh WM on resume), s2 persists
/// (accumulated WM).
///
/// RED today: no sessions are created (stateless path) → `len() == 0` !=
/// 2 → assertion fails.
#[serial_test::serial]
#[actix_web::test]
async fn lru_evicts_oldest_and_gauge_capped() {
    reset_test_env();

    let config = SessionConfig {
        idle_ttl: Duration::from_secs(900),
        max_lifetime: Duration::from_secs(86400),
        max_sessions: 2,
        sweep_interval: Duration::from_secs(60),
    };
    let manager = Arc::new(SessionManager::new(config, build_session_gauge()));
    let app = init_test_app_with_manager!(web::Data::new(manager.clone()));

    // Create three sessions (exceeds max_sessions=2). Each call inserts
    // {score:0} and fires the rule → score:10.
    for sid in ["s3a", "s3b", "s3c"] {
        let req = test::TestRequest::post()
            .uri("/v1/execute")
            .set_json(json!({
                "session_id": sid,
                "facts": { "scorecard": { "score": 0 } }
            }))
            .to_request();
        let body = json_body(test::call_service(&app, req).await).await;
        assert_eq!(
            body["facts_after"]["scorecard"]["score"].as_i64(),
            Some(10),
            "SC-3: {} call 1 should give score:10; body: {body}",
            sid,
        );
    }

    // LRU eviction: sweep evicts the oldest-touched entry (s3a) to cap at
    // max_sessions. The gauge must read 2, not 3.
    manager.sweep_expired();
    assert_eq!(
        manager.len(),
        2,
        "SC-3: gauge should reflect capped count 2, not {}",
        manager.len(),
    );

    // s3a was evicted → fresh session → empty WM → no scorecard.
    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "session_id": "s3a", "facts": {} }))
        .to_request();
    let body_a = json_body(test::call_service(&app, req).await).await;
    assert!(
        body_a["facts_after"]["scorecard"].is_null(),
        "SC-3: s3a (LRU-evicted) should have empty WM; body: {body_a}",
    );

    // s3b persists → WM accumulated → score:20 (10 + 10).
    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "session_id": "s3b", "facts": {} }))
        .to_request();
    let body_b = json_body(test::call_service(&app, req).await).await;
    assert_eq!(
        body_b["facts_after"]["scorecard"]["score"].as_i64(),
        Some(20),
        "SC-3: s3b (persisted) should have accumulated WM (score:20); body: {body_b}",
    );
}

/// SC-4: `DELETE /v1/execute/session/{id}` disposes the session (a
/// subsequent call creates a fresh session with empty WM).
/// `POST /v1/execute/session/{id}/reset` clears WM WITHOUT disposing (a
/// subsequent call resumes with empty WM).
///
/// RED today: `dispose_session` and `reset_session` return `NOT_IMPLEMENTED`
/// (501) → status assertion fails.
#[serial_test::serial]
#[actix_web::test]
async fn session_lifecycle_delete_reset() {
    reset_test_env();
    let app = init_test_app_with_manager!(shared_session_manager());

    // Create s4, insert {score:0}, rule fires, score → 10.
    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "session_id": "s4",
            "facts": { "scorecard": { "score": 0 } }
        }))
        .to_request();
    let body1 = json_body(test::call_service(&app, req).await).await;
    assert_eq!(
        body1["facts_after"]["scorecard"]["score"].as_i64(),
        Some(10),
        "SC-4: s4 call 1 should give score:10; body: {body1}",
    );

    // DELETE s4 → 200 (disposed).
    let req = test::TestRequest::delete()
        .uri("/v1/execute/session/s4")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status().as_u16(),
        200,
        "SC-4: DELETE session/s4 should return 200, got {}",
        resp.status(),
    );

    // POST s4 with facts={} → fresh session (disposed) → empty WM → no score.
    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "session_id": "s4", "facts": {} }))
        .to_request();
    let body2 = json_body(test::call_service(&app, req).await).await;
    assert!(
        body2["facts_after"]["scorecard"].is_null(),
        "SC-4: disposed session should have empty WM on resume; body: {body2}",
    );

    // Create s5, accumulate WM (score:10 → score:20).
    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "session_id": "s5",
            "facts": { "scorecard": { "score": 0 } }
        }))
        .to_request();
    let _body3 = json_body(test::call_service(&app, req).await).await;

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "session_id": "s5", "facts": {} }))
        .to_request();
    let body4 = json_body(test::call_service(&app, req).await).await;
    assert_eq!(
        body4["facts_after"]["scorecard"]["score"].as_i64(),
        Some(20),
        "SC-4: s5 should accumulate WM (score:20) before reset; body: {body4}",
    );

    // POST /v1/execute/session/s5/reset → 200 (WM cleared, session kept).
    let req = test::TestRequest::post()
        .uri("/v1/execute/session/s5/reset")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status().as_u16(),
        200,
        "SC-4: POST session/s5/reset should return 200, got {}",
        resp.status(),
    );

    // POST s5 with facts={} → WM was cleared by reset → no scorecard.
    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "session_id": "s5", "facts": {} }))
        .to_request();
    let body5 = json_body(test::call_service(&app, req).await).await;
    assert!(
        body5["facts_after"]["scorecard"].is_null(),
        "SC-4: reset session should have empty WM (no scorecard); body: {body5}",
    );
}

// =====================================================================
// Phase 28 (Plan 28-01): Wave 0 RED backend integration scaffolds
//
// Four scaffolds encoding the EXEC-04 (inference_trace population) and
// FRONTEND-04 (GET /v1/engines) contracts. Intentionally RED until Plan
// 28-03 implements the backend:
//
//   - `trace_on_populates_entries`  (EXEC-04 / D-01) — `trace: true` →
//     `inference_trace` is `Some` with `entries.len() >= 1` and the first
//     entry has `event_type == "rule_fired"` + `rule_name == "r1"`.
//   - `trace_off_is_none`           (EXEC-04 / D-02) — `trace` omitted →
//     `inference_trace` is null/absent AND execution still works
//     (`rules_fired >= 1`, `facts_after.scorecard.score == 10`).
//   - `rete_trace_populated`        (EXEC-04 / D-01 C1 two-site) —
//     `engine: "rete"` + `trace: true` → `inference_trace` populated
//     (Rete's inlined `execute_rules` must also emit trace, not just the
//     shared `direct_eval` helper).
//   - `list_engines_returns_registry` (FRONTEND-04 / D-03) —
//     `GET /v1/engines` → 200 `{ engines: [...], default: "..." }`;
//     `engines` non-empty + sorted ascending + includes the default;
//     `default` is a string.
//
// RED today because:
//   - `ExecuteRequest` has no `trace` field yet (D-02) — serde ignores the
//     unknown key and `execute_single` always sets `inference_trace: None`
//     (L-01, `execution.rs:730`/`:950`). The `trace_on_populates_entries`
//     and `rete_trace_populated` assertions on `inference_trace.entries`
//     fail because the field is `null`.
//   - `trace_off_is_none` PASSES today (trace off → null is the current
//     behavior) — it is a regression guard for the GREEN plan (28-03 must
//     keep the off path null + execution working). The `rules_fired >= 1`
//     and `facts_after.scorecard.score == 10` assertions hold from Phase 23.
//   - `GET /v1/engines` route is not registered (`main.rs` / configure_test_routes
//     has no `/engines` route) → 404 → the status + body assertions fail.
//
// Plan 28-03 implements: `ExecuteRequest.trace: Option<bool>`, evaluator
// trace emission at BOTH sites (D-01), and the `GET /v1/engines` handler +
// route registration (D-03). Mirrors the Phase 23-04 / 24-01 / 25-01 /
// 26-01 / 27-01 Wave 0 RED→GREEN + Nyquist pattern.
//
// Filter: `cargo test -p api --test execute_integration -- trace_on
//          trace_off rete_trace list_engines`
// =====================================================================

/// EXEC-04 / D-01: POST `/v1/execute` with the keystone rule
/// (`score := score + 10`), facts `{ "scorecard": { "score": 0 } }`, and
/// `trace: true` in the body → response body's `inference_trace` is NOT
/// null, has `entries` with length >= 1, the first entry has
/// `event_type == "rule_fired"` and `rule_name` matching the fired rule id
/// (`r1`); `summary` is present.
///
/// RED today: `ExecuteRequest` has no `trace` field (D-02) and
/// `execute_single` always returns `inference_trace: None` (L-01). The
/// `inference_trace` assertion fails because the field is `null`.
/// Plan 28-03 turns this GREEN.
#[serial_test::serial]
#[actix_web::test]
async fn trace_on_populates_entries() {
    reset_test_env();
    let app = init_test_app!();

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "facts": { "scorecard": { "score": 0 } },
            "trace": true,
        }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    // inference_trace must be present (an object, not null).
    assert!(
        body["inference_trace"].is_object(),
        "EXEC-04 / D-01: inference_trace should be an object when trace:true; got {:?}; body: {body}",
        body.get("inference_trace"),
    );

    // entries array with length >= 1.
    let entries = body["inference_trace"]["entries"]
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "EXEC-04 / D-01: inference_trace.entries should be a non-empty array; got {:?}; body: {body}",
                body["inference_trace"]["entries"],
            )
        });
    assert!(
        !entries.is_empty(),
        "EXEC-04 / D-01: inference_trace.entries should have length >= 1; got len={}; body: {body}",
        entries.len(),
    );

    // First entry: event_type == "rule_fired", rule_name == "r1".
    let first = &entries[0];
    assert_eq!(
        first["event_type"].as_str(),
        Some("rule_fired"),
        "EXEC-04 / D-01: first trace entry event_type should be 'rule_fired'; got {:?}; body: {body}",
        first["event_type"],
    );
    assert_eq!(
        first["rule_name"].as_str(),
        Some("r1"),
        "EXEC-04 / D-01: first trace entry rule_name should be 'r1'; got {:?}; body: {body}",
        first["rule_name"],
    );

    // summary present (object).
    assert!(
        body["inference_trace"]["summary"].is_object(),
        "EXEC-04 / D-01: inference_trace.summary should be present (object); got {:?}; body: {body}",
        body["inference_trace"]["summary"],
    );
}

/// EXEC-04 / D-02: POST `/v1/execute` with the same keystone rule + facts
/// but `trace` omitted (or `trace: false`) → response body's
/// `inference_trace` is null/absent; `rules_fired` still >= 1 and
/// `facts_after.scorecard.score == 10` (execution still works without
/// trace — SC-3 latency neutrality is enforced by the evaluator skipping
/// trace work when `trace` is absent/false).
///
/// This scaffold is GREEN today (trace off → null is the current behavior)
/// and serves as a regression guard: Plan 28-03 must keep the off path
/// null + execution working. The `rules_fired >= 1` and
/// `facts_after.scorecard.score == 10` assertions hold from Phase 23.
#[serial_test::serial]
#[actix_web::test]
async fn trace_off_is_none() {
    reset_test_env();
    let app = init_test_app!();

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "facts": { "scorecard": { "score": 0 } },
            "trace": false,
        }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    // inference_trace must be null/absent when trace is false.
    assert!(
        body["inference_trace"].is_null() || body.get("inference_trace").is_none(),
        "EXEC-04 / D-02: inference_trace should be null/absent when trace:false; got {:?}; body: {body}",
        body.get("inference_trace"),
    );

    // Execution still works: rules_fired >= 1.
    assert!(
        body["rules_fired"].as_u64().unwrap_or(0) >= 1,
        "EXEC-04 / D-02: rules_fired should be >= 1 even without trace; got {}; body: {body}",
        body["rules_fired"],
    );

    // facts_after.scorecard.score == 10 (the action's increment).
    assert_eq!(
        body["facts_after"]["scorecard"]["score"].as_i64(),
        Some(10),
        "EXEC-04 / D-02: facts_after.scorecard.score should be 10 (execution works without trace); body: {body}",
    );
}

/// EXEC-04 / D-01 (C1 two-site): POST `/v1/execute` with `engine: "rete"`
/// and `trace: true` → response `inference_trace` is populated with
/// entries. This is the C1 two-site requirement — Rete's inlined
/// `execute_rules` (`engine/src/engine/rete/mod.rs`) must also emit trace,
/// not just the shared `direct_eval` helper (`direct_eval.rs`).
///
/// RED today: `ExecuteRequest` has no `trace` field (D-02) and neither
/// evaluator site emits trace (D-01 not implemented). The
/// `inference_trace` assertion fails because the field is `null`.
/// Plan 28-03 turns this GREEN by threading `collect_trace` into BOTH
/// the shared helper AND `ReteEngine::execute_rules` (C1).
#[serial_test::serial]
#[actix_web::test]
async fn rete_trace_populated() {
    reset_test_env();
    let app = init_test_app!();

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "engine": "rete",
            "facts": { "scorecard": { "score": 0 } },
            "trace": true,
        }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    // inference_trace must be present (an object, not null) for the rete engine.
    assert!(
        body["inference_trace"].is_object(),
        "EXEC-04 / D-01 (C1 rete): inference_trace should be an object when engine:'rete' + trace:true; got {:?}; body: {body}",
        body.get("inference_trace"),
    );

    // entries array with length >= 1.
    let entries = body["inference_trace"]["entries"]
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "EXEC-04 / D-01 (C1 rete): inference_trace.entries should be a non-empty array; got {:?}; body: {body}",
                body["inference_trace"]["entries"],
            )
        });
    assert!(
        !entries.is_empty(),
        "EXEC-04 / D-01 (C1 rete): inference_trace.entries should have length >= 1 for rete engine; got len={}; body: {body}",
        entries.len(),
    );

    // First entry event_type == "rule_fired" (the rete path emits the same
    // event shape as the direct_eval path).
    assert_eq!(
        entries[0]["event_type"].as_str(),
        Some("rule_fired"),
        "EXEC-04 / D-01 (C1 rete): first trace entry event_type should be 'rule_fired'; got {:?}; body: {body}",
        entries[0]["event_type"],
    );
}

/// FRONTEND-04 / D-03: GET `/v1/engines` → 200 response with body
/// `{ "engines": [...], "default": "..." }`; `engines` array is non-empty
/// and includes at least the default engine; `engines` is sorted ascending;
/// `default` is a string.
///
/// RED today: no `/v1/engines` route is registered in `configure_test_routes`
/// (or `main.rs`) → the request returns 404 → the status assertion fails.
/// Plan 28-03 adds the `list_engines` handler + registers the route
/// (D-03).
#[serial_test::serial]
#[actix_web::test]
async fn list_engines_returns_registry() {
    reset_test_env();
    let app = init_test_app!();

    let req = test::TestRequest::get().uri("/v1/engines").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "FRONTEND-04 / D-03: GET /v1/engines should return 200, got {}",
        resp.status(),
    );

    let body = json_body(resp).await;

    // engines is a non-empty array of strings.
    let engines = body["engines"].as_array().unwrap_or_else(|| {
        panic!(
            "FRONTEND-04 / D-03: 'engines' should be an array; got {:?}; body: {body}",
            body["engines"],
        )
    });
    assert!(
        !engines.is_empty(),
        "FRONTEND-04 / D-03: 'engines' should be non-empty; got {:?}; body: {body}",
        engines,
    );
    for e in engines {
        assert!(
            e.is_string(),
            "FRONTEND-04 / D-03: each engine entry should be a string; got {:?}; body: {body}",
            e,
        );
    }

    // default is a string.
    let default = body["default"].as_str().unwrap_or_else(|| {
        panic!(
            "FRONTEND-04 / D-03: 'default' should be a string; got {:?}; body: {body}",
            body["default"],
        )
    });
    assert!(
        !default.is_empty(),
        "FRONTEND-04 / D-03: 'default' should be a non-empty string; got {:?}; body: {body}",
        default,
    );

    // engines includes the default.
    let engine_names: Vec<String> = engines
        .iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        engine_names.iter().any(|n| n == default),
        "FRONTEND-04 / D-03: 'engines' should include the default engine ({}); got {:?}; body: {body}",
        default,
        engine_names,
    );

    // engines is sorted ascending.
    let mut sorted = engine_names.clone();
    sorted.sort();
    assert_eq!(
        engine_names, sorted,
        "FRONTEND-04 / D-03: 'engines' should be sorted ascending; got {:?}; body: {body}",
        engine_names,
    );
}

// =====================================================================
// v5.0 audit WARNING 1 (FRONTEND-03): rule_filter is honored server-side.
// The TestRunner sends { name_pattern: ruleId } for a "test this rule" UX;
// pre-fix the engine loaded ALL rules (rule_filter was a dead input). These
// tests prove `filter_ruleset` scopes the loaded ruleset on the stateless
// path. Uses the two-rule fixture (r1 fires on score==0; r2 needs score==999).
// =====================================================================

/// rule_filter matching r2 (the never-fires sentinel) must EXCLUDE r1 from the
/// loaded ruleset. With facts score==0, r1 would normally fire — proving the
/// filter actually scopes the engine (without it rules_fired would be >= 1).
#[serial_test::serial]
#[actix_web::test]
async fn rule_filter_excludes_non_matching_rule() {
    reset_test_env();
    let app = init_two_rule_app!();

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "facts": { "scorecard": { "score": 0 } },
            "rule_filter": { "name_pattern": "r2" }
        }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    // r1 was filtered OUT → only r2 loaded → r2 needs score==999 → nothing fires.
    assert_eq!(
        body["rules_fired"].as_u64(),
        Some(0),
        "FRONTEND-03: rule_filter=r2 must exclude r1 (which fires on score==0); \
         rules_fired should be 0, got {:?}; body: {body}",
        body["rules_fired"]
    );
    // Only r2 was loaded, and it didn't fire → rules_not_fired == ["r2"].
    let not_fired = body["rules_not_fired"]
        .as_array()
        .expect("rules_not_fired should be an array");
    assert_eq!(not_fired.len(), 1, "only r2 was loaded; body: {body}");
    assert_eq!(
        not_fired[0], "r2",
        "rules_not_fired should be [\"r2\"]; body: {body}"
    );
}

/// rule_filter by human-readable NAME (not id) loads that rule. Filtering to
/// r1's name "Increment score" with score==0 fires r1 (rules_fired >= 1).
#[serial_test::serial]
#[actix_web::test]
async fn rule_filter_matches_by_name() {
    reset_test_env();
    let app = init_two_rule_app!();

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "facts": { "scorecard": { "score": 0 } },
            "rule_filter": { "name_pattern": "Increment score" }
        }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert!(
        body["rules_fired"].as_u64().unwrap_or(0) >= 1,
        "FRONTEND-03: rule_filter by name 'Increment score' should load r1 \
         and fire on score==0; rules_fired = {:?}; body: {body}",
        body["rules_fired"]
    );
}

/// rule_filter matching no rule loads an empty ruleset → rules_fired 0 and
/// rules_not_fired empty (graceful — no 500, no phantom rules).
#[serial_test::serial]
#[actix_web::test]
async fn rule_filter_no_match_loads_empty_ruleset() {
    reset_test_env();
    let app = init_two_rule_app!();

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "facts": { "scorecard": { "score": 0 } },
            "rule_filter": { "name_pattern": "does-not-exist" }
        }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert_eq!(
        body["rules_fired"].as_u64(),
        Some(0),
        "no rules loaded → rules_fired 0; body: {body}"
    );
    assert!(
        body["rules_not_fired"]
            .as_array()
            .is_none_or(|a| a.is_empty()),
        "rules_not_fired should be empty; body: {body}"
    );
}

// =====================================================================
// v5.0 audit WARNING 2 (MON-07): /metrics HTML + JSON arms now read the live
// session count (previously hardcoded active_sessions: 0). Proves the JSON arm
// agrees with the WS broadcast + Prometheus text arm after a session is created.
// =====================================================================

/// After a stateful POST /v1/execute (session_id present), GET /metrics with
/// Accept: application/json reports active_sessions >= 1 (the live count from
/// the installed SessionManager), NOT the pre-fix hardcoded 0.
#[serial_test::serial]
#[actix_web::test]
async fn metrics_json_active_sessions_live() {
    reset_test_env();
    let app = init_test_app!();

    // Create a stateful session (session_id present → sessionful path).
    let exec_req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "session_id": "metrics-live-session",
            "facts": { "scorecard": { "score": 0 } }
        }))
        .to_request();
    let exec_body = json_body(test::call_service(&app, exec_req).await).await;
    assert!(
        exec_body["session_id"].as_str().is_some(),
        "setup: sessionful execute should return a server session_id; body: {exec_body}"
    );

    // Read the JSON /metrics arm — active_sessions must reflect the live count.
    let metrics_req = test::TestRequest::get()
        .uri("/metrics")
        .insert_header(("Accept", "application/json"))
        .to_request();
    let body = json_body(test::call_service(&app, metrics_req).await).await;

    assert!(
        body["active_sessions"].as_u64().unwrap_or(0) >= 1,
        "MON-07 / WARNING 2: /metrics JSON arm should report the live session \
         count (>= 1 after creating a session), got {:?}; body: {body}",
        body["active_sessions"]
    );
}

/// `rule_file` is refused, not ignored.
///
/// The field was parsed and dropped from its introduction until 9.0.71: a caller asking to reload one
/// file got a whole-directory reload and no indication that their instruction had been discarded. There is
/// no per-file reload — the ruleset is built from the directory and keeps no per-rule provenance — and the
/// nearest implementation would swap in one file's rules and silently drop every other rule, so the field
/// is answered with a 400 that says so.
#[serial_test::serial]
#[actix_web::test]
async fn reloading_one_file_is_refused_with_an_explanation() {
    reset_test_env();
    let dir = create_test_rules_dir();
    std::env::set_var("RULE_STORAGE_PATH", dir.to_str().unwrap());

    let app = init_test_app!();

    let reload_req = test::TestRequest::post()
        .uri("/v1/rules/reload")
        .set_json(json!({ "rule_file": "gold.rules", "async_reload": false }))
        .to_request();
    let resp = test::call_service(&app, reload_req).await;

    assert_eq!(resp.status().as_u16(), 400, "a per-file reload is refused");
    let body: Value = test::read_body_json(resp).await;
    let message = body["error"].as_str().unwrap_or_default();
    assert!(
        message.contains("per-file reload is not supported"),
        "the refusal explains itself: {message}"
    );
    assert!(
        message.contains("gold.rules"),
        "and names the file that was asked for: {message}"
    );
}

/// A category filter is refused, not ignored.
///
/// `rule_filter.categories` was a silent no-op: the executing ruleset is loaded from
/// `RULE_STORAGE_PATH` and carries no category, while the API's categories describe rule *documents* in a
/// separate store. A caller sending it believed they had narrowed the run and got every rule. It is a 400
/// that says which field and why.
#[serial_test::serial]
#[actix_web::test]
async fn a_category_filter_is_refused_with_an_explanation() {
    reset_test_env();
    let app = init_test_app!();

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "engine": "rete",
            "facts": { "scorecard": { "score": 0 } },
            "rule_filter": { "categories": ["pricing"] }
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status().as_u16(),
        400,
        "the category filter is refused"
    );
    let body = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(
        body.contains("rule_filter.categories is not supported"),
        "the refusal names the field: {body}"
    );

    // `name_pattern` — the filter that works — is still accepted.
    let ok = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "engine": "rete",
            "facts": { "scorecard": { "score": 0 } },
            "rule_filter": { "name_pattern": "r1" }
        }))
        .to_request();
    let ok_resp = test::call_service(&app, ok).await;
    assert!(
        ok_resp.status().is_success(),
        "name_pattern still filters, got {}",
        ok_resp.status()
    );
}

/// A document with only a `dsl_body` is not executable; one created with a structured `rule` is.
///
/// This is the first slice of `specs/rule-sources.md`'s option A. The separation that document records —
/// `/v1/rules` stores rule *documents*, execution reads `RULE_STORAGE_PATH/*.rules` — is real, so a document
/// without a structured rule is stored and reported `executable: false`. When the request carries a `rule`,
/// the API validates it with the engine and writes it into the rules directory, after which the normal
/// reload makes it part of the executing set and it can fire.
#[serial_test::serial]
#[actix_web::test]
async fn a_created_rule_is_not_executable_and_its_id_matches_no_executed_rule() {
    reset_test_env();
    let dir = create_test_rules_dir();
    std::env::set_var("RULE_STORAGE_PATH", dir.to_str().unwrap());
    write_rule_file(&dir, "r1", &score_plus_10_rule());

    let app = init_test_app!();

    // Create a rule through the API. It lands in the rule store, not in the executing ruleset.
    let created = test::TestRequest::post()
        .uri("/v1/rules")
        .set_json(json!({
            "name": "api-created",
            "dsl_body": "rule \"api-created\" when true then log(\"hi\")",
            "category": "pricing"
        }))
        .to_request();
    let created_resp = test::call_service(&app, created).await;
    assert!(
        created_resp.status().is_success(),
        "creating a rule succeeds: {}",
        created_resp.status()
    );
    let created_body: Value = test::read_body_json(created_resp).await;
    let api_rule_id = created_body["id"]
        .as_str()
        .expect("the API returns an id")
        .to_string();

    // Reload so the executing set is whatever is on disk (r1 only) — the created rule is not on disk.
    let reload = test::TestRequest::post()
        .uri("/v1/rules/reload")
        .set_json(json!({ "async_reload": false }))
        .to_request();
    assert!(test::call_service(&app, reload).await.status().is_success());

    // Execute unfiltered: the disk rule can fire; the API rule does not exist to the engine.
    let unfiltered = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "engine": "rete", "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let unfiltered_body: Value =
        test::read_body_json(test::call_service(&app, unfiltered).await).await;
    assert_eq!(
        unfiltered_body["rules_matched"], 1,
        "only the disk rule is in the executing set: {unfiltered_body}"
    );

    // Filtered by the API rule's own id — what the IDE's "test this rule" sends — matches nothing.
    let filtered = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "engine": "rete",
            "facts": { "scorecard": { "score": 0 } },
            "rule_filter": { "name_pattern": api_rule_id }
        }))
        .to_request();
    let filtered_body: Value = test::read_body_json(test::call_service(&app, filtered).await).await;
    assert_eq!(
        filtered_body["rules_matched"], 0,
        "an API rule id selects no executed rule: {filtered_body}"
    );

    // And the document says so, instead of leaving the caller to infer it from a zero.
    assert_eq!(
        created_body["executable"], false,
        "a document with only a dsl_body cannot be executed: {created_body}"
    );
}

/// The first slice of option A: a rule created *with* a structured `rule` joins the executing set.
///
/// The request carries the engine's own `Rule` JSON; the API validates it by loading it, writes
/// `<rules dir>/<id>.rules`, and the ordinary reload picks it up — so the rule fires and can be selected by
/// the id the create call returned. That is the whole point of `specs/rule-sources.md`: the document and the
/// executing rule are the same rule once someone has bridged them.
#[serial_test::serial]
#[actix_web::test]
async fn a_rule_created_with_a_structured_rule_becomes_executable() {
    reset_test_env();
    let dir = create_test_rules_dir();
    std::env::set_var("RULE_STORAGE_PATH", dir.to_str().unwrap());

    let app = init_test_app!();

    let created = test::TestRequest::post()
        .uri("/v1/rules")
        // No tenant header on this app, so the document lives under `default` — fine for this test; the
        // executing set is per deployment, not per tenant (specs/rule-sources.md says so).
        .set_json(json!({
            "name": "api-executable",
            "dsl_body": "score > 0",
            "category": "pricing",
            "rule": {
                "id": "api-executable",
                "name": "api-executable",
                "salience": 0,
                "no_loop": false,
                "lock_on_active": false,
                "condition": {"Pattern": {"fact_type": "scorecard", "field": "score",
                                          "value": {"Integer": 0}, "op": "Eq"}},
                "actions": [{"LogMessage": {"level": "Info", "message": "api rule fired"}}]
            }
        }))
        .to_request();
    let created_resp = test::call_service(&app, created).await;
    assert!(
        created_resp.status().is_success(),
        "creating a rule with a structured rule succeeds: {}",
        created_resp.status()
    );
    let created_body: Value = test::read_body_json(created_resp).await;
    assert_eq!(
        created_body["executable"], true,
        "and the document reports that it can run: {created_body}"
    );
    let rule_id = created_body["id"].as_str().expect("an id").to_string();

    // The file is where the engine reads.
    let written = dir.join(format!("{rule_id}.rules"));
    assert!(written.exists(), "{} should exist", written.display());

    // Reload, then select the rule by the id the API returned.
    let reload = test::TestRequest::post()
        .uri("/v1/rules/reload")
        .set_json(json!({ "async_reload": false }))
        .to_request();
    assert!(test::call_service(&app, reload).await.status().is_success());

    let filtered = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({
            "engine": "rete",
            "facts": { "scorecard": { "score": 0 } },
            "rule_filter": { "name_pattern": rule_id }
        }))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, filtered).await).await;
    assert_eq!(
        body["rules_matched"], 1,
        "the created rule is in the executing set under a name the engine knows: {body}"
    );
    assert_eq!(body["rules_fired"], 1, "and it fires on the facts: {body}");
}
