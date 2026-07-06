# rRules - Business Rules Management System

## What This Is

rRules is a production-grade Business Rules Management System (BRMS) built in Rust. It provides a complete platform for defining, deploying, and managing business decision logic — from rule authoring through execution, governance, and monitoring — with a full React/TypeScript web frontend, enterprise governance controls, Prometheus monitoring, and Docker/Kubernetes deployment.

**Shipped:** v4.0 Production Platform (2026-07-02). The platform now delivers end-to-end: authors write rules in a browser IDE, governance controls who can change them and how those changes are approved, the engine executes them, and Prometheus/metrics dashboards monitor their live behaviour.

## Core Value

**The ONE thing that matters most:** Users can define complex business decisions as reusable rules, deploy them instantly, and monitor their impact — all without modifying application code.

## Requirements

### Validated

- ✓ **CORE-01..08**: Rule engine — forward/backward chaining, Rete algorithm, stateful/stateless modes, salience/conflict resolution, loop control, KIE-base caching — v1.0
- ✓ **RULE-01..06**: Rule authoring — web IDE, decision tables, guided editor, templates, expression language, inline validation — v1.0
- ✓ **API-01..03**: REST API for rule execution + management + OpenAPI spec — v1.0
- ✓ **DEPLOY-01..02**: Docker container + Kubernetes manifests — v1.0
- ✓ **DMN-01..03**: DMN Level 3 compliance (Decision Tables, Contexts, Literals, FEEL) — v1.0
- ✓ **TEST-01..08**: Testing framework — fixtures, unit tests, forward/backward chaining verification, deterministic mode, no-loop/lock-on-active, snapshot testing, coverage — v2.0
- ✓ **ENGINE-01..06**: Pluggable engine architecture — Rete, TREAT, LEAPS, PHREAK with runtime selection — v3.0
- ✓ **METRIC-01..02**: Per-engine metrics + algorithm comparison dashboard — v3.0
- ✓ **FRONTEND-01..02**: Complete rule authoring interface + rule management dashboard — v4.0
- ✓ **GOV-01..04**: RBAC, maker-checker approval workflow, full audit trail, Git-backed version control & branching — v4.0
- ✓ **MON-01..04**: Prometheus metrics integration, real-time monitoring dashboard, dead-rule detection, coverage reporting — v4.0
- ✓ **API-04**: Complete management API (rules/categories/tags/approvals/audit/branches) — v4.0
- ✓ **DEPLOY-03..04**: Multi-environment Docker/Kubernetes support + hot reload — v4.0
- ✓ **EXEC-01, EXEC-02, ENGINE-07, ENGINE-08, MON-05**: Real `/v1/execute` evaluator + `EngineRegistry` wiring (fresh `Box<dyn ExecutionEngine>` per request) — v5.0 Phase 23
- ✓ **MON-07**: Per-execution Prometheus counters (`rrules_execute_requests_total` / `_rules_fired_total` / `_duration_seconds`, engine-labeled, dedicated registry + `TextEncoder`) + WebSocket `MetricsSnapshot` broadcast after each execute — v5.0 Phase 24
- ✓ **SESS-01**: Stateful session execution — `SessionManager` (DashMap 6.2.1, per-session `Box<dyn ExecutionEngine>` under `std::sync::Mutex`, server UUID v4 primary key + client `session_id` as idempotency alias) lets a caller resume a session and see accumulated working memory; bounded by idle/max-lifetime TTL + LRU + max-count + 60s background sweep + `rrules_sessions_active` gauge on the dedicated Prometheus registry; stateless one-shot path (Phase 23) unchanged when `session_id` absent; `DELETE /v1/execute/session/{id}` disposes, `POST /v1/execute/session/{id}/reset` clears WM without disposing (rebuilds fresh engine because `ReteEngine::reset()` clears rules) — v5.0 Phase 27
- ✓ **DEPLOY-05, EXEC-03**: Real reload-then-swap of the shared `RuleSet` snapshot (`SharedRuleSet{ruleset, generation}` under one `Arc<RwLock>`, swap-don't-mutate with `generation.wrapping_add(1)`); async reload writes `ReloadJob{Pending→Running→Completed/Failed}` to `JobStore` (Pending inserted before spawn) so `GET /v1/jobs/{id}` returns real status; sync reload shares one `reload_and_swap` helper; `build_ruleset_from_dir` parses real `.rules` JSON; batch-execute aggregates truthfully via a `Result<ExecuteResponse,String>` three-arm collector (unknown-engine→failed, panic→JoinError→failed) — v5.0 Phase 25

### Active

v5.0 Live Execution — see `## Current Milestone` below and `.planning/REQUIREMENTS.md`. Closes the `execute_rules` stub and wires execution → trackers → live monitoring data.

### Out of Scope

- Mobile native apps — Web-first, responsive UI serves all user types
- Embedded in-process SDK — Focus on standalone service/deployment modes
- Legacy system migration tools — New implementation focus only
- OAuth/social login — Email/password + JWT auth first; integrate with enterprise auth later

## Current State

**Shipped version:** v4.0 Production Platform (2026-07-02)

**Codebase:** ~31,500 LOC across Rust backend + React/TypeScript frontend; 6 phases (17–22) delivered over 22 days.

**Tech stack:**
- Backend: Rust (actix-web), hybrid Git + PostgreSQL storage
- Frontend: React 19 + TypeScript + Vite + TanStack Router + Zustand, CodeMirror + ag-Grid
- Monitoring: Prometheus `/metrics` endpoint, WebSocket streaming
- Deployment: Multi-stage Alpine Docker, Helm chart (dev/staging/prod), hot-reload with async job tracking

**Known tech debt (deferred, not blocking v4.0):**
- `execute_rules` stub — **RESOLVED in v5.0 Phase 23 (2026-07-03)**: `POST /v1/execute` now fires real rules via `EngineRegistry` (a fresh `Box<dyn ExecutionEngine>` constructed per request through `spawn_blocking`), returns truthful `rules_fired`/`rules_not_fired`/`facts_after` (serialized from post-execution working memory), and auto-populates `CoverageTracking` once per fired rule. The v3.0 `engine::`→`crate::` E0433 import bug is fixed across all four engine modules and `ReteEngine::execute_rules` is a real direct evaluator (mirrored onto TREAT/LEAPS/PHREAK). **Phase 24 (2026-07-03) added live per-execution Prometheus counters + WebSocket `MetricsSnapshot` broadcast, so `/metrics` and the monitoring dashboard now read live execute data. Phase 25 (2026-07-03) made reload real: `reload_and_swap` swaps the shared `RuleSet` snapshot (generation-bumped) under one write lock, async reload writes `ReloadJob` transitions to `JobStore` so `GET /v1/jobs/{id}` works, and batch-execute aggregates truthfully (per-item failure no longer masked as a zero-fired success).** Phase 26 (2026-07-03) made dead-rule detection live: `RuleExecutionTracker` is now a process-wide `LazyLock<Mutex<…>>` singleton (mirroring `CoverageTracking`), `record_evaluation`/`record_fire` wired at both evaluator sites (`direct_eval.rs` + `rete/mod.rs`), and `/v1/metrics/dead-rules` reads from the singleton — no longer always returns `[]`; three keystone e2e tests pin SC-1/SC-2/SC-3. Remaining v5.0 work: stateful sessions (Phase 27), frontend test-runner (Phase 28)
- Pre-existing `cargo build -p engine --all-features` failure on the `metrics`/`prometheus` feature (`prometheus::Opts` API drift at `engine/src/engine/metrics.rs:288-293`) — off the production path (the `api` crate enables only the four engine features, not `metrics`; the `/metrics` endpoint uses hand-rolled Prometheus text). Logged in `.planning/phases/23-execute-evaluator-wiring/deferred-items.md` for a future cleanup phase
- Some v1.0 governance/monitoring phases shipped as stubs originally and were promoted to real implementations across v3.0/v4.0

## Previous Milestones

- **v1.0:** Core Engine Foundation, Rule Authoring Interface, REST API Layer, Governance & Access Control, Testing & Monitoring (stub), DMN Compliance
- **v2.0:** Testing Framework Foundation — Rust testing stack, working memory fixtures, forward/backward chaining verification, snapshot testing, deterministic firing mode
- **v3.0:** Pluggable Engine Architecture — multiple Rete-family algorithms (Rete, TREAT, LEAPS, PHREAK) with runtime selection and engine-specific metrics

## Current Milestone: v5.0 Live Execution

**Goal:** Make rule execution real end-to-end so the v4.0 monitoring surfaces read live data — close the `execute_rules` stub and wire execution → trackers → metrics.

**Target features:**
- Real execution wiring for `POST /v1/execute` (single), `batch-execute`, and async reload (execute against refreshed rules)
- Pluggable engine selection per request (`engine` param) with a config-defined default, exercising the v3.0 Rete/TREAT/LEAPS/PHREAK selector
- Stateless one-shot **and** stateful session execution modes
- Auto-populate `RuleExecutionTracker` + `CoverageTracking` so Prometheus `/metrics`, the monitoring dashboard, dead-rule detection, and coverage reporting show live data
- Frontend test-runner UI in the rule IDE: run/test a rule, see fired rules, results, and coverage

**Progress (4/6 phases):**
- ✅ **Phase 23 — Execute evaluator + wiring** (complete 2026-07-03, verified PASS): fixed the v3.0 `engine::`→`crate::` E0433 import bug across all four engine modules; built a minimal direct rule evaluator on `ReteEngine` (mirrored onto TREAT/LEAPS/PHREAK); wired `POST /v1/execute` through `EngineRegistry` with a fresh `Box<dyn ExecutionEngine>` per request; `ExecuteRequest.engine` validated against `available_engines()` (400 on unknown) with `RRULES_DEFAULT_ENGINE` default; `facts_after` from post-execution working memory; `CoverageTracking` recorded once per fired rule. Requirements met: EXEC-01, EXEC-02, ENGINE-07, ENGINE-08, MON-05
- ✅ **Phase 24 — Execution counters + metrics broadcast** (complete 2026-07-03, verified PASS): added `ExecutionCounters` (`AtomicU64` mirrors + three labeled Prometheus metrics on a dedicated `Registry`) held as `web::Data`; `execute_single` times via `Instant`, increments counters + broadcasts a `MetricsSnapshot` over the existing WS channel AFTER `web::block` (Pitfalls 24-A/B/C); `/metrics` rewritten with `TextEncoder::encode` over the dedicated registry; `ExecuteResponse.engine_generation: u64` added (Pitfall 8). Nyquist RED→GREEN closure (4 RED tests now GREEN); api 96/0, engine 241/0. Frontend dashboards render live numbers with zero frontend changes (SC-3 structural proof). Requirements met: MON-07
- ✅ **Phase 25 — Real reload + JobStore** (complete 2026-07-03, verified PASS, 17/17 must-haves): introduced `SharedRuleSet{ruleset, generation}` under the existing `Arc<RwLock>` (D-02) and migrated the execute-path handlers; implemented the real `.rules` JSON parser in `build_ruleset_from_dir` (R1 — was empty); added `reload_and_swap` (D-04 — builds off-to-side, single write-lock assignment with `generation.wrapping_add(1)`, no I/O under lock); wired `reload_rules` with sync (inline swap) + async (`ReloadJob` Pending→Running→Completed/Failed in `JobStore`, Pending inserted before spawn — Pitfall 14) so `GET /v1/jobs/{id}` returns real status; fixed batch truthful aggregation via a `Result<ExecuteResponse,String>` three-arm collector (Ok(Ok)→succeeded, Ok(Err)→failed+error, Err(JoinError)→failed — L-06) and refactored `execute_single` to `Result<ExecuteResponse,(StatusCode,String)>` (R4). Nyquist RED→GREEN closure: all 7 Wave 0 RED tests now GREEN (8th `#[ignore]`d, passes with `--include-ignored`); api 103/0/1, engine 241/0/3. Requirements met: DEPLOY-05, EXEC-03
- ✅ **Phase 26 — RuleExecutionTracker singleton + dead-rule live data** (complete 2026-07-03, verified PASS, 10/10 must-haves): converted `RuleExecutionTracker` to a `LazyLock<Mutex<TrackerState>>` singleton mirroring `CoverageTracking` (associated-function API, `#[deprecated] new()`, `clear()` for test isolation); wired `record_evaluation`/`record_fire` at BOTH post-fixpoint recording blocks — `direct_eval.rs` (Treat/Leaps/Phreak) and `rete/mod.rs` (C1 — Rete's inlined `execute_rules`); rewrote `dead_rules_endpoint` to read `RuleExecutionTracker::find_dead_rules()` from the singleton (no `new()` per call — no longer always returns `[]`); pinned with 3 `#[serial]` keystone e2e tests (SC-1 dead-rules excludes fired `r1`, SC-2 coverage shows `r1.fired`, SC-3 three surfaces agree — no counter drift). C1 (both evaluator sites) + C4 (engine `metrics.rs` untouched) prohibitions honored. Requirements met: MON-06
- ✅ **Phase 27 — Stateful sessions** (complete 2026-07-06, verified PASS, 12/12 must-haves): `SessionManager` (`Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>>` + alias→UUID DashMap, DashMap 6.2.1 pinned) with per-session `Box<dyn ExecutionEngine>`; `execute_single` branches on `session_id` (D-05 — stateless path unchanged when absent); server UUID v4 primary key + client `session_id` as idempotency alias (D-04); engine fixed at creation, 400 on conflicting resume engine (D-06); snapshot retained with `engine_generation` tag (D-07); four OOM controls shipped together — idle/max-lifetime TTL + LRU + max-count(1024) + 60s `tokio::time::interval` sweep + `rrules_sessions_active` IntGauge on the Phase-24 dedicated Prometheus `Registry` (D-08, Pitfall 24-A); `MetricsSnapshot.active_sessions` from `SessionManager::len()`; `DELETE /v1/execute/session/{id}` + `POST /v1/execute/session/{id}/reset` (reset rebuilds fresh engine — `ReteEngine::reset()` clears rules too, Open Q#1); Pitfall 7 web::block closure (std::mem::replace engine out, drop guard, move into web::block, re-lock to write back). Four `#[serial]` keystone tests (SC-1 score:10→20 accumulation, SC-2 TTL eviction, SC-3 LRU+gauge capped count, SC-4 delete/reset lifecycle) GREEN; api 110/0/1, engine 242/0/3. Requirements met: SESS-01
- ⬜ Phase 28 — Frontend test-runner (FRONTEND-03, FRONTEND-04, EXEC-04)

## Context

rRules targets organizations needing to automate and govern complex business decisions: financial services (risk scoring, compliance), insurance (underwriting, claims), healthcare (clinical pathways), and any domain with evolving decision logic.

Key technical context:
- Rust backend for performance, safety, and concurrency
- React + TypeScript frontend for the web IDE
- Hybrid Git + PostgreSQL storage (Git for versioning rules as code, DB for runtime metadata)
- Multi-mode deployment: cloud-native (Kubernetes) or on-premises
- The engine layer is pluggable, allowing users to select different Rete-family algorithms based on workload characteristics

## Constraints

- **Tech Stack**: Rust for backend, React/TypeScript for frontend — ensures memory safety and type correctness
- **Production Ready**: Must support high availability, monitoring, and governance requirements from day one
- **Algorithm Correctness**: All engine implementations must pass identical functional test suite regardless of algorithm selected

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust as primary language | Memory safety, performance, concurrency without GC pauses | ✓ Good |
| Hybrid Git + DB storage | Git provides natural versioning for rules as code; DB handles runtime metadata and queries | ✓ Good |
| Web IDE first | Browser-native access for all users (business analysts, developers, admins) | ✓ Good |
| Pluggable engine architecture | Different algorithms excel at different workloads; user selection enables optimization | ✓ Good (v3.0) |
| Production frontend focus | Enterprise users need comprehensive web UI for day-to-day operations | ✓ Good (v4.0) |
| Frontend `/v1` API alignment via single VITE_API_URL | One env-driven baseURL + vite proxy eliminated the /api/v1 vs /v1 mismatch that orphaned the UI | ✓ Good (v4.0, Phase 22) |
| JWT auth with alg-pinned HS256 validation | Pragmatic auth without external IdP dependency; pinning alg prevents alg-confusion attacks | ✓ Good (v4.0, Phase 22) |
| Goal-backward verification of frontend integration (Phase 22) | Verified the 9 partial requirements deliver end-to-end against live code, not SUMMARY claims | ✓ Good — 9/9 MET (v4.0) |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-07-06 — Phase 27 complete (stateful sessions via SessionManager); v5.0 Live Execution in progress (5/6 phases)*