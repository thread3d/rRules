---
phase: 24-execution-counters-metrics-broadcast
plan: 03
subsystem: api
tags: [metrics, prometheus, websocket, instrumentation, monitoring]
requires:
  - 24-02 (ExecutionCounters struct, Default impl, web::Data wiring in main.rs)
  - 24-01 (Wave 0 RED test scaffolds, MetricsTx test wiring)
provides:
  - Live /metrics Prometheus exposition via TextEncoder (SC-1)
  - Live WS MetricsSnapshot broadcast after each execute (SC-2)
  - Instrumented execute_single with timing, counter increment, broadcast
affects:
  - api/src/v1/execution.rs (execute_single, execute_rules, batch_execute_rules signatures)
  - api/src/metrics.rs (metrics_endpoint, ExecutionCounters::new)
  - api/tests/execute_integration.rs (init_test_app! macro, shared_counters)
  - api/tests/metrics_integration.rs (App::new chains, ExecutionCounters injection)
  - api/tests/metrics_ws.rs (spawn_server_with_execute, ExecutionCounters injection)
  - api/tests/batch_execution_integration.rs (init_test_app! macro, counters + metrics_tx)
tech-stack:
  added: []
  patterns:
    - prometheus::TextEncoder::new().encode(&registry.gather(), &mut buffer) for Prometheus exposition
    - std::time::Instant::now() / elapsed() for web::block timing (Don't Hand-Roll)
    - let _ = metrics_tx.send(snapshot) for broadcast (Pitfall 24-C — discard SendError)
    - Counter increment AFTER web::block returns, in async context (Pitfall 24-B)
    - Pre-initialize label combinations in counter constructor to avoid gather() pruning
key-files:
  created: []
  modified:
    - api/src/v1/execution.rs
    - api/src/metrics.rs
    - api/tests/execute_integration.rs
    - api/tests/metrics_integration.rs
    - api/tests/metrics_ws.rs
    - api/tests/batch_execution_integration.rs
decisions:
  - "Pre-initialize the 'rete' label combination in ExecutionCounters::new() so Registry::gather() does not prune empty MetricFamilies on a cold /metrics scrape (Rule 1 bug fix — prometheus 0.13 registry.rs:126 prunes families with empty Metric lists)"
  - "active_sessions=0, memory_bytes=0 in MetricsSnapshot (Open Q3 — Phase 27 populates active_sessions)"
  - "engine_name stays 'default' in JSON/HTML /metrics arms; per-execute engine_name goes through WS broadcast only"
metrics:
  duration: 6m 28s
  completed: 2026-07-03T08:59:14Z
  tasks: 3
  files: 6
status: complete
---

# Phase 24 Plan 03: Wave 2 Instrumentation Summary

Wired the inert ExecutionCounters (built in 24-02) into the live execute path and /metrics endpoint, turning all 4 Wave 0 RED tests GREEN via prometheus::TextEncoder exposition, Instant-based timing, post-web::block counter increment, and MetricsSnapshot broadcast.

## What Was Built

### Task 1: Instrument execute_single (commit 0c0f061b2)
- Extended `execute_single`, `execute_rules`, and `batch_execute_rules` signatures with `counters: web::Data<ExecutionCounters>` and `metrics_tx: web::Data<MetricsTx>` params
- Captured `Instant::now()` before `web::block`; after it returns (in the async context — Pitfall 24-B / Open Q5), incremented the three labeled Prometheus counters (`requests_total`, `rules_fired_total`, `duration_seconds`) and three AtomicU64 mirrors (`snapshot_requests`, `snapshot_rules_fired`, `snapshot_facts_inserted`)
- Built a populated `MetricsSnapshot` (engine_name from the resolved engine, rules_fired/facts_inserted from mirrors, active_sessions=0, memory_bytes=0 per Open Q3) and broadcast via `let _ = metrics_tx.send(snapshot)` (Pitfall 24-C — send error discarded)
- `batch_execute_rules` clones `counters` + `metrics_tx` web::Data into each JoinSet task alongside the existing registry/selector/ruleset clones (PATTERNS.md batch pattern — N items = N increments + N broadcasts)
- Updated `batch_execution_integration.rs` test App to inject `ExecutionCounters` + `MetricsTx` app_data (Rule 3 — signature change would block extraction otherwise)

### Task 2: Rewrite metrics_endpoint (commit 6aaf0f798)
- Added `counters: web::Data<ExecutionCounters>` param to `metrics_endpoint`
- Replaced the hardcoded text/plain arm with `prometheus::TextEncoder::new().encode(&counters.registry.gather(), &mut buffer)` (Don't Hand-Roll — TextEncoder, not manual format!; reads from the dedicated `counters.registry`, NOT `prometheus::default_registry()` — Pitfall 24-A)
- JSON arm reads live `counters.snapshot_rules_fired.load(Ordering::Relaxed)` and `counters.snapshot_facts_inserted.load(Ordering::Relaxed)` instead of hardcoded 0
- HTML arm reads live `counters.snapshot_rules_fired.load(Ordering::Relaxed)`
- Removed hardcoded `engine_rules_fired 0` / `engine_memory_bytes 0` lines (Pitfall 24-D)
- `active_sessions` and `memory_bytes` stay 0 in JSON/HTML arms (Open Q3)

### Task 3: Update test App wiring (commit 5a095c7c1)
- `execute_integration.rs`: added `shared_counters()` helper, injected `.app_data(shared_counters())` into `init_test_app!` macro
- `metrics_integration.rs`: injected `.app_data(ExecutionCounters::default())` into all four test Apps
- `metrics_ws.rs`: injected `.app_data(ExecutionCounters::default())` into `spawn_server_with_execute`
- [Rule 1 - Bug] Pre-initialized the "rete" label combination on each counter in `ExecutionCounters::new()` so `Registry::gather()` does not prune empty MetricFamilies on a cold /metrics scrape (prometheus 0.13 registry.rs:126 prunes families with empty `Metric` lists; without this, `metrics_prometheus_text_format` fails with an empty body because no POST /v1/execute has created the label combination yet)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Pre-initialize "rete" label combination in ExecutionCounters::new()**
- **Found during:** Task 3 (metrics_prometheus_text_format test still RED after Task 2)
- **Issue:** `prometheus::Registry::gather()` (prometheus 0.13, registry.rs:126) prunes MetricFamilies whose `Metric` list is empty. When no `with_label_values` call has created a label combination, the IntCounterVec/HistogramVec `collect()` returns a MetricFamily with an empty metric list, which `gather()` skips. The TextEncoder then receives an empty `Vec<MetricFamily>` and produces an empty string. The `metrics_prometheus_text_format` test scrapes /metrics WITHOUT first calling POST /v1/execute, so the output was empty and the test failed.
- **Fix:** Added `requests_total.with_label_values(&["rete"]); rules_fired_total.with_label_values(&["rete"]); duration_seconds.with_label_values(&["rete"]);` at the end of `ExecutionCounters::new()`, after registration. This creates the "rete" label combination at construction time so the metric families are always present in `gather()` output. "rete" is the default engine; other engine labels (treat/leaps/phreak) are created lazily on first `with_label_values` call in `execute_single`.
- **Files modified:** api/src/metrics.rs
- **Commit:** 5a095c7c1

**2. [Rule 3 - Blocking] Update batch_execution_integration.rs test App**
- **Found during:** Task 1 (signature change blocks extraction)
- **Issue:** `execute_rules` and `batch_execute_rules` signature change (adding `counters` + `metrics_tx` params) means the `batch_execution_integration.rs` test App — which calls both handlers — would fail extraction with 500 errors without the new app_data.
- **Fix:** Added `shared_counters()` and `shared_metrics_tx()` helpers and injected them into the `init_test_app!` macro, mirroring `execute_integration.rs`.
- **Files modified:** api/tests/batch_execution_integration.rs
- **Commit:** 0c0f061b2

## Test Gate Results

### Final GREEN gate: `cargo test -p api --tests --no-fail-fast`

| Test binary | Tests passed | Failed | Ignored |
|-------------|-------------|--------|---------|
| (lib tests) | 27 | 0 | 0 |
| (bin tests) | 27 | 0 | 0 |
| approval_integration | 7 | 0 | 0 |
| auth_integration | 7 | 0 | 0 |
| batch_execution_integration | 5 | 0 | 0 |
| branches | 4 | 0 | 0 |
| execute_integration | 8 | 0 | 0 |
| metrics_integration | 4 | 0 | 0 |
| metrics_ws | 4 | 0 | 0 |
| simulation_integration | 3 | 0 | 0 |
| **Total** | **96** | **0** | **0** |

### Wave 0 RED tests now GREEN (Nyquist closure)

| Test | Binary | Status |
|------|--------|--------|
| counters_exposed_in_metrics | execute_integration | GREEN (SC-1) |
| counters_increment_per_request | execute_integration | GREEN (SC-1) |
| response_includes_engine_generation | execute_integration | GREEN (from 24-02) |
| metrics_prometheus_text_format | metrics_integration | GREEN (SC-1) |
| broadcasts_snapshot_after_execute | metrics_ws | GREEN (SC-2) |

### Engine regression: `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak`

| Suite | Tests passed | Failed | Ignored |
|-------|-------------|--------|---------|
| lib tests | 218 | 0 | 0 |
| integration | 1 | 0 | 0 |
| (other) | 3 | 0 | 0 |
| (other) | 7 | 0 | 0 |
| (other) | 10 | 0 | 0 |
| (other) | 2 | 0 | 3 |
| **Total** | **241** | **0** | **3** |

No engine-side regression.

## ROADMAP Success Criteria

- **SC-1**: GET /metrics exposes `rrules_execute_requests_total{engine="rete"}`, `rrules_execute_rules_fired_total{engine="rete"}`, and `rrules_execute_duration_seconds_bucket{engine="rete",...}` after a run. Proven by `counters_exposed_in_metrics` + `counters_increment_per_request` + `metrics_prometheus_text_format` all GREEN.
- **SC-2**: WS monitoring channel emits a populated MetricsSnapshot reflecting the most-recent execute (engine_name="rete", rules_fired > 0). Proven by `broadcasts_snapshot_after_execute` GREEN.
- **SC-3**: Frontend dashboards render live numbers with zero frontend changes. Structural proof: `frontend/src/hooks/useEngineMetrics.ts:4-11` `EngineState` interface fields `{engine_name, rules_fired, facts_inserted, active_sessions, memory_bytes, last_updated}` exactly match `api/src/metrics.rs:18-25` `MetricsSnapshot` struct fields. No frontend file modified in this phase.

## Open Questions Resolved

- Q1: exponential_buckets(0.001, 2.0, 12) — applied in 24-02
- Q2: option b assertion update — N/A (assertions unchanged; tests pass as-is)
- Q3: active_sessions=0, memory_bytes=0 in MetricsSnapshot — Phase 27 populates active_sessions
- Q4: dedicated Registry::new() — applied in 24-02
- Q5: increment after web::block returns (not inside closure) — applied in Task 1
- Q6: engine_generation=0 in Phase 24 — confirmed in ExecuteResponse
- Q7: test app_data injection — applied in Task 3

## Self-Check: PASSED

- FOUND: api/src/v1/execution.rs
- FOUND: api/src/metrics.rs
- FOUND: api/tests/execute_integration.rs
- FOUND: api/tests/metrics_integration.rs
- FOUND: api/tests/metrics_ws.rs
- FOUND: api/tests/batch_execution_integration.rs
- FOUND: commit 0c0f061b2
- FOUND: commit 6aaf0f798
- FOUND: commit 5a095c7c1