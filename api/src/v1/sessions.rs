//! Phase 27 (SESS-01): stateful multi-interaction execution via a real
//! `SessionManager`.
//!
//! A caller supplies a `session_id` (idempotency alias), inserts facts, fires,
//! then calls again with the same `session_id` and sees accumulated working
//! memory from the prior call — with bounded resource use (TTL + LRU + max-
//! count + background sweep + a Prometheus gauge so a leak is visible on the
//! dashboard). The stateless one-shot path (Phase 23) stays unchanged when
//! `session_id` is absent (D-05).
//!
//! Wave 0 RED scaffold (D-09): the public types and signatures compile, but
//! `lookup_or_create` / `dispose_session` / `reset_session` return
//! `StatusCode::NOT_IMPLEMENTED` so the four SC tests fail RED. Task 2
//! replaces the stub bodies with real implementations; Task 3 wires the live
//! `execute_single` branch + `main.rs` glue.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use actix_web::{web, HttpResponse, http::StatusCode};
use dashmap::DashMap;
use prometheus::IntGauge;
use uuid::Uuid;

use engine::engine::registry::{EngineRegistry, ExecutionEngine};

use crate::v1::execution::SharedRuleSet;

// =====================================================================
// Public types
// =====================================================================

/// A single stateful session: a per-session `Box<dyn ExecutionEngine>` (the
/// real Phase-23 evaluator — D-01; NOT the v1.0 `engine::environment::Session`
/// stub which is a non-functional placeholder), its captured ruleset
/// generation (D-07), and the metadata the TTL/LRU sweep needs (D-03).
pub struct StatefulSessionEntry {
    /// The per-session engine. `!Sync` (`execute(&mut self)`) — serialized by
    /// the per-session `std::sync::Mutex` (D-11). Holds accumulated working
    /// memory across calls (SC-1's keystone honesty anchor).
    pub engine: Box<dyn ExecutionEngine>,
    /// Engine name fixed at session creation (D-06). A resume call with a
    /// conflicting `engine` param returns HTTP 400.
    pub engine_name: String,
    /// Generation captured at session creation (D-07). Stamped into every
    /// sessionful `ExecuteResponse.engine_generation` so callers detect
    /// staleness after a reload.
    pub engine_generation: u64,
    /// Last-touch timestamp for idle-TTL eviction (D-03).
    pub last_used: Instant,
    /// Creation timestamp for max-lifetime eviction (D-03).
    pub created_at: Instant,
    /// Server-generated primary key (D-04). The client `session_id` is an
    /// idempotency alias mapped via `SessionManager::aliases`.
    pub server_id: Uuid,
    /// Client-supplied idempotency alias (D-04). `None` is possible in
    /// principle but the live path always supplies one.
    pub client_alias: Option<String>,
}

/// Env-var config bundle (D-03 / Pattern 5). Read once at startup via
/// `read_session_config()` so the hot path does not re-read env vars.
pub struct SessionConfig {
    pub idle_ttl: Duration,
    pub max_lifetime: Duration,
    pub max_sessions: usize,
    pub sweep_interval: Duration,
}

/// Process-shared session registry (D-02 / D-12).
///
/// `Arc<SessionManager>` is shared across actix workers via
/// `web::Data<Arc<SessionManager>>`. The primary `sessions` map is keyed by
/// the server-generated UUID; the secondary `aliases` map is keyed by the
/// client-supplied idempotency alias for O(1) resume (D-04 / Pattern 3).
///
/// The per-session `Box<dyn ExecutionEngine>` is `!Sync`; each entry is
/// wrapped in `std::sync::Mutex` (NOT `tokio::sync::Mutex` — D-11 / Pitfall 2)
/// so same-session calls serialize while different sessions stay concurrent
/// (different DashMap shards + different per-entry Mutexes).
pub struct SessionManager {
    /// Primary registry: server UUID → session entry.
    pub sessions: DashMap<Uuid, Arc<Mutex<StatefulSessionEntry>>>,
    /// Alias → server UUID (O(1) resume; D-04 idempotency).
    pub aliases: DashMap<String, Uuid>,
    pub idle_ttl: Duration,
    pub max_lifetime: Duration,
    pub max_sessions: usize,
    pub sweep_interval: Duration,
    /// `rrules_sessions_active` Prometheus IntGauge (D-08). Registered on the
    /// Phase-24 dedicated `Registry` (NOT `default_registry()` — Pitfall
    /// 24-A). Set to `len()` after each execute and on every sweep eviction.
    pub sessions_active: IntGauge,
}

impl SessionManager {
    /// Construct a new manager with the given config + gauge. The gauge MUST
    /// already be registered on the Phase-24 dedicated `Registry` by the
    /// caller (mirrors `ExecutionCounters::new()`).
    pub fn new(config: SessionConfig, gauge: IntGauge) -> Self {
        Self {
            sessions: DashMap::new(),
            aliases: DashMap::new(),
            idle_ttl: config.idle_ttl,
            max_lifetime: config.max_lifetime,
            max_sessions: config.max_sessions,
            sweep_interval: config.sweep_interval,
            sessions_active: gauge,
        }
    }

    /// Live session count (mirrors `DashMap::len`). Used by the
    /// `MetricsSnapshot.active_sessions` population (D-08) and the
    /// `rrules_sessions_active` gauge.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Test-isolation primitive (D-09): drain both maps + reset the gauge.
    /// Called by `reset_test_env()` in the integration tests so SC-1..SC-4
    /// start from a known-empty state.
    pub fn clear(&self) {
        self.sessions.clear();
        self.aliases.clear();
        self.sessions_active.set(0);
    }

    /// Lookup by client alias → server UUID → entry, OR create a new session
    /// if the alias is unknown or stale (D-04 / Pattern 3).
    ///
    /// On resume: returns the existing `(server_uuid, entry_arc)`; a
    /// conflicting `req_engine` returns HTTP 400 (D-06). On create: resolves
    /// the engine name (req > `RRULES_DEFAULT_ENGINE` > "rete"), validates
    /// against `registry.available_engines()`, builds a fresh
    /// `Box<dyn ExecutionEngine>`, loads the shared ruleset, captures the
    /// generation (D-07), and inserts into both maps.
    ///
    /// CRITICAL (Pitfall 1): the caller clones the `Arc<Mutex<Entry>>` out
    /// and the DashMap `Ref` guards drop BEFORE the per-session Mutex is
    /// locked — no DashMap shard guard is held across `web::block`.
    ///
    /// Wave 0 RED: returns `NOT_IMPLEMENTED`. Task 2 replaces the body.
    pub fn lookup_or_create(
        &self,
        _client_alias: &str,
        _req_engine: Option<&str>,
        _registry: &EngineRegistry,
        _ruleset: &Arc<std::sync::RwLock<SharedRuleSet>>,
    ) -> Result<(Uuid, Arc<Mutex<StatefulSessionEntry>>), (StatusCode, String)> {
        Err((
            StatusCode::NOT_IMPLEMENTED,
            "SessionManager not yet implemented (Wave 0 RED)".to_string(),
        ))
    }

    /// Evict sessions by (a) idle TTL, (b) max lifetime, then (c) LRU when
    /// over `max_sessions` (D-03). Updates the gauge after each sweep
    /// (Pitfall 6 — a sweep that evicts without updating the gauge leaves
    /// the dashboard lying about the live count).
    ///
    /// Wave 0 RED: no-op stub. Task 2 replaces the body.
    pub fn sweep_expired(&self) {}

    /// Dispose a session by client alias (D-05). Removes from both maps and
    /// updates the gauge. Returns `true` if a session was removed; `false`
    /// if the alias was not found or stale.
    ///
    /// Wave 0 RED: returns `false`. Task 2 replaces the body.
    pub fn dispose_by_alias(&self, _alias: &str) -> bool {
        false
    }

    /// Reset a session's working memory WITHOUT disposing (D-05 / Open Q#1).
    /// `ReteEngine::reset()` clears `working_facts` AND `rules`
    /// (`rete/mod.rs:525-532`), so reset MUST rebuild a fresh engine via
    /// `registry.create_engine_by_name` + `load_ruleset` with the session's
    /// captured generation rather than relying on `reset()` alone.
    ///
    /// Wave 0 RED: returns `NOT_IMPLEMENTED`. Task 2 replaces the body.
    pub fn reset_by_alias(
        &self,
        _alias: &str,
        _registry: &EngineRegistry,
        _ruleset: &Arc<std::sync::RwLock<SharedRuleSet>>,
    ) -> Result<(), (StatusCode, String)> {
        Err((
            StatusCode::NOT_IMPLEMENTED,
            "reset not yet implemented (Wave 0 RED)".into(),
        ))
    }
}

// =====================================================================
// Env-var config knobs (Pattern 5 — read once at startup)
// =====================================================================

/// Read the session-config env vars once (D-03). Defaults: idle TTL 900s,
/// max lifetime 86400s, max sessions 1024, sweep interval 60s. Matches the
/// `RRULES_DEFAULT_ENGINE` knob pattern (`main.rs:57-65`).
pub fn read_session_config() -> SessionConfig {
    let idle_ttl = std::env::var("RRULES_SESSION_IDLE_TTL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(900));
    let max_lifetime = std::env::var("RRULES_SESSION_MAX_LIFETIME_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(86400));
    let max_sessions = std::env::var("RRULES_MAX_SESSIONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1024);
    let sweep_interval = std::env::var("RRULES_SESSION_SWEEP_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(60));
    SessionConfig {
        idle_ttl,
        max_lifetime,
        max_sessions,
        sweep_interval,
    }
}

// =====================================================================
// Background sweep task (D-10 — mirrors start_file_watcher spawn pattern)
// =====================================================================

/// Background sweep task. Loops on `tokio::time::interval(sweep_interval)` and
/// calls `SessionManager::sweep_expired()` (evicts by TTL/max-lifetime/LRU +
/// updates the gauge). Lives for the process lifetime (no explicit
/// `CancellationToken` — matches `start_file_watcher`).
pub async fn run_session_sweep(manager: Arc<SessionManager>) {
    let mut interval = tokio::time::interval(manager.sweep_interval);
    loop {
        interval.tick().await;
        manager.sweep_expired();
    }
}

// =====================================================================
// Actix route handlers (D-05)
// =====================================================================

/// `DELETE /v1/execute/session/{id}` — dispose a session (D-05).
///
/// Wave 0 RED: returns `NOT_IMPLEMENTED`. Task 2 replaces the body.
///
/// Returns `impl Responder` (matching the `execute_rules` pattern at
/// `execution.rs:733-745`) rather than `Result<HttpResponse, (StatusCode,
/// String)>` because `(StatusCode, String)` does not implement
/// `actix_web::ResponseError` and cannot be used directly as a handler
/// return type.
pub async fn dispose_session(
    _id: web::Path<String>,
    _manager: web::Data<Arc<SessionManager>>,
) -> impl actix_web::Responder {
    HttpResponse::build(StatusCode::NOT_IMPLEMENTED)
        .json(serde_json::json!({ "error": "dispose not implemented (Wave 0 RED)" }))
}

/// `POST /v1/execute/session/{id}/reset` — clear WM without disposing (D-05).
///
/// Wave 0 RED: returns `NOT_IMPLEMENTED`. Task 2 replaces the body. Task 2
/// also widens the signature to include `web::Data<EngineRegistry>` and
/// `web::Data<Arc<RwLock<SharedRuleSet>>>` so `reset_by_alias` can rebuild a
/// fresh engine (Open Q#1).
///
/// Returns `impl Responder` — see `dispose_session` doc comment for rationale.
pub async fn reset_session(
    _id: web::Path<String>,
    _manager: web::Data<Arc<SessionManager>>,
) -> impl actix_web::Responder {
    HttpResponse::build(StatusCode::NOT_IMPLEMENTED)
        .json(serde_json::json!({ "error": "reset not implemented (Wave 0 RED)" }))
}