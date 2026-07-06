# Requirements: rRules v6.0 — DMN Level 3 Compliance

**Defined:** 2026-07-06
**Core Value:** Users can define complex business decisions as reusable rules, deploy them instantly, and monitor their impact — all without modifying application code.

**Scope note:** "Full DMN Level 3 FEEL" selected → complete FEEL semantics. All four v6.x differentiators (trace, metrics+dashboard, hot-reload, DMNDI round-trip) are in-scope. Decision evaluation MVP is **full decision-service support (§10.4 multi-output)**, not single-decision-only. Requirement IDs continue the `DMN-XX` series; `DMN-01..03` (PROJECT.md "Validated v1.0") are the stubbed parent requirements this milestone makes genuinely true — they are not re-listed here.

## v1 Requirements

Requirements for the v6.0 milestone. Each maps to roadmap phases 29–35.

### DMN Model & Interchange

- [x] **DMN-04**: DMN 1.5 XML parser — namespace-aware (accepts DMN 1.1–1.5), XXE-hardened, CDATA-handled; imports `.dmn` files into a canonical in-memory IR; `hitPolicy` defaults to UNIQUE
- [x] **DMN-05**: DMN item definitions + information items (Clause 7) parsed and represented in the canonical IR
- [ ] **DMN-06**: DMN XML exporter — serializes the canonical IR back to `.dmn` XML preserving annotations, descriptions, IDs, and element order; round-trip byte/DOM-compare passes against the import corpus
- [ ] **DMN-07**: DMNDI diagram layout round-trip — parser preserves DMNDI layout and exporter emits it unchanged (no rendering) for external-tool interop *(differentiator)*

### FEEL Value System

- [ ] **DMN-08**: FEEL Decimal128 numeric domain (`rust_decimal`) — `0.1 + 0.2 = 0.3`; precision validated against OMG TCK numeric vectors (fallback to `bigdecimal` if 28–29 digits prove insufficient)
- [ ] **DMN-09**: FEEL date/time/duration value types — `date`, `time`, `date and time` (IANA timezones via `chrono-tz`), `days and time duration`, `years and months duration`; ISO 8601 `@""` lexical literals parse and round-trip
- [ ] **DMN-10**: FEEL type lattice — conformance (`<:`), equivalence (`≡`), implicit conversions (number↔string, singleton-list↔value), null propagation through the lattice

### FEEL Expression Language

- [ ] **DMN-11**: FEEL lexer/parser — full grammar rules 1–65; dual entry points (`SimpleUnaryTests`, `Expression`); keyword-as-name disambiguation; hand-written recursive descent with Pratt precedence
- [ ] **DMN-12**: FEEL evaluator with three-valued logic — `feel_equals`/`feel_compare` return `Option<_>` (null → `Value::Null`); explicit truth tables for `and`/`or`/`not`; conformance cases (`"a"=1`→null, `null<3`→null, `null=null`→true, `not(null)`→null)
- [ ] **DMN-13**: FEEL operators — arithmetic, comparison, boolean, string concatenation, date-time arithmetic and duration operations
- [ ] **DMN-14**: FEEL control + quantified expressions — `if`, `for`, `some`/`every`, path, filter, `instance of`
- [ ] **DMN-15**: FEEL boxed expressions — all 10 types (literal, context, list, relation, function invocation, function definition, conditional, filter, for, every/some), recursive; boxed contexts evaluated in dependency (topological) order with cycle detection
- [ ] **DMN-16**: FEEL user-defined functions (lambdas, non-external) — grammar rule 55
- [ ] **DMN-17**: FEEL built-in function library — all ~70+ functions across conversion/string/list/numeric/date-time/context/boolean/range/equality families
- [ ] **DMN-18**: FEEL AST cache — cells/expressions pre-parsed to `Arc<FeelExpression>` at DMN load time; the evaluator takes `&FeelExpression` and never re-parses (prevents the 4–6× re-parse perf bug)

### Decision Tables

- [ ] **DMN-19**: Decision-table engine — all 7 hit policies (UNIQUE, ANY, PRIORITY, FIRST, RULE ORDER, OUTPUT ORDER, COLLECT) per Clause 8; input entries evaluated as FEEL unary tests
- [ ] **DMN-20**: COLLECT aggregators — SUM/MIN/MAX/COUNT; reject multi-output aggregator tables; empty-match semantics (empty SUM/MIN/MAX → null, empty COUNT → 0)
- [ ] **DMN-21**: Hit-policy correctness — ANY conflict detection errors (not silent first-match); PRIORITY orders by the output column's `allowedValues` enumeration (not row); output `allowedValues` lists and default output values validated at table-load

### Decision Services

- [ ] **DMN-22**: Decision-service evaluation (§10.4) — `POST /v1/decisions/{id}/evaluate` resolves a multi-output decision service and returns all output decisions; enclosures (input data, decisions) wired per the decision service
- [ ] **DMN-23**: DRD topological resolution — required-decision inputs resolved via the Decision Requirement Graph with cycle detection; downstream decisions evaluated before their dependents
- [ ] **DMN-24**: Decision CRUD + persistence — create/read/update/delete decisions; persistence via Git `.dmn` files + PostgreSQL metadata; import/export endpoints

### Observability

- [ ] **DMN-25**: Inference trace for decisions — `TraceEvent`/`TraceData` emitted during decision evaluation (two-site pattern, new `event_type` constants); opt-in via request flag *(differentiator)*
- [ ] **DMN-26**: Per-decision Prometheus metrics — `rrules_dmn_evaluations_total`, `rrules_dmn_decision_duration_seconds`, `rrules_dmn_hit_policy_violations_total` on the existing dedicated registry; WebSocket `MetricsSnapshot` broadcast after each evaluate *(differentiator)*
- [ ] **DMN-27**: Decision monitoring surfaces — `DmnDecisionTracker` (separate from `RuleExecutionTracker`), dead-decision-row detection, coverage extension, and a dashboard panel reading the new metrics *(differentiator)*
- [ ] **DMN-28**: Hot-reload of decisions — `SharedDmnModel{model, generation}` under `Arc<RwLock>` with file-watcher reload-then-swap, mirroring v5.0 `SharedRuleSet` *(differentiator)*

### Frontend

- [ ] **DMN-29**: Wire the existing `DecisionTable.tsx` + `HitPolicySelector.tsx` (v4.0) to the real backend evaluator; "Evaluate Decision" action + results/trace panel (reuse v5.0 `InferenceTrace` rendering)
- [ ] **DMN-30**: `HitPolicySelector` refactor — `{policy, aggregator?}` data model; add missing FIRST + OUTPUT ORDER; spec-accurate descriptions; engine-dropdown hidden on the DMN view
- [ ] **DMN-31**: `dmnSerializer.ts` — editor-shape ↔ backend DMN-model mapper (edit-through-IR-projection; never reconstruct DMN from the frontend model)
- [ ] **DMN-32**: Allowed-values editor for output columns (PRIORITY/OUTPUT ORDER usability)

### Conformance

- [ ] **DMN-33**: OMG DMN TCK test vectors as the acceptance suite — import + evaluation; documented known divergences (e.g. DMN15-31 Collect distinct-sum, `sum([])`) if any
- [ ] **DMN-34**: Level 3 conformance checklist — one item per built-in family, grammar rule, and boxed expression type; 100% completion + TCK pass required before the v6.0 tag; phase-exit gate (no "Level 3-ish" claim)

## v2 Requirements

Acknowledged but deferred past v6.0. Tracked, not in the current roadmap.

- **DMN-V2-01**: FEEL `external` functions — parser-recognized but eval-unsupported (returns null + logged warning) for v6.0; full external Java/PMML integration deferred
- **DMN-V2-02**: DMN ↔ Rete engine bridge — wire DMN decisions as a trigger/source for the Rete-family engines (genuine differentiator, HIGH complexity)
- **DMN-V2-03**: Cross-engine DMN execution — DMN evaluation routed through `EngineRegistry` algorithm selection (currently a single stateless `DecisionEvaluator`)
- **DMN-V2-04**: DMN 1.6 features — Collect distinct-sum, `sum([])` semantics, other 1.6 additions (spec unsettled at v6.0 time)

## Out of Scope

Explicitly excluded from v6.0. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Visual DRD canvas | Level 1 notation concern; zero Level 3 value. DMNDI is round-tripped (DMN-07), not rendered. |
| External Java/PMML function integration | Massive scope expansion; not required for Level 3; Rust BRMS. Parser-recognized, eval-unsupported (DMN-V2-01). |
| FEEL → SQL transpilation | Research project, not implementation task. |
| Replacing the existing `Expression` AST with the FEEL AST | Would regress v1.0–v5.0 rule execution. `FeelExpression` is a distinct enum. |
| Routing DMN through `/v1/execute` or `EngineRegistry` | DMN is stateless/side-effect-free/three-valued; Rete engines are forward-chaining/two-valued. New parallel surface (`engine/src/dmn/`). |
| Visual DMNDI rendering | DMNDI layout is preserved on round-trip (DMN-07) but no canvas/render is built. |
| Mobile-native DMN editor | Web-first, responsive UI (existing project constraint). |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| DMN-04 | Phase 29 | Complete |
| DMN-05 | Phase 29 | Complete |
| DMN-06 | Phase 29 | Pending |
| DMN-07 | Phase 35 | Pending |
| DMN-08 | Phase 30 | Pending |
| DMN-09 | Phase 30 | Pending |
| DMN-10 | Phase 30 | Pending |
| DMN-11 | Phase 31 | Pending |
| DMN-12 | Phase 31 | Pending |
| DMN-13 | Phase 31 | Pending |
| DMN-14 | Phase 31 | Pending |
| DMN-15 | Phase 31 | Pending |
| DMN-16 | Phase 31 | Pending |
| DMN-17 | Phase 31 | Pending |
| DMN-18 | Phase 31 | Pending |
| DMN-19 | Phase 32 | Pending |
| DMN-20 | Phase 32 | Pending |
| DMN-21 | Phase 32 | Pending |
| DMN-22 | Phase 33 | Pending |
| DMN-23 | Phase 33 | Pending |
| DMN-24 | Phase 33 | Pending |
| DMN-25 | Phase 33 | Pending |
| DMN-26 | Phase 33 | Pending |
| DMN-28 | Phase 33 | Pending |
| DMN-29 | Phase 34 | Pending |
| DMN-30 | Phase 34 | Pending |
| DMN-31 | Phase 34 | Pending |
| DMN-32 | Phase 34 | Pending |
| DMN-27 | Phase 35 | Pending |
| DMN-33 | Phase 35 | Pending |
| DMN-34 | Phase 35 | Pending |

**Coverage:**

- v1 requirements: 31 total (DMN-04..DMN-34)
- Mapped to phases: 31
- Unmapped: 0 ✓
- Phase load: 29→3, 30→3, 31→8, 32→3, 33→6, 34→4, 35→4

---
*Requirements defined: 2026-07-06*
*Last updated: 2026-07-06 after initial definition*
