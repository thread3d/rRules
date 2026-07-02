# Architecture Research: v5.0 Live Execution Integration

**Domain:** Live rule execution wired into an existing BRMS (Rust actix-web + React)
**Researched:** 2026-07-02
**Confidence:** HIGH (grounded in direct reading of the existing codebase; integration points cited to file:line)

## Scope and Method

This document answers ONE question: **how does live execution integrate with the
existing rRules architecture?** It does NOT redesign existing components. Every
recommendation is framed as "touch file X" vs "create file Y", traced against the
code as it stands today. The six sub-questions from the milestone brief are
answered in §1–§6, then consolidated into a build order in §7.

The findings come from reading the actual source, not external ecosystem surveys.
External library research was not needed — all integration points are internal
to this codebase.

## Critical Discovery: Two Engine Surfaces Exist

Before the integration plan, the single most important finding:

**There are TWO separate, unrelated "engine" surfaces in the codebase today, and
they are NOT connected:**

| Surface | Location | Used by `/v1/execute`? | Real or stub? |
|---------|----------|----------------------|---------------|
| `engine::environment::Engine` | `engine/src/environment.rs` | YES (`execute_single` builds it at execution.rs:316) | **STUB** — `load_rules`, `execute_forward`, `Session::execute` all return placeholder strings ("Not implemented", "Forward chaining not yet implemented") |
| `engine::engine::ExecutionEngine` trait + `EngineRegistry` | `engine/src/engine/{registry,rete,treat,leaps,phreak}.rs` | NO | **REAL** — `ReteEngine::execute_rules()` (rete/mod.rs:321) actually fires, calls `CoverageTracking::record_rule_eval/fire`, updates `SimpleMetricsCollector` |

`POST /v1/execute` (`api/src/v1/execution.rs:312 execute_single`) builds an
`environment::Engine` (the stub one), never drives it, and returns
`rules_fired: 0`. The pluggable v3.0 engines — `ReteEngine` etc. — are never
constructed by the API layer at all. The `EngineSelector` and `EngineRegistry`
from v3.0 are dead code from the API's perspective.

**The v5.0 integration's central act is: route `/v1/execute` through the
`EngineRegistry`/`ExecutionEngine` surface (the real one), not the
`environment::Engine` surface (the stub one).** Everything else follows from
that.

## Standard Architecture (As-Is + To-Be)

### System Overview — As-Is (v4.0)

```
┌──────────────────────────────────────────────────────────────────┐
│ Frontend (React 19)                                              │
│  rules.ts (CRUD) │ monitoring dashboards │ NO execute client     │
└──────────────┬──────────────────────────┬─────────────────────────┘
               │ /v1/rules, /v1/metrics/* │ /v1/execute (stub)
┌──────────────▼──────────────────────────▼─────────────────────────┐
│ actix-web /v1                                                    │
│  execution.rs ──► environment::Engine (STUB) ──► rules_fired: 0  │
│  metrics.rs / monitoring.rs ──► hardcoded zeros                   │
│  EngineState (Arc<RwLock<environment::Engine>>) — hot-reload only │
└──────────────┬───────────────────────────────────────────────────┘
               │ (never used)
┌──────────────▼───────────────────────────────────────────────────┐
│ engine crate                                                     │
│  engine::{ReteEngine, EngineRegistry, EngineSelector} — REAL     │
│  monitoring::{CoverageTracking (singleton), RuleExecutionTracker│
│               (struct, NOT singleton)} — never populated         │
└──────────────────────────────────────────────────────────────────┘
```

### System Overview — To-Be (v5.0)

```
┌──────────────────────────────────────────────────────────────────┐
│ Frontend (React 19)                                              │
│  api/execution.ts (NEW) ── TestRunner panel in rule IDE          │
│  monitoring dashboards unchanged (now show live data)            │
└──────────────┬───────────────────────────────────────────────────┘
               │ POST /v1/execute {facts, engine?, session_id?}
┌──────────────▼───────────────────────────────────────────────────┐
│ actix-web /v1                                                    │
│  execution.rs (MODIFIED) ──► EngineRegistry (shared web::Data)   │
│   ├─ resolve engine by `engine` param or config default         │
│   ├─ load rules from shared RuleSet / rule_files                │
│   ├─ insert facts ──► Box<dyn ExecutionEngine>::execute()       │
│   ├─ record into ExecutionCounters (NEW, process-wide atomics)  │
│   └─ broadcast EngineState via metrics_tx                        │
│  SessionManager (NEW, web::Data) ── stateful sessions           │
└──────────────┬───────────────────────────────────────────────────┘
               │
┌──────────────▼───────────────────────────────────────────────────┐
│ engine crate                                                     │
│  EngineRegistry ──► ReteEngine::execute_rules() (REAL)          │
│   ├─ CoverageTracking (singleton) — auto-populated ✓            │
│   └─ RuleExecutionTracker hooks (NEW in rete/mod.rs)            │
│  ExecutionCounters (NEW) — atomics read by /metrics             │
└──────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Status | Responsibility | File |
|-----------|--------|----------------|------|
| `EngineRegistry` (shared) | NEW wiring | Holds registered factories; creates `Box<dyn ExecutionEngine>` by name | `engine/src/engine/registry.rs` (exists, must be instantiated as app state) |
| `EngineSelector` | EXISTING (use as config source) | Resolves default engine name from config; validates per-request `engine` param | `engine/src/engine/selector.rs` |
| `execute_single` / `execute_rules` | MODIFIED | Resolve engine, load rules, insert facts, fire, return real result | `api/src/v1/execution.rs:312,355` |
| `SessionManager` | NEW | Holds stateful sessions (engine + working memory per session_id) with expiry | NEW file, e.g. `api/src/v1/sessions.rs` |
| `ExecutionCounters` | NEW | Process-wide `AtomicU64` counters (rules_fired, executions, etc.) for Prometheus | NEW module, e.g. `api/src/metrics.rs` extension or `engine/src/monitoring/counters.rs` |
| `CoverageTracking` | EXISTING (singleton) | Already auto-populated by `ReteEngine`; read by `/v1/metrics/coverage` | `engine/src/monitoring/coverage.rs` |
| `RuleExecutionTracker` | EXISTING but NOT wired | Dead-rule detection; needs singleton + hooks in ReteEngine | `engine/src/monitoring/detection.rs` |
| `metrics_tx` broadcast | EXISTING but no producer | Pushes `EngineState` to WS clients; needs execution to send | `api/src/metrics.rs:22` |
| Frontend `api/execution.ts` | NEW | `executeRule(req)`, `getJobStatus(id)` API client | NEW `frontend/src/api/execution.ts` |
| Frontend TestRunner panel | NEW | Run/test UI in rule IDE: facts input, run button, results | NEW component in `frontend/src/features/rules/` |

## §1 Request-Scoped Engine Construction vs Worker Pool

**Recommendation: request-scoped engine construction, no worker pool.**

This is already what `execute_single` does (`EngineBuilder::new().build()` at
execution.rs:316) — and it is the right call. Reasons grounded in the code:

1. **actix-web workers each own a cloned `web::Data`** (main.rs:114). `web::Data`
   uses `Arc` internally, so app state is shared via clone-on-write reference, not
   per-worker ownership. A `Box<dyn ExecutionEngine>` is `!Clone` (it holds
   `ReteNetwork`, `HashMap`s, `SimpleMetricsCollector`), so it cannot be put in
   `web::Data` and cloned per worker the way `EngineState` is. Each request MUST
   therefore construct its own engine instance from the registry.

2. **`EngineRegistry` IS `Send + Sync`-shareable** (it holds
   `HashMap<EngineType, Box<dyn EngineFactory>>` where `EngineFactory: Send + Sync`).
   Wrap the registry in `web::Data<EngineRegistry>` once at startup; each request
   calls `registry.create_engine_by_name(name)` to get a fresh owned
   `Box<dyn ExecutionEngine>`. This is the cheap, correct unit of sharing: the
   *factory* is shared, the *engine instance* is per-request.

3. **`execute_single` is already self-contained** (execution.rs:304-311 docstring:
   "each task builds its own engine + working memory"). `batch_execute_rules`
   (execution.rs:374) leans on this — it spawns a `JoinSet` with a semaphore and
   calls `execute_single` per task with no shared locks. Keep that contract: do
   NOT introduce a shared mutable engine into `execute_single`. The batch path
   continues to work unchanged once `execute_single` is real.

4. **No performance case for a pool.** Rete network construction is cheap relative
   to the HTTP request overhead the system already accepts. A pool would add
   eviction/invalidation complexity (when rules reload, every pooled engine is
   stale) and break the existing "no shared locks between requests" invariant
   that the batch endpoint documents. The hot-reload shared `EngineState`
   (execution.rs:75) is a SEPARATE concern — see §5.

**What to touch:**
- `api/src/main.rs:32` — alongside `engine_state`, build an
  `EngineRegistry` with `register_default_engines()` (gated by the
  `engine-rete` cargo feature) and wrap it in `web::Data`.
- `api/src/v1/execution.rs:312` — replace `EngineBuilder::new().build()` with
  `registry.create_engine_by_name(engine_name)` and drive the returned
  `Box<dyn ExecutionEngine>`.

**What NOT to touch:** the `EngineState` (`Arc<RwLock<environment::Engine>>`)
at execution.rs:75 stays as-is for hot-reload only. It is NOT the execution
engine. (See §5 for whether it should be repurposed or replaced.)

## §2 Engine Resolution via the v3.0 Selector/Registry

The v3.0 selector and registry already do the right things; they are just never
called from the API. Wire them as follows.

**Two distinct responsibilities, two distinct types — keep them separate:**
- `EngineSelector` (selector.rs) — owns "which engine is the default?" +
  "is this name valid?". It's a stateful little config object. Use it to resolve
  the default and validate.
- `EngineRegistry` (registry.rs) — owns "construct an engine instance by name".
  It's the factory. Use it to actually build.

**Resolution algorithm for a request:**

```
1. If ExecuteRequest.engine is present:
     name = req.engine
   else:
     name = selector.current_engine()   // "rete" by default; config-overridable
2. selector.select_engine(&name)?        // validates against available list
3. let engine = registry.create_engine_by_name(&name)
       .ok_or(EngineNotFound(name))??    // double-Option: None=unregistered, Err=factory failed
4. engine
```

**Changes required:**

| Change | File | Detail |
|--------|------|--------|
| Add `engine: Option<String>` to `ExecuteRequest` | `api/src/v1/execution.rs:16` | Backward-compatible; serde default = None |
| Build + share `EngineRegistry` at startup | `api/src/main.rs:32` | `let mut reg = EngineRegistry::new(); reg.register_default_engines(); web::Data::new(reg)` |
| Build + share `EngineSelector` at startup (or fold into registry) | `api/src/main.rs` | `EngineSelector::with_defaults()`. Config default via env var `RRULES_DEFAULT_ENGINE` calling `select_engine` at startup. |
| Inject both into `execute_rules` / `execute_single` | `api/src/v1/execution.rs:355` | Change signature to accept `web::Data<EngineRegistry>` and `web::Data<EngineSelector>`; resolve per algorithm above |
| Enable `engine-rete` feature for the `api` crate | `api/Cargo.toml` | Currently `engine = { path = "../engine" }` with no features; add `features = ["engine-rete"]` (and `engine-treat/leaps/phreak` when their factories exist). Without this, `register_default_engines` registers nothing. |

**Note on `register_default_engines` (registry.rs:158):** it only registers
`ReteEngineFactory` and only under `#[cfg(feature = "engine-rete")]`. The
`treat`/`leaps`/`phreak` modules are gated by their own cargo features
(engine/mod.rs:31-39) but `register_default_engines` does NOT register them even
when those features are on. **Phase-specific research flag:** if v5.0 wants to
expose all four engines via the `engine` param, `register_default_engines` must
be extended to register `Treat`/`Leaps`/`Phreak` factories under their
respective `cfg` blocks. Verify those factories exist and impl `EngineFactory`
before promising >1 engine in the API. From what I read, only `ReteEngineFactory`
is wired (rete/mod.rs:392).

## §3 Trackers, Metrics, and the Monitoring Surfaces

This is where the biggest gap between "exists" and "is wired" lives.

### 3.1 CoverageTracking — already auto-populated (HIGH confidence)

`CoverageTracking` is a process-wide singleton
(`LazyLock<Mutex<TrackingState>>`, coverage.rs:86). `ReteEngine` already calls:
- `CoverageTracking::record_rule_eval(...)` (rete/mod.rs:293)
- `CoverageTracking::record_rule_fire(...)` (rete/mod.rs:332)

So the moment `execute_single` uses a `ReteEngine`, coverage data flows
automatically. `/v1/metrics/coverage` (api/src/metrics.rs:231
`coverage_endpoint`) reads `CoverageTracking.to_json()` directly — already
correct, already wired. **No change needed here** beyond making execute_real.

The frontend `RuleCoverageDashboard` (frontend/src/features/monitoring/dashboards/RuleCoverageDashboard.tsx)
already has a contract adapter (`adaptCoverage`) that maps the backend
`{rules[], summary}` shape to `RuleCoverageData[]`. It will display live data
the moment the backend produces any.

### 3.2 RuleExecutionTracker — NOT a singleton, NOT hooked (HIGH confidence)

Two independent problems:

**Problem A — not a singleton.** `RuleExecutionTracker` (detection.rs:64) is a
plain struct with `&mut self` methods. The `/v1/metrics/dead-rules` handler
(api/src/metrics.rs:212 `dead_rules_endpoint`) does
`let tracker = RuleExecutionTracker::new();` — a fresh empty instance every
request. It will ALWAYS return `[]`. This is a bug.

**Problem B — never called.** `ReteEngine` calls `CoverageTracking` but never
calls `RuleExecutionTracker::record_fire/record_evaluation`. So even if it were
a singleton, it would stay empty.

**Two fix options:**

| Option | What | Pros | Cons |
|--------|------|------|------|
| (a) Make `RuleExecutionTracker` a `LazyLock` singleton like `CoverageTracking`, and add `record_*` calls next to the existing `CoverageTracking::record_*` calls in rete/mod.rs:293,332 | Mirrors the CoverageTracking pattern; one source of truth per tracker | Two trackers to keep in sync; some duplication of (rule_id, fire_count) data | |
| (b) Derive dead rules FROM CoverageTracking data (a rule is "dead" iff `fired == false` in coverage) | Single source of truth; no new singleton | `RuleExecutionTracker` carries extra fields (`last_fired_at`, `category`, recommendations) that CoverageTracking doesn't; would need enrichment | |

**Recommendation: (a) for v5.0.** It's the smaller change, mirrors an existing
pattern, and preserves `RuleExecutionTracker`'s richer report generation
(`DeadRuleReport::generate`, recommendations). Convert `RuleExecutionTracker`
to the same `LazyLock<Mutex<...>>` singleton shape as `CoverageTracking`, expose
`record_evaluation`/`record_fire` as associated functions, and add the calls in
`rete/mod.rs` alongside the existing `CoverageTracking` calls. Then change
`dead_rules_endpoint` to read from the singleton instead of `::new()`.

### 3.3 ExecutionCounters for Prometheus — NEW (HIGH confidence)

The `/metrics` Prometheus endpoint (api/src/metrics.rs:188-200) currently emits
hardcoded zeros (`engine_rules_fired 0`). `EngineStateResponse` at
`/v1/monitoring/engine-state` (routes/monitoring.rs:30) is also hardcoded zeros.
The `metrics_tx` broadcast channel (api/src/metrics.rs:22) exists but nothing
sends to it; the WebSocket actor (websocket.rs, configured at main.rs:215)
broadcasts nothing.

To make these live, introduce a small process-wide counter struct. Put it in
`engine/src/monitoring/counters.rs` (mirrors the existing `monitoring/` module
layout) OR extend `api/src/metrics.rs`. Either way:

```rust
pub struct ExecutionCounters {
    pub rules_fired: AtomicU64,
    pub executions: AtomicU64,
    pub facts_inserted: AtomicU64,
    // ...
}
```

Wrap a single instance in `web::Data<ExecutionCounters>` at startup. After each
`execute_single` run, increment the atomics from the `EngineResult` +
`EngineMetrics`. Then:
- `metrics_endpoint` (Prometheus) reads atomics and emits real values.
- `get_engine_state` (monitoring.rs:30) reads atomics instead of literal 0.
- After incrementing, broadcast an `EngineState` snapshot via `metrics_tx` so
  the WS path (websocket.rs) streams live updates. The frontend
  `useEngineMetrics.ts` already subscribes to this WS and already has an
  `adaptEngineStateResponse` for the HTTP fallback — both wired, just no producer.

### 3.4 Per-engine metrics (engine-specific) — optional for v5.0

`EngineMetrics` (engine/src/engine/metrics.rs) is per-engine-instance and reset
on `ReteEngine::reset()`. It's useful for the algorithm-comparison dashboard
(v3.0 METRIC-01/02) but NOT for the live `/metrics` Prometheus output (which is
cumulative across all requests). Don't try to make `EngineMetrics` cumulative —
that's what `ExecutionCounters` is for. `EngineMetrics` from a single execution
can optionally be included in `ExecuteResponse` (e.g., an `engine_metrics` field)
for the test-runner UI to show "this run fired N rules in M ms". That's a
different concern from Prometheus.

### 3.5 Shared-state summary

| Reader (existing) | Data source (to-be) | Shared via |
|-------------------|---------------------|------------|
| `/v1/metrics/coverage` → `RuleCoverageDashboard` | `CoverageTracking` singleton | `LazyLock<Mutex>` (already) |
| `/v1/metrics/dead-rules` → `DeadRuleReportDashboard` | `RuleExecutionTracker` singleton (NEW) | `LazyLock<Mutex>` (new) |
| `/metrics` (Prometheus) | `ExecutionCounters` (NEW) | `web::Data<ExecutionCounters>` (Arc) |
| `/v1/monitoring/engine-state` + WS → `EngineMetricsDashboard` | `ExecutionCounters` + `metrics_tx` broadcast | `web::Data` + broadcast channel (already) |

The frontend dashboards need NO protocol changes — their existing contract
adapters (`adaptCoverage`, `adaptDeadRules`, `adaptEngineStateResponse`) already
handle the shapes the backends emit. They only need the backends to emit
non-zero data.

## §4 Stateful Session Lifecycle

The `environment::Session` struct (environment.rs:152) is a stub: it holds a raw
`*const Engine` pointer, `insert_fact` does nothing, `execute` returns
`ExecutionResult::new(true, "Not implemented")`. The `execute_with_session`
endpoint (execution.rs:561) is also a stub returning `rules_fired: 0`.

**Recommendation: build a real `SessionManager` as a new component; do NOT
salvage the `environment::Session` stub.** The stub uses a raw pointer to the
`environment::Engine` (the stub engine surface) — keeping it would inherit both
stubs. The real execution surface is `Box<dyn ExecutionEngine>`, which is owned
and `Send` but NOT `Clone`. A session must therefore OWN its engine instance.

### SessionManager design

```rust
// NEW file: api/src/v1/sessions.rs
pub struct Session {
    engine: Box<dyn ExecutionEngine>,   // owned, real
    session_id: String,
    created_at: Instant,
    last_active: Instant,
    stateful: bool,
}

pub struct SessionManager {
    sessions: Mutex<HashMap<String, Session>>,  // or DashMap for concurrency
    ttl: Duration,                               // configurable, default e.g. 30 min
}
```

Lifecycle:
- **Creation:** `POST /v1/execute` with `session_id: Some(id)` and no existing
  session → `SessionManager::create(id)` → builds engine via registry, inserts
  initial facts, executes, stores session. Subsequent requests with the same
  `session_id` → `SessionManager::get(id)` → insert more facts, execute again
  (fact accumulation across calls).
- **Stateless (default):** `session_id: None` → create a one-shot `Session`,
  execute, drop. No entry stored. This is the v4.0 behavior made real.
- **Reset:** `POST /v1/execute/session/{id}/reset` (new route) →
  `engine.reset()` (registry.rs:84 trait method, already impl'd by ReteEngine at
  rete/mod.rs:381) clears working memory + metrics but keeps the session alive.
- **Expiry:** A background tokio task (spawned at startup like the existing
  file watcher at main.rs:79) sweeps `SessionManager` every N seconds, dropping
  sessions whose `last_active + ttl < now`. Alternatively, lazy expiry on access.
- **Explicit destroy:** `DELETE /v1/execute/session/{id}` → remove from map.

### Where session state lives

In-memory only (`Mutex<HashMap<...>>` wrapped in `web::Data`). Sessions hold
`Box<dyn ExecutionEngine>` which is not serializable and not portable across
processes. This matches the existing in-memory store pattern used for rules,
approvals, audit, etc. (main.rs:38-45) — all `web::Data<Mutex<...>>`. For
multi-replica prod this means sticky sessions or per-replica state; that is an
acceptable v5.0 limitation and should be documented as a known constraint
(consistent with v4.0's in-memory store choices). Do NOT push sessions to
PostgreSQL — `ExecutionEngine` is not serializable.

### What to touch vs create

| Action | File |
|--------|------|
| NEW | `api/src/v1/sessions.rs` — `Session`, `SessionManager`, handlers |
| MODIFIED | `api/src/main.rs` — instantiate `SessionManager`, wrap in `web::Data`, spawn expiry task |
| MODIFIED | `api/src/v1/execution.rs:561` — replace `execute_with_session` stub; route through SessionManager |
| MODIFIED | `api/src/v1/execution.rs:312` — `execute_single` checks `session_id`: Some→upsert session, None→one-shot |
| NEW routes | `main.rs` /v1 scope — `DELETE /v1/execute/session/{id}`, `POST /v1/execute/session/{id}/reset` |
| LEAVE | `engine/src/environment.rs` `Session` struct — abandon it; do not break callers but do not extend |

## §5 Async Reload-then-Execute Data Flow

The hot-reload path (execution.rs:131 `reload_engine`, execution.rs:444
`reload_rules`) has three problems for v5.0:

1. **It targets the stub engine.** `EngineState::engine` is
   `Arc<RwLock<environment::Engine>>` (execution.rs:76) — the stub surface.
   Even if reload rebuilt it, `execute_single` doesn't use it (it builds its
   own `environment::Engine` at line 316). So reload has zero effect on
   execution today.

2. **It doesn't actually reload rules.** `reload_engine` reads files into
   `rule_files` (execution.rs:141-161), then acquires and immediately drops the
   engine write lock (lines 163-166) — it never calls `load_rules` or rebuilds
   the network. The `environment::Engine::load_rules` is itself a stub
   (environment.rs:122) that does nothing.

3. **The job store is never populated.** `reload_rules` async branch
   (execution.rs:450-461) spawns a task that calls `reload_engine` and discards
   the result. It generates a `job_id` but never writes a `ReloadJob` into the
   `JobStore` (execution.rs:239). So `GET /v1/jobs/{job_id}` always 404s for
   async reloads.

### To-be reload-then-execute flow

For v5.0, "execute against refreshed rules" requires reload to actually update
the rule set that `execute_single` reads. Two viable designs:

**Design A — shared compiled RuleSet (recommended).** Introduce a shared
`Arc<RwLock<RuleSet>>` (or `ArcSwap<RuleSet>`) built once at startup from the
rules directory, updated on reload. `execute_single` reads the current RuleSet
(shares it), clones it into the per-request engine, then fires. Reload rebuilds
the shared RuleSet in place. This decouples "which rules" (shared, reloadable)
from "the engine instance" (per-request, not shared).

```
startup: load rules dir → build RuleSet → ArcSwap<RuleSet> (web::Data)
reload (sync): rebuild RuleSet from dir → ArcSwap.store(new)
reload (async): spawn task → on completion store ReloadJob{Completed} in JobStore
execute_single: let rs = ruleset_snapshot.load(); engine.load_ruleset(rs.clone()); engine.execute()
```

**Design B — rules-as-text snapshot.** `EngineState::rule_files`
(`Arc<RwLock<HashMap<String, String>>>`) already exists and IS updated by
`reload_from_directory` (execution.rs:121-125). `execute_single` could read
`rule_files`, parse each into `Rule`s, and `RuleSet::add_rule` them. Simpler
but pushes parsing into the hot path per request. Design A is preferred because
parsing happens once per reload, not once per request.

**Either way, the fixes:**
- Make `reload_engine` actually rebuild the shared RuleSet (Design A) and update
  the shared snapshot. The `environment::Engine` write lock is irrelevant —
  drop it; `EngineState::engine` becomes unused for execution (it can stay as a
  v4.0 compat artifact or be removed).
- In the async branch of `reload_rules`, write a `ReloadJob` into `JobStore`
  with `Pending`→`Running`→`Completed`/`Failed` transitions so `GET /v1/jobs/{id}`
  works. The `JobStore` type and `ReloadJob` struct already exist
  (execution.rs:191,239) — just populate them.
- `execute_single` reads the current shared RuleSet snapshot at the start of
  each request, so a reload that completes before the request lands is
  automatically reflected. No explicit "reload-then-execute" ordering needed at
  the API level; the client polls `GET /v1/jobs/{id}` until `Completed`, then
  calls `POST /v1/execute`.

**Note on `EngineState::engine` field:** Under Design A this field is no longer
the execution engine. Either remove it (breaking the `EngineState::new()`
constructor at execution.rs:87) or repurpose `EngineState` to hold the shared
`RuleSet` snapshot instead. Renaming `EngineState` → `RuleSetState` would be
cleaner but touches more callers; leaving the name and changing the field is
lower-risk. Document the repurpose in the docstring.

## §6 Frontend Test-Runner → API → Engine → Results Data Flow

### Current frontend state

- `frontend/src/api/client.ts` — axios instance, `baseURL = VITE_API_URL || '/v1'`, JWT interceptor. Ready to use for execute calls.
- `frontend/src/api/rules.ts` — CRUD only (`getRules`, `getRule`, `createRule`, `updateRule`, `deleteRule`, `changeRuleStatus`). NO execute/test function.
- `frontend/src/api/types.ts` — NO `ExecuteRequest`/`ExecuteResponse` types.
- `frontend/src/features/rules/` — rule list/IDE types; no test-runner component.
- `frontend/src/hooks/useEngineMetrics.ts` — already subscribes to
  `/v1/metrics/ws` and polls `/v1/monitoring/engine-state`; already has
  `adaptEngineStateResponse`. Ready to consume live data with no protocol change.
- `frontend/src/features/monitoring/dashboards/*` — all three dashboards
  (Engine, Coverage, DeadRule) already have contract adapters for the backend
  shapes. They display empty today only because the backend emits zeros.

### To-be data flow

```
[Rule IDE] user edits rule, enters facts, clicks "Run"
   │
   ▼
[frontend/src/api/execution.ts]  (NEW)
   executeRule({ facts, engine?, session_id? })  ── POST /v1/execute ──┐
   getJobStatus(id)                                                  │
   ▼                                                                 │
[actix-web /v1/execute]                                              │
   execute_rules(req, registry, selector, counters, metrics_tx)      │
   │                                                                 │
   ├─ resolve engine name (§2)                                        │
   ├─ load current RuleSet snapshot (§5)                             │
   ├─ build Box<dyn ExecutionEngine> via registry                     │
   ├─ engine.load_ruleset(rs); insert facts; engine.execute()         │
   │    └─ ReteEngine::execute_rules()                                │
   │         ├─ CoverageTracking::record_rule_eval/fire  (singleton)   │
   │         └─ RuleExecutionTracker::record_eval/fire  (NEW singleton)│
   ├─ ExecutionCounters.fetch_add(result.rules_fired)  (NEW atomics)   │
   ├─ metrics_tx.send(EngineState snapshot)  → WS clients              │
   └─ return ExecuteResponse { execution_id, rules_fired,              │
        rules_not_fired, facts_after, inference_trace,                 │
        engine_metrics? }   ──────────────────────────────────────────┘
   │
   ▼
[frontend TestRunner panel]  (NEW component in features/rules/)
   displays: rules_fired, rules_not_fired[], facts_after,
             inference_trace, engine_metrics, execution_id
   "Run" button → executeRule(); "Test" button → executeRule() with
                  a fixture; results render inline below the editor

Meanwhile, in parallel:
   /v1/metrics/coverage  → RuleCoverageDashboard   (live, no UI change)
   /v1/metrics/dead-rules → DeadRuleReportDashboard (live, no UI change)
   /v1/monitoring/engine-state + WS → EngineMetricsDashboard (live, no UI change)
```

### Frontend changes

| Action | File |
|--------|------|
| NEW | `frontend/src/api/execution.ts` — `executeRule`, `batchExecute`, `getJobStatus`, `listJobs`; types `ExecuteRequest`, `ExecuteResponse`, `BatchExecuteRequest/Response`, `ReloadJob` |
| NEW types | `frontend/src/api/types.ts` — add `ExecuteRequest`, `ExecuteResponse`, `InferenceTrace`, `BatchExecute*` (mirror backend `#[derive(Serialize)]` shapes, snake_case as existing adapters expect) |
| NEW component | `frontend/src/features/rules/TestRunner.tsx` (or `RuleTestPanel.tsx`) — facts editor (key/value rows), engine selector dropdown (populated from a new `GET /v1/engines` or hardcoded `["rete"]` initially), Run/Test buttons, results panel |
| MODIFIED | The rule IDE route — embed `<TestRunner />` alongside the CodeMirror editor (the editor itself is unchanged) |
| OPTIONAL | `frontend/src/api/execution.ts` — `reloadRules({async:true})` + poll `getJobStatus` to drive "reload then test" UX |

The monitoring dashboards need NO frontend changes — only the backend data
sources change (§3). Their existing adapters already tolerate the real shapes.

## §7 Suggested Build Order (Respecting Existing-Component Dependencies)

The principle: **connect existing components before building new ones, and
make the cheapest end-to-end path real first.** Every phase below ends with a
verifiable vertical slice.

### Phase A — Wire execute_single through the real engine (vertical slice)
**Goal:** `POST /v1/execute` returns non-zero `rules_fired` for the first time.
- A.1 Add `engine: Option<String>` to `ExecuteRequest` (execution.rs:16).
- A.2 Build `EngineRegistry` + `EngineSelector` at startup; wrap in `web::Data`
  (main.rs). Enable `engine-rete` feature on the `api` crate (api/Cargo.toml).
- A.3 Modify `execute_single` to resolve the engine via registry, build a
  `ReteEngine`, insert facts from `req.facts`, call `engine.execute()`, return
  real `rules_fired`. Leave RuleSet loading for Phase C (start with empty rule
  set — fires zero rules but proves the wiring).
- A.4 Verify: `curl -X POST /v1/execute -d '{"facts":{...}}'` returns
  `rules_fired >= 0` from a real engine, not a hardcoded 0.

**Respects:** Engine, EngineRegistry, ReteEngine, CoverageTracking all exist
unchanged. Only API wiring changes.

### Phase B — Make monitoring surfaces read live data
**Goal:** `/metrics`, `/v1/monitoring/engine-state`, and the dashboards show
non-zero counts after running executions.
- B.1 Add `ExecutionCounters` (atomics) as `web::Data` (api/src/metrics.rs or
  new engine/src/monitoring/counters.rs). Increment in `execute_single`.
- B.2 Update `metrics_endpoint` (Prometheus) and `get_engine_state` to read
  counters. Broadcast `EngineState` via `metrics_tx` after each execution.
- B.3 Verify: run executions, then `GET /metrics` shows `engine_rules_fired N`;
  the Engine dashboard (WS) updates live.

**Respects:** `metrics_tx`, WS actor, frontend `useEngineMetrics.ts` + adapters
all exist unchanged. CoverageTracking already populated by Phase A (no work).

### Phase C — Rule loading + shared RuleSet + reload-then-execute
**Goal:** Executions fire real rules loaded from disk; reload actually refreshes
them; async reload jobs are queryable.
- C.1 Introduce shared `ArcSwap<RuleSet>` (or `Arc<RwLock<RuleSet>>`) built at
  startup from `RULE_STORAGE_PATH`. `execute_single` loads it into each request's
  engine before executing.
- C.2 Make `reload_engine` rebuild the shared RuleSet from the rules dir and
  store it. Drop the no-op `environment::Engine` write lock.
- C.3 Populate `JobStore` in the async reload branch with
  Pending→Running→Completed/Failed transitions.
- C.4 Verify: add a rule file, `POST /v1/rules/reload {async:true}`, poll
  `GET /v1/jobs/{id}`, then `POST /v1/execute` fires the new rule.

**Respects:** `EngineState::rule_files`, `JobStore`, `ReloadJob` all exist;
they get real content. `execute_single` gains a RuleSet argument but its
signature change is internal.

### Phase D — RuleExecutionTracker singleton + dead-rule live data
**Goal:** `/v1/metrics/dead-rules` returns real dead rules.
- D.1 Convert `RuleExecutionTracker` to a `LazyLock<Mutex<...>>` singleton with
  associated `record_evaluation`/`record_fire` functions (mirror
  `CoverageTracking`).
- D.2 Add `RuleExecutionTracker::record_*` calls in `rete/mod.rs` next to the
  existing `CoverageTracking::record_*` calls (lines 293, 332).
- D.3 Update `dead_rules_endpoint` (api/src/metrics.rs:212) to read from the
  singleton (use `DeadRuleReport::generate` for the richer payload — optional
  but the frontend `adaptDeadRules` already maps `Vec<String>`; consider
  extending the endpoint to return `DeadRuleInfo[]` and updating the adapter).
- D.4 Verify: run executions, then `GET /v1/metrics/dead-rules` lists rules
  that never fired.

**Respects:** `RuleExecutionTracker` API, `DeadRuleReport`, frontend adapter
all exist. The `DeadRuleReportDashboard`'s `adaptDeadRules` currently expects
`Vec<String>`; if the endpoint is upgraded to `DeadRuleInfo[]`, update the
adapter in the same phase.

### Phase E — Stateful sessions
**Goal:** `session_id` works across multiple `POST /v1/execute` calls; sessions
expire; reset works.
- E.1 New `api/src/v1/sessions.rs` with `Session` + `SessionManager`
  (`Mutex<HashMap<String, Session>>`, TTL).
- E.2 `execute_single` upserts/looks-up session by `session_id`; stateless
  path unchanged.
- E.3 New routes `DELETE /v1/execute/session/{id}`,
  `POST /v1/execute/session/{id}/reset`; replace `execute_with_session` stub.
- E.4 Spawn expiry sweeper task at startup.
- E.5 Verify: two executes with same `session_id` accumulate facts; reset
  clears; TTL drops idle sessions.

**Respects:** `ExecutionEngine::reset` (registry.rs:84) already impl'd by
ReteEngine. Does NOT touch the abandoned `environment::Session` stub.

### Phase F — Frontend test-runner
**Goal:** User can run/test a rule from the IDE and see results.
- F.1 New `frontend/src/api/execution.ts` + types.
- F.2 New `TestRunner` component (facts editor, engine dropdown, Run/Test,
  results panel). Embed in the rule IDE route.
- F.3 Wire "Run" → `executeRule` → render `rules_fired`, `rules_not_fired`,
  `facts_after`, `inference_trace`.
- F.4 Optional: "Reload then Run" → `reloadRules({async:true})` + poll
  `getJobStatus` + then `executeRule`.
- F.5 Verify: in browser, edit rule, click Run, see fired rules + results;
  coverage/dead-rule dashboards update on next refresh.

**Respects:** All monitoring dashboards and the metrics hook already consume
the right endpoints — Phase F is pure new UI over already-working APIs.

### Phase ordering rationale

- A before B: B's counters are incremented by A's `execute_single`. Without A,
  B reads zeros forever.
- A before C: C adds RuleSet loading into the `execute_single` that A made
  real. Doing C first would load rules into a stub that never fires them.
- B independent of C/D: dashboards light up as soon as executions run, even
  with an empty rule set (they show "0 fired, 0 dead" — still live, not
  hardcoded).
- D after A: D's tracker calls are added next to A's CoverageTracking calls in
  the same rete/mod.rs lines.
- E after A: sessions wrap the per-request engine that A built.
- F last: it depends on A–D being real so the UI shows meaningful data. Could
  be done in parallel with E if the session_id path is deferred.

## Architectural Patterns

### Pattern 1: Factory shared, instance per-request
**What:** Share the `EngineRegistry` (factory) as `web::Data`; construct a fresh
`Box<dyn ExecutionEngine>` per request.
**When to use:** Engine instances are `!Clone` and carry mutable state
(working memory, metrics); factories are `Send + Sync`.
**Trade-offs:** Pro — no shared mutable state between requests, batch endpoint
contract preserved, no eviction logic. Con — repeated construction cost (cheap
for Rete network build).

### Pattern 2: Process-wide singleton via LazyLock<Mutex>
**What:** `CoverageTracking` and (to-be) `RuleExecutionTracker` are
`LazyLock<Mutex<State>>` with associated functions, accessed globally.
**When to use:** Cross-request, cross-handler accumulative state that has no
per-request owner.
**Trade-offs:** Pro — any handler reads live data without `web::Data` plumbing.
Con — global mutable state; harder to test in isolation (the existing tests
call `tracker.clear()` first — see coverage.rs tests). Acceptable because it
mirrors the established v4.0 pattern.

### Pattern 3: ArcSwap for hot-reloadable shared snapshot
**What:** Shared `RuleSet` behind `ArcSwap` (or `Arc<RwLock<RuleSet>>`); reload
stores a new snapshot; readers load() the current one without blocking writers.
**When to use:** Read-heavy, write-rarely shared immutable-ish data that must
update atomically.
**Trade-offs:** Pro — readers never block on reload; reload is atomic. Con —
new dependency (`arc-swap` crate) if choosing ArcSwap; `Arc<RwLock<>>` is
zero-dependency but readers contend with writers briefly. Either is fine;
prefer `Arc<RwLock<RuleSet>>` to avoid a new dep unless contention shows up.

## Anti-Patterns to Avoid

### Anti-Pattern 1: Sharing a mutable engine across requests
**What:** Putting a `Box<dyn ExecutionEngine>` in `web::Data` and locking it per
request.
**Why bad:** Serializes all executions on one lock; breaks the batch
endpoint's "no shared locks between requests" contract (execution.rs:304-311);
stale engine after reload unless you also lock-and-rebuild on every request.
**Instead:** Share the factory (`EngineRegistry`), construct per request.

### Anti-Pattern 2: Routing through the `environment::Engine` stub
**What:** Wiring `execute_single` to the `environment::Engine` at
execution.rs:316 and calling its stub `execute_forward()`.
**Why bad:** `execute_forward` returns
`ExecutionResult::new(true, "Forward chaining not yet implemented")` — it's a
placeholder (environment.rs:132-140). The whole v3.0 pluggable-engine work
(selector, registry, ReteEngine) is bypassed.
**Instead:** Route through `EngineRegistry` → `ExecutionEngine` trait.

### Anti-Pattern 3: Per-request rule parsing
**What:** Reading `rule_files` HashMap and parsing `.rules`/`.drl` text into
`Rule`s inside `execute_single` on every call.
**Why bad:** Pushes parsing cost onto the hot path; couples execution to file
format; reload would still have to update the same text map.
**Instead:** Build a compiled `RuleSet` once per reload; share the snapshot
(§5 Design A).

### Anti-Pattern 4: Persisting sessions to PostgreSQL
**What:** Serializing `Session` (containing `Box<dyn ExecutionEngine>`) to the
DB for multi-replica durability.
**Why bad:** `ExecutionEngine` is not serializable (network, HashMaps of fact
ids); the existing stores are all in-memory `web::Data<Mutex<...>>`; adding
DB persistence is a much larger change with different consistency guarantees.
**Instead:** In-memory `SessionManager` with TTL; document single-replica
session affinity as a v5.0 constraint (consistent with v4.0 store choices).

## Scalability Considerations

| Concern | At 100 req/s | At 10K req/s | At 1M req/s |
|---------|--------------|--------------|-------------|
| Engine construction per request | Cheap (Rete network build is µs-scale); no pool needed | Still fine; actix worker threads parallelize | If profiled as hot, introduce a small idle-engine pool keyed by (engine_name, ruleset_version) — but only then |
| `ExecutionCounters` atomics | `Relaxed`-ordering AtomicU64 — no contention | Same | Same; switch to per-shard counters only if profiles show contention |
| `CoverageTracking`/`RuleExecutionTracker` Mutex | Single global lock; held for one HashMap insert | May show contention; consider DashMap or sharded locks | Replace singletons with sharded trackers |
| `SessionManager` Mutex | Fine | Contention on one HashMap; use DashMap | DashMap + per-session locks |
| Shared RuleSet read | `Arc<RwLock<>>` read lock per request — cheap | Cheap | Switch to `ArcSwap` (lock-free reads) if readers contend |
| `metrics_tx` broadcast | 16-capacity channel (main.rs:35); fine for few WS clients | Same | Bump capacity or drop slow subscribers (broadcast already handles this) |

## Sources

- `api/src/v1/execution.rs` — execute_single stub (lines 312-343), execute_rules (355), batch_execute_rules (374), EngineState (75), reload_engine (131), reload_rules (444), execute_with_session (561)
- `api/src/main.rs` — app state wiring (32-124), routes (152-217), file watcher spawn (79)
- `engine/src/engine/registry.rs` — EngineRegistry, EngineType, ExecutionEngine trait, register_default_engines (158)
- `engine/src/engine/selector.rs` — EngineSelector, EngineConfig
- `engine/src/engine/rete/mod.rs` — ReteEngine::execute_rules (321), CoverageTracking calls (293, 332), ExecutionEngine impl (350), ReteEngineFactory (392)
- `engine/src/engine/metrics.rs` — EngineMetrics, SimpleMetricsCollector, MetricsExporter
- `engine/src/environment.rs` — EngineBuilder, Engine (stub execute_forward 132, load_rules 122), Session (stub 152)
- `engine/src/monitoring/coverage.rs` — CoverageTracking singleton (86)
- `engine/src/monitoring/detection.rs` — RuleExecutionTracker (64, NOT singleton), DeadRuleReport
- `api/src/metrics.rs` — metrics_endpoint Prometheus (188), dead_rules_endpoint (212), coverage_endpoint (231), metrics_tx broadcast (22)
- `api/src/v1/metrics.rs` — get_engine_metrics stub, reset_coverage stub
- `api/src/v1/routes/monitoring.rs` — get_engine_state hardcoded zeros (30)
- `api/src/v1/simulation.rs` — existing real execution surface (SimulationRunner) for reference pattern
- `engine/src/rules.rs` — RuleSet::add_rule (245), compile (285)
- `engine/Cargo.toml`, `api/Cargo.toml` — feature flags (engine-rete etc.)
- `frontend/src/api/client.ts` — axios baseURL + JWT interceptor
- `frontend/src/api/rules.ts` — CRUD only, no execute
- `frontend/src/hooks/useEngineMetrics.ts` — WS + HTTP adapter (already wired)
- `frontend/src/features/monitoring/dashboards/{EngineMetrics,RuleCoverage,DeadRuleReport}Dashboard.tsx` — existing adapters

## Confidence Assessment

| Area | Confidence | Reason |
|------|------------|--------|
| Two-engine-surface finding | HIGH | Read both environment.rs and rete/mod.rs directly; stubs confirmed at cited lines |
| Per-request engine construction recommendation | HIGH | Confirmed `Box<dyn ExecutionEngine>: !Clone`, actix `web::Data` Arc-clone semantics, existing batch contract docstring |
| Engine resolution via selector+registry | HIGH | Both types read in full; resolution algorithm is direct composition of their existing methods |
| CoverageTracking auto-population | HIGH | ReteEngine calls confirmed at rete/mod.rs:293,332; singleton confirmed at coverage.rs:86 |
| RuleExecutionTracker not-a-singleton bug | HIGH | `::new()` in dead_rules_endpoint confirmed at api/src/metrics.rs:214; struct definition at detection.rs:64 |
| Reload-engine no-op | HIGH | reload_engine body read; lock acquire+drop at execution.rs:163-166; load_rules stub at environment.rs:122 |
| JobStore never populated | HIGH | async reload branch at execution.rs:450-461 spawns task, discards result, never writes JobStore |
| Frontend adapters already wired | HIGH | Read all three dashboard .tsx files + useEngineMetrics.ts; adapters present |
| SessionManager as new component | MEDIUM | Pattern matches existing stores; specific TTL/sweep design is a recommendation, not a verified-existing pattern |
| ArcSwap vs RwLock for RuleSet | MEDIUM | Both viable; choice is a perf trade-off not yet measured |

## Gaps to Address in Phase-Specific Research

- **TREAT/LEAPS/PHREAK factory registration:** `register_default_engines`
  (registry.rs:158) only registers Rete. If v5.0 promises all four engines via
  the `engine` param, verify the other three `EngineFactory` impls exist and
  register them. Read `engine/src/engine/{treat,leaps,phreak}/mod.rs` for
  `impl EngineFactory` before committing to >1 engine in the API.
- **RuleSet → ExecutionEngine bridging:** The exact API for loading a
  `RuleSet` into a `ReteEngine` (registry.rs `ExecutionEngine` does not expose
  `load_ruleset`; rete/mod.rs builds its own network). Phase C must define how
  `RuleSet` rules become Rete network nodes. Read rete/mod.rs network
  construction in full.
- **`inference_trace` population:** `ExecuteResponse.inference_trace` is
  `Option<InferenceTrace>` and currently always None. `ReteEngine::execute_rules`
  does not emit a trace. Phase F's "see fired rules" relies on either the trace
  or on `rules_not_fired`/`CoverageTracking` data. Decide whether to populate
  the trace from ReteEngine or derive the display from coverage data.
- **Session TTL default and sweep interval:** Pick values; not architecturally
  load-bearing but needs a config knob.