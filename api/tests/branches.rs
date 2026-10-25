//! Integration tests for the /v1/branches endpoints (Plan 22-06).
//!
//! Covers the P-5 regression guard (two sequential POST /v1/branches with
//! different `name` values create two DISTINCT branches — guards against the
//! prior `create_branch` hardcode of `"feature/default"`) and the T-22-04
//! branch-name sanitization (path separators / `..` / `refs/` prefixes /
//! whitespace / empty are rejected with 400 before reaching libgit2).
//!
//! Routes are registered WITHOUT the project-wide `requires_auth` middleware
//! so these tests stay focused on the branches handler contract; auth
//! protection is exercised by `auth_protection.rs`.

use actix_web::{body::BoxBody, dev::ServiceResponse, test, web, App};
use serde_json::{json, Value};

use api::v1::branches::{
    create_branch, delete_branch, get_branch, get_git_log, list_branches, merge_branch,
};
use engine::governance::versioning::{VcsConfig, VersionControlRepo};

/// Create a fresh bare git repo in a unique temp dir and wrap it in the
/// `web::Data<Mutex<VersionControlRepo>>` shape the handlers expect.
fn fresh_vcs() -> web::Data<tokio::sync::Mutex<VersionControlRepo>> {
    let mut unique = String::from("rRules-branches-it-");
    unique.push_str(&uuid::Uuid::new_v4().simple().to_string());
    let dir = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let config = VcsConfig::new(dir.to_str().expect("utf8 temp path"));
    let repo_path = config.repo_path.clone();
    let repo = VersionControlRepo::open_or_init(&repo_path, config).expect("open_or_init");
    web::Data::new(tokio::sync::Mutex::from(repo))
}

/// Initialize a test App with the branch routes mounted under `/v1`. No auth
/// middleware — exercised elsewhere. Inlined as a macro so the `App<...>`
/// service-factory type is inferred at the call site (matching the pattern in
/// `metrics_integration.rs`).
macro_rules! init_branch_app {
    ($vcs:expr) => {
        test::init_service(
            App::new()
                .app_data($vcs)
                .route("/v1/branches", web::get().to(list_branches))
                .route("/v1/branches", web::post().to(create_branch))
                .route("/v1/branches/{name}", web::get().to(get_branch))
                .route("/v1/branches/{name}", web::delete().to(delete_branch))
                .route("/v1/branches/{name}/merge", web::post().to(merge_branch))
                .route("/v1/rules/{id}/git-log", web::get().to(get_git_log)),
        )
        .await
    };
}

async fn body_json(resp: ServiceResponse<BoxBody>) -> Value {
    let bytes = test::read_body(resp).await;
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        panic!(
            "invalid JSON body: {e}; body={}",
            String::from_utf8_lossy(&bytes)
        )
    })
}

/// P-5 regression guard: two distinct `name` values create two distinct branches.
#[actix_web::test]
async fn create_branch_two_distinct_names() {
    let vcs = fresh_vcs();
    let app = init_branch_app!(vcs);

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/branches")
            .set_json(json!({"name":"branch-a","source_branch":"main"}))
            .to_request(),
    )
    .await;
    assert_eq!(
        resp.status().as_u16(),
        201,
        "first create should be 201 (got {:?})",
        resp.status()
    );

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/branches")
            .set_json(json!({"name":"branch-b","source_branch":"main"}))
            .to_request(),
    )
    .await;
    assert_eq!(
        resp.status().as_u16(),
        201,
        "second create should be 201 — P-5 regression (got {:?})",
        resp.status()
    );

    // List and verify both distinct branches exist.
    let resp = test::call_service(
        &app,
        test::TestRequest::get().uri("/v1/branches").to_request(),
    )
    .await;
    assert_eq!(resp.status().as_u16(), 200);
    let body = body_json(resp).await;
    let names: Vec<String> = body["branches"]
        .as_array()
        .expect("branches array")
        .iter()
        .map(|b| b["name"].as_str().expect("name").to_string())
        .collect();
    assert!(
        names.contains(&"branch-a".to_string()) && names.contains(&"branch-b".to_string()),
        "expected both branch-a and branch-b, got {names:?}"
    );
}

/// POST /v1/branches WITHOUT a `name` field returns 400 — name is required.
#[actix_web::test]
async fn create_branch_requires_name() {
    let vcs = fresh_vcs();
    let app = init_branch_app!(vcs);

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/branches")
            .set_json(json!({"source_branch":"main"}))
            .to_request(),
    )
    .await;
    let status = resp.status().as_u16();
    assert!(
        status == 400 || status == 422,
        "missing name should be rejected (got {status})"
    );
}

/// T-22-04: malicious branch names are rejected before reaching libgit2.
#[actix_web::test]
async fn create_branch_rejects_malicious_names() {
    let vcs = fresh_vcs();
    let app = init_branch_app!(vcs);

    let bad_names = vec![
        "../escape",
        "refs/heads/evil",
        "..",
        "has space",
        "",
        "trailing ",
        " leading",
    ];

    for name in bad_names {
        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/branches")
                .set_json(json!({"name": name, "source_branch":"main"}))
                .to_request(),
        )
        .await;
        let status = resp.status().as_u16();
        assert!(
            status == 400 || status == 422,
            "malicious name {:?} should be rejected (got {status})",
            name
        );
    }
}

/// Merge endpoint exists and is wired (smoke): creating a branch then merging
/// returns one of the documented merge statuses (200 clean / 409 conflict /
/// 500 infra). This does not assert success — the bare-repo checkout/merge
/// path is exercised end-to-end by the engine's own versioning tests; here we
/// only confirm the route is reachable and responds with a documented status.
#[actix_web::test]
async fn merge_endpoint_is_reachable() {
    let vcs = fresh_vcs();
    let app = init_branch_app!(vcs);

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/branches")
            .set_json(json!({"name":"feature-clean","source_branch":"main"}))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status().as_u16(), 201);

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/branches/feature-clean/merge")
            .to_request(),
    )
    .await;
    let status = resp.status().as_u16();
    assert!(
        matches!(status, 200 | 409 | 500),
        "merge should return a documented status (200/409/500); got {status}"
    );
}
