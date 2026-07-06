# Phase 27: Stateful sessions - Pattern Map

**Mapped:** 2026-07-04
**Files analyzed:** 5 (1 new + 4 modified)
**Analogs found:** 5 / 5 (every file has a real codebase analog)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| NEW `api/src/v1/sessions.rs` | service / store | event-driven + request-response (TTL sweep + per-id lookup/create/dispose) | `engine/src/monitoring/detection.rs` (`RuleExecutionTracker` singleton + `clear()`) + `api/src/metrics.rs` (`ExecutionCounters` dedicated `Registry` + `IntGauge`-style metrics) + `api/src/main.rs:146` (`start_file_watcher` spawn pattern) | role-match (composite — no single analog covers all three concerns) |
| `api/src/v1/execution.rs` (`execute_single` branch + `ExecuteResponse.session_id` + `MetricsSnapshot.active_sessions` + `execute_with_session` removal) | controller / handler | request-response (branch on `session_id`, populate response, build `MetricsSnapshot`) | `api/src/v1/execution.rs:504-709` (the existing `execute_single` — the sessionful branch mirrors this shape) | exact (same file, additive branch) |
| `api/src/main.rs` (construct `SessionManager`, spawn sweep, add `web::Data`, register DELETE/reset routes) | config / wiring | request-response (startup wiring + route registration) | `api/src/main.rs:44-102, 146, 186-198, 208-267` (existing `web::Data` + `tokio::spawn(start_file_watcher)` + `/v1` scope) | exact (same file, additive wiring) |
| `api/Cargo.toml` (add `dashmap = "6.2.1"`) | config | — (manifest edit) | existing dep declarations in `api/Cargo.toml:10-56` | exact (same file) |
| `api/tests/execute_integration.rs` (SC-1/SC-2/SC-3/SC-4 keystone e2e + `SessionManager::clear()` isolation) | test | request-response (e2e HTTP assertions) | `api/tests/execute_integration.rs:304-465` (`init_test_app!` macro + `#[serial_test::serial]` + `reset_test_env()` + `CoverageTracking::clear()` / `RuleExecutionTracker::clear()` + `json_body`) | exact (same file, extends the established pattern) |

## Pattern Assignments

### NEW `api/src/v1/sessions.rs` (service / store, event-driven + request-response)

**Primary analog — singleton + `clear()` test-isolation:** `engine/src/monitoring/detection.rs:88-249`

This is the closest structural match for the `SessionManager`'s "process-shared state with a `clear()` test primitive" contract. `RuleExecutionTracker` is a unit-like struct with all state behind a `LazyLock<Mutex<TrackerState>>`; `SessionManager` instead uses `Arc<DashMap<...>>` (sharded, shared across actix workers via `web::Data`), but the `clear()`-for-test-isolation primitive and the "associated functions on a unit-like/manager type, do not instantiate per-call" discipline transfer directly.

```rust
// engine/src/monitoring/detection.rs:88-97 — singleton pattern (analog for SessionManager::clear)
pub struct RuleExecutionTracker;

static INSTANCE: LazyLock<Mutex<TrackerState>> = LazyLock::new(|| {
    Mutex::new(TrackerState {
        rules: HashMap::new(),
        total_evaluations: 0,
        total_fires: 0,
    })
});

// engine/src/monitoring/detection.rs:239-249 — clear() test-isolation primitive
// (SessionManager::clear() mirrors this shape: drain the maps, reset the gauge)
pub fn clear() {
    let mut state = INSTANCE.lock().unwrap();
    state.rules.clear();
    state.total_evaluations = 0;
    state.total_fires = 0;
}
```

**Key difference:** `RuleExecutionTracker` is a process-wide singleton accessed via static associated functions; `SessionManager` is `Arc`-wrapped, constructed once at startup, and injected via `web::Data<Arc<SessionManager>>` (mirrors `ExecutionCounters` wiring at `main.rs:102,189`). `clear()` is a `&self` method on the manager, not a static associated function — because the manager instance is shared, not global.

**Secondary analog — dedicated Prometheus `Registry` + `IntGauge` registration:** `api/src/metrics.rs:56-160`

The `rrules_sessions_active` IntGauge MUST be registered on the Phase-24 dedicated `Registry` (NOT `prometheus::default_registry()` — Pitfall 24-A: `register()` panics on duplicate names). The `ExecutionCounters::new()` shape — construct `Registry::new()`, build the metric, `registry.register(Box::new(metric.clone()))`, store both on the struct — is the exact pattern to copy for the gauge.

```rust
// api/src/metrics.rs:94-107 — ExecutionCounters::new() dedicated-Registry + register pattern
// (rrules_sessions_active IntGauge mirrors this exactly)
pub fn new() -> Self {
    let registry = Registry::new();

    let requests_total = IntCounterVec::new(
        Opts::new(
            "rrules_execute_requests_total",
            "Total number of POST /v1/execute requests",
        ),
        &["engine"],
    )
    .expect("construct rrules_execute_requests_total counter");
    registry
        .register(Box::new(requests_total.clone()))
        .expect("register rrules_execute_requests_total");
    // ...
}
```

**Tertiary analog — background sweep task spawn pattern:** `api/src/main.rs:139-147`

The sweep task `tokio::spawn(v1::sessions::run_session_sweep(sweep_manager))` mirrors the `start_file_watcher` spawn: a `tokio::spawn`'d async fn that lives for the process lifetime with no explicit `CancellationToken` (D-10: match the existing pattern). `run_session_sweep` uses `tokio::time::interval(manager.sweep_interval)` and loops on `interval.tick().await` then `manager.sweep_expired()`.

```rust
// api/src/main.rs:139-147 — start_file_watcher spawn pattern (analog for run_session_sweep)
let watch_rules = std::env::var("WATCH_RULES_DIR")
    .map(|v| v == "true" || v == "1")
    .unwrap_or(true);

if watch_rules {
    println!("Starting file watcher for rules directory: {}", rules_dir);
    tokio::spawn(v1::execution::start_file_watcher(rules_dir));
}
```

**Quaternary analog — per-request engine factory path (the surface a session wraps):** `api/src/v1/execution.rs:525-575` + `engine/src/engine/registry.rs:64-108, 222-231`

`SessionManager::lookup_or_create` reuses the Phase-23 factory path verbatim: `registry.available_engines()` validation + `registry.create_engine_by_name(&engine_name)` + `engine.load_ruleset(&guard.ruleset)` under a read lock on the shared `Arc<RwLock<SharedRuleSet>>`, then release before `web::block`. The `ExecutionEngine` trait (`execute(&mut self)`, `insert_fact(&mut self)`, `reset(&mut self)`, `load_ruleset`, `get_facts`) is the contract the per-session `Box<dyn ExecutionEngine>` satisfies.

```rust
// api/src/v1/execution.rs:535-575 — engine factory + load_ruleset under read lock
// (SessionManager::lookup_or_create reuses this exact sequence at session creation)
let mut engine = match registry.create_engine_by_name(&engine_name) {
    Some(Ok(engine)) => engine,
    Some(Err(e)) => return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("engine creation failed: {}", e))),
    None => return Err((StatusCode::BAD_REQUEST, format!("unknown engine: {}", engine_name))),
};

let (all_rule_ids, generation): (Vec<String>, u64) = {
    let guard = ruleset.read().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("ruleset lock poisoned: {}", e)))?;
    engine.load_ruleset(&guard.ruleset).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("load_ruleset failed: {}", e)))?;
    let ids = guard.ruleset.rules.iter().map(|r| r.id.clone()).collect();
    let gen = guard.generation;
    (ids, gen)
};
```

```rust
// engine/src/engine/registry.rs:64-108 — ExecutionEngine trait (the !Sync contract D-11 honors)
pub trait ExecutionEngine: Send + Sync + std::fmt::Debug {
    fn execute(&mut self) -> EngineResult;
    fn insert_fact(&mut self, _fact: crate::fact::Fact) -> Result<(), String>;
    fn reset(&mut self);
    fn load_ruleset(&mut self, _rules: &crate::rules::RuleSet) -> Result<(), String> { /* ... */ }
    fn get_facts(&self) -> Vec<crate::fact::Fact> { Vec::new() }
    // ...
}

// engine/src/engine/registry.rs:222-231 — create_engine_by_name (factory path)
pub fn create_engine_by_name(&self, name: &str) -> Option<Result<Box<dyn ExecutionEngine>, String>> {
    let engine_type = match name.to_lowercase().as_str() {
        "rete" => EngineType::Rete,
        // ...
        _ => return None,
    };
    self.create_engine(engine_type)
}
```

**Critical planner-facing constraint — DashMap shard guard discipline (Pitfall 1 of RESEARCH):**
Clone the `Arc<Mutex<StatefulSessionEntry>>` OUT of the `DashMap` and drop the `Ref` guard BEFORE locking the per-session Mutex. Never hold the DashMap shard lock across `web::block`. The lookup helper should follow this shape:

```rust
// RESEARCH Pattern 2 — the load-bearing invariant:
//   no DashMap shard guard is held across web::block.
pub fn lookup(&self, alias: &str) -> Option<(Uuid, Arc<Mutex<StatefulSessionEntry>>)> {
    let uuid = *self.aliases.get(alias)?;       // Ref guard drops here
    let entry = self.sessions.get(&uuid)?.clone(); // Ref guard drops here
    Some((uuid, entry))
}
```

**Imports pattern to copy** (combine `execution.rs:1-17` API-layer conventions with the DashMap/tokio/prometheus deps):
```rust
// Mirror execution.rs:1-17 for the API-layer import style, then add:
use dashmap::DashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;
use prometheus::{IntGauge, Registry};
use actix_web::{web, HttpResponse};
use engine::engine::registry::{EngineRegistry, ExecutionEngine};
use crate::v1::execution::SharedRuleSet;
```

---

### `api/src/v1/execution.rs` (controller, request-response)

**Analog:** the same file — `execute_single` at lines 504-709 (the sessionful branch is an additive `if let Some(alias) = req.session_id.clone() { ... } else { /* existing stateless body */ }`).

**Imports pattern** (lines 1-17 — already present, no new top-level imports needed for the branch itself; `SessionManager` is imported from the new `crate::v1::sessions`):
```rust
// execution.rs:1-17 (existing — the branch reuses all of these)
use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;
use engine::engine::{EngineRegistry, EngineSelector};
use engine::rules::{Rule, RuleSet};
use crate::metrics::{ExecutionCounters, MetricsSnapshot, MetricsTx};
// NEW (added by Phase 27):
use crate::v1::sessions::SessionManager;
```

**Request/response field pattern** (lines 20-65 — `ExecuteRequest.session_id` already exists at line 23; add `ExecuteResponse.session_id: Option<String>` with the same `#[serde(skip_serializing_if = "Option::is_none")]` convention used by the existing optional fields):
```rust
// execution.rs:20-34 — ExecuteRequest (session_id already present from Phase 23)
pub struct ExecuteRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,    // Phase 27 gives this resume/idempotency semantics
    #[serde(default)]
    pub facts: HashMap<String, serde_json::Value>,
    // ...
}

// execution.rs:46-65 — ExecuteResponse (add session_id: Option<String> mirroring
// the engine_generation field's #[serde(default)] + skip_serializing_if convention)
pub struct ExecuteResponse {
    pub execution_id: String,
    pub rules_fired: usize,
    // ...
    #[serde(default)]
    pub engine_generation: u64,
    // NEW (Phase 27 / D-04): server UUID, absent on the stateless path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}
```

**Core `execute_single` branch pattern** (lines 504-575 — the sessionful branch wraps the stateless body in an `else` clause; the stateless path is unchanged):
```rust
// execution.rs:504-511 — handler signature (add session_manager param)
pub async fn execute_single(
    req: ExecuteRequest,
    registry: web::Data<EngineRegistry>,
    selector: web::Data<EngineSelector>,
    ruleset: web::Data<Arc<std::sync::RwLock<SharedRuleSet>>>,
    counters: web::Data<ExecutionCounters>,
    metrics_tx: web::Data<MetricsTx>,
    session_manager: web::Data<Arc<SessionManager>>,   // NEW (D-12)
) -> Result<ExecuteResponse, (actix_web::http::StatusCode, String)> {
    let execution_id = Uuid::new_v4().to_string();

    // NEW (D-05): branch on session_id.
    if let Some(client_alias) = req.session_id.clone() {
        // Sessionful path — lookup_or_create via session_manager, lock per-session
        // Mutex, insert facts, web::block execute, update last_used, build
        // response with server UUID + creation generation, update gauge.
        // ... (mirror the stateless body's structure: engine name resolution,
        // load_ruleset under read lock, insert facts, web::block, serialize
        // facts_after, build ExecuteResponse)
    } else {
        // Phase-23 stateless path — UNCHANGED (execution.rs:514-709).
        // ... existing body ...
    }
}
```

**`web::block` pattern** (lines 607-622 — the sessionful path reuses this shape but must NOT capture a `!Send` `MutexGuard` in the closure; clone/move the engine OUT of the session under the lock, drop the guard, move the engine into `web::block`, then re-lock to write back `last_used` and read `get_facts()`):
```rust
// execution.rs:607-622 — existing stateless web::block (analog for the sessionful path
// with the engine moved out of the session, NOT a MutexGuard captured)
let start = Instant::now();
let (engine_result, post_facts) = match web::block(move || {
    let result = engine.execute();
    let facts = engine.get_facts();
    (result, facts)
})
.await
{
    Ok(pair) => pair,
    Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("engine execution panicked: {}", e))),
};
```

**`MetricsSnapshot.active_sessions` population** (lines 651-660 — replace `active_sessions: 0,` at line 657 with `session_manager.len()`):
```rust
// execution.rs:651-660 — MetricsSnapshot build (the D-08 change is one line)
let snapshot = MetricsSnapshot {
    engine_name: engine_name.clone(),
    rules_fired: counters.snapshot_rules_fired.load(Ordering::Relaxed),
    facts_inserted: counters.snapshot_facts_inserted.load(Ordering::Relaxed),
    // Phase 27 (D-08): was `active_sessions: 0` — now from SessionManager::len().
    active_sessions: session_manager.len(),
    memory_bytes: 0,
    last_updated: chrono::Utc::now().to_rfc3339(),
};
let _ = metrics_tx.send(snapshot);
```

**`execute_with_session` stub to remove/repurpose** (lines 1081-1094 — the dead-code stub that returns `rules_fired: 0`; D-05 says remove or alias to `execute_single`):
```rust
// execution.rs:1081-1094 — the stub to remove (NOT registered as a route in main.rs;
// confirmed by grep — only the function exists, no .route() binding)
pub async fn execute_with_session(
    session_id: web::Path<String>,
    _req: web::Json<ExecuteRequest>,
) -> impl Responder {
    let execution_id = Uuid::new_v4().to_string();
    HttpResponse::Ok().json(serde_json::json!({
        "execution_id": execution_id,
        "session_id": session_id.into_inner(),
        "status": "executed",
        "rules_fired": 0
    }))
}
```

**Error handling pattern** (lines 527-548 — the `(StatusCode, String)` tuple-return convention is used throughout `execute_single`; the sessionful branch reuses it for the D-06 conflicting-engine 400 and the D-11 poison 500):
```rust
// execution.rs:527-548 — the (StatusCode, String) error convention
return Err((actix_web::http::StatusCode::BAD_REQUEST, format!("unknown engine: {}", engine_name)));
// D-06 conflicting engine on resume → same shape:
return Err((StatusCode::BAD_REQUEST, format!("session engine is fixed: expected {}, got {}", g.engine_name, req_eng)));
```

**Key differences to watch for:**
- `web::block` closure must be `Send + 'static` — a `std::sync::MutexGuard` is `!Send`. Move the engine by value into the closure (clone out under the lock, drop guard, move engine in), then re-lock after to update `last_used` and read `get_facts()`. (RESEARCH Pitfall 7.)
- The sessionful `engine_generation` is the **creation** generation (D-07), not the current shared generation. Capture it once at `lookup_or_create` and stamp it into the response; do NOT re-read `guard.generation` on the resume path.
- `active_sessions` is populated for **both** branches (stateless and sessionful) from `session_manager.len()` — the manager is shared state present in both paths.

---

### `api/src/main.rs` (config / wiring, request-response)

**Analog:** the same file — lines 44-102 (shared-state construction), 146 (`start_file_watcher` spawn), 186-198 (`.app_data` chain), 208-267 (`/v1` route scope).

**Shared-state construction pattern** (lines 52-102 — construct once at startup, wrap in `web::Data` or `Arc`):
```rust
// main.rs:52-66 — EngineRegistry + EngineSelector construction (analog for SessionManager)
let mut registry = EngineRegistry::new();
registry.register_default_engines();
let registry = web::Data::new(registry);

let mut selector = EngineSelector::with_defaults();
let default_engine = std::env::var("RRULES_DEFAULT_ENGINE")
    .unwrap_or_else(|_| "rete".to_string());
// ...
let selector = web::Data::new(selector);

// main.rs:102 — ExecutionCounters (analog for SessionManager::new(config, gauge))
let counters = web::Data::new(metrics::ExecutionCounters::new());
```

**Env-var config-knob pattern** (lines 57-65 — read once at startup; `read_session_config()` mirrors this exactly for `RRULES_SESSION_IDLE_TTL_SECS` / `RRULES_SESSION_MAX_LIFETIME_SECS` / `RRULES_MAX_SESSIONS` / `RRULES_SESSION_SWEEP_INTERVAL_SECS`):
```rust
// main.rs:57-65 — RRULES_DEFAULT_ENGINE read-once pattern (analog for read_session_config)
let default_engine = std::env::var("RRULES_DEFAULT_ENGINE")
    .unwrap_or_else(|_| "rete".to_string());
if selector.select_engine(&default_engine).is_err() {
    tracing::warn!(engine = %default_engine, "Invalid RRULES_DEFAULT_ENGINE, falling back to rete");
    let _ = selector.select_engine("rete");
}
```

**Background-task spawn pattern** (lines 139-147 — `tokio::spawn(v1::execution::start_file_watcher(rules_dir))` is the exact shape for `tokio::spawn(v1::sessions::run_session_sweep(sweep_manager))`):
```rust
// main.rs:139-147 — start_file_watcher spawn (analog for run_session_sweep)
let watch_rules = std::env::var("WATCH_RULES_DIR")
    .map(|v| v == "true" || v == "1")
    .unwrap_or(true);

if watch_rules {
    println!("Starting file watcher for rules directory: {}", rules_dir);
    tokio::spawn(v1::execution::start_file_watcher(rules_dir));
}
```

**`web::Data` injection pattern** (lines 181-198 — the `.app_data(...)` chain inside the `HttpServer::new` closure; add `.app_data(web::Data::new(session_manager.clone()))` alongside `counters.clone()`):
```rust
// main.rs:179-198 — the app_data chain (add session_manager.clone() alongside counters.clone())
App::new()
    .wrap(cors)
    .app_data(engine_state.clone())
    .app_data(registry.clone())
    .app_data(selector.clone())
    .app_data(ruleset.clone())
    .app_data(web::Data::new(metrics_tx.clone()))
    // Phase 24 (Plan 24-02): inject ExecutionCounters as web::Data
    .app_data(counters.clone())
    // ... other stores ...
    // Phase 27 (D-12): inject the shared SessionManager.
    .app_data(web::Data::new(session_manager.clone()))
```

**Route registration pattern** (lines 226-272 — register DELETE/reset inside the `/v1` scope; static sub-paths before wildcards, mirroring the `/rules/batch-execute` before `/rules/{id}` ordering note at line 230):
```rust
// main.rs:226-235 — existing /v1 route registrations (analog for DELETE/reset routes)
.route("/execute", web::post().to(v1::execution::execute_rules))
.route("/rules", web::get().to(v1::rules::get_rules))
// static sub-paths MUST be registered before /rules/{id} so actix matches the
// literal segment instead of binding it to the {id} wildcard.
.route("/rules/batch-execute", web::post().to(v1::execution::batch_execute_rules))
// Phase 27 (D-05): add lifecycle routes — register BEFORE any wildcard
// /execute/{...} route (none currently exists, but the discipline holds).
.route("/execute/session/{id}", web::delete().to(v1::sessions::dispose_session))
.route("/execute/session/{id}/reset", web::post().to(v1::sessions::reset_session))
```

**Key differences to watch for:**
- `SessionManager` is `Arc`-wrapped (`Arc<SessionManager>`), so `web::Data::new(session_manager.clone())` clones the `Arc` (cheap), not the manager. The `HttpServer::new` closure is `move`, so `session_manager` must be cloned into the closure scope before the `move` — same shape as `counters.clone()` (which is `web::Data<ExecutionCounters>`, also cheap-clone).
- The sweep task takes `session_manager.clone()` (an `Arc<SessionManager>`) — clone BEFORE the `HttpServer::new(move || ...)` closure captures it, otherwise the `move` closure captures the original and the sweep task cannot take it. Pattern: `let sweep_manager = session_manager.clone(); tokio::spawn(run_session_sweep(sweep_manager));` BEFORE the `HttpServer::new` block, mirroring line 146's placement before line 164.
- Module declaration: add `pub mod sessions;` (or `mod sessions;`) to `api/src/v1/mod.rs` alongside the existing `pub mod execution;` (line 10 of `mod.rs`). The new module must be reachable as `v1::sessions::*` from `main.rs`.

---

### `api/Cargo.toml` (config, manifest edit)

**Analog:** existing dep declarations — lines 17-56.

**Dependency declaration pattern** (lines 17-56 — pinned-version string with a comment explaining why it's pinned):
```rust
// api/Cargo.toml:17 — engine dep with feature flags + inline comment
engine = { path = "../engine", features = ["engine-rete", "engine-treat", "engine-leaps", "engine-phreak"] }

// api/Cargo.toml:34-38 — tokio / uuid / chrono (the supporting deps the new module uses)
tokio = { version = "1.35", features = ["full"] }
uuid = { version = "1.0", features = ["v4", "serde", "fast-rng"] }
chrono = { version = "0.4", features = ["serde"] }

// api/Cargo.toml:56 — prometheus workspace inheritance
prometheus = { workspace = true }

// NEW (Phase 27 / D-02): add to [dependencies] section. Pin 6.2.1 — NOT 7.0.0-rc2 (ROADMAP pins 6.2.1).
// Comment style mirrors the existing inline explanations:
// dashmap = "6.2.1"  # Sharded concurrent HashMap for the SessionManager registry (Phase 27 / D-02).
//                     # Pinned per ROADMAP — NOT 7.0.0-rc2.
dashmap = "6.2.1"
```

**Key differences to watch for:**
- `dashmap` is a direct version-pinned dep (like `actix-cors = "0.7"` at line 23), NOT a workspace dep. The `uuid`/`chrono`/`tokio`/`prometheus` deps the new module uses are already present — no re-declaration.
- `serial_test` (line 68) is already a dev-dep — no change to `[dev-dependencies]`.

---

### `api/tests/execute_integration.rs` (test, request-response e2e)

**Analog:** the same file — `init_test_app!` macro (304-320), `reset_test_env()` (326-338), `#[serial_test::serial]` + `#[actix_web::test]` (348-350), `json_body` (285-298), and the keystone `score_plus_10_rule()` (59-83) fixture.

**Test isolation pattern** (lines 326-338 — `reset_test_env()` clears `CoverageTracking` + `RuleExecutionTracker`; Phase 27 adds `SessionManager::clear()`):
```rust
// execute_integration.rs:326-338 — reset_test_env() (add SessionManager::clear() alongside)
fn reset_test_env() {
    CoverageTracking::clear();
    // Plan 26-02: also reset the RuleExecutionTracker singleton ...
    RuleExecutionTracker::clear();
    // SAFETY on env vars: tests run serially (#[serial_test::serial]); ...
    std::env::set_var("RRULES_DEFAULT_ENGINE", "rete");
    std::env::set_var("RULE_STORAGE_PATH", "./rules");
    // Phase 27 (D-09): also clear the SessionManager registry so SC-1..SC-4
    // start from a known-empty state. Tests that exercise TTL/max-sessions
    // env knobs must save/restore those vars here too.
    // SessionManager::clear();  // ← planner wires this once the manager is
    //                              //   reachable from the test (either via a
    //                              //   shared helper that constructs the same
    //                              //   Arc<SessionManager> the App uses, or via
    //                              //   a test-only `pub fn clear()` on a shared
    //                              //   instance).
}
```

**Test app wiring pattern** (lines 175-191 + 304-320 — `init_two_rule_app!` / `init_test_app!` macros install `app_data` for `registry`/`selector`/`ruleset`/`counters`/`metrics_tx`/`job_store`/`engine_state`; Phase 27 adds `shared_session_manager()`):
```rust
// execute_integration.rs:175-191 — init_two_rule_app! macro (add .app_data(shared_session_manager()))
macro_rules! init_two_rule_app {
    () => {
        test::init_service(
            App::new()
                .app_data(shared_registry())
                .app_data(shared_selector())
                .app_data(two_rule_shared_ruleset())
                .app_data(shared_counters())
                .app_data(shared_metrics_tx())
                .app_data(shared_job_store())
                .app_data(shared_engine_state())
                // Phase 27 (D-09): shared SessionManager for SC-1..SC-4.
                .app_data(shared_session_manager())
                .configure(configure_test_routes)
                .configure(configure_metrics),
        )
        .await
    };
}

// NEW helper (mirrors shared_counters() at lines 234-236):
fn shared_session_manager() -> web::Data<std::sync::Arc<SessionManager>> {
    web::Data::new(std::sync::Arc::new(SessionManager::new(
        read_session_config(),
        // Gauge: register on the shared_counters().registry — see planner note
    )))
}
```

**Test route registration pattern** (lines 269-281 — `configure_test_routes` mirrors `main.rs`'s `/v1` scope; add DELETE/reset routes here so SC-4 can hit them):
```rust
// execute_integration.rs:269-281 — configure_test_routes (add DELETE/reset routes)
fn configure_test_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/execute", web::post().to(execute_rules))
            // Phase 27 (D-09): lifecycle routes for SC-4.
            .route("/execute/session/{id}", web::delete().to(v1::sessions::dispose_session))
            .route("/execute/session/{id}/reset", web::post().to(v1::sessions::reset_session))
            .route("/metrics/coverage", web::get().to(coverage_endpoint))
            // ...
    );
}
```

**Keystone e2e test pattern** (lines 348-365 — `#[serial_test::serial]` + `#[actix_web::test]` + `reset_test_env()` + `init_test_app!()` + `test::TestRequest::post().uri("/v1/execute").set_json(json!({...})).to_request()` + `json_body(test::call_service(&app, req).await).await` + `assert!(body["..."].as_u64()...`):
```rust
// execute_integration.rs:348-365 — the established keystone test shape
// (SC-1/SC-2/SC-3/SC-4 mirror this exactly, varying the assertion + the env knobs)
#[serial_test::serial]
#[actix_web::test]
async fn execute_fires_real_rule() {
    reset_test_env();
    let app = init_test_app!();

    let req = test::TestRequest::post()
        .uri("/v1/execute")
        .set_json(json!({ "facts": { "scorecard": { "score": 0 } } }))
        .to_request();
    let body = json_body(test::call_service(&app, req).await).await;

    assert!(
        body["rules_fired"].as_u64().unwrap_or(0) >= 1,
        "EXEC-01: keystone rule should fire, got rules_fired = {}; body: {body}",
        body["rules_fired"]
    );
}

// SC-1 (the keystone honesty anchor) — the second call asserts score:20, not score:10:
//   call 1: set_json(json!({ "session_id": "s1", "facts": { "scorecard": { "score": 0 } } }))
//           → assert facts_after.scorecard.score == 10, response.session_id is present
//   call 2: set_json(json!({ "session_id": "s1", "facts": {} }))
//           → assert facts_after.scorecard.score == 20  (proves WM accumulated)
```

**Fixture rule pattern** (lines 59-83 — `score_plus_10_rule()` is already the keystone fixture; SC-1/SC-2/SC-3/SC-4 reuse it directly. No new fixture needed for the session tests — the existing `score_plus_10_rule()` + `two_rule_shared_ruleset()` cover the SC scenarios.)

**`json_body` helper** (lines 285-298 — reads success response body as JSON, panicking on non-2xx; reuse for SC-1/SC-2/SC-3. For SC-3's gauge assertion, also need a `/metrics` text-body reader, OR read the `MetricsSnapshot` from the broadcast channel — planner picks per D-09.)

**Key differences to watch for:**
- SC-2 (TTL) and SC-3 (LRU+max-sessions) mutate env vars (`RRULES_SESSION_IDLE_TTL_SECS`, `RRULES_MAX_SESSIONS`); they MUST save the prior value and restore it after the test (the existing `reset_test_env()` only restores `RRULES_DEFAULT_ENGINE` and `RULE_STORAGE_PATH`). Use `std::env::var()` save + `std::env::set_var()` restore in a `drop` guard or explicit finally block. Tests are `#[serial]`, so no concurrent env-var races.
- SC-2's TTL eviction is deterministic ONLY if the test triggers `SessionManager::sweep_expired()` directly (white-box) OR sets `RRULES_SESSION_SWEEP_INTERVAL_SECS=1` + sleeps. RESEARCH recommends the direct `sweep_expired()` call for determinism.
- SC-3's gauge assertion can read `/metrics` text output (Phase 24 `TextEncoder`) and grep for `rrules_sessions_active <N>`, OR subscribe a `MetricsTx` receiver and read the `MetricsSnapshot.active_sessions` field. The `/metrics` path is closer to the user-facing dashboard contract.
- `SessionManager::clear()` must be reachable from the test. Two viable shapes: (a) the test constructs its own `Arc<SessionManager>` (the same one `shared_session_manager()` returns) and calls `.clear()` on it directly; (b) `SessionManager` exposes a test-only associated `clear_all()` that drains a process-wide registry. Shape (a) is cleaner and mirrors the `Arc`-wired pattern — the test's `app` and the test's `manager` are the same `Arc`. Planner picks (D-09 discretion).

---

## Shared Patterns

### `web::Data<Arc<T>>` shared-state wiring
**Source:** `api/src/main.rs:44-102, 181-198` + `api/src/v1/execution.rs:504-511`
**Apply to:** `SessionManager` (NEW), `ExecutionCounters` (existing — the gauge-registration analog), `EngineRegistry`/`EngineSelector`/`SharedRuleSet` (existing — the factory path the session wraps).
```rust
// Construct once at startup (main.rs:52-102), wrap in web::Data or Arc,
// inject via .app_data(...) inside the HttpServer::new closure (main.rs:181-198),
// extract as web::Data<T> in the handler signature (execution.rs:504-511).
```

### `(StatusCode, String)` error convention
**Source:** `api/src/v1/execution.rs:511, 527-548`
**Apply to:** `execute_single` (sessionful branch), `SessionManager::lookup_or_create`, `dispose_session`, `reset_session`.
```rust
// Handler return type: Result<ExecuteResponse, (StatusCode, String)>
// Validation failures → (StatusCode::BAD_REQUEST, format!("..."))
// Lock/engine failures → (StatusCode::INTERNAL_SERVER_ERROR, format!("..."))
// D-06 conflicting engine on resume → (StatusCode::BAD_REQUEST, format!("session engine is fixed: expected {}, got {}", g.engine_name, req_eng))
```

### `web::block` for CPU-bound engine work
**Source:** `api/src/v1/execution.rs:607-622`
**Apply to:** the sessionful `execute_single` branch (engine `execute()` is CPU-bound and `!Sync`).
**Critical:** the closure must be `Send + 'static`; do NOT capture a `std::sync::MutexGuard` (it's `!Send`). Clone/move the engine OUT of the session under the lock, drop the guard, move the engine into `web::block`, execute, then re-lock to write back `last_used` and read `get_facts()`. The load-bearing invariant is "no DashMap shard guard is held across `web::block`" (RESEARCH Pitfall 1).

### Dedicated Prometheus `Registry` + `register()`
**Source:** `api/src/metrics.rs:94-160` (`ExecutionCounters::new`)
**Apply to:** the `rrules_sessions_active` IntGauge (D-08).
```rust
// Register on ExecutionCounters.registry (the Phase-24 dedicated Registry::new()),
// NOT prometheus::default_registry() — Pitfall 24-A: register() panics on duplicate names.
let gauge = IntGauge::new("rrules_sessions_active", "Active stateful session count")
    .expect("construct rrules_sessions_active");
counters.registry.register(Box::new(gauge.clone()))
    .expect("register rrules_sessions_active");
// Set after each execute AND after each sweep eviction (Pitfall 6):
gauge.set(manager.len() as i64);
```

### `#[serde(skip_serializing_if = "Option::is_none")]` for optional response fields
**Source:** `api/src/v1/execution.rs:22, 54` (existing `ExecuteRequest.session_id` and `ExecuteResponse.inference_trace`)
**Apply to:** the new `ExecuteResponse.session_id: Option<String>` (D-04).
```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub session_id: Option<String>,
```

### `#[serial_test::serial]` + singleton `clear()` test isolation
**Source:** `api/tests/execute_integration.rs:326-338` + `engine/src/monitoring/detection.rs:244-249`
**Apply to:** all four SC-1/SC-2/SC-3/SC-4 tests.
```rust
#[serial_test::serial]
#[actix_web::test]
async fn stateful_session_accumulates_wm() {
    reset_test_env();  // ← add SessionManager::clear() here (D-09)
    let app = init_test_app!();
    // ...
}
```

### Env-var config knobs read once at startup
**Source:** `api/src/main.rs:57-65` (`RRULES_DEFAULT_ENGINE`)
**Apply to:** `read_session_config()` (Pattern 5 of RESEARCH) for `RRULES_SESSION_IDLE_TTL_SECS` / `RRULES_SESSION_MAX_LIFETIME_SECS` / `RRULES_MAX_SESSIONS` / `RRULES_SESSION_SWEEP_INTERVAL_SECS`.
```rust
let idle_ttl = std::env::var("RRULES_SESSION_IDLE_TTL_SECS")
    .ok().and_then(|s| s.parse().ok()).map(Duration::from_secs)
    .unwrap_or(Duration::from_secs(900));
```

## No Analog Found

None. Every file has a real codebase analog:

| File | Analog | Notes |
|------|--------|-------|
| NEW `api/src/v1/sessions.rs` | composite of `detection.rs` (singleton + `clear()`) + `metrics.rs` (dedicated `Registry` + gauge) + `main.rs:146` (spawn pattern) + `execution.rs:525-575` (factory path) | No single analog covers all three concerns, but each concern has a concrete existing pattern to copy. |
| `api/src/v1/execution.rs` | itself (the `execute_single` body the sessionful branch mirrors) | exact |
| `api/src/main.rs` | itself (the `web::Data` wiring + `start_file_watcher` spawn + `/v1` scope) | exact |
| `api/Cargo.toml` | itself (existing dep declarations) | exact |
| `api/tests/execute_integration.rs` | itself (`init_test_app!` + `reset_test_env()` + `#[serial]` keystone pattern) | exact |

## Metadata

**Analog search scope:**
- `api/src/` (main.rs, v1/execution.rs, v1/mod.rs, metrics.rs)
- `api/tests/execute_integration.rs`
- `api/Cargo.toml`
- `engine/src/engine/registry.rs`
- `engine/src/monitoring/detection.rs`

**Files scanned:** 8 (5 directly cited + 3 supporting: `engine/src/engine/registry.rs`, `engine/src/monitoring/detection.rs`, `api/src/metrics.rs`)

**Pattern extraction date:** 2026-07-04

**Phase:** 27-stateful-sessions