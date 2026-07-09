# Phase 33: Decision Evaluator + DRD Resolution + REST API + Metrics - Context

**Gathered:** 2026-07-09
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — no user-facing behavior)

<domain>
## Phase Boundary

Phase 33 builds the **decision evaluation service layer** that connects Phases 29–32's
DMN parser/IR, FEEL evaluator, and decision-table engine to a REST API with observability.

Specifically:
- An `engine::dmn::evaluator` module (or similar) that takes a parsed `DmnModel`, resolves
  DRD references (`InformationRequirement`), evaluates the root `Decision` against an input
  `Scope<Value>`, and returns results + trace events.
- A `POST /v1/decisions/{id}/evaluate` endpoint with multi-output support (MVP: evaluate all
  outputs of a single decision; DRD chaining deferred to a follow-up if not already in scope).
- Per-decision Prometheus metrics (`rrules_dmn_evaluations_total`, `rrules_dmn_decision_duration_seconds`,
  `rrules_dmn_hit_policy_violations_total`) on the existing dedicated registry (per ROADMAP spec).
- Inference trace emission (matching Phase 28's `TraceEvent`/`TraceData` pattern) for each
  decision evaluation, surfaced at `GET /v1/decisions/{id}/trace`.
- Hot-reload: when a `.dmn` file changes on disk, the next evaluate call picks up the new IR
  without restart (mirroring Phase 25's `SharedRuleSet` pattern).

This phase is **infrastructure**: it produces no user-facing behavior beyond API contracts.
Phase 34 wires these contracts to the frontend editor.
</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase with no
user-facing behavior described in the ROADMAP goal. Use the existing v5.0 patterns (shared
snapshot swap, dedicated metrics registry, trace event emission) as the reference architecture.

Key decisions to make:
- Where the decision evaluator lives (`engine/src/dmn/evaluator.rs`? `engine/src/dmn/service.rs`?)
- Whether DRD resolution is recursive (evaluate referenced decisions first) or flat (caller chains)
- How AST cache lifetime is managed (per-evaluate vs. shared snapshot with the IR)
- Trace format: reuse Phase 28's `TraceEvent`/`TraceData` or define a DMN-specific trace type

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- **DMN Model** (`engine/src/dmn/model.rs`) — Full IR with `DmnModel`, `DecisionTable`,
  `InputClause`, `OutputClause`, `InformationRequirement` (for DRD refs).
- **Parser + Cache** (`engine/src/dmn/parser.rs::parse_dmn_with_cache`) — Already returns
  `(DmnModel, FeelAstCache)` together. Phase 33 consumes this output directly.
- **Decision Table Evaluator** (`engine/src/dmn/decision_table.rs::DecisionTableEvaluator`)
  Stateless `evaluate(table, scope, cache) -> Result<Value, Error>`. The evaluator's work.
- **FEEL Scope** (`engine/src/dmn/feel/eval.rs::Scope`) — Input context for FEEL evaluation.

### Established Patterns (must follow)
- **Shared snapshot swap** (Phase 25): `Arc<RwLock<SharedDmnModel>>` with generation counter,
  mirroring `SharedRuleSet`. Sync reload writes to a `ReloadJob` in `JobStore`.
- **Dedicated metrics registry** (Phase 24): separate `Registry` from the main one; use
  `TextEncoder` for `/metrics` endpoint. Labels: `decision_id`, `hit_policy`.
- **Trace emission** (Phase 28): opt-in via request field (`trace: bool`). `TraceEvent`/`TraceData`
  emitted at each evaluation step. Stored in-memory, served by `GET /v1/decisions/{id}/trace`.
- **Actix-web routing**: use `web::scope("/decisions")` with nested routes; path params via
  `web::Path<IdParam>`.

### Integration Points
- **Phase 32 (decision-table engine):** the evaluator calls
  `DecisionTableEvaluator::evaluate()` for each decision table in a DRD.
- **Phase 29 (parser/exporter):** load `.dmn` files via `parse_dmn_with_cache()`. Export via
  `exporter::export_to_xml()`.
- **v5.0 observability:** metrics go to the dedicated DMN registry; trace events follow
  Phase 28's pattern.
</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Refer to v5.0 patterns for shared snapshots,
metrics, and tracing. The MVP evaluates a single decision (with its table); DRD chaining
(calling one decision from another) is in scope but the primary success criterion is
single-decision evaluation with correct output aggregation.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
