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
//! Task 2 (GREEN): the public types, signatures, and full implementations
//! are in place — `lookup_or_create` with D-04/D-06 semantics, `sweep_expired`
//! with D-03 TTL/LRU/max-count + gauge update, `dispose_by_alias`,
//! `reset_by_alias` with Open Q#1 rebuild approach, and `dispose_session` /
//! `reset_session` actix handlers. Task 3 wires the live `execute_single`
//! branch + `main.rs` glue to turn the four SC tests GREEN.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use actix_web::{http::StatusCode, web, HttpResponse};
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
/// Shared handle to a live session entry.
///
/// The lookup path clones this `Arc` out of the `DashMap` and then releases the
/// shard guard, so a slow per-session operation never holds a map guard.
pub type SessionHandle = Arc<Mutex<StatefulSessionEntry>>;

/// Result of [`SessionManager::lookup_or_create`]: the server UUID plus the
/// shared session handle, or an HTTP status + message pair on validation error.
pub type SessionLookupResult = Result<(Uuid, SessionHandle), (StatusCode, String)>;

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

    /// True when no sessions are registered (companion to [`Self::len`]).
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
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
    pub fn lookup_or_create(
        &self,
        client_alias: &str,
        req_engine: Option<&str>,
        registry: &EngineRegistry,
        ruleset: &Arc<std::sync::RwLock<SharedRuleSet>>,
    ) -> SessionLookupResult {
        // --- Resume path (D-04 / Pattern 3) ---
        // Clone the alias→UUID out of the aliases map, then drop the Ref guard
        // immediately (Pitfall 1 — no shard guard held across anything slow).
        if let Some(uuid) = self.aliases.get(client_alias).map(|r| *r) {
            // Clone the Arc<Mutex<Entry>> out, drop the Ref guard immediately.
            if let Some(entry_arc) = self.sessions.get(&uuid).map(|r| r.clone()) {
                // D-06: engine is fixed at session creation. A resume call
                // with a conflicting engine param returns HTTP 400.
                if let Some(req_name) = req_engine {
                    let locked = entry_arc.lock().map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("session mutex poisoned: {}", e),
                        )
                    })?;
                    if locked.engine_name != req_name {
                        return Err((
                            StatusCode::BAD_REQUEST,
                            format!(
                                "session engine is fixed: expected {}, got {}",
                                locked.engine_name, req_name
                            ),
                        ));
                    }
                }
                return Ok((uuid, entry_arc));
            }
            // Stale alias: UUID not in sessions map (evicted). Fall through
            // to create — the alias will be overwritten (Pitfall 3 avoidance).
        }

        // --- Create path ---
        // 1. Resolve engine name: req > env RRULES_DEFAULT_ENGINE > "rete".
        let engine_name = req_engine.map(|s| s.to_string()).unwrap_or_else(|| {
            std::env::var("RRULES_DEFAULT_ENGINE").unwrap_or_else(|_| "rete".to_string())
        });

        // 2. Validate against registered factories (ENGINE-07).
        let available = registry.available_engines();
        if !available.contains(&engine_name.as_str()) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("unknown engine: {}", engine_name),
            ));
        }

        // 3. Build a fresh per-session engine.
        let mut engine = match registry.create_engine_by_name(&engine_name) {
            Some(Ok(engine)) => engine,
            Some(Err(e)) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("engine creation failed: {}", e),
                ))
            }
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("unknown engine: {}", engine_name),
                ))
            }
        };

        // 4. Load the shared ruleset (read-only guard) + capture generation.
        //    The guard is dropped before the entry is inserted so no lock
        //    spans the registry insertion (D-07 / L-03).
        let generation = {
            let guard = ruleset.read().map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("ruleset lock poisoned: {}", e),
                )
            })?;
            engine.load_ruleset(&guard.ruleset).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("load_ruleset failed: {}", e),
                )
            })?;
            guard.generation
        };

        // 5. Build the entry + insert into both maps + update gauge.
        let server_id = Uuid::new_v4();
        let entry = StatefulSessionEntry {
            engine,
            engine_name,
            engine_generation: generation,
            last_used: Instant::now(),
            created_at: Instant::now(),
            server_id,
            client_alias: Some(client_alias.to_string()),
        };
        let entry_arc = Arc::new(Mutex::new(entry));
        self.sessions.insert(server_id, entry_arc.clone());
        self.aliases.insert(client_alias.to_string(), server_id);
        self.sessions_active.set(self.sessions.len() as i64);

        Ok((server_id, entry_arc))
    }

    /// Evict sessions by (a) idle TTL, (b) max lifetime, then (c) LRU when
    /// over `max_sessions` (D-03). Updates the gauge after each sweep
    /// (Pitfall 6 — a sweep that evicts without updating the gauge leaves
    /// the dashboard lying about the live count).
    pub fn sweep_expired(&self) {
        let now = Instant::now();

        // Phase 1: evict by idle TTL or max lifetime. Collect UUIDs first so
        // no shard lock is held across `remove` (RESEARCH Pattern 4 note).
        let mut to_evict: Vec<Uuid> = Vec::new();
        for entry in self.sessions.iter() {
            let uuid = *entry.key();
            // Lock the per-session Mutex to read timestamps. Skip on poison
            // (a poisoned session is likely broken anyway — evict it).
            let should_evict = entry
                .value()
                .lock()
                .map(|g| {
                    now.duration_since(g.last_used) > self.idle_ttl
                        || now.duration_since(g.created_at) > self.max_lifetime
                })
                .unwrap_or(true);
            if should_evict {
                to_evict.push(uuid);
            }
        }
        for uuid in &to_evict {
            self.remove_by_uuid(uuid);
        }

        // Phase 2: LRU eviction while over max_sessions. Evict the entry with
        // the oldest last_used (min_by_key). Collect the key, drop the iter
        // guard, then remove — no shard lock held across remove.
        while self.sessions.len() > self.max_sessions {
            let lru_uuid = self
                .sessions
                .iter()
                .min_by_key(|e| {
                    e.value()
                        .lock()
                        .map(|g| g.last_used)
                        .unwrap_or(Instant::now())
                })
                .map(|e| *e.key());
            match lru_uuid {
                Some(uuid) => self.remove_by_uuid(&uuid),
                None => break,
            }
        }

        // Pitfall 6: update the gauge after every sweep so the dashboard
        // reflects the true live count.
        self.sessions_active.set(self.sessions.len() as i64);
    }

    /// Dispose a session by client alias (D-05). Removes from both maps and
    /// updates the gauge. Returns `true` if a session was removed; `false`
    /// if the alias was not found or stale.
    pub fn dispose_by_alias(&self, alias: &str) -> bool {
        if let Some(uuid) = self.aliases.get(alias).map(|r| *r) {
            // Check the session still exists (not stale).
            if self.sessions.contains_key(&uuid) {
                self.remove_by_uuid(&uuid);
                self.sessions_active.set(self.sessions.len() as i64);
                return true;
            }
            // Stale alias — clean up the orphaned alias entry.
            self.aliases.remove(alias);
        }
        false
    }

    /// Reset a session's working memory WITHOUT disposing (D-05 / Open Q#1).
    /// `ReteEngine::reset()` clears `working_facts` AND `rules`
    /// (`rete/mod.rs:525-532`), so reset MUST rebuild a fresh engine via
    /// `registry.create_engine_by_name` + `load_ruleset` with the session's
    /// captured generation rather than relying on `reset()` alone.
    ///
    /// Reset rebuilds the engine (Open Q#1: ReteEngine::reset() clears rules
    /// too; rebuild rather than rely on reset()). Reloads the CURRENT shared
    /// ruleset — D-07 discretion: reset-also-reload. This is the cleaner
    /// contract for "reset clears WM without disposing" and aligns with
    /// D-07's "default = just clear WM" since clearing WM + reloading the
    /// same ruleset is equivalent when no reload has happened.
    pub fn reset_by_alias(
        &self,
        alias: &str,
        registry: &EngineRegistry,
        ruleset: &Arc<std::sync::RwLock<SharedRuleSet>>,
    ) -> Result<(), (StatusCode, String)> {
        // Lookup alias → UUID → entry Arc. Clone the Arc out, drop all Ref
        // guards BEFORE locking the per-session Mutex (Pitfall 1).
        let (uuid, entry_arc) = {
            let uuid = self.aliases.get(alias).map(|r| *r).ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    format!("session not found: {}", alias),
                )
            })?;
            let entry_arc = self.sessions.get(&uuid).map(|r| r.clone()).ok_or_else(|| {
                // Stale alias — clean up.
                self.aliases.remove(alias);
                (
                    StatusCode::NOT_FOUND,
                    format!("session not found: {}", alias),
                )
            })?;
            (uuid, entry_arc)
        };

        // Lock the per-session Mutex and rebuild the engine.
        let mut guard = entry_arc.lock().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("session mutex poisoned: {}", e),
            )
        })?;

        // Read the engine_name for the rebuild.
        let engine_name = guard.engine_name.clone();

        // Rebuild a fresh engine (Open Q#1 — reset() clears rules too).
        let mut new_engine = match registry.create_engine_by_name(&engine_name) {
            Some(Ok(e)) => e,
            Some(Err(e)) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("engine creation failed during reset: {}", e),
                ))
            }
            None => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("engine factory lost during reset: {}", engine_name),
                ))
            }
        };

        // Load the CURRENT shared ruleset (D-07 discretion: reset-also-reload).
        {
            let rs_guard = ruleset.read().map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("ruleset lock poisoned: {}", e),
                )
            })?;
            new_engine.load_ruleset(&rs_guard.ruleset).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("load_ruleset failed during reset: {}", e),
                )
            })?;
            // Update the generation to the current ruleset generation.
            guard.engine_generation = rs_guard.generation;
        }

        // Replace the engine + touch the session.
        guard.engine = new_engine;
        guard.last_used = Instant::now();

        // Suppress unused-variable warning for uuid (it's part of the lookup
        // tuple but not needed after the entry Arc is extracted).
        let _ = uuid;

        Ok(())
    }

    /// Private helper: remove a session by server UUID from both maps.
    /// Called by `sweep_expired` and `dispose_by_alias`. Does NOT update
    /// the gauge — the caller is responsible for the gauge update (so sweep
    /// can batch-evict and set the gauge once at the end).
    fn remove_by_uuid(&self, uuid: &Uuid) {
        if let Some((_, entry_arc)) = self.sessions.remove(uuid) {
            // Clean up the alias entry. Lock the per-session Mutex to read
            // the client_alias (brief hold — no blocking work inside).
            if let Ok(guard) = entry_arc.lock() {
                if let Some(alias) = &guard.client_alias {
                    self.aliases.remove(alias);
                }
            }
        }
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
/// Returns 200 `{"status":"disposed","session_id":alias}` on success,
/// 404 on not-found.
///
/// Returns `impl Responder` (matching the `execute_rules` pattern at
/// `execution.rs:733-745`) rather than `Result<HttpResponse, (StatusCode,
/// String)>` because `(StatusCode, String)` does not implement
/// `actix_web::ResponseError` and cannot be used directly as a handler
/// return type.
#[utoipa::path(
    delete,
    path = "/v1/execute/session/{id}",
    params(("id" = String, Path, description = "Session id")),
    responses((status = 204, description = "The session was disposed"), (status = 404, description = "Unknown session")),
    tag = "execution",
)]
pub async fn dispose_session(
    id: web::Path<String>,
    manager: web::Data<Arc<SessionManager>>,
) -> impl actix_web::Responder {
    let alias = id.into_inner();
    if manager.dispose_by_alias(&alias) {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "disposed",
            "session_id": alias,
        }))
    } else {
        HttpResponse::build(StatusCode::NOT_FOUND)
            .json(serde_json::json!({ "error": format!("session not found: {}", alias) }))
    }
}

/// `POST /v1/execute/session/{id}/reset` — clear WM without disposing (D-05).
///
/// Returns 200 `{"status":"reset","session_id":alias}` on success,
/// 404 on not-found.
///
/// Signature includes `web::Data<EngineRegistry>` and
/// `web::Data<Arc<RwLock<SharedRuleSet>>>` so `reset_by_alias` can rebuild
/// a fresh engine (Open Q#1 — ReteEngine::reset() clears rules too).
///
/// Returns `impl Responder` — see `dispose_session` doc comment for rationale.
#[utoipa::path(
    post,
    path = "/v1/execute/session/{id}/reset",
    params(("id" = String, Path, description = "Session id")),
    responses((status = 204, description = "The session state was reset"), (status = 404, description = "Unknown session")),
    tag = "execution",
)]
pub async fn reset_session(
    id: web::Path<String>,
    manager: web::Data<Arc<SessionManager>>,
    registry: web::Data<EngineRegistry>,
    ruleset: web::Data<Arc<std::sync::RwLock<SharedRuleSet>>>,
) -> impl actix_web::Responder {
    let alias = id.into_inner();
    match manager.reset_by_alias(&alias, &registry, &ruleset) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "status": "reset",
            "session_id": alias,
        })),
        Err((code, msg)) => HttpResponse::build(code).json(serde_json::json!({ "error": msg })),
    }
}
