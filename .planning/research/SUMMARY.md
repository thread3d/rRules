# Project Research Summary

**Project:** rRules — DMN 1.5 Level 3 Compliance (milestone v6.0)
**Domain:** DMN Decision Tables + FEEL Expression Language integrated into an existing production Rust BRMS
**Researched:** 2026-07-06
**Confidence:** HIGH

## Executive Summary

rRules v6.0 adds OMG DMN 1.5 Level 3 compliance to a shipped production BRMS (Rust actix-web + React/TS, v5.0 audit-passed). Level 3 is binary: it requires **full FEEL** (Clause 10 grammar rules 1–65, all 10 boxed expression types, ~70+ built-in functions, three-valued logic, the type lattice, decision-service semantics) on top of decision tables (Clause 8) and the DMN interchange model (Clauses 6–7). "Level 3-ish" is not a claim — partial implementations may only say "based on DMN." The dominant cost of the milestone is the FEEL evaluator; everything else (parser, hit policies, API, frontend) is straightforward once FEEL exists.

The recommended approach is a **new, parallel evaluation surface** — `engine/src/dmn/` with its own `DecisionEvaluator` behind `POST /v1/decisions/{id}/evaluate` — NOT a compilation of DMN into the existing Rete/TREAT/LEAPS/PHREAK `ExecutionEngine`. DMN is stateless, side-effect-free, single-pass, and three-valued; the Rete engines are forward-chaining, working-memory-mutating, two-valued, and v5.0-audit-passed. Sharing `Value` (extended additively) and `TraceEvent`/`TraceData` is correct; sharing the execution engine or the `Expression` AST is not. Stack additions are minimal and lean: `quick-xml 0.40` for DMN XML, `rust_decimal 1.42` (with `maths`) for Decimal128, `chrono-tz 0.10` for IANA timezones, and a hand-written recursive-descent FEEL parser in-tree.

The top risks are (1) three-valued-logic leak — reusing Rust `PartialEq`/`compare_values` for FEEL equality silently coerces `null` to `false`, breaking filter/`if`/ANY semantics; (2) PRIORITY-as-first-match — DMN PRIORITY orders by the output column's `allowedValues` enumeration, not by row; (3) FEEL re-parsing on every evaluation — the #1 documented DMN performance bug, a 4–6× throughput loss; (4) round-trip fidelity loss — the existing frontend `DecisionTable.tsx` shape is not DMN and cannot be the canonical store; (5) false Level 3 claim — shipping S-FEEL + decision tables and labeling it Level 3 is non-compliant and an enterprise-buyer audit risk. All five are preventable if the phase sequence below is followed and the conformance checklist is a phase-exit gate.

## Key Findings

### Recommended Stack

NEW additions only — existing validated deps (actix-web 4.4, tokio, the Rete engines, `EngineRegistry`, Prometheus 0.13, `chrono 0.4`, hybrid Git+PostgreSQL, the v4.0 frontend editor) are reused unchanged. The single most important stack decision: **do NOT take the `dsntk-*` crate family as a dependency** (pre-1.0 SemVer risk, `Value`/`AstNode` impedance mismatch, pulls `reqwest` into a pure-compute evaluator, designed as a standalone CLI). Build from scratch; read `dsntk` source as a reference for FEEL semantics.

**Core technologies (NEW):**
- `quick-xml 0.40` — DMN 1.5 XML import/export via streaming `NsReader` + `Writer`; handles DMN's multi-namespace XML (MODEL/DMNDI/DC/DI); ~50× faster than alternatives.
- `rust_decimal 1.42` (features `serde`, `macros`, `maths`) — FEEL Decimal128 numeric domain; 28–29 significant digits (sufficient for every real-world BRMS use case). **Fallback:** `bigdecimal` only if OMG TCK conformance vectors prove 28–29 digits is insufficient — flagged as a phase-specific research item.
- `chrono-tz 0.10` — IANA timezone database for FEEL `time`/`date and time` with named zones (DST-correct duration arithmetic). ~7 MB binary add.
- Hand-written recursive-descent FEEL parser (in-tree, `engine/src/dmn/feel/parser.rs`) — FEEL has no left recursion; O(n); ~600 LoC per the Go reference. Keyword-as-name disambiguation and dual entry points (`SimpleUnaryTests` vs `Expression`) are context-sensitive and easier in hand-written code than in generated parsers. Confirmed by 3 independent reference implementations.
- `chrono 0.4` (existing) — reused for FEEL `date`/`time`/`date and time`/`days and time duration`; no version bump.
- `proptest` + `criterion` (existing) — fuzz the FEEL coercion matrix; bench the hot path. No new dev deps.

**Reconciliation note (parser strategy):** STACK.md and ARCHITECTURE.md recommend hand-written recursive descent; FEATURES.md mentions a Pratt parser or `pest`/`lalrpop`. These are compatible — Pratt parsing is a precedence-handling technique *within* a hand-written recursive-descent parser, and is the right choice for FEEL's expression-precedence layers. Parser generators (`lalrpop`/`pest`) are explicitly rejected. **Synthesis: hand-written recursive descent with Pratt-style precedence for the expression grammar.**

**Reconciliation note (numeric crate):** STACK.md and PITFALLS.md recommend `rust_decimal` (28–29 digits); FEATURES.md mentions `bigdecimal`/`rust_decimal`. The 34-digit Decimal128 mandate is normative, but 28–29 digits exceeds every real-world financial/insurance/healthcare requirement. **Synthesis: `rust_decimal 1.42` as the default; `bigdecimal` is a TCK-gated fallback, flagged for phase-specific research if conformance vectors demand strict 34-digit precision.**

### Expected Features

**Must have (table stakes for the Level 3 claim):**
- FEEL lexer/parser — grammar rules 1–65 (the enabler; without it, no FEEL, no Level 3).
- FEEL evaluator — arithmetic, comparison, boolean with **ternary logic**, `if`, `for`, `some`/`every`, path, filter, `instance of`.
- FEEL type lattice + implicit conversions + null/ternary propagation — semantic backbone.
- Decimal128 numeric type + `date`/`time`/`date and time`/`days and time duration`/`years and months duration` value types.
- FEEL built-in function library — all ~70+ functions across conversion/string/list/numeric/date-time/context/boolean/range/equality.
- All 10 FEEL boxed expression types (recursive).
- User-defined FEEL functions (lambdas, non-external) — grammar rule 55.
- DMN 1.5 XML parser + exporter (round-trip) — canonical IR is the store, frontend is a projection.
- Item definitions + information items — Clause 7.
- Decision-table engine: all 7 hit policies (U/A/P/F/C/R/O) + COLLECT aggregators + default outputs + allowed-values lists.
- Decision CRUD + persistence (Git `.dmn` files + PostgreSQL metadata).
- `POST /v1/decisions/{id}/evaluate` — mirrors v5.0 `/v1/execute` wiring.
- Decision-eval integration with Prometheus (`rrules_dmn_*` on existing dedicated registry) + trace (`TraceEvent`/`TraceData`, extended `event_type`).
- Frontend wiring: `DecisionTable.tsx` + `HitPolicySelector.tsx` → backend evaluator (PROJECT.md explicit scope; does NOT rebuild the editor).
- `HitPolicySelector` refactor: `{policy, aggregator?}`; add missing FIRST + OUTPUT ORDER; fix misleading descriptions.
- Allowed-values editor for output columns (PRIORITY/OUTPUT ORDER usability).
- OMG DMN TCK conformance vectors as the acceptance suite.

**Should have (differentiators, v6.x):**
- Inference-trace emission for decisions (reuse v5.0 two-site trace pattern).
- Per-decision Prometheus metrics + dashboard panel.
- Hot-reload of decisions (`SharedDmnModel{model, generation}` mirroring v5.0 `SharedRuleSet`).
- DMNDI round-trip preservation (parse + persist layout; NO rendering).

**Defer (v7+, anti-features for v6.0):**
- Visual DRD canvas — Level 1 notation concern, zero Level 3 value.
- External Java/PMML function integration — massive scope expansion; not required for Level 3; Rust BRMS.
- Cross-table `in` evaluation / decision-service chaining — defer to post-MVP (single-decision eval per API call for v6.0).
- DMN↔Rete engine bridge — genuine differentiator but HIGH complexity; post-MVP.
- DMN 1.6 features (Collect distinct-sum, `sum([])`) — spec unsettled; target 1.5.
- Replacing the existing `Expression` AST with the FEEL AST — would regress v1.0–v5.0.
- FEEL→SQL transpilation — research project, not implementation task.

### Architecture Approach

A new `engine/src/dmn/` module tree is the parallel evaluation surface, sibling to `engine/src/engine/` (NOT nested inside it — DMN is not a Rete-family algorithm and must never be wired through `EngineRegistry`). The `DecisionEvaluator` is stateless, `Send + Sync + 'static`, held in `web::Data`, constructed per-request inside `web::block` (mirrors v5.0's CPU-off-async-executor discipline without the `EngineRegistry`/`Box<dyn ExecutionEngine>` ceremony). A `SharedDmnModel{model, generation}` under `Arc<RwLock>` mirrors `SharedRuleSet` for reload-then-swap. `engine::data_types::Value` is extended additively with FEEL variants (`Date`, `Time`, `DaysTimeDuration`, `YearsMonthsDuration`, `Range`, `Function`) — NOT forked. The `Expression` AST is NOT shared; a distinct `FeelExpression` enum with `evaluate(&Scope)` lives in `dmn/feel/ast.rs`. `TraceEvent`/`TraceData` are reused with new `event_type` constants. `DecisionCounters` registers on the SAME dedicated Prometheus `Registry` as `ExecutionCounters`. The existing Rete path is UNCHANGED — zero regression risk.

**Major components:**
1. `engine/src/dmn/model.rs` — DMN 1.5 in-memory model; canonical IR for round-trip.
2. `engine/src/dmn/parser.rs` — `.dmn` XML → model via `quick-xml` `NsReader`; namespace-aware, XXE-hardened, CDATA-handled.
3. `engine/src/dmn/exporter.rs` — model → `.dmn` XML via `quick-xml::Writer`; preserves annotations, DMNDI, IDs, element order.
4. `engine/src/dmn/feel/{ast,parser,eval,builtins,temporal,coercion}.rs` — FEEL parser (hand-written RD + Pratt), evaluator (three-valued, type lattice, ~70 builtins), temporal, coercion.
5. `engine/src/dmn/decision_table.rs` — hit-policy engine (U/A/P/F/C/R/O + aggregators + allowed-values + default outputs).
6. `engine/src/dmn/evaluator.rs` — decision orchestrator + DRD topological resolution (single-decision for v6.0 MVP).
7. `api/src/v1/decisions.rs` — REST surface (evaluate + CRUD + import/export); `DecisionCounters`; WS broadcast; OpenAPI.
8. `frontend/src/features/rules/dmnSerializer.ts` + `frontend/src/api/decisions.ts` — editor-shape ↔ DMN-model mapper; API client; results/trace panel reuse.

### Critical Pitfalls

1. **Three-valued-logic leak** — Never reuse Rust `==`/`compare_values` for FEEL. Implement `feel_equals(a,b) -> Option<bool>` and `feel_compare(a,b) -> Option<Ordering>` in `dmn/feel/eval.rs`; map `None` to `Value::Null`. Encode FEEL truth tables for `and`/`or`/`not` explicitly. Conformance tests: `"a"=1`→null, `null<3`→null, `null=null`→true, `not(null)`→null. **Prevent in Phase 31.**

2. **PRIORITY-as-first-match** — PRIORITY selects by the output column's `allowedValues` enumeration order (decreasing priority), NOT by row. Validate at load that P tables have an enumerated `allowedValues`; build `{output_value → rank}`; select lowest rank among matches; emit warnings for "misleading rules." OUTPUT ORDER is the multi-result counterpart. **Prevent in Phase 32.**

3. **FEEL re-parsing on every evaluation** — The #1 documented DMN performance bug (4–6× throughput loss). Pre-parse all cell FEEL into `Arc<FeelExpression>` ASTs at DMN load time; evaluator takes `&FeelExpression`, never `&str`. Benchmark: 30-cell table × 10k evals ≈ 30 parses, not 300k. **Prevent in Phase 31 (cache primitive) + Phase 32 (pre-parse at load).**

4. **Round-trip fidelity loss** — The existing frontend `DecisionTableRow`/`DecisionTableColumn` shape is NOT DMN. Make the **DMN XML IR the canonical store**; the frontend is a projection. On export, mutate the IR with frontend edits — never reconstruct DMN from the frontend model. **Prevent in Phase 29 (canonical IR) + Phase 34 (edit through projection) + Phase 35 (exporter round-trip).**

5. **False Level 3 claim** — Level 3 is binary. Track conformance as a checklist (one item per built-in family, grammar rule, boxed expression type); **do not tag v6.0 as Level 3 until the checklist is complete and TCK vectors pass.** If scope must be cut, cut the claim to Level 2, not the checklist. **Make the conformance checklist a phase-exit criterion (Phase 35).**

**Additional high-impact pitfalls:** ANY conflict detection must error (not silently pick first); COLLECT aggregators must reject multi-output tables, return `null` for empty SUM/MIN/MAX and `0` for empty COUNT; boxed contexts evaluate in dependency (topological) order with cycle detection; DMN XML parser must accept namespaces 1.1–1.5, hard-disable XXE, handle CDATA, default `hitPolicy` to UNIQUE; `HitPolicySelector` must be refactored to `{policy, aggregator?}` with FIRST + OUTPUT ORDER added.

## Implications for Roadmap

The dependency graph in FEATURES.md and the build order in ARCHITECTURE.md force a specific phase sequence. PITFALLS.md's phase mapping (29–34) aligns. The FEEL evaluator is the dominant scope and the critical path. Suggested 7-phase structure (continuing from v5.0 Phase 28):

### Phase 29: DMN 1.5 Model + XML Parser + Canonical IR
**Rationale:** No upstream deps. The canonical IR is the foundation for round-trip fidelity (Pitfall 11) and every downstream phase parses into or evaluates against it.
**Delivers:** `engine/src/dmn/{model.rs, parser.rs, exporter.rs (skeleton)}`; DMN 1.5 model structs; `quick-xml` namespace-aware parse; XXE hardening; CDATA handling; `hitPolicy` defaults to UNIQUE; annotation/description preservation; import corpus (1.1/1.3/1.5 files) passes.
**Addresses:** DMN XML parser, item definitions, DMN XML exporter (skeleton), Clause 6/7 acceptance.
**Avoids:** Pitfall 10 (parser bugs), Pitfall 11 (round-trip — IR is canonical from day 1).
**Stack:** `quick-xml 0.40`.

### Phase 30: FEEL Value Types + Type Lattice + Temporal Domain
**Rationale:** FEEL's semantic domain is normative and the type lattice is the backbone every evaluator feature depends on. Co-locating the `Value` extension with the type lattice avoids a churn-later retrofit.
**Delivers:** `Value` extended additively with `Date(NaiveDate)`, `Time(NaiveTime)`, `DaysTimeDuration(TimeDelta)`, `YearsMonthsDuration{years,months}`, `Range{...}`, `Function{...}`; `rust_decimal::Decimal` numeric path for FEEL; FEEL type lattice + conformance (`<:`) + equivalence (`≡`) + to/from-singleton-list conversions; temporal lexical formats (ISO 8601 `@""` literals); `0.1 + 0.2 = 0.3` passes.
**Addresses:** Decimal128 + date/time/duration types, FEEL type lattice, implicit conversions.
**Avoids:** Pitfall 3 (Value gaps), the `f64`-precision trap, the `bigdecimal`-vs-`rust_decimal` ambiguity (validate against TCK here).
**Stack:** `rust_decimal 1.42` (`serde`, `macros`, `maths`), `chrono-tz 0.10`, existing `chrono 0.4`.
**Research flag:** Validate 28–29-digit precision against OMG TCK numeric vectors; if any fail, switch to `bigdecimal` before Phase 31 builds operators on top.

### Phase 31: FEEL Parser + Evaluator + Built-in Function Library
**Rationale:** The dominant scope and the critical path. The parser (grammar 1–65), the evaluator (three-valued logic, all operators, all boxed expressions), the ~70+ built-in function library, and the AST cache all land here. The largest single phase — likely to need `/gsd-plan-phase --research-phase 31` and possibly sub-phase splitting.
**Delivers:** `engine/src/dmn/feel/{ast.rs, parser.rs, eval.rs, builtins.rs, temporal.rs, coercion.rs}`; `FeelExpression` enum distinct from `Expression`; `evaluate(&Scope)` signature; ternary logic (`feel_equals`/`feel_compare` returning `Option<_>`); arithmetic/comparison/boolean/if/for/some/every/path/filter/instance-of; all 10 boxed expressions; boxed-context dependency-order evaluation with cycle detection; user-defined FEEL lambdas; AST cache primitive (`DashMap<String, Arc<FeelExpression>>`); full built-in library.
**Addresses:** FEEL lexer/parser, FEEL evaluator, type lattice usage, built-in library, boxed expressions, user-defined functions, ternary logic propagation.
**Avoids:** Pitfall 1 (three-valued logic leak), Pitfall 2 (AST reuse/duplication), Pitfall 8 (no AST cache — primitive lands here), Pitfall 9 (context textual-order eval), Pitfall 12 (Level 3 conformance surface — bulk lands here).
**Stack:** Hand-written recursive descent + Pratt precedence; `proptest` for coercion fuzzing.

### Phase 32: Decision-Table Engine + Hit Policies
**Rationale:** Depends on Phase 31 (input entries are FEEL unary tests; output entries are FEEL expressions). Hit-policy semantics are the highest-bug-yield surface after ternary logic.
**Delivers:** `engine/src/dmn/decision_table.rs`; all 7 hit policies (U/A/P/F/C/R/O) + COLLECT aggregators (SUM/MIN/MAX/COUNT); `allowedValues` validation for P/O; default output values; `HitPolicyViolation` error type for ANY conflicts and UNIQUE violations; multi-output rejection for aggregator tables; empty-match semantics (`C+`→null, `C#`→0); pre-parse all cell FEEL into ASTs at table-load time.
**Addresses:** All Clause 8 hit-policy features, COLLECT aggregators, default outputs, allowed-values lists, input-entry unary-test evaluation.
**Avoids:** Pitfall 4 (PRIORITY-as-first-match), Pitfall 5 (ANY no-conflict), Pitfall 6 (COLLECT aggregator bugs), Pitfall 8 (pre-parse at load closes the cache loop).

### Phase 33: Decision Evaluator + DRD Resolution + REST API + Metrics
**Rationale:** Depends on Phase 32. The orchestrator + API wire the evaluator to the outside world and to rRules' v5.0 observability surfaces. Single-decision evaluation per call for v6.0 MVP (decision chaining deferred).
**Delivers:** `engine/src/dmn/evaluator.rs` (decision orchestrator + topological DRD resolution for single-decision scope); `api/src/v1/decisions.rs` (`POST /v1/decisions/{id}/evaluate` + CRUD + import/export); `DecisionCounters` on the existing dedicated Prometheus registry (`rrules_dmn_evaluations_total`, `rrules_dmn_decision_duration_seconds`, `rrules_dmn_hit_policy_violations_total`); `SharedDmnModel{model, generation}` under `Arc<RwLock>` with file-watcher reload; WS `MetricsSnapshot` broadcast reuse; `TraceEvent` emission with new `event_type` constants; `utoipa` OpenAPI schemas; `DMN_HIT_POLICY_VIOLATION` error code; `web::block` per-request pattern.
**Addresses:** `POST /v1/decisions/{id}/evaluate`, decision CRUD + persistence, Prometheus/trace integration, OpenAPI.
**Avoids:** Pitfall 5 (ANY conflict surfaced as `DMN_HIT_POLICY_VIOLATION`), the Anti-Pattern of routing DMN through `/v1/execute`, the Anti-Pattern of a second Prometheus registry, `RuleExecutionTracker`/`CoverageTracking` contamination (separate `DmnDecisionTracker` or `source` tag).
**Stack:** Existing actix-web, utoipa, Prometheus, `web::block` patterns — all reused.

### Phase 34: Frontend Wiring + Hit-Policy Model Refactor
**Rationale:** Depends on Phase 33 (the API contract). The editor exists (v4.0); this phase wires it to the backend and refactors the hit-policy data model to DMN canonical. The model refactor is the first task — the backend enum from Phase 32 is canonical, the frontend maps TO it.
**Delivers:** `HitPolicySelector` refactor to `{policy, aggregator?}` with FIRST + OUTPUT ORDER added and spec-accurate descriptions; `dmnSerializer.ts` (editor shape ↔ backend DMN model); `api/decisions.ts` client; "Evaluate Decision" button + results/trace panel (reuse v5.0 `InferenceTrace` rendering); allowed-values editor for output columns; engine-dropdown hidden on DMN view; edit-through-IR-projection.
**Addresses:** Frontend wiring (PROJECT.md explicit scope), allowed-values editor, HitPolicySelector alignment.
**Avoids:** Pitfall 7 (frontend hit-policy vocabulary mismatch), Pitfall 11 (round-trip fidelity — frontend edits through projection), the UX pitfalls (ANY conflict surfaced distinctly, `null` shown explicitly, FIRST/OUTPUT ORDER selectable).

### Phase 35: Conformance Verification + Round-trip + Monitoring Close-out
**Rationale:** The Level 3 claim gate. Depends on Phases 29–34. The conformance checklist must be 100% before the v6.0 tag; the exporter round-trip closes the loop started in Phase 29; monitoring surfaces parallel the v5.0 production surfaces.
**Delivers:** OMG DMN TCK test vectors as the acceptance suite (import + eval); exporter round-trip byte/DOM-compare preserving annotations, DMNDI, IDs, element order; XSD-valid output; `DmnDecisionTracker` + dead-decision-row detection + coverage extension; conformance checklist (one item per built-in family, grammar rule, boxed expression type) as phase-exit criterion; documented known divergences (DMN15-31 Collect distinct-sum, `sum([])`) if any.
**Addresses:** Conformance test suite, DMN XML exporter (round-trip close), monitoring/trace/coverage wiring, Level 3 claim verification.
**Avoids:** Pitfall 12 (false Level 3 claim — checklist is the gate), Pitfall 11 (round-trip fidelity — exporter closes the loop), monitoring contamination (separate `DmnDecisionTracker`).

### Phase Ordering Rationale

- **Dependencies force the order:** FEEL (Phase 31) requires the Value extension + type lattice (Phase 30), which requires nothing but the model (Phase 29). Hit policies (Phase 32) require FEEL to evaluate input entries. The API (Phase 33) requires the evaluator. Frontend (Phase 34) requires the API contract. Conformance (Phase 35) requires everything.
- **Grouping by architecture pattern:** Phase 29 is the IR foundation; Phases 30–31 are the FEEL semantic core (split because the type-lattice decision is load-bearing and co-locating `Value` extension with operators risks a churn-later retrofit); Phase 32 is the decision-table semantics; Phase 33 is the integration layer; Phase 34 is the presentation layer; Phase 35 is the verification gate.
- **Pitfall avoidance:** Three-valued logic (Pitfall 1), AST shape (Pitfall 2), Value gaps (Pitfall 3), AST cache primitive (Pitfall 8), context dependency order (Pitfall 9) all land in Phase 30–31 where they are cheap to fix. PRIORITY/ANY/COLLECT (Pitfalls 4–6) land in Phase 32. Round-trip fidelity (Pitfall 11) is structural: the IR is canonical from Phase 29, the frontend edits through projection in Phase 34, the exporter closes in Phase 35. The Level 3 claim (Pitfall 12) is gated in Phase 35.
- **Splitting FEEL into two phases (30 + 31):** FEEL is the dominant cost. Splitting the type-system foundation (Phase 30) from the parser+evaluator+builtins (Phase 31) keeps each phase verifiable in isolation and isolates the `rust_decimal`-vs-`bigdecimal` decision to Phase 30, where it can be researched against TCK vectors before operators are built on top. If the roadmapper prefers a single FEEL phase, merge 30+31 — but the type-lattice work must precede the operator work within the merged phase.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 30 (FEEL Value Types + Type Lattice):** The `rust_decimal`-vs-`bigdecimal` decision must be validated against OMG TCK numeric conformance vectors before Phase 31 builds operators. Run `/gsd-plan-phase --research-phase 30`.
- **Phase 31 (FEEL Parser + Evaluator + Builtins):** The largest phase; FEEL grammar 1–65, ~70 builtins, 10 boxed expressions, ternary logic, scope/longest-match QN resolution. Likely needs `/gsd-plan-phase --research-phase 31` and may warrant sub-phase splitting during planning.
- **Phase 35 (Conformance Verification):** Acquiring and integrating the OMG DMN TCK test vectors (TC archive) and mapping them to `engine/tests/` fixtures may need research if the TCK distribution format is non-trivial.

Phases with standard patterns (skip research-phase):
- **Phase 29 (DMN Model + Parser):** `quick-xml` patterns are well-documented (Context7 `/tafia/quick-xml`, 577 snippets); DMN XSD is normative.
- **Phase 32 (Decision-Table Engine):** Hit-policy semantics are spec-clear (DMN §8.2.12); Pitfalls 4–6 give precise prevention strategies.
- **Phase 33 (API + Metrics):** Direct mirror of v5.0 `/v1/execute` wiring patterns (already shipped + audit-passed).
- **Phase 34 (Frontend):** Existing editor + standard React/TS API-client work; the `HitPolicySelector` refactor is a small, well-specified model change.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Core deps verified via Context7 + crates.io/docs.rs cross-check. The `dsntk-*` rejection is robust to embedding-suitability uncertainty. MEDIUM on the 28–29-digit sufficiency (domain call — TCK validation flagged). |
| Features | HIGH | Authoritative: OMG DMN 1.5 plain-text spec Clause 10, §2.1 conformance, §8 hit policies. Corroborated by Camunda 8 + Drools FEEL handbook. Level 3 = full FEEL is normative. |
| Architecture | HIGH | Integration points verified by reading rRules source. The "new parallel evaluation surface" decision is settled by DMN's stateless/side-effect-free/three-valued nature vs Rete's forward-chaining/two-valued/working-memory-mutating nature. |
| Pitfalls | HIGH | Cross-verified against OMG spec, Camunda feel-scala issue tracker (#1149, #814, #6, #8571), Drools handbook, bpmn.io/kie-tools issue trackers (#3594, #1017), and live rRules source inspection. |

**Overall confidence:** HIGH

### Gaps to Address

- **Decimal128 precision (28–29 vs 34 digits):** `rust_decimal` provides 28–29 digits; FEEL mandates Decimal128 (34 digits). 28–29 exceeds every real-world BRMS requirement, but strict OMG TCK vectors may demand 34. **Handle in Phase 30:** run TCK numeric vectors early; switch to `bigdecimal` if any fail. Do not build Phase 31 operators on an unvalidated numeric domain.
- **OMG DMN TCK availability + format:** The TCK distribution format (TC archive vs public repo) needs confirmation during Phase 35 planning. If unavailable, fall back to DMN 1.5 spec examples + Camunda feel-scala test corpus and document the gap.
- **Decision-service execution semantics (§10.4):** Level 3 mandates decision-service support, but v6.0 MVP scopes to single-decision evaluation per API call. The Phase 35 conformance checklist must decide whether single-decision-only is a defensible Level 3 interpretation or a known divergence that downgrades the claim.
- **DMN 1.5 vs 1.6 edge cases:** DMN15-31 (Collect distinct-sum, `sum([])`) is deferred to 1.6. Pick a defensible behavior (recommend `0` for empty SUM, `null` for empty MIN/MAX, `0` for empty COUNT), document as a known divergence, verify the TCK does not penalize it.
- **FEEL `external` functions:** Parser-recognized but unsupported-at-eval (returns null + logged warning) for v6.0. Confirm this is a defensible Level 3 interpretation (the spec says FEEL "completely defines the meaning of expressions that do not invoke externally-defined functions") — likely yes, but the conformance checklist should note it.

## Sources

### Primary (HIGH confidence)
- OMG DMN 1.5 plain-text spec, Clause 10 — https://github.com/omg-dmn-taskforce/plain-text-spec/blob/main/10-expression-language-FEEL.adoc
- OMG DMN 1.3/1.5 §2.1 conformance — https://www.omg.org/spec/DMN/1.3/PDF
- OMG DMN XSD schemas (DMN15.xsd) — https://github.com/omg-dmn-taskforce/omg-dmn-spec
- OMG Issue DMN15-31 — https://issues.omg.org/issues/DMN15-31
- Camunda feel-scala issues #1149, #814, #6, #8571 — https://github.com/camunda/feel-scala/issues
- apache/kie-tools issue #3594, apache/kie-issues #1017 — https://github.com/apache/incubator-kie-tools/issues
- Flowable `DmnXMLConverter.java` — https://github.com/flowable/flowable-engine
- rRules codebase (source-verified integration points)
- Context7 `/tafia/quick-xml`, `/websites/rs_rust_decimal`, `/chronotope/chrono`, `/chronotope/chrono-tz`

### Secondary (MEDIUM confidence)
- Drools DMN FEEL Handbook — https://kiegroup.github.io/dmn-feel-handbook/
- Camunda 8 hit-policy docs — https://docs.camunda.io/docs/components/best-practices/modeling/choosing-the-dmn-hit-policy/
- KIE Community ANC blog — https://blog.kie.org/2021/04/demystifying-the-alpha-network-compiler-anc.html
- Camunda blog — DMN performance improvements — https://camunda.com/blog/2016/08/dmn-performance-improvements/
- Method & Style (Bruce Silver) — https://www.methodandstyle.com/blog/decision-table-hit-policy-explained/
- Operaton DMN Compatibility — https://iou-architectuur.open-regels.nl/linked-data-explorer/reference/operaton-dmn-compatibility/
- `dsntk` crate family (reference, NOT a dependency) — https://crates.io/crates/dsntk
- `pbinitiative/feel` (Go hand-written RD reference) — https://github.com/pbinitiative/feel

### Tertiary (LOW confidence, needs validation)
- Trisotech — "Make Way for B-FEEL" — https://www.trisotech.com/make-way-for-b-feel/
- CEUR-WS — "The Effectiveness of DMN Portability" — https://ceur-ws.org/Vol-1875/paper14.pdf
- ScratchyDisk.DmnEngine — https://github.com/Scratchydisk/ScratchyDisk.DmnEngine
- bpmn.io — DMN 1.3 migration — https://bpmn.io/blog/posts/2020-dmn-js-8-0-0.html

---
*Research completed: 2026-07-06*
*Ready for roadmap: yes*