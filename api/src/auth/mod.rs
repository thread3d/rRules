//! Pragmatic auth: `/v1/auth/login`, `/v1/auth/me`, `/v1/auth/refresh` (D-22-01).
//!
//! Signs HS256 JWTs with the existing `API_SECRET_KEY` (HMAC) and validates
//! them in `crate::middleware::auth`. The frontend `auth-context.tsx` consumes
//! `POST /v1/auth/login` → `{token, user}`, `GET /v1/auth/me` → `User`, and
//! `POST /v1/auth/refresh` → `{token}`. Passwords are plaintext for the demo
//! (D-22-01 permits); production hashing is deferred to v5.0.

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};

pub mod store;

pub use store::{find_user, init_store, UserRecord, UserStore};

// `decode_token` lives in `crate::middleware::auth` — the JWT validation
// concern belongs at the enforcement boundary (per plan 22-02 key_links:
// `requires_auth → decode::<Claims>(HS256, API_SECRET_KEY)`). The handlers
// below call it via `crate::middleware::auth::decode_token`.
use crate::middleware::auth::decode_token;

/// JWT claims. `sub` is the user id (== seeded username), `role` is one of
/// admin|author|reviewer|viewer, `exp`/`iat` are UTC seconds.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Claims {
    pub sub: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
    /// The tenant the token was issued for.
    ///
    /// `Option` with a serde default so tokens minted before 9.0.76 still decode; a token without one
    /// falls back to the `X-Tenant-ID` header, which is what every request used before this existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
}

/// User object returned to the frontend — mirrors the `User` interface in
/// `frontend/src/types/auth.ts` (`id`, `email`, `displayName`, `role`,
/// `permissions`). `#[serde(rename)]` matches the camelCase frontend contract.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub role: String,
    pub permissions: Vec<String>,
}

impl From<&UserRecord> for UserResponse {
    fn from(u: &UserRecord) -> Self {
        Self {
            id: u.id.clone(),
            email: u.email.clone(),
            display_name: u.display_name.clone(),
            role: u.role.clone(),
            permissions: u.permissions.clone(),
        }
    }
}

/// Login request. Accepts EITHER `username` (plan behavior tests) OR `email`
/// (frontend `AuthCredentials` = `{email, password}`) so both callers work.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct LoginRequest {
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RefreshResponse {
    pub token: String,
}

/// JWT lifetime: 1 hour (planner-chosen default per RESEARCH §Pattern 2).
const TOKEN_TTL_SECONDS: usize = 3600;

/// Read the HMAC signing secret. Falls back to an insecure dev default with a
/// warning if `API_SECRET_KEY` is unset — reuses the Phase 21 Helm secret
/// placeholder. Never hand-roll HMAC (V6 cryptography). Public so the
/// middleware (`crate::middleware::auth`) can reuse the same secret for
/// `decode_token`.
pub fn secret_key() -> Vec<u8> {
    match std::env::var("API_SECRET_KEY") {
        Ok(k) if !k.is_empty() => k.into_bytes(),
        _ => {
            tracing::warn!(
                "API_SECRET_KEY not set; using insecure dev default. \
                 Set API_SECRET_KEY in production (Phase 21 Helm secret)."
            );
            b"rrules-dev-secret-change-me".to_vec()
        }
    }
}

/// Sign a JWT (HS256) for the given claims. `Header::default()` uses HS256.
pub fn encode_token(claims: &Claims) -> Result<String, jsonwebtoken::errors::Error> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(&secret_key()),
    )
}

fn build_claims(user: &UserRecord) -> Claims {
    let now = Utc::now().timestamp() as usize;
    Claims {
        sub: user.id.clone(),
        role: user.role.clone(),
        exp: now + TOKEN_TTL_SECONDS,
        iat: now,
        tenant_id: Some(user.tenant_id.clone()),
    }
}

fn extract_bearer(req: &HttpRequest) -> Option<String> {
    let header = req.headers().get("Authorization")?;
    let s = header.to_str().ok()?;
    s.strip_prefix("Bearer ").map(|t| t.trim().to_string())
}

/// POST /v1/auth/login — validate credentials and return a signed JWT + user.
#[utoipa::path(
    post,
    path = "/v1/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = LoginResponse),
        (status = 401, description = "Invalid credentials"),
    ),
)]
pub async fn login(req: web::Json<LoginRequest>, store: web::Data<UserStore>) -> impl Responder {
    let identifier = req
        .username
        .clone()
        .or_else(|| req.email.clone())
        .unwrap_or_default();
    let user = match find_user(&store, &identifier) {
        Some(u) => u,
        None => {
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "message": "Invalid credentials"
            }));
        }
    };
    // Plaintext compare (D-22-01 demo only; v5.0 → argon2/bcrypt).
    if user.password != req.password {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "message": "Invalid credentials"
        }));
    }
    let claims = build_claims(&user);
    let token = match encode_token(&claims) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "failed to sign JWT");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "message": "Failed to issue token"
            }));
        }
    };
    HttpResponse::Ok().json(LoginResponse {
        token,
        user: UserResponse::from(&user),
    })
}

/// GET /v1/auth/me — resolve the current user from a Bearer token.
#[utoipa::path(
    get,
    path = "/v1/auth/me",
    tag = "auth",
    responses(
        (status = 200, description = "Current user", body = UserResponse),
        (status = 401, description = "Missing or invalid token"),
    ),
)]
pub async fn me(req: HttpRequest, store: web::Data<UserStore>) -> impl Responder {
    let token = match extract_bearer(&req) {
        Some(t) => t,
        None => {
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "message": "Missing or invalid authorization token"
            }));
        }
    };
    let claims = match decode_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "message": "Invalid or expired token"
            }));
        }
    };
    let users = match store.lock() {
        Ok(u) => u,
        Err(_) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "message": "Store unavailable"
            }));
        }
    };
    match users.get(&claims.sub) {
        Some(user) => HttpResponse::Ok().json(UserResponse::from(user)),
        None => HttpResponse::Unauthorized().json(serde_json::json!({
            "message": "User not found"
        })),
    }
}

/// POST /v1/auth/refresh — issue a fresh JWT from a valid Bearer token.
#[utoipa::path(
    post,
    path = "/v1/auth/refresh",
    tag = "auth",
    responses(
        (status = 200, description = "Fresh token", body = RefreshResponse),
        (status = 401, description = "Missing or invalid token"),
    ),
)]
pub async fn refresh(req: HttpRequest) -> impl Responder {
    let token = match extract_bearer(&req) {
        Some(t) => t,
        None => {
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "message": "Missing or invalid authorization token"
            }));
        }
    };
    let mut claims = match decode_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "message": "Invalid or expired token"
            }));
        }
    };
    let now = Utc::now().timestamp() as usize;
    claims.iat = now;
    claims.exp = now + TOKEN_TTL_SECONDS;
    match encode_token(&claims) {
        Ok(t) => HttpResponse::Ok().json(RefreshResponse { token: t }),
        Err(e) => {
            tracing::error!(error = %e, "failed to re-sign JWT");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "message": "Failed to issue token"
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // `decode_token` now lives in the middleware module (enforcement boundary).
    use crate::middleware::auth::decode_token;

    #[test]
    fn roundtrip_token_validates_with_hs256() {
        let claims = Claims {
            sub: "admin".into(),
            role: "admin".into(),
            exp: (Utc::now().timestamp() as usize) + 60,
            iat: Utc::now().timestamp() as usize,
            tenant_id: Some("default".to_string()),
        };
        let token = encode_token(&claims).expect("encode");
        let decoded = decode_token(&token).expect("decode");
        assert_eq!(decoded.sub, "admin");
        assert_eq!(decoded.role, "admin");
    }

    #[test]
    fn decode_rejects_wrong_secret() {
        let claims = Claims {
            sub: "admin".into(),
            role: "admin".into(),
            exp: (Utc::now().timestamp() as usize) + 60,
            iat: Utc::now().timestamp() as usize,
            tenant_id: Some("default".to_string()),
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"some-other-secret"),
        )
        .unwrap();
        assert!(decode_token(&token).is_err());
    }
}
