# Phase 27: Stateful sessions - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-04
**Phase:** 27-stateful-sessions
**Mode:** `--auto` (all gray areas auto-selected; recommended option chosen for each; logged inline — no interactive prompts)
**Areas discussed:** Session payload (keystone correction), registry data structure, TTL/LRU/max-count/sweep policy, session ID semantics, stateful execute path integration, engine selection per session, reload interaction, active-session gauge + MetricsSnapshot field, E2E keystone tests, background sweep lifecycle, session concurrency model, NEW sessions module wiring

---

## Session payload — keystone research correction (D-01)

| Option | Description | Selected |
|--------|-------------|----------|
| Wrap `engine::environment::Session` (v1.0) | Reuse the existing `Engine::create_session` surface | |
| Per-session `Box<dyn ExecutionEngine>` | Wrap the real Phase-23 evaluator; the v1.0 `Session` is a non-functional stub (returns "Not implemented", holds raw `*const Engine` — not `Send`) | ✓ |

**User's choice:** [auto] Per-session `Box<dyn ExecutionEngine>` — the real Phase-23 surface; the v1.0 `Session` is a non-functional stub (recommended — L-01; Phase 23 established `EngineRegistry` as the real surface)
**Notes:** Research correction mirroring Phase 23's own correction. The v1.0 `environment::Session` (`environment.rs:152-208`) is documented (D-01) but left in place — other v1.0 code references it; NOT used by the live path.

---

## Session registry data structure (D-02)

| Option | Description | Selected |
|--------|-------------|----------|
| `Arc<RwLock<HashMap>>` | Single global write lock on insert/evict | |
| `Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>>` (DashMap 6.2.1) | Sharded, `Send+Sync`, no global write lock — ROADMAP-pinned version | ✓ |

**User's choice:** [auto] `Arc<DashMap<...>>` — sharded, no global write lock (recommended — ROADMAP locks DashMap 6.2.1; avoids serializing unrelated sessions; matches the established no-shared-write-lock-on-hot-path discipline)
**Notes:** DashMap 7.0.0-rc2 explicitly avoided (ROADMAP pins 6.2.1). Per-entry `Arc<Mutex<...>>` handles the `&mut self` engine contract (D-11).

---

## TTL / LRU / max-count / sweep policy (D-03)

| Option | Description | Selected |
|--------|-------------|----------|
| Ship TTL only (minimal) | Smallest change that bounds idle sessions | |
| Ship all four controls together (TTL + LRU + max-count + sweep + gauge) | Partial fixes still leak (caller-collision, burst creation) | ✓ |

**User's choice:** [auto] All four controls together (recommended — ROADMAP deliverable + Pitfall 5 both mandate shipping together; the OOM failure mode is silent)
**Notes:** Defaults via env (matching `RRULES_DEFAULT_ENGINE`): idle TTL ~15 min (`900`), max lifetime ~24 h (`86400`), max sessions ~1024, sweep interval 60 s.

---

## Session ID semantics (D-04)

| Option | Description | Selected |
|--------|-------------|----------|
| Client `session_id` as primary key | Simplest; caller controls identity | |
| Server UUID v4 primary key + client `session_id` as idempotency alias | Resume-on-match; prevents caller-collision; canonical id returned to caller | ✓ |

**User's choice:** [auto] Server UUID primary key + client `session_id` as idempotency alias (resume-on-match) (recommended — ROADMAP deliverable: "Server-generated session IDs (UUID v4)... client-supplied IDs treated as idempotency keys"; Pitfall 5 step 3)
**Notes:** `session_id` already exists on `ExecuteRequest` (`execution.rs:23`); Phase 27 gives it resume semantics + adds `ExecuteResponse.session_id: Option<String>` (server UUID).

---

## Stateful execute path integration (D-05)

| Option | Description | Selected |
|--------|-------------|----------|
| Dedicated `execute_with_session` route | Separate route for sessionful calls | |
| `execute_single` branches on `session_id`; stub route removed/repurposed | Stateless path unchanged when `session_id` absent; sessionful path in `execute_single` | ✓ |

**User's choice:** [auto] `execute_single` branches on `session_id`; `execute_with_session` stub removed/repurposed (recommended — ROADMAP deliverable: "execute_single upserts/looks-up by session_id; stateless one-shot path unchanged")
**Notes:** Existing stub at `execution.rs:1081-1094` returns `rules_fired: 0` — dead after Phase 27. New routes: `DELETE /v1/execute/session/{id}` (dispose) + `POST /v1/execute/session/{id}/reset` (clear WM without disposing).

---

## Engine selection per session (D-06)

| Option | Description | Selected |
|--------|-------------|----------|
| Engine mutable per resume call | Caller can swap engine mid-session | |
| Engine fixed at creation; conflicting `engine` on resume → HTTP 400 | Changing engines discards WM; honest 400 beats silent discard | ✓ |

**User's choice:** [auto] Engine fixed at session creation; conflicting `engine` on resume → HTTP 400 (recommended — changing engines mid-session discards working memory; honest 400 beats silent discard)
**Notes:** Default-engine resolution (`RRULES_DEFAULT_ENGINE` > `selector.current_engine()`) applies only at creation.

---

## Reload interaction with live sessions (D-07)

| Option | Description | Selected |
|--------|-------------|----------|
| Existing sessions silently reload on `reload_rules` swap | Always-current ruleset | |
| Existing sessions keep their snapshot; `engine_generation` reflects creation-generation | Per-request-clone isolation (Phase 25 L-05); staleness visible via generation tag | ✓ |

**User's choice:** [auto] Keep snapshot; `engine_generation` reflects creation-generation so callers detect staleness (recommended — matches Phase 25 L-05 isolation; silent mid-session reload would surprise callers)
**Notes:** Contract is "existing sessions are NOT silently reloaded." New sessions created after a reload get the new generation.

---

## `rrules_sessions_active` gauge + MetricsSnapshot.active_sessions (D-08)

| Option | Description | Selected |
|--------|-------------|----------|
| Prometheus gauge only | Just `/metrics` surface | |
| MetricsSnapshot field only | Just the WS broadcast | |
| Both: dedicated gauge + populate existing `MetricsSnapshot.active_sessions` field | Gauge on Phase-24 prometheus `Registry`; field at `execution.rs:657` (left at 0 for Phase 27) | ✓ |

**User's choice:** [auto] Both (recommended — ROADMAP mandates the gauge; the field already exists and was explicitly left for Phase 27)
**Notes:** Field at `execution.rs:657` hardcoded `0` in Phase 24 with inline "Phase 27 populates active_sessions" comment. Gauge set from `SessionManager::len()` after each execute + on every sweep eviction. The leak becomes visible on the dashboard (Pitfall 5 step 4).

---

## E2E keystone tests (D-09)

| Option | Description | Selected |
|--------|-------------|----------|
| New `api/tests/sessions_integration.rs` | Separate test file | |
| Extend `api/tests/execute_integration.rs`; four RED→GREEN scenarios; `#[serial]` + `clear()` | Established v5.0 Wave 0 pattern | ✓ |

**User's choice:** [auto] Extend `api/tests/execute_integration.rs`; four RED→GREEN scenarios mirroring Phase 23/24/25/26 Wave 0; `#[serial]` + `clear()` for registry isolation (recommended — established v5.0 test pattern)
**Notes:** SC-1 (cross-call `score:20`), SC-2 (TTL via env `RRULES_SESSION_IDLE_TTL_SECS=1` + short sleep OR injectable fake clock — planner picks), SC-3 (LRU + gauge capped count, NOT raw), SC-4 (DELETE disposes; reset clears WM without disposing).

---

## Background sweep task lifecycle (D-10)

| Option | Description | Selected |
|--------|-------------|----------|
| New dedicated runtime + `CancellationToken` | Custom shutdown machinery | |
| `tokio::spawn` in `main.rs` alongside `start_file_watcher`; `tokio::time::interval` loop; match existing shutdown pattern | Consistent with established background-task wiring | ✓ |

**User's choice:** [auto] `tokio::spawn` alongside `start_file_watcher` (`main.rs:146`); match existing shutdown pattern (recommended — no new shutdown machinery)
**Notes:** `SessionManager` constructed once at startup, injected as `web::Data<Arc<SessionManager>>`; sweep task takes an `Arc<SessionManager>` clone.

---

## Session concurrency model — per-session Mutex (D-11)

| Option | Description | Selected |
|--------|-------------|----------|
| Single global `RwLock` over all sessions | Serialize all sessionful calls | |
| Per-session `Mutex` (same-session serialize, different sessions concurrent) | Required by `!Sync` engine (`execute(&mut self)`) | ✓ |

**User's choice:** [auto] Per-session `Mutex` — same-session calls serialize, different sessions stay concurrent (recommended — required by the `!Sync` engine contract; mirrors Phase 23's isolation discipline at session granularity)
**Notes:** Session-level analogue of Phase 23's "factory shared, instance per request" → here "registry shared, engine per session, Mutex per session." Not explicitly named in ROADMAP text but implied by the engine `&mut self` contract (Pitfall 1).

---

## NEW `sessions.rs` module + wiring (D-12)

| Option | Description | Selected |
|--------|-------------|----------|
| Inline session code in `execution.rs` | No new module | |
| NEW `api/src/v1/sessions.rs`; `web::Data<Arc<SessionManager>>` injected in `main.rs`; `execute_single` gains the param; DELETE/reset routes in `/v1` scope | Matches ROADMAP deliverable "NEW `api/src/v1/sessions.rs`" + established `web::Data` wiring | ✓ |

**User's choice:** [auto] NEW `api/src/v1/sessions.rs` + `web::Data<Arc<SessionManager>>` injection + `execute_single` param + DELETE/reset routes (recommended — matches ROADMAP deliverable + the established `web::Data` wiring pattern)
**Notes:** Add `dashmap = "6.2.1"` to `api/Cargo.toml` (NOT 7.0.0-rc2 — ROADMAP pins 6.2.1). `uuid` crate already a dep.

---

## Claude's Discretion

Technical impl details left to the planner/researcher (not gray areas — implementation freedom within locked contracts):
- `StatefulSessionEntry` exact field set (flat vs factored `StatefulSession` struct)
- TTL test mechanism (real short TTL + sleep vs injectable `Clock`/`now()` trait for determinism)
- `reset` semantics — clear WM only (D-07 lean) vs also reload against current ruleset
- `execute_with_session` route fate — remove entirely vs keep as thin alias to `execute_single`
- Sweep-task shutdown — `CancellationToken` vs runtime-teardown-only (match existing pattern)
- Idempotency-alias storage — separate `DashMap<String, Uuid>` (O(1)) vs entry field + linear scan (simpler)
- LRU eviction selection — `DashMap` iter + `min_by(last_used)` vs auxiliary ordering structure

## Deferred Ideas

- RBAC-gated `/v1/sessions` admin list/count endpoint — Pitfall 5 step 4 mentions it; NOT in ROADMAP Phase 27. Future governance/ops phase.
- Frontend test-runner / session UI — Phase 28 (consumes `session_id` from `ExecuteResponse`).
- `inference_trace` population — Phase 28 (EXEC-04).
- Removing the v1.0 `engine::environment::Session` stub — documented (D-01) but left in place (other v1.0 code references it); future cleanup phase.
- Cross-session shared working memory / undeclared state — explicitly a permanent anti-feature (FEATURES.md §"Anti-Features"), not a deferral.
- Session-scoped `record_coverage: bool` opt-out flag — not in ROADMAP; deferred.