---
phase: 24
slug: execution-counters-metrics-broadcast
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-03
---

# Phase 24 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from the `## Validation Architecture` section of `24-RESEARCH.md`.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` via `cargo test`; `actix-test` 0.1 for HTTP/WS integration (`api/Cargo.toml:54`); `serial_test` (workspace) for singleton isolation |
| **Config file** | `api/Cargo.toml` `[dev-dependencies]`; per-test files in `api/tests/` |
| **Quick run command** | `cargo check -p api && cargo test -p api --test execute_integration` |
| **Full suite command** | `cargo test -p api --tests && cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` |
| **Estimated runtime** | ~180 seconds (full api + engine regression) |

---

## Sampling Rate

- **After every task commit:** Run `cargo check -p api && cargo test -p api --test execute_integration`
- **After every plan wave:** Run `cargo test -p api --tests && cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 180 seconds

> **Nyquist dimension:** amplitude at the endpoint — observe the metric value before and after the execute event. The sampling rate is "once per execute" (the counter is the sampler); a regression is any post-execute scrape that fails to show the increment.

---

## Per-Task Verification Map

> Success-criterion-level observation points (from `24-RESEARCH.md`). Task-level rows (24-NN-MM) are added by the planner as PLAN.md tasks are defined; every task MUST map to one of these observation points or to a no-regression check.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| SC-1 / MON-07 | TBD | TBD | MON-07 | T-24-01 / — | `GET /metrics` (Accept: text/plain) exposes `rrules_execute_requests_total{engine="rete"}` `>= 1`, `rrules_execute_rules_fired_total{engine="rete"}` `>= 1`, and `rrules_execute_duration_seconds_bucket{engine="rete",...}` populated | integration | `cargo test -p api --test execute_integration -- counters_exposed_in_metrics -x` | ❌ W0 | ⬜ pending |
| SC-2 / MON-07 | TBD | TBD | MON-07 | T-24-02 / — | WebSocket monitoring channel emits a `MetricsSnapshot` reflecting the most-recent execute (non-zero `rules_fired` after a run) | integration (WS) | `cargo test -p api --test metrics_ws -- broadcasts_snapshot_after_execute -x` | ❌ W0 | ⬜ pending |
| SC-3 / MON-07 | TBD | TBD | MON-07 | — | Existing frontend Engine/Coverage/DeadRule dashboards render live numbers after a run with **no frontend code changes** (structural proof — see Manual-Only) | manual + structural | n/a (structural proof + manual observation) | n/a | ⬜ pending |
| counter-correctness | TBD | TBD | MON-07 | — | Counters increment exactly once per execute; batch of N increments N times | integration | `cargo test -p api --test execute_integration -- counters_increment_per_request -x` | ❌ W0 | ⬜ pending |
| response-shape | TBD | TBD | MON-07 | — | `ExecuteResponse` includes `engine_generation: u64` (default 0); `execution_id` still present | integration | `cargo test -p api --test execute_integration -- response_includes_engine_generation -x` | ❌ W0 | ⬜ pending |
| no-regression (exec) | TBD | TBD | MON-07 | — | Phase 23's five execute_integration tests still pass after the handler signature change | integration | `cargo test -p api --test execute_integration` | ✅ exists — must stay GREEN | ⬜ pending |
| no-regression (metrics) | TBD | TBD | MON-07 | — | `metrics_prometheus_text_format` still passes (or is updated — Pitfall 24-D / Open Q2) | integration | `cargo test -p api --test metrics_integration -- metrics_prometheus_text_format` | ✅ exists — may need update | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `api/tests/execute_integration.rs` — add `counters_exposed_in_metrics`, `counters_increment_per_request`, `response_includes_engine_generation` (extend the existing fixture/injection pattern from Plan 23-04)
- [ ] `api/tests/metrics_ws.rs` — add `broadcasts_snapshot_after_execute` (extend the existing `spawn_server` + `handshake_then_broadcast_and_read_frame` helpers; the test POSTs `/v1/execute` and asserts the WS frame's `rules_fired > 0`). Requires the test service to wire `execute_rules` AND `configure_metrics_ws` AND the `metrics_tx`/`counters` app_data together.
- [ ] `api/tests/metrics_integration.rs` — update `metrics_prometheus_text_format` if the `engine_rules_fired` substring is removed (Pitfall 24-D / Open Q2)

*Existing `api/tests/execute_integration.rs` (Phase 23's five tests) and `api/tests/metrics_ws.rs` harness already cover the surrounding infrastructure; Wave 0 extends them, does not rebuild them.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| SC-3 dashboards render live (non-hardcoded) numbers after a run | MON-07 | Structural proof covers the contract (no frontend change needed); the live "open the dashboard" observation is a single verify-phase check, not a Wave 0 automated test | (1) Structural proof: `grep` confirms `frontend/src/hooks/useEngineMetrics.ts` `EngineState` interface fields ⊆ `MetricsSnapshot` struct fields — `engine_name`/`rules_fired`/`facts_inserted`/`active_sessions`/`memory_bytes`/`last_updated` (already verified at `useEngineMetrics.ts:4-11` and `api/src/metrics.rs:16-23`; WS `onmessage` reads them by name at `useEngineMetrics.ts:135-150`). (2) Manual: run the server, `POST /v1/execute`, open the dashboard, observe non-zero numbers. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 180s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending