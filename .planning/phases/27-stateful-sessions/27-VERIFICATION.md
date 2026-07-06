---
phase: 27-stateful-sessions
verified: 2026-07-06T11:05:00Z
status: passed
score: 12/12 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification:
  previous_status: none
  previous_score: N/A
  gaps_closed: []
  gaps_remaining: []
  regressions: []
---

# Phase 27: Stateful Sessions — Verification Report

**Phase Goal (ROADMAP.md):** Users can run multi-interaction stateful executions (insert facts -> fire -> insert more -> fire again -> dispose) via `session_id`, with bounded resource use. Requirement SESS-01: caller supplies `session_id`, inserts facts, fires, calls again with same `session_id` and sees accumulated working memory; bounded resource use via TTL+LRU+max-count+sweep+gauge; stateless one-shot path unchanged when `session_id` absent.

**Verified:** 2026-07-06T11:05:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SC-1: POST /v1/execute with session_id='s1' + facts={scorecard:{score:0}} fires r1, returns facts_after.scorecard.score==10 with non-null response.session_id (server UUID); a second call with SAME session_id and facts={} returns score==20 (WM accumulated) | VERIFIED | `api/tests/execute_integration.rs:1651 stateful_session_accumulates_wm` PASS. Accumulating rule `score_plus_10_accumulating_rule` (line 105, `ComparisonOp::Ge score >= 0`) fires on every call. `execute_sessionful` (`api/src/v1/execution.rs:750-955`) calls `SessionManager::lookup_or_create` (line 762), inserts facts into the per-session engine (line 779-793), runs `engine.execute()` in `web::block` (line 838), writes the engine back (line 862). Test asserts score:10 then score:20. |
| 2 | SC-2: A session untouched for >idle TTL is evicted (subsequent call creates FRESH session, score:10 not score:20); a session touched within TTL persists (score:20) | VERIFIED | `api/tests/execute_integration.rs:1710 session_ttl_evicts_idle` PASS. Custom `SessionConfig{idle_ttl: 1s}` (line 1715); `tokio::time::sleep(1100ms)` (line 1743) + direct `manager.sweep_expired()` (line 1744). `sweep_expired` (`api/src/v1/sessions.rs:257-297`) evicts entries where `now-last_used > idle_ttl OR now-created_at > max_lifetime` (line 268-269). Test asserts evicted session has empty WM (no scorecard), touched session returns score:20. |
| 3 | SC-3: When session count exceeds MAX_SESSIONS, LRU entry evicted; rrules_sessions_active gauge reflects CAPPED count (reads MAX_SESSIONS, not MAX_SESSIONS+1); gauge on Phase-24 DEDICATED Registry (not default_registry — Pitfall 24-A) | VERIFIED | `api/tests/execute_integration.rs:1796 lru_evicts_oldest_and_gauge_capped` PASS. `max_sessions=2` (line 1802); creates s3a/s3b/s3c; `sweep_expired()` (line 1829) evicts LRU via `min_by_key(last_used)` (sessions.rs:283-287); `manager.len()==2` asserted (line 1830). Gauge registered on `counters.registry` (`api/src/main.rs:118-121`) which is `Registry::new()` (`api/src/metrics.rs:95`) — NOT `default_registry()`. `/metrics` renders from `counters.registry.gather()` (metrics.rs:355), so the gauge appears in /metrics text. `grep 'default_registry' main.rs` returns 0 code hits (only a comment at line 109). |
| 4 | SC-4: DELETE /v1/execute/session/{id} disposes (next call creates fresh empty WM); POST /v1/execute/session/{id}/reset clears WM WITHOUT disposing (next call resumes with empty WM, score:10 not score:20) | VERIFIED | `api/tests/execute_integration.rs:1870 session_lifecycle_delete_reset` PASS. `dispose_session` (sessions.rs:493-507) calls `dispose_by_alias` -> `remove_by_uuid` (sessions.rs:415-425). `reset_session` (sessions.rs:519-535) calls `reset_by_alias` (sessions.rs:328-409) which REBUILDS a fresh engine via `registry.create_engine_by_name` + `load_ruleset` (Open Q#1 — `ReteEngine::reset()` clears rules too, lines 365-398). Test asserts DELETE returns 200 + fresh WM; reset returns 200 + empty WM (no scorecard). |
| 5 | D-05: Stateless one-shot path UNCHANGED when session_id absent (no regression to Phase 23) | VERIFIED | `api/src/v1/execution.rs:524` branches `if req.session_id.is_some()` -> `execute_sessionful`; else stateless body (lines 535-734) continues unchanged. `execute_fires_real_rule` (Phase 23 keystone) PASS in the same suite. `cargo test -p api --test execute_integration -- execute_fires_real_rule --exact` exits 0. Statelesss path sets `session_id: None` (line 733) and `active_sessions: session_manager.len()` (line 680 — replaces Phase-24 hardcoded `0`, D-08 improvement, not a behavior change). |
| 6 | D-04: Server UUID v4 primary key + client session_id as idempotency alias (resume-on-match); stale alias falls through to create | VERIFIED | `api/src/v1/sessions.rs:84-86`: `sessions: DashMap<Uuid, ...>` keyed by server UUID; `aliases: DashMap<String, Uuid>` keyed by client alias. `lookup_or_create` resume path (line 152-178): `aliases.get(alias)` -> uuid -> `sessions.get(uuid)` -> clone Arc; stale alias (uuid not in sessions) falls through to create (line 177). Create path: `Uuid::new_v4()` (line 235), inserts into both maps (lines 246-247). `ExecuteResponse.session_id: Some(server_uuid.to_string())` (execution.rs:953). |
| 7 | D-06: Engine fixed at session creation; conflicting engine on resume -> HTTP 400 | VERIFIED | `api/src/v1/sessions.rs:157-172`: resume path locks entry, compares `locked.engine_name != req_name`, returns `Err((StatusCode::BAD_REQUEST, "session engine is fixed: expected {X}, got {Y}"))`. `execute_sessionful` surfaces this via `?` at line 762-767. |
| 8 | D-07: engine_generation reflects creation-generation; reset_session rebuilds FRESH engine (ReteEngine::reset clears rules too) | VERIFIED | Create: `engine_generation: generation` captured under read-lock at create time (sessions.rs:218-232, 239). `execute_sessionful` stamps `engine_generation` into response (execution.rs:818, 951). `reset_by_alias` rebuilds fresh engine via `registry.create_engine_by_name` + `load_ruleset` (sessions.rs:366-398) — Open Q#1 resolution documented in code comment (lines 316-327). D-07 discretion "reset-also-reload" updates generation to current (line 397) — plan-sanctioned. |
| 9 | D-08: MetricsSnapshot.active_sessions populated from SessionManager::len() (not hardcoded 0) | VERIFIED | `api/src/v1/execution.rs:680` (stateless branch) and `:896` (sessionful branch) both set `active_sessions: session_manager.len()`. `grep -c 'active_sessions: 0' execution.rs` returns 0 — no hardcoded zero remains. Gauge also set to len() after each execute (execution.rs:889-891) and after each sweep (sessions.rs:296). |
| 10 | Pitfall 7: No DashMap/Mutex guard held across web::block (std::mem::replace engine out, drop guard, move into web::block, re-lock to write back) | VERIFIED | `api/src/v1/execution.rs:810-834`: MutexGuard scoped to its own block; `std::mem::replace(&mut guard.engine, placeholder)` swaps real engine out (line 833); guard drops at block end (line 834) BEFORE `web::block(...).await` (line 838). Placeholder is a throwaway fresh engine from `registry.create_engine_by_name` (line 821). Re-lock at line 855-864 to write engine back + bump `last_used`. `lookup_or_create` clones the `Arc<Mutex<Entry>>` out and drops DashMap `Ref` immediately (sessions.rs:152-154) — no shard guard held across web::block. |
| 11 | Prohibition: v1.0 engine::environment::Session stub NOT used by the live path | VERIFIED | `api/src/v1/sessions.rs:38-42`: `StatefulSessionEntry.engine: Box<dyn engine::engine::registry::ExecutionEngine>` — the Phase-23 surface, NOT `engine::environment::Session`. Code comment at line 35-37 explicitly documents D-01. `grep 'environment::Session' sessions.rs execution.rs` returns 0 hits. (`engine::environment::Engine` appears in `EngineState` at execution.rs:216 — pre-existing v1.0 hot-reload state, NOT the sessionful path; the sessionful path uses `registry.create_engine_by_name`.) |
| 12 | Prohibition: All four OOM controls (TTL + LRU + max-count + sweep + gauge) shipped TOGETHER | VERIFIED | TTL: `idle_ttl`/`max_lifetime` in `SessionConfig` (sessions.rs:64-69), enforced in `sweep_expired` (lines 267-270). LRU: `while len > max_sessions, evict min_by_key(last_used)` (sessions.rs:282-292). Max-count: `max_sessions` (default 1024, sessions.rs:446-449). Sweep: `run_session_sweep` spawned in `main.rs:179` via `tokio::spawn(run_session_sweep(sweep_manager))`. Gauge: `rrules_sessions_active IntGauge` registered on dedicated Registry (main.rs:113-121), set to len() in sweep_expired (sessions.rs:296), dispose_by_alias (sessions.rs:307), and execute_sessionful (execution.rs:889-891). All five shipped. |

**Score:** 12/12 truths verified (0 present, behavior-unverified; 0 overrides)

All four behavior-dependent truths (SC-1..SC-4) have passing behavioral tests (`cargo test -p api --test execute_integration -- stateful_session_accumulates_wm session_ttl_evicts_idle lru_evicts_oldest_and_gauge_capped session_lifecycle_delete_reset --exact` -> 4 passed, 0 failed). The state-transition invariants (WM accumulation, TTL eviction, LRU cap, dispose/reset lifecycle) are exercised by these tests, not just symbol presence.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `api/src/v1/sessions.rs` | NEW module: SessionManager, StatefulSessionEntry, SessionConfig, read_session_config, run_session_sweep, dispose_session, reset_session | VERIFIED | 535 lines; all pub types + fns present; lookup_or_create/sweep_expired/dispose_by_alias/reset_by_alias full impls; no NOT_IMPLEMENTED stubs (`grep NOT_IMPLEMENTED` returns 0) |
| `api/src/v1/execution.rs` | ExecuteResponse.session_id: Option<String> with skip_serializing_if; execute_single branches on session_id; execute_sessionful; MetricsSnapshot.active_sessions from len(); execute_with_session removed | VERIFIED | `session_id` field at line 24/72; branch at line 524; `execute_sessionful` at line 750; `active_sessions: session_manager.len()` at 680+896; `grep 'fn execute_with_session'` returns 0 |
| `api/src/main.rs` | SessionManager constructed once; sweep task spawned; app_data injected; DELETE/reset routes registered; gauge on dedicated Registry | VERIFIED | Construction at lines 112-126; sweep spawn at line 179; `.app_data(web::Data::new(session_manager.clone()))` at line 224; routes at lines 266-273; gauge on `counters.registry` (line 118-121) |
| `api/src/v1/mod.rs` | `pub mod sessions;` declaration | VERIFIED | Line 13 |
| `api/Cargo.toml` | `dashmap = "6.2.1"` (NOT 7.0.0-rc2) | VERIFIED | Line 59; `grep 'dashmap = "7'` returns 0 |
| `api/tests/execute_integration.rs` | shared_session_manager() helper; SessionManager::clear() in reset_test_env(); DELETE/reset routes; four #[serial_test::serial] SC tests | VERIFIED | Helpers at lines 331-379; `reset_test_env` clear at line 497; routes at lines 398-405; four SC tests at lines 1651, 1710, 1796, 1870 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| execute_single sessionful branch | SessionManager::lookup_or_create | `session_manager.lookup_or_create(&client_alias, req.engine.as_deref(), &registry, &ruleset)` at execution.rs:762 | WIRED | Arc<Mutex<Entry>> cloned out by lookup_or_create (sessions.rs:154), no DashMap guard held across web::block |
| SessionManager construction (main.rs) | rrules_sessions_active IntGauge on Phase-24 dedicated Registry | `counters.registry.register(Box::new(sessions_active.clone()))` at main.rs:118-121 | WIRED | `counters.registry` is `Registry::new()` (metrics.rs:95); `/metrics` gathers from same registry (metrics.rs:355) |
| SessionManager | second DashMap<String, Uuid> for alias->UUID resume | `aliases: DashMap<String, Uuid>` at sessions.rs:86; lookup at line 152; insert at line 247 | WIRED | Stale-alias fall-through-to-create at line 177 (Pitfall 3 avoidance) |
| run_session_sweep task (main.rs) | SessionManager::sweep_expired | `tokio::spawn(run_session_sweep(sweep_manager))` at main.rs:179; interval loop at sessions.rs:471-477 | WIRED | Evicts by TTL/max-lifetime/LRU + updates gauge after (sessions.rs:296) |
| reset_session handler | registry.create_engine_by_name + load_ruleset (fresh engine rebuild) | `reset_by_alias` at sessions.rs:328-409; rebuild at lines 366-398 | WIRED | Open Q#1: ReteEngine::reset() clears rules too, so rebuild is required |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|---------------------|--------|
| execute_sessionful | `facts_after` | `engine.get_facts()` post `engine.execute()` in web::block (execution.rs:838-841, 904-923) | Yes — real WorkingMemory reverse-mapped via `engine_value_to_json` | FLOWING |
| MetricsSnapshot.active_sessions | `session_manager.len()` | `SessionManager::sessions.len()` (DashMap len) at execution.rs:680,896 | Yes — real live session count | FLOWING |
| rrules_sessions_active gauge | `sessions_active.set(len())` | `SessionManager::sessions.len()` at sessions.rs:248,296,307; execution.rs:891 | Yes — real live session count | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| SC-1: WM accumulates across calls | `cargo test -p api --test execute_integration -- stateful_session_accumulates_wm --exact` | 1 passed, 0 failed | PASS |
| SC-2: TTL eviction | `cargo test -p api --test execute_integration -- session_ttl_evicts_idle --exact` | 1 passed, 0 failed | PASS |
| SC-3: LRU + gauge cap | `cargo test -p api --test execute_integration -- lru_evicts_oldest_and_gauge_capped --exact` | 1 passed, 0 failed | PASS |
| SC-4: DELETE/reset lifecycle | `cargo test -p api --test execute_integration -- session_lifecycle_delete_reset --exact` | 1 passed, 0 failed | PASS |
| D-05: stateless path unchanged | `cargo test -p api --test execute_integration -- execute_fires_real_rule --exact` | 1 passed, 0 failed | PASS |
| Full execute_integration suite | `cargo test -p api --test execute_integration` | 22 passed, 0 failed, 1 ignored | PASS |
| api crate (no regressions) | `cargo test -p api` | 110 passed, 0 failed, 1 ignored | PASS |
| engine crate (no regressions) | `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` | 242 passed, 0 failed, 3 ignored | PASS |

### Probe Execution

No phase-declared probes (`scripts/*/tests/probe-*.sh`) — Phase 27 is an API feature phase, not a migration/tooling phase. The behavioral spot-checks above serve as the probe equivalent.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| SESS-01 | 27-01-PLAN.md | Multi-interaction stateful execution via session_id with bounded resource use (TTL+LRU+max-count+sweep+gauge); stateless path unchanged when session_id absent | SATISFIED | SC-1..SC-4 tests PASS; all four OOM controls shipped (truth #12); stateless path unchanged (truth #5); SessionManager with DashMap-sharded per-session engine; lifecycle routes DELETE/reset registered. REQUIREMENTS.md traceability table marks SESS-01 Complete. |

No orphaned requirements — REQUIREMENTS.md maps only SESS-01 to Phase 27, and 27-01-PLAN.md claims only SESS-01.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | - | No TBD/FIXME/XXX markers | - | - |
| (none) | - | No scope-reduction language ("simplified version"/"static for now"/etc.) | - | - |

`grep -rnE 'TBD|FIXME|XXX' api/src/v1/sessions.rs api/src/v1/execution.rs api/src/main.rs` returns 0 hits. The "placeholder" hits in execution.rs are all legitimate references to the Pitfall 7 web::block engine-swap pattern (a throwaway engine swapped in via `std::mem::replace` — a real implementation technique, not a stub). The "hardcoded" hit at execution.rs:1047 is in pre-existing `batch_execute_rules` error-masking, not the sessionful path.

### Human Verification Required

None. All truths are behaviorally verified by passing automated tests. The `/metrics` gauge text rendering is deterministic (gauge registered on `counters.registry` at main.rs:118-121; `/metrics` gathers from `counters.registry.gather()` at metrics.rs:355) — no live-server spot-check needed.

### Gaps Summary

No gaps. All 12 must-haves verified with behavioral evidence. The four SC keystone tests (SC-1 accumulate, SC-2 TTL, SC-3 LRU+gauge, SC-4 lifecycle) pass GREEN. The stateless one-shot path is unchanged (execute_fires_real_rule PASS). No engine regressions (242 passed). No api regressions (110 passed). The `rrules_sessions_active` gauge is registered on the Phase-24 dedicated Prometheus Registry (Pitfall 24-A avoided). The v1.0 `engine::environment::Session` stub is NOT used by the live path (D-01). All four OOM controls shipped together (D-03/Pitfall 5). Pitfall 7 web::block !Send guard scope handled via scoped MutexGuard + `std::mem::replace` engine-out pattern.

SESS-01 requirement SATISFIED. Phase 27 goal achieved.

---

_Verified: 2026-07-06T11:05:00Z_
_Verifier: Claude (gsd-verifier)_