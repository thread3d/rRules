# Phase 35: Conformance Verification + Round-trip + Monitoring Close-out - Context

**Gathered:** 2026-07-09
**Status:** Ready for planning

<domain>
## Phase Boundary

The v6.0 milestone passes the OMG DMN 1.5 Level 3 conformance gate — TCK vectors pass, the exporter round-trips (including DMNDI), monitoring surfaces are live, and the Level 3 conformance checklist is 100% before the v6.0 tag. This phase is the phase-exit gate: no "Level 3-ish" claim is allowed.

</domain>

<decisions>
## Implementation Decisions

### TCK Coverage
- Full TCK sweep now — run all available OMG DMN TCK vectors in one pass; extended test scaffolding for future additions
- Prioritize numeric + temporal types (DMN-08, DMN-09) as foundation, then extend to full coverage

### Round-trip Scope
- Structural round-trip verification — structural/DOM comparison (ignore whitespace, attribute order); more robust to parser differences than byte-exact
- Catches regressions in exporter fidelity without being brittle to formatting

### Monitoring Surfaces
- Extended DMN-specific metrics — per-output evaluation counters, DRD chain depth gauge, decision service invocation trace events
- Build on Phase 33 foundation (per-decision evaluate counters + duration histogram + hit policy violations) with additional granularity for Level 3 conformance evidence

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `engine/src/dmn/parser.rs` — DMN 1.5 XML parser (Phase 29)
- `engine/src/dmn/exporter.rs` — DMN XML exporter with annotations support
- `engine/src/dmn/evaluator/mod.rs` — Decision evaluator with trace emission (Phase 33)
- `api/src/v1/dmn_metrics.rs` — Dedicated DMN metrics registry from Phase 33

### Established Patterns
- TCK test vectors stored as `.dmn` files in `tests/tck/` directory
- Structural comparison via DOM normalization before diffing (ignore whitespace/attribute order)
- Prometheus metrics on dedicated `DMN_METRICS_REGISTRY` (not default_registry)

### Integration Points
- Phase 33: `POST /v1/decisions/{id}/evaluate` endpoint with multi-output support + DRD chaining
- Phase 34: Frontend wiring to real backend evaluator with DMN canonical hit-policy model

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches for OMG DMN TCK conformance verification.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
