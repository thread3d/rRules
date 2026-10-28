//! The tenant registry in PostgreSQL.
//!
//! The in-memory tests in `tenants.rs` prove the semantics; these prove the *other* backend answers
//! identically — trim, reject blanks and duplicates, list newest first, report the first delete and
//! not the second — and that a tenant created through the API is still there for a connection the
//! writer never shared.
//!
//! `make db-test` runs these against a PostgreSQL that `pg-embed` downloads and starts.

use actix_web::test;
use api::v1::tenant_store::TenantBackend;
use pg_embed::pg_enums::PgAuthMethod;
use pg_embed::pg_fetch::{PgFetchSettings, PG_V17};
use pg_embed::postgres::{PgEmbed, PgSettings};
use std::path::PathBuf;
use std::time::Duration;

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    listener.local_addr().expect("local address").port()
}

async fn registry(tag: &str) -> (PgEmbed, TenantBackend) {
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

    let backend = TenantBackend::Postgres(
        api::v1::tenant_postgres::PostgresTenantStore::connect(&server.db_uri)
            .await
            .expect("the registry connects and applies its schema"),
    );
    (server, backend)
}

#[actix_web::test]
#[ignore = "starts a real PostgreSQL; run `make db-test`"]
async fn the_postgres_registry_answers_like_the_memory_one() {
    let (_server, backend) = registry("tenant-semantics").await;
    assert_eq!(backend.kind(), "postgres");

    // Trimming, as the memory store does.
    let acme = backend.create("  Acme  ").await.expect("created");
    assert_eq!(acme.name, "Acme", "the stored name is trimmed");

    // A duplicate is a duplicate, not a second tenant.
    assert!(
        matches!(
            backend.create("Acme").await,
            Err(engine::governance::tenant::TenantStoreError::DuplicateName(
                _
            ))
        ),
        "a duplicate name is rejected by the unique constraint"
    );

    // A blank is rejected before it reaches the database.
    assert!(
        matches!(
            backend.create("   ").await,
            Err(engine::governance::tenant::TenantStoreError::EmptyName)
        ),
        "a blank name is rejected"
    );

    let beta = backend.create("Beta").await.expect("created");
    assert_eq!(
        backend
            .get(&acme.id)
            .await
            .expect("looked up")
            .map(|tenant| tenant.name),
        Some("Acme".to_string())
    );

    // Newest first.
    let listed = backend.list().await.expect("listed");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, beta.id);

    // First delete removes it, the second reports it was gone.
    assert!(backend.delete(&acme.id).await.expect("deleted"));
    assert!(!backend.delete(&acme.id).await.expect("deleted"));
    assert!(backend.get(&acme.id).await.expect("looked up").is_none());
}

#[actix_web::test]
#[ignore = "starts a real PostgreSQL; run `make db-test`"]
async fn a_tenant_created_through_the_api_is_still_there_for_a_later_connection() {
    let (server, backend) = registry("tenant-api").await;
    let state = api::app::ApiState::for_testing(
        &std::env::temp_dir().join(format!("rrules-tenant-pg-{}", std::process::id())),
    )
    .with_tenant_backend(backend);
    let app = test::init_service(api::app::build_app(&state)).await;

    let login = test::TestRequest::post()
        .uri("/v1/auth/login")
        .set_json(serde_json::json!({"email": "admin", "password": "admin"}))
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, login).await;
    let token = body["token"].as_str().expect("a token").to_string();

    let create = test::TestRequest::post()
        .uri("/v1/tenants")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(serde_json::json!({"name": "Persisted Ltd"}))
        .to_request();
    let response = test::call_service(&app, create).await;
    assert!(
        response.status().is_success(),
        "creating a tenant returned {}",
        response.status()
    );

    // The endpoint lists it…
    let list = test::TestRequest::get()
        .uri("/v1/tenants")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, list).await;
    assert!(
        body["tenants"].as_array().is_some_and(|tenants| tenants
            .iter()
            .any(|tenant| tenant["name"] == serde_json::json!("Persisted Ltd"))),
        "the new tenant should be listed: {body}"
    );

    // …and a second connection, sharing nothing but the database, sees it too.
    let later = TenantBackend::Postgres(
        api::v1::tenant_postgres::PostgresTenantStore::connect(&server.db_uri)
            .await
            .expect("the second registry connects"),
    );
    let names: Vec<String> = later
        .list()
        .await
        .expect("listed")
        .into_iter()
        .map(|tenant| tenant.name)
        .collect();
    assert!(
        names.contains(&"Persisted Ltd".to_string()),
        "a tenant created over HTTP should outlive the request, found {names:?}"
    );
}
