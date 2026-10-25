# rRules - Business Rules Management System

## What This Is

rRules is a production-grade Business Rules Management System (BRMS) built in Rust. It provides a complete platform for defining, deploying, and managing business decision logic — from rule authoring through execution, governance, and monitoring — with a full React/TypeScript web frontend, enterprise governance controls, Prometheus monitoring, and Docker/Kubernetes deployment.

**Shipped:** v8.0 Close TCK Evaluator Gaps (2026-07-15), on top of v7.0 Performance + Production Readiness (2026-07-15), v6.0 DMN Level 3 Compliance (2026-07-10) and v5.0 Live Execution (2026-07-06). The platform now delivers end-to-end: authors write rules in a browser IDE, governance controls who can change them and how those changes are approved, the engine **actually executes them** (real rule evaluator + `/v1/execute`), Prometheus/metrics dashboards + a frontend test-runner monitor and exercise their live behaviour, and a real DMN 1.5 parser + FEEL evaluator + decision-table engine now pass **51/51 OMG DMN TCK evaluator vectors** (123/123 layer passes, zero regression).

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
- ✓ **MON-06**: Live dead-rule detection — `RuleExecutionTracker` as a process-wide `LazyLock<Mutex<…>>` singleton (mirroring `CoverageTracking`), `record_evaluation`/`record_fire` wired at both evaluator sites (`direct_eval.rs` + `rete/mod.rs`); `/v1/metrics/dead-rules` reads live firings (no longer always `[]`); three surfaces agree on fire count — v5.0 Phase 26
- ✓ **EXEC-04, FRONTEND-04**: Inference trace (`TraceEvent`/`TraceData`, two-site emission) opt-in via `ExecuteRequest.trace` + `GET /v1/engines` engine registry; trace surfaced in the frontend results panel only when requested (latency unaffected when off) — v5.0 Phase 28
- ✓ **FRONTEND-03**: Frontend test-runner — `execution.ts` API client + `TestRunner` component (facts editor, engine dropdown, Run + Reload-then-Run, stacked results, trace toggle); `rule_filter` now honored server-side on the stateless path (commit `384b1f17d`) for a true "test this rule" UX — v5.0 Phase 28
- ✓ **DMN-08, DMN-09, DMN-10**: FEEL semantic value domain — Decimal128 numeric path via `rust_decimal` (`0.1 + 0.2 = 0.3` proven, no f64 in production path); temporal types (date/time/date-and-time/days-time-duration/years-months-duration) with ISO 8601 `@""` lexical round-trip, IANA timezones via `chrono-tz`, DST-correct duration arithmetic (gap→null, ambiguous→earliest); type lattice with conformance (`<:`), equivalence (`≡`), exactly the three spec-defined implicit conversions (to-singleton-list, from-singleton-list, conforms-to — no number↔string; `1 + "2" → null` prohibition) and null propagation; `engine::data_types::Value` extended additively with 8 FEEL variants and zero Rete regression; validated against vendored OMG TCK numeric + temporal vectors — v6.0 Phase 30

### Active

**None — milestone v8.0 is complete (2026-07-15).** The next milestone is not yet
scoped. Candidate work, drawn from the recorded backlog in
`.planning/TECH-DEBT.md`, `TCK-COVERAGE-GAPS.md`, and the v8.0 audit:

- **DMN 1.6 / FEEL 1.6 extensions** — the one documented TCK divergence
  (DMN15-31 COLLECT with `distinct`) requires DMN 1.6 semantics.
- ~~**Engine retraction correctness**~~ — resolved 2026-10-25: `retract_fact()`
  now removes facts from working/alpha/beta memory in all three engines (see
  `.planning/TECH-DEBT.md`).
- **Multi-tenancy finish** — `TenantStore` exists but no `/v1/tenants` routes and
  rule-CRUD isolation is incomplete.
- **Postgres persistence** — `engine_state::PostgresDmnStore` is a skeleton
  (`todo!()` bodies); `DMN_STORAGE_PATH` file store is authoritative.

See `.planning/STATE.md` and `.planning/MILESTONES.md` for shipped milestones.

### Out of Scope

- Mobile native apps — Web-first, responsive UI serves all user types
- Embedded in-process SDK — Focus on standalone service/deployment modes
- Legacy system migration tools — New implementation focus only
- OAuth/social login — Email/password + JWT auth first; integrate with enterprise auth later

## Current State

**Shipped version:** v8.0 Close TCK Evaluator Gaps (2026-07-15); prior v7.0 Performance + Production Readiness (2026-07-15), v6.0 DMN Level 3 Compliance (2026-07-10), v5.0 Live Execution (2026-07-06), v4.0 Production Platform (2026-07-02).

**Codebase:** Rust backend (engine + actix-web API) + React/TypeScript frontend; ~55k lines of Rust across 3 crates (~1,000 test functions) and ~6.5k lines of TS/TSX. Phases 1–43 across nine milestones; 473 commits at v8.0 close.

**Tech stack:**
- Backend: Rust (actix-web), hybrid Git + PostgreSQL storage
- Frontend: React 19 + TypeScript + Vite + TanStack Router + Zustand, CodeMirror + ag-Grid, Vitest
- Monitoring: Prometheus `/metrics` endpoint (HTML/JSON/text, dedicated registry), WebSocket `MetricsSnapshot` streaming, live dead-rule + coverage surfaces
- Deployment: Multi-stage Alpine Docker, Helm chart (dev/staging/prod), hot-reload with async job tracking
- Execution: real direct rule evaluator on Rete/TREAT/LEAPS/PHREAK (fresh `Box<dyn ExecutionEngine>` per request), stateful sessions (`SessionManager`), frontend test-runner
- DMN: DMN 1.5 XML parser + canonical IR + exporter (zero-copy `quick-xml`), full FEEL evaluator (`rust_decimal` Decimal128, `chrono`/`chrono-tz` temporal domain), decision-table engine with all 7 hit policies, DRD resolution with rayon parallel levels, per-thread AST cache, `/v1/decisions` API; 51/51 OMG DMN TCK evaluator vectors passing

**Known tech debt (deferred, non-blocking — logged in the v5.0 milestone audit, `.planning/milestones/v5.0-MILESTONE-AUDIT.md`):**
- `execute_rules` stub — **FULLY RESOLVED in v5.0 (Phases 23–28, 2026-07-03 → 2026-07-06)**: `POST /v1/execute` fires real rules via `EngineRegistry`; `facts_after` is post-execution working memory (not echoed input); `CoverageTracking` records once per fired rule; live Prometheus counters + WS `MetricsSnapshot` broadcast; real reload-then-swap with `JobStore`; `RuleExecutionTracker` singleton makes dead-rule detection live; stateful sessions; frontend test-runner. The v5.0 milestone audit's 2 cross-phase WARNINGs (`rule_filter` dead input; `/metrics` HTML/JSON `active_sessions` hardcoded) were **resolved post-audit in commit `384b1f17d`** and verified (cargo 118/0, vitest 5/5) — see the re-audit (status `passed`).
- Pre-existing `cargo build -p engine --all-features` failure on the `metrics`/`prometheus` feature (`prometheus::Opts` API drift at `engine/src/engine/metrics.rs:288-293`) — off the production path (the `api` crate enables only the four engine features, not `metrics`; the `/metrics` endpoint uses hand-rolled Prometheus text / the Phase-24 dedicated registry). Logged in `.planning/phases/23-execute-evaluator-wiring/deferred-items.md` for a future cleanup phase.
- Legacy dead duplicate `ExecuteRequest` at `api/src/models/execution.rs:8` (no usages; live model is `api::v1::execution::ExecuteRequest`) — safe-to-delete cleanup surfaced during the audit debt fix.
- Pre-existing engine TODO `rete/mod.rs:557` (retraction network cleanup, Phase 10 era) + `treat/mod.rs` dead-code warnings — no functional impact.
- Some v1.0 governance/monitoring phases shipped as stubs originally and were promoted to real implementations across v3.0/v4.0

## Previous Milestones

- **v1.0:** Core Engine Foundation, Rule Authoring Interface, REST API Layer, Governance & Access Control, Testing & Monitoring (stub), DMN Compliance
- **v2.0:** Testing Framework Foundation — Rust testing stack, working memory fixtures, forward/backward chaining verification, snapshot testing, deterministic firing mode
- **v3.0:** Pluggable Engine Architecture — multiple Rete-family algorithms (Rete, TREAT, LEAPS, PHREAK) with runtime selection and engine-specific metrics

## Current Milestone: v8.0 Close TCK Evaluator Gaps — COMPLETE (2026-07-15)

**Goal (met):** raise OMG DMN TCK evaluator coverage from 21/51 to 51/51 vectors
by implementing the missing FEEL built-in functions across numeric, string,
list, context, relational, and temporal families, with zero regression in the 30
vectors that already parsed and round-tripped.

**Delivered:**
- Numeric family (Phase 40) — `abs()` duration support, `number(from, grouping, decimal)`
  3-arg form, named-parameter resolution, `median`/`mode`/`stddev`/`product`, arithmetic
  edge cases.
- String + list family (Phase 41) — `sort()` comparator lambdas, empty-list guard,
  `split`/`filter`/`concatenate`/`insert`/`remove` edge cases and null propagation.
- Context + relational + temporal families (Phase 42) — verified pre-existing
  Phase-31 implementations against the TCK; no code changes required.
- Conformance close-out (Phase 43) — final sweep 51/51 vectors / 123/123 layer
  passes (compliance-level-2 110/110, compliance-level-3 13/13), audit `passed`,
  `TCK-COVERAGE-GAPS.md` documenting the single out-of-scope divergence.

**Artifacts:** `.planning/milestones/v8.0-ROADMAP.md`,
`.planning/milestones/v8.0-MILESTONE-AUDIT.md`, `TCK-COVERAGE-GAPS.md`,
`.planning/phases/43-tck-conformance-verification-close-out/VERIFICATION.md`.

**Next milestone:** unscoped — see "Active" above for the candidate backlog.

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
| Per-request engine via `web::block` (Phase 23) | Fresh `Box<dyn ExecutionEngine>` constructed per request from the shared `EngineRegistry` factory — avoids `&mut self` engine-sharing serialization (Pitfall 1/9); `ReteEngine: !Clone` so no pool | ✓ Good (v5.0) |
| `RuleExecutionTracker` + `CoverageTracking` as `LazyLock<Mutex<…>>` singletons (Phase 26) | Process-wide single source of truth for fire/eval recording; dead-rules + coverage read live data; `#[serial]` + `clear()` for test isolation | ✓ Good (v5.0) |
| `SessionManager` — TTL+LRU+max-count+sweep+gauge ship together (Phase 27) | The OOM failure mode is silent; all four controls + the Prometheus gauge ship as one unit (Pitfall 5); server UUID v4 primary key + client `session_id` as idempotency alias | ✓ Good (v5.0) |
| `Cow<'a, RuleSet>` zero-clone `rule_filter` (Phase 28 debt fix) | `Borrowed` on the no-filter hot path (zero alloc), `Owned` only when filtering; honors the frontend "test this rule" UX on the stateless path; `Option<web::Data<…>>` extractor keeps minimal test apps backward-compatible | ✓ Good (v5.0) |
| Milestone audit → debt-fix → re-audit closeout (v5.0) | Initial audit flagged 2 cross-phase WARNINGs as `tech_debt`; fixed both + verified (cargo 118/0, vitest 5/5) before tagging; re-audit `passed` | ✓ Good (v5.0) |

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
*Last updated: 2026-10-25 — Milestone v8.0 COMPLETE and tagged: 51/51 DMN TCK evaluator vectors pass, audit passed, zero regression. v7.0 (12/12 PERF satisfied) and v6.0 shipped before it.*