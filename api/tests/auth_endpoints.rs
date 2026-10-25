//! Integration tests for `/v1/auth/*` (plan 22-02 Task 1, D-22-01).
//!
//! Covers login (valid + wrong password + all four roles + email-field),
//! me (valid + no token), and refresh (fresh exp). Mirrors `main.rs` by
//! registering the `/v1` scope with the auth routes against an in-memory user
//! store. The `requires_auth` middleware is NOT wrapped here — that is
//! exercised by `auth_protection.rs` (Task 2); these tests target the handlers
//! directly.

use actix_web::{test, web, App};
use api::auth::{self, init_store, Claims};
use serde_json::Value;

/// Configure the `/v1/auth/*` routes on an App — used so each test can call
/// `test::init_service` and let the service type be inferred (an explicit
/// return type on an `async fn` helper breaks `call_service` type inference).
fn configure_auth_v1(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/auth/login", web::post().to(auth::login))
            .route("/auth/me", web::get().to(auth::me))
            .route("/auth/refresh", web::post().to(auth::refresh)),
    );
}

async fn json_body(resp: actix_web::dev::ServiceResponse) -> Value {
    assert!(
        resp.status().is_success(),
        "expected success, got {}",
        resp.status()
    );
    let bytes = test::read_body(resp).await;
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        panic!(
            "invalid JSON: {e}; body={}",
            String::from_utf8_lossy(&bytes)
        )
    })
}

/// Decode the JWT payload (middle segment) without verifying — for assertions.
fn decode_payload(token: &str) -> Value {
    let segs: Vec<&str> = token.split('.').collect();
    assert_eq!(segs.len(), 3, "expected 3 JWT segments");
    let bytes = base64url_decode(segs[1]).expect("payload base64url");
    serde_json::from_slice(&bytes).expect("payload JSON")
}

/// base64url decode (no padding) — self-contained so the tests stay
/// dependency-free.
fn base64url_decode(input: &str) -> Result<Vec<u8>, &'static str> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let bytes: Vec<u8> = input.bytes().collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let val = |b: u8| CHARS.iter().position(|&c| c == b).map(|i| i as u8);
    for chunk in bytes.chunks(4) {
        let v: Vec<u8> = chunk.iter().map(|b| val(*b).unwrap_or(0)).collect();
        out.push((v[0] << 2) | (v.get(1).copied().unwrap_or(0) >> 4));
        if chunk.len() > 2 {
            out.push((v[1] << 4) | (v.get(2).copied().unwrap_or(0) >> 2));
        }
        if chunk.len() > 3 {
            out.push((v[2] << 6) | v[3]);
        }
    }
    Ok(out)
}

#[actix_web::test]
async fn login_valid_credentials_returns_token_and_user() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_auth_v1),
    )
    .await;
    let req = test::TestRequest::post()
        .uri("/v1/auth/login")
        .set_json(serde_json::json!({"username": "admin", "password": "admin"}))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert!(body["token"].is_string(), "token present: {body}");
    let token = body["token"].as_str().unwrap();
    let claims = decode_payload(token);
    assert_eq!(claims["sub"], "admin");
    assert_eq!(claims["role"], "admin");
    assert!(claims["exp"].as_u64().unwrap() > claims["iat"].as_u64().unwrap());

    assert_eq!(body["user"]["id"], "admin");
    assert_eq!(body["user"]["role"], "admin");
    assert_eq!(body["user"]["displayName"], "admin");
}

#[actix_web::test]
async fn login_wrong_password_returns_401() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_auth_v1),
    )
    .await;
    let req = test::TestRequest::post()
        .uri("/v1/auth/login")
        .set_json(serde_json::json!({"username": "admin", "password": "wrong"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[actix_web::test]
async fn login_with_email_field_works_for_frontend_contract() {
    // Frontend AuthCredentials = {email, password}; the store resolves
    // `admin@rrules.dev` as well as the bare username `admin`.
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_auth_v1),
    )
    .await;
    let req = test::TestRequest::post()
        .uri("/v1/auth/login")
        .set_json(serde_json::json!({"email": "admin@rrules.dev", "password": "admin"}))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;
    assert_eq!(body["user"]["id"], "admin");
}

#[actix_web::test]
async fn me_with_valid_token_returns_user() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_auth_v1),
    )
    .await;
    let token = auth::encode_token(&Claims {
        sub: "author".into(),
        role: "author".into(),
        exp: (chrono::Utc::now().timestamp() as usize) + 600,
        iat: chrono::Utc::now().timestamp() as usize,
    })
    .unwrap();

    let req = test::TestRequest::get()
        .uri("/v1/auth/me")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;
    assert_eq!(body["id"], "author");
    assert_eq!(body["role"], "author");
}

#[actix_web::test]
async fn me_without_token_returns_401() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_auth_v1),
    )
    .await;
    let req = test::TestRequest::get().uri("/v1/auth/me").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[actix_web::test]
async fn refresh_with_valid_token_returns_fresh_token() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_auth_v1),
    )
    .await;
    let now = chrono::Utc::now().timestamp() as usize;
    let old_token = auth::encode_token(&Claims {
        sub: "reviewer".into(),
        role: "reviewer".into(),
        exp: now + 10,
        iat: now - 60,
    })
    .unwrap();

    let req = test::TestRequest::post()
        .uri("/v1/auth/refresh")
        .insert_header(("Authorization", format!("Bearer {old_token}")))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert!(body["token"].is_string(), "new token present: {body}");
    let new_claims = decode_payload(body["token"].as_str().unwrap());
    assert_eq!(new_claims["sub"], "reviewer");
    assert_eq!(new_claims["role"], "reviewer");
    assert!(new_claims["exp"].as_u64().unwrap() > new_claims["iat"].as_u64().unwrap());
}

#[actix_web::test]
async fn all_four_roles_can_login_and_role_matches_token() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(init_store()))
            .configure(configure_auth_v1),
    )
    .await;
    for role in ["admin", "author", "reviewer", "viewer"] {
        let req = test::TestRequest::post()
            .uri("/v1/auth/login")
            .set_json(serde_json::json!({"username": role, "password": role}))
            .to_request();
        let body = json_body(test::call_service(&app, req).await).await;
        assert_eq!(body["user"]["role"], role, "role {role}");
        let claims = decode_payload(body["token"].as_str().unwrap());
        assert_eq!(claims["role"], role, "token role {role}");
    }
}
