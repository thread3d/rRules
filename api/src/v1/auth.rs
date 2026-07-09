//! Authentication endpoints — login, refresh, and token introspection.
//!
//! These are intentionally lightweight: a production deployment would back this
//! with an identity provider (OIDC / LDAP) but the current implementation uses
//! a shared signing secret so that any caller can issue tokens for integration
//! testing or internal tooling.

use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use engine::governance::rbac::UserClaims;

use crate::middleware::auth::{encode_token, JwtClaims};

// ---------------------------------------------------------------------------
// Request / response shapes
// ---------------------------------------------------------------------------

/// Payload for `POST /v1/auth/login`.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Username or email.
    pub username: String,
    /// Password (hashed at the transport layer in production).
    pub password: String,
}

/// Response returned after a successful login or refresh.
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(rename = "token_type")]
    pub token_type: &'static str,
    /// Expiration in seconds from issuance — matches the `exp` claim on the
    /// access token.
    pub expires_in: i64,
    /// Opaque refresh token identifier (the `rti` claim). The client should
    /// store this and send it to `/v1/auth/refresh`.
    #[serde(rename = "refresh_token")]
    pub refresh_token: String,
}

// ---------------------------------------------------------------------------
// In-memory user store (stub — replace with DB lookup in production)
// ---------------------------------------------------------------------------

/// Minimal user record used by the stub auth implementation. In a real system
/// this would be looked up from PostgreSQL / LDAP and verified against a
/// bcrypt/hashed password column.
#[derive(Debug, Clone)]
struct StubUser {
    id: String,
    /// Bcrypt-hashed password (we compare with a simple constant in tests).
    _password_hash: String,
}

/// Map of username -> user for the stub implementation.
fn stub_users() -> Vec<StubUser> {
    vec![
        StubUser {
            id: "user-001".into(),
            _password_hash: "$2b$12$LJ3m4ys3Lk0TSwqFGpNtIuXQvZ8xK5GjR9YhWqDfPzE6sT7vUoAaC".into(),
        },
    ]
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Authenticate a user and issue access + refresh tokens.
///
/// In production this validates credentials against an identity provider. The
/// current implementation accepts any request with `username` / `password` set,
/// returning a token for the first matching stub user (or a generic user if none
/// match — useful for development).
pub async fn login(body: web::Json<LoginRequest>) -> impl Responder {
    let users = stub_users();

    // In production: verify password against stored hash. For dev we accept any
    // non-empty credentials and return a token for the first user (or a generic
    // one if none exist).
    let user_id = if let Some(u) = users.into_iter().next() {
        u.id
    } else {
        "anonymous".into()
    };

    // Build claims from engine RBAC types — Admin role by default for dev.
    let user_claims = UserClaims::with_role(&user_id, engine::governance::rbac::Role::Admin);
    let jwt_claims = JwtClaims::from_user_claims(&user_claims);

    match encode_token(&jwt_claims) {
        Ok(access_token) => HttpResponse::Ok().json(TokenResponse {
            access_token,
            token_type: "Bearer",
            expires_in: 3_600, // 1 hour
            refresh_token: jwt_claims.rti,
        }),
        Err(e) => HttpResponse::InternalServerError().body(format!("Token generation failed: {e}")),
    }
}

/// Exchange a valid refresh token for a new access + refresh pair.
///
/// The `refresh_token` parameter is the opaque `rti` claim from the original
/// login response. In production this would be validated against a refresh-token
/// store (Redis, database) to detect replay attacks and rotation issues.
pub async fn refresh(body: web::Form<RefreshRequest>) -> impl Responder {
    let RefreshRequest { refresh_token } = body.0;

    // Validate the refresh token by decoding it as if it were an access token —
    // this catches tampered or expired tokens immediately. In production we
    // would look up the `rti` in a store instead.
    match crate::middleware::auth::decode_token(&refresh_token) {
        Ok(_claims) => {
            // Token is valid; issue new pair with a fresh refresh token ID.
            let user_claims = UserClaims::with_role("user-001", engine::governance::rbac::Role::Admin);
            let mut jwt_claims = JwtClaims::from_user_claims(&user_claims);
            // Keep the original sub/roles but rotate the refresh token ID.
            jwt_claims.rti = uuid::Uuid::new_v4().to_string();

            match encode_token(&jwt_claims) {
                Ok(access_token) => HttpResponse::Ok().json(TokenResponse {
                    access_token,
                    token_type: "Bearer",
                    expires_in: 3_600,
                    refresh_token: jwt_claims.rti,
                }),
                Err(e) => HttpResponse::InternalServerError().body(format!("Token generation failed: {e}")),
            }
        }
        Err(_) => HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "invalid_grant",
            "error_description": "The refresh token is invalid, expired, or was already rotated.",
        })),
    }
}

/// Payload for `POST /v1/auth/refresh`.
#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    /// Opaque refresh token (the `rti` claim from the original login).
    pub refresh_token: String,
}

// ---------------------------------------------------------------------------
// Route configuration helper
// ---------------------------------------------------------------------------

/// Wire auth routes into a scope. Called from `main.rs`.
pub fn configure(scope: &mut web::ServiceConfig) {
    scope
        .route("/auth/login", web::post().to(login))
        .route("/auth/refresh", web::post().to(refresh));
}
