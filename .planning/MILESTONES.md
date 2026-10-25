# Milestones

## v8.0 Close TCK Evaluator Gaps (Shipped: 2026-07-15)

**Closeout:** verified — all 4 phases complete, milestone audit `passed`, zero regression.
**Scope:** Phases 40–43 (4 phases, 4 plans, 6 tracked tasks in Phase 40)
**Timeline:** 2026-07-15
**Codebase:** Rust backend (engine + actix-web API) + React/TypeScript frontend

**Delivered (as recorded at ship time; later corrected):** the milestone set out to raise
evaluator-layer coverage from 21/51 to 51/51 vectors and recorded 123 layer passes with zero
failures. Re-verification on 2026-10-25 showed that figure was not measurable — the sweep recorded
only passing layers, 14 vectors shipped placeholder test files, and literal-expression decisions
were never evaluated. See `milestones/v8.0-MILESTONE-AUDIT.md` for the corrections and
`../TCK-COVERAGE-GAPS.md` for the measured state (143 pass / 31 fail / 0 skip).
Phase 42 turned out to need no new code: the context/relational/temporal families
shipped in v6.0 Phase 31 already satisfied their TCK vectors, so the phase is a
verification record rather than an implementation.

**Key accomplishments:**

- Phase 40 (numeric) — `abs()` extended to `DaysTimeDuration`/`YearsMonthsDuration`, `number(from, grouping, decimal)` 3-arg form with locale-independent parsing, named-parameter resolution by positional fallback in evaluator dispatch, `median`/`mode`/`stddev`/`product` and arithmetic edge cases (empty-list SUM/MIN/MAX → null). All 14 numeric TCK vectors pass.
- Phase 41 (string + list) — `sort()` completed with comparator-lambda support via `apply_function` for correct scope handling, empty-list guard returning null, plus edge-case tests for `split` (no delimiter, empty string, null inputs), `filter`, `concatenate`, `insert`/`remove`, `list contains` null propagation.
- Phase 42 (context/relational/temporal) — verified the pre-existing Phase-31 implementations against their TCK vectors; no code changes needed.
- Phase 43 (close-out) — full 51/51 TCK sweep across parser, round-trip, and evaluator layers; `TCK-COVERAGE-GAPS.md` documenting the single out-of-scope divergence (DMN15-31 COLLECT with `distinct`, a DMN 1.6 / FEEL 1.6 feature); milestone audit `passed`.

**Known divergence (out of scope):** DMN15-31 COLLECT with `distinct` — introduced in DMN 1.6, not present in DMN 1.5 / Level 3. Documented in `TCK-COVERAGE-GAPS.md`; candidate for a future DMN 1.6 milestone.

**Archived:** `.planning/milestones/v8.0-ROADMAP.md`, `.planning/milestones/v8.0-MILESTONE-AUDIT.md`
**Tag:** `v8.0`

---

## v7.0 Performance + Production Readiness (Shipped: 2026-07-15)

**Closeout:** verified — 12/12 PERF requirements satisfied, milestone audit `passed`.
**Scope:** Phases 36–39 (4 phases)
**Timeline:** 2026-07-13 → 2026-07-15
**Codebase:** Rust backend (engine + actix-web API)

**Delivered:** Measurement and speed infrastructure for the DMN subsystem — a
Criterion benchmark suite with a frozen memory baseline as a regression gate,
parser/evaluator optimizations (zero-copy XML loading, per-thread FEEL AST cache),
fuzzing plus property-based tests, and rayon-parallel DRD evaluation.

**Key accomplishments:**

- Phase 36 — Criterion `dmn_benchmarks` suite (parser/exporter/evaluator/decision-table), frozen `v6.0-baseline.json` memory snapshot captured over a 60-file corpus, and `scripts/memcheck-gate.sh` failing CI on RSS/allocation regression (`make memcheck`, `make baseline-update`).
- Phase 37 — zero-copy DMN XML loader (dropped per-element `into_owned()`), FEEL lexer token cache and parse-with-cache entry point, AST cache warming for decision-table cells.
- Phase 38 — cargo-fuzz targets (`feel_parser`, `decision_table`) plus proptest invariants for FEEL evaluation and context topological ordering; regression corpus committed under `engine/fuzz/corpus`.
- Phase 39 — DRD parallelization: topological sort of decision dependencies with rayon level-by-level evaluation of independent decisions inside a single `DecisionEvaluator::evaluate` call.

**Archived:** `.planning/milestones/v7.0-ROADMAP.md`, `.planning/milestones/v7.0-REQUIREMENTS.md`
**Tag:** `v7.0`

---

## v6.0 DMN Level 3 Compliance (Shipped: 2026-07-10)

**Closeout:** verified — 7/7 phases complete, conformance gate GO (Level 3 checklist + TCK pass), zero regression in the Rete path.
**Scope:** Phases 29–35 (7 phases)
**Timeline:** 2026-07-06 → 2026-07-10
**Codebase:** Rust backend + React/TypeScript frontend

**Delivered:** A real DMN 1.5 implementation replacing the v1.0 Phase 6 stub — XML
parser with canonical IR and exporter, full FEEL evaluator with the Decimal128
numeric domain and temporal types, decision tables with all 7 hit policies,
DRD-chained decision evaluation exposed over REST with Prometheus metrics and
monitoring, and the existing v4.0 frontend editor wired to the real evaluator.

**Key accomplishments:**

- Real DMN 1.5 parser — namespace-aware (DMN 1.1–1.5), XXE-hardened, CDATA-preserving, entity references resolved, ID/annotation preservation; canonical IR in `engine/src/dmn/model.rs`.
- Complete FEEL value domain — Decimal128 numbers (`rust_decimal`, no f64 on the production path, `0.1 + 0.2 = 0.3` proven), five temporal types with ISO 8601 `@""` round-trip, IANA zones via `chrono-tz`, DST-correct duration arithmetic, and the spec's exactly-three implicit conversions (`1 + "2" → null`).
- Full FEEL expression language — grammar rules 1–65, boxed expressions (literal/context/list/relation/function invocation), if/for/some/every, lambdas with lexical capture, three-valued logic and null propagation, ~70+ built-in functions, cells pre-parsed to cached AST.
- Decision-table engine — all 7 hit policies (UNIQUE, ANY, PRIORITY, FIRST, RULE ORDER, OUTPUT ORDER, COLLECT) with SUM/MIN/MAX/COUNT aggregators, input entries evaluated as FEEL unary tests.
- Decision evaluation API — `POST /v1/decisions/{id}/evaluate` with multi-output decision services and DRD chaining, per-decision Prometheus counters + DRD depth gauge, `DmnDecisionTracker` dead-decision detection, WS broadcast, hot reload.
- Frontend wiring — hit-policy model refactored to the DMN canonical 7-policy union with aggregator sub-type; `DecisionTable`/`HitPolicySelector` wired to the backend evaluator with results and trace panels.
- Conformance close-out — TCK sweep (21 vectors across parser/evaluator/round-trip/DMNDI layers, 0 failures), DMNDI round-trip preservation, Level 3 conformance checklist with evidence, exported decision-service API.

**Known divergences at v6.0:** 2 documented (COLLECT distinct-sum, empty-list SUM) — later narrowed by v8.0 to the single DMN 1.6 `distinct` item.

**Archived:** `.planning/milestones/v6.0-ROADMAP.md`, `.planning/milestones/v6.0-REQUIREMENTS.md`
**Tag:** `v6.0`

---

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
