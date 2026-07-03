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
- `execute_rules` stub — **RESOLVED in v5.0 Phase 23 (2026-07-03)**: `POST /v1/execute` now fires real rules via `EngineRegistry` (a fresh `Box<dyn ExecutionEngine>` constructed per request through `spawn_blocking`), returns truthful `rules_fired`/`rules_not_fired`/`facts_after` (serialized from post-execution working memory), and auto-populates `CoverageTracking` once per fired rule. The v3.0 `engine::`→`crate::` E0433 import bug is fixed across all four engine modules and `ReteEngine::execute_rules` is a real direct evaluator (mirrored onto TREAT/LEAPS/PHREAK). Remaining v5.0 work: auto-populate `RuleExecutionTracker` + Prometheus counters/WS broadcast (Phase 24), real reload + batch execution (Phase 25), dead-rule live data (Phase 26), stateful sessions (Phase 27), frontend test-runner (Phase 28)
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

**Progress (1/6 phases):**
- ✅ **Phase 23 — Execute evaluator + wiring** (complete 2026-07-03, verified PASS): fixed the v3.0 `engine::`→`crate::` E0433 import bug across all four engine modules; built a minimal direct rule evaluator on `ReteEngine` (mirrored onto TREAT/LEAPS/PHREAK); wired `POST /v1/execute` through `EngineRegistry` with a fresh `Box<dyn ExecutionEngine>` per request; `ExecuteRequest.engine` validated against `available_engines()` (400 on unknown) with `RRULES_DEFAULT_ENGINE` default; `facts_after` from post-execution working memory; `CoverageTracking` recorded once per fired rule. Requirements met: EXEC-01, EXEC-02, ENGINE-07, ENGINE-08, MON-05
- ⬜ Phase 24 — Execution counters + metrics broadcast (MON-07)
- ⬜ Phase 25 — Real reload + JobStore (DEPLOY-05, EXEC-03)
- ⬜ Phase 26 — RuleExecutionTracker singleton + dead-rule live data (MON-06)
- ⬜ Phase 27 — Stateful sessions (SESS-01)
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
*Last updated: 2026-07-03 — Phase 23 complete (execute evaluator + wiring); v5.0 Live Execution in progress (1/6 phases)*