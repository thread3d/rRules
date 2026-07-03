# Phase 24: Execution counters + metrics broadcast - Research

**Researched:** 2026-07-03
**Domain:** Backend metrics instrumentation (Rust / actix-web / prometheus 0.13 / tokio broadcast)
**Confidence:** HIGH (every current-state claim verified by direct read of the live codebase with file:line; no external library docs consulted — this is a brownfield internal-instrumentation phase and the `prometheus` crate API is already exercised in-tree at `engine/src/engine/metrics.rs`)

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| MON-07 | Per-execution Prometheus metrics + WebSocket `EngineState` snapshot make the v4.0 monitoring dashboards display live activity with zero protocol/UI changes. (Full requirement text lives in the ROADMAP Phase 24 entry; REQUIREMENTS.md carries only the traceability row at line 17.) | `ExecutionCounters` (AtomicU64) wired as `web::Data`; `prometheus` 0.13 promoted into `api/Cargo.toml`; `metrics_endpoint` `/metrics` extended via `TextEncoder`; `MetricsSnapshot` broadcast on the existing `metrics_tx` after each execute; `ExecuteResponse.engine_generation` field added. Frontend `useEngineMetrics.ts` already consumes the exact `MetricsSnapshot` field shape — confirmed zero frontend changes. |
</phase_requirements>

## Summary

Phase 24 instruments the live `execute_single` path that Phase 23 rewrote (`api/src/v1/execution.rs:327`) with three pieces of plumbing so the v4.0 monitoring surfaces display real activity instead of hardcoded zeros: (1) an `ExecutionCounters` struct of `AtomicU64`/`prometheus` metrics held as `web::Data`, incremented on every `POST /v1/execute`; (2) extension of the `/metrics` Prometheus exposition and the `get_engine_state` snapshot to read those counters; (3) a broadcast of a populated `MetricsSnapshot` on the existing `metrics_tx` tokio broadcast channel after each execute, plus a new `engine_generation` field on `ExecuteResponse` to structurally close the Pitfall 8 stale-polling race.

The single most load-bearing finding: **the frontend already consumes the exact `MetricsSnapshot` field shape**. `frontend/src/hooks/useEngineMetrics.ts:4-11` defines `interface EngineState { engine_name, rules_fired, facts_inserted, active_sessions, memory_bytes, last_updated }` and the WS `onmessage` handler at `:135` does `if ('engine_name' in message)` then reads `message.rules_fired` / `message.facts_inserted` / `message.last_updated`. The backend `MetricsSnapshot` struct (`api/src/metrics.rs:16-23`) already carries exactly those six fields. So Phase 24's WS broadcast just needs to send a `MetricsSnapshot` populated from the live counters — the frontend renders live numbers with **zero code changes**. This is verified, not assumed.

**Primary recommendation:** Build `ExecutionCounters` as a `web::Data`-held struct of `prometheus::IntCounterVec` (engine label) + `prometheus::HistogramVec` (duration) + plain `AtomicU64` mirrors (for the WS snapshot, since reading a `prometheus::Counter` instance for the snapshot is awkward — see Open Questions). Promote `prometheus = { workspace = true }` into `api/Cargo.toml` (the workspace already pins `0.13` at `Cargo.toml:15`). Increment counters inside `execute_single` immediately after the `web::block` returns, then broadcast a `MetricsSnapshot` built from the counters on `metrics_tx`. Add `engine_generation: u64` (default `0` for Phase 24; Phase 25 increments it on reload) to `ExecuteResponse`. Do NOT touch the frontend.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Per-execution counter increment | API / Backend (actix handler) | — | `execute_single` is the single choke point every request passes through (`execution.rs:327`); incrementing here captures every fire without engine-crate coupling. |
| Prometheus exposition (`/metrics`) | API / Backend (`metrics_endpoint`) | — | The `/metrics` route is an API-layer concern (`api/src/metrics.rs:172`); the engine-side `export_prometheus_metrics` (`engine/src/engine/metrics.rs:284`) is currently broken (deferred-items.md #1) and out of scope. |
| WebSocket `MetricsSnapshot` broadcast | API / Backend (`execute_single` → `metrics_tx`) | — | The `metrics_tx` broadcast channel is created in `main.rs:123` and injected at `:207`; `execute_single` must receive it and call `metrics_tx.send(snapshot)`. |
| `engine_generation` on response | API / Backend (`ExecuteResponse`) | Engine (Phase 25) | Phase 24 adds the field shape (default `0`); Phase 25 wires the real generation counter on the shared `RuleSet`/engine. |
| Frontend dashboard rendering | Browser / Client (existing) | — | No frontend changes — `useEngineMetrics.ts` already tolerates the real `MetricsSnapshot` shape (verified at `:135-150`). |

## Phase Boundary

**IN scope (backend metrics + WS + response-shape extension only):**

- New `ExecutionCounters` struct (AtomicU64 + prometheus counters/histogram) wired as `web::Data` in `main.rs`; injected alongside the existing `registry`/`selector`/`ruleset`.
- Promote `prometheus` from a workspace-only dep into `api/Cargo.toml` (workspace already pins `0.13`).
- `metrics_endpoint` (`api/src/metrics.rs:172`) — extend the `text/plain` arm to encode the api-layer counters via `prometheus::TextEncoder` alongside the existing hardcoded engine lines; extend the JSON/HTML arms to read the live counters.
- `execute_single` (`api/src/v1/execution.rs:327`) — accept `web::Data<ExecutionCounters>` + `web::Data<MetricsTx>`, time the `web::block` call, increment counters after it returns, broadcast a populated `MetricsSnapshot`.
- `ExecuteResponse` (`execution.rs:42`) — add `engine_generation: u64` (default `0`). The `execution_id` field already exists at `execution.rs:44` — do NOT re-add it.
- `batch_execute_rules` (`execution.rs:514`) — clone the new `web::Data` handles into each `JoinSet` task so per-item execution also increments counters and broadcasts.
- Integration tests: extend `api/tests/execute_integration.rs` and `api/tests/metrics_integration.rs` / `metrics_ws.rs` to assert the three ROADMAP success criteria.

**OUT of scope (deferred to later phases):**

- Frontend code changes of any kind (Phase 28 owns the test-runner UI; the dashboards already render the real shape — verified).
- Real `engine_generation` incrementing on reload (Phase 25 — D-25 generation counter on the shared `Arc<RwLock<RuleSet>>`).
- RuleExecutionTracker singleton-ization / dead-rule live data (Phase 26 — MON-06).
- Fixing the pre-existing `cargo build -p engine --all-features` prometheus `Opts` drift at `engine/src/engine/metrics.rs:288-293` (logged in `23-deferred-items.md` #1; unrelated to the api-layer counters Phase 24 adds).
- Inference trace population (Phase 28 — EXEC-04).
- Histogram bucket tuning beyond a reasonable default (flagged in Open Questions for the planner).

## Current State (what Phase 23 left)

All file:line citations are against the live codebase on `master` (post-23-06 commit `c09de6eaa`).

### `execute_single` — the path Phase 24 instruments
`api/src/v1/execution.rs:327-471` is the rewritten Plan 23-05 function. Its signature (line 327-332):
```rust
pub async fn execute_single(
    req: ExecuteRequest,
    registry: web::Data<EngineRegistry>,
    selector: web::Data<EngineSelector>,
    ruleset: web::Data<Arc<std::sync::RwLock<RuleSet>>>,
) -> Result<ExecuteResponse, HttpResponse>
```
It does NOT currently receive `ExecutionCounters` or `MetricsTx` — Phase 24 adds both. The CPU-bound work is at line 411-424:
```rust
let (engine_result, post_facts) = match web::block(move || {
    let result = engine.execute();
    let facts = engine.get_facts();
    (result, facts)
}).await { ... };
```
No timing is taken today. Phase 24 wraps this with `std::time::Instant::now()` … `elapsed()` to feed the histogram. `engine_result.rules_fired` (line 466) and `engine_result.fired_rule_ids` (line 453) are the values the counters aggregate. `execution_id` is already generated at line 333 (`Uuid::new_v4()`).

### `ExecuteResponse` — already has `execution_id`
`api/src/v1/execution.rs:42-52`:
```rust
pub struct ExecuteResponse {
    pub execution_id: String,           // line 44 — ALREADY PRESENT
    pub rules_fired: usize,
    pub rules_not_fired: Vec<String>,
    pub facts_after: HashMap<String, serde_json::Value>,
    pub inference_trace: Option<InferenceTrace>,
}
```
**Phase 24 adds ONLY `engine_generation: u64`** (with `#[serde(default)]` so existing clients don't break). The `batch_execute_rules` zero-fired placeholder at line 546-552 must also gain the new field.

### `main.rs` — app state wiring
`api/src/main.rs:123` creates the metrics broadcast channel:
```rust
let (metrics_tx, _) = metrics::create_metrics_channel(16);
```
and injects it at `:207`:
```rust
.app_data(web::Data::new(metrics_tx.clone()))
```
The `metrics_tx` is `Arc<tokio::sync::broadcast::Sender<MetricsSnapshot>>` (`api/src/metrics.rs:26`). Phase 24 adds an `.app_data(counters.clone())` line alongside it. Note: the `_` receiver at `:123` is discarded — a broadcast with no receivers drops the message; Phase 24's `execute_single` calls `metrics_tx.send(...)` which is a no-op if no WS session is subscribed (acceptable — the WS actor subscribes at `websocket.rs:47`).

### `MetricsSnapshot` — the WS broadcast payload
`api/src/metrics.rs:16-23`:
```rust
pub struct MetricsSnapshot {
    pub engine_name: String,
    pub rules_fired: u64,
    pub facts_inserted: u64,
    pub active_sessions: usize,
    pub memory_bytes: u64,
    pub last_updated: String,
}
```
This is the struct that the WS actor (`api/src/websocket.rs:37-60`) serializes and pushes as WS text frames. Today nothing ever calls `metrics_tx.send(...)` — the channel exists but is silent. Phase 24 makes `execute_single` send a populated snapshot after each run.

### `metrics_endpoint` — `/metrics`
`api/src/metrics.rs:172-205` handles `GET /metrics`. The `text/plain` arm (line 192-203) returns a hardcoded string:
```rust
let metrics_text = r#"# HELP engine_rules_fired Total number of rules fired
# TYPE engine_rules_fired counter
engine_rules_fired 0
...
"#;
```
Phase 24 replaces this with a `prometheus::TextEncoder` encoding of the api-layer `IntCounterVec`/`HistogramVec`. The JSON arm (line 181-191) and HTML arm (line 176-180) likewise read hardcoded `"default"`/`0` — Phase 24 reads the live counters. The existing test `api/tests/metrics_integration.rs:43` (`metrics_prometheus_text_format`) only asserts the response `contains("# HELP")` and `contains("engine_rules_fired")` — Phase 24 must keep those substrings present (the engine-side hardcoded line can stay as a fallback or be removed; see Open Questions).

### `prometheus` dependency — workspace-level, NOT in api
`Cargo.toml:15` (workspace):
```toml
prometheus = { version = "0.13", features = ["process"] }
```
`engine/Cargo.toml:15`:
```toml
prometheus = { workspace = true, optional = true }
```
`api/Cargo.toml` has NO `prometheus` entry (verified — read in full, lines 1-60). Phase 24 adds `prometheus = { workspace = true }` to `api/Cargo.toml`'s `[dependencies]`. The workspace pin (`0.13`) is the source of truth — do NOT bump to `0.14` (the deferred `engine/src/engine/metrics.rs:288` drift is against this same 0.13 API; bumping would widen the surface).

### Engine-side Prometheus — DO NOT extend
`engine/src/engine/metrics.rs:284-310` (`export_prometheus_metrics`) uses `prometheus::Opts { ... }` directly and currently FAILS to compile under `--all-features` (deferred-items.md #1: `namespace: "rrules"` is `&str` not `String`, missing `variable_labels` field). This is a pre-existing v3.0 bug unrelated to Phase 24. The ROADMAP's "engine-side per-engine gauges stay as-is — extension, not replacement" means Phase 24 builds api-layer counters in parallel; it does NOT touch `engine/src/engine/metrics.rs`. The reference impl cited in the ROADMAP (`engine/src/metrics/prometheus.rs:45`) is the `TextEncoder` pattern — Phase 24 mirrors that pattern at the api layer.

### Frontend consumer — already tolerates the real shape
`frontend/src/hooks/useEngineMetrics.ts:4-11`:
```ts
export interface EngineState {
  engine_name: string
  rules_fired: number
  facts_inserted: number
  active_sessions: number
  memory_bytes: number
  last_updated: string
}
```
The WS `onmessage` handler at `:131-161` parses `JSON.parse(event.data)`, checks `if ('engine_name' in message)`, then reads `message.engine_name`, `message.rules_fired`, `message.facts_inserted`, `message.last_updated`. These are EXACTLY the `MetricsSnapshot` fields. So once `execute_single` broadcasts a non-zero snapshot, the Engine dashboard renders live numbers with no frontend change. The HTTP fallback at `:92-99` hits `/v1/monitoring/engine-state` (a separate route, not `/metrics`) — that route is out of scope for Phase 24; the WS path is the one that goes live.

## Implementation Approach

Ordered, concrete. Each deliverable maps to a verifiable check.

### Deliverable 1: `ExecutionCounters` struct + `web::Data` wiring
**What:** A new struct in `api/src/metrics.rs` (or a new `api/src/metrics_counters.rs` module — recommend keeping it in `metrics.rs` to co-locate with `metrics_endpoint`):
```rust
use std::sync::atomic::{AtomicU64, Ordering};
use prometheus::{IntCounterVec, HistogramVec, Registry};

pub struct ExecutionCounters {
    pub requests_total: IntCounterVec,        // label: engine
    pub rules_fired_total: IntCounterVec,     // label: engine
    pub duration_seconds: HistogramVec,       // label: engine
    // AtomicU64 mirrors for the WS snapshot (cheap to read; avoids the
    // prometheus counter get() cell-cell indirection for a hot snapshot path)
    pub snapshot_requests: AtomicU64,
    pub snapshot_rules_fired: AtomicU64,
    pub snapshot_facts_inserted: AtomicU64,
    pub registry: Registry,                   // dedicated api-layer registry
}
```
**Why a dedicated `Registry`:** the engine-side `export_prometheus_metrics` (`engine/src/engine/metrics.rs:284`) uses the default registry pattern implicitly; registering api-layer counters on the same default registry risks a name-collision panic if the engine ever registers `rrules_*` names. A dedicated `Registry::new()` for the api layer sidesteps the collision entirely (Pitfall: registry collision — see below). The `TextEncoder` then encodes only the api-layer registry; the engine-side gauges continue to be exported via their own (broken-but-separate) path, which Phase 24 does not touch.

**Construction (`main.rs`):** build once at startup (alongside `registry`/`selector` at `:87-101`), inject via `.app_data(counters.clone())` at the `HttpServer::new` closure (alongside `metrics_tx` at `:207`).

**Counter registration:** use `prometheus::IntCounterVec::new(opts, &["engine"])?` and `registry.register(Box::new(counter.clone()))?`. Use `Opts::new("rrules_execute_requests_total", "...").namespace("rrules")` OR embed the prefix in the name directly — recommend the explicit-name form (`rrules_execute_requests_total`) to match the ROADMAP success criterion #1's expected `rrules_execute_requests_total` string exactly. The engine label is `"rete"`/`"treat"`/`"leaps"`/`"phreak"` (the resolved `engine_name`).

**Histogram buckets:** default `prometheus::HistogramOpts::new("rrules_execute_duration_seconds", "...")` with exponential buckets starting at 0.001s (~1ms) up to ~10s — `prometheus::exponential_buckets(0.001, 2.0, 12)` covers 1ms to ~4s. The planner should confirm the bucket boundaries (Open Question Q3).

**Verify:** `cargo check -p api` exits 0; `grep -n 'ExecutionCounters' api/src/metrics.rs` returns a match; `grep -n '.app_data(counters' api/src/main.rs` returns a match.

### Deliverable 2: Promote `prometheus` into `api/Cargo.toml`
**What:** Add `prometheus = { workspace = true }` to `api/Cargo.toml` `[dependencies]` (the workspace already pins `0.13` with the `process` feature at `Cargo.toml:15`; the api crate does not need the `process` feature — `{ workspace = true }` inherits it, but that feature is harmless for the api crate; alternatively `prometheus = { version = "0.13" }` to avoid the process feature. Recommend `{ workspace = true }` for version-pin consistency).
**Verify:** `cargo check -p api` exits 0; `grep -n 'prometheus' api/Cargo.toml` returns a match.

### Deliverable 3: `execute_single` increments counters + broadcasts
**What:** Extend `execute_single` (`api/src/v1/execution.rs:327`) to accept `counters: web::Data<ExecutionCounters>` and `metrics_tx: web::Data<MetricsTx>`. Inside:
1. Capture `let start = std::time::Instant::now();` before the `web::block` call (line 411).
2. After `web::block` returns (line 424), compute `let elapsed = start.elapsed().as_secs_f64();`.
3. Increment: `counters.requests_total.with_label_values(&[&engine_name]).inc();`
   `counters.rules_fired_total.with_label_values(&[&engine_name]).inc_by(engine_result.rules_fired as u64);`
   `counters.duration_seconds.with_label_values(&[&engine_name]).observe(elapsed);`
4. Increment the AtomicU64 mirrors: `counters.snapshot_requests.fetch_add(1, Ordering::Relaxed);` etc.
5. Build and broadcast a `MetricsSnapshot`:
   ```rust
   let snapshot = MetricsSnapshot {
       engine_name: engine_name.clone(),
       rules_fired: counters.snapshot_rules_fired.load(Ordering::Relaxed),
       facts_inserted: counters.snapshot_facts_inserted.load(Ordering::Relaxed),
       active_sessions: 0,  // Phase 27 populates this
       memory_bytes: 0,     // out of scope; the dashboard tolerates 0
       last_updated: chrono::Utc::now().to_rfc3339(),
   };
   let _ = metrics_tx.send(snapshot);  // no-op if no subscribers
   ```
6. Add `engine_generation: 0u64` to the returned `ExecuteResponse` (line 464-470).

**Where to increment relative to `spawn_blocking`:** increment AFTER `web::block` returns (line 424), in the async context — NOT inside the `web::block` closure. The `prometheus` counters are `Send + Sync` and could technically be incremented inside the blocking closure, but the `metrics_tx.send()` is an async-channel send and belongs in the async context. Keeping both increments outside the closure is simpler and avoids moving the `web::Data` clones into the blocking task. The `AtomicU64` mirrors and `IntCounterVec` are lock-free, so incrementing them in the async context after the CPU-bound work completes is correct — the counter is incremented exactly once per request, after the work is done.

**`batch_execute_rules` (`execution.rs:514`):** clone `counters` and `metrics_tx` into each `JoinSet` task (alongside the existing `registry`/`selector`/`ruleset` clones at line 529-531) and pass them through to `execute_single`. The zero-fired placeholder at line 546-552 must also include `engine_generation: 0`. Each per-item execution increments the shared counters and broadcasts its own snapshot — this means a batch of N items sends N snapshots and increments N times, which is the correct semantic (each item is an execute).

**Verify:** `cargo test -p api --test execute_integration` exits 0 (existing 5 tests still pass after signature change); a new test `counters_increment_after_execute` POSTs `/v1/execute` then GETs `/metrics` and asserts `rrules_execute_requests_total{engine="rete"}` is present and `>= 1`.

### Deliverable 4: `metrics_endpoint` reads counters
**What:** `metrics_endpoint` (`api/src/metrics.rs:172`) must accept `counters: web::Data<ExecutionCounters>` (add to the handler signature; actix extracts it). In the `text/plain` arm (line 192-203), replace the hardcoded string with:
```rust
let mut buffer = Vec::new();
let encoder = prometheus::TextEncoder::new();
let metric_families = counters.registry.gather();
encoder.encode(&metric_families, &mut buffer).unwrap();
let text = String::from_utf8(buffer).unwrap_or_default();
// Append the existing engine_rules_fired/engine_memory_bytes lines if still
// desired for the metrics_prometheus_text_format test (or update that test —
// see Open Questions Q2).
HttpResponse::Ok().content_type("text/plain; version=0.0.4").body(text)
```
The JSON arm (line 181-191) should read `counters.snapshot_rules_fired.load(...)` etc. instead of `0`. The HTML arm (line 176-180) likewise.

**Verify:** `cargo test -p api --test metrics_integration` exits 0; the existing `metrics_prometheus_text_format` test still passes (asserts `# HELP` present — the `TextEncoder` output contains `# HELP` lines for each registered metric; and `engine_rules_fired` — see Open Questions Q2 about keeping that line).

### Deliverable 5: `ExecuteResponse.engine_generation`
**What:** Add `#[serde(default)] pub engine_generation: u64` to `ExecuteResponse` (`execution.rs:42`). Update both construction sites: the success path at line 464-470 and the batch placeholder at line 546-552. Phase 24 always sets it to `0`; Phase 25 wires the real generation counter.
**Why:** This structurally closes Pitfall 8 — the frontend (Phase 28) can detect "rules changed since this run" by comparing `engine_generation` across responses. Phase 24 ships the field shape so Phase 25/28 don't need a response-shape migration.
**Verify:** `grep -n 'engine_generation' api/src/v1/execution.rs` returns matches in the struct + both construction sites; `cargo check -p api` exits 0.

## Pitfalls

### Pitfall 8: Stale-polling race (the one the ROADMAP flags)
**What goes wrong:** A frontend that polls `/v1/metrics/coverage` immediately after `POST /v1/execute` can see pre-write state (PITFALLS.md:201-228). The WS `EngineState` stream is engine-status, not per-execution results.
**How Phase 24 addresses it:** `ExecuteResponse` gains `engine_generation: u64` (default `0` here; Phase 25 increments it on reload). The `execution_id` field already exists (`execution.rs:44`). Together these let Phase 28's frontend correlate a response with a specific run and detect when the engine's rule set has rotated since. Phase 24 does NOT eliminate polling for the long-term monitoring dashboard — it ships the correlation fields so Phase 28 can drive the test-runner UI from the response itself (PITFALLS.md:216).
**Warning signs:** `ExecuteResponse` missing `engine_generation`; frontend building a polling-only refresh contract.

### Pitfall 24-A: Prometheus registry collision with engine-side gauges
**What goes wrong:** Registering api-layer `IntCounterVec`/`HistogramVec` on the `prometheus::default_registry()` panics at registration if the engine crate (under the `metrics` feature) ever registers a colliding name (e.g. `engine_rules_fired`).
**How to avoid:** Use a dedicated `prometheus::Registry::new()` for the api-layer `ExecutionCounters` (not the default registry). Encode only that registry in `metrics_endpoint`. The engine-side `export_prometheus_metrics` (`engine/src/engine/metrics.rs:284`) is a separate, currently-broken, out-of-scope path — Phase 24 does not unify the two registries.
**Warning signs:** `cargo test -p api` panicking at `registry.register(...)` with "duplicate metric name".

### Pitfall 24-B: `AtomicU64` under `spawn_blocking` — no hazard
**What goes wrong (hypothesized):** Could `fetch_add` on `AtomicU64` from inside the `web::block` closure race with reads from `metrics_endpoint`?
**Resolution:** Phase 24 increments the counters AFTER `web::block` returns (in the async context), not inside the closure (see Implementation Approach D3). `AtomicU64` with `Ordering::Relaxed` is correct for monotonic counters — `metrics_endpoint` reads with `Ordering::Relaxed` and may see a slightly-stale value, which is acceptable for a monitoring scrape. No mutex is needed. This is the standard Prometheus-client pattern.

### Pitfall 24-C: Broadcast `send` returns `Err` when no receivers
**What goes wrong:** `metrics_tx.send(snapshot)` returns `Err(SendError(snapshot))` when there are zero subscribers. Today the WS actor subscribes only when a client connects (`websocket.rs:47`) — most of the time there are no subscribers.
**How to avoid:** `let _ = metrics_tx.send(snapshot);` — explicitly discard the error. The broadcast channel is capacity-16 (`main.rs:123`); a no-subscriber send is a no-op, not a panic. This is the correct shape (the WS actor pattern already tolerates this — see `metrics_ws.rs` test guard receiver).

### Pitfall 24-D: `metrics_prometheus_text_format` test asserts `engine_rules_fired`
**What goes wrong:** The existing test at `api/tests/metrics_integration.rs:43-62` asserts the `/metrics` text contains `engine_rules_fired`. If Phase 24 replaces the hardcoded text entirely with the `TextEncoder` output of api-layer counters named `rrules_execute_*`, that substring disappears and the test breaks.
**How to avoid:** Either (a) keep a fallback `engine_rules_fired 0` line appended after the `TextEncoder` output (lowest-risk), or (b) update the test to assert `rrules_execute_requests_total` instead. Recommend (b) — the test exists to prove Prometheus exposition format works, and the new metric name is the Phase 24 keystone. Either way the planner must decide (Open Questions Q2).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Prometheus exposition format | Manual `format!("# HELP ...\n# TYPE ...\nmetric value\n")` string building | `prometheus::TextEncoder::new().encode(&families, &mut buf)` | The encoder handles label escaping, `# HELP`/`# TYPE` ordering, histogram bucket emission, and float formatting correctly — hand-rolling gets label escaping wrong for `engine="rete"` and histogram quantiles. Reference impl: `engine/src/metrics/prometheus.rs:43-83`. |
| Labeled counters | A `HashMap<String, AtomicU64>` keyed by engine name | `prometheus::IntCounterVec` with `&["engine"]` label | The vec handles label-cardinality, atomic increment per label cell, and exposition in one primitive. |
| Request timing | Manual `SystemTime::now()` arithmetic | `std::time::Instant::now()` + `elapsed()` | `Instant` is monotonic; `SystemTime` can go backwards on NTP adjustments and is wrong for duration measurement. |
| Broadcast channel | A `tokio::sync::Mutex<Vec<Sender>>` fan-out | The existing `tokio::sync::broadcast::Sender<MetricsSnapshot>` (`api/src/metrics.rs:26`) | Already wired through `main.rs:123`/`:207` and consumed by `api/src/websocket.rs:47`. Phase 24 reuses it as-is. |

## Validation Architecture

`config.json` has `workflow.nyquist_validation: true` — this section is mandatory.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` via `cargo test`; `actix-test` 0.1 for HTTP/WS integration (`api/Cargo.toml:54`); `serial_test` (workspace) for singleton isolation |
| Config file | `api/Cargo.toml` `[dev-dependencies]`; per-test files in `api/tests/` |
| Quick run command | `cargo check -p api && cargo test -p api --test execute_integration` |
| Full suite command | `cargo test -p api --tests` (and `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` for engine regression) |

### Phase Requirements → Test Map (Nyquist observation points)
Each ROADMAP success criterion maps to one observable check. The Nyquist dimension here is **amplitude at the endpoint** — observe the metric value before and after the execute event; the sampling rate is "once per execute" (the counter is the sampler; a regression is any post-execute scrape that fails to show the increment).

| Req / Success Criterion | Behavior | Test Type | Automated Command | File Exists? |
|------------------------|----------|-----------|-------------------|-------------|
| SC-1 / MON-07 | After `POST /v1/execute`, `GET /metrics` (Accept: text/plain) exposes `rrules_execute_requests_total{engine="rete"}` `>= 1`, `rrules_execute_rules_fired_total{engine="rete"}` `>= 1`, and `rrules_execute_duration_seconds_bucket{engine="rete",...}` populated | integration | `cargo test -p api --test execute_integration -- counters_exposed_in_metrics -x` (NEW — Wave 0) | ❌ Wave 0 |
| SC-2 / MON-07 | The WebSocket monitoring channel emits a `MetricsSnapshot` reflecting the most-recent execute (non-zero `rules_fired` after a run) | integration (WS) | `cargo test -p api --test metrics_ws -- broadcasts_snapshot_after_execute -x` (NEW — extend the existing `metrics_ws.rs` harness which already does handshake+broadcast+read-frame) | ❌ Wave 0 |
| SC-3 / MON-07 | The existing frontend Engine/Coverage/DeadRule dashboards render live numbers after a run with no frontend code changes | manual + structural | Structural proof: `grep` confirms `frontend/src/hooks/useEngineMetrics.ts` `EngineState` interface fields ⊆ `MetricsSnapshot` struct fields (already verified in Current State). Manual: run the server, POST `/v1/execute`, open the dashboard, observe non-zero. | n/a (structural — see Validation Notes) |
| (counter-correctness) | Counters increment exactly once per execute; batch of N increments N times | integration | `cargo test -p api --test execute_integration -- counters_increment_per_request -x` (NEW) | ❌ Wave 0 |
| (response-shape) | `ExecuteResponse` includes `engine_generation: u64` (default 0); `execution_id` still present | integration | `cargo test -p api --test execute_integration -- response_includes_engine_generation -x` (NEW) | ❌ Wave 0 |
| (no-regression) | Phase 23's five execute_integration tests still pass after the handler signature change | integration | `cargo test -p api --test execute_integration` (existing 5 tests) | ✅ exists — must stay GREEN |
| (no-regression) | `metrics_prometheus_text_format` still passes (or is updated — see Pitfall 24-D) | integration | `cargo test -p api --test metrics_integration -- metrics_prometheus_text_format` | ✅ exists — may need update |

### Sampling Rate (Nyquist)
- **Per task commit:** `cargo check -p api && cargo test -p api --test execute_integration` (fast — the keystone suite).
- **Per wave merge:** `cargo test -p api --tests && cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` (full api + engine regression).
- **Phase gate:** full suite green before `/gsd-verify-work`; the three ROADMAP success criteria above must pass.

### Wave 0 Gaps
- [ ] `api/tests/execute_integration.rs` — add `counters_exposed_in_metrics`, `counters_increment_per_request`, `response_includes_engine_generation` (extend the existing fixture/injection pattern from Plan 23-04).
- [ ] `api/tests/metrics_ws.rs` — add `broadcasts_snapshot_after_execute` (extend the existing `spawn_server` + `handshake_then_broadcast_and_read_frame` helpers; the test POSTs `/v1/execute` and asserts the WS frame's `rules_fired > 0`). Note: this requires the test service to wire `execute_rules` AND `configure_metrics_ws` AND the `metrics_tx`/`counters` app_data together — a slightly heavier App than the existing WS-only test.
- [ ] `api/tests/metrics_integration.rs` — update `metrics_prometheus_text_format` if the `engine_rules_fired` substring is removed (Pitfall 24-D / Q2).

### Validation Notes
- **SC-3 (frontend, no changes):** This is structurally provable without a browser test. The proof is: (1) `frontend/src/hooks/useEngineMetrics.ts:4-11` `EngineState` interface field set = {engine_name, rules_fired, facts_inserted, active_sessions, memory_bytes, last_updated}; (2) `api/src/metrics.rs:16-23` `MetricsSnapshot` struct field set = the same six; (3) the WS `onmessage` handler at `useEngineMetrics.ts:135-150` reads those fields by name. Therefore broadcasting a populated `MetricsSnapshot` renders live numbers with no frontend change. The planner should record this structural proof in the plan and treat the manual "open the dashboard" check as a single verify-phase observation, not a Wave 0 test.

## Open Questions / Flags

1. **Histogram bucket boundaries.** Proposed: `prometheus::exponential_buckets(0.001, 2.0, 12)` → 1ms, 2ms, 4ms … ~4s. The ReteEngine direct evaluator is sub-millisecond for small rulesets, so the first bucket captures the common case; the upper bound covers pathological fixpoint runs. Planner to confirm or tune.
2. **`metrics_prometheus_text_format` test fate.** Does Phase 24 (a) keep a fallback `engine_rules_fired 0` line appended after the `TextEncoder` output so the existing assertion stays GREEN untouched, or (b) update the test to assert `rrules_execute_requests_total`? Recommend (b) — it's the Phase 24 keystone metric and the test exists to prove exposition format, not a specific metric name. Planner to pick.
3. **`MetricsSnapshot.active_sessions` / `memory_bytes`.** Phase 24 sets both to `0` (Phase 27 populates `active_sessions` from the `SessionManager`; `memory_bytes` is out-of-scope for v5.0). The frontend already tolerates `0` for both (verified at `useEngineMetrics.ts:148` — `executionTimeMs: 0` is the analogous "not provided" case). Confirm `0` is acceptable for the WS snapshot in Phase 24.
4. **Dedicated `Registry` vs default.** Recommend a dedicated `prometheus::Registry::new()` for the api-layer counters (Pitfall 24-A). The cost is the `metrics_endpoint` must `gather()` from that specific registry, not the default. Planner to confirm.
5. **Where to increment counters relative to `spawn_blocking`.** Researched recommendation: increment AFTER `web::block` returns, in the async context (not inside the closure) — see Implementation Approach D3. Planner to confirm this is the chosen shape.
6. **`engine_generation` source for Phase 24.** Phase 24 ships the field as `0` always. The real generation counter is Phase 25's scope (D-25 on the shared `Arc<RwLock<RuleSet>>`). `ReteEngine.generation` exists at `engine/src/engine/rete/mod.rs:23` but is never incremented and is per-engine-instance (not shared), so it is NOT the source for Phase 24. Confirm `0` is the Phase 24 value.
7. **`metrics_endpoint` signature change.** Adding `counters: web::Data<ExecutionCounters>` to the `metrics_endpoint` handler (`api/src/metrics.rs:172`) means every test that calls `/metrics` must inject the counters app_data. The existing `metrics_integration.rs:44` test (`test::init_service(App::new().configure(configure_metrics))`) does NOT inject counters — it will fail to extract. Planner must update those tests to inject `ExecutionCounters` (or provide a `Default` impl + `web::Data::default()`). Flag for the plan.

## File Impact

| File | Action | Role |
|------|--------|------|
| `api/Cargo.toml` | modify | Add `prometheus = { workspace = true }` to `[dependencies]` (D2). |
| `api/src/metrics.rs` | modify | Add `ExecutionCounters` struct + constructor; extend `metrics_endpoint` signature to accept `web::Data<ExecutionCounters>`, replace the hardcoded text/plain arm with `TextEncoder::encode(counters.registry.gather(), ...)`, read live counters in the JSON/HTML arms; (optionally) add a `Default` impl for `ExecutionCounters` so tests can `web::Data::default()`. |
| `api/src/main.rs` | modify | Construct `ExecutionCounters` at startup (alongside `registry`/`selector` at `:87-101`); inject via `.app_data(counters.clone())` at `:207` alongside `metrics_tx`. |
| `api/src/v1/execution.rs` | modify | Add `engine_generation: u64` (`#[serde(default)]`) to `ExecuteResponse` (`:42`); extend `execute_single` signature (`:327`) with `counters: web::Data<ExecutionCounters>` + `metrics_tx: web::Data<MetricsTx>`; time the `web::block` call (`:411`); increment counters + broadcast `MetricsSnapshot` after it returns (`:424`); set `engine_generation: 0` in the success response (`:464`) and the batch placeholder (`:546`); extend `execute_rules` (`:484`) and `batch_execute_rules` (`:514`) handler signatures to extract the two new `web::Data` and clone them into the `JoinSet` tasks. |
| `api/tests/execute_integration.rs` | modify | Update existing 5 tests to inject `ExecutionCounters` + `MetricsTx` app_data (signature change); add `counters_exposed_in_metrics`, `counters_increment_per_request`, `response_includes_engine_generation`. |
| `api/tests/metrics_integration.rs` | modify | Update `metrics_prometheus_text_format` (and the JSON test) to inject `ExecutionCounters` app_data; update assertions per Q2. |
| `api/tests/metrics_ws.rs` | modify | Add `broadcasts_snapshot_after_execute` (extend `spawn_server` to wire the execute route + counters + metrics_tx together). |
| `frontend/src/hooks/useEngineMetrics.ts` | NO CHANGE | Already consumes the real `MetricsSnapshot` shape (verified at `:4-11`, `:135-150`). |
| `frontend/src/features/monitoring/dashboards/*` | NO CHANGE | Dashboards render from the hook; zero frontend changes (ROADMAP SC-3). |
| `engine/src/engine/metrics.rs` | NO CHANGE | The pre-existing `Opts` drift (deferred-items.md #1) is out of scope; api-layer counters use a dedicated `Registry`. |

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hardcoded `/metrics` text (`api/src/metrics.rs:192-203`) | `prometheus::TextEncoder` encoding a dedicated api-layer `Registry` | Phase 24 | `/metrics` exposes real `rrules_execute_*` counters with engine labels — Prometheus scrape contract becomes honest. |
| Silent `metrics_tx` channel (created `main.rs:123`, never sent) | `execute_single` broadcasts a populated `MetricsSnapshot` after each execute | Phase 24 | The WS monitoring stream goes live; the Engine dashboard shows non-zero `rules_fired` after a run. |
| `ExecuteResponse` without `engine_generation` | `engine_generation: u64` field (default `0`) | Phase 24 | Structurally closes Pitfall 8; Phase 25 increments it on reload; Phase 28's frontend correlates runs by generation. |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The `prometheus` 0.13 `IntCounterVec`/`HistogramVec`/`TextEncoder`/`Registry` API used in the Implementation Approach is stable and matches the workspace-pinned version. | Implementation Approach D1/D4 | The engine-side `Opts` drift (deferred-items.md #1) shows 0.13 has API surfaces that drift; but `IntCounterVec::new`/`HistogramVec::new`/`TextEncoder::encode`/`Registry::gather` are the stable core API used in the reference impl at `engine/src/metrics/prometheus.rs:45`. LOW risk. |
| A2 | The frontend `useEngineMetrics.ts` is the only WS consumer of `MetricsSnapshot`. | Phase Boundary / Validation | If there are other consumers, they would also need to tolerate the same shape — but they'd be reading the same struct, so no change needed anyway. LOW risk. |
| A3 | `engine_generation: 0` for Phase 24 is acceptable (Phase 25 wires the real counter). | Implementation Approach D5 | If Phase 25's generation counter has a different type (e.g. `u32` or `String`), the Phase 24 field shape would need migration. Mitigated by using `u64` (the natural counter type). LOW risk. |

**If this table is empty:** All other claims were verified by direct codebase reads (file:line cited inline) — HIGH confidence.

## Sources

### Primary (HIGH confidence — direct codebase reads)
- `api/src/v1/execution.rs` — `execute_single` (327-471), `ExecuteResponse` (42-52, `execution_id` already at 44), `batch_execute_rules` (514-590, placeholder at 546-552), `engine_value_to_json` (725-744).
- `api/src/main.rs` — `metrics_tx` channel creation (123), app_data injection (207), registry/selector/ruleset wiring (87-120).
- `api/src/metrics.rs` — `MetricsSnapshot` struct (16-23), `MetricsTx` type alias (26), `create_metrics_channel` (29-32), `metrics_endpoint` (172-205, hardcoded text at 192-203), `dead_rules_endpoint` (216), `coverage_endpoint` (235), `configure_metrics` (253), `configure_metrics_dashboards` (268).
- `api/src/websocket.rs` — `MetricsWsSession` actor (37-60), subscribe at `started` (47), `BROADCAST_POLL_INTERVAL` 50ms (28), `metrics_ws` upgrade (89-101), `configure_metrics_ws` (109-111).
- `api/Cargo.toml` — no `prometheus` dep (verified absent); `engine` features enabled (17); `actix-test` 0.1 (54); `serial_test` (59).
- `Cargo.toml` (workspace) — `prometheus = { version = "0.13", features = ["process"] }` (15).
- `engine/Cargo.toml` — `prometheus = { workspace = true, optional = true }` (15); `metrics = ["prometheus"]` feature (31).
- `engine/src/engine/registry.rs` — `ExecutionEngine` trait (64-108), `EngineResult` (112-148, `fired_rule_ids` at 126), `EngineResult::success` (129-138).
- `engine/src/engine/metrics.rs` — `export_prometheus_metrics` (284-310, broken `Opts` at 288), `IntCounterVec::new` usage (302) — reference for the api-layer pattern.
- `engine/src/metrics/prometheus.rs` — `EnginePrometheusMetrics::export_prometheus_text` (43-83) — the `TextEncoder` reference impl cited by the ROADMAP.
- `engine/src/engine/rete/mod.rs` — `generation: usize` field (23, never incremented — confirms Phase 24 uses `0`).
- `frontend/src/hooks/useEngineMetrics.ts` — `EngineState` interface (4-11), `EngineStateResponse` (39-50), `adaptEngineStateResponse` (53-67), WS `onmessage` handler (131-161), `buildMetricsWsUrl` (78-84) — confirms zero frontend changes.
- `api/tests/metrics_integration.rs` — `metrics_prometheus_text_format` (43-62, asserts `# HELP` + `engine_rules_fired`), `metrics_json_engine_state` (65-75).
- `api/tests/metrics_ws.rs` — `spawn_server` (38-53), `create_metrics_channel` reuse (44), `MetricsSnapshot`/`MetricsTx` import (21).
- `.planning/research/PITFALLS.md` — Pitfall 8 (201-228), Pitfall 10 (247-254).
- `.planning/phases/23-execute-evaluator-wiring/deferred-items.md` — pre-existing `Opts` drift at `engine/src/engine/metrics.rs:288-293` (#1).
- `.planning/phases/23-execute-evaluator-wiring/23-05-SUMMARY.md` + `23-06-SUMMARY.md` — Phase 23 deliverables (the execute path Phase 24 extends).
- `.planning/ROADMAP.md` — Phase 24 entry (125-150): goal, deliverables, pitfalls, success criteria.

### Secondary (MEDIUM confidence)
- None — no external docs consulted. The `prometheus` crate API claims (A1) are based on the in-tree reference impl at `engine/src/metrics/prometheus.rs:45` and `engine/src/engine/metrics.rs:302`, which are verified to compile under the `engine-rete` feature (the `--all-features` failure is in a different function).

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack (`prometheus` 0.13 API at the api layer): HIGH — the reference impl is in-tree and compiles under the features the api crate already enables; the workspace pin is read at `Cargo.toml:15`.
- Architecture (counter wiring + WS broadcast + response shape): HIGH — every claim verified by direct read of the live codebase with file:line; the frontend-tolerates-real-shape claim is verified by reading `useEngineMetrics.ts` in full.
- Pitfalls: HIGH — Pitfall 8 re-verified against `PITFALLS.md:201-228` and the live `ExecuteResponse`/`execution_id`; the registry-collision and broadcast-no-receiver pitfalls are standard `prometheus`/`tokio::broadcast` semantics.
- Validation: HIGH — the existing `metrics_ws.rs` harness already does the WS handshake+broadcast+read-frame pattern Phase 24's new test mirrors; the structural proof for SC-3 is grounded in two file:line-cited field-set comparisons.

**Research date:** 2026-07-03
**Valid until:** 2026-08-03 (stable — internal codebase; the only external surface is the `prometheus` 0.13 API, which is pinned at the workspace level and unlikely to drift within 30 days)