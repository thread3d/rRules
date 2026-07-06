# Phase 27: Stateful sessions - Research

**Researched:** 2026-07-04
**Domain:** Rust actix-web stateful session registry (DashMap + per-session Mutex + TTL/LRU/sweep) wrapping the Phase-23 `Box<dyn ExecutionEngine>` surface
**Confidence:** HIGH

## Summary

Phase 27 makes `/v1/execute` carry multi-interaction stateful execution through a real `SessionManager`: a caller supplies a `session_id`, inserts facts, fires, then calls again with the same `session_id` and sees accumulated working memory from the prior call — with bounded resource use (TTL + LRU + max-count + background sweep + a Prometheus gauge so a leak is visible on the dashboard). The stateless one-shot path (Phase 23) stays unchanged when `session_id` is absent.

The 12 locked decisions (D-01..D-12) in `27-CONTEXT.md` already fix the architecture. This research confirms the *how*: the DashMap 6.2.1 API (`entry()`/`get()`/`remove()`/`iter()`/`len()`) for the sharded registry, the per-session `Arc<Mutex<StatefulSessionEntry>>` discipline required by the `!Sync` `execute(&mut self)` engine contract, the `tokio::time::interval` sweep-task pattern (mirroring `start_file_watcher`), and the env-var config-knob pattern (`RRULES_SESSION_IDLE_TTL_SECS` etc., matching `RRULES_DEFAULT_ENGINE`). The keystone honesty anchor is SC-1's `score:20` on the second same-session call — impossible in the stateless path because the stateless path starts from request facts each call.

**Primary recommendation:** Build `SessionManager` as `Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>>` plus a second `DashMap<String, Uuid>` for O(1) alias→server-UUID resolution; ship registry + TTL + LRU + sweep + max-count + gauge in one phase (the OOM failure mode is silent); branch `execute_single` on `req.session_id` with the stateless path unchanged; spawn the sweep task alongside `start_file_watcher`; register `rrules_sessions_active` IntGauge on the Phase-24 dedicated prometheus `Registry`; drive Wave 0 RED→GREEN for SC-1/SC-2/SC-3/SC-4 in `api/tests/execute_integration.rs` with `#[serial]` + `SessionManager::clear()`.

## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** `StatefulSession` wraps a per-session `Box<dyn ExecutionEngine>` (the real Phase-23 evaluator), NOT the v1.0 `engine::environment::Session` stub (`environment.rs:152-208` is a non-functional v1.0-era stub: `execute()` returns "Not implemented", `insert_fact` is a no-op, holds a raw `*const Engine` pointer that is not `Send`).
- **D-02:** `Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>>` (DashMap 6.2.1, pinned per ROADMAP — NOT 7.0.0-rc2). Sharded so lookups/upserts across different `session_id`s do not contend on a single lock. `Arc`-wrapped and shared across actix workers via `web::Data`.
- **D-03:** Ship TTL + LRU + max-count + background sweep + Prometheus gauge TOGETHER. Defaults: idle TTL ~15 min (`RRULES_SESSION_IDLE_TTL_SECS=900`), max lifetime ~24 h (`RRULES_SESSION_MAX_LIFETIME_SECS=86400`), max sessions ~1024 (`RRULES_MAX_SESSIONS=1024`), sweep interval 60 s (`RRULES_SESSION_SWEEP_INTERVAL_SECS=60`). Evict if (a) `now - last_used > idle_ttl`, (b) `now - created_at > max_lifetime`, or (c) registry size exceeds `MAX_SESSIONS` and this entry is the LRU.
- **D-04:** Server-generated UUID v4 is the primary key; client-supplied `session_id` is an idempotency alias. `None` → stateless one-shot (Phase 23 path unchanged). `Some(alias)` → resume if alias exists, else create with a fresh `Uuid::new_v4()` and record the alias→UUID mapping. Response carries `ExecuteResponse.session_id: Option<String>` (the server UUID).
- **D-05:** `execute_single` branches on `req.session_id`; the sessionful path goes through `execute_single`, NOT a separate route. NEW routes `DELETE /v1/execute/session/{id}` (dispose) and `POST /v1/execute/session/{id}/reset` (clear WM without disposing). The `execute_with_session` stub at `execution.rs:1081-1094` is removed/repurposed (it returns `rules_fired: 0` and is not wired to the registry).
- **D-06:** Engine is fixed at session creation. A resume call with a conflicting `engine` param returns HTTP 400 (`"session engine is fixed: expected {X}, got {Y}"`). A resume call may omit `engine`. Default-engine resolution applies only at creation.
- **D-07:** Existing stateful sessions keep their already-loaded `RuleSet` snapshot after a reload; `engine_generation` in the sessionful response reflects the generation captured at session creation. `POST /v1/execute/session/{id}/reset` MAY optionally reload against the current ruleset (planner's discretion; default = just clear WM).
- **D-08:** Add a dedicated `rrules_sessions_active` IntGauge on the Phase-24 prometheus `Registry` AND populate the existing `MetricsSnapshot.active_sessions` field from `SessionManager::len()`. The gauge is set after each execute and on every sweep eviction.
- **D-09:** Extend `api/tests/execute_integration.rs` with the four SC scenarios, Wave 0 RED → GREEN + Nyquist. `#[serial]` + `SessionManager::clear()` for registry isolation.
- **D-10:** Spawn the sweep task in `main.rs` alongside `start_file_watcher` using `tokio::spawn` + `tokio::time::interval`. Match the existing shutdown pattern (no explicit `CancellationToken` unless the planner finds one already in use).
- **D-11:** `DashMap<String, Arc<Mutex<StatefulSessionEntry>>>` — concurrent calls on the SAME `session_id` serialize on the per-entry Mutex; different sessions stay fully concurrent (different DashMap shards + different per-entry Mutexes). Required by the `!Sync` `execute(&mut self)` engine contract.
- **D-12:** NEW `api/src/v1/sessions.rs` with `SessionManager` + `StatefulSessionEntry` + sweep task; inject as `web::Data<Arc<SessionManager>>`. Constructed once at startup after `EngineRegistry`/`EngineSelector`/`SharedRuleSet`. Add `dashmap = "6.2.1"` to `api/Cargo.toml` (NOT 7.0.0-rc2). `uuid` crate is already a dep.

### Claude's Discretion
- `StatefulSessionEntry` exact field set (a `StatefulSession` struct separate from the entry, or flat — planner's choice). Contract: `Box<dyn ExecutionEngine>` + `engine_name: String` + `engine_generation: u64` + `last_used: Instant` + `created_at: Instant` + `server_id: Uuid` + `client_alias: Option<String>`.
- TTL test mechanism — real short TTL + `tokio::time::sleep` vs. an injectable `Clock`/`now()` trait. SC-2 must be deterministic; mechanism is planner's pick.
- `reset` semantics — whether reset ALSO reloads against the current shared ruleset (refreshing `engine_generation`) or keeps the session's original ruleset. D-07 leans "keep original".
- `execute_with_session` route fate — remove entirely vs. keep as a thin alias to `execute_single`. Contract: "no production code returns `rules_fired: 0` for a sessionful call".
- Sweep-task shutdown — `CancellationToken` vs. runtime-teardown-only. Match `start_file_watcher`.
- Idempotency-alias storage — a separate `DashMap<String, Uuid>` (O(1) lookup) vs. a field on the entry + linear scan. This research recommends the second-DashMap approach (see Architecture Patterns).
- LRU eviction selection — `DashMap` iter + `min_by(last_used)` vs. an auxiliary ordering structure. At `MAX_SESSIONS ~1024` a periodic iter is cheap.

### Deferred Ideas (OUT OF SCOPE)
- RBAC-gated `/v1/sessions` admin list/count endpoint — deferred to a future governance/ops phase.
- Frontend test-runner / session UI — Phase 28.
- `inference_trace` population — Phase 28 (EXEC-04).
- Removing the v1.0 `engine::environment::Session` stub — documented (D-01) but left in place; a future cleanup phase can remove it once confirmed unused.
- Cross-session shared working memory / "undeclared state" — explicitly an anti-feature (FEATURES.md §"Anti-Features"); stateless stays strictly per-request. Permanent design constraint.
- Session-scoped `record_coverage: bool` flag — deferred.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SESS-01 | Stateful multi-interaction execution via `session_id` with bounded resource use | Standard Stack (DashMap 6.2.1), Architecture Patterns (per-session Mutex, second-DashMap alias, TTL/LRU/sweep), Code Examples (execute_single branch, sweep task, IntGauge), Validation Architecture (SC-1/SC-2/SC-3/SC-4 RED→GREEN) |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Session registry (lookup/create/upsert/dispose/reset) | API / Backend | — | The `SessionManager` is a server-side in-process registry shared across actix workers via `web::Data`; no DB/CDN/client involvement. |
| Per-session engine ownership (`Box<dyn ExecutionEngine>`) | API / Backend | — | The engine is `!Sync` (`execute(&mut self)`); it lives server-side inside a per-session `Mutex`. Browser never touches it. |
| TTL/LRU/max-count eviction | API / Backend | — | Background `tokio::time::interval` sweep task spawned in `main.rs`; pure server-side lifecycle. |
| Session ID generation (UUID v4) + idempotency alias | API / Backend | — | Server-generated primary key; client supplies only an alias. The alias→UUID map is server-side. |
| `rrules_sessions_active` Prometheus gauge | API / Backend | — | Registered on the Phase-24 dedicated prometheus `Registry` (api crate); exposed at `/metrics`. |
| `MetricsSnapshot.active_sessions` field | API / Backend | — | Populated from `SessionManager::len()` in the snapshot built after each execute (`execution.rs:651-660`); broadcast over the WS monitoring channel. |
| Stateful execute path branching | API / Backend | — | `execute_single` branches on `req.session_id`; the stateless path (Phase 23) is unchanged. |
| Session lifecycle routes (DELETE/reset) | API / Backend | — | NEW routes registered in the `/v1` scope in `main.rs`; actix handlers. |
| Facts accumulation (working memory) | API / Backend | — | The session's engine holds `working_facts` across calls; `get_facts()` returns accumulated WM. |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `dashmap` | `6.2.1` (pinned, NOT 7.0.0-rc2) | Sharded concurrent `HashMap` for the session registry — `Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>>` | ROADMAP-pinned; `Send + Sync`; sharded so lookups/upserts across different `session_id`s do not contend on a single lock (avoids the Pitfall-1-style regression a single `RwLock<HashMap>` would reintroduce). `[VERIFIED: crates.io]` — see Package Legitimacy Audit. |
| `uuid` | `1.0` (already a dep) | `Uuid::new_v4()` for server-generated session IDs; already used at `execution.rs:11,512` | Already in `api/Cargo.toml:37` with `["v4", "serde", "fast-rng"]` features. No new dep. `[VERIFIED: api/Cargo.toml]` |
| `chrono` | `0.4` (already a dep) | Timestamps for `last_used`/`created_at` if the planner picks `chrono::Utc::now()` over `Instant` (CONTEXT L-06 mentions `chrono`; `Instant` is also viable and avoids a new import) | Already in `api/Cargo.toml:38` with `["serde"]`. `[VERIFIED: api/Cargo.toml]` |
| `tokio` | `1.35` (already a dep, `["full"]`) | `tokio::spawn` + `tokio::time::interval` for the background sweep task; `tokio::sync::Mutex` is NOT used (the per-session Mutex is `std::sync::Mutex` — see Architecture Patterns) | Already in `api/Cargo.toml:34`. The `["full"]` feature includes `time`. `[VERIFIED: api/Cargo.toml + ctx7 tokio docs]` |
| `prometheus` | `0.13` (workspace, already a dep) | `IntGauge` for `rrules_sessions_active` on the Phase-24 dedicated `Registry` | Already promoted into `api/Cargo.toml:56` in Phase 24. The `Registry` lives on `ExecutionCounters` (`metrics.rs:80`). `[VERIFIED: api/src/metrics.rs]` |
| `actix-web` | `4.4` (already a dep) | `web::Data`, `web::Path`, `web::Json`, `web::block` for the sessionful execute path and the DELETE/reset routes | Already in `api/Cargo.toml:18`. `[VERIFIED: api/Cargo.toml]` |
| `serial_test` | `3` (workspace dev-dep) | `#[serial]` for registry-isolation tests (`SessionManager::clear()` between tests) | Already in `api/Cargo.toml:68` as `{ workspace = true }`; workspace pin at `Cargo.toml:8`. Established in Phase 23/26. `[VERIFIED: api/Cargo.toml]` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `std::sync::Mutex` | std | Per-session `Mutex<Box<dyn ExecutionEngine>>` — serializes same-session calls (the `!Sync` engine contract) | Always — the per-session lock. Prefer `std::sync::Mutex` over `tokio::sync::Mutex` because the engine work runs inside `web::block` (blocking, not async); holding a `tokio::Mutex` across `web::block` is an anti-pattern. `[CITED: execution.rs:608 web::block pattern]` |
| `std::time::Instant` | std | `last_used`/`created_at` for TTL/LRU computation | Cheaper than `chrono` and sufficient for duration math (`now.duration_since(last_used) > idle_ttl`). Use `chrono::Utc::now()` only if a human-readable timestamp is needed in observability. `[ASSUMED]` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `DashMap<String, Arc<Mutex<...>>>` | `Arc<RwLock<HashMap<String, ...>>>` | A single global write lock on insert/evict would serialize session creation across workers — the exact Pitfall-1-style regression Phase 23 avoided for engines. D-02 picks DashMap (sharded). |
| `std::sync::Mutex` (per-session) | `tokio::sync::Mutex` | `tokio::Mutex` is for async-aware locking across `.await`; the engine `execute()` runs inside `web::block` (blocking). `std::sync::Mutex` is correct and cheaper. Holding a `tokio::Mutex` guard across `web::block` is an anti-pattern. |
| Second `DashMap<String, Uuid>` for alias lookup | Field on entry + linear scan | O(1) vs O(n). At `MAX_SESSIONS ~1024` a scan is cheap-ish, but the second-DashMap is cleaner and the same sharding benefits apply. This research recommends the second-DashMap (D-12 discretion). |
| `Instant` for `last_used`/`created_at` | `chrono::Utc::now()` | `Instant` is monotonic and cheaper; `chrono` gives wall-clock for observability. Use `Instant` for TTL/LRU math; optionally also stamp `chrono` for display. |
| Real short TTL + `tokio::time::sleep` for SC-2 | Injectable fake `Clock`/`now()` trait | Real short TTL is simpler (no new trait) but slower (sleep); a fake clock is deterministic and instant but adds a trait abstraction. Planner's pick (D-discretion). This research recommends the real-short-TTL approach for SC-2 to keep the implementation honest (the fake-clock path risks hiding real-sweep bugs); set `RRULES_SESSION_IDLE_TTL_SECS=1` + `tokio::time::sleep(Duration::from_millis(1100))`. |

**Installation:**
```bash
# Add to api/Cargo.toml [dependencies]:
#   dashmap = "6.2.1"   # NOT 7.0.0-rc2 (ROADMAP pins 6.2.1)
# uuid, chrono, tokio, prometheus, actix-web, serial_test already present.
```

**Version verification:** `cargo info dashmap` confirms the crate exists (latest `7.0.0-rc2`; `6.2.1` is a prior published version — verified via the crates.io API: version `6.2.1`, not yanked, MIT, by xacrimon, 8.5M+ downloads). The `gsd-tools query package-legitimacy check --ecosystem crates dashmap` returns `OK` (5.18M weekly downloads, repo `github.com/xacrimon/dashmap`, not deprecated).

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `dashmap` | crates.io | since 2019-08-25 | 5.18M/week | github.com/xacrimon/dashmap | OK | Approved |
| `dashmap` 6.2.1 (pinned version) | crates.io | published (not yanked) | (part of above) | github.com/xacrimon/dashmap | OK | Approved — pinned per ROADMAP (NOT 7.0.0-rc2) |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

*All other deps (`uuid`, `chrono`, `tokio`, `prometheus`, `actix-web`, `serial_test`) are already in `api/Cargo.toml` from prior phases — no new legitimacy gate needed for them.*

## Architecture Patterns

### System Architecture Diagram

```
                    POST /v1/execute (with session_id)
                              │
                              ▼
                   ┌─────────────────────┐
                   │   execute_single    │   (execution.rs:504)
                   │  branch on          │
                   │  req.session_id     │
                   └──────────┬──────────┘
                              │
              ┌───────────────┴───────────────┐
              │                               │
      session_id None                 session_id Some(alias)
              │                               │
              ▼                               ▼
   ┌───────────────────┐         ┌─────────────────────────┐
   │  Phase-23 stateless│         │  Sessionful path        │
   │  one-shot (unchanged)│       │  1. resolve alias→UUID  │
   │  fresh engine/req   │       │     via second DashMap   │
   │  web::block execute │       │  2. lookup-or-create     │
   │  return response    │       │     entry in registry     │
   └───────────────────┘         │     DashMap<String,      │
                                 │     Arc<Mutex<Entry>>>   │
                                 │  3. clone Arc<Mutex> out │
                                 │     drop DashMap guard   │
                                 │  4. lock per-session Mutex│
                                 │  5. insert request facts │
                                 │  6. web::block execute   │
                                 │  7. update last_used     │
                                 │  8. build response w/    │
                                 │     server UUID +        │
                                 │     creation gen         │
                                 │  9. update gauge         │
                                 └──────────┬──────────────┘
                                            │
                                            ▼
                              ┌──────────────────────────┐
                              │  SessionManager          │
                              │  Arc<DashMap<String,     │  ← shared across actix workers via web::Data
                              │   Arc<Mutex<Entry>>>>    │
                              │  + DashMap<String,Uuid> │  ← alias→server UUID (O(1))
                              │  + IntGauge             │  ← rrules_sessions_active
                              │  len() / clear() /      │
                              │  sweep_expired()        │
                              └──────────┬──────────────┘
                                         │
              ┌──────────────────────────┴───────────────────────────┐
              │                                                      │
              ▼                                                      ▼
   ┌─────────────────────┐                              ┌────────────────────┐
   │  DELETE /v1/execute/ │                              │  Background sweep  │
   │  session/{id}        │                              │  tokio::time::     │
   │  → remove()          │                              │  interval(60s)     │
   │  → update gauge      │                              │  evict TTL/LRU/    │
   └─────────────────────┘                              │  max-count         │
                                                         │  → update gauge    │
   ┌─────────────────────┐                              └────────────────────┘
   │  POST /v1/execute/  │
   │  session/{id}/reset │
   │  → lock Mutex        │
   │  → engine.reset()    │
   │  → update last_used  │
   └─────────────────────┘
```

A reader can trace the primary use case (SC-1): `POST /v1/execute` with `session_id` → `execute_single` branches right → alias→UUID lookup → registry lookup-or-create → clone `Arc<Mutex>` out → lock → insert facts → `web::block` execute → update `last_used` → response with server UUID + creation generation → gauge update. The second call with the same alias resumes at the lookup-or-create step and sees accumulated working memory in the session's engine.

### Recommended Project Structure
```
api/src/v1/
├── sessions.rs       # NEW — SessionManager, StatefulSessionEntry, sweep_expired, dispose_session, reset_session
├── execution.rs      # MODIFIED — execute_single branches on session_id; ExecuteResponse.session_id added; MetricsSnapshot.active_sessions populated; execute_with_session removed/repurposed
└── ...
api/src/
├── main.rs           # MODIFIED — construct SessionManager, spawn sweep task, add web::Data, register DELETE/reset routes
├── metrics.rs        # ExecutionCounters.registry reused for the new IntGauge (or a separate gauge registered alongside)
└── ...
api/tests/
└── execute_integration.rs  # EXTENDED — SC-1/SC-2/SC-3/SC-4 keystone e2e (#[serial] + SessionManager::clear())
```

### Pattern 1: `SessionManager` as `Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>>` + second alias DashMap
**What:** The session registry is a sharded concurrent map keyed by server UUID; each entry is an `Arc<Mutex<StatefulSessionEntry>>` so concurrent calls on the SAME `session_id` serialize while different sessions stay concurrent. A second `DashMap<String, Uuid>` maps client aliases to server UUIDs for O(1) resume.
**When to use:** Always in Phase 27 — this is the D-02/D-04 data structure.
**Example:**
```rust
// Source: DashMap 6.2.1 API [VERIFIED: ctx7 /xacrimon/dashmap] + rRules Phase-23 web::Data pattern
use dashmap::DashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;
use prometheus::IntGauge;

pub struct StatefulSessionEntry {
    pub engine: Box<dyn engine::engine::registry::ExecutionEngine>,
    pub engine_name: String,
    pub engine_generation: u64,           // captured at creation (D-07)
    pub last_used: Instant,
    pub created_at: Instant,
    pub server_id: Uuid,
    pub client_alias: Option<String>,
}

pub struct SessionManager {
    /// Primary registry: server UUID → session entry.
    /// DashMap is sharded so lookups across different session_ids don't contend.
    pub sessions: DashMap<Uuid, Arc<Mutex<StatefulSessionEntry>>>,
    /// Alias → server UUID (O(1) resume; D-04 idempotency).
    pub aliases: DashMap<String, Uuid>,
    pub idle_ttl: std::time::Duration,
    pub max_lifetime: std::time::Duration,
    pub max_sessions: usize,
    pub sessions_active: IntGauge,         // rrules_sessions_active (D-08)
}

impl SessionManager {
    pub fn len(&self) -> usize { self.sessions.len() }
    pub fn clear(&self) { self.sessions.clear(); self.aliases.clear(); self.sessions_active.set(0); }

    /// Lookup by client alias → server UUID → entry. Clone the Arc<Mutex> out
    /// and drop the DashMap guard BEFORE locking, so the shard lock is not held
    /// across web::block (critical — see Pattern 2).
    pub fn lookup(&self, alias: &str) -> Option<(Uuid, Arc<Mutex<StatefulSessionEntry>>)> {
        let uuid = self.aliases.get(alias)?.clone();      // shard lock dropped here
        let entry = self.sessions.get(&uuid)?.clone();    // shard lock dropped here
        Some((uuid, entry))
    }
}
```

### Pattern 2: Per-session Mutex + clone-Arc-out-then-drop-guard discipline (the `!Sync` engine contract)
**What:** `Box<dyn ExecutionEngine>` is `!Sync` in practice because `execute(&mut self)`, `insert_fact(&mut self)`, and `reset(&mut self)` all require exclusive mutable access (`registry.rs:64-108`). The per-session `std::sync::Mutex` serializes same-session calls. CRITICAL: clone the `Arc<Mutex<StatefulSessionEntry>>` out of the DashMap and drop the DashMap shard guard BEFORE locking the per-session Mutex — never hold the DashMap guard across `web::block`.
**When to use:** Every sessionful execute path.
**Why `std::sync::Mutex` not `tokio::sync::Mutex`:** the engine `execute()` runs inside `web::block` (blocking, off the async runtime — `execution.rs:608`). `std::sync::Mutex` is correct and cheaper; holding a `tokio::Mutex` guard across `web::block` is an anti-pattern (the guard is `!Send` across the blocking boundary).
**Example:**
```rust
// Source: rRules execution.rs:608 web::block pattern [VERIFIED: codebase] + DashMap entry API [VERIFIED: ctx7]
// 1. Clone the Arc<Mutex> out, drop the DashMap guard.
let (server_uuid, entry_arc): (Uuid, Arc<Mutex<StatefulSessionEntry>>) = match manager.lookup(&alias) {
    Some(pair) => pair,
    None => /* create new session */ manager.create_session(alias, &engine_name, &ruleset, &registry)?,
};

// 2. Lock the per-session Mutex. Same-session calls serialize here; different
//    sessions are unaffected (different Mutexes, different DashMap shards).
let mut guard = entry_arc.lock().map_err(|e| /* poison 500 */)?;

// 3. Insert request facts into the session's engine (D-05).
for (fact_type, fact_value) in &req.facts {
    if let serde_json::Value::Object(fields) = fact_value.clone() {
        let mut payload = HashMap::new();
        for (k, v) in fields { payload.insert(k, json_to_engine_value(v)); }
        guard.engine.insert_fact(Fact::new(fact_type.clone(), payload))?;
    }
}

// 4. Execute inside web::block (CPU-bound; engine is !Sync). The Mutex guard
//    is held across web::block — this is safe because std::sync::MutexGuard is
//    not Send, so the closure captures it by &mut and runs on the blocking
//    pool. (Alternatively, clone the engine out, drop the guard, execute, then
//    re-lock to update last_used — the planner picks; the contract is "don't
//    hold the DashMap shard lock across web::block", NOT "don't hold the
//    per-session Mutex".)
let (result, post_facts) = web::block(move || {
    let r = guard.engine.execute();
    let f = guard.engine.get_facts();
    (r, f)
}).await?;

// 5. Update last_used under the lock, build response, update gauge.
```

**Important planner note:** The `web::block` closure captures `guard` (a `std::sync::MutexGuard`). `MutexGuard` is `!Send`, and `web::block` requires the closure to be `Send + 'static`. The cleanest approach is to clone the engine OUT of the session under the lock, drop the guard, move the cloned engine into `web::block`, execute, then re-lock to write back the post-execution state (or store post-execution state in the response and only re-lock to update `last_used`). The planner should pick whichever variant compiles and preserves "DashMap shard lock never held across web::block." The load-bearing invariant is: **no DashMap shard guard is held across `web::block`**. Holding the per-session Mutex across `web::block` is fine if the closure can be made `Send` (e.g., by moving the engine, not the guard, into the closure).

### Pattern 3: Second `DashMap<String, Uuid>` for O(1) alias→server-UUID resume
**What:** A separate sharded map maps client aliases to server UUIDs so resume is O(1) (no linear scan of the primary registry).
**When to use:** D-04 (client `session_id` is an idempotency alias). This is the recommended choice under D-12 discretion.
**Example:**
```rust
// Source: DashMap 6.2.1 API [VERIFIED: ctx7 /xacrimon/dashmap] + D-04
// Create path:
let server_uuid = Uuid::new_v4();
let mut entry = StatefulSessionEntry { engine, engine_name, engine_generation: gen, last_used: now, created_at: now, server_id: server_uuid, client_alias: alias.clone() };
manager.sessions.insert(server_uuid, Arc::new(Mutex::new(entry)));
if let Some(a) = &alias { manager.aliases.insert(a.clone(), server_uuid); }

// Resume path (O(1)):
if let Some(uuid) = manager.aliases.get(alias).map(|r| *r) {
    if let Some(entry_arc) = manager.sessions.get(&uuid).map(|r| r.clone()) {
        return Some((uuid, entry_arc));
    }
    // Stale alias (session evicted) — fall through to create.
}
```

### Pattern 4: TTL/LRU/max-count sweep via `tokio::time::interval`
**What:** A background task spawned in `main.rs` loops on `tokio::time::interval(sweep_interval)` and evicts sessions by (a) idle TTL, (b) max lifetime, or (c) LRU when over `MAX_SESSIONS`. Updates the gauge after each sweep.
**When to use:** D-03/D-10 — the OOM-bounding sweep. Mirrors `start_file_watcher` (`execution.rs:1024`, `main.rs:146`).
**Example:**
```rust
// Source: tokio::time::interval [VERIFIED: ctx7 /tokio-rs/tokio] + start_file_watcher pattern [VERIFIED: main.rs:146]
pub async fn run_session_sweep(manager: Arc<SessionManager>) {
    let mut interval = tokio::time::interval(manager.sweep_interval);
    loop {
        interval.tick().await;
        manager.sweep_expired();
    }
}

impl SessionManager {
    pub fn sweep_expired(&self) {
        let now = Instant::now();
        // Evict by TTL/max-lifetime. DashMap iter locks one shard at a time
        // (iter() does not hold all shards simultaneously), so removing
        // during iter is safe if you collect keys first.
        let mut to_evict: Vec<Uuid> = Vec::new();
        for entry in self.sessions.iter() {
            let uuid = *entry.key();
            let g = entry.value().lock().unwrap();
            if now.duration_since(g.last_used) > self.idle_ttl
               || now.duration_since(g.created_at) > self.max_lifetime {
                to_evict.push(uuid);
            }
        }
        for uuid in to_evict {
            self.remove_by_uuid(&uuid);
        }
        // LRU: if still over MAX_SESSIONS, evict the oldest last_used.
        while self.sessions.len() > self.max_sessions {
            let lru = self.sessions.iter()
                .min_by_key(|e| e.value().lock().unwrap().last_used)
                .map(|e| *e.key());
            match lru {
                Some(uuid) => self.remove_by_uuid(&uuid),
                None => break,
            }
        }
        self.sessions_active.set(self.sessions.len() as i64);
    }

    fn remove_by_uuid(&self, uuid: &Uuid) {
        if let Some((_, entry)) = self.sessions.remove(uuid) {
            if let Ok(g) = entry.lock() {
                if let Some(alias) = &g.client_alias {
                    self.aliases.remove(alias);
                }
            }
        }
    }
}
```
**Note on iter + remove:** DashMap's `iter()` acquires shard locks one at a time; collecting keys to evict into a `Vec` first (then removing after the iter completes) avoids holding shard locks across `remove`. The ctx7 DashMap docs confirm `iter()` yields `Ref` guards and `remove()` is safe on a non-overlapping key.

### Pattern 5: Env-var config knobs read once at startup
**What:** TTL/LRU/max-count/sweep defaults are env vars read once at startup (matching `RRULES_DEFAULT_ENGINE` at `main.rs:57-65`), not per-request.
**Example:**
```rust
// Source: RRULES_DEFAULT_ENGINE pattern [VERIFIED: main.rs:57-65, execution.rs:518-521]
fn read_session_config() -> SessionConfig {
    let idle_ttl = std::env::var("RRULES_SESSION_IDLE_TTL_SECS")
        .ok().and_then(|s| s.parse().ok()).map(std::time::Duration::from_secs)
        .unwrap_or(std::time::Duration::from_secs(900));
    let max_lifetime = std::env::var("RRULES_SESSION_MAX_LIFETIME_SECS")
        .ok().and_then(|s| s.parse().ok()).map(std::time::Duration::from_secs)
        .unwrap_or(std::time::Duration::from_secs(86400));
    let max_sessions = std::env::var("RRULES_MAX_SESSIONS")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(1024);
    let sweep_interval = std::env::var("RRULES_SESSION_SWEEP_INTERVAL_SECS")
        .ok().and_then(|s| s.parse().ok()).map(std::time::Duration::from_secs)
        .unwrap_or(std::time::Duration::from_secs(60));
    SessionConfig { idle_ttl, max_lifetime, max_sessions, sweep_interval }
}
```

### Pattern 6: `rrules_sessions_active` IntGauge on the Phase-24 dedicated `Registry`
**What:** Register an `IntGauge` on the same `prometheus::Registry` that `ExecutionCounters` holds (`metrics.rs:80`), so the gauge appears in `/metrics` text output alongside the existing execute counters. Set it to `SessionManager::len()` after each execute and on every sweep eviction.
**When to use:** D-08 — the leak becomes visible on the dashboard (Pitfall 5 step 4).
**Example:**
```rust
// Source: ExecutionCounters::new() pattern [VERIFIED: api/src/metrics.rs:94-160]
let sessions_active = IntGauge::new("rrules_sessions_active", "Active stateful session count")
    .expect("construct rrules_sessions_active");
registry.register(Box::new(sessions_active.clone()))
    .expect("register rrules_sessions_active");
// After each execute and after each sweep:
sessions_active.set(manager.len() as i64);
```
**Planner note:** The gauge can live on `SessionManager` directly (constructed with a `Registry` reference) or be owned by `ExecutionCounters` and set via a shared `Arc<IntGauge>`. The contract is "the gauge is registered on the Phase-24 dedicated Registry (NOT `default_registry()`) and reflects `SessionManager::len()`." The planner picks the ownership shape; mirroring `ExecutionCounters` (gauge on the manager, registry passed in) is cleanest.

### Pattern 7: `execute_single` branch on `req.session_id`
**What:** The stateless path (Phase 23, `execution.rs:504-575`) runs unchanged when `session_id` is absent. When present, the sessionful path runs: lookup-or-create, lock per-session Mutex, insert facts, `web::block` execute, update `last_used`, build response with server UUID + creation generation, update gauge. The `web::Data<Arc<SessionManager>>` is a new param.
**Example:**
```rust
// Source: execute_single [VERIFIED: execution.rs:504-575] + D-05/D-06
pub async fn execute_single(
    req: ExecuteRequest,
    registry: web::Data<EngineRegistry>,
    selector: web::Data<EngineSelector>,
    ruleset: web::Data<Arc<RwLock<SharedRuleSet>>>,
    counters: web::Data<ExecutionCounters>,
    metrics_tx: web::Data<MetricsTx>,
    session_manager: web::Data<Arc<SessionManager>>,   // NEW (D-12)
) -> Result<ExecuteResponse, (StatusCode, String)> {
    let execution_id = Uuid::new_v4().to_string();

    // Branch: sessionful vs stateless (D-05).
    if let Some(client_alias) = req.session_id.clone() {
        // Sessionful path. Resolve engine name + validate (D-06).
        // Lookup-or-create via SessionManager.
        // Lock per-session Mutex, insert facts, web::block execute,
        // update last_used, build response w/ server UUID + creation gen,
        // update gauge.
        // ... (see Pattern 2)
    } else {
        // Phase-23 stateless path — UNCHANGED (execution.rs:518-689).
        // ... existing body ...
    }
}
```

### Anti-Patterns to Avoid
- **Holding the DashMap shard guard across `web::block`:** the shard lock serializes unrelated sessions on the same shard; clone the `Arc<Mutex<Entry>>` out, drop the guard, THEN lock the per-session Mutex. `[CITED: DashMap sharding semantics — ctx7 /xacrimon/dashmap]`
- **Using `tokio::sync::Mutex` for the per-session lock and holding it across `web::block`:** the engine is `!Sync` and `execute()` is CPU-bound; `std::sync::Mutex` is correct. Holding a `tokio::Mutex` guard across `web::block` is an anti-pattern.
- **Client `session_id` as the primary key:** caller-collision leaks (a caller generating fresh IDs per call still leaks, but a caller reusing an id collides with prior state). D-04 makes the server UUID the primary key and the client id an alias. `[CITED: PITFALLS.md Pitfall 5 step 3]`
- **Shipping TTL without max-count (or any partial fix):** a partial fix still leaks under caller-collision or burst creation. D-03 ships all four controls together. `[CITED: PITFALLS.md Pitfall 5]`
- **Wrapping the v1.0 `engine::environment::Session`:** it is a non-functional stub (`execute()` returns "Not implemented", `insert_fact` is a no-op, raw `*const Engine` not `Send`). D-01 wraps `Box<dyn ExecutionEngine>` instead. `[VERIFIED: engine/src/environment.rs:152-208]`
- **Changing engines mid-session:** would discard accumulated working memory. D-06 returns HTTP 400 on conflicting `engine` param. `[CITED: 27-CONTEXT.md D-06]`
- **Per-request env-var reads for TTL/LRU knobs:** env reads are cheap but the established pattern (`RRULES_DEFAULT_ENGINE`) reads once at startup. Read once, pass a `SessionConfig` to `SessionManager::new()`. `[CITED: main.rs:57-65]`
- **Registering `rrules_sessions_active` on `prometheus::default_registry()`:** `register()` panics on duplicate names; the api crate uses a dedicated `Registry::new()` (`metrics.rs:80`) to avoid colliding with the engine crate's metrics. `[VERIFIED: api/src/metrics.rs:78-80]`

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Concurrent session map | `RwLock<HashMap<String, Entry>>` (single global write lock serializes creation) | `DashMap<String, Arc<Mutex<Entry>>>` (sharded) | Sharding avoids the Pitfall-1 regression; DashMap is `Send + Sync` and battle-tested (5.18M weekly downloads). `[VERIFIED: crates.io]` |
| Session ID generation | Custom UUID-ish string / caller-supplied ID as primary key | `Uuid::new_v4()` (server-generated, already a dep) | UUID v4 is collision-resistant and standard; caller-supplied IDs are idempotency aliases (D-04). `[VERIFIED: uuid crate in api/Cargo.toml]` |
| Per-session mutual exclusion | `RwLock` (the engine is `&mut self`, no readers benefit) | `std::sync::Mutex` | The engine is `!Sync`; a `Mutex` is the minimum viable exclusion. `RwLock` adds overhead with no read-concurrency benefit. `[VERIFIED: registry.rs:64-108]` |
| TTL sweep loop | Manual thread + sleep | `tokio::time::interval` | Standard tokio primitive; matches `start_file_watcher`'s spawn pattern. `[VERIFIED: ctx7 /tokio-rs/tokio]` |
| Active-session gauge | Custom AtomicU64 + manual `/metrics` text rendering | `prometheus::IntGauge` on the dedicated `Registry` | Phase 24 already established the dedicated-`Registry` + `TextEncoder` pattern; reusing it keeps `/metrics` output consistent. `[VERIFIED: api/src/metrics.rs]` |
| Engine creation | Reimplementing `EngineRegistry::create_engine_by_name` | `registry.create_engine_by_name(&engine_name)` (Phase 23 path) | The sessionful path reuses the same factory path as the stateless path (`execution.rs:535-549`); no new engine-construction code. `[VERIFIED: registry.rs:222]` |
| Ruleset loading at session creation | Reimplementing `load_ruleset` | `engine.load_ruleset(&guard.ruleset)` (Phase 23 path) | The sessionful path reads the shared `SharedRuleSet` once at creation and releases before execute, exactly as the stateless path does. `[VERIFIED: execution.rs:559-575]` |

**Key insight:** Phase 27 is a *session-lifecycle* layer over the Phase-23 execution surface. The engine, ruleset, factory, and `web::block` patterns all already exist. The new code is: the registry data structure, the lifecycle routes, the sweep task, and the gauge. Everything else is reuse.

## Runtime State Inventory

> Phase 27 is a NEW feature phase (greenfield session registry), NOT a rename/refactor/migration. The Runtime State Inventory is included for completeness because the sweep task introduces a new runtime background process — but there is no existing state to migrate.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — sessions are in-memory only (`DashMap`); no DB/Postgres/Redis persistence. Verified by D-02 (in-process `DashMap`). | None (sessions are ephemeral by design) |
| Live service config | None — `SessionManager` is constructed in `main.rs` and injected via `web::Data`; no external service holds session config. | None |
| OS-registered state | None — the sweep task is a `tokio::spawn` (in-process), NOT a systemd unit / launchd plist / Windows Task Scheduler task. | None |
| Secrets/env vars | NEW env vars: `RRULES_SESSION_IDLE_TTL_SECS`, `RRULES_SESSION_MAX_LIFETIME_SECS`, `RRULES_MAX_SESSIONS`, `RRULES_SESSION_SWEEP_INTERVAL_SECS`. Read once at startup (Pattern 5). No existing secrets/env vars are renamed. | None (additive only) |
| Build artifacts | `api/Cargo.toml` gains `dashmap = "6.2.1"` — `cargo build` will fetch and compile the new dep. No stale artifacts (it's a new dep, not a rename). | `cargo build` after Cargo.toml edit |

**Nothing found in category:** N/A — this is a greenfield session registry; the only "runtime state" is the new in-memory `DashMap`, which is empty at startup and populated by requests.

## Common Pitfalls

### Pitfall 1: Holding the DashMap shard guard across `web::block`
**What goes wrong:** If `execute_single` does `manager.sessions.get(&uuid)` and holds the `Ref` guard while locking the per-session Mutex and calling `web::block`, the DashMap shard lock is held for the entire engine execution duration. Two unrelated sessions that hash to the same shard serialize — reintroducing the Pitfall-1 regression Phase 23 spent effort avoiding.
**Why it happens:** The natural `map.get(&k).lock().execute()` chain holds both locks.
**How to avoid:** Clone the `Arc<Mutex<StatefulSessionEntry>>` out (`let entry = manager.sessions.get(&uuid).unwrap().clone();` — the guard drops here), then lock the per-session Mutex. The DashMap shard is released before `web::block`.
**Warning signs:** Load tests showing same-shard sessions serializing; flamegraphs with `DashMap` shard locks held across `spawn_blocking`.

### Pitfall 2: Using `tokio::sync::Mutex` for the per-session lock
**What goes wrong:** `tokio::Mutex` is async-aware but holding its guard across `web::block` (a blocking call) is an anti-pattern; the guard is `!Send` across the blocking boundary in some configurations, and it ties up an async worker thread.
**Why it happens:** "It's an async handler, so use an async Mutex" is a common intuition.
**How to avoid:** Use `std::sync::Mutex` for the per-session lock. The engine work is CPU-bound and runs in `web::block`; `std::sync::Mutex` is correct and cheaper. The established pattern in rRules (`ExecutionCounters`, `CoverageTracking`, `RuleExecutionTracker`) uses `std::sync::Mutex` (or `parking_lot::Mutex`) for blocking-internal state.
**Warning signs:** `tokio::sync::Mutex` imports in `sessions.rs`; `await` between lock and `web::block`.

### Pitfall 3: Client `session_id` as the primary key (caller-collision leak)
**What goes wrong:** If the client `session_id` is the map key directly, a caller that generates fresh IDs per call leaks sessions until `MAX_SESSIONS` evicts them (bounded but wasteful), and a caller that reuses an id collides with prior state (silent data corruption).
**Why it happens:** `ExecuteRequest.session_id` already exists (`execution.rs:23`); using it as the key is the obvious path.
**How to avoid:** D-04: server-generated `Uuid::new_v4()` is the primary key; the client id is an alias mapped via the second `DashMap<String, Uuid>`. Resume-on-match prevents collision; TTL+LRU+max-count bound the leak.
**Warning signs:** `sessions.insert(req.session_id.unwrap(), ...)` directly; no `aliases` map.

### Pitfall 4: Shipping a partial OOM fix (TTL without max-count, etc.)
**What goes wrong:** A partial fix still leaks. TTL without max-count leaks under burst creation; max-count without TTL keeps stale sessions forever; LRU without a gauge makes the leak invisible.
**Why it happens:** "Ship TTL now, add the rest later" under time pressure.
**How to avoid:** D-03: ship TTL + LRU + max-count + sweep + gauge TOGETHER. The OOM failure mode is silent (Pitfall 5).
**Warning signs:** Any of the four controls missing from the plan; the gauge absent from `/metrics`.

### Pitfall 5: The v1.0 `engine::environment::Session` stub looks usable
**What goes wrong:** A planner reading `environment.rs:102` (`Engine::create_session`) might assume the v1.0 `Session` is the surface to wrap. It is not: `execute()` returns `"Not implemented"` (`environment.rs:191`), `insert_fact` is a no-op (`environment.rs:176`), and it holds a raw `*const Engine` pointer (`environment.rs:154`, not `Send`).
**Why it happens:** The v1.0 API exists and looks plausible.
**How to avoid:** D-01: wrap `Box<dyn ExecutionEngine>` (the Phase-23 surface). Document the v1.0 stub but do not delete it (other v1.0 code references it). The `EngineRegistry::create_engine_by_name` path is the real factory.
**Warning signs:** `use engine::environment::Session` in `sessions.rs`; `Engine::create_session` calls in the sessionful path.

### Pitfall 6: Forgetting to update the gauge on sweep eviction
**What goes wrong:** If the gauge is only set on execute, a sweep that evicts 100 sessions leaves `rrules_sessions_active` reporting stale (too high) until the next execute. The dashboard lies about the live count.
**Why it happens:** The sweep task and the execute path both need to set the gauge; it's easy to wire only one.
**How to avoid:** D-08: set `sessions_active.set(self.len() as i64)` in BOTH the execute path (after each execute) AND `sweep_expired` (after the eviction loop).
**Warning signs:** Gauge reading higher than actual session count after a sweep; SC-3 asserting the capped count fails because the gauge wasn't updated.

### Pitfall 7: `web::block` closure capturing a `!Send` guard
**What goes wrong:** `web::block` requires the closure to be `Send + 'static`. A `std::sync::MutexGuard` is `!Send`; capturing it in the closure fails to compile, or forces a workaround that reintroduces Pitfall 1.
**Why it happens:** The natural "lock, then `web::block(move || { engine.execute() })`" captures the guard.
**How to avoid:** Either (a) clone the engine OUT of the session under the lock, drop the guard, move the engine into `web::block`, execute, then re-lock to write back post-execution state; or (b) restructure so the closure owns the engine by value. The contract is "no DashMap shard lock held across `web::block`" — the per-session Mutex can be released before `web::block` if the engine is moved out.
**Warning signs:** Compile error `MutexGuard is not Send`; `web::block` closure borrowing a guard.

## Code Examples

### `SessionManager::create_session` (lookup-or-create with D-06 engine-fixed check)
```rust
// Source: D-04/D-06 + EngineRegistry::create_engine_by_name [VERIFIED: registry.rs:222]
use engine::engine::registry::ExecutionEngine;

impl SessionManager {
    /// Lookup by alias, or create a new session. Returns (server_uuid, entry_arc).
    /// On a resume with a conflicting engine param, returns a 400 error (D-06).
    pub fn lookup_or_create(
        &self,
        client_alias: &str,
        req_engine: Option<&str>,
        registry: &EngineRegistry,
        ruleset: &Arc<RwLock<SharedRuleSet>>,
    ) -> Result<(Uuid, Arc<Mutex<StatefulSessionEntry>>), (StatusCode, String)> {
        // 1. Resume path (O(1) via alias map).
        if let Some(uuid) = self.aliases.get(client_alias).map(|r| *r) {
            if let Some(entry_arc) = self.sessions.get(&uuid).map(|r| r.clone()) {
                // D-06: engine is fixed at creation. Check conflicting engine param.
                let g = entry_arc.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("lock poisoned: {e}")))?;
                if let Some(req_eng) = req_engine {
                    if req_eng != g.engine_name {
                        return Err((StatusCode::BAD_REQUEST,
                            format!("session engine is fixed: expected {}, got {}", g.engine_name, req_eng)));
                    }
                }
                return Ok((uuid, entry_arc));
            }
            // Stale alias (session evicted) — fall through to create.
        }

        // 2. Create path. Resolve engine name + validate (D-06, Phase-23 path).
        let engine_name = req_engine.cloned().unwrap_or_else(|| {
            std::env::var("RRULES_DEFAULT_ENGINE").unwrap_or_else(|_| "rete".to_string())
        });
        let available = registry.available_engines();
        if !available.iter().any(|n| *n == engine_name.as_str()) {
            return Err((StatusCode::BAD_REQUEST, format!("unknown engine: {}", engine_name)));
        }
        let mut engine = match registry.create_engine_by_name(&engine_name) {
            Some(Ok(e)) => e,
            Some(Err(e)) => return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("engine creation failed: {e}"))),
            None => return Err((StatusCode::BAD_REQUEST, format!("unknown engine: {}", engine_name))),
        };

        // 3. Load the shared ruleset once at creation, capture generation (D-07/L-03).
        let (generation,): (u64,) = {
            let guard = ruleset.read().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("ruleset lock poisoned: {e}")))?;
            engine.load_ruleset(&guard.ruleset).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("load_ruleset failed: {e}")))?;
            (guard.generation,)
        };

        // 4. Insert into registry + alias map.
        let server_uuid = Uuid::new_v4();
        let now = Instant::now();
        let entry = StatefulSessionEntry {
            engine, engine_name, engine_generation: generation,
            last_used: now, created_at: now, server_id: server_uuid,
            client_alias: Some(client_alias.to_string()),
        };
        let entry_arc = Arc::new(Mutex::new(entry));
        self.sessions.insert(server_uuid, entry_arc.clone());
        self.aliases.insert(client_alias.to_string(), server_uuid);
        self.sessions_active.set(self.sessions.len() as i64);

        Ok((server_uuid, entry_arc))
    }
}
```

### DELETE /v1/execute/session/{id} + POST /v1/execute/session/{id}/reset
```rust
// Source: D-05 + actix-web web::Path pattern [VERIFIED: main.rs routes]
/// DELETE /v1/execute/session/{id} — dispose session (D-05).
pub async fn dispose_session(
    id: web::Path<String>,
    manager: web::Data<Arc<SessionManager>>,
) -> Result<HttpResponse, (StatusCode, String)> {
    let alias = id.into_inner();
    // Resolve alias → UUID, remove from both maps, update gauge.
    let removed = manager.dispose_by_alias(&alias);
    if removed {
        Ok(HttpResponse::Ok().json(serde_json::json!({"status": "disposed", "session_id": alias})))
    } else {
        Err((StatusCode::NOT_FOUND, format!("session not found: {}", alias)))
    }
}

/// POST /v1/execute/session/{id}/reset — clear WM without disposing (D-05).
pub async fn reset_session(
    id: web::Path<String>,
    manager: web::Data<Arc<SessionManager>>,
) -> Result<HttpResponse, (StatusCode, String)> {
    let alias = id.into_inner();
    match manager.lookup(&alias) {
        Some((_, entry_arc)) => {
            let mut g = entry_arc.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("lock poisoned: {e}")))?;
            g.engine.reset();   // clears working memory (registry.rs:84 / rete/mod.rs:525)
            g.last_used = Instant::now();
            Ok(HttpResponse::Ok().json(serde_json::json!({"status": "reset", "session_id": alias})))
        }
        None => Err((StatusCode::NOT_FOUND, format!("session not found: {}", alias))),
    }
}
```
**Note:** `reset()` on the real `ReteEngine` clears `working_facts` (and the network/alpha/beta — see `rete/mod.rs:525-532`). The planner should verify whether `reset()` also clears the loaded `rules` (`rete/mod.rs:530` does `self.rules.clear()`) — if so, the sessionful reset path must re-`load_ruleset` after `reset()` to keep the session's ruleset intact while clearing only working memory. The contract is "reset clears WM without disposing"; if `engine.reset()` is too aggressive, the planner should clear `working_facts` directly (or rebuild a fresh engine with the same ruleset + generation) instead of calling `reset()`. `[VERIFIED: rete/mod.rs:525-532 — reset() clears rules too]` — **this is a planner decision point**: either re-load the ruleset after `reset()`, or clear WM without `reset()`.

### main.rs wiring (construct + spawn sweep + app_data + routes)
```rust
// Source: main.rs [VERIFIED: main.rs:44-102, 146, 186-198, 208-267]
// Construct SessionManager once at startup (after ruleset, before HttpServer).
let session_config = read_session_config();  // Pattern 5
let sessions_active = prometheus::IntGauge::new("rrules_sessions_active", "Active stateful session count")
    .expect("construct gauge");
// Register on the Phase-24 dedicated Registry (counters.registry).
// Option A: pass counters.registry into SessionManager::new.
// Option B: construct a separate Registry for the gauge (cleaner if SessionManager owns it).
counters.registry.register(Box::new(sessions_active.clone()))
    .expect("register rrules_sessions_active");
let session_manager = std::sync::Arc::new(SessionManager::new(session_config, sessions_active));
let sweep_manager = session_manager.clone();

// Spawn the sweep task alongside start_file_watcher (D-10).
tokio::spawn(v1::sessions::run_session_sweep(sweep_manager));

// In HttpServer::new closure, add:
//   .app_data(web::Data::new(session_manager.clone()))
// In the /v1 scope, add:
//   .route("/execute/session/{id}", web::delete().to(v1::sessions::dispose_session))
//   .route("/execute/session/{id}/reset", web::post().to(v1::sessions::reset_session))
```

### MetricsSnapshot.active_sessions population (execution.rs:651-660)
```rust
// Source: execution.rs:651-660 [VERIFIED: codebase] + D-08
// Replace `active_sessions: 0,` at execution.rs:657 with:
let snapshot = MetricsSnapshot {
    engine_name: engine_name.clone(),
    rules_fired: counters.snapshot_rules_fired.load(Ordering::Relaxed),
    facts_inserted: counters.snapshot_facts_inserted.load(Ordering::Relaxed),
    active_sessions: session_manager.len(),   // NEW — Phase 27 (D-08)
    memory_bytes: 0,
    last_updated: chrono::Utc::now().to_rfc3339(),
};
```
**Note:** `execute_single` will need `session_manager: web::Data<Arc<SessionManager>>` as a new param (D-12) so `len()` is available. For the stateless path (no session), the manager is still present (shared state) and `len()` reflects whatever sessions exist from other callers.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| v1.0 `engine::environment::Session` (raw `*const Engine`, `execute()` returns "Not implemented") | Phase-23 `Box<dyn ExecutionEngine>` via `EngineRegistry::create_engine_by_name` | Phase 23 (v5.0) | The real execution surface; D-01 wraps this, not the v1.0 stub. |
| `execute_with_session` stub returning `rules_fired: 0` (`execution.rs:1081-1094`) | `execute_single` branches on `session_id`; stub removed/repurposed | Phase 27 | The sessionful path is real; no production code returns `rules_fired: 0` for a sessionful call. |
| `MetricsSnapshot.active_sessions: 0` hardcoded (Phase 24, `execution.rs:657`) | `SessionManager::len()` populates the field + `rrules_sessions_active` IntGauge | Phase 27 | The leak becomes visible on the dashboard (Pitfall 5 step 4). |
| `HashMap`-based session stores (Pitfall 5 recommendation) | `DashMap<String, Arc<Mutex<Entry>>>` (sharded, D-02) | Phase 27 | No global write lock; different sessions stay concurrent. |

**Deprecated/outdated:**
- `engine::environment::Session` (`environment.rs:152-208`): non-functional v1.0 stub. Documented (D-01) but left in place (other v1.0 code references it); a future cleanup phase can remove it once confirmed unused.
- `execute_with_session` (`execution.rs:1081-1094`): returns `rules_fired: 0`, not wired to the registry. Removed/repurposed in Phase 27 (D-05).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `std::time::Instant` is sufficient for `last_used`/`created_at` (monotonic, cheaper than `chrono`). | Standard Stack / Alternatives | LOW — if wall-clock timestamps are needed for observability, `chrono::Utc::now()` can be added alongside; both are already deps. |
| A2 | The real-short-TTL approach (`RRULES_SESSION_IDLE_TTL_SECS=1` + `tokio::time::sleep`) is the recommended SC-2 mechanism (vs. an injectable fake clock). | Alternatives Considered / Validation Architecture | LOW — if the planner prefers determinism, a fake-clock trait is a viable alternative; both are within D-discretion. |
| A3 | `engine.reset()` on `ReteEngine` clears `working_facts` AND `rules` (`rete/mod.rs:525-532`), so reset semantics may require re-`load_ruleset` or a direct WM clear. | Code Examples (reset_session) | MEDIUM — if `reset()` is too aggressive, the reset route must clear WM without `reset()`. Flagged as a planner decision point. |
| A4 | The second `DashMap<String, Uuid>` for alias lookup is the recommended approach (over a field + linear scan). | Architecture Patterns / Pattern 3 | LOW — both are within D-12 discretion; the second-DashMap is O(1) and clean. |
| A5 | `web::block`'s closure must not capture a `!Send` `std::sync::MutexGuard`; the engine should be moved out (by value) into the closure, OR the guard dropped before `web::block`. | Pitfalls / Pitfall 7 | MEDIUM — the exact compile-safe shape is a planner implementation detail; the load-bearing invariant is "no DashMap shard lock held across `web::block`". |
| A6 | The `rrules_sessions_active` IntGauge is registered on the Phase-24 dedicated `Registry` (the one `ExecutionCounters` holds), not a separate registry. | Pattern 6 | LOW — a separate Registry is viable but would require a second `TextEncoder` gather in `metrics_endpoint`; reusing the existing Registry is cleaner. |

## Open Questions

1. **Does `ReteEngine::reset()` clear loaded rules, requiring re-`load_ruleset` after reset?**
   - What we know: `rete/mod.rs:525-532` shows `reset()` clears `network`, `alpha_memory`, `beta_memory`, `working_facts`, AND `rules` (line 530).
   - What's unclear: whether the sessionful reset path should (a) call `reset()` then re-`load_ruleset` (clean WM, keep ruleset), (b) clear `working_facts` directly without `reset()` (requires engine-specific access), or (c) rebuild a fresh engine with the same ruleset + generation.
   - Recommendation: the planner should implement `reset_session` to rebuild a fresh engine (`registry.create_engine_by_name` + `load_ruleset` with the session's captured generation) rather than relying on `reset()`, since `reset()` clears the rules too. This keeps the session's engine/generation stable while clearing WM. Alternatively, if `reset()` is deemed correct (WM + rules both cleared, then re-load), the planner documents which.

2. **Sweep-task shutdown mechanism**
   - What we know: `start_file_watcher` (`main.rs:146`) uses `tokio::spawn` with no explicit cancellation; the task lives for the process lifetime.
   - What's unclear: whether a `CancellationToken` is already in use elsewhere (no evidence in `main.rs`).
   - Recommendation: match `start_file_watcher` (no `CancellationToken`) — the sweep task lives for the process lifetime and tears down with the runtime. D-10 discretion.

3. **`web::block` closure shape for the sessionful path**
   - What we know: `web::block` requires `Send + 'static`; `std::sync::MutexGuard` is `!Send`.
   - What's unclear: the exact compile-safe shape (clone engine out + drop guard + move engine in, vs. another approach).
   - Recommendation: clone/move the engine OUT of the session under the lock, drop the guard, move the engine into `web::block`, execute, then re-lock to write back `last_used` and read `get_facts()`. This is the cleanest compile-safe shape and preserves "no DashMap shard lock held across `web::block`".

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` | Build | ✓ | (rust toolchain) | — |
| `dashmap` 6.2.1 | Session registry | ✓ on crates.io (verified) | 6.2.1 (pinned) | — |
| `tokio` (full) | Sweep task (`time::interval`, `spawn`) | ✓ (already a dep) | 1.35 | — |
| `prometheus` | IntGauge | ✓ (already a dep) | 0.13 (workspace) | — |
| `uuid` | Session IDs | ✓ (already a dep) | 1.0 | — |
| `chrono` | Timestamps (optional, Instant preferred) | ✓ (already a dep) | 0.4 | Use `Instant` |
| `actix-web` | Routes, web::Data, web::block | ✓ (already a dep) | 4.4 | — |
| `serial_test` | Test isolation | ✓ (already a dev-dep) | 3 (workspace) | — |

**Missing dependencies with no fallback:** none — all required crates are either already deps or being added (`dashmap` 6.2.1) to `api/Cargo.toml`.

**Missing dependencies with fallback:** none.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `actix_web::test` + `serial_test` (workspace dev-dep, established Phase 23/26) |
| Config file | none (test wiring lives in `api/tests/execute_integration.rs` macros: `init_test_app!`, `init_two_rule_app!`) |
| Quick run command | `cargo test -p api --test execute_integration` |
| Full suite command | `cargo test -p api && cargo test -p engine` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SESS-01 / SC-1 | Same-`session_id` accumulates WM across two calls (`score:10` then `score:20`) | integration (e2e) | `cargo test -p api --test execute_integration -- stateful_session_accumulates_wm --exact` | ❌ Wave 0 (NEW) |
| SESS-01 / SC-2 | Session untouched >TTL is gone; touched within TTL persists | integration (e2e) | `cargo test -p api --test execute_integration -- session_ttl_evicts_idle --exact` | ❌ Wave 0 (NEW) |
| SESS-01 / SC-3 | LRU eviction at `MAX_SESSIONS`; gauge reflects capped count | integration (e2e) | `cargo test -p api --test execute_integration -- lru_evicts_oldest_and_gauge_capped --exact` | ❌ Wave 0 (NEW) |
| SESS-01 / SC-4 | DELETE disposes; reset clears WM without disposing | integration (e2e) | `cargo test -p api --test execute_integration -- session_lifecycle_delete_reset --exact` | ❌ Wave 0 (NEW) |

### Sampling Rate
- **Per task commit:** `cargo test -p api --test execute_integration` (the four SC tests + existing Phase-23/24/25/26 tests — all `#[serial]`)
- **Per wave merge:** `cargo test -p api && cargo test -p engine`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 RED→GREEN Strategy (D-09)
The four SC tests are written RED first (compile, but assert against behavior that doesn't exist yet), then turned GREEN by implementing `SessionManager` + the `execute_single` branch + the routes + the sweep task + the gauge.

**SC-1 (accumulated WM — keystone honesty anchor):**
1. `POST /v1/execute` with `session_id: "s1"`, `facts: {scorecard: {score: 0}}` → `rules_fired >= 1`, `facts_after.scorecard.score == 10`, response carries `session_id: "<server-uuid>"`.
2. `POST /v1/execute` with the SAME `session_id: "s1"` and NO new facts (`facts: {}`) → fires `r1` again against accumulated WM → `facts_after.scorecard.score == 20`.
3. Assert `score == 20` (not `10`) — proves WM accumulated. The stateless path would return `score:10` both times (it starts from request facts each call).
4. `#[serial]` + `SessionManager::clear()` at test start.

**SC-2 (TTL — recommended real-short-TTL mechanism):**
1. Set `RRULES_SESSION_IDLE_TTL_SECS=1` for this test (restore after).
2. `POST /v1/execute` with `session_id: "s2"`, `facts: {scorecard: {score: 0}}` → `score:10`.
3. `tokio::time::sleep(Duration::from_millis(1100)).await` (just over 1s TTL).
4. Trigger a sweep (either call `SessionManager::sweep_expired()` directly via a test helper, or wait for the 60s interval — the former is deterministic; recommend a direct `sweep_expired()` call for the test).
5. Assert a subsequent `POST /v1/execute` with `session_id: "s2"` creates a FRESH session (`score:10` again, not `score:20`) — the old session was evicted.
6. Counter-assert: a session touched within TTL (no sleep) persists — second call returns `score:20`.
7. `#[serial]` + `clear()` + env-var save/restore.
   - **Planner note:** if a direct `sweep_expired()` call is undesirable (white-box), the planner can inject a fake clock or set `RRULES_SESSION_SWEEP_INTERVAL_SECS=1` + sleep. The contract is "SC-2 must be deterministic and fast" (<30s).

**SC-3 (LRU + gauge):**
1. Set `RRULES_MAX_SESSIONS=2`.
2. Create 3 sessions in order: `s1` (touch), `s2` (touch), `s3` (touch) — each `POST /v1/execute` with the respective `session_id` + `facts: {scorecard: {score: 0}}`.
3. Trigger `sweep_expired()` (or rely on the create-path LRU eviction — recommend making create-path evict LRU when `len > max_sessions`, per D-03 condition (c)).
4. Assert `s1` (the LRU — oldest `last_used`) is evicted: a subsequent `POST /v1/execute` with `session_id: "s1"` creates a fresh session (`score:10`, not `score:20`).
5. Assert `s2` and `s3` persist: `POST /v1/execute` with `session_id: "s2"` resumes (`score:20`).
6. Assert `rrules_sessions_active` gauge (read from `/metrics` text or the WS `MetricsSnapshot`) reads `2` — NOT `3`.
7. `#[serial]` + `clear()` + env-var save/restore.

**SC-4 (lifecycle):**
1. `POST /v1/execute` with `session_id: "s4"`, `facts: {scorecard: {score: 0}}` → `score:10`.
2. `DELETE /v1/execute/session/s4` → 200 `{"status": "disposed"}`.
3. `POST /v1/execute` with `session_id: "s4"` (same id) → creates a FRESH session (`score:10`, not `score:20`) — proves dispose worked.
4. `POST /v1/execute` with `session_id: "s5"`, `facts: {scorecard: {score: 0}}` → `score:10`.
5. `POST /v1/execute` with `session_id: "s5"` (no new facts) → `score:20` (accumulated).
6. `POST /v1/execute/session/s5/reset` → 200 `{"status": "reset"}`.
7. `POST /v1/execute` with `session_id: "s5"` (no new facts) → `score:10` (not `score:20`) — proves reset cleared WM without disposing (the session resumed with empty WM but the same engine/generation).
8. `#[serial]` + `clear()`.

### Wave 0 Gaps
- [ ] `api/tests/execute_integration.rs` — add SC-1/SC-2/SC-3/SC-4 tests (extend the existing `init_test_app!` macro to include `shared_session_manager()` app_data; add a `configure_test_routes` extension for DELETE/reset routes)
- [ ] `api/src/v1/sessions.rs` — NEW module (`SessionManager`, `StatefulSessionEntry`, `run_session_sweep`, `dispose_session`, `reset_session`, `read_session_config`)
- [ ] `api/Cargo.toml` — add `dashmap = "6.2.1"` to `[dependencies]`
- [ ] `api/src/main.rs` — construct `SessionManager`, spawn sweep, add `app_data`, register DELETE/reset routes
- [ ] `api/src/v1/execution.rs` — add `ExecuteResponse.session_id: Option<String>`, branch `execute_single` on `req.session_id`, populate `active_sessions` from `SessionManager::len()`, remove/repurpose `execute_with_session`

*(If no gaps: N/A — this is a new-feature phase; all infrastructure is new.)*

## Security Domain

> `security_enforcement` is not explicitly `false` in `.planning/config.json` (absent = enabled). Phase 27 introduces session lifecycle endpoints and an in-memory registry — security-relevant but not a primary security phase. ASVS categories are assessed below.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes (indirectly) | Phase 22 JWT middleware wraps all `/v1/*` routes (`main.rs:216-218`); the new DELETE/reset routes inherit JWT validation. No new auth code. |
| V3 Session Management | yes | The `SessionManager` IS a session management subsystem. Bounded resource use (TTL/LRU/max-count — D-03) is the primary control. Server-generated UUID v4 IDs (D-04) prevent caller-collision attacks. The registry is in-memory only (no persistent session tokens). |
| V4 Access Control | yes (indirectly) | Phase 22 JWT middleware; no new RBAC in Phase 27 (the deferred `/v1/sessions` admin endpoint would need RBAC — out of scope). |
| V5 Input Validation | yes | `session_id` is a client-supplied `Option<String>`; treated as an idempotency alias (D-04), not trusted as a primary key. Engine param validated against `registry.available_engines()` (Phase-23 path); conflicting engine on resume → 400 (D-06). Route path params (`{id}`) are actix-validated strings. |
| V6 Cryptography | no | No new crypto; UUID v4 is generation, not cryptography. |

### Known Threat Patterns for Rust actix-web + in-memory session registry

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Session leak / OOM (Pitfall 5) | Denial of Service | TTL + LRU + max-count + background sweep + Prometheus gauge — all ship together (D-03). `[CITED: PITFALLS.md Pitfall 5]` |
| Caller-collision / session hijack via client-supplied ID | Spoofing / Tampering | Server-generated UUID v4 primary key; client ID is an alias only (D-04). `[CITED: 27-CONTEXT.md D-04]` |
| Cross-session data leakage | Information Disclosure | Per-session `Mutex<Box<dyn ExecutionEngine>>`; each session has its own engine + WM; no cross-session sharing (anti-feature, FEATURES.md). |
| Concurrent same-session calls corrupting engine state | Tampering | Per-session `std::sync::Mutex` serializes same-session calls (D-11). `[CITED: 27-CONTEXT.md D-11]` |
| DashMap shard lock held across `web::block` (DoS via serialization) | Denial of Service | Clone `Arc<Mutex<Entry>>` out, drop guard, then lock (Pitfall 1 above). |
| Stale alias pointing to evicted session | Information Disclosure | `lookup()` falls through to create if the alias maps to a UUID not in the registry (Pattern 3). |

## Sources

### Primary (HIGH confidence)
- DashMap 6.2.1 API — `entry()`, `get()`, `remove()`, `iter()`, `len()`, sharding semantics, `Arc<DashMap>` sharing across threads. `[VERIFIED: ctx7 /xacrimon/dashmap llms.txt]`
- `tokio::time::interval` + `tokio::spawn` — background sweep task pattern. `[VERIFIED: ctx7 /tokio-rs/tokio api-reference/time.md]`
- `api/src/v1/execution.rs` — `execute_single` (504-689), `ExecuteRequest.session_id` (23), `ExecuteResponse.engine_generation` (63), `MetricsSnapshot.active_sessions: 0` (657), `execute_with_session` stub (1081-1094), `start_file_watcher` (1024), `SharedRuleSet` (83), `web::block` (608). `[VERIFIED: codebase read]`
- `engine/src/engine/registry.rs` — `ExecutionEngine` trait (`execute(&mut self)`, `insert_fact(&mut self)`, `reset(&mut self)`, `load_ruleset`, `get_facts` — 64-108), `EngineRegistry::create_engine_by_name` (222), `available_engines()` (234). `[VERIFIED: codebase read]`
- `engine/src/engine/rete/mod.rs` — `reset()` clears `working_facts` AND `rules` (525-532), `load_ruleset` (538), `get_facts` (545). `[VERIFIED: codebase read]`
- `engine/src/environment.rs` — v1.0 `Session` stub (152-208): `execute()` returns "Not implemented" (191), `insert_fact` no-op (176), raw `*const Engine` (154). `[VERIFIED: codebase read]`
- `api/src/main.rs` — `web::Data` wiring (44-102), `start_file_watcher` spawn (146), `.app_data` chain (186-198), `/v1` scope (208-267), `RRULES_DEFAULT_ENGINE` (57-65). `[VERIFIED: codebase read]`
- `api/src/metrics.rs` — `MetricsSnapshot` (18-25), `ExecutionCounters` + dedicated `Registry` (56-160), `IntCounterVec`/`HistogramVec` registration pattern. `[VERIFIED: codebase read]`
- `api/Cargo.toml` — deps (uuid 1.0, chrono 0.4, tokio 1.35 full, prometheus workspace, serial_test workspace); `dashmap` NOT yet present (confirmed absent). `[VERIFIED: codebase read]`
- `api/tests/execute_integration.rs` — `init_test_app!` macro (304-320), `configure_test_routes` (269-281), `reset_test_env()` (326-338) with `CoverageTracking::clear()` + `RuleExecutionTracker::clear()` + env-var pinning, `score_plus_10_rule()` (59-83), `#[serial_test::serial]` pattern. `[VERIFIED: codebase read]`

### Secondary (MEDIUM confidence)
- `27-CONTEXT.md` D-01..D-12 — locked decisions (the source of truth for this research). `[CITED: .planning/phases/27-stateful-sessions/27-CONTEXT.md]`
- `.planning/research/PITFALLS.md` Pitfall 5 (sessions never expire / leak to OOM) — TTL + LRU + max-count + sweep + gauge ship together. `[CITED: .planning/research/PITFALLS.md]`
- `.planning/research/FEATURES.md` §"Stateful KIE-session-style sessions" + §"Anti-Features" (cross-request shared WM is an anti-feature). `[CITED: .planning/research/FEATURES.md]`
- `.planning/ROADMAP.md` §"Phase 27" — goal, deliverables, SC-1..SC-4, Pitfall 5, research flags. `[CITED: .planning/ROADMAP.md]`

### Tertiary (LOW confidence)
- `crates.io API` for `dashmap` 6.2.1 — version exists, not yanked, MIT, by xacrimon, 8.5M+ downloads. `[VERIFIED: WebFetch crates.io/api/v1/crates/dashmap/6.2.1]`
- `gsd-tools query package-legitimacy check --ecosystem crates dashmap` — verdict `OK` (5.18M weekly downloads, repo github.com/xacrimon/dashmap, not deprecated, published 2019-08-25). `[VERIFIED: gsd-tools]`

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — DashMap 6.2.1 verified via ctx7 + crates.io + gsd-tools legitimacy; all other deps already in `api/Cargo.toml` (verified by reading it).
- Architecture: HIGH — patterns grounded in the actual codebase (`execute_single`, `web::block`, `start_file_watcher`, `ExecutionCounters.registry`, `ReteEngine::reset`) + DashMap/tokio official docs via ctx7.
- Pitfalls: HIGH — derived from reading the actual codebase + `PITFALLS.md` Pitfall 5 + DashMap sharding semantics (ctx7).
- Validation: HIGH — extends the established `api/tests/execute_integration.rs` `#[serial]` + `clear()` pattern (verified by reading the test file).

**Research date:** 2026-07-04
**Valid until:** 2026-08-04 (30 days — stable; DashMap 6.2.1 is pinned and the codebase integration points are stable post-Phase-26)