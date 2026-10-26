//! Every endpoint `docs/API.md` documents must exist.
//!
//! The user-facing reference is prose, and prose rots: an endpoint can be renamed, moved under a
//! different scope, or never have been registered, while the document keeps promising it. This
//! test reads the document, drives each documented route against the real application (built
//! in-process by `build_app`, so no socket is involved), and fails when the router does not know
//! the route.
//!
//! Two signals identify a route that does not exist:
//!
//! - under `/v1`, `/metrics`, `/api-docs` or `/swagger-ui` the SPA answers
//!   `{"error":"no such endpoint: …"}` (see `api/src/spa.rs`);
//! - at the root the SPA answers with the application shell, which is HTML.
//!
//! Anything else — including a `404` from a handler that looked for an id it did not find — proves
//! the route is registered, which is what this test asserts.

use actix_web::http::Method;
use actix_web::test;

/// `## GET /v1/rules` headings are how the reference presents an endpoint.
fn documented_endpoints() -> Vec<(Method, String)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("API.md");
    let text = std::fs::read_to_string(&path).expect("docs/API.md");
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("## ") else {
            continue;
        };
        let mut parts = rest.split_whitespace();
        let (Some(method), Some(uri)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Ok(method) = Method::from_bytes(method.as_bytes()) else {
            continue;
        };
        if !uri.starts_with('/') {
            continue;
        }
        out.push((method, uri.to_string()));
    }
    out
}

/// Documented paths carry placeholders (`{id}`, `{name}`); the router matches any segment.
fn concrete(uri: &str) -> String {
    uri.replace("{id}", "00000000-0000-0000-0000-000000000000")
        .replace("{name}", "main")
        .replace("{tag_name}", "example")
        .replace("{approval_id}", "00000000-0000-0000-0000-000000000000")
        .replace("{job_id}", "00000000-0000-0000-0000-000000000000")
}

fn looks_like_missing_route(uri: &str, body: &str) -> bool {
    if body.contains("no such endpoint") {
        return true;
    }
    // A root-level path the router does not know falls through to the SPA shell.
    !uri.starts_with("/v1")
        && uri != "/metrics"
        && (body.contains("<!doctype html") || body.trim_start().starts_with("<html"))
}

#[actix_web::test]
async fn documented_endpoints_are_registered() {
    let root = std::env::temp_dir().join("rrules-documented-routes");
    std::fs::create_dir_all(&root).expect("temp dir");
    let state = api::app::ApiState::for_testing(&root);
    let app = test::init_service(api::app::build_app(&state)).await;

    let login = test::TestRequest::post()
        .uri("/v1/auth/login")
        .set_json(serde_json::json!({"email": "admin", "password": "admin"}))
        .to_request();
    let token: serde_json::Value = test::call_and_read_body_json(&app, login).await;
    let token = token["token"].as_str().expect("a token").to_string();

    let endpoints = documented_endpoints();
    assert!(
        endpoints.len() > 20,
        "the document should list many endpoints, found {}",
        endpoints.len()
    );

    let mut missing = Vec::new();
    for (method, uri) in endpoints {
        let path = concrete(&uri);
        let mut request = test::TestRequest::default()
            .method(method.clone())
            .uri(&path)
            .insert_header(("Authorization", format!("Bearer {token}")));
        if matches!(method, Method::POST | Method::PUT | Method::PATCH) {
            request = request.set_json(serde_json::json!({}));
        }
        let response = test::call_service(&app, request.to_request()).await;
        let status = response.status();
        let body = String::from_utf8_lossy(&test::read_body(response).await).to_string();
        if looks_like_missing_route(&path, &body) {
            missing.push(format!("{method} {uri} (status {status}, {})", body.trim()));
        }
    }

    assert!(
        missing.is_empty(),
        "docs/API.md documents endpoints the router does not serve:\n  {}",
        missing.join("\n  ")
    );
}
