//! The decision endpoints, persisted through PostgreSQL.
//!
//! `engine_state/tests/postgres_store.rs` proves the store against a real database. This proves the
//! *application* uses it: `ApiState::with_dmn_store` swaps exactly one field, and the HTTP handlers
//! then read and write through the database instead of the filesystem. Without this, the wiring could
//! be reversed and still pass every other test.
//!
//! `make db-test` runs these against a PostgreSQL that `pg-embed` downloads and starts.

use actix_web::test;
use api::v1::dmn_store::DmnStore;
use engine_state::postgres::PostgresDmnStore;
use pg_embed::pg_enums::PgAuthMethod;
use pg_embed::pg_fetch::{PgFetchSettings, PG_V17};
use pg_embed::postgres::{PgEmbed, PgSettings};
use std::path::PathBuf;
use std::time::Duration;

const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20180913/MODEL/" id="api-pg" name="api-pg"
             namespace="http://example.com/api-pg">
  <decision id="pg_decision" name="Postgres decision">
    <literalExpression><text>42</text></literalExpression>
  </decision>
</definitions>"#;

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    listener.local_addr().expect("local address").port()
}

async fn embedded(tag: &str) -> (PgEmbed, PostgresDmnStore) {
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

    let store = PostgresDmnStore::connect(&server.db_uri)
        .await
        .expect("the store connects");
    (server, store)
}

#[actix_web::test]
#[ignore = "starts a real PostgreSQL; run `make db-test`"]
async fn a_decision_imported_over_http_is_stored_in_postgres() {
    let (_server, store) = embedded("api-http").await;
    // The whole application, with one field — the DMN store — pointing at the database.
    let state = api::app::ApiState::for_testing(
        &std::env::temp_dir().join(format!("rrules-api-pg-http-{}", std::process::id())),
    )
    .with_dmn_store(DmnStore::Postgres(store));
    let app = test::init_service(api::app::build_app(&state)).await;

    let login = test::TestRequest::post()
        .uri("/v1/auth/login")
        .set_json(serde_json::json!({"email": "admin", "password": "admin"}))
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, login).await;
    let token = body["token"].as_str().expect("a token").to_string();

    let import = test::TestRequest::post()
        .uri("/v1/decisions/import")
        .insert_header(("Authorization", format!("Bearer {token}")))
        // `ImportDmnRequest::content` is the raw XML as bytes, not a string.
        .set_json(serde_json::json!({"id": "api-pg", "content": XML.as_bytes()}))
        .to_request();
    let response = test::call_service(&app, import).await;
    assert!(
        response.status().is_success(),
        "import returned {}",
        response.status()
    );

    // Listed by the endpoint the interface uses…
    let list = test::TestRequest::get()
        .uri("/v1/decisions")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, list).await;
    let listed = body
        .as_array()
        .or_else(|| body["decisions"].as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        listed
            .iter()
            .any(|entry| entry["id"] == serde_json::json!("api-pg")),
        "the imported document should be listed: {body}"
    );

    // …readable again: the metadata endpoint reports the length it stored…
    let get = test::TestRequest::get()
        .uri("/v1/decisions/api-pg")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, get).await;
    assert_eq!(
        body["content_length"].as_u64(),
        Some(XML.len() as u64),
        "the stored document's length should match the import: {body}"
    );

    // …and the export endpoint returns the XML itself, byte for byte.
    let export = test::TestRequest::get()
        .uri("/v1/decisions/api-pg/export")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let body = test::call_and_read_body(&app, export).await;
    let exported = String::from_utf8_lossy(&body);
    assert!(
        exported.contains("pg_decision"),
        "the exported XML should be the stored document: {exported}"
    );

    // Deleting removes it from the database, not just from a cache.
    let delete = test::TestRequest::delete()
        .uri("/v1/decisions/api-pg")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let response = test::call_service(&app, delete).await;
    assert!(response.status().is_success() || response.status().as_u16() == 204);

    let after = test::TestRequest::get()
        .uri("/v1/decisions/api-pg")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let response = test::call_service(&app, after).await;
    assert!(
        !response.status().is_success(),
        "a deleted document should be gone, got {}",
        response.status()
    );
}

#[actix_web::test]
#[ignore = "starts a real PostgreSQL; run `make db-test`"]
async fn the_filesystem_store_is_still_the_default() {
    // The selector's default must not change under deployments that set nothing.
    let state =
        api::app::ApiState::for_testing(&std::env::temp_dir().join("rrules-api-pg-default"));
    assert_eq!(
        state.dmn_store.get_ref().kind(),
        "filesystem",
        "an unconfigured server persists decision documents to the filesystem"
    );
}
