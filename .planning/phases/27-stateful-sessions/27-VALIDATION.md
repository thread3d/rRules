---
phase: 27
slug: stateful-sessions
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-04
---

# Phase 27 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `actix_web::test` + `serial_test` (workspace dev-dep, established Phase 23/26) |
| **Config file** | none — test wiring lives in `api/tests/execute_integration.rs` (`init_test_app!` / `init_two_rule_app!` macros; extended with `shared_session_manager()` app_data for Phase 27) |
| **Quick run command** | `cargo test -p api --test execute_integration` |
| **Full suite command** | `cargo test -p api && cargo test -p engine` |
| **Estimated runtime** | ~25 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p api --test execute_integration`
- **After every plan wave:** Run `cargo test -p api && cargo test -p engine`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 25 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 27-01-01 | 01 | 1 | SESS-01 / SC-1 | — | N/A — stateless path unchanged | integration | `cargo test -p api --test execute_integration -- stateful_session_accumulates_wm --exact` | ❌ W0 | ⬜ pending |
| 27-01-02 | 01 | 1 | SESS-01 / SC-2 | T-27-02 | Idle/max-lifetime TTL bounds sessions (Pitfall 5 OOM) | integration | `cargo test -p api --test execute_integration -- session_ttl_evicts_idle --exact` | ❌ W0 | ⬜ pending |
| 27-01-03 | 01 | 1 | SESS-01 / SC-3 | T-27-01 | `MAX_SESSIONS` LRU cap bounds count; gauge reflects capped count | integration | `cargo test -p api --test execute_integration -- lru_evicts_oldest_and_gauge_capped --exact` | ❌ W0 | ⬜ pending |
| 27-01-04 | 01 | 1 | SESS-01 / SC-4 | — | DELETE disposes; reset clears WM without disposing | integration | `cargo test -p api --test execute_integration -- session_lifecycle_delete_reset --exact` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `api/tests/execute_integration.rs` — RED stubs for SC-1/SC-2/SC-3/SC-4 (extend `init_test_app!` with `shared_session_manager()` app_data; add `configure_test_routes` for DELETE/reset)
- [ ] `api/src/v1/sessions.rs` — NEW module (`SessionManager`, `StatefulSessionEntry`, `run_session_sweep`, `dispose_session`, `reset_session`, `read_session_config`)
- [ ] `api/Cargo.toml` — add `dashmap = "6.2.1"` to `[dependencies]`
- [ ] `api/src/main.rs` — construct `SessionManager`, spawn sweep task, add `app_data`, register `DELETE /v1/execute/session/{id}` + `POST /v1/execute/session/{id}/reset` routes
- [ ] `api/src/v1/execution.rs` — add `ExecuteResponse.session_id: Option<String>`, branch `execute_single` on `req.session_id`, populate `active_sessions` from `SessionManager::len()`, remove/repurpose `execute_with_session` stub

*Existing `serial_test` + `clear()` infrastructure covers registry isolation.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `/metrics` exposes `rrules_sessions_active` text line after sweep | SESS-01 / SC-3 (gauge visibility) | Prometheus text format assertion is brittle in-process; the gauge value is asserted via `MetricsSnapshot.active_sessions` in the integration tests, but the dashboard surface is a manual spot-check | `curl /metrics | grep rrules_sessions_active` after creating/evicting sessions |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (the four SC tests)
- [ ] No watch-mode flags
- [ ] Feedback latency < 25s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending