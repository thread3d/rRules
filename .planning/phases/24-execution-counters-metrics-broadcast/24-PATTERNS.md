# Phase 24: Execution counters + metrics broadcast - Pattern Map

**Mapped:** 2026-07-03
**Files analyzed:** 7 (modify) + 3 (NO CHANGE — verified, listed for completeness)
**Analogs found:** 7 / 7 (every modified file has an in-tree analog)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `api/Cargo.toml` | config | n/a (manifest) | `engine/Cargo.toml:15` (`prometheus = { workspace = true, optional = true }`) | exact |
| `api/src/metrics.rs` | model + controller (struct def + HTTP handler) | request-response + shared-state | `engine/src/metrics/prometheus.rs` (TextEncoder pattern) + existing `metrics_endpoint` at `api/src/metrics.rs:172` | exact (same file extends in place) |
| `api/src/main.rs` | config (app startup wiring) | app-startup | existing `web::Data` injection block at `api/src/main.rs:200-216` | exact (same file extends in place) |
| `api/src/v1/execution.rs` | controller | request-response | existing `execute_single` at `api/src/v1/execution.rs:327` (the Phase 23 rewrite) | exact (same function extends in place) |
| `api/tests/execute_integration.rs` | test | request-response (HTTP integration) | existing `init_test_app!` macro + serial-test fixtures at `api/tests/execute_integration.rs:140-162` | exact (same file extends in place) |
| `api/tests/metrics_integration.rs` | test | request-response (HTTP integration) | existing `metrics_prometheus_text_format` at `api/tests/metrics_integration.rs:42-62` | exact (same file extends in place) |
| `api/tests/metrics_ws.rs` | test | event-driven (WS broadcast) | existing `spawn_server` + `handshake_then_broadcast_and_read_frame` at `api/tests/metrics_ws.rs:38-112` | exact (same file extends in place) |

**NO CHANGE files (verified, listed so the planner can cite the structural proof):**

| File | Role | Reason |
|------|------|--------|
| `frontend/src/hooks/useEngineMetrics.ts` | hook (consumer) | `EngineState` interface (`:4-11`) + WS `onmessage` (`:131-161`) already consume the exact `MetricsSnapshot` shape — zero frontend changes (RESEARCH SC-3 structural proof). |
| `frontend/src/features/monitoring/dashboards/*` | UI component | Render from the hook; no changes. |
| `engine/src/engine/metrics.rs` | service (engine-side metrics) | Pre-existing `Opts` drift at `:288-293` is out of scope (23-deferred-items.md #1). Phase 24 builds api-layer counters in parallel on a dedicated `Registry`; do NOT touch this file. |

---

## Pattern Assignments

### `api/Cargo.toml` (config, manifest)

**Analog:** `engine/Cargo.toml:15`

**Workspace-dep promotion pattern** (`engine/Cargo.toml:15`):
```toml
prometheus = { workspace = true, optional = true }
```

**Workspace pin** (`Cargo.toml:15` — the source of truth, do NOT bump):
```toml
prometheus = { version = "0.13", features = ["process"] }
```

**Action for Phase 24:** Add to `api/Cargo.toml` `[dependencies]` (NOT `[dev-dependencies]` — the counters are runtime, not test-only). The api crate does not need `optional = true` (the engine crate marks it optional because the `metrics` feature gates it; the api crate always uses it). Recommended form:
```toml
prometheus = { workspace = true }
```
The `process` feature is inherited but harmless for the api crate. The existing api `[dependencies]` block runs lines 10-47; insert the new entry alongside the other runtime deps (e.g. after `jsonwebtoken` at line 47).

---

### `api/src/metrics.rs` (model + controller, request-response + shared-state)

**Analog:** `engine/src/metrics/prometheus.rs` (TextEncoder pattern) + the existing `metrics_endpoint` in the same file.

**Existing `MetricsSnapshot` struct — the WS payload, do NOT change field set** (`api/src/metrics.rs:15-23`):
```rust
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub engine_name: String,
    pub rules_fired: u64,
    pub facts_inserted: u64,
    pub active_sessions: usize,
    pub memory_bytes: u64,
    pub last_updated: String,
}
```
This is the struct `execute_single` will broadcast. The frontend `EngineState` interface is field-for-field identical (RESEARCH SC-3).

**Existing `MetricsTx` type alias + channel constructor** (`api/src/metrics.rs:26-32`):
```rust
pub type MetricsTx = Arc<tokio::sync::broadcast::Sender<MetricsSnapshot>>;

pub fn create_metrics_channel(capacity: usize) -> (MetricsTx, tokio::sync::broadcast::Receiver<MetricsSnapshot>) {
    let (tx, rx) = tokio::sync::broadcast::channel(capacity);
    (Arc::new(tx), rx)
}
```
Phase 24 reuses this channel as-is — `main.rs:123` already creates it; `execute_single` just needs to call `metrics_tx.send(snapshot)` and discard the `Err(SendError)` (Pitfall 24-C).

**Existing `metrics_endpoint` signature — Phase 24 adds a `counters: web::Data<ExecutionCounters>` parameter** (`api/src/metrics.rs:163-205`):
```rust
#[utoipa::path(
    get,
    path = "/metrics",
    tag = "metrics",
    responses(
        (status = 200, description = "Metrics page (HTML), JSON, or Prometheus text format depending on Accept header"),
    ),
)]
pub async fn metrics_endpoint(req: actix_web::HttpRequest) -> impl Responder {
    let accept = req.headers().get("Accept").and_then(|h| h.to_str().ok()).unwrap_or("");

    match accept {
        _ if accept.contains("text/html") || accept.contains("*/*") => {
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(metrics_html("default", 0, 0, 0))
        }
        _ if accept.contains("application/json") => {
            HttpResponse::Ok()
                .content_type("application/json")
                .json(serde_json::json!({
                    "engine_name": "default",
                    "rules_fired": 0,
                    "facts_inserted": 0,
                    "active_sessions": 0,
                    "memory_bytes": 0
                }))
        }
        _ => {
            let metrics_text = r#"# HELP engine_rules_fired Total number of rules fired
# TYPE engine_rules_fired counter
engine_rules_fired 0
# HELP engine_memory_bytes Current memory usage in bytes
# TYPE engine_memory_bytes gauge
engine_memory_bytes 0
"#;
            HttpResponse::Ok()
                .content_type("text/plain; version=0.0.4")
                .body(metrics_text)
        }
    }
}
```
**Note for the planner:** the current signature takes only `req: actix_web::HttpRequest`. Adding `counters: web::Data<ExecutionCounters>` means every test that calls `/metrics` MUST inject the counters `app_data` (Pitfall — see `metrics_integration.rs:44` which currently registers only `configure_metrics`). Either provide a `Default` impl for `ExecutionCounters` so tests can `web::Data::default()`, or update each test's `App::new()` to inject a real `ExecutionCounters`.

**TextEncoder reference pattern** (`engine/src/metrics/prometheus.rs:43-83`):
```rust
pub fn export_prometheus_text(&self) -> String {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();

    // Firing rate (gauge - rate per second)
    buffer.extend_from_slice(b"# HELP engine_firing_rate_current Current firing rate (rules/sec)\n");
    buffer.extend_from_slice(b"# TYPE engine_firing_rate_current gauge\n");
    buffer.extend_from_slice(format!("engine_firing_rate_current {}\n", self.firing_rate).into_bytes().as_slice());
    // ... repeat per metric ...
    String::from_utf8(buffer).unwrap_or_else(|_| String::new())
}
```
**Prefer the `Encoder::encode` form over hand-rolled `extend_from_slice`** (RESEARCH "Don't Hand-Roll"). Phase 24 should call:
```rust
let encoder = prometheus::TextEncoder::new();
let metric_families = counters.registry.gather();
let mut buffer = Vec::new();
encoder.encode(&metric_families, &mut buffer).unwrap();
let text = String::from_utf8(buffer).unwrap_or_default();
```
This handles label escaping for `engine="rete"` and histogram bucket emission correctly.

**`IntCounterVec` reference (engine-side, DO NOT extend — pattern only)** (`engine/src/engine/metrics.rs:284-306`):
```rust
let opts = prometheus::Opts {
    namespace: "rrules",
    subsystem: engine_name.to_string(),
    name: format!("{}_metrics", engine_name),
    help: format!("{} execution metrics", engine_name),
    const_labels: labels
        .map(|l| { l.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>() })
        .unwrap_or_default(),
};
let counter = prometheus::IntCounterVec::new(opts, &[]).expect("Failed to create counter");
```
**WARNING:** this engine-side code is BROKEN under `--all-features` (deferred-items.md #1 — `namespace: "rrules"` is `&str` not `String`, missing `variable_labels` field). It is the pattern source ONLY; do NOT copy its `Opts` struct-literal form. Phase 24 should use `prometheus::Opts::new("rrules_execute_requests_total", "Total execute requests").const_label("engine", ...)` OR embed the prefix in the metric name directly (recommended — RESEARCH D1 says use explicit name `rrules_execute_requests_total` to match ROADMAP SC-1's expected string exactly). Register on a **dedicated `prometheus::Registry::new()`**, NOT the default registry (Pitfall 24-A).

**Recommended `ExecutionCounters` shape (per RESEARCH D1)** — to be added in `api/src/metrics.rs` (co-located with `metrics_endpoint`):
```rust
use std::sync::atomic::{AtomicU64, Ordering};
use prometheus::{IntCounterVec, HistogramVec, Registry};

pub struct ExecutionCounters {
    pub requests_total: IntCounterVec,        // label: engine
    pub rules_fired_total: IntCounterVec,     // label: engine
    pub duration_seconds: HistogramVec,       // label: engine
    // AtomicU64 mirrors for cheap WS snapshot reads
    pub snapshot_requests: AtomicU64,
    pub snapshot_rules_fired: AtomicU64,
    pub snapshot_facts_inserted: AtomicU64,
    pub registry: Registry,                   // dedicated api-layer registry (Pitfall 24-A)
}
```

**Route registration (unchanged)** (`api/src/metrics.rs:253-258`):
```rust
pub fn configure_metrics(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/metrics")
            .route("", web::get().to(metrics_endpoint)),
    );
}
```
Phase 24 does NOT change `configure_metrics` — only the `metrics_endpoint` handler signature. `main.rs` registers it at root (separate from `/v1`).

---

### `api/src/main.rs` (config, app-startup)

**Analog:** the existing `web::Data` wiring block in the same file.

**Existing channel creation + app_data injection** (`api/src/main.rs:123` + `:207`):
```rust
// line 123 — created once at startup
let (metrics_tx, _) = metrics::create_metrics_channel(16);

// line 207 — injected per worker inside HttpServer::new closure
.app_data(web::Data::new(metrics_tx.clone()))
```

**Existing registry/selector/ruleset construction pattern** (`api/src/main.rs:87-120`):
```rust
let mut registry = EngineRegistry::new();
registry.register_default_engines();
let registry = web::Data::new(registry);

let mut selector = EngineSelector::with_defaults();
let default_engine = std::env::var("RRULES_DEFAULT_ENGINE")
    .unwrap_or_else(|_| "rete".to_string());
if selector.select_engine(&default_engine).is_err() {
    tracing::warn!(engine = %default_engine, "Invalid RRULES_DEFAULT_ENGINE, falling back to rete");
    let _ = selector.select_engine("rete");
}
let selector = web::Data::new(selector);

// D-03: shared read-only Arc<RwLock<RuleSet>>
let ruleset = build_ruleset_from_dir(&rules_dir);
let ruleset = web::Data::new(std::sync::Arc::new(std::sync::RwLock::new(ruleset)));
```

**Existing app_data block in HttpServer::new closure** (`api/src/main.rs:200-216`):
```rust
App::new()
    .wrap(cors)
    .app_data(engine_state.clone())
    // Phase 23 (D-03 / L-07): real engine wiring for POST /v1/execute.
    .app_data(registry.clone())
    .app_data(selector.clone())
    .app_data(ruleset.clone())
    .app_data(web::Data::new(metrics_tx.clone()))
    .app_data(web::Data::new(rule_store.clone()))
    .app_data(web::Data::new(category_store.clone()))
    // ...
```

**Action for Phase 24:** Construct `ExecutionCounters` once at startup (alongside `registry`/`selector` at `:87-120`), wrap as `web::Data::new(counters)`, and add `.app_data(counters.clone())` in the `HttpServer::new` closure alongside `metrics_tx` at `:207`. The `counters` must be constructed before the `HttpServer::new` closure (it is captured by the `move` closure, same as `metrics_tx`). Pattern: `let counters = web::Data::new(ExecutionCounters::new());` then `.app_data(counters.clone())` inside the closure.

---

### `api/src/v1/execution.rs` (controller, request-response)

**Analog:** the existing `execute_single` / `execute_rules` / `batch_execute_rules` in the same file (Phase 23 rewrite).

**Existing `ExecuteResponse` — add `engine_generation: u64` ONLY** (`api/src/v1/execution.rs:42-52`):
```rust
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ExecuteResponse {
    pub execution_id: String,
    pub rules_fired: usize,
    #[serde(default)]
    pub rules_not_fired: Vec<String>,
    #[serde(default)]
    pub facts_after: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference_trace: Option<InferenceTrace>,
}
```
Phase 24 adds ONE field:
```rust
#[serde(default)]
pub engine_generation: u64,
```
Do NOT re-add `execution_id` (already present at `:44`). The `#[serde(default)]` keeps existing clients (and existing tests) from breaking.

**Existing `execute_single` signature — Phase 24 adds two `web::Data` params** (`api/src/v1/execution.rs:327-332`):
```rust
pub async fn execute_single(
    req: ExecuteRequest,
    registry: web::Data<EngineRegistry>,
    selector: web::Data<EngineSelector>,
    ruleset: web::Data<Arc<std::sync::RwLock<RuleSet>>>,
) -> Result<ExecuteResponse, HttpResponse>
```
Phase 24 shape:
```rust
pub async fn execute_single(
    req: ExecuteRequest,
    registry: web::Data<EngineRegistry>,
    selector: web::Data<EngineSelector>,
    ruleset: web::Data<Arc<std::sync::RwLock<RuleSet>>>,
    counters: web::Data<ExecutionCounters>,
    metrics_tx: web::Data<MetricsTx>,
) -> Result<ExecuteResponse, HttpResponse>
```
Imports needed at the top of the file (`execution.rs:1-13` currently): add `use crate::metrics::{ExecutionCounters, MetricsTx, MetricsSnapshot};` and `use std::sync::atomic::Ordering;`.

**Existing `web::block` call — Phase 24 wraps with `Instant::now()` / `elapsed()`** (`api/src/v1/execution.rs:411-424`):
```rust
let (engine_result, post_facts) = match web::block(move || {
    let result = engine.execute();
    let facts = engine.get_facts();
    (result, facts)
})
.await
{
    Ok(pair) => pair,
    Err(e) => {
        return Err(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("engine execution panicked: {}", e)
        })))
    }
};
```
**Phase 24 increments AFTER `web::block` returns (in the async context — Pitfall 24-B), NOT inside the closure.** Insert between this match and the `facts_after` block at `:426`:
```rust
let elapsed = start.elapsed().as_secs_f64();
counters.requests_total.with_label_values(&[&engine_name]).inc();
counters.rules_fired_total.with_label_values(&[&engine_name]).inc_by(engine_result.rules_fired as u64);
counters.duration_seconds.with_label_values(&[&engine_name]).observe(elapsed);
counters.snapshot_requests.fetch_add(1, Ordering::Relaxed);
counters.snapshot_rules_fired.fetch_add(engine_result.rules_fired as u64, Ordering::Relaxed);
// snapshot_facts_inserted: increment by req.facts.len() or 0 per RESEARCH Q3
let snapshot = MetricsSnapshot {
    engine_name: engine_name.clone(),
    rules_fired: counters.snapshot_rules_fired.load(Ordering::Relaxed),
    facts_inserted: counters.snapshot_facts_inserted.load(Ordering::Relaxed),
    active_sessions: 0,   // Phase 27
    memory_bytes: 0,      // out of scope (RESEARCH Q3)
    last_updated: chrono::Utc::now().to_rfc3339(),
};
let _ = metrics_tx.send(snapshot);  // no-op if no subscribers (Pitfall 24-C)
```
Capture `let start = std::time::Instant::now();` BEFORE the `web::block` call at `:411`.

**Existing success response construction — add `engine_generation: 0`** (`api/src/v1/execution.rs:464-470`):
```rust
Ok(ExecuteResponse {
    execution_id,
    rules_fired: engine_result.rules_fired,
    rules_not_fired,
    facts_after,
    inference_trace: None,
})
```
Phase 24 adds `engine_generation: 0u64,` to this struct literal.

**Existing `execute_rules` (the actix handler at `/v1/execute`)** (`api/src/v1/execution.rs:484-494`):
```rust
pub async fn execute_rules(
    req: web::Json<ExecuteRequest>,
    registry: web::Data<EngineRegistry>,
    selector: web::Data<EngineSelector>,
    ruleset: web::Data<Arc<std::sync::RwLock<RuleSet>>>,
) -> impl Responder {
    match execute_single(req.into_inner(), registry, selector, ruleset).await {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(err) => err,
    }
}
```
Phase 24 adds `counters: web::Data<ExecutionCounters>` and `metrics_tx: web::Data<MetricsTx>` to the handler signature and forwards them into `execute_single`. actix extracts `web::Data` by type from the `app_data` registry.

**Existing `batch_execute_rules` — clone the new `web::Data` into each `JoinSet` task** (`api/src/v1/execution.rs:514-555`):
```rust
pub async fn batch_execute_rules(
    req: web::Json<BatchExecuteRequest>,
    registry: web::Data<EngineRegistry>,
    selector: web::Data<EngineSelector>,
    ruleset: web::Data<Arc<std::sync::RwLock<RuleSet>>>,
) -> impl Responder {
    // ...
    for request in requests {
        let permit_sem = semaphore.clone();
        let registry = registry.clone();
        let selector = selector.clone();
        let ruleset = ruleset.clone();
        join_set.spawn(async move {
            let _permit = permit_sem.acquire_owned().await.expect("batch semaphore closed unexpectedly");
            match execute_single(request, registry, selector, ruleset).await {
                Ok(resp) => resp,
                Err(_) => ExecuteResponse {
                    execution_id: Uuid::new_v4().to_string(),
                    rules_fired: 0,
                    rules_not_fired: Vec::new(),
                    facts_after: HashMap::new(),
                    inference_trace: None,
                },
            }
        });
    }
    // ...
}
```
Phase 24: (1) add `counters` + `metrics_tx` to the handler signature; (2) clone both into the `move` closure alongside `registry`/`selector`/`ruleset` at `:529-531`; (3) pass them through to `execute_single`; (4) add `engine_generation: 0u64` to the zero-fired placeholder at `:546-552`. Each per-item execution increments the shared counters and broadcasts its own snapshot (correct semantic — N items = N executes).

---

### `api/tests/execute_integration.rs` (test, request-response HTTP integration)

**Analog:** the existing `init_test_app!` macro + `shared_*` fixture helpers in the same file.

**Existing test app wiring — Phase 24 must add `ExecutionCounters` + `MetricsTx` app_data** (`api/tests/execute_integration.rs:136-151`):
```rust
/// Initialize the test service with the full Plan 23-05 app_data stack.
macro_rules! init_test_app {
    () => {
        test::init_service(
            App::new()
                .app_data(shared_registry())
                .app_data(shared_selector())
                .app_data(shared_ruleset())
                .configure(configure_test_routes),
        )
        .await
    };
}
```
Phase 24 adds two more `.app_data(...)` lines:
```rust
.app_data(shared_counters())       // web::Data<ExecutionCounters>
.app_data(shared_metrics_tx())     // web::Data<MetricsTx>
```
Define `fn shared_counters() -> web::Data<ExecutionCounters>` (returns a fresh `ExecutionCounters::new()` — or `web::Data::default()` if a `Default` impl exists) and `fn shared_metrics_tx() -> web::Data<MetricsTx>` (calls `create_metrics_channel(16).0` wrapped in `web::Data::new`). Add `use api::metrics::{ExecutionCounters, MetricsTx, create_metrics_channel};` to the test's imports at `:35-41`.

**Existing serial-test isolation pattern** (`api/tests/execute_integration.rs:155-162`):
```rust
fn reset_test_env() {
    CoverageTracking.clear();
    std::env::set_var("RRULES_DEFAULT_ENGINE", "rete");
}
```
All tests are `#[serial_test::serial]` (`:172`, `:199`, `:228`, `:266`, `:299`). New Phase 24 tests (`counters_exposed_in_metrics`, `counters_increment_per_request`, `response_includes_engine_generation`) MUST also be `#[serial_test::serial]` and call `reset_test_env()` first — they share the `CoverageTracking` singleton and the env var. Each test that POSTs `/v1/execute` will increment the shared `ExecutionCounters`; if counter-order assertions are sensitive, use a fresh `ExecutionCounters` per test (the `shared_counters()` helper returns a fresh one each call, but the macro inlines one per app — so each test's app gets its own counters, which is correct).

**Existing `json_body` helper** (`api/tests/execute_integration.rs:121-134`):
```rust
async fn json_body(resp: actix_web::dev::ServiceResponse<BoxBody>) -> Value {
    assert!(resp.status().is_success(), "expected success, got {}", resp.status());
    let bytes = test::read_body(resp).await;
    serde_json::from_slice(&bytes).unwrap_or_else(|e| { panic!(...) })
}
```
Reuse for the new tests. The `counters_exposed_in_metrics` test must also register `configure_metrics` at root and GET `/metrics` with `Accept: text/plain` (mirror `metrics_integration.rs:43-62`) — add `.configure(configure_metrics)` to the `init_test_app!` macro OR build a dedicated app for that test.

**Existing `configure_test_routes`** (`api/tests/execute_integration.rs:111-117`):
```rust
fn configure_test_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/execute", web::post().to(execute_rules))
            .route("/metrics/coverage", web::get().to(coverage_endpoint)),
    );
}
```
For `counters_exposed_in_metrics`, add `api::metrics::configure_metrics` at root (outside the `/v1` scope) so `GET /metrics` resolves.

---

### `api/tests/metrics_integration.rs` (test, request-response HTTP integration)

**Analog:** the existing `metrics_prometheus_text_format` + `metrics_json_engine_state` in the same file.

**Existing test wiring — Phase 24 must inject `ExecutionCounters` app_data** (`api/tests/metrics_integration.rs:42-62`):
```rust
#[actix_web::test]
async fn metrics_prometheus_text_format() {
    let app = test::init_service(App::new().configure(configure_metrics)).await;

    let req = test::TestRequest::get()
        .uri("/metrics")
        .insert_header(("Accept", "text/plain"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "status: {}", resp.status());
    let bytes = test::read_body(resp).await;
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("# HELP"), "expected Prometheus exposition format; got: {text}");
    assert!(text.contains("engine_rules_fired"), "expected the engine_rules_fired metric; got: {text}");
}
```
After Phase 24 adds `counters: web::Data<ExecutionCounters>` to `metrics_endpoint`, this test's `App::new().configure(configure_metrics)` will FAIL to extract the counters (actix returns 500 for missing `app_data`). **Fix:** add `.app_data(web::Data::new(ExecutionCounters::default()))` (or a `test_counters()` helper) to the `App::new()` chain. Imports to add: `use api::metrics::{ExecutionCounters, configure_metrics, configure_metrics_dashboards};` (currently `:19` imports only `configure_metrics, configure_metrics_dashboards`).

**Assertion update (Pitfall 24-D / RESEARCH Q2):** the existing assertion `text.contains("engine_rules_fired")` will break if the hardcoded text is fully replaced by `TextEncoder` output of `rrules_execute_*` metrics. Recommended fix (RESEARCH Q2 option b): change the assertion to `text.contains("rrules_execute_requests_total")`. The `# HELP` assertion stays — `TextEncoder` emits `# HELP` lines for every registered metric family. Planner to pick (a) keep fallback line or (b) update assertion — recommend (b).

**Existing `metrics_json_engine_state`** (`api/tests/metrics_integration.rs:64-77`) — same `app_data` injection fix; assertions on `engine_name`/`rules_fired`/`memory_bytes` numeric stay valid (the JSON arm now reads live counter values, which are `0` until a POST `/v1/execute` happens in-process — these tests don't POST, so values stay `0` and the numeric-shape assertions still pass).

---

### `api/tests/metrics_ws.rs` (test, event-driven WS broadcast)

**Analog:** the existing `spawn_server` + `handshake_then_broadcast_and_read_frame` helpers in the same file.

**Existing `spawn_server` — Phase 24's new test must wire execute route + counters + metrics_tx together** (`api/tests/metrics_ws.rs:38-53`):
```rust
fn spawn_server() -> (
    std::net::SocketAddr,
    MetricsTx,
    tokio::sync::broadcast::Receiver<MetricsSnapshot>,
    actix_test::TestServer,
) {
    let (metrics_tx, guard_rx) = create_metrics_channel(64);
    let metrics_tx_for_server = metrics_tx.clone();
    let server = start(move || {
        App::new()
            .app_data(web::Data::new(metrics_tx_for_server.clone()))
            .service(web::scope("/v1").configure(configure_metrics_ws))
    });
    let addr = server.addr().to_owned();
    (addr, metrics_tx, guard_rx, server)
}
```
The existing `spawn_server` wires ONLY the WS route. The new `broadcasts_snapshot_after_execute` test needs the execute route AND `ExecutionCounters` AND `metrics_tx` AND a real `EngineRegistry`/`Selector`/`RuleSet` (so `POST /v1/execute` actually fires a rule and broadcasts). Build a heavier `spawn_server_with_execute()` variant mirroring `execute_integration.rs`'s `shared_registry()`/`shared_selector()`/`shared_ruleset()` fixture pattern (re-import or duplicate those helpers — they live in a different test file, so either duplicate inline or extract to a `tests/common/` module if the planner prefers). The `App::new()` chain becomes:
```rust
App::new()
    .app_data(shared_registry())
    .app_data(shared_selector())
    .app_data(shared_ruleset())
    .app_data(web::Data::new(ExecutionCounters::default()))
    .app_data(web::Data::new(metrics_tx_for_server.clone()))
    .service(web::scope("/v1")
        .route("/execute", web::post().to(execute_rules))
        .configure(configure_metrics_ws))
```

**Existing broadcast helper — Phase 24's new test does NOT broadcast manually** (`api/tests/metrics_ws.rs:59-112`):
```rust
async fn handshake_then_broadcast_and_read_frame(
    addr: std::net::SocketAddr,
    metrics_tx: MetricsTx,
) -> (String, String) {
    // ... handshake ...
    let state = MetricsSnapshot { engine_name: "test-engine".to_string(), rules_fired: 42, ... };
    let _ = metrics_tx.send(state);
    // ... read frame ...
}
```
The existing helper broadcasts a hardcoded snapshot on `metrics_tx`. The new `broadcasts_snapshot_after_execute` test should instead: (1) complete the WS handshake (so the session subscribes); (2) POST `/v1/execute` via an HTTP client to the same server (`reqwest` or `actix_test::TestServer`'s client); (3) read the WS frame; (4) assert the frame's `rules_fired > 0` and `engine_name == "rete"`. This proves `execute_single` broadcasts a populated snapshot end-to-end. Reuse `read_one_ws_frame` (`:117-155`) as-is. A new helper `handshake_then_post_execute_and_read_frame` is the cleanest split — or parameterize the existing helper with an enum `{BroadcastManual, PostExecute}`.

**Existing `MetricsSnapshot` literal for reference** (`api/tests/metrics_ws.rs:94-101`):
```rust
let state = MetricsSnapshot {
    engine_name: "test-engine".to_string(),
    rules_fired: 42,
    facts_inserted: 7,
    active_sessions: 1,
    memory_bytes: 1024,
    last_updated: "2026-07-02T00:00:00Z".to_string(),
};
```
This is the exact struct shape `execute_single` must construct (RESEARCH D3 step 5).

**Imports at top of file** (`api/tests/metrics_ws.rs:17-24`): add `use api::metrics::ExecutionCounters;`, `use api::v1::execution::execute_rules;`, `use engine::engine::registry::EngineRegistry;`, `use engine::engine::selector::EngineSelector;`, `use engine::rules::{Action, ComparisonOp, Condition, Expression, Rule, RuleSet};`, `use engine::data_types::Value as EngineValue;`, `use std::collections::HashMap;`, `use std::sync::{Arc, RwLock};` — mirror `execute_integration.rs:29-41`.

---

## Shared Patterns

### `web::Data` injection (actix)
**Source:** `api/src/main.rs:200-216`, `api/tests/execute_integration.rs:140-151`, `api/tests/metrics_ws.rs:46-50`
**Apply to:** `api/src/main.rs` (add counters app_data), all three test files (inject counters + metrics_tx in every `App::new()` that calls a handler needing them).
```rust
// Production (main.rs HttpServer::new closure):
.app_data(web::Data::new(metrics_tx.clone()))
.app_data(counters.clone())   // counters: web::Data<ExecutionCounters> built once before the closure

// Tests:
.app_data(web::Data::new(ExecutionCounters::default()))   // or shared_counters()
.app_data(web::Data::new(metrics_tx_for_test.clone()))
```
**Critical:** actix extracts `web::Data<T>` by type `T`. If a handler signature includes `counters: web::Data<ExecutionCounters>` but the `App` does not have `ExecutionCounters` in `app_data`, the request returns 500 (extraction failure), NOT a compile error. Every test calling `/metrics` or `/v1/execute` must inject both new types.

### Broadcast channel send + discard error
**Source:** `api/tests/metrics_ws.rs:105` (`let _ = metrics_tx.send(state);`), `api/src/websocket.rs:47` (subscribe)
**Apply to:** `api/src/v1/execution.rs` (`execute_single` post-block broadcast).
```rust
let _ = metrics_tx.send(snapshot);  // Err(SendError) when no subscribers — no-op, NOT a panic
```
The channel is capacity-16 (`main.rs:123`); `send` is a no-op when no WS session is subscribed. Do NOT unwrap. Do NOT retry.

### Error handling (HTTP error responses in handlers)
**Source:** `api/src/v1/execution.rs:348-367`, `:419-423`
**Apply to:** any new error branch in `execute_single` (none expected — counter increment is infallible; `metrics_tx.send` error is discarded).
```rust
return Err(HttpResponse::BadRequest().json(serde_json::json!({
    "error": format!("unknown engine: {}", engine_name)
})))
```
Counter `fetch_add`/`inc`/`observe` and `TextEncoder::encode` are infallible (the encoder `unwrap` is safe — UTF-8 from prometheus primitives). No new error branches needed in `execute_single`.

### Test isolation (`serial_test`)
**Source:** `api/tests/execute_integration.rs:155-162`, `:172`, `:199`
**Apply to:** all new Phase 24 tests in `execute_integration.rs` and `metrics_integration.rs` that touch the `CoverageTracking` singleton or `RRULES_DEFAULT_ENGINE` env var.
```rust
#[serial_test::serial]
#[actix_web::test]
async fn counters_increment_per_request() {
    reset_test_env();
    let app = init_test_app!();
    // ...
}
```
`serial_test` is already a dev-dep (`api/Cargo.toml:59`). Tests that only assert `/metrics` text format and don't POST `/v1/execute` don't strictly need serial, but marking them serial is harmless and consistent.

### Imports pattern (api crate internal)
**Source:** `api/src/v1/execution.rs:1-13`
**Apply to:** `api/src/v1/execution.rs` (add metrics imports), `api/src/metrics.rs` (add prometheus + atomic imports).
```rust
// In execution.rs, add to the existing use block:
use crate::metrics::{ExecutionCounters, MetricsTx, MetricsSnapshot};
use std::sync::atomic::Ordering;

// In metrics.rs, add:
use std::sync::atomic::{AtomicU64, Ordering};
use prometheus::{Encoder, IntCounterVec, HistogramVec, Opts, Registry, TextEncoder};
```
Note: `api` is a binary crate (`api/Cargo.toml:6-8` `[[bin]] name = "rRules-server"`), so `crate::metrics` resolves to `api/src/metrics.rs`. The test files import via `use api::metrics::...` (the `api` crate name).

---

## No Analog Found

None. Every modified file has an in-tree analog (most are exact same-file extensions — Phase 24 extends existing files rather than creating new ones). The `ExecutionCounters` struct is new but its constituent primitives (`IntCounterVec`/`HistogramVec`/`TextEncoder`/`Registry`) all have in-tree reference usage in `engine/src/metrics/prometheus.rs` and `engine/src/engine/metrics.rs`.

---

## Metadata

**Analog search scope:**
- `api/src/metrics.rs` (full read, 272 lines)
- `api/src/v1/execution.rs` (targeted reads: lines 1-120, 320-590)
- `api/src/main.rs` (targeted read: lines 80-310)
- `api/src/websocket.rs` (full read, 111 lines)
- `engine/src/metrics/prometheus.rs` (full read, 112 lines)
- `engine/src/engine/metrics.rs` (targeted read: lines 280-320)
- `api/Cargo.toml` (full read, 60 lines)
- `engine/Cargo.toml` (lines 1-20)
- `Cargo.toml` (workspace, full read, 16 lines)
- `api/tests/execute_integration.rs` (full read, 334 lines)
- `api/tests/metrics_integration.rs` (full read, 139 lines)
- `api/tests/metrics_ws.rs` (full read, 196 lines)

**Files scanned:** 12
**Pattern extraction date:** 2026-07-03
**Confidence:** HIGH — every analog is an exact same-file extension or a same-crate sibling; all signatures and struct shapes verified by direct read with file:line citations.