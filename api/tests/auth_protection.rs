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
use actix_web::{test, web, App, HttpMessage, HttpRequest, HttpResponse, Responder};
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
    cfg.route(
        "/metrics",
        web::get().to(|| async { HttpResponse::Ok().body("metrics") }),
    );
    cfg.route(
        "/health",
        web::get().to(|| async { HttpResponse::Ok().body("health") }),
    );
}

fn valid_token(sub: &str, role: &str) -> String {
    let now = chrono::Utc::now().timestamp() as usize;
    auth::encode_token(&Claims {
        sub: sub.to_string(),
        role: role.to_string(),
        exp: now + 3600,
        iat: now,
        tenant_id: Some("default".to_string()),
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
        tenant_id: Some("default".to_string()),
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
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
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
    assert_eq!(
        resp.status().as_u16(),
        401,
        "wrong-secret token must be 401"
    );
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
    assert_eq!(
        resp.status().as_u16(),
        200,
        "login must be public (allowlist)"
    );

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
    let req = test::TestRequest::post()
        .uri("/v1/auth/refresh")
        .to_request();
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

/// For a non-admin, the tenant comes from the token — not from a header the caller can set.
///
/// Before 9.0.76 tenant identity existed only as `X-Tenant-ID`, so any authenticated caller could read
/// another tenant's rules by sending a different value — the specification said as much for several
/// releases. A non-admin's token now pins the tenant and the header is ignored; an `admin` may still select
/// a tenant by header, which is the "manages several tenants" flow the isolation requirement describes.
#[actix_web::test]
async fn a_token_tenant_overrides_a_forged_header() {
    use api::auth::{encode_token, Claims};

    let forged = Claims {
        sub: "author".to_string(),
        role: "author".to_string(),
        exp: (chrono::Utc::now().timestamp() as usize) + 3600,
        iat: chrono::Utc::now().timestamp() as usize,
        tenant_id: Some("acme".to_string()),
    };
    let token = encode_token(&forged).expect("token");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(api::v1::rules::init_store()))
            .app_data(web::Data::new(api::auth::store::init_store()))
            .app_data(web::Data::new(api::v1::audit::init_store()))
            .route("/rules", web::post().to(api::v1::rules::create_rule))
            .route("/rules", web::get().to(api::v1::rules::get_rules)),
    )
    .await;

    // Created with the token's tenant, while the header claims another one.
    let create = test::TestRequest::post()
        .uri("/rules")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("X-Tenant-ID", "someone-else"))
        .set_json(serde_json::json!({
            "name": "acme-only",
            "dsl_body": "score > 0",
            "category": "pricing"
        }))
        .to_request();
    let created = test::call_service(&app, create).await;
    assert!(
        created.status().is_success(),
        "create: {}",
        created.status()
    );

    // The document is visible to the token's tenant...
    let own = test::TestRequest::get()
        .uri("/rules?page=1&page_size=10")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let own_body: serde_json::Value =
        test::read_body_json(test::call_service(&app, own).await).await;
    assert_eq!(
        own_body["total_count"], 1,
        "acme sees its own rule: {own_body}"
    );

    // ...and not to a caller claiming a different tenant without a token.
    let other = test::TestRequest::get()
        .uri("/rules?page=1&page_size=10")
        .insert_header(("X-Tenant-ID", "someone-else"))
        .to_request();
    let other_body: serde_json::Value =
        test::read_body_json(test::call_service(&app, other).await).await;
    assert_eq!(
        other_body["total_count"], 0,
        "another tenant cannot see it by forging the header: {other_body}"
    );
}

/// The admin isolation flow that `API-003`'s scenario describes: an admin creates a rule for one tenant and
/// cannot see it while acting for another.
///
/// This is the counterexample CodeWitness produced against 9.0.76 (`201|1` where `201|0` was required): the
/// scenario logs in **once** as `admin` and switches tenants with the header. Pinning every caller to the
/// token's tenant broke that flow, which is why the rule is split by role — the token pins a non-admin, an
/// admin selects the tenant.
#[actix_web::test]
async fn an_admin_can_act_for_one_tenant_and_not_see_another() {
    use api::auth::{encode_token, Claims};

    let admin = Claims {
        sub: "admin".to_string(),
        role: "admin".to_string(),
        exp: (chrono::Utc::now().timestamp() as usize) + 3600,
        iat: chrono::Utc::now().timestamp() as usize,
        tenant_id: Some("default".to_string()),
    };
    let token = encode_token(&admin).expect("token");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(api::v1::rules::init_store()))
            .app_data(web::Data::new(api::auth::store::init_store()))
            .app_data(web::Data::new(api::v1::audit::init_store()))
            .route("/rules", web::post().to(api::v1::rules::create_rule))
            .route("/rules", web::get().to(api::v1::rules::get_rules)),
    )
    .await;

    let create = test::TestRequest::post()
        .uri("/rules")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("X-Tenant-ID", "tenant-a"))
        .set_json(serde_json::json!({ "name": "scenario-rule", "dsl_body": "rule r" }))
        .to_request();
    let created = test::call_service(&app, create).await;
    assert_eq!(
        created.status().as_u16(),
        201,
        "the admin creates for tenant-a"
    );

    let list_other = test::TestRequest::get()
        .uri("/rules?page=1&page_size=10")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("X-Tenant-ID", "tenant-b"))
        .to_request();
    let other: serde_json::Value =
        test::read_body_json(test::call_service(&app, list_other).await).await;
    assert_eq!(
        other["total_count"], 0,
        "tenant-b does not see tenant-a's rule: {other}"
    );

    let list_own = test::TestRequest::get()
        .uri("/rules?page=1&page_size=10")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .insert_header(("X-Tenant-ID", "tenant-a"))
        .to_request();
    let own: serde_json::Value =
        test::read_body_json(test::call_service(&app, list_own).await).await;
    assert_eq!(own["total_count"], 1, "tenant-a does see it: {own}");
}
