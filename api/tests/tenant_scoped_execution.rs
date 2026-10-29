//! Execution is scoped to the caller's tenant.
//!
//! Before 9.0.77 `SharedRuleSet` was one flat set for the whole deployment, so every tenant executed every
//! tenant's rules — which is why 9.0.75 had to refuse executable rules from a non-default tenant rather than
//! let them leak. The rules directory now carries ownership in its layout
//! (`RULE_STORAGE_PATH/<tenant>/<file>.rules`, with root files belonging to `default`), and
//! `POST /v1/execute` selects the caller's share.

use actix_web::{test, web};
use api::metrics::create_metrics_channel;
use api::v1::execution::{execute_rules, init_counters, SharedRuleSet};
use api::v1::sessions::{read_session_config, SessionManager};
use serde_json::{json, Value};

const RULE_TEMPLATE: &str = r#"{"rules":[{"id":"TENANT-RULE","name":"TENANT-RULE","salience":0,
 "no_loop":false,"lock_on_active":false,
 "condition":{"Pattern":{"fact_type":"scorecard","field":"score","value":{"Integer":0},"op":"Eq"}},
 "actions":[{"LogMessage":{"level":"Info","message":"fired"}}]}]}"#;

fn write_rule(dir: &std::path::Path, tenant: Option<&str>, file: &str, rule_id: &str) {
    let mut path = dir.to_path_buf();
    if let Some(tenant) = tenant {
        path = path.join(tenant);
    }
    std::fs::create_dir_all(&path).expect("tenant dir");
    std::fs::write(
        path.join(format!("{file}.rules")),
        RULE_TEMPLATE.replace("TENANT-RULE", rule_id),
    )
    .expect("rule file");
}

/// A session gauge on its own registry: registering the same metric name twice on one registry panics.
fn build_session_gauge() -> prometheus::IntGauge {
    let gauge = prometheus::IntGauge::new("rrules_sessions_active_tenant_test", "test gauge")
        .expect("gauge");
    prometheus::Registry::new()
        .register(Box::new(gauge.clone()))
        .expect("register");
    gauge
}

macro_rules! app_for {
    ($dir:expr) => {{
        let dir = $dir;
        std::env::set_var("RULE_STORAGE_PATH", dir.to_str().unwrap());
        let (ruleset, _tenants) =
            api::v1::execution::build_tenant_ruleset_from_dir(dir.to_str().unwrap());
        let shared = std::sync::Arc::new(std::sync::RwLock::new(SharedRuleSet::from_loaded(
            ruleset,
            dir.to_str().unwrap(),
        )));
        let mut registry = engine::engine::EngineRegistry::new();
        registry.register_default_engines();
        let (metrics_tx, _rx) = create_metrics_channel(16);

        actix_web::test::init_service(
            actix_web::App::new()
                .app_data(web::Data::new(shared))
                .app_data(web::Data::new(registry))
                .app_data(web::Data::new(
                    engine::engine::EngineSelector::with_defaults(),
                ))
                .app_data(web::Data::new(init_counters()))
                .app_data(web::Data::new(metrics_tx))
                .app_data(web::Data::new(std::sync::Arc::new(SessionManager::new(
                    read_session_config(),
                    build_session_gauge(),
                ))))
                .route("/execute", web::post().to(execute_rules)),
        )
        .await
    }};
}

macro_rules! execute_with_tenant {
    ($app:expr, $tenant:expr) => {{
        let req = actix_web::test::TestRequest::post()
            .uri("/execute")
            .insert_header(("X-Tenant-ID", $tenant))
            .set_json(serde_json::json!({
                "engine": "rete",
                "facts": { "scorecard": { "score": 0 } }
            }))
            .to_request();
        let body: serde_json::Value =
            actix_web::test::read_body_json(actix_web::test::call_service(&$app, req).await).await;
        body
    }};
}

#[actix_web::test]
async fn each_tenant_runs_only_its_own_rules() {
    let dir = std::env::temp_dir().join(format!("rrules-tenant-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("dir");
    write_rule(&dir, Some("acme"), "acme-rule", "acme-rule");
    write_rule(&dir, Some("globex"), "globex-rule", "globex-rule");
    write_rule(&dir, None, "default-rule", "default-rule");

    let app = app_for!(&dir);

    let acme = execute_with_tenant!(&app, "acme");
    assert_eq!(
        acme["rules_matched"], 1,
        "acme sees exactly its own rule: {acme}"
    );
    assert_eq!(acme["rules_fired"], 1, "and runs it: {acme}");

    let globex = execute_with_tenant!(&app, "globex");
    assert_eq!(
        globex["rules_matched"], 1,
        "globex sees exactly its own: {globex}"
    );

    let root = execute_with_tenant!(&app, "default");
    assert_eq!(
        root["rules_matched"], 1,
        "a file in the rules directory root belongs to the default tenant: {root}"
    );
}

#[actix_web::test]
async fn a_tenant_with_no_rules_matches_nothing() {
    let dir = std::env::temp_dir().join(format!("rrules-tenant-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("dir");
    write_rule(&dir, Some("acme"), "acme-rule", "acme-rule");

    let app = app_for!(&dir);
    let other = execute_with_tenant!(&app, "someone-else");

    assert_eq!(
        other["rules_matched"], 0,
        "nothing of acme's leaks: {other}"
    );
    assert_eq!(other["rules_fired"], 0);
    assert!(
        other["rules_not_fired"]
            .as_array()
            .expect("array")
            .is_empty(),
        "and no rule ids from another tenant are reported: {other}"
    );
}

#[actix_web::test]
async fn a_named_rule_is_still_scoped_to_the_caller() {
    let dir = std::env::temp_dir().join(format!("rrules-tenant-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("dir");
    write_rule(&dir, Some("acme"), "acme-rule", "acme-rule");

    let app = app_for!(&dir);

    // Asking for acme's rule id as another tenant selects nothing, even though the rule exists.
    let req = test::TestRequest::post()
        .uri("/execute")
        .insert_header(("X-Tenant-ID", "globex"))
        .set_json(json!({
            "engine": "rete",
            "facts": { "scorecard": { "score": 0 } },
            "rule_filter": { "name_pattern": "acme-rule" }
        }))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(
        body["rules_matched"], 0,
        "the name filter stays inside the tenant: {body}"
    );
}
