---
phase: 24-execution-counters-metrics-broadcast
plan: 02
subsystem: api-metrics-foundation
tags: [metrics, prometheus, foundation, wave-1]
requires:
  - 24-01
provides:
  - api::metrics::ExecutionCounters struct with dedicated Registry
  - ExecuteResponse.engine_generation field (default 0)
  - main.rs web::Data wiring for ExecutionCounters
affects:
  - api/Cargo.toml
  - api/src/metrics.rs
  - api/src/v1/execution.rs
  - api/src/main.rs
tech-stack:
  added:
    - prometheus = { workspace = true } in api/Cargo.toml (inherits workspace 0.13 pin)
  patterns:
    - dedicated prometheus::Registry::new() per crate (Pitfall 24-A isolation)
    - Opts::new(name, help) for IntCounterVec (NOT the broken struct-literal Opts form)
    - exponential_buckets(0.001, 2.0, 12) for request-duration histogram (1ms..~4s)
    - #[serde(default)] on a newly-added response field for backward-compatible schema evolution
key-files:
  created: []
  modified:
    - api/Cargo.toml
    - api/src/metrics.rs
    - api/src/v1/execution.rs
    - api/src/main.rs
decisions:
  - "Used dedicated Registry::new() instead of default_registry() to isolate api-layer metric names from the engine crate (Pitfall 24-A)"
  - "Built counters via Opts::new(name, help) — NOT the broken struct-literal Opts { ... } form at engine/src/engine/metrics.rs:288"
  - "Histogram buckets fixed at exponential_buckets(0.001, 2.0, 12) per Open Q1 (1ms, 2ms, 4ms ... ~4s)"
  - "engine_generation typed as u64 with #[serde(default)]; always 0 in Phase 24 (Phase 25 wires the real counter)"
  - "Deferred Encoder/TextEncoder imports to Plan 24-03 to keep this plan's diff additive-only and avoid unused-import warnings"
metrics:
  duration: ~25m
  completed: 2026-07-03T08:47:56Z
  tasks: 2
  files: 4
status: complete
---

# Phase 24 Plan 02: Execution Counters Foundation (Wave 1) Summary

ExecutionCounters scaffolding with a dedicated Prometheus registry, the prometheus dep promoted into api/Cargo.toml, an engine_generation field on ExecuteResponse (default 0), and main.rs web::Data wiring — all inert, no behavioral change.

## What Was Built

### Task 1: ExecutionCounters struct + prometheus dep + engine_generation field

**api/Cargo.toml** — Added `prometheus = { workspace = true }` to `[dependencies]` after the `jsonwebtoken` line. Inherits the workspace `0.13` pin (Cargo.toml:15, already vetted by the engine crate). NOT `optional` (the api crate always uses it). No new supply-chain surface (T-24-03-W1).

**api/src/metrics.rs** — Added `pub struct ExecutionCounters` with the shape required by the plan's `must_haves`:
- `requests_total: IntCounterVec` (label `engine`, name `rrules_execute_requests_total`)
- `rules_fired_total: IntCounterVec` (label `engine`, name `rrules_execute_rules_fired_total`)
- `duration_seconds: HistogramVec` (label `engine`, name `rrules_execute_duration_seconds`, buckets `exponential_buckets(0.001, 2.0, 12)`)
- `snapshot_requests: AtomicU64`, `snapshot_rules_fired: AtomicU64`, `snapshot_facts_inserted: AtomicU64` (mirrors initialized to 0)
- `registry: Registry` — a **dedicated** `prometheus::Registry::new()` (NOT `default_registry()` — Pitfall 24-A)

The constructor `pub fn new() -> Self` builds the two counters via `prometheus::Opts::new(name, help)` (NOT the broken struct-literal `Opts { ... }` form at `engine/src/engine/metrics.rs:288`), registers each on the dedicated registry (clone-before-register so the struct field retains ownership), and builds the histogram via `HistogramOpts::new(...).buckets(exponential_buckets(0.001, 2.0, 12))`. `impl Default for ExecutionCounters` delegates to `new()` so test fixtures can `web::Data::new(ExecutionCounters::default())` (Open Q7).

Imports added: `std::sync::atomic::AtomicU64`, `prometheus::{HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry}`. `Encoder`/`TextEncoder` deferred to Plan 24-03 (the metrics_endpoint rewrite) to keep this plan additive-only and avoid unused-import warnings.

**api/src/v1/execution.rs** — Added `#[serde(default)] pub engine_generation: u64` to `ExecuteResponse` after `inference_trace`. Updated both construction sites:
- `execute_single` success path (~line 464): `engine_generation: 0u64,`
- `batch_execute_rules` zero-fired placeholder (~line 546): `engine_generation: 0u64,`

`#[serde(default)]` keeps existing clients and the Phase 23 test fixtures parsing correctly (Pitfall 8). No handler signature changed — `execute_single`, `execute_rules`, `batch_execute_rules`, and `metrics_endpoint` keep their current signatures (Plan 24-03 changes them).

### Task 2: main.rs web::Data wiring

**api/src/main.rs** — Constructed `let counters = web::Data::new(metrics::ExecutionCounters::new());` at startup immediately after the `metrics_tx` channel creation (line 123), mirroring the existing `registry`/`selector`/`ruleset` `web::Data::new(...)` pattern. Added `.app_data(counters.clone())` in the `HttpServer::new` closure immediately after the existing `.app_data(web::Data::new(metrics_tx.clone()))` line. The existing `metrics_tx` line is unchanged (no-regression). The `counters` variable is captured by the `move` closure and cloned per worker (same as `metrics_tx`, `registry`, `selector`, `ruleset`).

Used `metrics::ExecutionCounters::new()` (the `metrics::` path) to match the existing `metrics::create_metrics_channel` / `metrics::configure_metrics` style — no separate `use` import needed.

The counters are **inert** in Plan 24-02: no handler reads or increments them. Plan 24-03 wires `execute_single` and `metrics_endpoint` to these counters, turning the Wave 0 RED tests GREEN.

## Verification

### Build gate
- `cargo check -p api` exits **0** (only expected `dead_code` warnings on the unread `ExecutionCounters` fields — they will be consumed by Plan 24-03).

### Test gate (cargo test -p api --tests --no-fail-fast)

| Test file | Passed | Failed | Failed tests |
|-----------|--------|--------|--------------|
| (lib unit tests) | 27 | 0 | — |
| (bin unit tests) | 27 | 0 | — |
| auth_endpoints | 7 | 0 | — |
| auth_protection | 7 | 0 | — |
| batch_execution_integration | 5 | 0 | — |
| branches | 4 | 0 | — |
| execute_integration | 6 | 2 | `counters_exposed_in_metrics`, `counters_increment_per_request` |
| metrics_integration | 3 | 1 | `metrics_prometheus_text_format` |
| metrics_ws | 3 | 1 | `broadcasts_snapshot_after_execute` |
| simulation_integration | 3 | 0 | — |

**Final RED set: 4 tests** (down from the Wave 0 baseline of 5):
- `counters_exposed_in_metrics` (execute_integration) — RED: counters exist but `/metrics` does not yet render them (metrics_endpoint signature unchanged)
- `counters_increment_per_request` (execute_integration) — RED: execute_single does not yet increment the counters
- `metrics_prometheus_text_format` (metrics_integration) — RED: /metrics still returns hardcoded text
- `broadcasts_snapshot_after_execute` (metrics_ws) — RED: execute_single does not yet broadcast a snapshot

**`response_includes_engine_generation` flipped GREEN** — as predicted by the plan's `<acceptance_criteria>` and `<verification>` sections. The Wave 0 test asserts `body["engine_generation"].is_u64()`; once the field exists with `#[serde(default)]` and both construction sites set `0u64`, the assertion passes. This is the expected, documented behavior for Plan 24-02 and is NOT a regression — the plan's `must_haves` truths and `<done>` criteria both call it out explicitly.

No existing GREEN test regressed. No new test was added or modified (foundation plan — no test files touched, per the plan's `files_modified`).

## Deviations from Plan

None — plan executed exactly as written. The single expected behavior shift (`response_includes_engine_generation` GREEN) is documented in the plan's own acceptance criteria and verification section, not a deviation.

## Threat Flags

None. The threat register in the plan's `<threat_model>` covers the three relevant threats (T-24-02-W1 tampering on monotonic counters, T-24-03-W1 supply chain for the promoted workspace dep, T-24-04-W1 information disclosure on engine_generation) — all disposition `accept` at low severity, all validated by the implementation:
- Counters are monotonic (no reset/decrement path in Phase 24); web::Data is shared-immutable Arc.
- `prometheus = { workspace = true }` inherits the already-vetted 0.13 pin; no new package introduced.
- `engine_generation` is always 0 in Phase 24 (no per-tenant/request correlation data).

## Known Stubs

None. The `ExecutionCounters` struct is fully constructed (not stubbed) — its fields are simply unread until Plan 24-03 wires the handlers. This is intentional foundation scaffolding, not a stub.

## Self-Check: PASSED

- `api/Cargo.toml` contains `prometheus = { workspace = true }` — FOUND
- `api/src/metrics.rs` contains `pub struct ExecutionCounters`, `fn new() -> Self`, `impl Default for ExecutionCounters`, `Registry::new()`, all three metric names, `exponential_buckets(0.001, 2.0, 12)` — FOUND
- `api/src/v1/execution.rs` contains `engine_generation` at the struct field + both construction sites with `#[serde(default)]` on the field — FOUND
- `api/src/main.rs` contains `ExecutionCounters::new()` construction + `.app_data(counters.clone())` injection + unchanged `metrics_tx` line — FOUND
- Commit `7dfe758b8` (Task 1) exists in git log — FOUND
- Commit `3d66f37a1` (Task 2) exists in git log — FOUND
- `cargo check -p api` exits 0 — CONFIRMED
- RED set is exactly the 4 tests predicted by the plan; `response_includes_engine_generation` is GREEN as predicted — CONFIRMED