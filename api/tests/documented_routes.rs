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

/// Routes the OpenAPI document cannot express, each with the reason. An exemption is a statement
/// about the formats, not a way to silence the check: the user guide still documents all of them.
const OPENAPI_EXEMPT: &[(&str, &str)] = &[
    (
        "GET /live",
        "the same handler serves /health/live; one operation, one path",
    ),
    (
        "GET /ready",
        "the same handler serves /health/ready; one operation, one path",
    ),
    (
        "PATCH /v1/rules/{id}",
        "an alias of PUT /v1/rules/{id}; OpenAPI has one operation per path and method",
    ),
    (
        "GET /v1/metrics/ws",
        "a WebSocket upgrade, which OpenAPI 3.0's operation model cannot describe",
    ),
    (
        "GET /v1/metrics/dmn/ws",
        "a WebSocket upgrade, which OpenAPI 3.0's operation model cannot describe",
    ),
];

/// The generated OpenAPI document must describe the routes the server actually serves.
///
/// `/api-docs/openapi.json` is what a client generator reads, and it is assembled from a hand-written
/// list in `api/src/openapi.rs` — a list that can fall behind the router exactly like prose can. This
/// compares it against `docs/API.md`, which the test above already proves matches the router.
#[actix_web::test]
async fn the_openapi_document_describes_the_documented_routes() {
    let root = std::env::temp_dir().join("rrules-openapi-routes");
    std::fs::create_dir_all(&root).expect("temp dir");
    let state = api::app::ApiState::for_testing(&root);
    let app = test::init_service(api::app::build_app(&state)).await;

    let request = test::TestRequest::get()
        .uri("/api-docs/openapi.json")
        .to_request();
    let document: serde_json::Value = test::call_and_read_body_json(&app, request).await;
    let paths = document["paths"]
        .as_object()
        .expect("the document has a paths object");

    // The document speaks OpenAPI's `{parameter}` syntax; the user guide uses the same shape.
    let mut missing = Vec::new();
    for (method, uri) in documented_endpoints() {
        let path = uri.split('?').next().unwrap_or(&uri).to_string();
        let described = paths
            .get(&path)
            .map(|entry| entry.get(method.as_str().to_lowercase()).is_some())
            .unwrap_or(false);
        let key = format!("{method} {path}");
        if !described && !OPENAPI_EXEMPT.iter().any(|(exempt, _)| *exempt == key) {
            missing.push(key);
        }
    }

    assert!(
        missing.is_empty(),
        "docs/API.md documents routes the OpenAPI document does not describe:\n  {}",
        missing.join("\n  ")
    );
}

/// Every exemption must still be a route the server serves, or the list is stale.
#[actix_web::test]
async fn the_openapi_exemptions_are_real_routes() {
    let documented: Vec<String> = documented_endpoints()
        .into_iter()
        .map(|(method, uri)| format!("{method} {uri}"))
        .collect();

    for (exempt, reason) in OPENAPI_EXEMPT {
        assert!(
            documented.iter().any(|documented| documented == exempt),
            "exemption `{exempt}` ({reason}) does not name a route docs/API.md documents"
        );
    }
}
