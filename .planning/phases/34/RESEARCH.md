# Phase 34: Frontend Wiring + Hit-Policy Model Refactor - Research

**Researched:** 2026-07-09
**Domain:** React TypeScript frontend, DMN canonical data model, REST API integration
**Confidence:** HIGH

## Summary

Phase 34 wires the existing v4.0 decision-table editor to the real backend evaluator from Phase 33, while refactoring the hit-policy data model to align with the DMN 1.5 canonical specification (7 policies + COLLECT aggregators as sub-type). The core work is threefold:

1. **Hit-policy model refactor:** The frontend currently uses a flat list of 9 policies (`unique`, `any`, `priority`, `collect`, `rule-order`, `sum`, `min`, `max`, `count`) where `sum`/`min`/`max`/`count` are top-level siblings of the standard 7 DMN hit policies. The backend's canonical model (Phase 32, `engine/src/dmn/model.rs`) uses exactly 7 variants (`Unique`, `Any`, `Priority`, `First`, `RuleOrder`, `OutputOrder`, `Collect`) with a separate `BuiltinAggregator` enum (`Sum`, `Min`, `Max`, `Count`) that is only meaningful when the hit policy is `Collect`. The frontend must migrate to this canonical structure.

2. **API contract bridge:** The Phase 33 backend (`POST /v1/decisions/{id}/evaluate`) expects inputs keyed by input-expression names from the DMN model (e.g., `"age"`, `"income"`), not the current frontend pattern of `input_${idx}_${key}`. A serializer layer (`dmnSerializer.ts`) must map between the editor's column/row shape and the IR shape that the backend evaluator consumes.

3. **Evaluation wiring:** The existing `DecisionTable.tsx` already imports `evaluateDecision` from `@/api/decisions` but uses a simplified input construction pattern. This phase makes it call correctly against real decisions loaded via `GET /v1/decisions/{id}`.

**Primary recommendation:** Build an edit-through-IR-projection pattern: the frontend owns an editor-friendly shape (columns, rows with conditions/actions), and `dmnSerializer.ts` maps TO and FROM the canonical DMN IR (`InputClause`, `OutputClause`, `DmnRule`, `HitPolicy`) on every save/evaluate round-trip. Never reconstruct DMN from the frontend model — always go through the serializer as a single source of truth for shape conversion.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Hit-policy selection UI | Browser / Client | — | Purely presentational; backend enum is canonical |
| Decision table editing (columns, rows) | Browser / Client | — | Editor state lives in React component tree |
| Input/output clause definition | Browser / Client | — | Part of the editor shape; mapped to IR at save time |
| DMN IR serialization/deserialization | Browser / Client | API Server | `dmnSerializer.ts` is a client-side mapper; backend never interprets frontend model |
| Decision evaluation (hit-policy logic) | API / Backend | — | Phase 32 evaluator owns all 7 policies + aggregators |
| DRD resolution + multi-output | API / Backend | Database | Phase 33 `DecisionEvaluator` handles topological ordering |
| Rule execution (Rete family) | API / Backend | — | v5.0 path; NOT used for DMN decisions |
| Prometheus metrics for DMN | API / Backend | — | Phase 33 dedicated registry (`rrules_dmn_*`) |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| React 18+ | — | UI framework (existing) | v4.0 established stack; all components use functional components with hooks |
| TypeScript | — | Type safety for frontend model mapping | Required for serializer correctness; DMN IR types must be exact |
| `@tanstack/react-table` | — | Decision table rendering (existing) | Already used by `DecisionTable.tsx`; column/row abstraction matches IR projection well |
| `@tanstack/react-query` | — | API data fetching + caching (existing) | Used by TestRunner; must be reused for decisions list/get |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| CodeMirror (`@uiw/react-codemirror`) | — | FEEL expression editing in cells (existing) | Phase 31 parses cell text as FEEL ASTs; frontend must preserve exact text for round-trip |
| `lucide-react` | — | Icons (Play, RefreshCw, etc.) | Already used by TestRunner; reuse for Evaluate button, trace toggle |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Editor shape → IR serializer (`dmnSerializer.ts`) | Direct IR editing in React state | Would require React components to understand `InputClause`/`OutputClause`/`DmnRule` shapes — tighter coupling, harder to test the mapper |
| Backend sends full DMN model to frontend on load | Frontend stores its own serialized copy of decision tables | Round-trip fidelity depends entirely on serializer; backend is single source of truth for canonical shape |

**Installation:** No new npm packages required. All dependencies already exist in `frontend/package.json`.

## Package Legitimacy Audit

No new external packages are introduced in this phase. The refactor reuses only existing frontend dependencies:
- `@tanstack/react-table` — already installed, 8+ years old, core to decision table rendering
- `@uiw/react-codemirror` — already installed, used for FEEL expression editing
- `lucide-react` — already installed, icon library

**Packages removed due to SLOP verdict:** none
**Packages flagged as suspicious [SUS]:** none

*All packages are existing dependencies; no registry verification needed.*

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│  Browser (React + TypeScript)                               │
│                                                             │
│  ┌──────────────────┐     ┌─────────────────────────────┐  │
│  │ HitPolicySelector │     │ DecisionTable (editor)      │  │
│  │ {policy, agg?}    │     │ columns / rows / conditions │  │
│  └────────┬─────────┘     └──────────────┬──────────────┘  │
│           │                              │                  │
│           ▼                              ▼                  │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ dmnSerializer.ts (IR projection layer)              │    │
│  │ editorShape ↔ DMN IR (InputClause, OutputClause,    │    │
│  │   DmnRule, HitPolicy, BuiltinAggregator)            │    │
│  └────────┬────────────────────────────────┬───────────┘    │
│           │                                │                │
│           ▼ (serialize to DMN XML)        ▼ (evaluate)     │
│  ┌──────────────────────┐       ┌──────────────────────┐   │
│  │ decisions.ts client   │       │ evaluateDecision()   │   │
│  │ GET /v1/decisions/{id}│       │ POST .../evaluate    │   │
│  │ PUT /v1/decisions/{id}│       │ inputs: IR-shaped    │   │
│  └──────────┬───────────┘       └──────────┬───────────┘   │
└────────────┼────────────────────────────────┼───────────────┘
             │                                │
             ▼                                ▼
┌──────────────────────────────────────────────────────────────┐
│  API Server (actix-web, Rust)                                 │
│                                                               │
│  ┌──────────────┐    ┌───────────────────────────────────┐   │
│  │ decisions.rs  │    │ DecisionEvaluator                  │   │
│  │ CRUD + import │    │ SharedDmnModel (Arc<RwLock>)      │   │
│  │ export + reload│   │ evaluate(decision_id, request)    │   │
│  └──────────────┘    │ → DRD resolution → table eval     │   │
│                      └───────────────────────────────────┘   │
│                                                               │
│  engine/src/dmn/                                                │
│  ├── model.rs (canonical IR: HitPolicy, DecisionTable)        │
│  ├── decision_table.rs (7 hit policies + COLLECT agg)         │
│  └── evaluator/mod.rs (§10.4 multi-output + DRD)              │
└───────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure
```
frontend/src/
├── api/
│   ├── decisions.ts          # Phase 33 API client (extend with new endpoints)
│   └── types.ts              # Existing shared API types
├── features/rules/
│   ├── types.ts              # REFACTOR: HitPolicy model → {policy, aggregator?}
│   ├── HitPolicySelector.tsx # REFACTOR: show aggregator selector for COLLECT
│   ├── DecisionTable.tsx     # REFACTOR: wire to real evaluateDecision; hide engine dropdown
│   └── AllowedValuesEditor.tsx  # NEW: editor for output column allowedValues
├── lib/
│   └── dmnSerializer.ts      # NEW: editorShape ↔ DMN IR mapper (single source of truth)
└── components/
    ├── TestRunner.tsx         # REUSE pattern: results panel + trace rendering
    └── DecisionEvalPanel.tsx  # NEW: reusable evaluation results component
```

### Pattern 1: Edit-Through-IR-Projection
**What:** The frontend owns an editor-friendly data shape (columns, rows with conditions/actions). A serializer module (`dmnSerializer.ts`) maps TO the canonical DMN IR for API calls and FROM the backend's response for rendering. The serializer is the single source of truth for shape conversion — no component directly constructs `InputClause` or `DmnRule` objects.

**When to use:** Every time the frontend needs to (a) send data to the backend evaluator, (b) display a decision loaded from the backend, or (c) import/export DMN XML. The serializer handles all three directions.

**Example:**
```typescript
// dmnSerializer.ts — maps editor shape to DMN IR for evaluation
export function editorShapeToDmnIr(
  columns: DecisionTableColumn[],
  rows: DecisionTableRow[],
  hitPolicy: HitPolicy,
  aggregator?: BuiltinAggregator,
): DmnDecisionTable {
  const inputClauses = columns
    .filter(col => col.type === 'condition')
    .map((col, i) => ({
      id: undefined,
      label: col.label,
      input_expression: col.accessorKey ?? col.label ?? `input_${i}`,
      input_values: undefined,
      description: undefined,
    }))

  const outputClauses = columns
    .filter(col => col.type === 'action')
    .map((col, i) => ({
      id: undefined,
      label: col.label,
      name: col.accessorKey ?? col.label ?? `output_${i}`,
      type_ref: undefined,
      output_values: undefined,
      default_output_entry: undefined,
      description: undefined,
    }))

  const rules = rows.map(row => ({
    id: undefined,
    description: undefined,
    input_entries: inputClauses.map(clause => ({
      text: { content: String(row.conditions[clause.label ?? clause.input_expression] ?? ''), is_cdata: false },
    })),
    output_entries: outputClauses.map(clause => ({
      text: { content: String(row.actions[clause.label ?? clause.name ?? ''] ?? ''), is_cdata: false },
    })),
    annotation_entries: [],
  }))

  return {
    id: undefined,
    name: undefined,
    hit_policy: hitPolicy,
    aggregation: aggregator,
    preferred_orientation: undefined,
    output_label: undefined,
    input_clauses: inputClauses,
    output_clauses: outputClauses,
    rules,
  }
}

// Reverse direction: DMN IR → editor shape for rendering
export function dmnIrToEditorShape(table: DmnDecisionTable): EditorShape {
  const columns = [
    ...table.input_clauses.map((cl, i) => ({
      id: `col-in-${i}`,
      label: cl.label ?? cl.input_expression,
      accessorKey: cl.input_expression,
      type: 'condition' as const,
    })),
    ...table.output_clauses.map((cl, i) => ({
      id: `col-out-${i}`,
      label: cl.label ?? cl.name ?? `Output ${i + 1}`,
      accessorKey: cl.name ?? `output_${i}`,
      type: 'action' as const,
    })),
  ]

  const rows = table.rules.map((rule, i) => ({
    id: `row-${i}`,
    conditions: Object.fromEntries(
      table.input_clauses.map((cl, j) => [
        cl.label ?? cl.input_expression,
        rule.input_entries[j]?.text.content ?? '',
      ])
    ),
    actions: Object.fromEntries(
      table.output_clauses.map((cl, j) => [
        cl.label ?? cl.name ?? `Output ${j + 1}`,
        rule.output_entries[j]?.text.content ?? '',
      ])
    ),
  }))

  return { columns, rows, hitPolicy: table.hit_policy, aggregation: table.aggregation }
}
```

### Pattern 2: Hit-Policy Data Model (DMN Canonical)
**What:** Replace the flat `HIT_POLICIES` array with a structured model where aggregators are sub-types of COLLECT. The frontend enum mirrors the backend's canonical shape exactly.

**When to use:** Every component that references hit policies — selectors, badges, validation rules, serializer mappings.

**Example:**
```typescript
// types.ts (refactored)
export type HitPolicy = 'UNIQUE' | 'ANY' | 'PRIORITY' | 'FIRST' | 'RULE ORDER' | 'OUTPUT ORDER' | 'COLLECT'

export type Aggregator = 'SUM' | 'MIN' | 'MAX' | 'COUNT'

/** Which hit policies support aggregation (only COLLECT in DMN spec). */
export const AGGREGATOR_POLICIES: HitPolicy[] = ['COLLECT']

/** Default aggregator for new COLLECT tables. */
export const DEFAULT_AGGREGATOR: Aggregator = 'SUM'

/** All aggregators available when hit policy is COLLECT. */
export const AGGREGATORS: { id: Aggregator; name: string }[] = [
  { id: 'SUM', name: 'Sum' },
  { id: 'MIN', name: 'Min' },
  { id: 'MAX', name: 'Max' },
  { id: 'COUNT', name: 'Count' },
]

/** Hit policy metadata — canonical names, DMN spec descriptions. */
export interface HitPolicyMeta {
  id: HitPolicy
  name: string
  description: string
  requiresAggregator: boolean
}

export const HIT_POLICY_META: Record<HitPolicy, HitPolicyMeta> = {
  UNIQUE: {
    id: 'UNIQUE',
    name: 'Unique',
    description: 'Only one rule can match. Multiple matches is a violation.',
    requiresAggregator: false,
  },
  ANY: {
    id: 'ANY',
    name: 'Any',
    description: 'Multiple rules can match; all must agree on outputs.',
    requiresAggregator: false,
  },
  PRIORITY: {
    id: 'PRIORITY',
    name: 'Priority',
    description: 'Select the matching rule whose output has highest allowedValues position.',
    requiresAggregator: false,
  },
  FIRST: {
    id: 'FIRST',
    name: 'First',
    description: 'First matching rule in document order wins. No conflict detection.',
    requiresAggregator: false,
  },
  'RULE ORDER': {
    id: 'RULE ORDER',
    name: 'Rule Order',
    description: 'All matching rules outputs returned in document order.',
    requiresAggregator: false,
  },
  'OUTPUT ORDER': {
    id: 'OUTPUT ORDER',
    name: 'Output Order',
    description: 'All matching rules outputs sorted by allowedValues enumeration.',
    requiresAggregator: false,
  },
  COLLECT: {
    id: 'COLLECT',
    name: 'Collect',
    description: 'Collect all matching rule outputs; optionally aggregate (SUM/MIN/MAX/COUNT).',
    requiresAggregator: true,
  },
}

/** All hit policy IDs as an array (for dropdown rendering). */
export const HIT_POLICIES: HitPolicy[] = Object.keys(HIT_POLICY_META) as HitPolicy[]
```

### Anti-Patterns to Avoid
- **Never reconstruct DMN from the frontend model without going through `dmnSerializer.ts`:** The serializer is the single source of truth for shape conversion. If a component builds `InputClause` or `DmnRule` objects directly, it will drift from the canonical IR and cause evaluation failures.
- **Do NOT use camelCase for hit-policy values:** The backend uses exact DMN spec strings (`UNIQUE`, `RULE ORDER`, etc.). The frontend must send these exact strings — not lowercase, not kebab-case.
- **Do NOT build a custom FEEL expression parser on the frontend:** Cell text is carried as raw strings and parsed server-side by Phase 31's FEEL parser with AST caching. The frontend only stores/retrieves the text.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Hit-policy enum mapping | Custom policy ID → DMN string converter | `HIT_POLICY_META` record + serializer | 7 variants × aggregator combinations = edge cases; backend is source of truth |
| Column/row ↔ rule mapping | Ad-hoc mapping in DecisionTable.tsx | `dmnSerializer.ts` `editorShapeToDmnIr()` / `dmnIrToEditorShape()` | Single conversion point prevents drift between render and evaluate paths |
| Allowed-values editor | Custom dropdown with free-text input | Reuse DMN `AllowedValues::Enumeration(Vec<TextContent>)` shape | Backend validates enumeration against PRIORITY/OUTPUT ORDER at load time |
| Evaluation request building | Manual `inputs` object construction in component | Serializer maps column accessorKeys to input expression names | Input key mismatch is the #1 cause of empty evaluation results |

**Key insight:** The DMN decision-table shape (input clauses, output clauses, rules with entry arrays) is structurally different from a spreadsheet-like editor shape (columns with conditions/actions per row). The serializer handles this structural translation in one place so that every downstream consumer (render, evaluate, import, export) uses the same mapping.

## Common Pitfalls

### Pitfall 1: Input Key Mismatch Between Editor and DMN Model
**What goes wrong:** The frontend sends evaluation inputs keyed as `input_0_age`, `input_1_income` etc., but the backend evaluator looks up values by the input expression name from the DMN model (e.g., `"age"`, `"income"`). Result: zero rules match because no input clause evaluates to true — all unary tests fail against an empty scope.
**Why it happens:** The current `DecisionTable.tsx` builds inputs with a generic prefix pattern (`input_${idx}_${key}`) that has nothing to do with the DMN model's `input_expression` values. This was acceptable when the editor had no real backend, but breaks evaluation against Phase 32/33.
**How to avoid:** The serializer maps column `accessorKey` (which IS the input expression name from the IR) directly to the input key in the evaluate request. Column definitions loaded from the backend already carry the correct `input_expression` as their identifier.
**Warning signs:** Evaluation returns `{ outputs: {}, rules_fired: 0 }` or all rules not fired — check that input keys match `input_clauses[].input_expression` exactly.

### Pitfall 2: Hit-Policy String Case Mismatch
**What goes wrong:** Frontend sends `hit_policy: "unique"` (lowercase) but the backend expects `"UNIQUE"` (uppercase per DMN spec). Result: the evaluator may not match the enum variant, or JSON deserialization fails silently.
**Why it happens:** The current frontend types use lowercase IDs (`'unique'`, `'any'`, etc.) while the Rust `HitPolicy` enum serializes to uppercase strings (`"UNIQUE"`, `"ANY"`).
**How to avoid:** Use the canonical uppercase DMN spec strings on both sides. The backend's `as_str()` method returns exact DMN attribute values; the frontend must send matching strings.
**Warning signs:** Backend returns a 500 or unexpected error on evaluate with valid inputs — check hit-policy string format in network tab.

### Pitfall 3: COLLECT Without Aggregator Returns Array, Not Scalar
**What goes wrong:** The frontend's evaluation result panel expects a single output value (like UNIQUE), but COLLECT without aggregator returns an array of all matching outputs. Result: the results panel renders `[object Object]` or crashes trying to stringify an array inline.
**Why it happens:** DMN spec §8.2.12 — COLLECT returns a list unless an aggregation is specified. The current `EvaluationResult.outputs` type (`Record<string, unknown>`) doesn't distinguish between scalar and list outputs well enough for display.
**How to avoid:** Check the hit policy before rendering results. For COLLECT without aggregator, render as a list. For COLLECT with aggregator, render as a single value. The backend already returns the correct shape — the frontend just needs to handle both.
**Warning signs:** Results panel shows `[]` or `[null]` for a table that should have matches — check hit policy and aggregator settings.

### Pitfall 4: FEEL Text Loss During Round-Trip
**What goes wrong:** Cell text containing FEEL expressions (e.g., `> 5`, `["A", "B"]`) gets serialized as plain strings through the API, but special characters or quotes get escaped/unescaped incorrectly, corrupting the expression.
**Why it happens:** The DMN IR carries cell text as `TextContent` with a CDATA flag for round-trip fidelity. If the frontend serializer strips CDATA metadata or the API client doesn't preserve exact string content, FEEL expressions break on re-evaluation.
**How to avoid:** Use JSON serialization (not form encoding) for all request/response bodies. The serializer carries `TextContent` with `is_cdata: false` as default — this is fine because JSON preserves string content exactly; CDATA only matters for XML round-trip (Phase 35).
**Warning signs:** A decision that works in the editor fails evaluation after save/reload — compare cell text before and after.

## Runtime State Inventory

> This phase involves frontend model changes and API wiring, but no rename/migration of stored strings or runtime state. The refactor is purely a data-model alignment between frontend and backend.

| Category | Items Found | Action Required |
|----------|------------|------------------|
| Stored data | None — the editor shape lives only in React component state (not persisted) | Code edit: update types.ts, HitPolicySelector.tsx, DecisionTable.tsx |
| Live service config | None — no external services embed hit-policy strings | None |
| OS-registered state | None | None |
| Secrets/env vars | None | None |
| Build artifacts | `frontend/node_modules/` — no changes needed (no new packages) | None |

**Nothing found in category:** State explicitly confirmed for all 5 categories. This phase is a frontend model refactor + API wiring change with zero runtime state impact.

## Environment Availability

> The phase has no external dependencies beyond the existing frontend toolchain and backend server.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Node.js | Frontend build/dev | ✓ | (check via `node --version`) | — |
| npm | Frontend dependency management | ✓ | (check via `npm --version`) | — |
| Backend server (`cargo run`) | API integration testing | ✗ | — | Run backend in a separate terminal; tests against mock API are sufficient for unit work |
| Vitest | Frontend test runner | ✓ | ^2.0.0 (in package.json) | `npx vitest` |

**Missing dependencies with no fallback:** None identified — all required tools exist in the frontend project's own toolchain.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Vitest ^2.0.0 (jsdom environment) |
| Config file | `frontend/vite.config.ts` → `test` section |
| Quick run command | `cd frontend && npx vitest run --reporter=verbose` |
| Full suite command | `cd frontend && npx vitest run` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DMN-29 | HitPolicySelector renders 7 canonical policies with correct names/descriptions | unit | `vitest run frontend/src/features/rules/__tests__/HitPolicySelector.test.tsx -x` | ❌ Wave 0 |
| DMN-30 | Serializer maps editor shape ↔ DMN IR correctly (all 7 hit policies) | unit | `vitest run frontend/src/lib/__tests__/dmnSerializer.test.ts -x` | ❌ Wave 0 |
| DMN-31 | EvaluateDecision button calls backend with correct input keys from column definitions | integration | `vitest run frontend/src/features/rules/__tests__/DecisionTable.evaluate.test.tsx -x` | ❌ Wave 0 |
| DMN-32 | AllowedValues editor persists enumeration values through serializer round-trip | unit | `vitest run frontend/src/features/rules/__tests__/AllowedValuesEditor.test.tsx -x` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cd frontend && npx vitest run --reporter=verbose` (quick, < 30s)
- **Per wave merge:** `cd frontend && npx vitest run` (full suite)
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `frontend/src/features/rules/__tests__/HitPolicySelector.test.tsx` — covers DMN-29 (7 policies rendered with canonical names)
- [ ] `frontend/src/lib/__tests__/dmnSerializer.test.ts` — covers DMN-30 (editorShape ↔ IR round-trip for all 7 hit policies + aggregators)
- [ ] `frontend/src/features/rules/__tests__/DecisionTable.evaluate.test.tsx` — covers DMN-31 (input key mapping from column accessorKeys)
- [ ] `frontend/src/features/rules/__tests__/AllowedValuesEditor.test.tsx` — covers DMN-32 (allowed values persistence)
- [ ] Framework install: already present (`vitest ^2.0.0` in package.json, jsdom environment configured)

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 9 flat hit policies (sum/min/max/count as siblings) | 7 canonical + COLLECT aggregator sub-type | Phase 32 backend (Rust enum) | Frontend must migrate to match; serializer handles old→new format conversion |
| No IR projection layer | `dmnSerializer.ts` maps editor shape ↔ DMN IR | This phase (Phase 34) | Single source of truth for shape conversion; prevents drift |
| Engine dropdown visible in decision table view | Engine dropdown hidden on DMN view | This phase (D-02) | Eliminates confusion: DMN decisions use the DMN evaluator, not Rete engines |
| Hardcoded `input_${idx}_${key}` input keys | Column `accessorKey` mapped to input expression name | This phase (D-03) | Evaluation works against real decisions loaded from backend |

**Deprecated/outdated:**
- `HIT_POLICIES` array with 9 flat entries in `types.ts` — replaced by `HIT_POLICY_META` record + `HitPolicy` type union
- `requiresAggregation: boolean` flag on HitPolicy — replaced by `AGGREGATOR_POLICIES` lookup array (only COLLECT)
- `DecisionTable` hardcoded `HIT_POLICY_OPTIONS` — removed; uses `HIT_POLICY_META` from types

## Assumptions Log

> List all claims tagged [ASSUMED] in this research. The planner and discuss-phase use this section to identify decisions that need user confirmation before execution.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The backend `POST /v1/decisions/{id}/evaluate` request body accepts `inputs: Record<string, Value>` where keys are input expression names from the DMN model (not generic field names) | API Contract Gaps | If wrong, all evaluation tests will fail with empty results; frontend must be redesigned to load decision metadata first |
| A2 | The Phase 33 `DecisionDetail` response from `GET /v1/decisions/{id}` includes the full decision table IR (input_clauses, output_clauses, rules) — not just summary metadata | API Contract Gaps | If wrong, a new endpoint or expanded response shape is needed to load full table data for editing |
| A3 | The existing `frontend/src/api/client.ts` uses snake_case JSON keys matching the backend serde shapes (consistent with the pattern documented in api/types.ts) | API Contract Gaps | If wrong, all request/response field names need renaming; affects every API call |
| A4 | No migration of persisted decision data is needed because decisions are stored as raw DMN XML on disk (`FileSystemDmnStore`) — the frontend model change only affects how the editor interprets that XML | Migration Path | Low risk: if wrong, a data migration task must be added to Wave 1 |
| A5 | The `DecisionTable` component's current import of `evaluateDecision` from `@/api/decisions` can be wired directly without API changes because the Phase 33 client already exports this function | API Contract Gaps | If wrong, minimal adapter code in decisions.ts is needed; no backend changes required |

**If this table is empty:** All claims in this research were verified or cited — no user confirmation needed.

## Open Questions (RESOLVED)

1. **Does `GET /v1/decisions/{id}` return the full decision table IR (input_clauses, output_clauses, rules) or just metadata?** [RESOLVED: Plan 02 Task 2 — new endpoint `GET /v1/decisions/{id}/ir` returns parsed DecisionTable as JSON]
   - What we know: The current Phase 33 response returns `id`, `name`, `content_length`, `metadata`, timestamps — it's a file-store get, not an IR parse. It does NOT include the parsed decision table structure.
   - Resolution: Add a new endpoint `GET /v1/decisions/{id}/ir` in Plan 02 that returns the parsed DecisionTable as JSON (not raw XML). The frontend loads full table data for editing via this endpoint, then passes it through `dmnIrToEditorShape()` for rendering. This eliminates the need for client-side DMN parsing and keeps IR serialization authoritative on the backend.

2. **Should the allowed-values editor support both Enumeration and UnaryTests formats?** [RESOLVED: Plan 03 Task 2 — start with Enumeration-only; UnaryTests deferred to Phase 35]
   - What we know: The DMN model's `AllowedValues` enum has two variants: `UnaryTests(String)` (FEEL text) and `Enumeration(Vec<TextContent>)` (literal values). PRIORITY/OUTPUT ORDER require Enumeration.
   - Resolution: Start with Enumeration-only for allowed values (covers PRIORITY/OUTPUT ORDER, which are the main use cases for this phase). UnaryTests can be added in Phase 35 when full DMN authoring fidelity is needed. The `dmnSerializer.ts` in Plan 02 includes `AllowedValuesShape` type with both kinds to support forward compatibility.

## Security Domain

> `security_enforcement` is enabled by default (absent = enabled in config). This phase has minimal security surface because it's a frontend model refactor + API wiring change that does not introduce new authentication, authorization, or data persistence paths.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | N/A — no auth changes |
| V3 Session Management | no | N/A — no session changes |
| V4 Access Control | no | N/A — existing RBAC applies server-side |
| V5 Input Validation | yes | Frontend validates hit-policy string against known set before sending to backend; backend enum validation is authoritative |
| V6 Cryptography | no | N/A — no crypto changes |

### Known Threat Patterns for This Phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malformed hit-policy string in evaluate request | Tampering | Frontend validates against `HIT_POLICY_META` keys; backend Rust enum deserialization rejects unknown values |
| XSS via cell text content (FEEL expressions stored as strings) | Injection | Cell text passed through CodeMirror (escaped); no HTML interpolation of raw cell content |

## Sources

### Primary (HIGH confidence)
- `engine/src/dmn/model.rs` — canonical HitPolicy enum (7 variants), BuiltinAggregator, DecisionTable IR structure [VERIFIED: codebase grep]
- `engine/src/dmn/decision_table.rs` — all 7 hit-policy handlers implemented; COLLECT aggregators (SUM/MIN/MAX/COUNT) [VERIFIED: codebase grep]
- `engine/src/dmn/evaluator/mod.rs` — EvaluateRequest/EvaluateResult shapes, DRD resolution, SharedDmnModel pattern [VERIFIED: codebase grep]
- `api/src/v1/decisions.rs` — POST /v1/decisions/{id}/evaluate request/response types, route configuration [VERIFIED: codebase grep]
- `frontend/src/api/decisions.ts` — Phase 33 API client with evaluateDecision, listDecisions, getDecision functions [VERIFIED: codebase grep]
- `frontend/src/features/rules/types.ts` — current HitPolicy interface and HIT_POLICIES array (9 entries) [VERIFIED: codebase grep]
- `frontend/src/features/rules/HitPolicySelector.tsx` — current selector component using flat HIT_POLICIES list [VERIFIED: codebase grep]
- `frontend/src/features/rules/DecisionTable.tsx` — current editor component with hardcoded options + evaluate wiring [VERIFIED: codebase grep]

### Secondary (MEDIUM confidence)
- `frontend/vite.config.ts` — Vitest configuration (jsdom env, setupFiles) [VERIFIED: codebase grep]
- `frontend/src/components/TestRunner.tsx` — existing trace rendering pattern to reuse [VERIFIED: codebase grep]
- `frontend/src/types/common.ts` — existing shared types including HitPolicy interface [VERIFIED: codebase grep]

### Tertiary (LOW confidence)
- Phase 34 CONTEXT.md decisions — hybrid approach, aggregator integration into COLLECT, UI refactor scope [CITED: .planning/phases/34/frontend-wiring-hit-policy-model-refactor/34-CONTEXT.md]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all packages are existing dependencies verified in package.json; no new libraries needed
- Architecture: HIGH - backend IR types and API contract directly observed from source code; frontend component structure directly observed
- Pitfalls: HIGH - input key mismatch identified as #1 gap by comparing frontend evaluate logic against backend EvaluateRequest type

**Research date:** 2026-07-09
**Valid until:** 30 days (stable — based on existing code, not external dependencies)
