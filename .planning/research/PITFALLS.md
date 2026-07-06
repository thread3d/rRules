# Pitfalls Research

**Domain:** Adding DMN Level 3 / FEEL to an existing production Rust BRMS (rRules v5.0)
**Researched:** 2026-07-06
**Confidence:** HIGH (findings cross-verified against the OMG DMN 1.3/1.5 spec, Camunda feel-scala issue tracker, Drools FEEL handbook, bpmn.io/kie-tools issue trackers, and live inspection of the rRules `engine`/`frontend` source)

## Critical Pitfalls

### Pitfall 1: Treating Rust `PartialEq` on `Value` as FEEL equality (three-valued-logic leak)

**What goes wrong:**
The existing `engine::data_types::Value` derives `PartialEq`, so `Value::Null == Value::Null` is `true` and every comparison returns a Rust `bool`. FEEL equality is **three-valued**: `null = null` → `true`, but `"a" = 1` → `null` (not `false`), `null < 3` → `null`, and `null != 5` → `true`. A DMN evaluator that delegates to `==`/`compare_values` will silently coerce `null` results to `false`, dropping the "execution error" semantics that DMN models depend on (filter expressions retain items only when the predicate is **strictly** `true`; a `null` coerced to `false` is indistinguishable from a real `false`).

**Why it happens:**
Rust has no native three-valued equality, and the existing `compare_values` helper plus `Value: PartialEq` is the obvious reuse path. The single existing `Value::Null` variant *looks* like it already supports null, lulling implementers into thinking the equality story is done.

**How to avoid:**
Do not reuse Rust `==` or the existing `compare_values` for FEEL. Implement a dedicated `feel_equals(a, b) -> Option<bool>` (returns `None` for `null`) and a `feel_compare(a, b) -> Option<Ordering>` in the new `dmn` crate. Map `None` to `Value::Null` at the boundary. Encode the FEEL truth tables for `and`/`or`/`not` explicitly (short-circuit `true or X → true`, `false and X → false`, everything else with a `null` operand → `null`). Add a conformance test suite that asserts `"a" = 1` → `null`, `null < 3` → `null`, `null = null` → `true`, `not(null)` → `null`.

**Warning signs:**
- A `feel_eq` function returning `bool` instead of `Option<bool>`.
- Filter expressions dropping items that should pass (or vice versa) on tables with nullable inputs.
- `if null then ... else ...` taking the `then` branch instead of `else` (FEEL: `null` condition → `else`).
- Test cases that assert `null = null` is `false` or `null`.

**Phase to address:** Phase 30 (FEEL evaluator) — equality and ternary logic are foundational; every later FEEL feature depends on it.

---

### Pitfall 2: Reusing the existing `Expression` AST for FEEL (or duplicating it naively)

**What goes wrong:**
The existing `engine::rules::Expression` enum has 8 variants (`Literal`, `FieldRef`, `Add`/`Sub`/`Mul`/`Div`, `StringConcat`, `Compare`, `Call`). Full FEEL requires ~25+ expression forms: `if`, `for`, `some`/`every` (quantified), `filter` (`list[expr]`), `path` (`a.b.c`), `context` (`{k: v}`), `list`, `relation`, `function definition`, `instance of`, `unary tests`, boolean `and`/`or`/`not`, `between`, `in`. The existing `Call` variant is a stub returning `Null`. Two failure modes:
1. **Forced reuse:** shoehorn FEEL into the existing enum, producing a god-enum with weird optional fields and broken `evaluate(&[&Fact])` signature (FEEL needs a context/scope, not a fact slice).
2. **Naive duplication:** create a parallel `FeelExpression` enum that copy-pastes `Literal`/`Add`/`Compare` semantics, diverging from the engine's arithmetic/coercion behavior and creating two sources of truth for "what does `1 + 2.0` mean."

**Why it happens:**
The existing AST *almost* fits, and reuse feels like the DRY choice. But the existing `Expression::evaluate(&self, facts: &[&Fact])` signature is wrong for FEEL — FEEL evaluates against a **scope** (a stack of contexts: local → global → built-in), not a flat fact slice. Reuse forces a leaky abstraction.

**How to avoid:**
Create a **new `dmn::feel::ast::FeelExpression` enum** with FEEL-native variants and an `evaluate(&self, scope: &Scope) -> FeelValue` signature. Share the **leaf value type** by promoting the existing `Value` into a shared `engine::types` module and extending it (see Pitfall 3), but do **not** share the expression AST. Share *behavior* via free functions (`numeric_op`, `compare_values`) extracted to a shared `types` module, not via enum unification. Document the boundary: `engine::Expression` = Rete rule LHS/RHS; `dmn::FeelExpression` = FEEL boxed expressions.

**Warning signs:**
- `FeelExpression::evaluate` taking `&[&Fact]` instead of a scope object.
- An `Expression` enum with 30+ variants and `Option` fields used only by FEEL.
- Two `evaluate` impls with copy-pasted arithmetic that drift on edge cases (e.g., `Integer / Integer` division semantics).

**Phase to address:** Phase 30 (FEEL evaluator) — the AST shape is the first structural decision and is expensive to reverse.

---

### Pitfall 3: `Value` enum gaps — no `Date`, `Time`, `Duration`, or `Decimal` (FEEL uses Decimal128, not f64)

**What goes wrong:**
FEEL has distinct types: `date`, `time`, `date and time`, `years and months duration` (YearMonthDuration), `days and time duration` (DayTimeDuration), and `number` (IEEE 754-2008 **Decimal128**, 34 digits — no `-INF`/`+INF`/`NaN`). The existing `Value` has only `DateTime(NaiveDateTime)` and `Float(f64)`. Three concrete bugs:
1. **Precision loss:** `1.1 + 2.2` in `f64` → `3.3000000000000003`; in FEEL Decimal128 → `3.3`. Financial models (the BRMS target market) will produce visibly wrong results.
2. **Type errors:** `date("2026-07-06")` can't be represented — it'll be parsed as `DateTime` or `String`, breaking `date` vs `date and time` equality (`date("2026-07-06") = date and time("2026-07-06T00:00:00")` → `false` in FEEL).
3. **Duration arithmetic:** `date and time("2026-07-06T12:00:00") + duration("PT1H")` has no type to hold the duration.

**Why it happens:**
The existing `Value` was designed for the Rete rule engine, which only needed JSON-shaped facts. FEEL's type system is specified normatively in DMN §10.3.1.3 and is larger.

**How to avoid:**
Extend the shared `Value` enum (or a new `FeelValue`) with: `Date(NaiveDate)`, `Time(NaiveTime)`, `DateTime` (already present, keep), `YearMonthDuration` (months: i32), `DayTimeDuration` (nanoseconds: i128 or `chrono::Duration`), and replace `Float(f64)` for FEEL with `Decimal(rust_decimal::Decimal)` (or keep `Float` for the Rete path and add `Decimal` for FEEL — but then arithmetic across the two is undefined; prefer one numeric type for FEEL). Use `rust_decimal` crate (Decimal128-compatible). Add FEEL date/time/duration built-ins (`date`, `time`, `date and time`, `duration`) that parse into these.

**Warning signs:**
- FEEL `number` tests failing on `0.1 + 0.2`.
- `sum([1, 2, 3])` returning `Float` with trailing zeros.
- No way to represent `duration("P1Y")` or `duration("PT8H")`.
- `date and time` arithmetic returning `Null` because the duration type is missing.

**Phase to address:** Phase 30 (FEEL evaluator) — types come before operators; Phase 29 (DMN model) should define the type metamodel but the runtime `Value` extension lands with the evaluator.

---

### Pitfall 4: Implementing PRIORITY as "first matching rule" (it's enumerated-output ordering, not rule order)

**What goes wrong:**
PRIORITY (`P`) hit policy selects the matching rule whose **output value has the highest priority**, where priority is defined by the **order of enumerated output values in the output clause's `allowedValues`/`Allowed Values` list** (decreasing priority). A common bug: implement PRIORITY as "return the first matching rule in table row order" — that's the **FIRST** (`F`) hit policy, not PRIORITY. Another bug: implement it without requiring/checking the output enumeration, so a P table with no `allowedValues` silently misbehaves instead of erroring. A third: the "else rule" (hyphens in all inputs) whose output isn't the lowest-priority enumerated value — a "misleading rule" that produces surprising overrides.

**Why it happens:**
The names "Priority" and "First" sound similar, and the existing engine has a `salience`/`conflict resolution` concept that *also* sounds like priority. Implementers map the familiar Rete salience onto DMN PRIORITY and miss the normative definition (DMN §8.2.12 / §10.3.2.10).

**How to avoid:**
Implement PRIORITY strictly: (1) validate at load time that the output clause has an enumerated `allowedValues` list (reject P tables without one); (2) build a priority map `{output_value → rank}` from the enumeration order; (3) among all matching rules, select the one whose output has the **lowest rank** (highest priority); (4) emit a warning for "misleading rules" (non-else rules with hyphen inputs whose output isn't lowest-priority). Implement OUTPUT ORDER (`O`) as the multi-result counterpart (all matching outputs sorted by the same priority). Add tests: a P table where rule #1 outputs "Medium" and rule #5 outputs "High" with enumeration `[High, Medium, Low]` must return "High" even though rule #1 matched first.

**Warning signs:**
- PRIORITY tests that only use single-rule-match cases (hides the ordering bug).
- No `allowedValues` validation on P tables.
- PRIORITY and FIRST sharing an implementation.
- `HitPolicySelector.tsx` showing "Priority" with description "Rules are ordered by priority" (current text) — this *describes* the bug, not the spec.

**Phase to address:** Phase 31 (decision-table engine) — hit-policy semantics are the core of that phase.

---

### Pitfall 5: ANY conflict detection silently picks one output instead of erroring

**What goes wrong:**
ANY (`A`) hit policy allows multiple rules to match **only if all matching rules produce the same output value**. If two matching rules produce *different* outputs, the table is **violated** (a conflict). The correct behavior is to surface a clear error (`HitPolicyViolation`) to the caller. The common bug: silently return the first matching rule's output, or silently return `null`, masking a model defect that the modeler must fix.

**Why it happens:**
Erroring on conflict feels "heavy" and the easy path is to pick the first match. The `any` description in the frontend (`"Multiple rules can match, order doesn't matter"`) actively misleads — it omits the "must agree" constraint.

**How to avoid:**
At evaluation time, collect all matching rules' outputs; if they are not all FEEL-equal (use `feel_equals`, Pitfall 1 — note `null`-output equality is `true`), return a `HitPolicyViolation { decision_id, conflicting_rule_ids, outputs }` error. Expose this as a distinct API error code (`DMN_HIT_POLICY_VIOLATION`) so the frontend can show "rules R3 and R7 both matched but disagreed: 'Approved' vs 'Declined'". Add a test with a deliberately conflicting A table.

**Warning signs:**
- No `HitPolicyViolation` error type.
- ANY evaluation returning the first match without comparing.
- Tests that only use agreeing multi-match A tables.
- Frontend showing ANY conflicts as a normal result.

**Phase to address:** Phase 31 (decision-table engine) for detection; Phase 32 (API) for the error code; Phase 33 (frontend) for surfacing.

---

### Pitfall 6: COLLECT aggregator bugs — type validation, COUNT semantics, multi-output rejection

**What goes wrong:**
COLLECT with an aggregator (`C+`/`C<`/`C>`/`C#`) has three normative constraints that get missed:
1. **Multi-output rejection:** a COLLECT-with-aggregator table **may have only one output clause**. Silently aggregating across multiple outputs produces garbage.
2. **SUM on non-numerics:** `C+` requires numeric outputs; `sum(["a", "b"])` → `null`. Silently coercing strings to 0 or concatenating is wrong.
3. **COUNT counts matching rules** (or distinct outputs — the spec is "number of matching rules"; some implementations count distinct outputs). Picking the wrong semantics breaks models that rely on duplicate outputs counting once vs. N times.
4. **MIN/MAX on incomparables:** `C<`/`C>` require comparable values; mixing types → `null` (FEEL ternary), not a panic.
5. **Empty COLLECT:** `C+` over zero matches → `0`? `null`? (Spec: SUM of empty → `null` in classic FEEL, `0` in B-FEEL. Pick classic-FEEL for Level 3 and document it.)

**Why it happens:**
Aggregators look like SQL `SUM`/`MIN`/`MAX`/`COUNT`, so implementers apply SQL semantics. FEEL's null-propagation and the single-output constraint are easy to miss.

**How to avoid:**
Validate at load: aggregator tables have exactly one output. At eval: SUM requires all outputs numeric (else `null`); MIN/MAX compare with `feel_compare` (returns `None` on incomparable → `null`); COUNT returns the count of matching rules (document this choice). For empty matches, return `null` for SUM/MIN/MAX (classic FEEL) and `0` for COUNT. Add edge-case tests: empty match set, mixed-type outputs, single output.

**Warning signs:**
- `sum` returning `0` on empty matches (B-FEEL semantics, not classic FEEL Level 3).
- A `C+` table with two output clauses accepted at load.
- `COUNT` tests that assert distinct-output counting without documenting the choice.

**Phase to address:** Phase 31 (decision-table engine).

---

### Pitfall 7: Frontend `HitPolicySelector` vocabulary mismatch with DMN spec

**What goes wrong:**
The existing `frontend/src/features/rules/types.ts` defines `HIT_POLICIES` as 9 flat entries: `unique, any, priority, collect, rule-order, sum, min, max, count`. The DMN spec has **7 hit policies** (`U/A/P/F/C/O/R`) with **4 COLLECT aggregators** as modifiers (`C+/C</C>/C#`). Three concrete mismatches:
1. **Missing `first` (F) and `output-order` (O)** — the dropdown can't represent two valid hit policies.
2. **`sum/min/max/count` modeled as top-level hit policies** instead of `collect` + aggregator. DMN XML serializes them as `hitPolicy="COLLECT" aggregation="SUM"`, so the frontend model can't round-trip (see Pitfall 11).
3. **Descriptions are wrong/misleading:** "Priority — Rules are ordered by priority" (Pitfall 4), "Any — Multiple rules can match, order doesn't matter" (Pitfall 5, omits the must-agree constraint), "Collect — All matching rules are fired" (conflates with rule firing).

**Why it happens:**
The editor was built in v4.0 against a stubbed backend with no spec to validate against. The flat list was the simplest UI model.

**How to avoid:**
Refactor `HitPolicyId` to `{ policy: 'U'|'A'|'P'|'F'|'C'|'O'|'R', aggregator?: 'SUM'|'MIN'|'MAX'|'COUNT' }` (aggregator only valid with `C`). Update `HitPolicySelector.tsx` to render two-step (policy, then aggregator when C is chosen). Fix descriptions to match spec wording. Add `first` and `output-order`. Map to/from DMN XML `hitPolicy` + `aggregation` attributes at the API boundary, not in the frontend.

**Warning signs:**
- DMN XML import producing `hitPolicy="COLLECT"` with no aggregator surfacing as `collect` in the UI but losing the aggregator on export.
- No way to select FIRST or OUTPUT ORDER.
- A `HitPolicyId` that is a flat string union including `sum`/`min`/`max`/`count`.

**Phase to address:** Phase 33 (frontend wiring) — the model refactor is the first task; Phase 31 (decision-table engine) should define the canonical backend enum so the frontend maps *to* it, not the reverse.

---

### Pitfall 8: FEEL expression re-parsing on every evaluation (no AST cache)

**What goes wrong:**
Each decision-table cell holds a FEEL expression as a **string** (input entries, output entries, boxed-expression text). A 10-rule, 3-input-column table = 30+ FEEL expressions re-parsed from scratch on **every** `evaluate` call. This is the #1 documented DMN performance bug: Camunda feel-scala issue #1149 shows full fastparse grammar parsing + regex `Pattern.compile` per evaluation as the dominant CPU hotspot; Camunda's 2016 blog reports a **4–6x throughput improvement** from caching parsed expressions; Camunda 8 added an LRU cache of parsed DRGs (issue #8571) for the same reason. The existing rRules engine has no FEEL parse cache (there is no FEEL parser yet), so the easy mistake is to parse-on-eval without a cache layer.

**Why it happens:**
The simplest evaluator implementation is `parse(expr_str).unwrap().eval(scope)`. Caching requires a key (the string + any declared-types context), a concurrent map, and a decision about cache scope (per-decision? per-process? LRU-bounded?). It's easy to defer and then never come back.

**How to avoid:**
Cache parsed `FeelExpression` ASTs in a `DashMap<String, Arc<FeelExpression>>` (or `ConcurrentHashMap` equivalent) keyed by the raw expression string, built at **DMN load time** (parse every cell once when the model is loaded, store the AST on the clause/rule). The evaluator then takes the pre-parsed AST, never a string. Add an LRU bound if memory is a concern (Camunda 8.2 shipped a bounded LRU for DRGs). Benchmark: a 30-cell table evaluated 10k times should show parse work ≈ 30 ops, not 300k.

**Warning signs:**
- `evaluate(cell_text: &str, ...)` signature (string in, parse inside).
- CPU profiles showing the FEEL parser on the hot path.
- `Pattern::compile` / regex objects allocated per evaluation.
- A "parse once at load" step that doesn't exist.

**Phase to address:** Phase 30 (FEEL evaluator) for the cache primitive; Phase 31 (decision-table engine) for parsing cells at table-load time; revisit in Phase 32 if the API layer re-parses.

---

### Pitfall 9: Boxed-context evaluation in textual order instead of dependency (partial-order) order

**What goes wrong:**
DMN boxed contexts (§10.3.2.6) evaluate entries in **dependency order**, not textual order. Entry `B` may reference entry `A`'s key, and `A` may be declared *after* `B` in the XML/JSON. The references "SHALL be acyclic and form a partial order" and "SHALL be evaluated consistent with this partial order." A naive top-to-bottom evaluator produces wrong results whenever an entry references a later-declared entry, and produces a spurious "undefined variable" error instead of detecting the cycle.

**Why it happens:**
Top-to-bottom is the obvious implementation and works for the common case where modelers declare entries in dependency order. The spec constraint is invisible until you hit a model that doesn't.

**How to avoid:**
Build a dependency graph from each entry's free variables (FEEL name references that resolve to other entry keys in scope). Topologically sort; detect cycles at load time and reject with `CyclicContext { entries, cycle }`. Evaluate in topo order. Handle the **Result box** distinction: contexts *with* a final `result` entry keep intermediate entries local (not exposed outside); contexts *without* expose all entries as qualified names. Implement scope as a stack of contexts (local → global → built-in, §10.3.2.11) with **longest-match** QN resolution (§10.3.1.6).

**Warning signs:**
- Context tests that only use textually-ordered entries.
- No cycle-detection error type for contexts.
- A `Context` evaluator that returns entries in insertion order regardless of references.
- QN resolution that returns the first short match instead of the longest.

**Phase to address:** Phase 30 (FEEL evaluator) — contexts are a FEEL boxed expression, not a decision-table feature.

---

### Pitfall 10: DMN XML parser pitfalls — namespaces, DMNDI, XXE, CDATA, `hitPolicy` default

**What goes wrong:**
Ten concrete parser bugs observed across Camunda/Flowable/Operaton:
1. **Namespace versioning:** DMN changes its XML namespace every revision (1.1→1.2→1.3→1.4→1.5, dated URIs like `https://www.omg.org/spec/DMN/20230318/MODEL/`). A parser that only accepts one namespace rejects otherwise-valid files. DMN 1.7 will fix this with fixed URIs + `dmnVersion` attribute, but 1.5 is the target.
2. **Mixed DMN/DC/DI namespaces:** `DMNDI13.xsd` imports DC and DI namespaces; mismatched dated URIs across the three cause schema-validation failures.
3. **DMNDI `dmnElementRef` is `xsd:QName`:** must resolve to an existing element ID; multiple DI elements referencing the same `dmnElementRef` is invalid; pointing at an `inputData` ID instead of a `decision` ID breaks rendering.
4. **`hitPolicy` defaults to `UNIQUE`** when omitted (some engines default to `FIRST` — wrong per spec §8.2.10).
5. **`aggregation` only valid with `COLLECT`** — silently ignored or errored by different engines.
6. **Annotation/description entries are separate from input/output entries** and are truncated by some engines (Flowable: 255 chars). Dropping them loses modeler documentation.
7. **FEEL in `<text>` is CDATA-wrapped** — a parser that doesn't handle CDATA mangles expressions (`<`, `>`, `&` in FEEL).
8. **`href="#id"` references** must resolve to an existing `id` in the same definitions; using `name` instead of `id` breaks references. Duplicate IDs anywhere break the whole document.
9. **XXE:** DMNDI/DC/DI imports make the parser XXE-vulnerable unless DTD/external entities are disabled.
10. **Decision-table orientation:** rows = rules, columns = clauses; `input`/`output` clause ordering matters for FIRST/RULE ORDER semantics. Parsing columns out of order silently reorders rule evaluation.

**Why it happens:**
The DMN XSD is large and the spec prose is the authority, not the XSD. Quick-and-dirty XML readers miss the QName resolution, CDATA, and namespace-handling requirements.

**How to avoid:**
Use `quick-xml` (Rust) with explicit namespace handling and a per-version schema selector (accept 1.1–1.5 namespaces, normalize to an internal model). Hard-disable DTD and external entities (`quick_xml::Reader` with `disable_comments`, trim, and no entity expansion). Resolve all `QName`/`href` references in a second pass and error on dangling/duplicate IDs. Preserve annotation/description entries (no truncation). Read `<text>` as CDATA. Default `hitPolicy` to `UNIQUE`. Reject `aggregation` on non-COLLECT tables. Add a conformance-test corpus of real-world `.dmn` files across versions.

**Warning signs:**
- A parser that accepts only one DMN namespace.
- No XXE hardening (`quick_xml` default may not be enough — verify entity handling).
- `hitPolicy` defaulting to `FIRST` or empty.
- Round-trip tests that drop annotation entries.
- FEEL expressions with `<`/`>` failing to parse from XML.

**Phase to address:** Phase 29 (DMN parser + model) — these are parser-correctness issues that block every downstream phase.

---

### Pitfall 11: Round-trip fidelity loss — frontend model can't represent DMN XML

**What goes wrong:**
The existing frontend `DecisionTableRow` model (`conditions: Record<string, unknown>`, `actions: Record<string, unknown>`) and `DecisionTableColumn` are **not** DMN. Round-tripping DMN XML → frontend model → DMN XML loses, in order of severity:
1. **Annotation/description entries** (no slot in the frontend model).
2. **DMNDI** (graphical layout — no slot).
3. **Vendor extensions** (spec *permits* dropping these; kie-tools issue #3594 shows text content is silently stripped).
4. **`allowedValues` enumerations** on output clauses (needed for PRIORITY — no slot).
5. **`inputExpression` / `outputEntry` FEEL text** vs. the frontend's `unknown` cell values — the frontend stores *values*, not *FEEL expressions*.
6. **Element ordering** (XSD `xsd:sequence` mandates `import → itemDefinition → drgElement → artifact → ... → DMNDI`; kie-tools issue #1017 shows serializers get this wrong and fail validation).
7. **Hit policy + aggregator** (Pitfall 7 — `sum` can't serialize back to `hitPolicy="COLLECT" aggregation="SUM"`).
8. **IDs and `href` references** (the frontend model generates its own row IDs; DMN `id`/`href` round-trip requires stable ID preservation).

**Why it happens:**
The frontend was built for *authoring* a tabular view, not for *serializing* DMN. The path "import DMN → frontend model → export DMN" was never exercised because the backend was stubbed.

**How to avoid:**
Make the **DMN XML model the canonical store**; the frontend model is a *projection* for editing, not a storage format. On import, keep the full DMN DOM (or a faithful internal IR) server-side; the frontend receives a view projection plus the original element IDs. On export, mutate the canonical IR with the frontend's edits (cell values, hit policy) and serialize the IR — never reconstruct DMN from the frontend model. Preserve IDs, annotations, DMNDI, and ordering by construction. Validate serialized output against the XSD `xsd:sequence` ordering. Add a round-trip test: import a representative `.dmn` → export → byte-compare (modulo whitespace) or DOM-compare.

**Warning signs:**
- Export reconstructing DMN from `DecisionTableRow[]` instead of mutating the canonical IR.
- Round-trip tests that only check "table still has the same rows," ignoring annotations/DMNDI/IDs.
- Generated DMN failing XSD validation due to element order.
- Frontend row IDs (`crypto.randomUUID()`) leaking into DMN `id` attributes.

**Phase to address:** Phase 29 (parser/IR) defines the canonical IR; Phase 34 (exporter/round-trip) closes the loop; Phase 33 (frontend) must edit *through* the IR projection, not replace it.

---

### Pitfall 12: Claiming "DMN Level 3" without full FEEL (conformance-vs-pragmatism)

**What goes wrong:**
OMG DMN §2.1 is explicit: software may claim conformance with a level **if and only if it fully matches** all applicable compliance points. Partial matches may claim "based on DMN" but **not** conformance. Level 3 mandates **all 65 FEEL grammar rules, the entire built-in function library (~70 functions across conversion/boolean/string/list/numeric/date-time/range/temporal/sort/context), all boxed expression types, ternary logic, the type lattice, and decision-service execution semantics**. The pragmatic trap: ship decision tables + S-FEEL + a "nice" subset of FEEL (arithmetic, comparison, if, context) and label it "DMN Level 3." This is non-compliant and misleads enterprise buyers (the BRMS target market) who may verify with the OMG DMN TCK.

**Why it happens:**
Full FEEL is large (~70 built-ins, 65 grammar rules, 11 boxed expression types). The 80/20 instinct says "most models use arithmetic, if, and decision tables." But Level 3 is binary — there is no "Level 3-ish."

**How to avoid:**
Decide explicitly and *document the decision*: either (a) target **Level 2 (S-FEEL + decision tables)** and label it honestly ("DMN Level 2 with partial FEEL extensions"), or (b) target **Level 3** and budget for the full built-in library + quantified expressions + filter + path + instance of + type lattice + decision services. The PROJECT.md says "Full DMN Level 3 FEEL was selected → complete FEEL semantics, larger scope" — so (b) is the commitment. Track conformance as a checklist (one item per built-in function family, per grammar rule, per boxed expression type) and **do not tag v6.0 as Level 3 until the checklist is complete**. Run the OMG DMN TCK if feasible. If scope must be cut, cut the *claim*, not the checklist.

**Warning signs:**
- A `BUILTINS.md` checklist that's < 70 functions.
- `for`/`some`/`every`/`filter`/`instance of` unimplemented but the milestone tagged "Level 3."
- No decision-service support (§10.4) but "Level 3" claimed.
- The type lattice / `conforms to` unimplemented.
- "We'll do the rest later" items counted toward the claim.

**Phase to address:** Phase 30 (FEEL evaluator) owns the bulk of the conformance surface; Phase 34 (conformance verification) closes the checklist before the v6.0 tag. The roadmapper should make the conformance checklist a phase-exit criterion.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Reusing `Value: PartialEq` for FEEL equality | No new types, faster start | Three-valued-logic bugs across every FEEL feature; silent wrong results | Never — fix in Phase 30 |
| Parse FEEL per evaluation (no AST cache) | Simpler evaluator | 4–6x throughput loss on hot paths | Never for production; maybe in a Phase 30 spike, removed before Phase 31 |
| Flat `HitPolicyId` string union (keep frontend as-is) | No frontend refactor | Can't represent FIRST/OUTPUT ORDER; can't round-trip COLLECT aggregators | Never — must refactor in Phase 33 |
| Skip DMNDI parsing ("execution doesn't need it") | Smaller parser | Round-trip loses all graphical layout; frontend can't render imported diagrams | Acceptable to *skip execution*, but must still preserve DMNDI in the IR for round-trip (Phase 29) |
| Implement only S-FEEL + decision tables, label "Level 3" | Smaller scope, faster ship | Non-compliant claim; enterprise buyer audit risk; rework to defend the label | Never — claim Level 2 honestly if scope is cut |
| Frontend model as the canonical store | Reuse existing editor state | Round-trip fidelity loss; ID/annotation/DMNDI destruction | Never — IR is canonical (Phase 33) |
| Map DMN PRIORITY onto existing Rete `salience` | Reuse conflict-resolution code | Wrong semantics (salience ≠ enumerated-output priority) | Never |
| Store FEEL cell text, parse on eval | No IR needed | Performance cliff; no static validation | Only in MVP spike; Phase 31 must pre-parse |

## Integration Gotchas

Connecting the new DMN/FEEL stack to the existing rRules v5.0 engine, API, metrics, trace, and frontend.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| **`engine::Expression` AST** | Unifying `Expression` and `FeelExpression` into one enum | Separate enums; share `Value` (extended) and extracted free-function helpers (`numeric_op`, etc.) |
| **`ExecutionEngine` trait / `EngineRegistry`** | Forcing DMN through the Rete/TREAT/LEAPS/PHREAK `ExecutionEngine` trait (DMN is not a Rete algorithm) | DMN evaluator is a separate `DecisionEvaluator` that does *not* implement `ExecutionEngine`; `EngineRegistry` stays Rete-family; the `/v1/decisions/{id}/evaluate` route calls the DMN evaluator directly |
| **Prometheus dedicated registry** (v5.0 Phase 24) | Reusing `rrules_execute_requests_total` / `_rules_fired_total` for DMN evaluations → double-counts if a DMN decision internally invokes the rule engine, and conflates two eval models | Add DMN-specific counters on the same dedicated registry: `rrules_dmn_evaluations_total`, `rrules_dmn_decision_duration_seconds`, `rrules_dmn_hit_policy_violations_total` (labeled by `decision_id`); keep Rete counters untouched; if DMN calls Rete, both increment (documented as "DMN-driven rule firings") |
| **`RuleExecutionTracker` / `CoverageTracking` singletons** (v5.0 Phase 26) | Writing DMN rule-hit records into the Rete `RuleExecutionTracker` → dead-rule detection now reports DMN hits as Rete firings; coverage semantics differ (DMN "hit" ≠ Rete "fired") | Separate `DmnDecisionTracker` (or a `source: "dmn"|"rete"` tag on records); dead-rule detection stays Rete-only; add a separate dead-decision surface if needed |
| **`TraceEvent` / `TraceData`** (v5.0 Phase 28) | Reusing `TraceEvent { rule_id, timestamp, event_type: "rule_fired" }` for DMN — `event_type` is `&'static str` constrained to one value; DMN trace shape differs (decision hits, output entries, boxed-expr steps) | New `DmnTraceEvent` with `decision_id`, `matched_rule_ids`, `output_values`, `hit_policy`, `evaluation_step`; the API layer maps both `TraceData` and `DmnTraceData` into a unified `InferenceTrace` response shape with a `source` discriminator; do not widen `TraceEvent` |
| **Frontend `DecisionTable.tsx` + `HitPolicySelector.tsx`** (v4.0) | Treating the existing editor as already-correct and wiring it to the backend as-is | Refactor `HitPolicySelector` model first (Pitfall 7); the editor *projection* is fine, the *data model* is wrong; edit through the canonical IR (Pitfall 11) |
| **`/v1/execute` vs `/v1/decisions/{id}/evaluate`** | Routing DMN through `/v1/execute` (the Rete path) | Separate route; DMN has its own request/response shapes (decision inputs as a context, outputs as a context + trace); reuse auth/RBAC/governance middleware, not the executor |
| **JWT/RBAC governance** | Skipping maker-checker on DMN decisions because "they're just tables" | DMN decisions are governable artifacts like rules; reuse the approval/audit/Git-versioning pipeline; a decision table change is a Git commit + approval |
| **Git + PostgreSQL storage** | Storing DMN as JSON in the rules DB, losing XML fidelity | Store the canonical `.dmn` XML in Git (like `.rules` files today); store metadata + parsed-IR cache in PostgreSQL; the IR is regenerable from XML |
| **`compare_values` helper** | Sharing it for FEEL comparisons | Don't — it returns `bool`/Rust ordering, not three-valued; extract numeric helpers but reimplement comparison for FEEL (Pitfall 1) |
| **Engine dropdown in test-runner** (v5.0 Phase 28) | Showing the Rete/TREAT/LEAPS/PHREAK dropdown for DMN decisions | Hide or disable the engine dropdown on the DMN decision-evaluation view; DMN has no engine selection |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| FEEL re-parsing per evaluation | CPU pegged on `FeelParser`; `Pattern::compile` in profiles; 10k evals of a 30-cell table do 300k parses | Pre-parse all cell FEEL into ASTs at DMN load time; store `Arc<FeelExpression>` on the clause | >100 evals/sec on any non-trivial table |
| `f64` for FEEL numbers | `0.1 + 0.2 = 0.30000000000000004`; financial totals off by cents | `rust_decimal::Decimal` (Decimal128) for FEEL `number` | Any money/rate calculation |
| Per-evaluation `Scope` allocation | Decision eval allocates a fresh `HashMap` scope per cell | Arena-allocate or reuse a scope stack; `Scope` is a small linked structure | High QPS decision endpoints |
| Re-parsing DMN XML per evaluation | Every `evaluate` reads the `.dmn` file | Parse once at load into the IR; cache the IR (LRU, like Camunda 8.2's DRG cache) | Any production load |
| Building the dependency graph for boxed contexts per evaluation | Context eval re-topo-sorts every call | Build the topo order at load; store `Vec<entry_index>` evaluation order on the context IR | Deeply nested boxed contexts |
| No LRU bound on parsed-AST/IR cache | Memory grows unbounded with many decisions | `moka` or `lru` cache with a bound; evict by decision ID | Hundreds of deployed decisions |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| DMN XML parser without XXE hardening | External-entity injection reads local files / SSRF via DMNDI/DC/DI imports | `quick_xml::Reader` with DTD disabled, no external entity expansion, no external schema fetch; reject `<!DOCTYPE` and `SYSTEM`/`PUBLIC` identifiers |
| Accepting arbitrary FEEL from untrusted users without parsing limits | FEEL parser DoS (deeply nested expressions, exponential backtracking in a naive grammar) | Use a parser with bounded recursion depth; reject expressions over a size limit; benchmark catastrophic-backtracking cases |
| DMN decisions invoking external/host functions | If `externally-defined functions` (Java/PMML mapping) are exposed, callers can invoke arbitrary host code | Do NOT implement external-function invocation for v6.0 (it's a Level 3 feature but a security hazard in a multi-tenant BRMS); if needed later, sandbox/allowlist |
| Storing FEEL `null`-coercion results as "no error" | Silent null hides data-quality / injection-induced type mismatches | Log `null`-from-type-mismatch as a diagnostic; surface in trace when enabled; never silently coerce untrusted input |
| Decision-table `id`/`href` from untrusted XML used as DB keys | Injected IDs used in DB lookups or log injection | Validate IDs as `^[A-Za-z_][A-Za-z0-9_-]*$`; never interpolate raw into SQL/log |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Hit-policy descriptions that mislead (current: "Priority — Rules are ordered by priority") | Modelers build wrong tables; runtime surprises | Spec-accurate descriptions: "Priority — Highest-priority output among matches wins (output must be enumerated)" |
| ANY conflict surfaced as a generic 500 | Modeler can't find the conflicting rules | Distinct UI state: "Rules R3 and R7 matched but disagreed ('Approved' vs 'Declined')" |
| FEEL `null` results shown as empty cells | Modeler thinks the table is broken; can't distinguish "no match" from "evaluated to null" | Show `null` explicitly with a tooltip "evaluated to null (missing/incompatible input)"; show "no match" separately |
| Round-trip silently drops annotations | Modeler loses documentation on re-export | Preserve annotations; warn on import if annotations exceed display capacity |
| Frontend can't select FIRST or OUTPUT ORDER | Modeler can't express valid DMN models | Add both hit policies (Pitfall 7) |
| Aggregator hidden behind COLLECT (two-step UI) | Modeler misses C+/C</C>/C# | Two-step selector: choose COLLECT, then aggregator dropdown appears with COUNT default |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces — verify each before tagging v6.0.

- [ ] **FEEL equality:** Often missing three-valued semantics — verify `"a" = 1` → `null`, `null < 3` → `null`, `null = null` → `true`, `not(null)` → `null`.
- [ ] **PRIORITY:** Often implemented as first-match — verify a test where the highest-priority output is on a later row than another match.
- [ ] **ANY:** Often missing conflict detection — verify a conflicting A table returns a violation error, not a silent pick.
- [ ] **COLLECT aggregators:** Often missing empty-match semantics and multi-output rejection — verify `C+` on empty → `null`, `C#` on empty → `0`, `C+` with 2 outputs rejected at load.
- [ ] **Hit policies:** Often missing FIRST and OUTPUT ORDER — verify both are selectable and serializable.
- [ ] **FEEL built-ins:** Often missing the "long tail" — verify `sort`, `median`, `stddev`, `mode`, `product`, `context merge`, `context put`, `get entries`, the full range functions (`before`/`after`/`overlaps`/...), and temporal (`day of week`/`week of year`).
- [ ] **Boxed expressions:** Often missing relation, boxed function, boxed conditional, boxed filter, boxed iterator — verify each renders in the frontend and evaluates.
- [ ] **Context dependency order:** Often evaluated textually — verify a test with a forward reference resolves correctly.
- [ ] **DMN round-trip:** Often drops annotations + DMNDI — verify byte/DOM-compare after import→export on a file with annotations and DI.
- [ ] **DMN XML namespaces:** Often accepts only one version — verify import of 1.1, 1.3, and 1.5 files.
- [ ] **XXE hardening:** Often left default — verify `<!DOCTYPE` with `SYSTEM` is rejected.
- [ ] **AST cache:** Often parse-on-eval — verify a 30-cell table evaluated 10k times does ~30 parses, not 300k.
- [ ] **Metrics separation:** Often double-counts DMN as Rete — verify `rrules_dmn_evaluations_total` exists and `_execute_requests_total` is unchanged by a pure-DMN eval.
- [ ] **Trace shape:** Often reuses `rule_fired` — verify DMN trace has decision/rule-hit/output-entry fields.
- [ ] **Conformance claim:** Often "Level 3" without full FEEL — verify the built-in checklist is 100% before tagging.

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Three-valued logic leak (Pitfall 1) | HIGH | Introduce `feel_equals`/`feel_compare` returning `Option<bool>`/`Option<Ordering>`; audit every comparison call site; rewrite affected tests. Affects the whole FEEL evaluator — better to fix in Phase 30. |
| Wrong AST reuse (Pitfall 2) | HIGH | Extract shared leaf `Value`; split `FeelExpression` from `Expression`; migrate all FEEL call sites. Expensive if done after Phase 30. |
| `Value` missing types (Pitfall 3) | MEDIUM | Add `Date`/`Time`/`Duration`/`Decimal` variants; update serde; backfill parsers. Touches serialization — migrate carefully. |
| PRIORITY-as-first (Pitfall 4) | LOW | Fix the selection function; add `allowedValues` validation; add tests. Localized to hit-policy module. |
| ANY no-conflict (Pitfall 5) | LOW | Add output-comparison check; new error type; surface in API + frontend. Localized. |
| Frontend hit-policy model (Pitfall 7) | MEDIUM | Refactor `HitPolicyId` to `{policy, aggregator?}`; update selector + table editor + API client. Touches all frontend decision-table code. |
| No AST cache (Pitfall 8) | MEDIUM | Add `DashMap` cache or pre-parse-at-load; change evaluator signature from `&str` to `&FeelExpression`. Touches all call sites. |
| Context textual-order eval (Pitfall 9) | MEDIUM | Add dependency graph + topo sort + cycle detection. Localized to context evaluator. |
| DMN parser bugs (Pitfall 10) | MEDIUM–HIGH | Each is a localized fix, but a wrong namespace/XXE/config requires parser reconfiguration and re-testing the import corpus. |
| Round-trip fidelity (Pitfall 11) | HIGH | Restructure to canonical-IR-as-store; frontend edits through projection. Architecture-level change — must be done in Phase 33, not retrofitted. |
| False Level 3 claim (Pitfall 12) | HIGH (reputational) | Implement the missing FEEL surface OR downgrade the claim to Level 2. Either is large if deferred. |

## Pitfall-to-Phase Mapping

Assumed v6.0 phase structure (phase numbers continue from 29; adjust to actual roadmap):

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 1. Three-valued logic leak | Phase 30 (FEEL evaluator) | Conformance tests: `"a"=1`→null, `null<3`→null, `null=null`→true, `not(null)`→null, filter drops null predicates |
| 2. AST reuse/duplication | Phase 30 (FEEL evaluator) | `FeelExpression` is a distinct enum; `evaluate(&Scope)` signature; no `&[&Fact]` in FEEL path |
| 3. `Value` type gaps | Phase 30 (FEEL evaluator) | `Date`/`Time`/`YearMonthDuration`/`DayTimeDuration`/`Decimal` variants; `0.1+0.2=0.3` test passes |
| 4. PRIORITY as first-match | Phase 31 (decision-table engine) | Test: highest-priority output on a later row wins; `allowedValues` validation rejects P tables without enumeration |
| 5. ANY conflict not detected | Phase 31 (engine) + Phase 32 (API) + Phase 33 (UI) | Conflicting A table returns `DMN_HIT_POLICY_VIOLATION`; frontend shows the two disagreeing rules |
| 6. COLLECT aggregator bugs | Phase 31 (decision-table engine) | Empty-match (`C+`→null, `C#`→0), multi-output rejected at load, non-numeric SUM→null |
| 7. Frontend hit-policy vocabulary | Phase 33 (frontend wiring); backend enum in Phase 31 | `first` and `output-order` selectable; `HitPolicyId` is `{policy, aggregator?}`; round-trips to `hitPolicy="COLLECT" aggregation="SUM"` |
| 8. FEEL re-parsing (no AST cache) | Phase 30 (cache primitive) + Phase 31 (pre-parse at load) | Profile: 30-cell table × 10k evals = ~30 parses; `Pattern::compile` absent from hot path |
| 9. Boxed-context textual order | Phase 30 (FEEL evaluator) | Forward-reference test passes; cyclic context rejected with `CyclicContext` error |
| 10. DMN XML parser bugs | Phase 29 (DMN parser + model) | Import corpus (1.1/1.3/1.5 files) passes; XXE `<!DOCTYPE SYSTEM` rejected; `hitPolicy` defaults to UNIQUE; annotations preserved |
| 11. Round-trip fidelity | Phase 29 (canonical IR) + Phase 33 (frontend edits through IR) + Phase 34 (exporter) | Import→export DOM-compare preserves annotations, DMNDI, IDs, element order; XSD-valid output |
| 12. False Level 3 claim | Phase 30 (FEEL surface) + Phase 34 (conformance checklist exit) | Built-in checklist 100%; `for`/`some`/`every`/`filter`/`instance of`/boxed-conditional/boxed-function/decision-service all implemented; TCK run if feasible |

## Sources

- OMG DMN 1.3 spec (Clause 10 FEEL, Clause 8 decision tables, Clause 2.1 conformance) — https://www.omg.org/spec/DMN/1.3/PDF (HIGH, primary)
- OMG DMN task force plain-text FEEL spec — https://github.com/omg-dmn-taskforce/plain-text-spec/blob/main/10-expression-language-FEEL.adoc (HIGH, primary)
- Drools DMN FEEL handbook — https://kiegroup.github.io/dmn-feel-handbook/ (HIGH, implementer reference)
- Camunda 8 — "Choosing the DMN hit policy" + "Hit policy" — https://docs.camunda.io/docs/components/best-practices/modeling/choosing-the-dmn-hit-policy/ , https://docs.camunda.io/docs/8.8/components/modeler/dmn/decision-table-hit-policy/ (HIGH, implementer docs)
- Method & Style (Bruce Silver) — "Decision Table Hit Policy Explained" — https://www.methodandstyle.com/blog/decision-table-hit-policy-explained/ (MEDIUM, expert blog)
- Camunda feel-scala issue #1149 — FEEL re-parsing on every evaluate — https://github.com/camunda/feel-scala/issues/1149 (HIGH, real bug report)
- Camunda feel-scala issue #814 — 6–12x perf regression 1.16→1.17 — https://github.com/camunda/feel-scala/issues/814 (HIGH)
- Camunda blog — "Improving the Performance of the Camunda DMN Engine" (4–6x from caching) — https://camunda.com/blog/2016/08/dmn-performance-improvements/ (MEDIUM-HIGH, measured)
- Camunda/Zeebe issue #8571 — LRU cache for parsed DMN DRGs — https://github.com/camunda/camunda/issues/8571 (HIGH)
- Camunda feel-scala issue #6 — `null = null` equality handling — https://github.com/camunda/feel-scala/issues/6 (HIGH)
- Camunda docs issue #3964 — unary-test null-vs-numeric edge case — https://github.com/camunda/camunda-docs/issues/3964 (HIGH)
- Trisotech — "Make Way for B-FEEL" (DMN 1.6 dialect differences) — https://www.trisotech.com/make-way-for-b-feel/ (MEDIUM, vendor blog)
- Operaton DMN Compatibility — parser/DI/namespace pitfalls — https://iou-architectuur.open-regels.nl/linked-data-explorer/reference/operaton-dmn-compatibility/ (HIGH, implementer checklist)
- Flowable `DmnXMLConverter.java` — reference parser (XXE, CDATA, namespace selection, `hitPolicy` default) — https://github.com/flowable/flowable-engine/blob/master/modules/flowable-dmn-xml-converter/src/main/java/org/flowable/dmn/xml/converter/DmnXMLConverter.java (HIGH, source)
- OMG DMN namespace versioning PR #33 + DMN15-159 — https://github.com/omg-dmn-taskforce/omg-dmn-spec/pull/33 , https://issues.omg.org/issues/DMN15-159 (HIGH, primary)
- DMNDI13.xsd — QName/DI structure — https://github.com/omg-dmn-taskforce/omg-dmn-spec/blob/v1.4/xsd/DMNDI13.xsd (HIGH, primary)
- apache/kie-tools issue #3594 — vendor extension text dropped on round-trip — https://github.com/apache/incubator-kie-tools/issues/3594 (HIGH, real bug)
- apache/kie-issues #1017 — `xsd:sequence` order violation in marshaller — https://github.com/apache/incubator-kie-issues/issues/1017 (HIGH, real bug)
- OMG issue DMN11-155 — extension round-trip not mandated — https://issues.omg.org/issues/DMN11-155 (HIGH, primary)
- CEUR-WS — "The Effectiveness of DMN Portability" — https://ceur-ws.org/Vol-1875/paper14.pdf (MEDIUM, academic)
- bpmn.io — DMN 1.3 support / migration — https://bpmn.io/blog/posts/2020-dmn-js-8-0-0.html (MEDIUM-HIGH, implementer)
- ScratchyDisk.DmnEngine — configurable AST cache scope — https://github.com/Scratchydisk/ScratchyDisk.DmnEngine/blob/master/readme.md (MEDIUM, reference impl)
- rRules source inspection: `engine/src/data_types.rs` (`Value`), `engine/src/rules.rs` (`Expression` AST), `engine/src/engine/registry.rs` (`TraceEvent`/`TraceData`), `frontend/src/features/rules/types.ts` (`HIT_POLICIES`), `frontend/src/features/rules/HitPolicySelector.tsx` (HIGH, ground truth for integration pitfalls)

---
*Pitfalls research for: Adding DMN Level 3 / FEEL to rRules v5.0 (Rust actix-web + React/TS BRMS)*
*Researched: 2026-07-06*