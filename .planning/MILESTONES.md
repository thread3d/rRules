# Milestones

## v5.0 Live Execution (Shipped: 2026-07-06)

**Closeout:** verified — all 6 phases verified PASS (milestone audit `passed` after debt-fix), 0 open artifacts, 0 blockers, 13/13 requirements satisfied. The milestone audit's 2 cross-phase WARNINGs (`rule_filter` dead input; `/metrics` HTML/JSON `active_sessions` hardcoded) were resolved in commit `384b1f17d` and verified (cargo 118/0, vitest 5/5) before tagging — see `.planning/milestones/v5.0-MILESTONE-AUDIT.md` (re-audit `passed`).
**Scope:** Phases 23–28 (6 phases, 20 plans, 43 tracked tasks)
**Timeline:** 2026-07-02 → 2026-07-06 (4 days, 157 commits)
**Codebase:** Rust backend (engine + actix-web API) + React/TypeScript frontend

**Delivered:** Real end-to-end rule execution — `POST /v1/execute` fires real rules via `EngineRegistry` (a fresh `Box<dyn ExecutionEngine>` per request through `spawn_blocking`), returns truthful `rules_fired`/`rules_not_fired`/`facts_after`, and feeds live data to the v4.0 monitoring surfaces (Prometheus `/metrics`, dashboard, dead-rule, coverage). Stateful sessions (`SessionManager`) and a frontend test-runner make live execution a user-facing feature. The long-standing `rules_fired: 0` stub is closed.

**Phases completed:** 6 phases, 20 plans, 43 tasks

**Key accomplishments:**

- Fixed the E0433 import bug across all four engine modules, extended ExecutionEngine with load_ruleset/get_facts default methods + EngineResult.fired_rule_ids, and registered all four factories so available_engines() truthfully returns rete/treat/leaps/phreak — the pluggable-engine surface now compiles for the first time and the api crate links all four factories.
- Built the `Expression` AST (D-06) so `score := score + 10` is representable in `Action::UpdateFact`, plus a shared `compare_values` helper and a rewritten `Condition::evaluate` that handles the full `ComparisonOp` matrix across Integer/Float/String/Boolean and matches `Pattern` conditions against real fact payloads instead of returning unconditionally true.
- Rewrote the placeholder `ReteEngine::execute_rules` into a real direct evaluator that actually fires rules — the keystone of Phase 23. A `score := score + 10` rule now yields `rules_fired >= 1` and `get_facts()` showing `score == 10`. Coverage records once per fired rule (Pitfall 6 fixed), forward chaining iterates to fixpoint under `RRULES_MAX_CHAIN_ITERATIONS`, and `EngineResult.fired_rule_ids` is populated.
- Renamed `metrics::EngineState` → `MetricsSnapshot` (Pitfall 10) to disambiguate from `execution::EngineState` before Plan 23-05 adds execute-path logic, and landed the Wave 0 test scaffolds: a serial-gated `record_once_per_rule` coverage regression test (GREEN — Plan 23-03's evaluator fires rules truthfully) and five RED integration scaffolds in `api/tests/execute_integration.rs` encoding the four ROADMAP success criteria (EXEC-01/02, ENGINE-07/08, MON-05), installed against the full Plan 23-05 app_data stack so they go GREEN the moment `execute_single` is rewired.
- Rewrote `execute_single` so POST /v1/execute constructs a fresh `Box<dyn ExecutionEngine>` per request via `EngineRegistry::create_engine_by_name`, loads the shared read-only `Arc<RwLock<RuleSet>>`, inserts request facts, runs the real ReteEngine evaluator inside `web::block`, and serializes `facts_after` from post-execution `get_facts()` via a reverse `engine_value_to_json` mapping — turning all five Plan 23-04 RED scaffolds GREEN and satisfying all four ROADMAP success criteria (rules fire truthfully, facts_after is the incremented value not the echoed input, engine param validated with 400 on unknown / env-default on omitted, coverage auto-populates with once-per-rule fire_count).
- Rolled the real direct rule evaluator out to Treat, Leaps, and Phreak — `engine: "treat"|"leaps"|"phreak"` now actually fires rules instead of returning `rules_fired: 0`. Factored the engine-agnostic evaluate-and-apply loop into a shared `engine::direct_eval` module (3x duplication -> one shared impl + Rete's inlined reference), and pinned each engine with a serial-gated score+10 keystone test. The four-engine guarantee in `available_engines()` is honest, not merely compile-clean.
- RuleExecutionTracker converted from a locally-instantiated `&mut self` struct into a process-wide `LazyLock<Mutex<TrackerState>>` singleton mirroring CoverageTracking, with associated-function API, deprecated `new()`, internal chrono timestamp stamping, and 7 passing `#[serial]` + `clear()`-isolated unit tests including the `test_once_per_rule` Pitfall 6 regression guard.
- Wired RuleExecutionTracker recording into BOTH evaluator call sites (direct_eval.rs for Treat/Leaps/Phreak AND rete/mod.rs for Rete — C1 correction), rewrote dead_rules_endpoint to read live singleton data via find_dead_rules(), and pinned SC-1/SC-2/SC-3 with three keystone e2e tests using the r2-unfired-rule trick — dead-rules panel now returns live data and all three metric surfaces agree on fire count.
- SessionManager with DashMap-sharded per-session Box<dyn ExecutionEngine>, TTL+LRU+max-count+sweep+gauge shipped together — four SC tests GREEN (accumulate, TTL, LRU+gauge, lifecycle).

**Archived:** `.planning/milestones/v5.0-ROADMAP.md`, `.planning/milestones/v5.0-REQUIREMENTS.md`, `.planning/milestones/v5.0-MILESTONE-AUDIT.md`
**Tag:** `v5.0`

---

## v4.0 Production Platform (Shipped: 2026-07-02)

**Closeout:** verified — all milestone phases verified PASS, 0 open artifacts, 0 deferred items.
**Scope:** Phases 17–22 (6 phases, ~12 plans, 27 tracked tasks)
**Timeline:** 2026-06-10 → 2026-07-02 (22 days, 89 commits)
**Codebase:** ~31,500 LOC (Rust + TypeScript)

**Delivered:** A complete production-ready BRMS platform — full React/TypeScript web frontend wired end-to-end to a Rust backend with enterprise governance, Prometheus monitoring, complete management API, and Docker/Kubernetes deployment with hot reload.

**Key accomplishments:**

- Production React/TypeScript frontend — CodeMirror rule-authoring IDE, decision-table editor with all DMN hit policies, guided editors, and rule-management dashboard with filtering/sorting (Phase 17)
- Enterprise governance — full RBAC (admin/author/reviewer/viewer) with JWT auth, maker-checker approval workflow, immutable audit trail with CSV/JSON export, and Git-backed branching/merging with conflict resolution (Phase 18)
- Monitoring & metrics — Prometheus `/metrics` endpoint (HTML/JSON/text), dashboard API (engine-state/health), dead-rule detection with configurable thresholds, and coverage reporting reading real tracker structures (Phase 19)
- Complete management API — rules/categories/tags CRUD, approvals, audit, and branches endpoints (Phase 20)
- Deployment & CI/CD — multi-stage Alpine Docker image, Helm chart with dev/staging/prod values, and hot-reload mechanism with async job tracking (Phase 21)
- Frontend-backend integration closure — aligned frontend to `/v1` (VITE_API_URL + vite proxy + actix-cors), JWT-protected routes with AuthProvider, mounted all orphaned routes, real WebSocket metrics streaming, audit/approval/branch UI wired to backend; 9/9 partial requirements verified MET end-to-end (Phase 22)

**Archived:** `.planning/milestones/v4.0-ROADMAP.md`, `.planning/milestones/v4.0-REQUIREMENTS.md`
**Tag:** `v4.0`

---
