---
phase: 22-close-v4-0-frontend-integration-gaps-wire-orphaned-ui-to-bac
plan: "02"
subsystem: api
tags: [auth, jwt, jsonwebtoken, hs256, requires_auth, middleware, rbac, gov-01]

# Dependency graph
requires:
  - phase: 22-close-v4-0-frontend-integration-gaps-wire-orphaned-ui-to-bac
    provides: "Plan 22-01: /v1 path alignment + CORS + metrics split (the /v1 scope this plan registers auth routes inside)"
provides:
  - "/v1/auth/login, /v1/auth/me, /v1/auth/refresh endpoints signed with HS256 JWTs (API_SECRET_KEY)"
  - "In-memory user store seeded with 4 demo users (admin/author/reviewer/viewer) + per-role permissions"
  - "Upgraded requires_auth middleware — real HS256 JWT validation + role resolution, wrap()'ed on /v1 with a public allowlist bypass"
  - "Claims (sub, role, exp, iat) inserted into request extensions for v5.0 per-route role gating"
affects: [22-03 (frontend AuthProvider already targets /v1/auth/*; usePermission gating mounts here), 22-04, 22-05, 22-06, v5.0 (full RBAC: Postgres store, argon2/bcrypt, refresh-token rotation, per-route role enforcement)]

# Tech tracking
tech-stack:
  added: ["jsonwebtoken 10.4 (rust_crypto feature)"]
  patterns:
    - "Pattern: HS256 JWT encode in auth domain (auth/mod.rs::encode_token), decode at the enforcement boundary (middleware/auth.rs::decode_token) so Validation::new(Algorithm::HS256) pins the algorithm where requests are gated (T-22-01)"
    - "Pattern: actix from_fn middleware — requires_auth(req, next: Next<impl MessageBody + 'static>) returning Result<ServiceResponse<BoxBody>, Error>; public-allowlist bypass INSIDE the middleware (D-22-02)"
    - "Pattern: in-memory UserStore (Arc<Mutex<HashMap>>) following the existing init_store() convention; plaintext passwords per D-22-01 (demo only; hashing deferred to v5.0)"
    - "Pattern: LoginRequest accepts username OR email so the frontend AuthCredentials {email,password} and plan behavior tests {username,password} both resolve against the same store"

key-files:
  created:
    - api/src/auth/mod.rs
    - api/src/auth/store.rs
    - api/tests/auth_endpoints.rs
    - api/tests/auth_protection.rs
  modified:
    - api/Cargo.toml
    - api/src/lib.rs
    - api/src/main.rs
    - api/src/middleware/auth.rs
    - Cargo.lock

key-decisions:
  - "Moved decode_token into middleware/auth.rs (not auth/mod.rs) so the HS256 algorithm pinning lives at the enforcement boundary — matches the plan key_links ('requires_auth -> decode::<Claims>(HS256, API_SECRET_KEY)') and satisfies the T-22-01 acceptance grep on middleware/auth.rs. encode_token + Claims stay in auth/mod.rs (auth domain); the handlers call crate::middleware::auth::decode_token."
  - "Enabled jsonwebtoken's rust_crypto feature (pure-Rust HMAC/SHA2). jsonwebtoken 10.4 requires an explicit CryptoProvider; rust_crypto auto-selects one so HS256 works without install_default(). The plan said `jsonwebtoken = \"10.4\"` — the feature gate is the minimum change to make it functional."
  - "LoginRequest accepts both username and email (Option fields). The frontend AuthCredentials type is {email,password}; the plan behavior tests use {username,password}. find_user resolves by username OR email so both callers work against the same demo store."
  - "UserResponse uses #[serde(rename = \"displayName\")] to match the frontend User interface (camelCase) in frontend/src/types/auth.ts; permissions are surfaced per role so the frontend usePermission can gate the UI end-to-end."
  - "Wrapped the /v1 scope (not the whole App) so root-level public routes (/metrics, /health, /swagger-ui, /api-docs, fs::Files) stay naturally public; the allowlist bypass inside the middleware handles /v1/auth/login. Per D-22-02: authenticated-vs-public only for v4.0; per-route role enforcement deferred to v5.0."
  - "Committed Cargo.lock as a separate chore commit (60fa5cf0a) after Task 1 to keep the source commit atomic and avoid rewriting history (amend). Cargo.lock is tracked in this repo."

patterns-established:
  - "Pattern: JWT validation at the enforcement boundary — decode_token + Validation::new(Algorithm::HS256) lives in the middleware, not the auth domain module."
  - "Pattern: from_fn middleware with public-allowlist bypass — wrap() on a scope, is_public(path) check before token validation."
  - "Pattern: demo-grade in-memory auth store seeded with the 4 predefined roles + per-role permissions mirroring the frontend Permission union."

requirements-completed: [GOV-01]

# Coverage metadata (#1602) — one entry per shipped deliverable.
coverage:
  - id: A1
    description: "/v1/auth/login, /me, /refresh endpoints served with HS256 JWTs signed by API_SECRET_KEY; in-memory store seeds 4 demo users (admin/author/reviewer/viewer) with plaintext passwords (D-22-01)"
    requirement: GOV-01
    verification:
      - kind: integration
        ref: "cargo test -p api --test auth_endpoints — 7/7 pass (login valid + wrong password + email-field + all 4 roles; me valid + no token; refresh fresh exp)"
        status: pass
    human_judgment: false
  - id: A2
    description: "requires_auth upgraded from presence-only stub to HS256 JWT validation + role resolution; algorithm pinned (T-22-01 alg-confusion mitigated); protected /v1/* wrapped with a public allowlist (T-22-02)"
    requirement: GOV-01
    verification:
      - kind: integration
        ref: "cargo test -p api --test auth_protection — 7/7 pass (401 no-token, 200 valid, 401 wrong-secret, 401 alg:none, allowlist login+/metrics+/health 2xx, refresh requires token, Claims inserted into extensions)"
        status: pass
      - kind: automated
        ref: "grep -n 'Validation::new(Algorithm::HS256)' api/src/middleware/auth.rs → matches the decode call (T-22-01); grep 'Validation::default()' → only doc-comment warnings, no unpinned decode"
        status: pass
    human_judgment: false
  - id: A3
    description: "No regression across the api/engine workspace; clippy clean"
    requirement: GOV-01
    verification:
      - kind: integration
        ref: "cargo test --workspace — 254 passed, 0 failed (baseline 232 + 18 new auth tests)"
        status: pass
      - kind: automated
        ref: "cargo clippy -p api --all-targets — 0 warnings"
        status: pass
    human_judgment: false

# Metrics
duration: ~25 min
completed: 2026-07-02
status: complete
---

# Phase 22 Plan 02: Pragmatic Auth + JWT Summary

**Built `/v1/auth/login`, `/v1/auth/me`, `/v1/auth/refresh` from scratch (the backend had ZERO auth endpoints — RESEARCH Pitfall 1 verified STATE.md's "P1 done" claim as false), upgraded `requires_auth` from a presence-only stub to real HS256 JWT validation + role resolution, and wrapped protected `/v1/*` routes with a public allowlist — closing GOV-01's auth enforcement end-to-end with the frontend `usePermission` (mounted in 22-03).**

## Performance

- **Duration:** ~25 min
- **Tasks:** 2 implementation tasks, each committed atomically (+ 1 Cargo.lock chore commit)
- **Files:** 4 created, 5 modified
- **Tests:** 18 new (4 auth lib unit + 7 auth_endpoints integration + 7 auth_protection integration); workspace 254/0 (no regression from the 232 baseline)

## Accomplishments

- Confirmed the RESEARCH Pitfall 1 finding at build time: the backend had NO auth endpoints and `requires_auth` was a presence-only stub that only checked the `Bearer ` prefix (never validated a JWT, never `wrap()`ed). Built the endpoints and real validation from scratch.
- Added `jsonwebtoken = "10.4"` (with the `rust_crypto` feature — pure-Rust HMAC/SHA2 backend that auto-selects the CryptoProvider jsonwebtoken 10.4 requires) to `api/Cargo.toml`.
- Created `api/src/auth/store.rs`: in-memory `UserStore` (`Arc<Mutex<HashMap>>`) following the existing `init_store()` pattern; seeds 4 demo users (admin/author/reviewer/viewer) with plaintext passwords (D-22-01 permits; hashing deferred to v5.0) and per-role permissions mirroring the frontend `Permission` union; `find_user` resolves by username OR email so the frontend `AuthCredentials {email,password}` and the plan behavior tests `{username,password}` both work.
- Created `api/src/auth/mod.rs`: `Claims {sub, role, exp, iat}`, `UserResponse` matching the frontend `User` contract (camelCase `displayName`), `LoginRequest` (username OR email), `login`/`me`/`refresh` handlers with utoipa paths, `encode_token` + `secret_key` (reads `API_SECRET_KEY` with an insecure dev fallback + warning log, reusing the Phase 21 Helm secret placeholder).
- Upgraded `api/src/middleware/auth.rs` to an actix `from_fn` middleware: `requires_auth` validates the Bearer token as an HS256 JWT (algorithm pinned via `Validation::new(Algorithm::HS256)` in `decode_token` — rejects `alg:none` and wrong-secret tokens, T-22-01), bypasses a public allowlist (`/health`, `/swagger-ui`, `/api-docs`, `/v1/auth/login`, `/metrics`, D-22-02), and inserts the resolved `Claims` into request extensions so handlers can read the role (groundwork for v5.0 per-route role gating).
- Wrapped the `/v1` scope with `actix_web::middleware::from_fn(requires_auth)` in `api/src/main.rs`; `/v1/auth/login` stays public via the allowlist, `/v1/auth/me` and `/v1/auth/refresh` now require a valid Bearer token.
- Added 18 integration + unit tests covering login (valid, wrong password, email-field, all 4 roles), me (valid, no token), refresh (fresh exp), and the middleware behavior cases (401 no-token, 200 valid, 401 wrong-secret, 401 alg:none, allowlist 2xx, refresh requires token, Claims-in-extensions).

## Task Commits

Each task was committed atomically (with a HEAD-drift guard before each commit):

1. **Task 1: Auth module — in-memory store, Claims, /v1/auth/* handlers (D-22-01)** — `20f708b37` (feat)
2. **Cargo.lock for the jsonwebtoken dep** — `60fa5cf0a` (chore; committed separately to keep Task 1's source commit atomic and avoid rewriting history via amend)
3. **Task 2: Upgrade requires_auth to JWT validation + wrap protected /v1/*** — `3fa473e5b` (feat)

## Files Created/Modified

- `api/Cargo.toml` — `jsonwebtoken = { version = "10.4", features = ["rust_crypto"] }`
- `api/src/lib.rs` — `pub mod auth;` + `pub mod middleware;` so integration tests reach `api::auth` and `api::middleware::auth`
- `api/src/auth/mod.rs` (new) — `Claims`, `UserResponse`, `LoginRequest`/`LoginResponse`/`RefreshResponse`, `secret_key` (pub), `encode_token`, `login`/`me`/`refresh` handlers; `decode_token` lives in the middleware module
- `api/src/auth/store.rs` (new) — `UserRecord`, `UserStore`, `init_store` (4 demo users), `find_user`, per-role `permissions_for`
- `api/src/middleware/auth.rs` (rewritten) — `decode_token` (HS256 pinned, T-22-01) + `requires_auth` `from_fn` middleware (allowlist bypass, Claims into extensions, D-22-02)
- `api/src/main.rs` — `mod auth;` + `mod middleware;`, `user_store` app_data, `/v1/auth/{login,me,refresh}` routes, `wrap(from_fn(requires_auth))` on the `/v1` scope
- `api/tests/auth_endpoints.rs` (new) — 7 integration tests for the auth handlers
- `api/tests/auth_protection.rs` (new) — 7 integration tests for the middleware
- `Cargo.lock` — jsonwebtoken 10.4 + rust_crypto transitive deps

## Decisions Made

- **`decode_token` moved to `middleware/auth.rs`.** The plan's Task 2 acceptance criterion greps `api/src/middleware/auth.rs` for `Validation::new(Algorithm::HS256)`, and the plan key_links say `requires_auth → decode::<Claims>(HS256, API_SECRET_KEY)`. Hosting `decode_token` at the enforcement boundary satisfies both and makes the T-22-01 mitigation visible at the request gate. `encode_token` + `Claims` remain in `auth/mod.rs` (auth domain); handlers call `crate::middleware::auth::decode_token`.
- **`jsonwebtoken` `rust_crypto` feature.** jsonwebtoken 10.4 requires an explicit CryptoProvider (it panics otherwise). `rust_crypto` (hmac + sha2, pure Rust) auto-selects one so HS256 works without `CryptoProvider::install_default()`. This is the minimum change to make the plan's `jsonwebtoken = "10.4"` functional.
- **LoginRequest accepts username OR email.** The frontend `AuthCredentials` is `{email, password}`; the plan behavior tests use `{username, password}`. `find_user` resolves by either, so both callers work against the same demo store. `admin@rrules.dev` resolves to the `admin` demo user.
- **UserResponse uses `displayName` (camelCase).** Matches the frontend `User` interface in `frontend/src/types/auth.ts`; `permissions` are surfaced per role so the frontend `usePermission` can gate the UI end-to-end.
- **Wrapped the `/v1` scope, not the whole App.** Root-level public routes (`/metrics`, `/health`, `/swagger-ui`, `/api-docs`, `fs::Files`) stay naturally public (outside the wrapped scope); the allowlist bypass inside the middleware handles `/v1/auth/login`. Wrapping the whole App would have required the frontend `index.html` to load without a token (a `/` allowlist entry) — scoping to `/v1` avoids that.
- **Cargo.lock committed as a separate chore.** It is a tracked file in this repo and changed as a direct consequence of Task 1's Cargo.toml edit. Committed separately (non-destructive, no amend) to keep Task 1's source commit atomic.

## Deviations from Plan

### Auto-fixed Issues

- **jsonwebtoken CryptoProvider panic.** The plan specified `jsonwebtoken = "10.4"` with no feature flags; jsonwebtoken 10.4 panics at runtime ("Could not automatically determine the process-level CryptoProvider"). Auto-fixed by enabling the `rust_crypto` feature — the documented minimum to make HS256 functional. No plan semantics changed.
- **`actix_web::HttpMessage` trait not in scope.** `req.extensions_mut()` / `req.extensions()` require the `HttpMessage` trait import; the compiler flagged it. Added `use actix_web::HttpMessage;` to `middleware/auth.rs` and the test file. Standard fix.
- **`Next<impl MessageBody>` lifetime bound.** `from_fn` middleware returning `ServiceResponse<BoxBody>` requires `impl MessageBody + 'static` (the boxed body must be `'static`). Added the bound per the compiler's suggestion. Standard fix.

### Planned-layout deviation (documented, not a blocker)

- **`decode_token` location.** The plan's `files_modified` lists `decode_token`-relevant work under `api/src/auth/mod.rs` (Task 1) and `api/src/middleware/auth.rs` (Task 2), and the Task 2 acceptance criterion greps the middleware file for `Validation::new(Algorithm::HS256)`. I placed `decode_token` in `middleware/auth.rs` (Task 2) to satisfy the grep and the key_links, and removed it from `auth/mod.rs`. This is a layout refinement consistent with the plan's intent, not a scope change. The auth handlers (me/refresh) call it across the module boundary (same-crate, compiles fine).

**Total deviations:** 3 auto-fixes (runtime/compile requirements) + 1 documented layout refinement. No plan semantics changed.

## Issues Encountered

- **`test::init_service` return type + `call_service` inference.** An `async fn auth_app() -> impl Service<ServiceRequest, ...>` helper broke `call_service`'s `R` generic (it expected `ServiceRequest` instead of the test `Request`). Resolved by inlining `test::init_service(App::new()...)` at each call site so the service type is inferred per test (the pattern used by the existing `metrics_integration.rs`). No semantic change.

## User Setup Required

None for this plan. Set `API_SECRET_KEY` in production (the Phase 21 Helm secret placeholder) — the code falls back to an insecure dev default with a warning log if unset. Demo credentials are `admin`/`admin`, `author`/`author`, `reviewer`/`reviewer`, `viewer`/`viewer` (username == password). **Do not deploy the demo plaintext-password store to production** — v5.0 migrates to a Postgres user store + argon2/bcrypt hashing (D-22-03).

## Next Phase Readiness

- **Ready for 22-03 (dashboard fetch wiring + AuthProvider mounting).** `/v1/auth/login`, `/me`, `/refresh` are live and the frontend `auth-context.tsx` already targets `${API_URL}/auth/*` (plan 22-01). 22-03 can mount `AuthProvider` and the `usePermission` UI gating knowing the backend enforces auth. P-3 (auth enforcement) is covered by `auth_protection.rs`.
- **Ready for 22-04/22-05/22-06.** All `/v1/*` routes now require a valid HS256 JWT; the protected dashboard/simulation/branch endpoints are gated. Downstream plans should send the Bearer token from the frontend (the `client.ts` axios instance can attach it).
- **Orchestrator follow-up (D-22-02):** per-route role enforcement (viewer read-only, author cannot approve, admin full) is deferred to v5.0 — the backend currently enforces authenticated-vs-public only; the frontend `usePermission` gates the UI by role.
- **Orchestrator follow-up (D-22-03):** flag the v5.0 "Persistence & Identity" milestone (Postgres user/role/permission store, argon2/bcrypt password hashing, refresh-token rotation, per-route role enforcement) — out of scope for phase 22.
- **No blockers** for the next wave.

---
*Phase: 22-close-v4-0-frontend-integration-gaps-wire-orphaned-ui-to-bac*
*Completed: 2026-07-02*