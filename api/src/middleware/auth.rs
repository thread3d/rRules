//! JWT validation middleware for protected `/v1/*` routes (D-22-01, T-22-01,
//! T-22-02).
//!
//! Upgraded from the prior presence-only stub (which only checked the `Bearer`
//! prefix and was never `wrap()`ed) to real HS256 JWT validation + role
//! resolution. Wrap it with `actix_web::middleware::from_fn(requires_auth)`.
//!
//! - **T-22-01 (alg-confusion):** `decode_token` pins the algorithm to HS256
//!   via `Validation::new(Algorithm::HS256)`; `jsonwebtoken` rejects `alg:none`
//!   and wrong-secret tokens. Do NOT use `Validation::default()` without
//!   pinning.
//! - **T-22-02 (missing auth on mutating routes):** protected `/v1/*` routes
//!   are wrapped; an explicit public allowlist (login, /health, /swagger-ui,
//!   /api-docs, /metrics) is bypassed so public endpoints stay reachable.
//! - **D-22-02:** authenticated-vs-public only for v4.0. Per-route role
//!   enforcement (viewer read-only, admin full, etc.) is deferred to v5.0 —
//!   the frontend `usePermission` already gates the UI by role.

use actix_web::body::{BoxBody, MessageBody};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;
use actix_web::{Error, HttpResponse, HttpMessage};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

use crate::auth::{secret_key, Claims};

/// Validate a JWT and return its claims. **Pins the algorithm to HS256**
/// (T-22-01): `Validation::new(Algorithm::HS256)` rejects `alg:none` and
/// wrong-secret tokens and validates `exp` by default. Do NOT use
/// `Validation::default()` without pinning. Lives in the middleware module so
/// the enforcement boundary owns the validation (plan 22-02 key_links:
/// `requires_auth → decode::<Claims>(HS256, API_SECRET_KEY)`).
pub fn decode_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(&secret_key()),
        &Validation::new(Algorithm::HS256),
    )?;
    Ok(data.claims)
}

/// Public routes that must be reachable WITHOUT a valid JWT (D-22-02
/// allowlist). The middleware is wrapped on the `/v1` scope, so only
/// `/v1/auth/login` is strictly needed here; the root-level entries are
/// included for robustness if the middleware is ever applied to a broader
/// scope.
///
/// `/v1/metrics/ws` is public because a browser `new WebSocket()` cannot
/// attach a Bearer Authorization header (the WS handshake is a plain HTTP
/// upgrade), and the metrics stream is read-only — the server broadcasts
/// `MetricsSnapshot`; no user input is accepted (Plan 22-04 threat model). Without
/// this entry the `/v1` auth wrap added in Plan 22-02 would 401 the WS upgrade.
const PUBLIC_PREFIXES: &[&str] = &[
    "/health",
    "/swagger-ui",
    "/api-docs",
    "/v1/auth/login",
    "/v1/metrics/ws",
    "/metrics",
];

/// True if the request path is in the public allowlist (no token required).
fn is_public(path: &str) -> bool {
    PUBLIC_PREFIXES.iter().any(|p| path.starts_with(p))
}

/// Extract the Bearer token from the Authorization header.
fn bearer_token(req: &ServiceRequest) -> Option<String> {
    let header = req.headers().get("Authorization")?;
    let s = header.to_str().ok()?;
    s.strip_prefix("Bearer ").map(|t| t.trim().to_string())
}

/// Build a 401 response carrying a JSON `{message}` body, consuming the
/// request. Used for all auth-failure short-circuits.
fn unauthorized(req: ServiceRequest, msg: &'static str) -> ServiceResponse<BoxBody> {
    req.into_response(
        HttpResponse::Unauthorized().json(serde_json::json!({ "message": msg })),
    )
    .map_into_boxed_body()
}

/// JWT validation middleware. Wrap with
/// `actix_web::middleware::from_fn(requires_auth)` on a scope.
///
/// - Bypasses the public allowlist (D-22-02).
/// - Validates the Bearer token as an HS256 JWT (T-22-01: algorithm pinned in
///   `decode_token` above; `alg:none` and wrong-secret tokens are rejected).
/// - On success, inserts the resolved `Claims` into request extensions so
///   handlers can read the role (`req.extensions().get::<Claims>()`).
/// - On any failure, returns 401.
pub async fn requires_auth(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    // Public allowlist bypass (D-22-02).
    if is_public(req.path()) {
        return next.call(req).await.map(|r| r.map_into_boxed_body());
    }

    let token = match bearer_token(&req) {
        Some(t) => t,
        None => return Ok(unauthorized(req, "Missing or invalid authorization token")),
    };

    match decode_token(&token) {
        Ok(claims) => {
            // Make the resolved role available to handlers via extensions.
            req.extensions_mut().insert(claims);
            next.call(req).await.map(|r| r.map_into_boxed_body())
        }
        Err(_) => Ok(unauthorized(req, "Invalid or expired token")),
    }
}