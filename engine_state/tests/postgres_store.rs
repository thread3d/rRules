//! The PostgreSQL store, against a real PostgreSQL.
//!
//! `pg-embed` downloads a PostgreSQL build and runs it in userspace — no Docker, no root, no
//! preinstalled server — so these tests exercise real SQL against a real database rather than a
//! mock. They are marked `#[ignore]` because the first run downloads ~30 MB and starts a server, and
//! a gate that every `cargo test` pays for would be run less often, not more.
//!
//! `make db-test` runs them. The binary cache lives in `XDG_CACHE_HOME`, so in this workspace it must
//! point inside the tree: `XDG_CACHE_HOME=$PWD/.db-cache make db-test`.

#![cfg(feature = "postgres")]

use chrono::Utc;
use engine_state::postgres::PostgresDmnStore;
use engine_state::{DmnDocument, DmnPersistence, PersistenceError};
use pg_embed::pg_enums::PgAuthMethod;
use pg_embed::pg_fetch::{PgFetchSettings, PG_V17};
use pg_embed::postgres::{PgEmbed, PgSettings};
use std::path::PathBuf;
use std::time::Duration;

/// A free port, asked of the operating system: two test runs must not fight over 5432.
fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    listener.local_addr().expect("local address").port()
}

/// Start a real server, connect the store to it, and hand both back. The `PgEmbed` must outlive the
/// store: dropping it stops the server.
async fn store(tag: &str) -> (PgEmbed, PostgresDmnStore) {
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
        .expect("the store connects and applies its schema");
    (server, store)
}

fn document(id: &str, xml: &str) -> DmnDocument {
    DmnDocument {
        id: id.to_string(),
        name: format!("model {id}"),
        content: xml.as_bytes().to_vec(),
        metadata: serde_json::json!({ "version": "1", "author": "test" }),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

const XML: &str = r#"<?xml version="1.0"?><definitions id="d" name="d" namespace="http://x"/>"#;

#[tokio::test]
#[ignore = "starts a real PostgreSQL; run `make db-test`"]
async fn a_document_round_trips_through_the_database() {
    let (_server, store) = store("round-trip").await;

    let created = store
        .create(document("discount", XML))
        .await
        .expect("the document is inserted");
    assert_eq!(created.id, "discount");

    let fetched = store
        .get("discount")
        .await
        .expect("the document is read back");
    assert_eq!(
        fetched.content,
        XML.as_bytes(),
        "the XML must round-trip byte for byte"
    );
    assert_eq!(fetched.metadata["version"], serde_json::json!("1"));

    let listed = store.list().await.expect("the document is listed");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "discount");
    assert_eq!(listed[0].version.as_deref(), Some("1"));
}

#[tokio::test]
#[ignore = "starts a real PostgreSQL; run `make db-test`"]
async fn updates_replace_content_and_deletes_are_final() {
    let (_server, store) = store("updates").await;
    store
        .create(document("model", XML))
        .await
        .expect("inserted");

    let mut changed = document("model", "<definitions id=\"changed\"/>");
    changed.name = "renamed".to_string();
    let updated = store
        .update("model", changed)
        .await
        .expect("the document is updated");
    assert_eq!(updated.name, "renamed");
    assert_eq!(
        store.get("model").await.expect("read back").content,
        "<definitions id=\"changed\"/>".as_bytes()
    );

    store
        .delete("model")
        .await
        .expect("the document is deleted");
    assert!(matches!(
        store.get("model").await,
        Err(PersistenceError::NotFound(_))
    ));
    assert!(
        matches!(
            store.delete("model").await,
            Err(PersistenceError::NotFound(_))
        ),
        "deleting twice reports NotFound rather than succeeding"
    );
    assert!(
        matches!(
            store.update("model", document("model", XML)).await,
            Err(PersistenceError::NotFound(_))
        ),
        "updating a document that is gone reports NotFound"
    );
}

#[tokio::test]
#[ignore = "starts a real PostgreSQL; run `make db-test`"]
async fn creating_an_id_that_exists_is_a_conflict_not_an_overwrite() {
    let (_server, store) = store("conflict").await;
    store
        .create(document("dup", XML))
        .await
        .expect("first insert");

    match store
        .create(document("dup", "<definitions id=\"other\"/>"))
        .await
    {
        Err(PersistenceError::Conflict(message)) => {
            assert!(
                message.contains("dup"),
                "the message names the id: {message}"
            )
        }
        other => panic!("expected a conflict, got {other:?}"),
    }

    let stored = store.get("dup").await.expect("still readable");
    assert_eq!(
        stored.content,
        XML.as_bytes(),
        "the first document survived"
    );
}

#[tokio::test]
#[ignore = "starts a real PostgreSQL; run `make db-test`"]
async fn files_are_saved_and_loaded_as_bytes() {
    let (_server, store) = store("files").await;
    let payload = vec![0u8, 159, 146, 150, 255];

    store
        .save_file("binary.dmn", &payload)
        .await
        .expect("saved");
    assert_eq!(
        store.load_file("binary.dmn").await.expect("loaded"),
        payload,
        "arbitrary bytes survive the round trip"
    );

    // Saving the same name replaces the content rather than failing.
    store
        .save_file("binary.dmn", b"replaced")
        .await
        .expect("re-saved");
    assert_eq!(
        store.load_file("binary.dmn").await.expect("loaded"),
        b"replaced".to_vec()
    );

    assert!(matches!(
        store.load_file("absent.dmn").await,
        Err(PersistenceError::NotFound(_))
    ));
}

#[tokio::test]
#[ignore = "starts a real PostgreSQL; run `make db-test`"]
async fn listing_is_most_recent_first() {
    let (_server, store) = store("ordering").await;

    let mut older = document("older", XML);
    older.updated_at = Utc::now() - chrono::Duration::hours(2);
    let mut newer = document("newer", XML);
    newer.updated_at = Utc::now();

    store.create(older).await.expect("older inserted");
    store.create(newer).await.expect("newer inserted");

    let listed = store.list().await.expect("listed");
    let ids: Vec<&str> = listed.iter().map(|summary| summary.id.as_str()).collect();
    assert_eq!(ids, vec!["newer", "older"]);
}
