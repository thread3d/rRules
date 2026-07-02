# Stack Research

**Domain:** Wiring real rule execution into an existing Rust actix-web BRMS (v5.0 Live Execution)
**Researched:** 2026-07-02
**Confidence:** HIGH

> Scope: NEW stack additions/changes only. Existing validated capabilities
> (actix-web 4.4, tokio "full", the Rete/TREAT/LEAPS/PHREAK engines,
> `EngineSelector`/`EngineRegistry`, `CoverageTracking`/`RuleExecutionTracker`,
> the `prometheus` 0.13 workspace crate, hybrid Git+PostgreSQL storage) are
> **not** re-researched — they are reused. This document covers only what must
> be **added or changed** to make `POST /v1/execute` actually fire rules, back
> stateful sessions, expose per-request engine selection, and feed live data to
> the existing monitoring surfaces.

## Recommended Stack (NEW additions only)

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `dashmap` | 6.2.1 | Concurrent session store for stateful execution sessions keyed by `session_id` | actix-web runs one worker thread per CPU core; the existing `Arc<std::sync::RwLock<HashMap>>` pattern (used for `job_store`, `user_store`) serializes **all** session lookups behind a single write lock on every `/v1/execute` and `/v1/execute/session` call. DashMap is a sharded concurrent `HashMap` whose methods take `&self`, so it drops cleanly into `Arc<DashMap>` and scales to the hot per-request lookup path. It is the de-facto Rust concurrent map (~292M downloads, used by tokio-axum ecosystem projects). MSRV 1.65 fits the existing toolchain. |
| `prometheus` (workspace) | 0.13 (workspace) | Register API-layer `IntCounter`/`Gauge`/`Histogram` for live `/metrics` output | Already a workspace dep (`Cargo.toml:[workspace.dependencies] prometheus = { version = "0.13", features = ["process"] }`) and already optional in `engine/Cargo.toml`. **Change:** add `prometheus = { workspace = true }` to `api/Cargo.toml` so the API layer can register request-scoped metrics (`execute_requests_total`, `execute_rules_fired_total`, `execute_duration_seconds`, `active_sessions_gauge`). The existing `api/src/metrics.rs` `/metrics` handler currently **hardcodes zeros** — it must be extended to read these registered metrics. This is **extension, not replacement**: the engine-side `EnginePrometheusMetrics` (per-engine gauges) stays as-is. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tokio` (existing, "full") | 1.35 (workspace 1.x) | `task::spawn_blocking` for CPU-bound engine `execute()`; `time::sleep` for session TTL sweep | The `Box<dyn ExecutionEngine>::execute(&mut self)` is **synchronous CPU work** (Rete network propagation, beta-memory joins). Calling it directly in an actix handler blocks the worker thread and starves other connections. Wrap each `engine.execute()` call in `tokio::task::spawn_blocking` so the actix worker stays free. No new crate — already in `api/Cargo.toml` with `features = ["full"]`. Use the TTL sweep pattern (`tokio::spawn` + `tokio::time::interval`) to evict idle stateful sessions. |
| `tracing` (existing) | 0.1 | Structured spans for per-request execution traces | Already a dep. Wrap `execute_single` in a `#[tracing::instrument]` span carrying `execution_id`, `engine`, `session_id`, `rules_fired`. This feeds the inference-trace UI in the frontend test-runner without a new logging crate. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo bench` (criterion 0.5, existing dev-dep) | Microbench the `EngineRegistry::create_engine_by_name` + `execute` path per algorithm | Existing `criterion = { workspace = true, features = ["async_tokio"] }` dev-dep covers this. Add a bench comparing Rete/TREAT/LEAPS/PHREAK execute latency on a fixed fixture to validate the per-request engine selector. No new tool. |

## Installation

```toml
# api/Cargo.toml — additions only (existing deps elided)
[dependencies]
# NEW: concurrent session store for stateful /v1/execute/session
dashmap = "6.2.1"
# CHANGE: promote workspace prometheus from engine-only to also-API
prometheus = { workspace = true }
# (tokio, tracing, actix-web, engine, serde, uuid, chrono already present — unchanged)
```

```bash
# From repo root — no new binary install needed
cargo add -p api dashmap@6.2.1
cargo add -p api prometheus --features process   # matches workspace pin
```

No new dev-dependencies. No frontend stack changes required by the backend
wiring (the frontend test-runner consumes the existing `/v1/execute` JSON via
the already-shipped Vite + TanStack Query layer — that is a FEATURES/UI concern,
not a stack addition).

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `dashmap` 6.2.1 for session store | `Arc<RwLock<HashMap>>` (existing pattern) | Only if session churn is negligible. It is NOT — every execute call hits the session map. The existing pattern is fine for `job_store` (low write rate, polled via `/v1/jobs`) but wrong for the execute hot path. |
| `dashmap` 6.2.1 | `arc-swap` + immutable map | If sessions were read-mostly and updated via full replacement. Stateful sessions mutate in place (`insert_fact`), so a sharded mutable map is the right shape. |
| Request-scoped engine construction (build fresh `Box<dyn ExecutionEngine>` per `/v1/execute` call via `EngineRegistry::create_engine_by_name`) | `deadpool` 0.13.0 managed pool of pre-built engines | **Only if rule compilation is expensive.** Today `ReteEngine::new()` is a handful of `HashMap::new()` calls and `load_rules` is a stub — construction is O(1). Pooling would add a `Manager` trait impl + lifecycle for zero measurable benefit. **Defer pooling until rule loading does real parsing work**; revisit if a future milestone makes `load_rules` parse `.drl`/`.rules` files into the network. |
| `deadpool` 0.13.0 | `r2d2` | N/A — `r2d2` is sync-threaded, not async; the execute path is already inside an async actix handler. Deadpool is the correct family if pooling is ever needed, but see above — it is not needed for v5.0. |
| `tokio::task::spawn_blocking` for engine execute | `actix_web::web::block!` (deprecated) or a dedicated worker pool | `spawn_blocking` is the tokio-native, zero-extra-dep answer and integrates with the existing `tokio = { features = ["full"] }`. A dedicated worker pool would only be justified if engine execution exceeded the blocking-thread-pool depth regularly; start with `spawn_blocking` and measure. |
| Extend `api/src/metrics.rs` to read registered `prometheus` metrics | Replace with `metrics` crate + `metrics-exporter-prometheus` | The `prometheus` 0.13 crate is **already** a workspace dep and already used by `engine/src/metrics/prometheus.rs`. Switching to the `metrics` facade would require rewriting the engine-side exporter too — pure churn. Extend, do not replace. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `deadpool` (managed engine pool) for v5.0 | `EngineRegistry::create_engine_by_name` is O(1) today; rule loading is a stub. A pool adds a `Manager` impl + recycle logic for no gain. Premature optimization. | Request-scoped construction: `let mut engine = registry.create_engine_by_name(&engine_name).ok_or(...)?;` inside `execute_single`. Revisit only when `load_rules` does real parsing. |
| A new session-state crate (e.g. `tower-sessions`, `actix-session`) | Those crates solve cookie/JWT-backed user *authentication* sessions. The "session" in this milestone is a **rule-engine working-memory session** (`engine::environment::Session`, `stateful: bool`), not a user session. They are different concerns; the auth session already exists (Phase 22 JWT). | A `SessionStore = Arc<DashMap<String, StatefulSession>>` plus a `tokio::time::interval` sweep task. The `Session` type already exists in `engine/src/environment.rs:152`. |
| A new abstraction over `EngineSelector`/`EngineRegistry` | The v3.0 selector + registry already expose `available_engines()`, `create_engine_by_name()`, and validation. Building a new "EnginePool" trait would duplicate `EngineFactory` (`engine/src/engine/registry.rs:50`). | Reuse: share an `Arc<EngineRegistry>` via `web::Data`, call `create_engine_by_name(req.engine.unwrap_or(default))` per request. Validate with the existing `EngineSelector::with_defaults()` if a separate validation step is wanted. |
| Replacing the global `CoverageTracking` singleton (`LazyLock<Mutex<...>>`) | It already works across threads and is already called from inside `rete/treat/leaps/phreak` `execute_rules()`. The gap is that `execute_rules()` is **never invoked** by the API, so the singleton stays empty. | Just call the engine. Once `execute_single` drives a real `Box<dyn ExecutionEngine>::execute()`, the engines' existing `CoverageTracking::record_rule_eval/fire` calls populate the singleton automatically — and `/v1/metrics/coverage` + dead-rule detection read live data with **zero tracker-side changes**. |
| `sqlx`/PostgreSQL-backed session persistence in v5.0 | Out of scope for the wiring milestone; sessions are in-memory working memory by definition (a stateful rule session is live process state, not durable data). | In-memory `DashMap` with TTL. Persistence of *rule definitions* already uses the hybrid Git+PG storage — unchanged. |
| A second async runtime | actix-web is tokio-based; `tokio = { features = ["full"] }` is already a dep. | `tokio::task::spawn_blocking` and `tokio::time::interval`. |

## Stack Patterns by Variant

**Stateless one-shot execution (`POST /v1/execute` with no `session_id`):**
- Build a fresh `Box<dyn ExecutionEngine>` per request via the shared `EngineRegistry::create_engine_by_name(engine_name)`.
- Insert facts (`engine.insert_fact(...)`), call `engine.execute()` inside `tokio::task::spawn_blocking`.
- Drop the engine at the end of the request. No pool, no session store entry.
- Because the engine is dropped, working memory does not leak across requests.
- The engine's internal `CoverageTracking` calls fire on the global singleton → `/v1/metrics/coverage` reflects the request immediately.

**Stateful session execution (`POST /v1/execute/session/{id}` or `POST /v1/execute` with `session_id`):**
- Look up `SessionStore: Arc<DashMap<String, StatefulSession>>` (NEW). On miss, create a `StatefulSession { engine: Box<dyn ExecutionEngine>, created_at, last_used }` via the registry and insert.
- On hit, mutate the existing engine in place (`insert_fact`, `execute`) under `spawn_blocking`.
- Update `last_used` on every access; a background `tokio::time::interval` sweep task evicts sessions idle > `SESSION_TTL` (env-driven, default 15m).
- `execute_with_session` in `api/src/v1/execution.rs:561` is currently a stub returning `rules_fired: 0` — replace its body with this lookup.

**Batch execution (`POST /v1/rules/batch-execute`):**
- The existing `JoinSet` + `Semaphore` concurrency layer (`api/src/v1/execution.rs:374`) is correct and stays. Each task calls the *rewired* `execute_single` (which now really fires rules). No new concurrency primitive — `tokio::sync::Semaphore` + `JoinSet` are already there.

**Per-request engine selection:**
- Add `pub engine: Option<String>` to `ExecuteRequest` (and `BatchExecuteRequest` propagates it per-item).
- Resolve with `registry.create_engine_by_name(req.engine.unwrap_or(&default_engine))`; `None` → HTTP 400 with the list of available engines (from `registry.available_engines()`).
- The default comes from env `RRULES_DEFAULT_ENGINE` (default `"rete"`), read once in `main.rs` and shared via `web::Data<String>` (or a small `EngineConfig` struct).
- **Do not** mutate a global `EngineSelector::current` on per-request switch — `EngineSelector` is process-global state for the *config* default, not a per-request switch. The registry is the per-request factory.

**Prometheus /metrics extension:**
- Register, once at startup in `main.rs`, a small set of `prometheus` metrics on the default registry: `IntCounter` `execute_requests_total{engine}`, `IntCounter` `execute_rules_fired_total{engine}`, `Histogram` `execute_duration_seconds{engine}` (exponential buckets), `IntGauge` `active_sessions`.
- Increment inside `execute_single` / `execute_with_session` after `spawn_blocking` returns.
- Rewrite the hardcoded-zero branches in `api/src/metrics.rs:metrics_endpoint` (the `application/json` and `text/plain` Prometheus branches) to `encode` the registered metrics via `prometheus::TextEncoder` (already used by `engine/src/metrics/prometheus.rs:45`). The HTML branch can read the same counters for the cards.

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `dashmap@6.2.1` | Rust 1.65+ (MSRV) | Project is on edition 2021 with actix-web 4.4 / tokio 1.35 — well above 1.65. Pin `6.2.1`, not the `7.0.0-rc2` pre-release (no semver guarantees). |
| `prometheus@0.13` (workspace) | `engine/Cargo.toml` already uses it; `api/Cargo.toml` adds it | Same version on both sides of the workspace — no version skew. `TextEncoder` API used in `engine/src/metrics/prometheus.rs` is identical in the API layer. |
| `tokio@1.35` ("full") | `dashmap` (runtime-agnostic), `prometheus` (no runtime dep) | `spawn_blocking` and `time::interval` come from the already-enabled "full" feature set. |
| `dashmap` + `actix-web 4.4` | `web::Data<Arc<DashMap<K,V>>>` | DashMap is `Send + Sync` (sharded locks) → fits `web::Data`'s `Clone + Send + Sync + 'static` bound. |

## Integration Points with Existing Code

| New thing | Existing integration point | Action |
|-----------|---------------------------|--------|
| `Arc<EngineRegistry>` shared state | `engine/src/engine/registry.rs:139` (`EngineRegistry`), `api/src/main.rs:97` (`App::new().app_data(...)`) | Build a registry in `main.rs`, register all 4 factories (extend `register_default_engines` at `registry.rs:158` which currently only registers Rete), wrap in `web::Data`. Inject into `execute_rules` / `execute_single` / `batch_execute_rules` handlers. |
| `SessionStore = Arc<DashMap<String, StatefulSession>>` | `engine/src/environment.rs:152` (`Session`), `api/src/v1/execution.rs:561` (`execute_with_session` stub) | New type in `api/src/v1/execution.rs`. Wire into `execute_with_session` and the `session_id` branch of `execute_single`. Add `app_data` in `main.rs`. |
| `engine: Option<String>` on `ExecuteRequest` | `api/src/v1/execution.rs:16` (`ExecuteRequest`) | Add field. Validate against `registry.available_engines()` inside `execute_single`. |
| `prometheus` metrics in API layer | `api/src/metrics.rs:168` (`metrics_endpoint` hardcoded zeros), `engine/src/metrics/prometheus.rs:43` (`export_prometheus_text` reference impl) | Define metrics in a new `api/src/metrics_registry.rs` (or extend `api/src/metrics.rs`), register at startup, encode in `metrics_endpoint`. The engine-side exporter stays unchanged. |
| `spawn_blocking` around `engine.execute()` | `api/src/v1/execution.rs:312` (`execute_single`) | Replace the `_engine = EngineBuilder::new().build()` stub with `let mut engine = registry.create_engine_by_name(...)?;` then `tokio::task::spawn_blocking(move || engine.execute()).await?`. |
| TTL sweep task | `api/src/main.rs:79` (existing `tokio::spawn(start_file_watcher)` pattern) | Add a second `tokio::spawn(session_sweep_loop(session_store.clone(), ttl))`. |

## Sources

- [crates.io: dashmap](https://crates.io/crates/dashmap) — v6.2.1 stable, MSRV 1.65, ~292M downloads (HIGH confidence)
- [docs.rs: deadpool](https://docs.rs/deadpool/latest/deadpool/index.html) — v0.13.0, evaluated and **rejected for v5.0** (premature; engine construction is O(1)) (HIGH confidence on the rejection rationale)
- In-repo: `api/src/v1/execution.rs`, `engine/src/engine/{selector,registry,rete/mod}.rs`, `engine/src/monitoring/coverage.rs`, `engine/src/metrics/prometheus.rs`, `api/src/metrics.rs`, `Cargo.toml` (workspace + per-crate) — direct reads (HIGH confidence)