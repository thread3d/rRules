# Roadmap: rRules - Business Rules Management System

## Overview

A production-grade Business Rules Management System (BRMS) in Rust: define complex business decisions as reusable rules, deploy them instantly, and monitor their impact without modifying application code. The roadmap progressed from core engine → authoring interface → REST API → governance → testing → DMN compliance → pluggable engines → production platform.

> **Milestone archives:** Full per-phase detail for completed milestones is preserved in `.planning/milestones/v{X.Y}-ROADMAP.md`.

## Milestones

- ✅ **v1.0 MVP** — Phases 1–6 (shipped 2026-06-07) — Core engine, authoring, REST API, governance, testing, DMN
- ✅ **v2.0 Testing Framework** — Phases 7–9 (shipped 2026-06-07) — Testing infrastructure, advanced features, enhancements
- ✅ **v3.0 Pluggable Engines** — Phase 10 (shipped 2026-06-xx) — Rete/TREAT/LEAPS/PHREAK with runtime selection
- ✅ **v4.0 Production Platform** — Phases 17–22 (shipped 2026-07-02) — Full frontend, governance, monitoring, management API, deployment
- 📋 **v5.0 Live Execution** — Phases 23–28 (in progress) — Build a minimal real rule evaluator + wire `/v1/execute` to it; auto-populate trackers + metrics so v4.0 monitoring surfaces read live data *(milestone thesis corrected during Phase 23 research: this is build + wire, not wire-only — the v3.0 engines never compiled and `execute_rules` was a placeholder; see `23-RESEARCH.md`)*

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1–6) — SHIPPED 2026-06-07</summary>

- [x] Phase 1: Core Engine Foundation (6/6 plans) — completed 2026-05-23
- [x] Phase 2: Rule Authoring Interface (5/5 plans) — completed 2026-06-01
- [x] Phase 3: REST API Layer (6/6 plans) — completed 2026-06-07
- [x] Phase 4: Governance & Access Control (5/5 plans) — completed 2026-06-02
- [x] Phase 5: Testing & Monitoring (6/6 plans, stub) — completed 2026-06-01
- [x] Phase 6: DMN Compliance (3/3 plans) — completed 2026-06-01

</details>

<details>
<summary>✅ v2.0 Testing Framework (Phases 7–9) — SHIPPED 2026-06-07</summary>

- [x] Phase 7: Core Testing Infrastructure (6/6 plans) — completed 2026-06-07
- [x] Phase 8: Advanced Testing Features (5/5 plans) — completed 2026-06-07
- [x] Phase 9: Testing Framework Enhancements (4/5 plans; 1 deferred) — completed 2026-06-07

</details>

<details>
<summary>✅ v3.0 Pluggable Engines (Phase 10) — SHIPPED</summary>

- [x] Phase 10: Pluggable Engine Architecture (Rete/TREAT/LEAPS/PHREAK + runtime selection) — completed; dashboard deferred to v4.0

</details>

<details>
<summary>✅ v4.0 Production Platform (Phases 17–22) — SHIPPED 2026-07-02</summary>

- [x] Phase 17: Production Frontend Architecture (React/TS IDE, decision tables, dashboard)
- [x] Phase 18: Enterprise Governance (RBAC + JWT, approval workflow, audit trail, Git versioning)
- [x] Phase 19: Monitoring & Metrics (Prometheus /metrics, dashboards, dead-rule detection, coverage)
- [x] Phase 20: Complete Management API (rules/categories/tags/approvals/audit/branches)
- [x] Phase 21: Deployment & CI/CD (Alpine Docker, Helm chart, hot reload)
- [x] Phase 22: Close v4.0 frontend integration gaps — wire orphaned UI to backend (9/9 requirements MET, verified PASS)

</details>

<details>
<summary>📋 v5.0 Live Execution (Phases 23–28) — IN PROGRESS</summary>

- [x] **Phase 23: Execute evaluator + wiring** — Fix the v3.0 engine import bug, build a minimal real rule evaluator on `ReteEngine`, and route `/v1/execute` through `EngineRegistry`; factory shared, engine instance per request (completed 2026-07-03)
- [x] **Phase 24: Execution counters + metrics broadcast** — `ExecutionCounters` + Prometheus promote + WS broadcast; dashboards go live (completed 2026-07-03)
- [ ] **Phase 25: Real reload + JobStore** — Shared reloadable `RuleSet` snapshot; async reload writes `ReloadJob`; batch inherits real execution
- [ ] **Phase 26: RuleExecutionTracker singleton + dead-rule live data** — Singleton-ize tracker; dead-rules reads live firings; pin with e2e test
- [ ] **Phase 27: Stateful sessions** — `SessionManager` (DashMap, TTL, LRU, sweep); stateful `/v1/execute/session/{id}` endpoints
- [ ] **Phase 28: Frontend test-runner** — `execution.ts` + `TestRunner` component; engine selector dropdown; inference trace surfaced

</details>

## v5.0 Phase Details

### Phase 23: Execute evaluator + wiring

**Goal:** `POST /v1/execute` fires real rules via the v3.0 `EngineRegistry`/`ExecutionEngine` surface and returns truthful `rules_fired`/`rules_not_fired`/`facts_after` — closing the documented stub.
**Scope note (research correction):** The milestone research framed this phase as "wiring, not building." Phase-23 research against the live code found that is inaccurate — the v3.0 engine modules never compiled (an `engine::` → `crate::` import bug in all four modules; `cargo check -p engine --features engine-rete` fails `E0433`), and `ReteEngine::execute_rules` (`rete/mod.rs:321`) is a placeholder that never evaluates `Rule.condition` or applies `Rule.actions`. This phase therefore *builds* a minimal direct rule evaluator on `ReteEngine` (iterate rules, evaluate condition, apply actions to working memory — full Rete alpha/beta propagation is explicitly out of scope) and *wires* `/v1/execute` through `EngineRegistry`. See `23-RESEARCH.md`.
**Depends on:** v4.0 (Phase 22) — platform baseline
**Requirements:** EXEC-01, EXEC-02, ENGINE-07, ENGINE-08, MON-05
**Key deliverables:**

- **(build) Fix the engine import bug** in `engine/src/engine/{rete,treat,leaps,phreak}/mod.rs`: `use engine::monitoring::coverage::CoverageTracking;` → `use crate::monitoring::coverage::CoverageTracking;` (the `E0433` that has kept the pluggable engines from ever compiling)
- **(build) Extend the `ExecutionEngine` trait** with `load_ruleset(&mut self, ...)` + a facts accessor so an engine can accept a `RuleSet` and expose post-execution working memory (the trait currently has neither — see `23-RESEARCH.md` Flag 2)
- **(build) Implement a minimal direct rule evaluator on `ReteEngine`** — rewrite the placeholder `execute_rules` (`rete/mod.rs:321`) into a direct loop that evaluates each `Rule.condition` against inserted facts and applies matching `Rule.actions` to working memory; record fired rule ids for `rules_not_fired` (full Rete alpha/beta propagation is out of scope)
- **(wire) Share `Arc<EngineRegistry>` + `EngineSelector` as `web::Data`** (built once at startup in `main.rs`)
- **(wire) Add `engine: Option<String>` to `ExecuteRequest`**; validate against `EngineRegistry.available_engines()` (HTTP 400 on unknown); config default `RRULES_DEFAULT_ENGINE` when `engine` absent
- **(wire) Construct a fresh `Box<dyn ExecutionEngine>` per request via `spawn_blocking`**; `load_ruleset`; insert facts; `engine.execute()`
- **(wire) Serialize `facts_after` from post-execution `WorkingMemory`** (reverse `engine_value_to_json`), not echoed input JSON
- **(wire) `CoverageTracking` records once-per-rule** outside the nagram inner loop (the current per-nagram `record_rule_fire` at `rete/mod.rs:332` double-counts; the evaluator collects a `HashSet<rule_id>` then records once)
- **(prep) Rename metrics `EngineState` → `MetricsSnapshot`** (Pitfall 10) before adding logic
- **(wire) Enable the `engine-rete` cargo feature on the `api` crate** (and `engine-treat`/`engine-leaps`/`engine-phreak` — all four factories are confirmed real per `23-RESEARCH.md` Flag 1)

**Pitfalls to avoid:**

- Pitfall 1 (`&mut self` engine-sharing serialization) — factory shared, instance per request; never `engine_state.engine.write()` in the execute path
- Pitfall 10 (`EngineState` name collision) — prep rename to `MetricsSnapshot`
- Pitfall 6 (coverage double-counting at `rete/mod.rs:332`) — record once-per-rule outside the inner loop
- Pitfall 11 (`facts_after` echo bug) — build from post-execution `WorkingMemory`
- Pitfall 7 (per-request compile cost) — spike; ship engine pool here if >1ms
- Pitfall 9 (`Engine::clone()` ≠ `ReteEngine` clone) — verify during compile-cost spike

**Research flags (resolved in `23-RESEARCH.md`):** (1) all four `EngineFactory` impls exist and are real, but all four modules have the same import bug → fix + enable all four features; (2) trait has no `load_ruleset` → extend the trait and implement a direct evaluator (not full Rete propagation); (3) `inference_trace` left `None` for Phase 23 (populating it is EXEC-04 / Phase 28); (4) per-request construction is O(1), `ReteEngine: !Clone` → ship per-request construction, no pool.
**Success criteria (what must be TRUE):**

1. `POST /v1/execute` against a rule that sets `score := score + 10` returns `rules_fired >= 1` and `facts_after` shows the incremented `score` (not the request input)
2. `POST /v1/execute` with `engine: "rete"` succeeds; with `engine: "unknown"` returns HTTP 400
3. Omitting `engine` resolves to the config-defined default and executes successfully
4. `GET /v1/metrics/coverage` shows `fired == true` for a rule fired by a recent `/v1/execute` (auto-populated, no separate record call)

**Plans:**
6/6 plans complete

- [x] 23-01-PLAN.md — Engine compile-unblock (L-01 import fix), ExecutionEngine trait bridge (L-02) + EngineResult.fired_rule_ids, register all four factories (D-01), enable all four engine features on the api crate
- [x] 23-02-PLAN.md — Rule-model extensions: Expression type (D-06) + Action::UpdateFact computed expressions, full ComparisonOp coverage + real Pattern matching in Condition::evaluate (D-05)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 23-03-PLAN.md — ReteEngine direct-evaluator rewrite (D-04): load_ruleset/get_facts impls, fixpoint-with-cap forward chaining (D-07), once-per-rule coverage (L-06/Pitfall 6), fired_rule_ids in EngineResult
- [x] 23-04-PLAN.md — Metrics EngineState → MetricsSnapshot rename (L-05/Pitfall 10) + Wave 0 RED test scaffolds (engine/tests/coverage.rs, api/tests/execute_integration.rs) for all four ROADMAP success criteria

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 23-05-PLAN.md — API wiring: rewrite execute_single (per-request engine via spawn_blocking, facts_after from get_facts reverse mapping D-08/Pitfall 11, engine param validation 400/ENGINE-07, default resolution ENGINE-08, rules_not_fired), shared Arc<RwLock<RuleSet>> from RULE_STORAGE_PATH (D-03/D-03a), main.rs web::Data wiring + RRULES_DEFAULT_ENGINE/RRULES_MAX_CHAIN_ITERATIONS env vars
- [x] 23-06-PLAN.md — Four-engine rollout (D-01): mirror direct evaluator onto Treat/Leaps/Phreak + per-engine score+10 keystone tests (D-02)

### Phase 24: Execution counters + metrics broadcast

**Goal:** Per-execution Prometheus metrics and a WebSocket `EngineState` snapshot make the v4.0 monitoring dashboards display live activity with zero protocol/UI changes.
**Depends on:** Phase 23 (counters are incremented by `execute_single`)
**Requirements:** MON-07
**Key deliverables:**

- `ExecutionCounters` (`AtomicU64`) as `web::Data` — `execute_requests_total`, `execute_rules_fired_total`, `execute_duration_seconds` histogram, engine label
- Promote `prometheus` 0.13 from workspace dep into `api/Cargo.toml`; register API-layer counters/gauges (engine-side per-engine gauges stay as-is — extension, not replacement)
- `metrics_endpoint` (`/metrics`) and `get_engine_state` read the counters
- Broadcast `EngineState` snapshot via the existing `metrics_tx` WebSocket channel after each execute
- Add `execution_id` + `engine_generation` to `ExecuteResponse` (addresses the stale-polling race, Pitfall 8)
- The three frontend dashboards (Engine, Coverage, DeadRule) display live data with **zero frontend changes** — adapters already tolerate real shapes

**Pitfalls to avoid:**

- Pitfall 8 (stale-polling race) — extend `ExecuteResponse` with `execution_id` + `engine_generation`; frontend will consume response directly in Phase 28

**Research flags:** None (standard `AtomicU64` + `prometheus::TextEncoder` extension; reference impl at `engine/src/metrics/prometheus.rs:45`)
**Success criteria:**

1. After `POST /v1/execute`, `GET /metrics` exposes `rrules_execute_requests_total` + `rrules_execute_rules_fired_total` + `rrules_execute_duration_seconds` with the engine label populated
2. The WebSocket monitoring channel emits an `EngineState` snapshot reflecting the most-recent execute (non-zero counters after a run)
3. The existing frontend Engine/Coverage/DeadRule dashboards render live (non-hardcoded) numbers after a run with no frontend code changes

**Plans:** 3/3 plans complete
**Wave 1**

- [x] 24-01-PLAN.md — Wave 0 RED test scaffolds: counters_exposed_in_metrics, counters_increment_per_request, response_includes_engine_generation (execute_integration.rs); broadcasts_snapshot_after_execute (metrics_ws.rs); metrics_prometheus_text_format assertion update (metrics_integration.rs) — COMPLETE 2026-07-03 (5 expected RED failures, build green, merged cd433b08a)
- [x] 24-02-PLAN.md — Wave 1 Foundation: ExecutionCounters struct + dedicated prometheus Registry + labeled counters/histogram (exponential_buckets 0.001..4s) + Default impl; prometheus = { workspace = true } in api/Cargo.toml; ExecuteResponse.engine_generation: u64 (#[serde(default)], 0); main.rs web::Data wiring — COMPLETE 2026-07-03 (cargo check exit 0; 4 RED remain, response_includes_engine_generation flipped GREEN, 0 regressions, merged d8b31c469)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 24-03-PLAN.md — Wave 2 Instrumentation: execute_single timing + counter increment + MetricsSnapshot broadcast after web::block (Pitfall 24-B/C); metrics_endpoint TextEncoder rewrite; handler signature changes + test app_data injection; turns all Wave 0 RED tests GREEN — COMPLETE 2026-07-03 (Nyquist closure: 4 RED→GREEN; api 96/0, engine 241/0; merged 8833f5202)

### Phase 25: Real reload + JobStore

**Goal:** Async reload-then-execute actually swaps the compiled rule set so subsequent executes run refreshed rules, and `GET /v1/jobs/{id}` returns real reload job status.
**Depends on:** Phase 23 (execute path is real)
**Requirements:** DEPLOY-05, EXEC-03
**Key deliverables:**

- Shared `Arc<RwLock<RuleSet>>` (or `ArcSwap<RuleSet>`) built once at startup from `RULE_STORAGE_PATH`; `execute_single` loads it into each request's engine
- `reload_engine` rebuilds the new `RuleSet` fully off to the side, then under the write-lock does a single swap (`*self.engine = new`) — **swap-in-place, not in-place mutation**
- Generation counter on the engine; `engine_generation` included in `ExecuteResponse`
- Async reload branch writes `ReloadJob{Pending → Running → Completed/Failed}` to `JobStore` so `GET /v1/jobs/{id}` works (no more 404)
- Batch-execute (`POST /v1/rules/batch-execute`) drives real per-item execution with per-item isolation (each item builds its own throwaway engine); `total_fired`/`succeeded`/`failed` aggregate truthfully

**Pitfalls to avoid:**

- Pitfall 4 (hot-reload race) — swap-don't-mutate + generation tag; pick drain-vs-generation-accept and document
- Pitfall 12 (`reload_engine` is currently a no-op) — replace the acquire-and-drop write-lock with real recompilation
- Pitfall 14 (`JobStore` never written for async reload) — spawned task must persist `ReloadJob`

**Research flags:** Read `rete/mod.rs` network construction in full (defines `RuleSet`→Rete network bridging); measure `ArcSwap` vs `Arc<RwLock<RuleSet>>` under realistic read concurrency — prefer `Arc<RwLock<RuleSet>>` to avoid a new dep unless contention shows up
**Success criteria:**

1. After a rule is edited and `reload_rules({async:true})` returns `202`, polling `GET /v1/jobs/{id}` transitions `Pending → Running → Completed`, and the next `POST /v1/execute` reflects the edited rule
2. `POST /v1/rules/batch-execute` with N items returns `total_fired`/`succeeded`/`failed` consistent with running each item through real execution; one panicking item does not poison the others
3. `ExecuteResponse.engine_generation` increments after a reload, so callers can detect they ran against a prior rule set

**Plans:** 1/3 plans executed

### Phase 26: RuleExecutionTracker singleton + dead-rule live data

**Goal:** `RuleExecutionTracker` becomes a process-wide singleton so dead-rule detection reads live firings — `GET /v1/metrics/dead-rules` no longer always returns `[]`.
**Depends on:** Phase 23 (needs real firings to record)
**Requirements:** MON-06
**Key deliverables:**

- Convert `RuleExecutionTracker` to `LazyLock<Mutex<…>>` singleton, mirroring the existing `CoverageTracking` pattern
- Add `record_evaluation`/`record_fire` calls next to the `CoverageTracking` calls in `rete/mod.rs:293,332` (same call sites, one canonical "fire" semantics — once per rule per execution, outside the per-nagram inner loop)
- `dead_rules_endpoint` reads from the singleton (no more `RuleExecutionTracker::new()` at `metrics.rs:214`)
- Unify the three-counter drift: define one canonical "fire" semantics; have `RuleExecutionTracker` and engine `metrics` derive from / delegate to `CoverageTracking` as the single source of truth
- Pin with an end-to-end integration test: after one `/v1/execute` firing rule `r1`, dead-rules excludes `r1` and coverage shows `r1.fired == true`

**Pitfalls to avoid:**

- Pitfall 3 (stale/empty tracker — `dead_rules_endpoint` instantiates a fresh empty tracker per call) — singleton-ize; remove/deprecate the public `new()` to catch stray local instantiation
- Pitfall 6 (three-counter drift — `CoverageTracking`, `RuleExecutionTracker.total_fires`, per-engine `metrics.record_rule_fired`) — unify around `CoverageTracking` as canonical; record once-per-rule outside the inner loop

**Research flags:** None — direct mirror of the existing `CoverageTracking` `LazyLock<Mutex<…>>` pattern; call sites already known
**Success criteria:**

1. After `POST /v1/execute` fires rule `r1`, `GET /v1/metrics/dead-rules` does NOT contain `r1` (previously always returned `[]`)
2. `GET /v1/metrics/coverage` shows `r1.fired == true` and `r1.evaluation_count >= 1` after the run
3. `/metrics`, `/v1/metrics/coverage`, and `/v1/metrics/dead-rules` agree on the fire count for a rule (no three-counter drift)

**Plans:** TBD

### Phase 27: Stateful sessions

**Goal:** Users can run multi-interaction stateful executions (insert facts → fire → insert more → fire again → dispose) via `session_id`, with bounded resource use.
**Depends on:** Phase 23 (sessions wrap the per-request engine that Phase 23 built)
**Requirements:** SESS-01
**Key deliverables:**

- NEW `api/src/v1/sessions.rs` with `Session` + `SessionManager` (`Arc<DashMap<String, StatefulSession>>`, TTL, LRU, max session count)
- `execute_single` upserts/looks-up by `session_id`; stateless one-shot path unchanged when `session_id` is absent
- NEW routes `DELETE /v1/execute/session/{id}`, `POST /v1/execute/session/{id}/reset`
- Background `tokio::time::interval` sweep task (every ~60s) evicts expired sessions
- `rrules_sessions_active` Prometheus gauge so a leak is visible on the dashboard
- Server-generated session IDs (UUID v4) returned in the response; client-supplied IDs treated as idempotency keys, not primary keys
- NEW stack: `dashmap` 6.2.1 (pinned — not 7.0.0-rc2)
- Ship registry + TTL + LRU + sweep + max-count TOGETHER (the OOM failure mode is silent)

**Pitfalls to avoid:**

- Pitfall 5 (sessions never expire / leak to OOM) — TTL + LRU + sweep + max-count + Prometheus gauge all ship together; server-generated IDs prevent caller-collision leaks

**Research flags:** Light — TTL/sweep defaults are config knobs (proposed: ~15 min idle TTL, ~24 h max lifetime, ~1024 max sessions); not architecturally load-bearing
**Success criteria:**

1. A caller can `POST /v1/execute` with a `session_id`, insert facts, fire, then call again with the same `session_id` and see accumulated working memory from the prior call
2. A session untouched for >TTL is gone from the registry; a session touched within TTL persists
3. When the session count exceeds `MAX_SESSIONS`, the LRU entry is evicted; `rrules_sessions_active` gauge reflects the live count
4. `DELETE /v1/execute/session/{id}` disposes the session; `POST /v1/execute/session/{id}/reset` clears working memory without disposing

**Plans:** TBD
**UI hint**: yes (session endpoints surface in the API the frontend will call)

### Phase 28: Frontend test-runner

**Goal:** Users can run/test a rule from the IDE and see fired rules, results, facts, and the inference trace — making live execution a user-facing feature.
**Depends on:** Phases 23, 24, 26 (real execute, live metrics, dead-rule data); can run in parallel with Phase 27 if the `session_id` path is deferred
**Requirements:** FRONTEND-03, FRONTEND-04, EXEC-04
**Key deliverables:**

- NEW `frontend/src/api/execution.ts` (`executeRule`, `batchExecute`, `getJobStatus`, `listJobs`, `listEngines`) + `ExecuteRequest`/`ExecuteResponse`/`InferenceTrace`/`BatchExecute*` types in `frontend/src/api/types.ts`
- NEW `TestRunner` component in the rule editor: facts editor, engine-selector dropdown populated from `GET /v1/engines`, Run/Test buttons, results panel rendering `rules_fired`/`rules_not_fired`/`facts_after`/`inference_trace`
- `trace: bool` opt-in flag on `ExecuteRequest` (off by default for performance); `ExecuteResponse.inference_trace` populated when requested (addresses EXEC-04 — decide population source in Phase 23 research)
- Optional "Reload then Run" flow → `reloadRules({async:true})` + poll `getJobStatus` + `executeRule`
- Monitoring dashboards need NO frontend changes (adapters already tolerate real shapes)
- Drive refresh from the execute response (not polling) — use `execution_id`/`engine_generation` for correlation

**Pitfalls to avoid:**

- Pitfall 8 (stale-polling race) — drive UI refresh from the `ExecuteResponse` itself, not from a follow-up poll; keep polling only for the long-term monitoring dashboard
- Pitfall 3 (dead-rules panel empty) — Phase 26 must land first so the panel has data

**Research flags:** `inference_trace` population source is decided in Phase 23; if ReteEngine cannot emit a trace, derive the display from `rules_not_fired`/`CoverageTracking` data
**Success criteria:**

1. From the rule editor, a user clicks "Run" and sees a results panel with `rules_fired`, `rules_not_fired`, and `facts_after` reflecting the real execution
2. The engine-selector dropdown lists the available engines (from `GET /v1/engines`) and the selected engine is sent on the execute request
3. With `trace: true`, the results panel shows the inference trace; with `trace` absent/false, the trace is omitted and latency is unaffected
4. After "Reload then Run", the results panel reflects the edited rule (correlated via `engine_generation`)

**Plans:** TBD
**UI hint**: yes

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Core Engine Foundation | v1.0 | 6/6 | Complete | 2026-05-23 |
| 2. Rule Authoring Interface | v1.0 | 5/5 | Complete | 2026-06-01 |
| 3. REST API Layer | v1.0 | 6/6 | Complete | 2026-06-07 |
| 4. Governance & Access Control | v1.0 | 5/5 | Complete | 2026-06-02 |
| 5. Testing & Monitoring | v1.0 | 6/6 | Complete (stub) | 2026-06-01 |
| 6. DMN Compliance | v1.0 | 3/3 | Complete | 2026-06-01 |
| 7. Core Testing Infrastructure | v2.0 | 6/6 | Complete | 2026-06-07 |
| 8. Advanced Testing Features | v2.0 | 5/5 | Complete | 2026-06-07 |
| 9. Testing Framework Enhancements | v2.0 | 4/5 | Complete (1 deferred) | 2026-06-07 |
| 10. Pluggable Engine Architecture | v3.0 | — | Complete | 2026-06-xx |
| 17. Production Frontend | v4.0 | — | Complete | 2026-07-02 |
| 18. Enterprise Governance | v4.0 | — | Complete | 2026-07-02 |
| 19. Monitoring & Metrics | v4.0 | — | Complete | 2026-07-02 |
| 20. Complete Management API | v4.0 | — | Complete | 2026-07-02 |
| 21. Deployment & CI/CD | v4.0 | — | Complete | 2026-07-02 |
| 22. Frontend Integration Closure | v4.0 | 6/6 | Complete (verified PASS) | 2026-07-02 |
| 23. Execute evaluator + wiring | v5.0 | 6/6 | Complete    | 2026-07-03 |
| 24. Execution counters + metrics broadcast | v5.0 | 3/3 | Complete    | 2026-07-03 |
| 25. Real reload + JobStore | v5.0 | 1/3 | In Progress|  |
| 26. RuleExecutionTracker singleton + dead-rule live data | v5.0 | 0/? | Not started | - |
| 27. Stateful sessions | v5.0 | 0/? | Not started | - |
| 28. Frontend test-runner | v5.0 | 0/? | Not started | - |

---
*Roadmap last updated: 2026-07-02 — v5.0 Live Execution milestone added (Phases 23–28)*
