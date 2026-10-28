//! The audit trail in PostgreSQL.
//!
//! `audit.rs`'s own tests prove the in-memory trail and the endpoints over it. These prove the other
//! backend: events recorded by a real mutation land in the table, the API reads them back, and they
//! outlive the process that wrote them — which is the property an audit trail is for.
//!
//! `make db-test` runs these against a PostgreSQL that `pg-embed` downloads and starts.

use actix_web::test;
use api::v1::audit_store::AuditBackend;
use pg_embed::pg_enums::PgAuthMethod;
use pg_embed::pg_fetch::{PgFetchSettings, PG_V17};
use pg_embed::postgres::{PgEmbed, PgSettings};
use std::path::PathBuf;
use std::time::Duration;

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    listener.local_addr().expect("local address").port()
}

/// A real server, and a trail connected to it.
async fn trail(tag: &str) -> (PgEmbed, AuditBackend) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("target")
        .join("db-test")
        .join(tag);
    let settings = PgSettings {
        database_dir: root,
        port: free_port(),
        user: "postgres".to_string(),
        password: "postgres".to_string(),
        auth_method: PgAuthMethod::Plain,
        persistent: false,
        timeout: Some(Duration::from_secs(60)),
        migration_dir: None,
    };
    let mut server = PgEmbed::new(
        settings,
        PgFetchSettings {
            version: PG_V17,
            ..Default::default()
        },
    )
    .await
    .expect("pg-embed is configured");
    server
        .setup()
        .await
        .expect("PostgreSQL downloads and initialises");
    server.start_db().await.expect("PostgreSQL starts");

    let trail = AuditBackend::Postgres(
        api::v1::audit_postgres::PostgresAuditTrail::connect(&server.db_uri)
            .await
            .expect("the trail connects and applies its schema"),
    );
    (server, trail)
}

#[actix_web::test]
#[ignore = "starts a real PostgreSQL; run `make db-test`"]
async fn a_mutation_is_recorded_in_postgres_and_read_back_through_the_api() {
    let (_server, trail) = trail("audit-http").await;
    let state = api::app::ApiState::for_testing(
        &std::env::temp_dir().join(format!("rrules-audit-pg-{}", std::process::id())),
    )
    .with_audit_backend(trail);
    let app = test::init_service(api::app::build_app(&state)).await;
    let login = test::TestRequest::post()
        .uri("/v1/auth/login")
        .set_json(serde_json::json!({"email": "admin", "password": "admin"}))
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, login).await;
    let token = body["token"].as_str().expect("a token").to_string();

    let create = test::TestRequest::post()
        .uri("/v1/rules")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("User-Agent", "audit-postgres-test/1.0"))
        .set_json(serde_json::json!({
            "name": "persisted-rule",
            "dsl_body": "rule \"p\" when true then log(\"ok\")",
        }))
        .to_request();
    let response = test::call_service(&app, create).await;
    assert!(
        response.status().is_success(),
        "creating the rule returned {}",
        response.status()
    );

    // The endpoint a reader calls shows it…
    let list = test::TestRequest::get()
        .uri("/v1/audit?page_size=50")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, list).await;
    assert!(
        body["events"].as_array().is_some_and(|events| events
            .iter()
            .any(|event| event["action"] == serde_json::json!("create"))),
        "the create should be in the trail: {body}"
    );

    // …and the same rows are in the table, which is what survives a restart.
    let stored: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_events WHERE action = 'create'")
            .fetch_one(state.audit_store.get_ref().pool_for_tests())
            .await
            .expect("the table answers");
    assert_eq!(stored, 1, "one create, one row");
}

#[actix_web::test]
#[ignore = "starts a real PostgreSQL; run `make db-test`"]
async fn the_trail_outlives_the_process_that_wrote_it() {
    let (server, trail) = trail("audit-restart").await;
    let state = api::app::ApiState::for_testing(
        &std::env::temp_dir().join(format!("rrules-audit-restart-{}", std::process::id())),
    )
    .with_audit_backend(trail);
    let app = test::init_service(api::app::build_app(&state)).await;
    let login = test::TestRequest::post()
        .uri("/v1/auth/login")
        .set_json(serde_json::json!({"email": "admin", "password": "admin"}))
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, login).await;
    let token = body["token"].as_str().expect("a token").to_string();

    let create = test::TestRequest::post()
        .uri("/v1/rules")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(serde_json::json!({
            "name": "survives-restart",
            "dsl_body": "rule \"r\" when true then log(\"ok\")",
        }))
        .to_request();
    assert!(test::call_service(&app, create).await.status().is_success());

    // A second application, a second connection, nothing shared but the database — which is what a
    // restart looks like.
    let second = AuditBackend::Postgres(
        api::v1::audit_postgres::PostgresAuditTrail::connect(&server.db_uri)
            .await
            .expect("the second trail connects"),
    );
    let count = second
        .count(&api::v1::audit_store::AuditFilter::default())
        .await
        .expect("the second trail counts");
    assert!(
        count >= 1,
        "an event recorded before the restart should still be there, found {count}"
    );
}
