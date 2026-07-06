# Architecture Research

**Domain:** DMN Level 3 compliance integrated into an existing production BRMS (Rust engine + actix-web API + React/TS frontend)
**Researched:** 2026-07-06
**Confidence:** HIGH (integration with existing rRules codebase verified by reading source; DMN/FEEL semantics verified against OMG spec + Camunda/KIE references)

## Standard Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                              FRONTEND (React/TS)                              │
│  DecisionTable.tsx + HitPolicySelector.tsx ──[serializer]──▶ DecisionEditor   │
│  new: api/decisions.ts ──▶ "Evaluate Decision" button + results/trace panel    │
└─────────────────────────────────────┬────────────────────────────────────────┘
                                      │ HTTP (VITE_API_URL → /v1)
┌─────────────────────────────────────▼────────────────────────────────────────┐
│                            API CRATE (actix-web)                              │
│  /v1/execute (existing)            /v1/decisions (NEW)                         │
│  ExecuteRequest/Response           DecisionEvaluateRequest/Response (NEW)     │
│  execution.rs ── EngineRegistry     decisions.rs (NEW) ── DecisionEvaluator     │
│       │                                  │                                    │
│       │  web::Data<EngineRegistry>        │  web::Data<DecisionEvaluator>      │
│       │  web::Data<ExecutionCounters>     │  web::Data<DecisionCounters> (NEW) │
│       ▼                                  ▼                                    │
│  ExecutionCounters ◀── shared dedicated Prometheus Registry (existing)        │
│  + DecisionCounters (NEW, same Registry)                                      │
│  MetricsSnapshot broadcast (existing channel, reused)                          │
└─────────────────────────────────────┬────────────────────────────────────────┘
                                      │ engine crate (api → engine, one-way dep)
┌─────────────────────────────────────▼────────────────────────────────────────┐
│                            ENGINE CRATE                                       │
│  EXISTING (unchanged)               NEW: engine/src/dmn/                       │
│  rules.rs (Expression/Condition/     ├── model.rs    DMN 1.5 model structs    │
│    Action/RuleSet/Value)             ├── parser.rs  .dmn XML → model          │
│  engine/ (Rete/TREAT/LEAPS/PHREAK    ├── feel.rs    FEEL evaluator            │
│    + registry + direct_eval)         ├── decision_table.rs hit-policy engine  │
│  monitoring/ (coverage, dead_rules,  ├── evaluator.rs decision orchestrator     │
│    detection)                        └── exporter.rs model → .dmn (round-trip) │
│  registry.rs (TraceEvent/TraceData ──────────────── reused by DMN trace)      │
└────────────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|-----------------|------------------------|
| `engine/src/dmn/model.rs` | DMN 1.5 in-memory model: Definitions, Decision, DecisionTable, InputClause, OutputClause, Rule, BusinessKnowledgeModel, DecisionService, ItemDefinition | Plain `#[derive(Serialize, Deserialize)]` structs mirroring the DMN 1.5 XSD (`DMN15.xsd`). Reuse `engine::data_types::Value` as the FEEL value type. |
| `engine/src/dmn/parser.rs` | Parse `.dmn` XML (DMN 1.5 interchange) into the model. Validate structure. | `quick-xml` (serde feature) deserializer; alternative: `roxmltree` (as `dsntk-model` does). Attribute rename `#[serde(rename = "@...")]`. |
| `engine/src/dmn/feel.rs` | Full FEEL expression-language evaluator: boxed expressions (literal/context/list/relation/function invocation), if/for/quantified, arithmetic/comparison/boolean/string/date-time operators, built-in function library, three-valued logic + coercion + null. | Recursive-descent parser → FEEL AST → evaluator over `Value`. AST reuses `Expression` where semantics align, extends with FEEL-only variants. |
| `engine/src/dmn/decision_table.rs` | Decision-table engine: all 7 hit policies (U/A/P/F/C/R/O) + COLLECT aggregators (SUM/MIN/MAX/COUNT). Compound-output restrictions. Default-output handling. No-match → null semantics. | Stateless evaluator: evaluate all rules unordered (except First/Rule Order), collect matches, apply hit policy + aggregation. |
| `engine/src/dmn/evaluator.rs` | Decision-evaluation orchestrator: resolve a `Decision` by id, evaluate its decision logic (table or boxed expression) against input data, support DRD dependency resolution (decisions referencing other decisions/BKMs). | Topological eval over the DRD; per-request construction (mirrors v5.0 per-request `Box<dyn ExecutionEngine>` pattern). |
| `engine/src/dmn/exporter.rs` | Serialize the model back to `.dmn` XML for round-trip. | `quick-xml` serializer (inverse of parser). |
| `api/src/v1/decisions.rs` (NEW) | REST surface: `POST /v1/decisions/{id}/evaluate`, `GET /v1/decisions`, `GET/POST/PUT/DELETE /v1/decisions/{id}` (CRUD), import/export `.dmn`. Wire to metrics + trace + WS broadcast. | actix-web handlers following `execution.rs` patterns; `web::Data<DecisionEvaluator>` + `web::Data<DecisionCounters>`. |
| `DecisionCounters` (NEW, in `api/src/metrics.rs`) | Per-decision Prometheus counters, mirror `ExecutionCounters`. | `IntCounterVec`/`HistogramVec` registered on the EXISTING dedicated `Registry` (same `Registry::new()`, not the default registry). |
| Frontend serializer (NEW) | Convert the existing `DecisionTable.tsx` editor shape (`DecisionTableColumn`/`DecisionTableRow`) ↔ backend DMN model (InputClause/OutputClause/DTRule with FEEL expressions). | A `toDmnModel()`/`fromDmnModel()` mapper in `frontend/src/features/rules/` + a new `api/decisions.ts` client. |

## Recommended Project Structure

```
engine/src/
├── dmn/                       # NEW — DMN Level 3 module (this milestone)
│   ├── mod.rs                 #   pub re-exports
│   ├── model.rs               #   DMN 1.5 model structs (Definitions, Decision, DecisionTable, ...)
│   ├── parser.rs             #   .dmn XML → model (quick-xml + serde)
│   ├── exporter.rs           #   model → .dmn XML (round-trip)
│   ├── feel/
│   │   ├── mod.rs             #   FEEL evaluator entry
│   │   ├── ast.rs             #   FEEL AST (reuses engine::rules::Expression where aligned)
│   │   ├── eval.rs            #   three-valued logic, coercion, built-in functions
│   │   └── builtins.rs        #   DMN built-in function library (string/list/numeric/datetime/context)
│   ├── decision_table.rs     #   hit-policy engine (U/A/P/F/C/R/O + aggregators)
│   └── evaluator.rs           #   decision orchestrator + DRD dependency resolution
├── rules.rs                   # EXISTING — Expression/Condition/Action/Rule/RuleSet (REUSED, extended)
├── data_types.rs              # EXISTING — Value enum (REUSED as the FEEL value type, extended)
├── engine/                    # EXISTING — Rete family (UNCHANGED; DMN does NOT go through this)
└── monitoring/                # EXISTING — coverage/dead_rules/detection (EXTENDED for decisions)

api/src/
├── v1/
│   ├── decisions.rs           # NEW — decision-evaluation REST surface
│   ├── execution.rs           # EXISTING — rule execution (UNCHANGED)
│   └── mod.rs                 # MODIFY — register decisions module + configure_v1_decisions()
├── metrics.rs                 # MODIFY — add DecisionCounters on the existing dedicated Registry
└── main.rs                    # MODIFY — wire /v1/decisions routes + DecisionEvaluator app state

frontend/src/
├── features/rules/
│   ├── DecisionTable.tsx      # MODIFY — align hit-policy IDs to DMN canonical (U/A/P/F/C/R/O)
│   ├── HitPolicySelector.tsx # MODIFY — fix missing FIRST/OUTPUT ORDER; split COLLECT aggregator
│   ├── dmnSerializer.ts       # NEW — editor shape ↔ backend DMN model
│   └── types.ts               # MODIFY — DMN-aligned types (InputClause/OutputClause/DTRule)
├── api/
│   ├── decisions.ts           # NEW — decision CRUD + evaluate client (mirrors execution.ts)
│   └── types.ts               # MODIFY — DecisionEvaluateRequest/Response + DMN model types
└── routes/                    # MODIFY — add decision-evaluation route/page
```

### Structure Rationale

- **`engine/src/dmn/` as a sibling of `engine/` (not inside it):** DMN is a parallel evaluation model, not a Rete-family algorithm. The existing `engine/engine/{rete,treat,leaps,phreak}` directory holds forward-chaining implementations behind the `ExecutionEngine` trait; DMN does not satisfy that trait (no fixpoint loop, no working-memory mutation, no rule firing). Keeping `dmn/` at the same level as `engine/` makes "DMN is its own surface" obvious and prevents anyone wiring it through `EngineRegistry` by mistake.
- **`feel/` as a sub-module of `dmn/`:** FEEL is the expression language OF DMN; it has no meaning outside DMN decision logic. Nesting it keeps the dependency direction clean (FEEL → `data_types::Value`, nothing else).
- **`decisions.rs` mirrors `execution.rs`:** one route module per REST surface, matching the established `api/src/v1/{execution,rules,sessions,...}` convention.
- **`DecisionCounters` in the existing `metrics.rs`:** Prometheus counters must share the existing dedicated `Registry::new()` so `/metrics` renders them in one `TextEncoder` pass. Adding a second registry would fragment the scrape endpoint.

## Architectural Patterns

### Pattern 1: NEW execution surface, NOT compiled-to-rules

**What:** DMN decisions are evaluated by a dedicated `DecisionEvaluator` behind `POST /v1/decisions/{id}/evaluate`, NOT compiled into the existing `Rule`/`RuleSet` format and run through the Rete/TREAT/LEAPS/PHREAK `ExecutionEngine`.

**When to use:** Always for this milestone. This is the single most consequential architectural decision and it is settled.

**Trade-offs:**
- PRO: Preserves DMN semantics that don't map to forward-chaining — hit policies (Unique/Any/Priority violation errors, Output-Order sorting by allowed-values), three-valued FEEL logic (null ≠ false), stateless single-pass evaluation (no fixpoint loop, no working-memory mutation, no rule-firing-once-per-call guard), compound-output restrictions, Collect aggregators reducing to a scalar. Compiling to rules would require the Rete engine to gain error-on-overlap semantics and lose its forward-chaining nature.
- PRO: DMN decisions are side-effect-free by spec (FEEL has no side effects). The existing engine mutates working memory (`Action::UpdateFact`/`InsertFact`) — forcing DMN into that model would violate the DMN conformance contract.
- PRO: Keeps the Rete engines untouched (they are v5.0-shipped, audit-passed, and carry `set_collect_trace`/`load_ruleset`/`get_facts` obligations). No regression risk to the production rule-execution path.
- CON: Two evaluators to maintain. Mitigated by sharing `Value`, the FEEL ↔ `Expression` alignment where semantics agree, and the same `TraceEvent`/`TraceData` types.
- CON: No Rete node-sharing / alpha-network optimization for large decision tables. The KIE ANC benchmarks show compiled evaluation is ~2.4x faster than brute-force on large tables with shared constraints. rRules decision tables are authored in a web IDE (human-scale, hundreds of rows, not millions) — the interpreted evaluator is fast enough and the simplicity + DMN-semantic-correctness wins.

**Example (decision evaluation vs rule execution):**
```rust
// EXISTING rule execution (forward chaining, fixpoint, mutates working memory):
// api/src/v1/execution.rs::execute_single
let engine = registry.create_engine(&engine_name)?;   // fresh Box<dyn ExecutionEngine>
engine.load_ruleset(&ruleset)?;
engine.insert_facts(facts);
let result = web::block(move || engine.execute()).await?;  // fixpoint loop

// NEW decision evaluation (stateless, single-pass, side-effect-free):
// api/src/v1/decisions.rs::evaluate_decision
let evaluator = web::Data::<DecisionEvaluator>::clone(&req_data);  // shared, stateless
let result = web::block(move || {
    evaluator.evaluate(&decision_id, &input_context)  // one pass, no fixpoint
}).await?;
// result: DecisionResult { outputs, matched_rules, trace: Option<TraceData> }
```

### Pattern 2: REUSE `Value` as the FEEL value type; EXTEND, don't fork

**What:** The existing `engine::data_types::Value` enum (`Bool/Integer/Float/String/DateTime/Object/Array/Null`) is the FEEL value type. FEEL needs the same variants plus FEEL-specific extensions (durations, ranges/interval literals, function values).

**When to use:** Always — forking a separate `FeelValue` would force N conversion functions at every DMN↔engine boundary and double the test surface.

**Trade-offs:**
- PRO: FEEL `null` maps to `Value::Null`; FEEL lists → `Value::Array`; FEEL contexts → `Value::Object`; FEEL numbers → `Integer`/`Float` (FEEL's `number` is one type — Rust models it as the existing two variants, matching the `numeric_op` Integer-preserving promotion already in `rules.rs`).
- PRO: The existing `compare_values` (rules.rs:368) and `DynamicCast` (data_types.rs) are reusable as FEEL comparison/coercion primitives.
- CON: FEEL needs `Duration` (years/months/days/time), `Range` (interval literals `a..b`, open/closed), and `Function` (closure) values not present in `Value`. Add them as new variants rather than a parallel type. This is additive and non-breaking (existing `Expression::evaluate` arms stay valid).
- CON: `Value` derives `PartialEq` but not `Eq` (f64 NaN) — FEEL three-valued logic needs custom `feel_equals` that treats NaN/null per spec rather than Rust `==`. Add a `feel_eq` helper in `feel/eval.rs` rather than changing `PartialEq` semantics used by the rule engine.

**Example (extend Value for FEEL):**
```rust
// engine/src/data_types.rs — ADD variants (non-breaking, additive)
pub enum Value {
    // ... existing variants unchanged ...
    /// FEEL days/time duration (DMN 1.5 §10.3.2.6)
    DaysTimeDuration(chrono::Duration),
    /// FEEL years/months duration
    YearsMonthsDuration(i64),  // months
    /// FEEL range/interval literal (a..b, (a..b), [a..b], (a..b])
    Range { start: Box<Value>, end: Box<Value>, start_open: bool, end_open: bool },
    /// FEEL function value (closure over a context)
    Function { name: Option<String>, params: Vec<String>, body: Box<FeelAst> },
}
```

### Pattern 3: REUSE `TraceEvent`/`TraceData`; extend `event_type`

**What:** DMN evaluation emits the same `engine::engine::registry::{TraceEvent, TraceData}` types the rule evaluator emits, with `event_type` extended beyond `"rule_fired"`.

**When to use:** When trace is opt-in on `POST /v1/decisions/{id}/evaluate` (mirror v5.0 `ExecuteRequest.trace`).

**Trade-offs:**
- PRO: One wire type — the frontend results/trace panel already renders `InferenceTrace`/`InferenceTraceEntry` (added in Phase 28); DMN trace reuses it.
- PRO: `TraceEvent` is owned by the engine crate (dependency direction `api → engine` already established — Critical Finding 1 in v5.0 research). DMN lives in the engine crate, so it can construct `TraceEvent` directly.
- CON: `event_type: &'static str` is currently always `"rule_fired"`. Change to `Cow<'static, str>` (or just `&'static str` with new constants like `"decision_evaluated"`, `"row_matched"`) so DMN events are distinguishable without allocation.

**Example:**
```rust
// engine/src/dmn/evaluator.rs — emit trace on decision evaluation
let event = TraceEvent {
    rule_id: decision_id.to_string(),      // reuse field name (semantic: "what fired")
    timestamp: chrono::Utc::now().to_rfc3339(),
    event_type: "decision_evaluated",        // new &'static str constant
};
```

### Pattern 4: Per-request evaluator construction (mirror v5.0, without `EngineRegistry`)

**What:** DMN evaluation does NOT use `EngineRegistry` — decisions are stateless and need no fresh `Box<dyn ExecutionEngine>`. But the evaluator should still be constructed/used per-request inside `web::block` to keep the CPU-bound evaluation off the actix async executor (Pitfall 1 from v5.0).

**When to use:** Every `POST /v1/decisions/{id}/evaluate` call.

**Trade-offs:**
- PRO: No `&mut self` shared across requests; no `MutexGuard` captured across `web::block` (the exact pitfall v5.0 Pitfall 7 solved).
- PRO: The `DecisionEvaluator` can be `Send + Sync + 'static` and held in `web::Data` (it's stateless — the DMN model is loaded separately, like `SharedRuleSet`).
- CON: The DMN model needs a shared snapshot analogous to `SharedRuleSet{ruleset, generation}` under `Arc<RwLock>`. Recommend `SharedDmnModel{model, generation}` with the same swap-don't-mutate + `generation.wrapping_add(1)` pattern (v5.0 Phase 25 D-02).

**Example:**
```rust
// api/src/main.rs — DMN app state (mirrors EngineState/SharedRuleSet)
let dmn_model = web::Data::new(SharedDmnModel::new(load_dmn_model_from_dir(&dmn_dir)));
let decision_counters = web::Data::new(DecisionCounters::new(registry.clone())); // same Registry
// /v1/decisions/{id}/evaluate → decisions::evaluate_decision(
//     dmn_model: web::Data<SharedDmnModel>,
//     counters: web::Data<DecisionCounters>,
//     ...)
```

## Data Flow

### End-to-end decision evaluation flow

```
[Author edits DecisionTable.tsx]
    ↓ (frontend editor shape: DecisionTableColumn/DecisionTableRow)
[dmnSerializer.ts → backend DMN model (InputClause/OutputClause/DTRule w/ FEEL text)]
    ↓ POST /v1/decisions  (create/update — stores .dmn XML via Git, like rules)
[api/src/v1/decisions.rs::create_decision]
    ↓ engine/src/dmn/exporter.rs (model → .dmn XML) OR store JSON model
[Git-backed rules dir (.dmn files)]  ← file watcher (existing) triggers reload
    ↓ start_file_watcher → SharedDmnModel swap (generation+1)  [mirrors v5.0 reload]
[SharedDmnModel under Arc<RwLock>]  ← read on each evaluate (coherent snapshot)
    ↓
[POST /v1/decisions/{id}/evaluate  with  { input: { ...FEEL context... }, trace?: bool }]
    ↓
[decisions.rs::evaluate_decision]
    ├── resolve Decision by id from SharedDmnModel (read lock, capture generation)
    ├── build DecisionEvaluator (stateless) with collect_trace flag
    └── web::block(move || evaluator.evaluate(&decision, &input_ctx))
         ↓
    [engine/src/dmn/evaluator.rs::evaluate]
         ├── resolve DRD dependencies (other Decisions/BKMs) — topological eval
         ├── for a DecisionTable decision:
         │     └── decision_table.rs::evaluate_table
         │           ├── evaluate each rule's input entries via feel.rs::eval
         │           │     (FEEL expression → Value, three-valued logic)
         │           ├── collect matched rules
         │           ├── apply hit policy (U/A/P/F/C/R/O + aggregators)
         │           └── emit TraceEvent per matched row (if trace on)
         ├── for a boxed-expression decision:
         │     └── feel.rs::eval_boxed (literal/context/list/relation/if/for/quantified)
         └── return DecisionResult { outputs: Value, matched_rule_ids, trace: Option<TraceData> }
    ↓
[decisions.rs post-web::block]
    ├── DecisionCounters.requests_total.inc()  (engine-labeled "dmn")
    ├── DecisionCounters.duration_seconds.observe(elapsed)
    ├── MetricsSnapshot broadcast on existing WS channel (reuse MetricsTx)
    └── map TraceData → InferenceTrace (existing wire type) → DecisionEvaluateResponse
    ↓
[Frontend results panel — reuses InferenceTrace rendering from TestRunner]
```

### Key Data Flows

1. **`.dmn` import flow:** `POST /v1/decisions/import` (multipart `.dmn` XML) → `engine/src/dmn/parser.rs` → model → validate → store. This is the standard DMN interchange entry point and enables round-trip with external DMN modelers (Camunda, Trisotech).
2. **Frontend editor → backend flow:** The existing `DecisionTable.tsx` emits a *frontend-specific* shape (`columns: DecisionTableColumn[]`, `rows: DecisionTableRow{conditions, actions}`) that is NOT DMN-compatible. A NEW serializer (`dmnSerializer.ts`) maps it to the backend DMN model: `DecisionTableColumn.id` containing `"condition"` → `InputClause`; columns → `OutputClause`; row `conditions`/`actions` records → `DTRule` input/output entries as FEEL expression text. This is the single biggest frontend gap (see Integration Points).
3. **Reload/snapshot flow:** The existing file watcher (`start_file_watcher`) + `SharedRuleSet` swap pattern is extended to a parallel `SharedDmnModel` — same `Arc<RwLock>`, same `generation.wrapping_add(1)`, so `DecisionEvaluateResponse.dmn_generation` mirrors `ExecuteResponse.engine_generation`.
4. **Trace flow:** Opt-in `trace: bool` on `DecisionEvaluateRequest` (mirrors `ExecuteRequest.trace`) → `TraceData` from the engine → mapped to `InferenceTrace` on the wire → rendered by the EXISTING frontend trace panel (Phase 28). Latency-neutral when off (no `Vec`, no `chrono` — same SC-3 discipline as v5.0).

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 0-1k users | Per-request `web::block` evaluation + in-memory `SharedDmnModel` snapshot (this milestone). Fine — decision tables are tens-to-hundreds of rows. |
| 1k-100k users | Cache compiled FEEL ASTs (parse-once-eval-many). The `DecisionEvaluator` already holds the parsed model; add an AST cache keyed by expression-text-hash to avoid re-parsing FEEL on every request. No architectural change. |
| 100k+ users | Consider the KIE ANC-style compile-to-Rete optimization for hot, large decision tables (shared-constraint node sharing). OUT OF SCOPE for this milestone — only revisit if a decision table exceeds ~1k rows AND is on a hot path. |

### Scaling Priorities

1. **First bottleneck:** FEEL parsing on every request. Mitigation: parse-once (the `SharedDmnModel` already holds the parsed model; FEEL expression text within rules can be pre-parsed into ASTs at load time and stored on the `DTRule`).
2. **Second bottleneck:** DRD dependency resolution for deep decision graphs. Mitigation: topological-sort once at load time, cache the eval order on the model.

## Anti-Patterns

### Anti-Pattern 1: Compiling DMN decisions into `Rule`/`RuleSet` and running them through the Rete engine

**What people do:** Translate each decision-table row into a `Rule` with `Condition` LHS + `Action::UpdateFact` RHS, load into `EngineRegistry`, and let the fixpoint loop "fire" them.
**Why it's wrong:** (a) DMN is stateless + side-effect-free; the Rete engine mutates working memory and runs a fixpoint loop — wrong semantics. (b) Hit policies have no analogue: Unique/Any *violation* must raise an error, not silently pick a fired rule; Priority sorts by the output-column allowed-values list (not rule salience); First short-circuits (Rete evaluates all then resolves). (c) FEEL three-valued logic (`null` ≠ `false`) doesn't match `Condition::evaluate` returning `bool`. (d) It regresses the production rule path (v5.0 audit-passed) by adding a second load path into the engines.
**Do this instead:** Separate `DecisionEvaluator` + `POST /v1/decisions/{id}/evaluate`. Share `Value` + `TraceEvent`/`TraceData` + the Prometheus registry, but not the execution engine.

### Anti-Pattern 2: Forking a separate `FeelValue` type

**What people do:** Define a parallel `FeelValue` enum because "FEEL has null/duration/range that `Value` doesn't."
**Why it's wrong:** Doubles the conversion + test surface at every DMN↔engine boundary; forces `compare_values` to be reimplemented for FEEL; makes trace payloads (`TraceEvent` carries a `rule_id: String`, not a value) inconsistent.
**Do this instead:** Extend `engine::data_types::Value` with the missing variants (`DaysTimeDuration`, `YearsMonthsDuration`, `Range`, `Function`). Add a `feel_eq`/`feel_compare` helper in `feel/eval.rs` for three-valued logic rather than mutating `Value::PartialEq` used by the rule engine.

### Anti-Pattern 3: A second Prometheus `Registry` for decision metrics

**What people do:** `Registry::new()` for `DecisionCounters` so "decision metrics are separate."
**Why it's wrong:** The `/metrics` endpoint renders ONE registry via `TextEncoder` (v5.0 Phase 24 deliberately uses a dedicated registry, NOT `prometheus::default_registry()`). A second registry means a second scrape pass or a merged render — fragmenting the single-source-of-truth design.
**Do this instead:** Register `DecisionCounters` on the SAME dedicated `Registry` that `ExecutionCounters` uses. New metric names: `rrules_decision_requests_total`, `rrules_decision_duration_seconds` (labeled `decision_id`/`hit_policy`). One `TextEncoder`, one `/metrics`.

### Anti-Pattern 4: Treating the frontend `DecisionTable.tsx` editor shape as already DMN-compatible

**What people do:** Assume `DecisionTableColumn`/`DecisionTableRow` can be POSTed straight to the backend as a DMN model.
**Why it's wrong:** The existing shape is a generic table editor (`conditions: Record<string, unknown>`, `actions: Record<string, unknown>`) with no DMN concepts (InputClause, OutputClause, allowed-values list for Priority/Output-Order, FEEL expression text, hit-policy *aggregation operator*). The hit-policy ID list is also wrong/incomplete (missing `first` and `output-order`; conflates `collect`+aggregator as 9 flat IDs instead of `C` + `+/#/</>`).
**Do this instead:** Build a `dmnSerializer.ts` that maps editor state ↔ the backend DMN model. Align `HitPolicySelector` to DMN canonical codes (U/A/P/F/C/R/O + aggregator suffix).

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| DMN 1.5 XSD (`DMN15.xsd`) | Code-gen or hand-write serde structs mirroring the schema; parse with `quick-xml` (serde feature) | `quick-xml` maps `xs:choice`→enum, attributes→`#[serde(rename="@...")]`. Alternative: `roxmltree` (used by `dsntk-model`). |
| DMN TCK test cases (`dmn-tck` repo) | Drop-in conformance fixtures (`/tests` camel/feel) | Use as the acceptance suite for the FEEL evaluator + decision-table engine — they are the canonical DMN conformance tests. |
| Existing `/metrics` Prometheus endpoint | Register `DecisionCounters` on the SAME dedicated `Registry` | Do NOT create a second registry (Anti-Pattern 3). |
| Existing WS `MetricsSnapshot` broadcast | `MetricsTx` clone + `send(MetricsSnapshot{...})` after each evaluate | Reuse the existing broadcast channel; extend `MetricsSnapshot` with decision fields or send a variant. |
| Existing Git-backed rules dir | Store `.dmn` files alongside `.rules` files; reuse file watcher + reload job | A `.dmn` file change triggers `SharedDmnModel` swap (parallel to `SharedRuleSet`). |
| Existing OpenAPI (`api/src/openapi.rs`) | Add `DecisionEvaluateRequest/Response` + decision schemas to `utoipa` `OpenApi` | Keeps Swagger UI complete (v4.0 requirement). |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `api → engine` (one-way) | `api/src/v1/decisions.rs` calls `engine::dmn::evaluator::DecisionEvaluator` | Same direction as `execution.rs → engine::engine`. DMN types live in the engine crate so the API maps them to wire types (mirrors `TraceData → InferenceTrace`). |
| `dmn → rules` (reuse `Value` + `Expression`) | `engine::dmn::feel` uses `engine::data_types::Value` and aligns with `engine::rules::Expression` where semantics agree | FEEL extends `Value` (additive variants). Where FEEL semantics differ from `Expression` (three-valued logic, null), use FEEL-specific helpers, don't mutate the rule-engine arms. |
| `dmn ↔ engine/engine` (NO direct coupling) | DMN does NOT call `EngineRegistry`, `ExecutionEngine`, or `direct_eval` | Critical: keeping these decoupled is what makes "separate surface" (Pattern 1) real. The only shared types are `TraceEvent`/`TraceData` (owned by `registry.rs`, used by both). |
| `dmn ↔ monitoring` (extend) | DMN evaluation records into a decision-tracking singleton (parallel to `RuleExecutionTracker`) | Reuse the `LazyLock<Mutex<…>>` singleton pattern. Track per-decision invocations + per-row matches → dead-row detection for decision tables. |
| Frontend `DecisionTable.tsx ↔ backend DMN model` | NEW `dmnSerializer.ts` maps editor shape ↔ `DecisionTable` model | The editor emits a non-DMN shape today; the serializer is the bridge (see Anti-Pattern 4). |

## Suggested Build Order (dependency-respecting)

The roadmapper should sequence phases so each builds on a verified predecessor. Dependencies force this order:

```
1. DMN 1.5 model + parser          (no deps — foundation)
   └─ engine/src/dmn/{model.rs, parser.rs}, exporter.rs
2. FEEL evaluator                   (depends on 1 + Value)
   └─ engine/src/dmn/feel/{ast,eval,builtins}.rs — extends Value
3. Decision-table engine            (depends on 2 — uses FEEL to eval rule entries)
   └─ engine/src/dmn/decision_table.rs — all 7 hit policies + aggregators
4. Decision evaluator + DRD resolution (depends on 3)
   └─ engine/src/dmn/evaluator.rs — orchestrator, trace emission
5. Decision-evaluation API          (depends on 4)
   └─ api/src/v1/decisions.rs + DecisionCounters + WS broadcast + SharedDmnModel + OpenAPI
6. Monitoring/trace/coverage wiring  (depends on 5)
   └─ DecisionExecutionTracker (parallel to RuleExecutionTracker) + dead-decision-row detection + coverage extension
7. Frontend wiring                   (depends on 5)
   └─ dmnSerializer.ts + api/decisions.ts + HitPolicySelector alignment + evaluate button + results/trace panel
8. DMN TCK conformance + round-trip  (depends on 1-4)
   └─ importer/exporter round-trip + dmn-tck test fixtures
```

**Why this order:** FEEL (2) is the heart and has no upstream except the model (1). Hit policies (3) cannot be implemented without FEEL to evaluate input entries. The API (5) cannot exist until the evaluator (4) works end-to-end. Monitoring (6) and frontend (7) are the "wire it up" layers and must follow the API. TCK conformance (8) is the acceptance gate and can run in parallel with 5-7 once 1-4 are solid.

**New vs modified components:**
- **NEW:** `engine/src/dmn/{mod,model,parser,exporter,decision_table,evaluator}.rs`, `engine/src/dmn/feel/{mod,ast,eval,builtins}.rs`, `api/src/v1/decisions.rs`, `frontend/src/features/rules/dmnSerializer.ts`, `frontend/src/api/decisions.ts`.
- **MODIFY (additive):** `engine/src/data_types.rs` (extend `Value`), `engine/src/lib.rs` (pub mod dmn), `api/src/v1/mod.rs` + `api/src/main.rs` (wire routes + `SharedDmnModel` state), `api/src/metrics.rs` (`DecisionCounters` on existing registry), `api/src/openapi.rs` (decision schemas), `frontend/src/features/rules/{DecisionTable,HitPolicySelector,types}.tsx` (DMN-aligned hit policies + serializer hook), `frontend/src/api/types.ts` (decision request/response types).
- **UNCHANGED (do not touch):** `engine/src/engine/{rete,treat,leaps,phreak,registry,direct_eval}.rs`, `api/src/v1/execution.rs` (rule execution path is v5.0-audit-passed — zero regression risk).

## Sources

- OMG DMN 1.5 Specification (hit policies, FEEL, boxed expressions) — https://www.omg.org/spec/DMN/1.5/About-DMN ; machine-readable XSD — https://www.omg.org/spec/DMN/machine-readable
- OMG DMN plain-text spec, FEEL chapter — https://github.com/omg-dmn-taskforce/plain-text-spec/blob/main/10-expression-language-FEEL.adoc
- Drools/KIE DMN FEEL handbook (built-in functions, three-valued logic) — https://kiegroup.github.io/dmn-feel-handbook/
- Camunda 8 — Choosing the DMN hit policy — https://docs.camunda.io/docs/next/components/best-practices/modeling/choosing-the-dmn-hit-policy/
- Camunda 8 — Decision table hit policy reference — https://docs.camunda.io/docs/8.8/components/modeler/dmn/decision-table-hit-policy/
- KIE Community — Demystifying the Alpha Network Compiler (ANC) for DMN (compile-vs-interpreted trade-off + benchmarks) — https://blog.kie.org/2021/04/demystifying-the-alpha-network-compiler-anc.html
- Red Hat Decision Manager — DMN support + `org.kie.dmn.compiler.execmodel` configurable compile-vs-interpreted — https://docs.redhat.com/en/documentation/red_hat_decision_manager/7.5/html-single/designing_a_decision_service_using_dmn_models/index
- `quick-xml` (Rust XML + serde) — https://github.com/tafia/quick-xml ; serde deserializer docs — https://docs.rs/quick-xml/latest/quick_xml/de/
- `dsntk-model` (existing Rust DMN parser, uses `roxmltree`) — https://crates.io/crates/dsntk-model
- rRules codebase (existing integration points, verified by reading source): `engine/src/{rules,data_types,engine/mod,engine/registry,engine/direct_eval,monitoring/*}.rs`, `api/src/{v1/execution,v1/mod,metrics,main,openapi}.rs`, `frontend/src/{features/rules/*,api/*}.tsx` — Confidence HIGH (read directly)

---
*Architecture research for: DMN Level 3 compliance integrated into rRules (Rust engine + actix-web API + React/TS frontend)*
*Researched: 2026-07-06*