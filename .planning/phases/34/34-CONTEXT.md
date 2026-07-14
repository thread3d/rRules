# Phase 34: Frontend Wiring + Hit-Policy Model Refactor - Context

**Gathered:** 2026-07-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire the existing v4.0 decision-table editor to the real backend evaluator from phase 33 — users can edit, evaluate, and see results/trace in the UI — with the hit-policy data model refactored to DMN canonical (7 standard policies + aggregators as COLLECT sub-type). The frontend edits through an IR projection (never reconstructing DMN from the frontend model).

</domain>

<decisions>
## Implementation Decisions

### Hit Policy Model
- Use hybrid approach: keep standard 7 DMN canonical policies (UNIQUE, ANY, FIRST, PRIORITY, RULE_ORDER, OUTPUT_ORDER, COLLECT) + add aggregators as sub-type of COLLECT (e.g., hit_policy='COLLECT', aggregator='SUM')
- Align frontend enum with DMN spec; remove custom aggregators from top-level hit policy list

### API Contract
- Use string enum (validated): backend returns structured JSON with hit_policy as enum string; frontend validates against known values
- DecisionDetail.decision_table.hit_policy remains a string field but validated against allowed set

### Aggregator Integration
- Integrate into COLLECT: aggregators (SUM, MIN, MAX, COUNT) become parameters on COLLECT hit policy rather than separate policies
- Frontend shows aggregator selector when hit_policy is COLLECT; backend receives {hitPolicy: 'COLLECT', aggregator: 'SUM'} or similar structure

### UI Refactor Scope
- Full table schema migration: refactor HitPolicySelector component and types.ts; update DecisionTable props to use new enum
- Include migration of existing decision table data from old format (sum/min/max/count as separate policies) to new canonical format (COLLECT + aggregator parameter)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `frontend/src/api/decisions.ts` — Phase 33 API client with evaluateDecision, listDecisions, getDecision, reloadDmn functions
- `frontend/src/features/rules/types.ts` — Current HitPolicy interface and HIT_POLICIES array (8 policies)
- `frontend/src/components/RuleEditor` — Existing rule editor component for reference

### Established Patterns
- React Table (`@tanstack/react-table`) for decision table rendering
- actix-web REST API pattern with `/v1/decisions/{id}/evaluate` endpoint
- Arc<RwLock<SharedDmnModel>> for shared snapshot swap (Phase 33 backend pattern)

### Integration Points
- `frontend/src/features/rules/DecisionTable.tsx` — Main decision table component to refactor
- `frontend/src/features/rules/HitPolicySelector.tsx` — Hit policy dropdown to update
- Backend phase 33: `api/src/v1/decisions.rs` — REST API with evaluate endpoint

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches for DMN canonical alignment.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
