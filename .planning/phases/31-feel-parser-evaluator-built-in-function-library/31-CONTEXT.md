# Phase 31: FEEL Parser + Evaluator + Built-in Function Library - Context

**Gathered:** 2026-07-07
**Status:** Ready for planning
**Mode:** Smart discuss (autonomous, batch table proposals)

<domain>
## Phase Boundary

Phase 31 builds the FEEL (Friendly Enough Expression Language) parsing and
evaluation layer on top of Phase 30's value-domain foundation. It delivers:

- A hand-written recursive-descent **parser** (Pratt precedence) covering FEEL
  grammar rules 1–65, with dual entry points (`SimpleUnaryTests` for unary-test
  cells, `Expression` for output cells) and keyword-as-name disambiguation —
  DMN-11.
- An **evaluator** with correct three-valued logic — `feel_equals` /
  `feel_compare` return `Option<_>` (null → `Value::Null`), explicit truth
  tables for `and` / `or` / `not` (no reuse of Rust `==` / `compare_values`),
  and the DMN-12 conformance cases (`"a"=1`→null, `null<3`→null,
  `null=null`→true, `not(null)`→null) — DMN-12.
- The full operator set: arithmetic, comparison, boolean, string
  concatenation, date-time arithmetic and duration operations — DMN-13.
- Control + quantified expressions: `if`, `for`, `some` / `every`, path,
  filter, `instance of` — DMN-14.
- All 10 **boxed expression** types (literal, context, list, relation, function
  invocation, function definition, conditional, filter, for, every/some),
  recursive; boxed contexts evaluated in dependency (topological) order with
  cycle detection — DMN-15.
- User-defined FEEL **lambdas** (grammar rule 55, non-external) — DMN-16.
- The full **built-in function library** (~70+ functions across conversion /
  string / list / numeric / date-time / context / boolean / range / equality
  families) — DMN-17.
- An **AST cache** (`DashMap<String, Arc<FeelExpression>>`) so cell / expression
  FEEL is pre-parsed to `Arc<FeelExpression>` at DMN load time and the evaluator
  takes `&FeelExpression` and never re-parses — preventing the #1 DMN
  performance bug (4–6× re-parse throughput loss; a 30-cell table × 10k evals
  ≈ 30 parses, not 300k) — DMN-18.

This is the largest phase of the milestone. It produces
`engine/src/dmn/feel/{ast.rs, parser.rs, eval.rs, builtins.rs, temporal.rs,
coercion.rs}` (extending the existing `feel/` module from Phase 30). The
`FeelExpression` AST is **distinct** from the rule engine's `Expression` AST;
the FEEL path is never routed through `EngineRegistry` / the Rete evaluators.

</domain>

<decisions>
## Implementation Decisions

### Plan Structure (Grey Area 1 + 2 synthesis)

- **Granularity:** Plans are aligned **one-per-DMN-requirement** (DMN-11..18),
  giving the finest verification gates (each plan verified before the next
  begins). This is the user's explicit preference over the coarser 4–5-plan
  by-layer option.
- **Builtins split:** DMN-17 (~70+ functions) is large enough to warrant **2
  plans by family** (per Grey Area 2): conversion/string/list/numeric, then
  date-time/context/boolean/range/equality.
- **Resulting plan set (9 plans)** — planner to confirm exact boundaries and
  may merge/split if a requirement proves trivially small or unexpectedly large:
  1. `31-01` — DMN-11: Lexer + recursive-descent parser (Pratt precedence,
     grammar 1–65), dual entry points `SimpleUnaryTests` / `Expression`,
     keyword-as-name disambiguation; `FeelExpression` AST skeleton.
  2. `31-02` — DMN-12: Evaluator core + three-valued logic (`feel_equals` /
     `feel_compare` → `Option<_>`, explicit `and` / `or` / `not` truth tables,
     null propagation, conformance cases).
  3. `31-03` — DMN-13: Operators (arithmetic, comparison, boolean, string
     concatenation, date-time arithmetic, duration operations).
  4. `31-04` — DMN-14: Control + quantified (`if`, `for`, `some` / `every`,
     path, filter, `instance of`).
  5. `31-05` — DMN-15: All 10 boxed expressions (recursive); boxed-context
     topological (dependency) evaluation with cycle detection.
  6. `31-06` — DMN-16: User-defined lambdas (grammar rule 55, non-external);
     extend `FeelFunction` to carry the closure body.
  7. `31-07` — DMN-17 part 1: Built-in functions (conversion / string / list /
     numeric families) + the `HashMap<&str, BuiltinFn>` registry.
  8. `31-08` — DMN-17 part 2: Built-in functions (date-time / context /
     boolean / range / equality families).
  9. `31-09` — DMN-18: AST cache (`DashMap<String, Arc<FeelExpression>>`),
     load-time pre-parse wiring into the DMN IR loader, and the
     evaluator-takes-`&FeelExpression` contract.
- **Verification cadence:** Verify each plan before starting the next (gate per
  plan). This is the user's preference (finest gates).
- **Cache placement:** A thin `FeelExpression` + cache-skeleton may be
  introduced in `31-01` (so the parser has something to produce into); the
  final load-time wiring lands in `31-09`. Planner to decide how much cache
  scaffolding `31-01` carries.

### Built-in Function Library (Grey Area 2)

- **Breadth:** Full ~70+ built-in functions now, native Rust implementations
  across all 8 families (meets the "are available" success criterion literally;
  Phase 35 is the conformance gate that proves them against TCK).
- **Dispatch:** A `HashMap<&str, BuiltinFn>` registry table (name → function
  pointer / closure), registered at init. Chosen over a nested `match` (poor
  scaling to ~70 entries) and trait objects per family (over-engineered for a
  closed, flat function space).
- **Numeric edge cases:** Division by zero, overflow, and invalid numeric
  operations return `Value::Null` per FEEL semantics (null propagation), NOT
  `Result` / panic. This follows Phase 30's `Option<bool>` null-propagation
  pattern.
- **List/range builtins** (`sort`, `distinct values`, `union`, `concatenate`,
  `index of`, `list contains`, etc.): implement all now (not deferred to
  Phase 35).

### Testing & TCK Conformance (Grey Area 3)

- **TCK vectors:** Scaffold the OMG TCK FEEL test harness now (loader + runner
  skeleton + a small vendored subset), but **defer full TCK vector passage to
  Phase 35** (the conformance gate). Phase 31 must not block on full TCK
  conformance.
- **Property tests:** Yes, with regression seeds — consistent with Phase 30-04
  (`proptest` + committed seeds). Cover three-valued logic truth tables,
  numeric Decimal128 invariants, and operator conformance.
- **Test corpus:** Vendored OMG TCK FEEL subset + hand-written conformance cases
  (the DMN-12 truth-table cases are mandatory unit tests).
- **Snapshot/golden tests:** Yes, for key boxed-expression and operator
  expressions (evaluator output is stable enough to snapshot).

### Scope, QN Resolution & Error Diagnostics (Grey Area 4)

- **Scope model (user override):** **Flat** `Scope` — a context is a flat map;
  nested scope resolution uses **explicit dependency tracking** rather than a
  parent/child HashMap chain. (Overrides the recommended chained-Scope
  option.)
- **Boxed-context dependency order:** Flat scope's explicit dependency
  tracking drives the topological (dependency-order) evaluation of boxed
  contexts required by DMN-15, with DFS-based cycle detection. This is
  consistent with — and is the mechanism for — DMN-15's topological-order
  requirement.
- **Parser error strategy (user override):** The parser performs **error
  recovery producing a partial AST** (error/`Err` nodes carrying source spans),
  not fail-fast. Rationale: forward-looking for the Phase 34 frontend editor,
  which needs per-cell parse errors displayed while the rest of the table still
  loads. (Overrides the recommended fail-fast option.)
- **Eval error propagation:** Evaluation errors carry **source location**
  (cell / rule / expression) so the Phase 33 inference trace can point at the
  failing expression. Default recommendation retained (not objected to).
- **Boxed-expression AST shape:** A single `FeelExpression` enum with all 10
  boxed variants (matches the deliverable's "`FeelExpression` enum"). Default
  recommendation retained; planner may introduce a `BoxedExpr` sub-enum only if
  enum size / readability demands it.

### Locked by success criteria / deliverables (not negotiable)

- Hand-written recursive-descent parser with **Pratt precedence**; dual entry
  points `SimpleUnaryTests` + `Expression`; keyword-as-name disambiguation
  (DMN-11).
- `FeelExpression` enum **distinct from** the rule engine's `Expression` AST;
  `evaluate(&Scope)` signature (DMN-11 / DMN-12).
- `feel_equals` / `feel_compare` return `Option<_>`; null → `Value::Null`;
  **explicit truth tables** for `and` / `or` / `not` — no Rust `==` /
  `compare_values` reuse for FEEL equality (DMN-12).
- All 10 boxed expressions recursive; boxed contexts topological order + cycle
  detection (DMN-15).
- User-defined lambdas, grammar rule 55, **non-external** (DMN-16).
- `DashMap<String, Arc<FeelExpression>>` AST cache; evaluator takes
  `&FeelExpression`, never re-parses (DMN-18).
- Files: `engine/src/dmn/feel/{ast.rs, parser.rs, eval.rs, builtins.rs,
  temporal.rs, coercion.rs}`.
- Decimal128 via `rust_decimal`, **never `f64`** in the FEEL production path
  (Phase 30 Anti-Pattern).
- **No implicit number↔string conversion** — that is the explicit `string()` /
  `number()` built-ins (Phase 30 Pitfall 6), not a lattice conversion.
- Zero Rete regression: the FEEL path is never routed through
  `EngineRegistry` / the Rete evaluators (`engine/src/engine/`).

### Claude's Discretion

- Exact plan-file boundaries (the 9-plan structure above is guidance; the
  planner may merge a trivially-small requirement into an adjacent plan or
  further split an unexpectedly-large one, provided each DMN-11..18 requirement
  has a home and each plan is independently verifiable).
- Internal `FeelExpression` variant naming and payload shapes (e.g., how
  `Path`, `Filter`, `Context`, `For`, `Quantified` are represented), subject to
  the single-enum constraint above.
- `BuiltinFn` type shape (function pointer vs closure capturing the registry),
  and registry initialization timing.
- How much `FeelExpression` / cache scaffolding `31-01` carries vs `31-09`.
- `coercion.rs` contents: the explicit FEEL coercions used by operators and
  built-ins (distinct from the Phase 30 lattice `implicit_convert`).
- Whether `temporal.rs` needs extension in this phase (Phase 30's `mod.rs`
  suggested `@""` parse/format and duration-arithmetic helpers may already have
  landed in Plan 30-03 — planner to verify and only add what is missing for
  DMN-13 date-time arithmetic).

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets (Phase 30 foundation — `engine/src/dmn/feel/`)

- **`types.rs`** — `FeelType` enum (the 14-node DMN 1.5 type lattice:
  `Null`/`Any`/`Number`/`String`/`Boolean`/`Date`/`Time`/`DateTime`/
  `DaysTimeDuration`/`YearsMonthsDuration`/`List`/`Context`/`Range`/
  `Function`), `FeelRange` carrier (start/end + inclusivity, boxed to break the
  `Value`↔`FeelRange` cycle), `FeelFunction` carrier (arity + param/return
  types — **closure body is Phase 31's job**, per the `Value::Function` doc
  comment). Lattice relations already implemented: `equivalent` (§10.3.2.9.1),
  `conforms` (§10.3.2.9.2, `Null <: T <: Any`), `ConvertCtx` / `implicit_convert`
  (§10.3.2.9.4 — exactly the three spec-defined conversions, NO number↔string),
  `type_of` (exhaustive over all `Value` variants, no wildcard), and
  `resolve_type_ref` (IR `feel:` type_ref strings → `FeelType` — **consumed by
  Phase 31 IR type resolution**).
- **`temporal.rs`** — `TimeValue`, `DateTimeValue` (fixed-offset, IANA-zone, or
  local forms), `YearsMonthsDuration`. `@""` parse/format and DST-correct
  duration arithmetic may have landed in Plan 30-03 (planner to verify).
- **`numeric.rs`** — `feel_numeric_eq(a, b) -> Option<bool>`: null-propagating
  FEEL numeric equality with number/integer cross-type promotion, `Decimal`
  comparison (never `f64`). **This `Option<bool>` pattern is the template for
  Phase 31's three-valued-logic operators** — they consume `None` as null.
- **`mod.rs`** — module wiring (`pub mod numeric; pub mod temporal; pub mod
  types;`). Phase 31 adds `ast`, `parser`, `eval`, `builtins`, `coercion`.

### `crate::data_types::Value` (the evaluator's value type)

Phase 30 extended `Value` **additively** with 8 FEEL variants (legacy Rete
variants unchanged): `Number(Decimal)`, `Date(NaiveDate)`,
`Time(TimeValue)`, `DateTimeTz(DateTimeValue)`, `DaysTimeDuration(TimeDelta)`,
`YearsMonthsDuration(YearsMonthsDuration)`, `Range(Box<FeelRange>)`,
`Function(FeelFunction)`. The evaluator produces and consumes `Value`. Note
`Value::DateTime` (`NaiveDateTime`) is the **legacy Rete** variant; FEEL
date-time uses `Value::DateTimeTz`.

### Established Patterns (must follow for consistency)

- **Closed enum + free functions**, not a trait graph (Phase 30 research
  Pattern 2): FEEL's value/type space is small and closed, so a table/function
  is clearer and property-testable. Apply the same idiom to `FeelExpression`,
  `BuiltinFn`, and the truth tables.
- **`Option<bool>` / `Option<Ordering>` for null-propagating comparisons** —
  the three-valued-logic signature shape. Never reuse Rust `==` / `Ord` for
  FEEL equality/ordering.
- **Decimal128 (`rust_decimal::Decimal`)** for all FEEL numbers; never `f64` in
  the production path (`0.1_f64 + 0.2_f64 = 0.30000000000000004`).
- **Rich spec-cited doc comments** (§10.3.x citations, pitfall references,
  negative-contrast tests) — match Phase 30's documentation density.
- **Tests:** inline `#[cfg(test)] mod tests` per module + integration tests in
  `engine/tests/` (cf. `engine/tests/dmn_parser.rs`, `dmn_round_trip.rs`).
  Property tests use `proptest` with committed regression seeds (Phase 30-04).
- **OMG TCK vectors** are vendored under `engine/tests/fixtures/` (Phase 30
  vendored numeric + temporal vectors) — reuse this fixture home for FEEL TCK.

### Integration Points

- **DMN IR loader (`engine/src/dmn/parser.rs`, Phase 29):** the AST cache
  (DMN-18) pre-parses every cell's FEEL text at DMN load time. The loader reads
  `TextContent` / `RuleEntry` / unary-test strings from the canonical IR and
  populates `DashMap<String, Arc<FeelExpression>>` keyed by cell source (or a
  canonical hash). Decision-table cells (Phase 32) consume these cached ASTs.
- **`feel/types.rs::resolve_type_ref`:** Phase 31 IR type resolution calls this
  to map `feel:` type_refs to `FeelType` nodes (for `instance of` and typed
  contexts).
- **Phase 32 (decision-table engine):** input entries are FEEL unary tests
  (`SimpleUnaryTests`); output entries are FEEL expressions (`Expression`).
  Phase 31's dual-entry-point parser + `evaluate(&Scope)` contract is the
  decision-table engine's foundation.
- **Phase 33 (decision evaluator + REST):** the evaluator + AST cache feed
  `POST /v1/decisions/{id}/evaluate`; eval errors carrying source location feed
  the inference trace.
- **Phase 34 (frontend):** the parser's error-recovery-with-spans behavior lets
  the editor display per-cell parse errors while the rest of the table loads.
- **`engine/src/engine/leaps/mod.rs:66`** (deferred from Phase 30): a Rete-only
  wildcard `_ => None` over `Value`. The FEEL path never reaches it; a future
  consistency comment can cite `engine/src/dmn/feel/`. Out of scope here.

</code_context>

<specifics>
## Specific Ideas

- The single most important correctness property is **three-valued logic**
  (DMN-12). The conformance cases `"a"=1`→null, `null<3`→null, `null=null`→true,
  `not(null)`→null must be mandatory unit tests, and `and` / `or` / `not` must
  use **explicit truth tables** — never Rust `==` / `compare_values`. This is
  flagged as Pitfall 1 in the ROADMAP ("Three-valued-logic leak must be
  prevented here") and is the most likely source of silent bugs.
- The headline performance property is the **AST cache** (DMN-18): a 30-cell
  table evaluated 10k times must do ~30 parses (at load), not 300k. The
  evaluator signature `evaluate(&FeelExpression, &Scope)` (taking a borrowed
  AST, not source text) is the contract that enforces this — it must be
  impossible to call the evaluator with raw source.
- The user explicitly chose **fine-grained per-requirement plans** (8–9 plans)
  with per-plan verification gates over fewer larger plans.
- The user explicitly chose **flat Scope + explicit dependency tracking** and
  **parser error recovery (partial AST)** over the chained-Scope + fail-fast
  alternatives. Both choices favor the Phase 34 editor UX.

</specifics>

<deferred>
## Deferred Ideas

- **Full OMG TCK FEEL vector passage** → Phase 35 (Conformance Verification).
  Phase 31 scaffolds the harness and runs a small vendored subset only.
- **External FEEL functions** (DMN allows externally-defined functions) → out
  of scope; DMN-16 covers only non-external user-defined lambdas. Could be a
  future phase if a real use case appears.
- **A full clippy cleanup** of the 26 pre-existing baseline errors (deferred
  from Phase 30) → a separate tech-debt `chore` phase, not Phase 31.
- **`#[serial]` guard for the flaky `monitoring::coverage` singleton test**
  (deferred from Phase 30) → a future fix phase.
- **FEEL grammar features beyond Level 3** (e.g., advanced `for` predicates,
  extension functions) → not required for Level 3 compliance; defer unless TCK
  demands.

</deferred>