# Roadmap: rRules - Business Rules Management System

## Overview

A production-grade Business Rules Management System (BRMS) in Rust: define complex business decisions as reusable rules, deploy them instantly, and monitor their impact without modifying application code. The roadmap progressed from core engine → authoring interface → REST API → governance → testing → DMN compliance → pluggable engines → production platform → live execution → DMN Level 3 compliance.

> **Milestone archives:** Full per-phase detail for completed milestones is preserved in `.planning/milestones/v{X.Y}-ROADMAP.md`.

## Milestones

- ✅ **v1.0 MVP** — Phases 1–6 (shipped 2026-06-07) — Core engine, authoring, REST API, governance, testing, DMN
- ✅ **v2.0 Testing Framework** — Phases 7–9 (shipped 2026-06-07) — Testing infrastructure, advanced features, enhancements
- ✅ **v3.0 Pluggable Engines** — Phase 10 (shipped 2026-06-xx) — Rete/TREAT/LEAPS/PHREAK with runtime selection
- ✅ **v4.0 Production Platform** — Phases 17–22 (shipped 2026-07-02) — Full frontend, governance, monitoring, management API, deployment
- ✅ **v5.0 Live Execution** — Phases 23–28 (shipped 2026-07-06) — Real rule evaluator + `/v1/execute` wiring; auto-populated trackers + metrics so v4.0 monitoring surfaces read live data; stateful sessions + frontend test-runner. Full per-phase detail archived in `milestones/v5.0-ROADMAP.md`.
- 🚧 **v6.0 DMN Level 3 Compliance** — Phases 29–35 (in progress) — Make DMN real: DMN 1.5 parser, complete FEEL expression-language evaluator, decision-table engine with all hit policies, decision-evaluation API, and frontend wiring. Closes the stubbed Phase 6 / `DMN-01..03` gap. Milestone-level success criterion: OMG DMN 1.5 Level 3 conformance (DMN-34 checklist 100% + TCK pass) with the existing Rete rule-execution path unchanged (zero regression).

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
<summary>✅ v5.0 Live Execution (Phases 23–28) — SHIPPED 2026-07-06</summary>

- [x] **Phase 23: Execute evaluator + wiring** — Fix the v3.0 engine import bug, build a minimal real rule evaluator on `ReteEngine`, and route `/v1/execute` through `EngineRegistry`; factory shared, engine instance per request (completed 2026-07-03)
- [x] **Phase 24: Execution counters + metrics broadcast** — `ExecutionCounters` + Prometheus promote + WS broadcast; dashboards go live (completed 2026-07-03)
- [x] **Phase 25: Real reload + JobStore** — Shared reloadable `RuleSet` snapshot; async reload writes `ReloadJob`; batch inherits real execution (completed 2026-07-03)
- [x] **Phase 26: RuleExecutionTracker singleton + dead-rule live data** — Singleton-ize tracker; dead-rules reads live firings; pin with e2e test (completed 2026-07-03)
- [x] **Phase 27: Stateful sessions** — `SessionManager` (DashMap, TTL, LRU, sweep); stateful `/v1/execute/session/{id}` endpoints (completed 2026-07-06)
- [x] **Phase 28: Frontend test-runner** — `execution.ts` + `TestRunner` component; engine selector dropdown; inference trace surfaced (completed 2026-07-06)

</details>

> **v5.0 phase detail archived:** See `.planning/milestones/v5.0-ROADMAP.md` for full per-phase goals, deliverables, pitfalls, and success criteria.

### 🚧 v6.0 DMN Level 3 Compliance (In Progress)

**Milestone Goal:** Make DMN real — deliver a DMN 1.5 parser, a complete FEEL expression-language evaluator, and a decision-table engine with all hit policies, exposed via a decision-evaluation API and wired to the existing frontend editor. Closes the stubbed Phase 6 / `DMN-01..03` gap. Milestone-level success criterion: OMG DMN 1.5 Level 3 conformance (DMN-34 checklist 100% + TCK pass) with the existing Rete rule-execution path unchanged (zero regression).

**Scope locks (user decisions):**

- "Full DMN Level 3 FEEL" — complete FEEL semantics (the ambitious option).
- All four v6.x differentiators in-scope: inference trace, per-decision Prometheus metrics + dashboard, hot-reload, DMNDI round-trip.
- Decision evaluation MVP = FULL §10.4 decision-service support (multi-output) + DRD chaining, NOT single-decision-only.
- New parallel evaluation surface (`engine/src/dmn/` + `POST /v1/decisions/{id}/evaluate`); NOT routed through `/v1/execute` or `EngineRegistry`.
- Build FEEL from scratch (no `dsntk` dependency); stack: `quick-xml 0.40`, `rust_decimal 1.42`, `chrono-tz 0.10`, hand-written recursive-descent FEEL parser.

- [x] **Phase 29: DMN 1.5 Model + XML Parser + Canonical IR** — DMN 1.1–1.5 `.dmn` import into a canonical IR (round-trip foundation) (completed 2026-07-06)
- [x] **Phase 30: FEEL Value Types + Type Lattice + Temporal Domain** — Decimal128, date/time/duration types, type lattice (research-needed) (completed 2026-07-07)
- [ ] **Phase 31: FEEL Parser + Evaluator + Built-in Function Library** — Full FEEL grammar 1–65, ternary logic, 10 boxed expressions, ~70 builtins, AST cache (research-needed; largest phase)
- [ ] **Phase 32: Decision-Table Engine + Hit Policies** — All 7 hit policies + COLLECT aggregators + allowed-values validation
- [ ] **Phase 33: Decision Evaluator + DRD Resolution + REST API + Metrics** — `POST /v1/decisions/{id}/evaluate` (multi-output decision services + DRD chaining), metrics, trace, hot-reload
- [ ] **Phase 34: Frontend Wiring + Hit-Policy Model Refactor** — Wire existing v4.0 editor to the real backend; refactor `HitPolicySelector` to DMN canonical
- [ ] **Phase 35: Conformance Verification + Round-trip + Monitoring Close-out** — OMG DMN TCK pass, exporter round-trip, `DmnDecisionTracker`, Level 3 checklist gate (research-needed)

## Phase Details

### Phase 29: DMN 1.5 Model + XML Parser + Canonical IR

**Goal**: Users can import DMN 1.1–1.5 `.dmn` files into a canonical in-memory IR with annotations, IDs, and element order preserved — the round-trip foundation that every downstream phase parses into or evaluates against.
**Depends on**: Nothing (first phase of v6.0; builds on the existing `engine` crate)
**Requirements**: DMN-04, DMN-05, DMN-06
**Key deliverables**: `engine/src/dmn/model.rs` (DMN 1.5 in-memory model / canonical IR); `engine/src/dmn/parser.rs` (namespace-aware `quick-xml` `NsReader`, XXE-hardened, CDATA-handled, `hitPolicy` defaults to UNIQUE, annotation/description preservation); `engine/src/dmn/exporter.rs` skeleton; item definitions + information items (Clause 7) parsed and represented in the IR; import corpus (DMN 1.1/1.3/1.5 files) passes.
**Success Criteria** (what must be TRUE):

  1. A `.dmn` file from DMN 1.1, 1.3, or 1.5 imports into the canonical IR without error, preserving item definitions, information items, descriptions, annotations, IDs, and element order.
  2. The parser is XXE-hardened — external entity references are rejected — and CDATA sections are handled.
  3. A decision table imported with no explicit `hitPolicy` attribute defaults to UNIQUE.
  4. The canonical IR is the sole store: nothing downstream reconstructs DMN from a non-IR shape (round-trip fidelity is structural from day one).**Plans**: 3/3 plans complete

**Wave 1**

- [x] 29-01-PLAN.md — Canonical DMN 1.5 IR + module wiring + locked stack deps (DMN-05)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 29-02-PLAN.md — Namespace-aware XXE-hardened CDATA-handled DMN parser + import corpus + parser tests (DMN-04, DMN-05)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 29-03-PLAN.md — DMN XML exporter + round-trip DOM-compare tests against import corpus (DMN-06)

### Phase 30: FEEL Value Types + Type Lattice + Temporal Domain

**Goal**: FEEL's semantic value domain — Decimal128 numbers, date/time/duration types, and the type lattice with conformance and implicit conversions — is in place and validated against OMG TCK numeric vectors, so the evaluator can be built on a correct foundation.
**Depends on**: Phase 29 (the IR holds item definitions and allowed value types that the value domain must represent)
**Requirements**: DMN-08, DMN-09, DMN-10
**Key deliverables**: `engine::data_types::Value` extended additively (no fork) with `Date(NaiveDate)`, `Time(NaiveTime)`, `DaysTimeDuration(TimeDelta)`, `YearsMonthsDuration{years,months}`, `Range{...}`, `Function{...}`; `rust_decimal::Decimal` numeric path for FEEL; FEEL type lattice + conformance (`<:`) + equivalence (`≡`) + to/from-singleton-list conversions; temporal lexical formats (ISO 8601 `@""` literals); `0.1 + 0.2 = 0.3` passes.
**Research needed**: Validate 28–29-digit `rust_decimal` precision against OMG TCK numeric conformance vectors BEFORE Phase 31 builds operators on top. If any TCK vector fails, switch to `bigdecimal` here. Run `/gsd-plan-phase --research-phase 30`.
**Success Criteria** (what must be TRUE):

  1. `0.1 + 0.2 = 0.3` holds in the FEEL numeric domain (Decimal128 via `rust_decimal`), validated against OMG TCK numeric vectors — or, if TCK vectors demand 34 digits, a documented fallback to `bigdecimal` is in place before Phase 31.
  2. ISO 8601 `@""` lexical literals for `date`, `time`, `date and time`, `days and time duration`, and `years and months duration` parse and round-trip, with IANA timezones via `chrono-tz` (DST-correct duration arithmetic).
  3. FEEL type-lattice conformance (`<:`), equivalence (`≡`), and implicit conversions (number↔string, singleton-list↔value) behave per spec, with null propagating through the lattice.
  4. `engine::data_types::Value` is extended additively (not forked) and the existing Rete rule-execution path is unchanged (zero regression).

**Plans**: 4/4 plans complete

**Wave 1**

- [x] 30-01-PLAN.md — Foundation: chrono-tz dep + feel/ module skeleton + Value additive extension (8 variants) + TCK vendoring + precision-audit gate (DMN-08 gate, DMN-10 setup)

**Wave 2** *(blocked on Wave 1 completion; 02/03/04 parallel, no file overlap)*

- [x] 30-02-PLAN.md — FEEL Decimal128 numeric domain: feel_numeric_eq + 0.1+0.2=0.3 proof + vendored TCK numeric vectors (DMN-08)
- [x] 30-03-PLAN.md — FEEL temporal domain: parse_at_literal/format_value/add_months_zoned (IANA tz, DST-correct) + vendored TCK temporal vectors (DMN-09)
- [x] 30-04-PLAN.md — FEEL type lattice (conforms/equivalent/3 implicit conversions, 1+"2"→null prohibition) + additive-extension regression audit (DMN-10, Success Criterion 4)

### Phase 31: FEEL Parser + Evaluator + Built-in Function Library

**Goal**: FEEL expressions parse and evaluate with full Level-3 semantics — grammar rules 1–65, three-valued logic, all 10 boxed expressions, ~70+ built-in functions, and user-defined lambdas — with cells pre-parsed to a cached AST so the #1 DMN performance bug (4–6× re-parse throughput loss) is prevented.
**Depends on**: Phase 30 (Value extension + type lattice + temporal domain are the evaluator's semantic foundation)
**Requirements**: DMN-11, DMN-12, DMN-13, DMN-14, DMN-15, DMN-16, DMN-17, DMN-18
**Key deliverables**: `engine/src/dmn/feel/{ast.rs, parser.rs, eval.rs, builtins.rs, temporal.rs, coercion.rs}`; `FeelExpression` enum distinct from the rule `Expression` AST; `evaluate(&Scope)` signature; `feel_equals`/`feel_compare` returning `Option<_>` (null → `Value::Null`); explicit `and`/`or`/`not` truth tables; arithmetic/comparison/boolean/string-concat/date-time operators; `if`/`for`/`some`/`every`/path/filter/`instance of`; all 10 boxed expressions (recursive); boxed-context dependency-order (topological) evaluation with cycle detection; user-defined FEEL lambdas (grammar rule 55); full built-in library (~70+ functions across conversion/string/list/numeric/date-time/context/boolean/range/equality); AST cache primitive (`DashMap<String, Arc<FeelExpression>>`).
**Research needed**: Largest phase — FEEL grammar 1–65, ~70 builtins, 10 boxed expressions, ternary logic, scope/longest-match QN resolution. Likely needs `/gsd-plan-phase --research-phase 31` and may warrant sub-phase splitting during planning. Three-valued-logic leak (Pitfall 1) must be prevented here: never reuse Rust `==`/`compare_values` for FEEL.
**Success Criteria** (what must be TRUE):

  1. Any FEEL expression within grammar rules 1–65 parses via the hand-written recursive-descent parser, with dual entry points (`SimpleUnaryTests`, `Expression`) and keyword-as-name disambiguation.
  2. Three-valued logic is correct: `"a"=1`→null, `null<3`→null, `null=null`→true, `not(null)`→null; `and`/`or`/`not` use explicit truth tables (no Rust `==`/`compare_values` reuse for FEEL equality).
  3. All 10 boxed expression types evaluate recursively; boxed contexts evaluate in dependency (topological) order with cycle detection.
  4. The ~70+ FEEL built-in functions are available across conversion/string/list/numeric/date-time/context/boolean/range/equality families, and user-defined lambdas (grammar rule 55) evaluate.
  5. Cell/expression FEEL is pre-parsed to `Arc<FeelExpression>` at load time; the evaluator takes `&FeelExpression` and never re-parses (a 30-cell table × 10k evals ≈ 30 parses, not 300k).

**Plans**: TBD

### Phase 32: Decision-Table Engine + Hit Policies

**Goal**: Decision tables evaluate correctly under all 7 DMN hit policies — with their aggregators, allowed-values validation, and conflict semantics — input entries evaluated as FEEL unary tests, and all cell FEEL pre-parsed into ASTs at table-load time.
**Depends on**: Phase 31 (input entries are FEEL unary tests; output entries are FEEL expressions)
**Requirements**: DMN-19, DMN-20, DMN-21
**Key deliverables**: `engine/src/dmn/decision_table.rs`; all 7 hit policies (UNIQUE, ANY, PRIORITY, FIRST, RULE ORDER, OUTPUT ORDER, COLLECT); COLLECT aggregators (SUM/MIN/MAX/COUNT); `allowedValues` validation for PRIORITY/OUTPUT ORDER; default output values; `HitPolicyViolation` error type for ANY conflicts and UNIQUE violations; multi-output rejection for aggregator tables; empty-match semantics (`C+`→null, `C#`→0); pre-parse all cell FEEL into ASTs at table-load time (closes the cache loop from Phase 31).
**Success Criteria** (what must be TRUE):

  1. All 7 hit policies (UNIQUE, ANY, PRIORITY, FIRST, RULE ORDER, OUTPUT ORDER, COLLECT) evaluate per Clause 8, with input entries evaluated as FEEL unary tests.
  2. PRIORITY orders by the output column's `allowedValues` enumeration (not by row order); ANY conflicts raise an error rather than silently picking the first match; output `allowedValues` lists and default output values are validated at table load.
  3. COLLECT aggregators (SUM/MIN/MAX/COUNT) compute correctly; multi-output aggregator tables are rejected; empty-match semantics hold (empty SUM/MIN/MAX → null, empty COUNT → 0).
  4. All cell FEEL is pre-parsed into ASTs at table-load time — the evaluator never re-parses a cell on each evaluation.

**Plans**: TBD

### Phase 33: Decision Evaluator + DRD Resolution + REST API + Metrics

**Goal**: Users can evaluate decisions end-to-end via `POST /v1/decisions/{id}/evaluate` — full §10.4 decision-service support with multi-output and DRD chaining (the MVP is multi-output, not single-decision-only) — with per-decision Prometheus metrics, inference trace, and hot-reload wired to the v5.0 observability surfaces.
**Depends on**: Phase 32 (decision-table engine), Phase 29 (canonical IR for persistence)
**Requirements**: DMN-22, DMN-23, DMN-24, DMN-25, DMN-26, DMN-28
**Key deliverables**: `engine/src/dmn/evaluator.rs` (decision orchestrator + topological DRD resolution for multi-output decision-service scope with cycle detection); `api/src/v1/decisions.rs` (`POST /v1/decisions/{id}/evaluate` + CRUD + import/export); `DecisionCounters` on the existing dedicated Prometheus registry (`rrules_dmn_evaluations_total`, `rrules_dmn_decision_duration_seconds`, `rrules_dmn_hit_policy_violations_total`); WS `MetricsSnapshot` broadcast reuse; `TraceEvent` emission with new `event_type` constants (two-site pattern, opt-in via request flag); `SharedDmnModel{model, generation}` under `Arc<RwLock>` with file-watcher reload-then-swap (mirrors v5.0 `SharedRuleSet`); `utoipa` OpenAPI schemas; `DMN_HIT_POLICY_VIOLATION` error code; `web::block` per-request pattern; stateless `DecisionEvaluator` (NOT routed through `EngineRegistry`).
**Success Criteria** (what must be TRUE):

  1. `POST /v1/decisions/{id}/evaluate` resolves a multi-output decision service and returns all output decisions (§10.4), with enclosures (input data, decisions) wired per the decision service.
  2. DRD topological resolution resolves required-decision inputs with cycle detection; downstream decisions evaluate before their dependents.
  3. Decisions support CRUD + persistence (Git `.dmn` files + PostgreSQL metadata) and import/export endpoints.
  4. Per-decision Prometheus counters register on the existing dedicated registry (NOT a second registry); WebSocket `MetricsSnapshot` broadcasts after each evaluate.
  5. Inference trace (`TraceEvent`/`TraceData`, new `event_type` constants) is opt-in via request flag, and `SharedDmnModel{model, generation}` reloads via file-watcher (hot-reload of decisions).

**Plans**: TBD

### Phase 34: Frontend Wiring + Hit-Policy Model Refactor

**Goal**: The existing v4.0 decision-table editor is wired to the real backend evaluator — users can edit, evaluate, and see results/trace in the UI — with the hit-policy data model refactored to DMN canonical and the frontend editing through an IR projection (never reconstructing DMN from the frontend model).
**Depends on**: Phase 33 (the API contract: evaluate, CRUD, import/export, trace)
**Requirements**: DMN-29, DMN-30, DMN-31, DMN-32
**Key deliverables**: `HitPolicySelector` refactor to `{policy, aggregator?}` with FIRST and OUTPUT ORDER added and spec-accurate descriptions (backend enum from Phase 32 is canonical; frontend maps TO it); `dmnSerializer.ts` (editor-shape ↔ backend DMN-model mapper, edit-through-IR-projection); `frontend/src/api/decisions.ts` client; "Evaluate Decision" button + results/trace panel (reuse v5.0 `InferenceTrace` rendering); allowed-values editor for output columns; engine-dropdown hidden on the DMN view.
**Success Criteria** (what must be TRUE):

  1. `DecisionTable.tsx` + `HitPolicySelector.tsx` call the real backend evaluator via an "Evaluate Decision" action; results and trace render in a panel reusing v5.0 `InferenceTrace` rendering.
  2. `HitPolicySelector` uses the `{policy, aggregator?}` data model with FIRST and OUTPUT ORDER added and spec-accurate descriptions; the engine-dropdown is hidden on the DMN view.
  3. `dmnSerializer.ts` maps editor-shape ↔ backend DMN model via edit-through-IR-projection — it never reconstructs DMN from the frontend model (round-trip fidelity preserved).
  4. An allowed-values editor for output columns supports PRIORITY/OUTPUT ORDER usability.

**Plans**: TBD
**UI hint**: yes

### Phase 35: Conformance Verification + Round-trip + Monitoring Close-out

**Goal**: The v6.0 milestone passes the OMG DMN 1.5 Level 3 conformance gate — TCK vectors pass, the exporter round-trips (including DMNDI), monitoring surfaces are live, and the Level 3 conformance checklist is 100% before the v6.0 tag. This phase is the phase-exit gate: no "Level 3-ish" claim is allowed.
**Depends on**: Phases 29–34 (everything must be in place before the gate)
**Requirements**: DMN-07, DMN-27, DMN-33, DMN-34
**Key deliverables**: OMG DMN TCK test vectors as the acceptance suite (import + evaluation) with documented known divergences (e.g. DMN15-31 Collect distinct-sum, `sum([])`) if any; `engine/src/dmn/exporter.rs` round-trip (model → `.dmn` XML preserving annotations, DMNDI layout, IDs, element order; byte/DOM-compare against the import corpus; XSD-valid output); `DmnDecisionTracker` (separate from `RuleExecutionTracker`) powering dead-decision-row detection, coverage extension, and a dashboard panel reading the new metrics; Level 3 conformance checklist (one item per built-in family, grammar rule, boxed expression type) as phase-exit criterion.
**Research needed**: OMG DMN TCK availability and distribution format (TC archive vs public repo) needs confirmation during planning. If the TCK is unavailable, fall back to DMN 1.5 spec examples + Camunda feel-scala test corpus and document the gap. Run `/gsd-plan-phase --research-phase 35`.
**Success Criteria** (what must be TRUE):

  1. OMG DMN TCK test vectors pass as the acceptance suite (import + evaluation); known divergences (e.g. DMN15-31 Collect distinct-sum, `sum([])`) are documented.
  2. The DMN XML exporter serializes the canonical IR back to `.dmn` XML preserving annotations, descriptions, IDs, element order, and DMNDI layout (DMN-07 round-trip); round-trip byte/DOM-compare passes against the import corpus; output is XSD-valid.
  3. `DmnDecisionTracker` (separate from `RuleExecutionTracker`) powers dead-decision-row detection, coverage extension, and a dashboard panel reading the new metrics — `RuleExecutionTracker`/`CoverageTracking` are not contaminated (separate `source` tag or tracker).
  4. The Level 3 conformance checklist (one item per built-in family, grammar rule, and boxed expression type) is 100% complete and is the phase-exit gate — the v6.0 tag is not cut until the checklist is complete and the TCK passes.
  5. The existing Rete rule-execution path (`/v1/execute`, `EngineRegistry`) is unchanged — zero regression.

**Plans**: TBD
**UI hint**: yes

## Progress

**Execution Order:**
Phases execute in numeric order: 29 → 30 → 31 → 32 → 33 → 34 → 35.

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
| 23. Execute evaluator + wiring | v5.0 | 6/6 | Complete | 2026-07-03 |
| 24. Execution counters + metrics broadcast | v5.0 | 3/3 | Complete | 2026-07-03 |
| 25. Real reload + JobStore | v5.0 | 3/3 | Complete | 2026-07-03 |
| 26. RuleExecutionTracker singleton + dead-rule live data | v5.0 | 2/2 | Complete | 2026-07-03 |
| 27. Stateful sessions | v5.0 | 1/1 | Complete | 2026-07-06 |
| 28. Frontend test-runner | v5.0 | 5/5 | Complete | 2026-07-06 |
| 29. DMN 1.5 Model + XML Parser + Canonical IR | v6.0 | 3/3 | Complete   | 2026-07-06 |
| 30. FEEL Value Types + Type Lattice + Temporal Domain | v6.0 | 4/4 | Complete   | 2026-07-07 |
| 31. FEEL Parser + Evaluator + Built-in Function Library | v6.0 | 0/TBD | Not started | - |
| 32. Decision-Table Engine + Hit Policies | v6.0 | 0/TBD | Not started | - |
| 33. Decision Evaluator + DRD Resolution + REST API + Metrics | v6.0 | 0/TBD | Not started | - |
| 34. Frontend Wiring + Hit-Policy Model Refactor | v6.0 | 0/TBD | Not started | - |
| 35. Conformance Verification + Round-trip + Monitoring Close-out | v6.0 | 0/TBD | Not started | - |

---
*Roadmap last updated: 2026-07-06 — v6.0 DMN Level 3 Compliance roadmap added (Phases 29–35); v5.0 per-phase detail archived in `milestones/v5.0-ROADMAP.md`*
