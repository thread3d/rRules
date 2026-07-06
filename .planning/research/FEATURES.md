# Feature Research

**Domain:** DMN 1.5 Decision Tables + FEEL Expression Language for an existing production BRMS (Rust backend, React/TS frontend)
**Researched:** 2026-07-06
**Confidence:** HIGH (authoritative: OMG DMN 1.5 plain-text spec Clause 10 grammar rules 1–65; Camunda 8 / Drools FEEL handbook corroborate semantics; OMG conformance clause 2.1)

## DMN Level 3 Conformance Checklist (the bar this milestone must clear)

Per OMG DMN spec §2.1, a **Level 3** claim requires full compliance with Clauses 6, 7, 8, **and 10** (Level 1 = 6/7/8 only; Level 2 = + S-FEEL Clause 9; Level 3 = + full FEEL Clause 10). S-FEEL is a strict subset of FEEL, so Level 3 implies 2 and 1. No partial claims are permitted ("based on" is not "conformant").

| Clause | Requirement | rRules status before v6.0 | v6.0 must deliver |
|--------|-------------|---------------------------|-------------------|
| §6 DRG/DRD | Decision Requirements Graph + Diagram notation (decision, BKM, input data, knowledge source, decision service; information/knowledge/authority requirements) | NOT required for this milestone's API-first scope — DRD visual editing is a Level 1 *notation* concern. rRules persists rules, not DRD diagrams. | Parser must accept `<definitions>`/`<decision>`/`<inputData>`/`<businessKnowledgeModel>` elements without error; visual DRD editor is an explicit anti-feature (see below). |
| §7 Decision Logic | Relating decision logic to DRG; boxed expressions (literal, invocation); item definitions; information items | No `dmn` module exists (Phase 6 was a stub). | Item definitions + information items modeled in the parser; boxed literal + boxed invocation supported (other boxed types come from Clause 10). |
| §8 Decision Table | Table notation; input/output clauses; entries; **all 7 hit policies**; default output values; orientations | Frontend `DecisionTable.tsx` + `HitPolicySelector.tsx` already render the table UI (v4.0); `HIT_POLICIES` incl. aggregating COLLECT already enumerated. Backend has no DMN table evaluator. | Decision-table engine: evaluate all 7 hit policies + COLLECT aggregators + default outputs; accept the frontend's existing table model. |
| §10 Full FEEL | Grammar rules 1–65; **all 10 boxed expression types**; complete operator set; ternary logic; type lattice; **built-in function library**; user-defined + external (Java/PMML) functions; decision-service execution semantics; XML data mapping | Engine has `Expression` AST + `Condition::evaluate` over `ComparisonOp` for Integer/Float/String/Boolean only. No FEEL parser/evaluator. | Full FEEL evaluator: parser (grammar 1–65), evaluator (semantic domain incl. Decimal128, date/time/duration, lists, contexts, ranges, functions), ternary logic, built-in function library. |

**Load-bearing consequence:** "Full DMN Level 3 FEEL" (per PROJECT.md key decision) means the FEEL scope is the large one — grammar rules 1–65, all boxed expressions, the full built-in function library, ternary logic, and the type lattice. This is the dominant cost of the milestone.

---

## Feature Landscape

### Table Stakes (Users Expect These) — Required for a Credible Level 3 Claim

Features without which the "DMN Level 3 compliant" claim is false. Organized by the quality-gate categories.

#### Parser (DMN 1.5 XML)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| DMN 1.5 XML parser (`<definitions>` → decision table + boxed expressions) | No DMN compliance without ingesting the standard interchange format (Clause 7 + §2.2). Also needed for round-trip export. | MEDIUM | Root `<definitions>` with namespace `https://www.omg.org/spec/DMN/20230324/MODEL/`; parse `<decision>`, `<inputData>`, `<businessKnowledgeModel>`, `<decisionService>`, `<itemDefinition>`, `<inputExpression>`, `<inputEntry>`, `<outputEntry>`, `<rule>`. `hitPolicy` + `aggregation` + `preferredOrientation` attrs. Recommend `quick-xml` (Rust) for streaming parse. FEEL text inside `<text>` elements is parsed by the FEEL parser (dependency). |
| DMN XML exporter (round-trip) | Round-trip is table stakes for an interchange format; a BRMS that can import but not export is not interoperable. | LOW–MEDIUM | Serialize the internal model back to DMN 1.5 XML with the correct namespace. `exporter`/`exporterVersion` attrs on `<definitions>` aid tool-to-tool interchange. Reuse the same model types as the parser. |
| Item definitions + information items (Clause 7) | Required by Clause 7; types flow into FEEL `instance of` and output `typeRef`. | MEDIUM | `itemDefinition` with `typeRef` (FEEL type names: number, string, boolean, date, time, date and time, days and time duration, years and months duration, list, context, function, range, Any). Feeds the FEEL type lattice. |

#### FEEL Expression Language (Clause 10) — the dominant scope

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| FEEL lexer + parser (grammar rules 1–65) | Level 3 = full FEEL (Clause 10). Cannot evaluate without parsing. | HIGH | 65 grammar rules; EBNF. Names can contain spaces and symbols (`-`, `'`, `+`, `*`, `/`) → ambiguous with operators, needs longest-match + scope disambiguation. `@`-prefixed date/time literals. Java-style comments. Recommend a Pratt parser over a hand-written recursive-descent parser, or a `pest`/`lalrpop` PEG grammar. This is the highest-complexity single artifact. |
| FEEL evaluator — arithmetic operators (`+ - * / **` unary `-`) | Clause 10 operator semantics; Decimal128 (34 digits). | MEDIUM | Numbers are IEEE 754-2008 Decimal128 → use `bigdecimal`/`rust_decimal` crate (NOT `f64`). `+` is also string concat when both operands are strings, and date+duration→date etc. Precedence: `**` > unary `-` > `* /` > `+ -`; all left-associative. Existing engine `ComparisonOp` over Integer/Float is NOT reusable (wrong numeric type, missing operators). |
| FEEL evaluator — comparison operators (`= != < <= > >=` `between` `in`) | Clause 10 rule 49. Decision-table input entries are unary tests built on these. | MEDIUM | `between` is sugar (`x between a and b` = `x>=a and x<=b`). `in` supports list membership, range/interval membership, and unary-test satisfaction. Type-specific equality (numbers by value; strings by char sequence; dates by value; lists element-wise; contexts by key-set + values). `e = null` tests nullity. |
| FEEL evaluator — boolean operators with **ternary logic** (`and` `or` `not()`) | Clause 10 ternary logic (true/false/null) is normative. SQL/PMML-style. | MEDIUM | Truth tables: `and`→false if any false else null if any null else true; `or`→true if any true else null if any null else false; `not(null)`→null. Critical: the existing engine's boolean evaluation is 2-valued; FEEL requires 3-valued. This is a semantic divergence, not just a syntax add. |
| FEEL evaluator — `if...then...else` | Grammar rule 45. | LOW | Condition true→then; otherwise (false OR null)→else. |
| FEEL evaluator — `for...in...return` | Grammar rule 44. Table stakes for list construction. | MEDIUM | Returns a list of results in order. Multiple iteration contexts = cross product. Implicit `partial` variable. Iteration context can be a range `a..b`. |
| FEEL evaluator — quantified `some`/`every...in...satisfies` | Grammar rule 46. | MEDIUM | `some` over empty → false; `every` over empty → true. Multiple iteration contexts supported. |
| FEEL evaluator — path expression (`.`) | Grammar rule 43. Needed for context access + list-of-contexts projection. | LOW | `context.key`; `[{x:1,y:2},{x:2,y:3}].y` → `[2,3]` (list projection). |
| FEEL evaluator — filter expression (`list[boolean]`) | Grammar rule 50. Table stakes for list processing. | MEDIUM | `item` references current element; if element is a context, its entries are directly accessible. Implicit to-singleton-list conversion. |
| FEEL evaluator — `instance of` | Grammar rule 51. Required by Clause 10 type system. | LOW | Type check against grammar rule 52 type expressions (qualified name, `range<T>`, `list<T>`, `context<k:T>`, `function<...>->T`). |
| FEEL boxed expressions — **all 10 types** | Clause 10 normative boxed expression inventory. | HIGH | (1) decision table (recursive — a boxed expression can be a decision table), (2) boxed literal, (3) boxed invocation (named params, ≥1 required), (4) boxed context (name/value pairs + optional result, names distinct), (5) boxed list, (6) relation (homogeneous contexts as a table), (7) boxed function definition (kind FEEL/PMML/Java + params + body), (8) boxed conditional (if/then/else), (9) boxed filter, (10) boxed iterator. Recursive. |
| FEEL type lattice + implicit conversions | Clause 10 semantic domain. `Any` top, `Null` bottom; equivalence (≡) + conformance (<:) relations; to-singleton-list, from-singleton-list, conforms-to conversions. | HIGH | This is the semantic backbone. Without it, `instance of`, function binding, and filter conversions are wrong. Types: number, string, boolean, date, time, date-and-time, days-and-time-duration, years-and-months-duration, list, context, function, range, null, any. |
| FEEL built-in function library | Clause 10 normative. | HIGH | ~70+ functions across categories (see Built-in Function Inventory below). Conversion / string / list / numeric / date-time / context / boolean / range / equality. This is a large surface area; each function has specified null-propagation semantics. |
| Date/time/duration value types + arithmetic | Clause 10 semantic domain + operator tables. | HIGH | `date`, `time`, `date and time`, `days and time duration`, `years and months duration`. Lexical formats (ISO 8601: `@"2019-03-31"`, `@"23:59:00z"`, `duration("P1Y1M")`). Arithmetic: date+ym-duration→date, datetime−datetime→dt-duration, time−time→dt-duration, duration×number→duration, duration÷duration→number. Recommend `chrono` + a dedicated FEEL temporal module. |
| FEEL null + three-valued logic propagation | Clause 10 ternary semantics. | MEDIUM | Most operations involving null → null (with specific exceptions in `and`/`or` short-circuits). `e = null` tests nullity. This permeates every operator and function. |
| User-defined functions (FEEL lambdas) | Grammar rule 55 (`function(params) body`); Clause 10 function semantics. | MEDIUM | Closures with lexical scope. External Java/PMML functions are out of scope for a Rust BRMS (see Anti-Features). |
| Function invocation (named + positional) | Grammar rules 38–42. | LOW | Named: unsupplied params → null. Positional: all must be supplied. from-singleton-list conversion on args. |

#### Decision-Table Engine

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Hit policy: UNIQUE (U) | Default; Clause 8. | LOW | At most one match; >1 match = runtime error. |
| Hit policy: ANY (A) | Clause 8. | LOW | Overlap allowed; all matching rules must agree on output; disagreement = error. |
| Hit policy: PRIORITY (P) | Clause 8. | MEDIUM | Output chosen by priority from output column's allowed-values list (earlier = higher priority). Rule order irrelevant. Requires allowed-values modeling on output columns. |
| Hit policy: FIRST (F) | Clause 8. | LOW | Top-down; first match wins; rule order is semantically meaningful. |
| Hit policy: COLLECT (C) | Clause 8. | LOW | All matches as an unordered list. |
| Hit policy: RULE ORDER (R) | Clause 8. | LOW | All matches in table row order. |
| Hit policy: OUTPUT ORDER (O) | Clause 8. | MEDIUM | All matches sorted by output column's allowed-values priority (decreasing). Requires allowed-values modeling. |
| COLLECT aggregators: SUM (C+) MIN (C<) MAX (C>) COUNT (C#) | Clause 8; already enumerated in frontend `HIT_POLICIES`. | MEDIUM | Single output column only; returns scalar. COUNT ignores output values (counts matching rules). Watch open issue DMN15-31: SUM is "sum of distinct outputs" (deferred to 1.6) and `sum([])` is undefined — pick a defensible behavior (recommend 0 for empty SUM, null for empty MIN/MAX) and document it. |
| Default output values | Clause 8. | LOW | When no rule matches: evaluate the output column's default expression; else null. Applies to ALL hit policies (Collect does NOT implicitly return empty list). |
| Input entry unary-test evaluation | Clause 8 + Clause 9 S-FEEL. | MEDIUM | Input entries are unary tests (`< 5`, `[1..10]`, `"A","B"`, `not(...)`, `-` = any). Built on FEEL comparison + range + `in` semantics. The decision-table engine depends on the FEEL evaluator. |
| Output entry FEEL expression evaluation | Clause 8. | LOW | Each output entry is a FEEL expression evaluated in the scope of the matched rule's input bindings. |
| Allowed-values lists on output columns | Clause 8 (required for PRIORITY + OUTPUT ORDER). | MEDIUM | Ordered list of allowed output values; drives priority. Must be modeled in the parser + table model + surfaced in the frontend. |

#### API

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| `POST /v1/decisions/{id}/evaluate` | PROJECT.md target. The decision-evaluation entry point. | MEDIUM | Accept input context (FEEL context of named values), resolve the decision by id, evaluate its decision logic (table or boxed expression), return the output (+ trace). Reuse actix-web patterns from `/v1/execute`. |
| Decision-evaluation integration with existing `EngineRegistry` / Prometheus / inference-trace | PROJECT.md target; consistency with v5.0 execution surfaces. | MEDIUM | Emit `rrules_decision_evaluate_*` Prometheus counters (mirror v5.0 `rrules_execute_*`), broadcast `MetricsSnapshot`, emit `TraceEvent`s on the dedicated registry. Reuse `web::block` per-request pattern (fresh evaluator, no `&mut self` sharing). |
| Decision CRUD + persistence (Git + PostgreSQL) | Decisions must be stored/versioned like rules. | MEDIUM | Reuse the hybrid Git+PostgreSQL storage from rules. A `.dmn` file is a natural Git artifact. Decision metadata (id, name, version, hit policy) in PostgreSQL. |

#### Frontend Wiring

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Wire existing `DecisionTable.tsx` to backend evaluator | PROJECT.md: "does NOT rebuild the editor" — wire it. | MEDIUM | The editor already exists (v4.0). Add an API client call to `POST /v1/decisions/{id}/evaluate`, surface results in the existing results panel. The `HIT_POLICIES` constant already includes aggregating COLLECT. |
| Wire `HitPolicySelector.tsx` to backend hit-policy semantics | The selector exists; backend must honor the selected policy. | LOW | Frontend already sends the hit policy; backend evaluator must respect it. Mostly a contract-alignment task. |
| Surface decision-evaluation results + trace in the UI | Consistency with v5.0 frontend test-runner trace panel. | MEDIUM | Reuse the `TraceEvent` display from the v5.0 test-runner. Show fired rules, matched outputs, aggregator result. |
| Allowed-values editor for output columns (PRIORITY/OUTPUT ORDER) | Required to make P + O hit policies usable. | MEDIUM | New frontend control (does not exist today) for ordering allowed output values. Not a rebuild of the table editor — an addition to it. |

### Differentiators (Competitive Advantage)

Not required for the Level 3 claim, but they materially improve the product against Drools/Camunda/Trisotech.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Inference-trace emission for decision evaluation | rRules already emits `TraceEvent` for rule execution (v5.0). Extending to DMN decisions gives a debuggable decision surface that Drools/Camunda do not expose by default. | MEDIUM | Reuse the v5.0 two-site trace emission pattern. Trace = which rules matched, which were filtered by hit policy, aggregator inputs. |
| Per-decision Prometheus metrics + dashboard | v5.0 has per-rule-execution metrics; per-decision metrics let operators monitor decision health alongside rule health. | MEDIUM | Mirror `rrules_execute_*` counters with `rrules_decision_*` (engine-labeled, hit-policy-labeled). |
| DMN ↔ existing Rete engine bridge | The existing Rete/TREAT/LEAPS/PHREAK engine evaluates `.rules`; DMN decisions are a different evaluation path. A bridge that lets a DMN boxed invocation call into the rule engine (or vice versa) is a genuine differentiator. | HIGH | Optional. The spec allows external functions; treating the rRules engine as an "external function" from FEEL's perspective is spec-compliant and uniquely valuable. Defer to post-MVP. |
| DMN XML round-trip with DMNDI (diagram interchange) | Most BRMS tools import DMN XML but drop the diagram layout. Preserving DMNDI (`<DMNDiagram>`/`<DMNShape>`/`<DMNEdge>`) on round-trip is a differentiator. | MEDIUM | DMNDI is a separate schema (DMNDI15.xsd). Parsing/storing layout is LOW complexity; rendering a DRD is HIGH and is an explicit anti-feature for this milestone. |
| Hot-reload of decisions (reload-then-swap) | v5.0 ships reload-then-swap for `RuleSet`. Extending the same `SharedRuleSet{generation}` pattern to decisions gives zero-downtime decision updates. | MEDIUM | Reuse the v5.0 Phase 25 `Arc<RwLock<…>>` + `generation.wrapping_add(1)` pattern. |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but would blow the milestone scope or add risk without Level 3 value.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Visual DRD (Decision Requirements Diagram) editor | "DMN has diagrams; a BRMS should render them." | Clause 6 DRD *notation* is a Level 1 concern, not a Level 3 FEEL concern. A full DRD canvas (nodes, edges, auto-layout, DMNDI render) is a large frontend effort that delivers zero Level 3 conformance value (Level 3 = Clause 10 FEEL, not diagram rendering). | Parse + persist DMNDI on round-trip (LOW effort, preserves interchange), but do NOT render a DRD canvas this milestone. The frontend decision-table editor is the authoring surface. |
| External Java/PMML function integration | Grammar rule 55 allows `external` functions; Clause 10 mentions Java/PMML. | rRules is Rust; a JVM integration is a massive scope expansion (JNI/FFI, classloader, serialization). PMML is a separate large spec. Neither is required for Level 3 (the spec says FEEL "completely defines the meaning of expressions that do not invoke externally-defined functions"). | Support user-defined FEEL lambdas (rule 55, non-external). Leave `external` as a parser-recognized but unsupported-at-eval token that returns null with a logged warning. |
| Full DMN 1.6 features (decision services composition, new boxed expression editors) | "Ship the latest." | DMN 1.6 is beta; the namespace + semantics are unsettled (e.g., DMN15-31 Collect semantics deferred to 1.6). Targeting 1.6 destabilizes the conformance claim. | Target DMN 1.5 (stable, normative XSD `DMN15.xsd`). Document the 1.6-deferred edge cases (Collect distinct-sum, `sum([])`) as known divergences. |
| Cross-table `in` evaluation / decision-service chaining (one decision calling another) | "Real DMN models chain decisions." | Multi-decision dependency graphs require a topological evaluator + cycle detection + a DRG (Clause 6) model. This is a Phase-6-level scope on its own. | Single-decision evaluation per API call for v6.0 MVP. A caller orchestrates multiple decisions via repeated `POST /v1/decisions/{id}/evaluate`. Add decision chaining as a post-MVP differentiator. |
| Replacing the existing `ComparisonOp`/`Expression` AST with the FEEL AST | "One AST to rule them all." | The existing engine AST is validated across Rete/TREAT/LEAPS/PHREAK + 118 tests. Migrating it to FEEL risks regressions in shipped v1.0–v5.0 capabilities and double the work. | Keep the FEEL AST separate (`dmn` crate / module). The two ASTs coexist; the optional DMN↔engine bridge (differentiator) translates between them. |
| ag-Grid replacement for the decision-table editor | "ag-Grid would be cleaner." | The editor already exists and is validated (v4.0). Rebuilding it risks scope blowout and gains no conformance value. | Wire the existing editor to the backend. Only add the allowed-values control (needed for P/O hit policies). |
| FEEL→SQL transpilation | "Compile FEEL to SQL for speed." | FEEL's ternary logic, date/duration semantics, and list/context types do not map cleanly to SQL. Transpilation is a research project, not an implementation task. | Interpret FEEL directly (tree-walking evaluator). Optimize later only if profiling demands it. |

## Feature Dependencies

```
[DMN XML parser]
    ├──requires──> [FEEL lexer/parser]   (FEEL text inside <text> elements)
    └──requires──> [Item definitions + type model]

[FEEL lexer/parser]
    └──requires──> [FEEL type lattice + implicit conversions]

[FEEL evaluator (arithmetic/comparison/boolean/if/for/quantified/path/filter/instance-of)]
    ├──requires──> [FEEL type lattice + implicit conversions]
    ├──requires──> [Decimal128 numeric type]
    └──requires──> [Date/time/duration value types]

[FEEL built-in function library]
    └──requires──> [FEEL evaluator] + [Date/time/duration types] + [List/context/range types]

[FEEL boxed expressions (all 10)]
    └──requires──> [FEEL evaluator] + [FEEL parser] (recursion)

[Decision-table engine]
    ├──requires──> [FEEL evaluator]  (input entries = unary tests; output entries = FEEL exprs)
    ├──requires──> [Hit policy evaluation: U/A/P/F/C/R/O]
    ├──requires──> [COLLECT aggregators: SUM/MIN/MAX/COUNT]
    └──requires──> [Allowed-values lists on output columns]  (P + O)

[Decision-evaluation API: POST /v1/decisions/{id}/evaluate]
    ├──requires──> [Decision-table engine] + [FEEL boxed expressions]
    ├──requires──> [Decision CRUD + persistence]
    └──enhances──> [Existing EngineRegistry / Prometheus / inference-trace]  (v5.0 surfaces)

[Frontend wiring: DecisionTable.tsx + HitPolicySelector.tsx → backend]
    └──requires──> [Decision-evaluation API]
    └──requires──> [Allowed-values editor for output columns]  (to make P/O usable)

[DMN XML exporter (round-trip)]
    └──requires──> [DMN XML parser]  (shared model types)

[DMN↔Rete engine bridge] ──enhances──> [FEEL evaluator]  (optional differentiator, defer)
[Hot-reload of decisions] ──enhances──> [Decision CRUD + persistence]  (reuse v5.0 pattern)
[Inference-trace for decisions] ──enhances──> [Decision-evaluation API]  (reuse v5.0 pattern)
```

### Dependency Notes

- **DMN XML parser requires FEEL parser:** `<inputExpression>`, `<inputEntry>`, `<outputEntry>` all contain FEEL text. A parser that only ingests XML structure but cannot parse FEEL is a Level 1/2 tool, not Level 3.
- **Decision-table engine requires FEEL evaluator:** input entries are S-FEEL unary tests (`< 5`, `[1..10]`, `"A","B"`); output entries are FEEL expressions. The existing `Condition::evaluate` over `ComparisonOp` is insufficient (2-valued logic, no ranges, no `in`, no null semantics).
- **PRIORITY + OUTPUT ORDER require allowed-values lists:** these hit policies are undefined without an ordered allowed-values list on the output column. The parser, table model, and frontend must all model it.
- **FEEL evaluator requires Decimal128 + date/time/duration types:** the semantic domain is normative. `f64` is non-conformant (34-digit Decimal128 precision). The existing engine's Integer/Float is not reusable for FEEL.
- **Boxed expressions require the full FEEL evaluator:** boxed expressions are recursive (a boxed context value can be a boxed list, which can contain a decision table). The evaluator must handle the full grammar.
- **The DMN↔Rete bridge is optional:** the two evaluation paths (Rete `.rules` vs DMN decisions) can coexist without a bridge for the MVP. The bridge is a differentiator, not table stakes.
- **Frontend wiring requires the API but NOT a DRD canvas:** the existing `DecisionTable.tsx` is the authoring surface; a DRD canvas is an anti-feature for this milestone.

## MVP Definition

### Launch With (v6.0 — the Level 3 claim)

Minimum to credibly claim DMN Level 3 compliance.

- [ ] FEEL lexer/parser (grammar 1–65) — without this, no FEEL, no Level 3.
- [ ] FEEL evaluator: arithmetic, comparison, boolean (ternary), `if`, `for`, `some`/`every`, path, filter, `instance of` — Clause 10 operator surface.
- [ ] FEEL type lattice + implicit conversions + null/ternary propagation — semantic backbone.
- [ ] Decimal128 numeric type + date/time/duration value types — normative semantic domain.
- [ ] FEEL built-in function library (all categories) — Clause 10 normative.
- [ ] All 10 FEEL boxed expressions — Clause 10 normative inventory.
- [ ] User-defined FEEL functions (lambdas, non-external) — grammar rule 55.
- [ ] DMN 1.5 XML parser + exporter (round-trip) — interchange format.
- [ ] Item definitions + information items — Clause 7.
- [ ] Decision-table engine: all 7 hit policies + COLLECT aggregators + default outputs + allowed-values — Clause 8.
- [ ] Decision CRUD + persistence (Git + PostgreSQL) — decisions are first-class artifacts.
- [ ] `POST /v1/decisions/{id}/evaluate` — the evaluation entry point.
- [ ] Decision-eval integration with Prometheus + inference-trace — consistency with v5.0.
- [ ] Frontend wiring: `DecisionTable.tsx` + `HitPolicySelector.tsx` → backend evaluator — PROJECT.md explicit scope.
- [ ] Allowed-values editor for output columns — required for P/O hit policies to be usable.
- [ ] Conformance test suite: DMN 1.5 spec examples + TCK (Technology Compatibility Kit) cases, if available.

### Add After Validation (v6.x)

- [ ] Inference-trace emission for decisions — high-value differentiator, low marginal cost after API exists.
- [ ] Per-decision Prometheus metrics + dashboard panel.
- [ ] Hot-reload of decisions (reload-then-swap, reusing v5.0 `SharedRuleSet{generation}` pattern).
- [ ] DMNDI round-trip preservation (parse + persist layout; no rendering).
- [ ] Decision chaining / decision-service composition (multi-decision dependency graph).

### Future Consideration (v7+)

- [ ] DMN↔Rete engine bridge (FEEL external function calling the rRules engine) — genuine differentiator, HIGH complexity.
- [ ] Visual DRD canvas — only if customer demand; large frontend effort.
- [ ] External PMML function support — separate spec, separate scope.
- [ ] DMN 1.6 features — wait for the spec to stabilize (Collect distinct-sum, `sum([])`).

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| FEEL lexer/parser (grammar 1–65) | HIGH (enabler) | HIGH | P1 |
| FEEL evaluator — arithmetic/comparison/boolean (ternary) | HIGH (enabler) | HIGH | P1 |
| FEEL type lattice + null/ternary propagation | HIGH (semantic backbone) | HIGH | P1 |
| Decimal128 + date/time/duration types | HIGH (normative) | HIGH | P1 |
| FEEL built-in function library | HIGH (Clause 10) | HIGH | P1 |
| FEEL boxed expressions (all 10) | HIGH (Clause 10) | HIGH | P1 |
| FEEL `if`/`for`/`some`/`every`/path/filter/`instance of` | HIGH (Clause 10) | MEDIUM | P1 |
| User-defined FEEL functions | MEDIUM | MEDIUM | P1 |
| DMN 1.5 XML parser + exporter | HIGH (interchange) | MEDIUM | P1 |
| Item definitions + info items | HIGH (Clause 7) | MEDIUM | P1 |
| Decision-table: U/A/F/C/R hit policies | HIGH (Clause 8) | LOW | P1 |
| Decision-table: P/O hit policies + allowed-values | HIGH (Clause 8) | MEDIUM | P1 |
| COLLECT aggregators (SUM/MIN/MAX/COUNT) | HIGH (Clause 8) | MEDIUM | P1 |
| Default output values | MEDIUM (Clause 8) | LOW | P1 |
| `POST /v1/decisions/{id}/evaluate` | HIGH (entry point) | MEDIUM | P1 |
| Decision CRUD + persistence | HIGH (storage) | MEDIUM | P1 |
| Frontend `DecisionTable.tsx` wiring | HIGH (PROJECT.md scope) | MEDIUM | P1 |
| Allowed-values frontend editor | MEDIUM (P/O usability) | MEDIUM | P1 |
| Inference-trace for decisions | MEDIUM (differentiator) | MEDIUM | P2 |
| Per-decision Prometheus metrics | MEDIUM (ops) | MEDIUM | P2 |
| Hot-reload of decisions | MEDIUM (ops) | MEDIUM | P2 |
| DMNDI round-trip preservation | LOW (interchange fidelity) | MEDIUM | P2 |
| Decision chaining / decision services | MEDIUM (real models) | HIGH | P3 |
| DMN↔Rete engine bridge | LOW–MEDIUM (differentiator) | HIGH | P3 |
| Visual DRD canvas | LOW (no Level 3 value) | HIGH | P3 (anti-feature for v6.0) |
| External Java/PMML functions | LOW (non-Rust) | HIGH | P3 (anti-feature) |

**Priority key:**
- P1: Must have for the DMN Level 3 claim (v6.0 launch)
- P2: Should have, add when possible (v6.x)
- P3: Nice to have / future consideration / anti-feature for this milestone

## Competitor Feature Analysis

| Feature | Drools (Red Hat) | Camunda 8 | Trisotech | Our approach (rRules v6.0) |
|---------|------------------|-----------|-----------|---------------------------|
| FEEL evaluator | Full FEEL (Java) — mature, DMN TCK-passing | Full FEEL (Java) — mature | Full FEEL — mature, cloud-first | New Rust FEEL evaluator (Decimal128 via `rust_decimal`/`bigdecimal`, `chrono` for temporal). Smaller ecosystem but native Rust integration with the existing engine. |
| Hit policies | All 7 + aggregators | All 7 + aggregators | All 7 + aggregators | All 7 + aggregators (table stakes — must match). |
| DMN XML interchange | Full import/export (1.1–1.5) | Full (1.1–1.5) | Full (1.5, cloud) | DMN 1.5 import/export (round-trip). Single-version focus is acceptable for v6.0. |
| DRD canvas | Full visual editor | Full visual editor (web) | Full visual editor (cloud) | **No DRD canvas** (anti-feature for v6.0). Decision-table editor is the authoring surface. Differentiate on engine + governance, not diagrams. |
| Inference trace | Limited (debug logging) | Limited | Limited | **Differentiator**: structured `TraceEvent` emission + frontend trace panel (reuse v5.0). |
| Per-decision metrics | Via Prometheus (generic) | Via Prometheus (generic) | Cloud metrics | **Differentiator**: dedicated `rrules_decision_*` counters, hit-policy-labeled. |
| External functions | Java + PMML | Java | Java + PMML | FEEL lambdas only (non-external). Java/PMML explicitly out of scope (Rust). |
| Governance (RBAC/approvals/audit/git) | Via Business Central | Via Camunda Console | Via cloud platform | **Already shipped** (v4.0) — a real differentiator. DMN decisions inherit rRules governance. |
| Pluggable engine (Rete/TREAT/LEAPS/PHREAK) | No (single algorithm) | No | No | **Already shipped** (v3.0) — unique. DMN decisions use a separate evaluator but can optionally bridge. |

## Built-in Function Inventory (Clause 10 reference)

For the FEEL built-in function library feature (P1, HIGH complexity). ~70+ functions; grouped by category. Each has specified null-propagation semantics.

- **Conversion** (13): `string`, `number` (1–2 arg), `date` (3 overloads), `time` (4 overloads), `date and time` (3 overloads), `duration`, `years and months duration`.
- **String** (14): `substring`, `string length`, `upper case`, `lower case`, `substring before`, `substring after`, `replace` (regex), `contains`, `starts with`, `ends with`, `matches` (regex), `split` (regex), `string join` (1–2 arg).
- **List** (20+): `list contains`, `list replace` (2 overloads), `count`, `min`, `max`, `sum`, `mean`, `all`, `any`, `sublist`, `append`, `concatenate`, `insert before`, `remove`, `reverse`, `index of`, `union`, `distinct values`, `flatten`, `product`, `median`, `stddev`, `mode`, `sort`.
- **Numeric** (13): `decimal`, `floor` (1–2 arg), `ceiling` (1–2 arg), `round up`, `round down`, `round half up`, `round half down`, `abs`, `modulo`, `sqrt`, `log`, `exp`, `odd`, `even`.
- **Date/Time** (6): `now`, `today`, `day of year`, `day of week`, `month of year`, `week of year`.
- **Context** (5, DMN 1.4+): `context`, `context put` (2 overloads), `context merge`, `get value`, `get entries`.
- **Boolean** (1): `not`.
- **Range ordering** (13): `before`, `after`, `meets`, `met by`, `overlaps`, `overlaps before`, `overlaps after`, `finishes`, `finished by`, `includes`, `during`, `starts`, `started by`, `coincides`.
- **Equality** (1): `is`.

## Sources

- OMG DMN 1.5 plain-text spec, Clause 10 (FEEL grammar rules 1–65, boxed expressions, semantic domain, operator semantics) — https://github.com/omg-dmn-taskforce/plain-text-spec/blob/main/10-expression-language-FEEL.adoc — **HIGH** (authoritative, OMG-published)
- OMG DMN 1.3/1.5 specification §2.1 (conformance levels) — https://www.omg.org/spec/DMN/1.3/PDF — **HIGH** (normative conformance clause)
- OMG DMN XSD schemas (DMN15.xsd, namespace `https://www.omg.org/spec/DMN/20230324/MODEL/`) — https://github.com/omg-dmn-taskforce/omg-dmn-spec — **HIGH** (normative interchange schema)
- Camunda 8 Docs — hit policies — https://docs.camunda.io/docs/next/components/best-practices/modeling/choosing-the-dmn-hit-policy/ — **MEDIUM** (vendor docs, corroborate spec)
- Drools DMN FEEL Handbook — https://kiegroup.github.io/dmn-feel-handbook/ — **MEDIUM** (vendor, corroborates function library + semantics)
- QuantumBPM — boxed expressions + hit policies — https://quantumbpm.com/docs/dmn/expressions/overview — **MEDIUM** (community reference)
- OMG Issue Tracker DMN15-31 — Collect semantics edge cases — https://issues.omg.org/issues/DMN15-31 — **HIGH** (normative; documents the distinct-sum + `sum([])` ambiguities deferred to 1.6)

---
*Feature research for: DMN 1.5 Decision Tables + FEEL Expression Language (DMN Level 3 compliance)*
*Researched: 2026-07-06*