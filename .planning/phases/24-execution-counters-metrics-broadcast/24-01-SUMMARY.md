---
phase: 24-execution-counters-metrics-broadcast
plan: 01
subsystem: api-tests
tags: [tdd-red, nyquist, prometheus, websocket, metrics, phase-24]
requires:
  - "Phase 23 execute_rules wired through real engine (5 GREEN integration tests)"
  - "api::metrics::{configure_metrics, create_metrics_channel, MetricsTx, MetricsSnapshot}"
  - "api::websocket::configure_metrics_ws (Phase 22 WS endpoint)"
provides:
  - "5 RED test scaffolds encoding the Phase 24 success contract (SC-1 Prometheus counters, SC-2 WS broadcast, counter-correctness, response-shape, Pitfall 24-D)"
  - "init_test_app! macro extended with configure_metrics at root + shared_metrics_tx() (forward-compatible with Plan 24-03)"
  - "spawn_server_with_execute() helper wiring execute + WS on one test server"
  - "raw_post_execute() helper (raw HTTP/1.1 POST, no new dev-dep)"
affects:
  - "api/tests/execute_integration.rs"
  - "api/tests/metrics_ws.rs"
  - "api/tests/metrics_integration.rs"
tech-stack:
  added: []
  patterns:
    - "Nyquist RED-first TDD — tests compile against existing API and fail on substantive assertions"
    - "Raw HTTP/1.1 over TcpStream for WS handshake + POST (no awc/reqwest dev-dep)"
    - "serial_test::serial isolation for tests touching global CoverageTracking singleton"
key-files:
  created: []
  modified:
    - api/tests/execute_integration.rs
    - api/tests/metrics_ws.rs
    - api/tests/metrics_integration.rs
decisions:
  - "D1: Used raw HTTP/1.1 POST over a second TcpStream (raw_post_execute) instead of awc::Client or reqwest — avoids adding a new dev-dep for the WS broadcast test's POST step"
  - "D2: Added shared_metrics_tx() to init_test_app! now (pre-24-03) — actix silently ignores unused app_data, so RED tests still compile against the Phase 23 API; the macro is forward-compatible for Plan 24-03's MetricsTx extraction"
  - "D3: Inlined score_plus_10_rule/shared_ruleset/shared_registry/shared_selector into metrics_ws.rs rather than sharing a test-support crate — mirrors the existing per-test-file helper pattern"
metrics:
  duration: ~12m
  completed: 2026-07-03
  tasks: 2
  files: 3
status: complete
---

# Phase 24 Plan 01: Wave 0 RED Test Scaffolds Summary

Landed five Nyquist RED-first TDD scaffolds encoding the Phase 24 success contract: SC-1 Prometheus counters, SC-2 WS broadcast, counter-correctness, response-shape `engine_generation`, and the Pitfall 24-D Prometheus metric-name update — all compiling against the existing Phase 23 API and failing on substantive assertions (not compile errors).

## What Was Built

### Task 1 — `api/tests/execute_integration.rs` (3 RED scaffolds + macro extension)

- **`counters_exposed_in_metrics`** (SC-1): POST `/v1/execute` then GET `/metrics` with `Accept: text/plain`; asserts the exposition body contains `rrules_execute_requests_total`. FAILS — current `metrics_endpoint` returns hardcoded text with only `engine_rules_fired`.
- **`counters_increment_per_request`** (counter-correctness): POST `/v1/execute` twice then GET `/metrics`; asserts body contains `rrules_execute_rules_fired_total`. FAILS — metric name absent from hardcoded text.
- **`response_includes_engine_generation`** (response-shape): POST `/v1/execute`; asserts `body["engine_generation"].is_u64()`. FAILS — `ExecuteResponse` has no `engine_generation` field; serde omits it and the value is `Null`.
- **`init_test_app!` macro** extended: added `.app_data(shared_metrics_tx())` and `.configure(configure_metrics)` at root so GET `/metrics` resolves and the macro is forward-compatible with Plan 24-03's `MetricsTx` extraction.
- **`shared_metrics_tx()` helper** added, mirroring `shared_registry`/`shared_selector`/`shared_ruleset` (returns a fresh broadcast channel sender wrapped in `web::Data`).
- All 5 Phase 23 tests stay GREEN (no-regression).

### Task 2 — `api/tests/metrics_ws.rs` + `api/tests/metrics_integration.rs` (SC-2 + Pitfall 24-D)

- **`broadcasts_snapshot_after_execute`** (SC-2): `spawn_server_with_execute()` wires `execute_rules` + `configure_metrics_ws` + `metrics_tx` + shared registry/selector/ruleset on one test server; completes the WS handshake at `/v1/metrics/ws`, POSTs `/v1/execute` via a raw HTTP/1.1 client, then reads one WS frame and asserts `engine_name == "rete"` and `rules_fired > 0`. FAILS (RED) — `execute_single` does not call `metrics_tx.send(...)`, so `read_one_ws_frame` times out at the 3s deadline (`timed out waiting for a WS frame; got 0 bytes`).
- **`spawn_server_with_execute()` helper** + inline `score_plus_10_rule`/`shared_ruleset`/`shared_registry`/`shared_selector` fixtures (mirroring `execute_integration.rs`).
- **`raw_post_execute()` helper** — raw HTTP/1.1 POST over a second `TcpStream` (chosen over `awc::Client`/`reqwest` to avoid adding a dev-dep; see Decision D1).
- **`metrics_prometheus_text_format`** assertion in `metrics_integration.rs` updated from `engine_rules_fired` to `rrules_execute_requests_total` (Pitfall 24-D / Open Q2 option b). FAILS (RED) — hardcoded text lacks the new metric name.
- All 3 existing `metrics_ws.rs` tests stay GREEN; all 3 other `metrics_integration.rs` tests (`metrics_json_engine_state`, `dead_rules_returns_valid_json_array`, `coverage_endpoint_reads_tracked_data`) stay GREEN.

## Verification Results

| Suite | Compile | Existing Tests | New/Updated Tests |
|-------|---------|----------------|-------------------|
| `cargo test -p api --test execute_integration` | OK | 5 PASS | 3 FAIL (RED — assertion panics) |
| `cargo test -p api --test metrics_ws` | OK | 3 PASS | 1 FAIL (RED — WS frame timeout) |
| `cargo test -p api --test metrics_integration` | OK | 3 PASS | 1 FAIL (RED — assertion on rrules_execute_requests_total) |

RED failure modes (all substantive, none compile/extraction):
- `counters_exposed_in_metrics`: `assert text.contains("rrules_execute_requests_total")` — body contains only `engine_rules_fired`.
- `counters_increment_per_request`: `assert text.contains("rrules_execute_rules_fired_total")` — absent.
- `response_includes_engine_generation`: `assert body["engine_generation"].is_u64()` — got `Null`.
- `broadcasts_snapshot_after_execute`: `read_one_ws_frame` deadline panic — no frame forwarded.
- `metrics_prometheus_text_format`: `assert text.contains("rrules_execute_requests_total")` — body contains only `engine_rules_fired`.

## Scope Compliance

- No `api/src/**` production source modified (verified via `git diff --name-only HEAD~2 HEAD -- api/src/` → empty).
- No modifications to STATE.md or ROADMAP.md (orchestrator owns post-wave writes).
- Only the 3 test files listed in the plan's `files_modified` were touched.

## Deviations from Plan

None — plan executed exactly as written. The only implementation choice left open by the plan was the HTTP-client approach for the WS test's POST step (raw HTTP/1.1 vs `awc::Client`); the plan's NOTE explicitly allowed the raw-TCP fallback, and that path was taken (Decision D1).

## Known Stubs

None. These are test-only scaffolds; no production data paths are stubbed.

## Threat Flags

None. The plan's threat register (T-24-01-W0, accept) covers the only test-only boundary; no new trust surface introduced.

## TDD Gate Compliance

This is a `type: execute` plan operating in RED-first mode under the Nyquist Wave 0 convention (per `<phase_context>`). Git log gate check:

1. `test(24-01):` RED commit exists — `bfe07e6f2` (Task 1, 3 RED scaffolds) and `7db038809` (Task 2, SC-2 + Pitfall 24-D RED).
2. No `feat(24-01):` GREEN commit exists — correct, GREEN is Plan 24-03's job (this plan is RED-only by design).

RED gate satisfied: 5 new tests compile and fail on substantive assertions; GREEN deferred to Plan 24-03 per the phase plan.

## Self-Check: PASSED

- `api/tests/execute_integration.rs` — FOUND (3 new test fns + macro extension + shared_metrics_tx helper)
- `api/tests/metrics_ws.rs` — FOUND (broadcasts_snapshot_after_execute + spawn_server_with_execute + raw_post_execute + inline fixtures)
- `api/tests/metrics_integration.rs` — FOUND (assertion updated to rrules_execute_requests_total; engine_rules_fired removed)
- Commit `bfe07e6f2` — FOUND (Task 1)
- Commit `7db038809` — FOUND (Task 2)
- No `api/src/**` modifications — verified empty