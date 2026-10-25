//! Integration tests for the upgraded `requires_auth` middleware (plan 22-02
//! Task 2, D-22-01, D-22-02, T-22-01, T-22-02).
//!
//! Mirrors `main.rs` by wrapping the `/v1` scope with
//! `actix_web::middleware::from_fn(requires_auth)` and exercising every
//! behavior case: a protected `/v1/*` route without a token returns 401; with
//! a valid HS256 JWT returns 2xx; with a wrong-secret token returns 401; with
//! an `alg:none` (tampered) token returns 401 (T-22-01); and the public
//! allowlist (`/v1/auth/login`, `/metrics`, `/health`) returns 2xx without a
//! token (D-22-02). A bonus test proves the middleware inserts `Claims` (with
//! the resolved role) into request extensions.
//!
//! `test::init_service` is inlined per test so the service type is inferred
//! at each call site (an explicit return type on an `async fn` helper breaks
//! `call_service` type inference).

// Requirements covered by this suite: API-001

use actix_web::middleware::from_fn;
use actix_web::{test, web, App, HttpRequest, HttpResponse, HttpMessage, Responder};
use api::auth::{self, init_store, Claims};
use api::middleware::auth::requires_auth;
use serde_json::Value;

/// A protected route that just returns 200 — proves the middleware let the
/// request through. No store dependency, so the test stays hermetic.
async fn protected_ok() -> impl Responder {
    HttpResponse::Ok().body("ok")
}

/// A protected route that echoes the resolved role from request extensions —
/// proves the middleware inserts `Claims`.
async fn whoami(req: HttpRequest) -> impl Responder {
    match req.extensions().get::<Claims>() {
        Some(c) => HttpResponse::Ok().json(serde_json::json!({
            "sub": c.sub,
            "role": c.role,
        })),
        None => HttpResponse::Ok().json(serde_json::json!({
            "sub": serde_json::Value::Null,
            "role": serde_json::Value::Null,
        })),
    }
}

/// Configure the test app to mirror `main.rs`: `/v1` scope wrapped with
/// `requires_auth` (login public via allowlist bypass; protected + whoami
/// behind the middleware), plus root-level `/metrics` and `/health`.
fn configure_protected_app(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .wrap(from_fn(requires_auth))
            .route("/auth/login", web::post().to(auth::login))
            .route("/auth/me", web::get().to(auth::me))
            .route("/auth/refresh", web::post().to(auth::refresh))
            .route("/protected", web::get().to(protected_ok))
            .route("/whoami", web::get().to(whoami)),
    );
    // Root-level public routes (outside the /v1 scope → never wrapped).
    cfg.route("/metrics", web::get().to(|| async { HttpResponse::Ok().body("metrics") }));
    cfg.route("/health", web::get().to(|| async { HttpResponse::Ok().body("health") }));
}

fn valid_token(sub: &str, role: &str) -> String {
    let now = chrono::Utc::now().timestamp() as usize;
    auth::encode_token(&Claims {
        sub: sub.to_string(),
        role: role.to_string(),
        exp: now + 3600,
        iat: now,
    })
    .expect("encode valid token")
}

/// A token signed with a DIFFERENT secret — must be rejected (T-22-01).
fn wrong_secret_token(sub: &str, role: &str) -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: sub.to_string(),
        role: role.to_string(),
        exp: now + 3600,
        iat: now,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(b"this-is-not-the-api-secret"),
    )
    .expect("encode wrong-secret token")
}

/// A token whose header is tampered to `alg:none` (no signature). Must be
/// rejected because `decode_token` pins HS256 (T-22-01).
fn alg_none_token(sub: &str, role: &str) -> String {
    let now = chrono::Utc::now().timestamp() as usize;
    let header = serde_json::json!({"alg":"none","typ":"JWT"});
    let payload = serde_json::json!({
        "sub": sub,
        "role": role,
        "exp": now + 3600,
        "iat": now,
    });
    let h = b64url_encode(serde_json::to_string(&header).unwrap().as_bytes());
    let p = b64url_encode(serde_json::to_string(&payload).unwrap().as_bytes());
    // alg:none → empty signature segment.
    format!("{h}.{p}.")
}

/// base64url encode (no padding) — self-contained so the test stays
/// dependency-free.
fn b64url_encode(input: &[u8]) -> String {
    const CHARS: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(CHARS[(b0 >> 2) as usize] as char);
        out.push(CHARS[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(CHARS[(b2 & 0x3f) as usize] as char);
        }
    }
    out
}

async fn json_body(resp: actix_web::dev::ServiceResponse) -> Value {
    let bytes = test::read_body(resp).await;
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        panic!(
            "invalid JSON: {e}; body={}",
            String::from_utf8_lossy(&bytes)
        )
    })
}

#[actix_web::test]
async fn protected_route_without_token_returns_401() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_protected_app),
    )
    .await;
    let req = test::TestRequest::get().uri("/v1/protected").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401, "no-token request must be 401");
}

#[actix_web::test]
async fn protected_route_with_valid_token_returns_200() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_protected_app),
    )
    .await;
    let token = valid_token("admin", "admin");
    let req = test::TestRequest::get()
        .uri("/v1/protected")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200, "valid-token request must pass");
    let bytes = test::read_body(resp).await;
    assert_eq!(&bytes[..], b"ok");
}

#[actix_web::test]
async fn protected_route_with_wrong_secret_token_returns_401() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_protected_app),
    )
    .await;
    let token = wrong_secret_token("admin", "admin");
    let req = test::TestRequest::get()
        .uri("/v1/protected")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401, "wrong-secret token must be 401");
}

#[actix_web::test]
async fn protected_route_with_alg_none_token_returns_401() {
    // T-22-01: an alg:none (tampered-header) token must be rejected because
    // decode_token pins the algorithm to HS256.
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_protected_app),
    )
    .await;
    let token = alg_none_token("admin", "admin");
    let req = test::TestRequest::get()
        .uri("/v1/protected")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401, "alg:none token must be 401");
}

#[actix_web::test]
async fn public_allowlist_routes_return_200_without_token() {
    // D-22-02: public routes must NOT require a token.
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_protected_app),
    )
    .await;

    // /v1/auth/login — allowlist bypass inside the middleware.
    let req = test::TestRequest::post()
        .uri("/v1/auth/login")
        .set_json(serde_json::json!({"username": "admin", "password": "admin"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200, "login must be public (allowlist)");

    // /metrics — root-level, outside the wrapped /v1 scope.
    let req = test::TestRequest::get().uri("/metrics").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200, "metrics must be public");

    // /health — root-level, outside the wrapped /v1 scope.
    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200, "health must be public");
}

#[actix_web::test]
async fn refresh_requires_a_valid_token() {
    // /v1/auth/refresh is NOT in the allowlist → must require a token.
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_protected_app),
    )
    .await;
    let req = test::TestRequest::post().uri("/v1/auth/refresh").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401, "refresh must require a token");
}

#[actix_web::test]
async fn valid_token_inserts_role_into_request_extensions() {
    // Bonus: the middleware resolves Claims and inserts them into extensions
    // so handlers can read the role (D-22-02 groundwork for v5.0 role gating).
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_protected_app),
    )
    .await;
    let token = valid_token("author", "author");
    let req = test::TestRequest::get()
        .uri("/v1/whoami")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;
    assert_eq!(body["sub"], "author");
    assert_eq!(body["role"], "author");
}