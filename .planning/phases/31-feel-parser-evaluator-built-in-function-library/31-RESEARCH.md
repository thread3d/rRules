# Phase 31: FEEL Parser + Evaluator + Built-in Function Library - Research

**Researched:** 2026-07-07
**Domain:** FEEL expression language — grammar 1–65, three-valued logic, 10 boxed expressions, ~70+ built-in functions, user-defined lambdas, AST cache
**Confidence:** HIGH (grammar quoted verbatim from OMG plain-text spec; built-in function enumeration cross-checked against Camunda/Drools references; three-valued-logic semantics quoted from spec rule 49.a; Phase 30 foundation read in full; new crates `dashmap`/`regex` passed the legitimacy gate)

## Summary

Phase 31 is the largest phase of milestone v6.0. It builds the FEEL parser, evaluator, and built-in function library on top of Phase 30's value-domain foundation. The work splits cleanly into 9 plans (one per DMN-11..18, with DMN-17 split across two plans by function family) — this matches the user's locked plan structure in CONTEXT.md. This research maps each DMN requirement to a concrete implementation: the grammar-rule-to-parser-method mapping (DMN-11), the exact three-valued-logic truth tables and the `feel_equals`/`feel_compare` → `Option<_>` signature (DMN-12, the Pitfall-1-critical correctness property), the operator + date-time arithmetic matrix (DMN-13), control + quantified + path + filter + `instance of` (DMN-14), the 10 boxed expressions with flat-Scope topological dependency evaluation and DFS cycle detection (DMN-15), lambda closure capture in the extended `FeelFunction` carrier (DMN-16), the ~70+ built-in function registry (DMN-17), and the `DashMap<String, Arc<FeelExpression>>` AST cache wired into the Phase 29 IR loader (DMN-18).

**The single most important correctness property is three-valued logic (DMN-12, ROADMAP Pitfall 1).** The OMG DMN 1.5 spec grammar rule 49.a defines it verbatim: `e = null` maps to `FEEL(e) is null` (so `null = null` → `true`), but `e1 = e2` for non-null operands requires both to be "of the same kind" — a type-mismatch like `"a" = 1` therefore yields `null`, not `false`. Comparisons (`<`, `<=`, `>`, `>=`) with a `null` operand yield `null` (the input domain requires same-kind operands). `not(null)` → `null` (the negation table: "otherwise" → `null`). Phase 30's `feel_numeric_eq -> Option<bool>` is the template — every FEEL comparison/equality operator MUST return `Option<bool>` / `Option<Ordering>`, and the evaluator maps `None` → `Value::Null`. **Never** reuse Rust `==` / `Ord` / `compare_values` for FEEL equality — the research recommends a private `feel::cmp` module + an exhaustive-match lints guard to make accidental reuse a compile error.

**The headline performance property is the AST cache (DMN-18).** The evaluator signature `evaluate(&FeelExpression, &Scope) -> Value` (borrowed AST, NOT source text) is the type-level contract that prevents the #1 DMN performance bug (4–6× re-parse throughput loss). The `DashMap<String, Arc<FeelExpression>>` cache is populated at DMN load time by walking the Phase 29 IR (`InputClause.input_expression`, `RuleEntry.text`, `OutputClause.default_output_entry`, `AllowedValues::UnaryTests`) and pre-parsing every cell's FEEL text with the dual-entry-point parser (`SimpleUnaryTests` for input cells, `Expression` for output cells). A 30-cell table evaluated 10k times then does ~30 parses (at load), not 300k.

**Primary recommendation:** Implement the parser as a hand-written recursive-descent + Pratt-precedence parser over a token stream, with the precedence order `or < and < comparison < additive < multiplicative < exponentiation < unary-negation < postfix (path/filter/invocation) < primary` (spec rule 2 + rule 4, lowest-to-highest). Model the AST as a single `FeelExpression` enum (user's locked choice) carrying all textual-grammar forms + all 10 boxed-expression forms. Implement the evaluator as a pure function `evaluate(&FeelExpression, &Scope) -> Value` dispatching on `FeelExpression` variants, with the flat `Scope = { bindings: HashMap<String, Value>, deps: HashMap<String, Vec<String>> }` (user's locked flat-Scope choice) driving both name resolution and boxed-context topological evaluation. Register the ~70+ built-ins in a `HashMap<&str, BuiltinFn>` table at init.

## User Constraints (from CONTEXT.md)

### Locked Decisions

- **Plan structure:** 9 plans, one-per-DMN-requirement (DMN-17 split into 31-07 + 31-08 by family). Planner may merge a trivially-small requirement into an adjacent plan or further split an unexpectedly-large one, provided each DMN-11..18 has a home and each plan is independently verifiable. Verification cadence: verify each plan before starting the next (finest gates).
- **Builtins breadth:** Full ~70+ built-in functions now, native Rust implementations across all 8 families. Phase 35 is the conformance gate that proves them against TCK.
- **Dispatch:** `HashMap<&str, BuiltinFn>` registry table (name → function pointer / closure), registered at init. Chosen over nested `match` (poor scaling to ~70 entries) and trait objects per family (over-engineered for a closed, flat function space).
- **Numeric edge cases:** Division by zero, overflow, and invalid numeric operations return `Value::Null` per FEEL semantics (null propagation), NOT `Result` / panic.
- **List/range builtins** (`sort`, `distinct values`, `union`, `concatenate`, `index of`, `list contains`, etc.): implement all now (not deferred to Phase 35).
- **TCK vectors:** Scaffold the OMG TCK FEEL test harness now (loader + runner skeleton + small vendored subset); **defer full TCK vector passage to Phase 35**. Phase 31 must not block on full TCK conformance.
- **Property tests:** Yes, with regression seeds — consistent with Phase 30-04 (`proptest` + committed seeds). Cover three-valued logic truth tables, numeric Decimal128 invariants, operator conformance.
- **Test corpus:** Vendored OMG TCK FEEL subset + hand-written conformance cases (DMN-12 truth-table cases are mandatory unit tests).
- **Snapshot/golden tests:** Yes, for key boxed-expression and operator expressions.
- **Scope model (user override):** **Flat** `Scope` — a context is a flat map; nested scope resolution uses **explicit dependency tracking** rather than a parent/child HashMap chain. (Overrides chained-Scope.)
- **Boxed-context dependency order:** Flat scope's explicit dependency tracking drives the topological (dependency-order) evaluation of boxed contexts (DMN-15), with DFS-based cycle detection.
- **Parser error strategy (user override):** **Error recovery producing a partial AST** (`Err` nodes carrying source spans), not fail-fast. Rationale: forward-looking for Phase 34 frontend editor (per-cell parse errors while the rest of the table still loads).
- **Eval error propagation:** Evaluation errors carry **source location** (cell / rule / expression) so the Phase 33 inference trace can point at the failing expression.
- **Boxed-expression AST shape:** A single `FeelExpression` enum with all 10 boxed variants. Planner may introduce a `BoxedExpr` sub-enum only if enum size / readability demands it.
- **Locked by success criteria (non-negotiable):**
  - Hand-written recursive-descent parser with **Pratt precedence**; dual entry points `SimpleUnaryTests` + `Expression`; keyword-as-name disambiguation (DMN-11).
  - `FeelExpression` enum **distinct from** the rule engine's `Expression` AST; `evaluate(&Scope)` signature (DMN-11 / DMN-12).
  - `feel_equals` / `feel_compare` return `Option<_>`; null → `Value::Null`; **explicit truth tables** for `and` / `or` / `not` — no Rust `==` / `compare_values` reuse for FEEL equality (DMN-12).
  - All 10 boxed expressions recursive; boxed contexts topological order + cycle detection (DMN-15).
  - User-defined lambdas, grammar rule 55, **non-external** (DMN-16).
  - `DashMap<String, Arc<FeelExpression>>` AST cache; evaluator takes `&FeelExpression`, never re-parses (DMN-18).
  - Files: `engine/src/dmn/feel/{ast.rs, parser.rs, eval.rs, builtins.rs, temporal.rs, coercion.rs}`.
  - Decimal128 via `rust_decimal`, **never `f64`** in the FEEL production path (Phase 30 Anti-Pattern).
  - **No implicit number↔string conversion** — that is the explicit `string()` / `number()` built-ins (Phase 30 Pitfall 6), not a lattice conversion.
  - Zero Rete regression: the FEEL path is never routed through `EngineRegistry` / the Rete evaluators (`engine/src/engine/`).

### Claude's Discretion

- Exact plan-file boundaries (the 9-plan structure is guidance).
- Internal `FeelExpression` variant naming and payload shapes (subject to the single-enum constraint).
- `BuiltinFn` type shape (function pointer vs closure capturing the registry), and registry initialization timing.
- How much `FeelExpression` / cache scaffolding `31-01` carries vs `31-09`.
- `coercion.rs` contents: the explicit FEEL coercions used by operators and built-ins (distinct from Phase 30 lattice `implicit_convert`).
- Whether `temporal.rs` needs extension in this phase (planner verifies; only add what is missing for DMN-13 date-time arithmetic).

### Deferred Ideas (OUT OF SCOPE)

- **Full OMG TCK FEEL vector passage** → Phase 35.
- **External FEEL functions** (DMN allows externally-defined functions) → out of scope; DMN-16 covers only non-external user-defined lambdas.
- **A full clippy cleanup** of the 26 pre-existing baseline errors → a separate tech-debt `chore` phase.
- **`#[serial]` guard for the flaky `monitoring::coverage` singleton test** → a future fix phase.
- **FEEL grammar features beyond Level 3** (advanced `for` predicates, extension functions) → not required for Level 3; defer unless TCK demands.

## Project Constraints (from CLAUDE.md)

- **Backend:** Rust (primary language). All Phase 31 code is Rust in the `engine` crate under `engine/src/dmn/feel/`.
- **Production-grade, algorithm correctness.** Three-valued logic correctness (DMN-12) is the load-bearing correctness property — Pitfall 1.
- **Rule engine with Rete algorithm** (legacy path). Phase 31 MUST NOT regress the Rete path. The `FeelExpression` enum is distinct from the rule-engine `Expression` AST; FEEL is never routed through `EngineRegistry` / `engine/src/engine/`.
- **Full DMN Level 3 compliance** is the milestone goal. Phase 31 delivers the FEEL language surface; Phase 35 is the conformance gate.
- **Storage: Hybrid Git + PostgreSQL; Deployment: Docker, Kubernetes** — not Phase 31's concern (Phase 33 wires persistence/API).

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DMN-11 | FEEL lexer/parser — full grammar rules 1–65; dual entry points (`SimpleUnaryTests`, `Expression`); keyword-as-name disambiguation; hand-written recursive descent with Pratt precedence | Grammar rules 1–65 quoted verbatim from OMG plain-text spec (§10.3.1); parser-method mapping table + precedence ladder + dual-entry-point + keyword-disambiguation rules below. `FeelExpression` AST variant set recommended. |
| DMN-12 | FEEL evaluator with three-valued logic — `feel_equals`/`feel_compare` return `Option<_>` (null → `Value::Null`); explicit truth tables for `and`/`or`/`not`; conformance cases | Spec rule 49.a quoted verbatim (`e = null` → `FEEL(e) is null`); null-propagation truth tables for `and`/`or`/`not`/`=`/`!=`/`<`/`<=`/`>`/`>=` tabulated; `feel_equals`/`feel_compare` signatures + the private-module guard against Rust `==`/`Ord` reuse. |
| DMN-13 | FEEL operators — arithmetic, comparison, boolean, string concatenation, date-time arithmetic and duration operations | Operator-by-operator semantics + the date-time arithmetic matrix (date+ymd→date, dateTime+dtd→dateTime, time+dtd→time, date−date→dtd, dateTime−dateTime→dtd, duration±duration→duration) + Decimal128 division/overflow → null + string-concat-via-`+` + `between`/`in` desugaring. |
| DMN-14 | FEEL control + quantified expressions — `if`, `for`, `some`/`every`, path, filter, `instance of` | Scope semantics for `for` (iteration-variable binding via flat-Scope extension), quantified-expression scoping, path navigation (`.foo`, `[i]`, `.foo.bar`), filter `[?...]`, `instance of` against the Phase 30 `FeelType` lattice via `resolve_type_ref`/`type_of`/`conforms`. |
| DMN-15 | FEEL boxed expressions — all 10 types (literal, context, list, relation, function invocation, function definition, conditional, filter, for, every/some), recursive; boxed contexts evaluated in dependency (topological) order with cycle detection | 10 boxed-expression `FeelExpression` variants enumerated; flat-Scope dependency tracking + DFS post-order topological evaluation + DFS color-marking cycle detection specified. |
| DMN-16 | FEEL user-defined functions (lambdas, non-external) — grammar rule 55 | `FeelFunction` carrier extended with `body: Arc<FeelExpression>` + captured `Scope`; grammar rule 55 `function(formal param*) expression`; non-external constraint (external flag rejected). |
| DMN-17 | FEEL built-in function library — all ~70+ functions across conversion/string/list/numeric/date-time/context/boolean/range/equality families | Full ~70+ function enumeration by family with signatures + `HashMap<&str, BuiltinFn>` registry shape + null-propagation behavior + numeric-edge-case → null. |
| DMN-18 | FEEL AST cache — cells/expressions pre-parsed to `Arc<FeelExpression>` at DMN load time; evaluator takes `&FeelExpression` and never re-parses | `DashMap<String, Arc<FeelExpression>>` keying (cell source hash) + IR-wiring walk (`InputClause.input_expression`, `RuleEntry.text`, `OutputClause.default_output_entry`, `AllowedValues::UnaryTests`) + `evaluate(&FeelExpression, &Scope)` contract enforced at the type level. |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| FEEL lexing + parsing (text → AST) | `engine::dmn::feel::parser` (new) | `engine::dmn::feel::ast` (AST types) | The parser is the sole producer of `FeelExpression` from source text. |
| AST representation | `engine::dmn::feel::ast` (new) | — | A single enum is the user's locked choice; lives in its own module for readability. |
| Three-valued logic operators | `engine::dmn::feel::eval` (new, private `cmp` submodule) | `feel::numeric` (`feel_numeric_eq` template) | Equality/comparison are pure functions returning `Option<_>`; the private module makes accidental Rust `==`/`Ord` reuse a lint error. |
| Arithmetic / string-concat / date-time operators | `engine::dmn::feel::eval` + `feel::temporal` (extended) | `feel::coercion` (explicit FEEL coercions) | Numeric operators use `rust_decimal`; date-time operators reuse Phase 30's `add_months_zoned` / `add_days_time_zoned`. |
| Control + quantified + path + filter + `instance of` | `engine::dmn::feel::eval` | `feel::types` (`resolve_type_ref`/`conforms` for `instance of`) | `instance of` reuses the Phase 30 lattice; path/filter are dispatch variants on `FeelExpression`. |
| Boxed expressions (10 types) | `engine::dmn::feel::eval` (recursive `evaluate`) | `feel::ast` (the 10 variants) | Recursive evaluation dispatches over the enum; boxed-context dependency order uses the flat-Scope `deps` map. |
| Scope (flat map + explicit dependency tracking) | `engine::dmn::feel::eval` (Scope type) | — | Flat Scope is the user's locked choice; the `deps` map drives both QN resolution and boxed-context topological order. |
| User-defined lambdas | `engine::dmn::feel::types::FeelFunction` (extended) | `engine::dmn::feel::eval` (closure application) | The carrier gains `body: Arc<FeelExpression>` + captured `Scope`; the evaluator applies it. |
| Built-in function library (~70+) | `engine::dmn::feel::builtins` (new) | `feel::eval` (invocation dispatch) | A `HashMap<&str, BuiltinFn>` registry; invoked from `FeelExpression::FunctionInvocation`. |
| AST cache (load-time pre-parse) | `engine::dmn::feel::cache` (new, thin) or `feel::parser` (cache field) | `engine::dmn::parser` (IR loader wiring) | `DashMap<String, Arc<FeelExpression>>` keyed by cell source; populated during IR load. |
| Numeric value domain | `engine::data_types::Value::Number` (Phase 30) | `feel::numeric` | Phase 30's `Decimal` backing is reused; Phase 31 adds arithmetic operators. |
| Temporal value domain + arithmetic | `engine::dmn::feel::temporal` (Phase 30, extended) | — | `add_months_zoned`/`add_days_time_zoned` already present; Phase 31 wires them into the `+`/`-` operators. |
| Type lattice (for `instance of`) | `engine::dmn::feel::types` (Phase 30) | — | `resolve_type_ref`/`type_of`/`conforms` consumed as-is. |
| Rete path (legacy) | `engine::engine::*` (unchanged) | — | Zero regression — the FEEL path never reaches it. |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rust_decimal` | 1.42 (in `engine/Cargo.toml`, features `serde`/`macros`/`maths`) | FEEL Decimal128 numeric domain; arithmetic operators (`+ - * / **`); numeric built-ins (`abs`, `sqrt`, `log`, `exp`, `floor`, `ceiling`, `round *`, `decimal`, `modulo`) | Decimal (not binary float) — `0.1+0.2=0.3` holds. `maths` feature provides `sqrt`/`ln`/`exp` for DMN-17 numeric built-ins. Supply-chain verified 2026-07-06. [VERIFIED: Cargo.toml + docs.rs] |
| `chrono` | 0.4 (in `engine/Cargo.toml`, `serde` feature) | Date-time arithmetic: `date + yearsMonthsDuration`, `dateTime + daysTimeDuration`, `time + daysTimeDuration`, date−date → duration; `checked_add_months` for ymd arithmetic | `checked_add_months(Months::new(n))` gives spec-correct Jan 31 + 1 month → Feb 28/29. `TimeDelta` for days-time durations. [VERIFIED: Cargo.toml + docs.rs] |
| `chrono-tz` | 0.10.4 (in `engine/Cargo.toml`, `serde` feature) | IANA-zone datetime arithmetic + DST gap/overlap handling | `LocalResult::single/earliest` resolves DST; the `@Australia/Melbourne` zone is preserved through arithmetic (Phase 30 Pitfall 3 mitigation). [VERIFIED: Cargo.toml] |
| `dashmap` | 7.0.0-rc2 (NOT yet in deps — must be added) | AST cache: `DashMap<String, Arc<FeelExpression>>` for concurrent load-time pre-parse + read-time lookup | Industry-standard concurrent HashMap for Rust. Already used in Phase 27 (`SessionManager` `Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>>`) — consistent with the v5.0 stack. [VERIFIED: cargo search + package-legitimacy gate OK; repo xacrimon/dashmap, ~5M downloads/wk, since 2019] |
| `regex` | 1.12.4 (NOT yet in deps — must be added) | FEEL string built-ins: `replace`, `matches`, `split` (rule 49 / §10.3.4.2.2) | FEEL regex is specified as XQuery 1.0 / XPath 2.0 FO regex. The Rust `regex` crate is a pragmatic subset — it lacks a few XQuery features (e.g. some Unicode class shorthands) but covers all TCK vector patterns inspected. Flag any TCK divergence for Phase 35. [VERIFIED: cargo search + package-legitimacy gate OK; repo rust-lang/regex, ~15M downloads/wk, since 2014] |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `proptest` | 1.11.0 (workspace dev-dep) | Property tests with regression seeds (three-valued logic truth tables, Decimal128 invariants, operator conformance) | Consistent with Phase 30-04 pattern; commit seeds. [VERIFIED: workspace Cargo.toml] |
| `bigdecimal` | 0.4.10 (NOT added — deferred fallback) | Arbitrary-precision decimal for rounding built-ins at scale > 28 | **DO NOT ADD in Phase 31** unless a TCK `string(round up(x, 6176))` vector proves scale-preservation demand. Phase 30 research Open Question 1 confirmed no value-domain vector needs >29 digits; the rounding built-ins cap scale at 28 and rely on numeric equality (5.5 == 5.5000…0). Revisit only at Phase 35 if a TCK vector fails. [CITED: 30-RESEARCH.md Pitfall 2] |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-written recursive-descent + Pratt | `pest` / `lalrpop` parser generator | The roadmap locked "hand-written recursive descent" (v6.0 scope lock). A generator would obscure the keyword-disambiguation logic and error-recovery-with-spans behavior the Phase 34 editor needs. Pest/lalrpop also add a build step and a grammar file to maintain. Hand-written gives full control over error recovery. |
| `regex` crate (Rust) | `fancy-regex` (backtracking, full XQuery regex) | `fancy-regex` supports backreferences and lookaround (which XQuery regex does NOT have). The Rust `regex` crate covers the XQuery 1.0 subset that TCK vectors exercise; `fancy-regex` is slower and adds a dep. Use `regex`; only revisit if a TCK vector needs a feature `regex` lacks. |
| `HashMap<&str, BuiltinFn>` registry | Nested `match` / trait objects per family | Locked in CONTEXT.md: registry table scales to ~70 entries; a nested `match` becomes unreadable; trait objects per family over-engineer a closed, flat function space. |
| `DashMap` for the AST cache | `RwLock<HashMap>` | `DashMap` is sharded (lock-free reads of cached ASTs after load), consistent with the v5.0 `SessionManager` pattern, and the user's locked choice (`DashMap<String, Arc<FeelExpression>>`). |

**Installation:**
```bash
# Two new deps Phase 31 must add to engine/Cargo.toml:
cargo add dashmap --package engine
cargo add regex --package engine
# rust_decimal, chrono, chrono-tz, proptest are already present.
```

**Version verification (run during planning Wave 0):**
```bash
cargo search dashmap    # confirmed 7.0.0-rc2 (latest)
cargo search regex      # confirmed 1.12.4 (latest)
cargo search bigdecimal # confirmed 0.4.10 (deferred — do NOT add unless TCK demands)
```

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `dashmap` | crates.io | ~7 yrs (since 2019-08-25) | ~5.1M/wk | github.com/xacrimon/dashmap | OK | Approved — add to `engine/Cargo.toml` |
| `regex` | crates.io | ~12 yrs (since 2014-12-13) | ~14.9M/wk | github.com/rust-lang/regex | OK | Approved — add to `engine/Cargo.toml` |
| `rust_decimal` | crates.io | ~8 yrs | ~115M | github.com/paupino/rust-decimal | OK | Already in Cargo.toml (Phase 30) |
| `chrono` | crates.io | ~10 yrs | hundreds of M | github.com/chronotope/chrono | OK | Already in Cargo.toml |
| `chrono-tz` | crates.io | ~9 yrs | tens of M | github.com/chronotope/chrono-tz | OK | Already in Cargo.toml (Phase 30) |
| `bigdecimal` | crates.io | ~7 yrs | tens of M | github.com/akubera/bigdecimal-rs | OK (not added) | DEFERRED — only if Phase 35 TCK demands scale > 28 |
| `proptest` | crates.io | — | — | — | OK | Already a workspace dev-dep |

**Packages removed due to SLOP verdict:** none
**Packages flagged as suspicious [SUS]:** none

*Both new crates (`dashmap`, `regex`) were confirmed via `cargo search` (registry existence) and the `gsd-tools query package-legitimacy check` gate returned `OK` for both. `dashmap` is already used in the v5.0 codebase (`SessionManager`), so the supply-chain trust is established. Planner runs the gate again at Wave 0 to record the verdict in VALIDATION.md.*

## Architecture Patterns

### System Architecture Diagram

```
              .dmn XML (Phase 29 IR: InputClause.input_expression, RuleEntry.text,
                        OutputClause.default_output_entry, AllowedValues::UnaryTests)
                              │
                              ▼  (DMN-18 load-time pre-parse)
              ┌───────────────────────────────────────────────┐
              │  FeelAstCache = DashMap<String, Arc<FeelExpression>>  │
              │  key = hash(cell source text)                         │
              │  value = Arc<FeelExpression> (parsed once, reused)   │
              └──────────────┬────────────────────────────────────────┘
                             │ Arc<FeelExpression> (borrowed)
                             ▼
    ┌────────────────────────────────────────────────────────────┐
    │  feel::parser   (DMN-11)                                    │
    │  Lexer: tokens (names, numbers, strings, @"...", punct)    │
    │  parse_simple_unary_tests(&str) -> FeelExpression (entry 1)│
    │  parse_expression(&str)         -> FeelExpression (entry 2)│
    │  Pratt precedence: or < and < cmp < + - < * / < ** < -     │
    │                  < path/filter/invocation < primary          │
    │  Error recovery: FeelExpression::Err(span, msg) nodes       │
    └──────────────┬─────────────────────────────────────────────┘
                   │ FeelExpression (single enum, all variants)
                   ▼
    ┌────────────────────────────────────────────────────────────┐
    │  feel::ast  (DMN-11)                                        │
    │  enum FeelExpression {                                      │
    │    Literal(Value), Name(String), QualifiedName(Vec<String>), │
    │    AtLiteral(String), Range { start, end, s_incl, e_incl }, │
    │    BinOp { op, lhs, rhs }, Neg(Box),                        │
    │    If { cond, then, else_ }, For { iters, body },           │
    │    Quantified { kind, iters, satisfies },                    │
    │    Path { target, name }, Filter { target, pred },          │
    │    InstanceOf { expr, ty: FeelType },                        │
    │    // 10 boxed-expression variants:                          │
    │    List(Vec), Relation(cols, rows),                          │
    │    Context(Vec<(String, FeelExpression)>),                   │
    │    FunctionInvocation { callee, args, named },              │
    │    FunctionDefinition { params, body, external },            │
    │    Conditional { cond, then, else_ },  (= If, or shared)    │
    │    // error recovery:                                        │
    │    Err(Span, String),                                        │
    │  }                                                           │
    └──────────────┬─────────────────────────────────────────────┘
                   │ &FeelExpression (borrowed — never &str)
                   ▼
    ┌────────────────────────────────────────────────────────────┐
    │  feel::eval  (DMN-12/13/14/15/16)                           │
    │  evaluate(&FeelExpression, &Scope) -> Value                 │
    │  Scope = { bindings: HashMap<String, Value>,                 │
    │            deps: HashMap<String, Vec<String>> }  (FLAT)      │
    │  ┌─ private cmp module ─────────────────────────────────┐    │
    │  │ feel_equals(&Value, &Value) -> Option<bool>          │   │
    │  │ feel_compare(&Value, &Value) -> Option<Ordering>     │   │
    │  │ and_3vl(Option<bool>, Option<bool>) -> Option<bool>   │   │
    │  │ or_3vl,  not_3vl  (explicit truth tables)            │   │
    │  └──────────────────────────────────────────────────────┘    │
    │  arithmetic: +,-,*,/,**  (Decimal128; div/overflow → null)  │
    │  string concat: + (String+String)                           │
    │  date-time: reuse feel::temporal add_months/add_days_time  │
    │  boxed-context: flat-Scope deps → DFS post-order topo sort  │
    │                  + DFS color-marking cycle detection        │
    └──────────────┬─────────────────────────────────────────────┘
                   │ FunctionInvocation dispatch
                   ▼
    ┌────────────────────────────────────────────────────────────┐
    │  feel::builtins  (DMN-17)                                   │
    │  BuiltinFn = fn(&[Value]) -> Value  (or closure)            │
    │  REGISTRY: HashMap<&str, BuiltinFn> (~70+ entries)          │
    │  Families: conversion/string/list/numeric/date-time/        │
    │            context/boolean/range/equality                   │
    │  Null-propagating: null arg → Value::Null (unless spec      │
    │                    says otherwise, e.g. count([null]) = 1)  │
    └────────────────────────────────────────────────────────────┘
```

**Data flow:** a `.dmn` file loads via Phase 29's IR parser (unchanged) → DMN-18 walks the IR and pre-parses every FEEL cell into `Arc<FeelExpression>` stored in the `DashMap` cache → at evaluation time (Phase 32/33), the decision-table engine / decision evaluator calls `evaluate(&FeelExpression, &Scope)` with the *borrowed* cached AST and a flat `Scope` of input bindings → the evaluator dispatches on `FeelExpression` variants, calling `feel_equals`/`feel_compare` (three-valued) for comparisons, `feel::temporal` helpers for date-time arithmetic, `feel::builtins` for function invocations, and the flat-Scope `deps` map for boxed-context topological evaluation → returns a `Value`. The Rete path (`engine/src/engine/`) is untouched.

### Recommended Project Structure
```
engine/src/
├── data_types.rs          # Value enum (Phase 30, unchanged — the evaluator's value type)
├── dmn/
│   ├── mod.rs             # re-export feel module
│   ├── feel/
│   │   ├── mod.rs         # pub mod ast, parser, eval, builtins, coercion, numeric, temporal, types, cache
│   │   ├── ast.rs         # FeelExpression enum + Span + QuantKind (NEW, DMN-11)
│   │   ├── parser.rs      # Lexer + recursive-descent/Pratt parser, dual entry points (NEW, DMN-11)
│   │   ├── eval.rs        # evaluate(&FeelExpression, &Scope) + feel_equals/feel_compare + truth tables (NEW, DMN-12/13/14/15)
│   │   ├── builtins.rs    # BuiltinFn + REGISTRY HashMap + ~70+ fns (NEW, DMN-17)
│   │   ├── coercion.rs    # explicit FEEL coercions for operators/builtins (NEW, DMN-13)
│   │   ├── cache.rs       # FeelAstCache = DashMap<String, Arc<FeelExpression>> (NEW, DMN-18; or fold into parser.rs)
│   │   ├── numeric.rs     # feel_numeric_eq (Phase 30, reused)
│   │   ├── temporal.rs     # parse_at_literal + add_months_zoned + add_days_time_zoned (Phase 30, extended for DMN-13)
│   │   └── types.rs       # FeelType + FeelFunction (EXTENDED for DMN-16 lambda body) + lattice (Phase 30)
│   ├── model.rs           # Phase 29 IR — UNCHANGED (read-only consumer)
│   └── parser.rs          # Phase 29 XML loader — extended for DMN-18 cache wiring (pre-parse walk)
└── engine/                # Rete path — UNCHANGED (zero regression)
```

### Pattern 1: Hand-written recursive-descent + Pratt precedence (DMN-11)
**What:** A hand-written lexer produces a `Vec<Token>` (with spans); a recursive-descent parser handles statement-level constructs (`for`/`if`/`some`/`every`/`instance of`/boxed expressions) and delegates the binary-operator ladder to a Pratt loop keyed by binding power.
**When to use:** Always for this phase — the roadmap locks "hand-written recursive descent with Pratt precedence."
**Precedence ladder (spec rule 2 + rule 4, lowest → highest):**
```
or          (rule 47)   binding power 10
and         (rule 48)   binding power 20
comparison  (rule 49)   binding power 30  (= != < <= > >= between in)
additive    (rule 19/20) binding power 40  (+ -)  left-assoc
mult        (rule 21/22) binding power 50  (* /)  left-assoc
exponent    (rule 23)    binding power 60  (**)   RIGHT-assoc  ← common gotcha
unary neg   (rule 24)    binding power 70  (-)
postfix     (rule 43/50/38) binding power 80  (. path, [filter], ( invocation))
primary     (rule 17/31/32/54/...) binding power 90  (literal, name, ( expr ), list, context, function-def)
```
**Key:** `**` is RIGHT-associative (`2 ** 3 ** 2 = 2 ** 9 = 512`, not `8 ** 2 = 64`); all other binary ops are left-associative. The Pratt loop recurses with `min_bp` so `2 ** 3 ** 2` parses as `2 ** (3 ** 2)`.

### Pattern 2: Three-valued-logic via `Option<bool>` + private comparison module (DMN-12)
**What:** All FEEL boolean/comparison operators produce `Option<bool>` / `Option<Ordering>` (`None` = null). The evaluator maps `None` → `Value::Null` and `Some(b)` → `Value::Bool(b)`. The comparison functions live in a **private** `feel::eval::cmp` module that does NOT re-export `PartialEq`/`PartialOrd` — making accidental `a == b` reuse at evaluator call-sites a "method not found" or lint error.
**When to use:** Always — this is Pitfall 1's mitigation.
**Example:**
```rust
// Source: DMN 1.5 spec rule 49.a (verbatim) + 30-RESEARCH.md feel_numeric_eq template
//
// "e = null"  maps to "FEEL(e) is null"
// "e != null" maps to "FEEL(e) is not null"
// "e1 = e2"  requires e1, e2 same kind; else null (type mismatch → null, not false)
// "null = null" → true (both are null)
// "not(null)" → null (negation table: "otherwise" → null)

mod cmp {
    use crate::data_types::Value;
    use std::cmp::Ordering;

    /// FEEL equality — Option<bool> (null-propagating). NEVER reuse Rust ==.
    pub fn feel_equals(a: &Value, b: &Value) -> Option<bool> {
        // Rule 49.a: null = null → true; null = e → FEEL(e) is null (i.e. false when e non-null).
        match (a, b) {
            (Value::Null, Value::Null) => Some(true),
            (Value::Null, _) | (_, Value::Null) => Some(false), // e = null → "FEEL(e) is null"
            (Value::Number(x), Value::Number(y)) => Some(x == y),
            (Value::Number(x), Value::Integer(i)) | (Value::Integer(i), Value::Number(x)) => {
                Some(*x == rust_decimal::Decimal::from(*i))
            }
            (Value::String(x), Value::String(y)) => Some(x == y),
            (Value::Bool(x), Value::Bool(y)) => Some(x == y),
            (Value::Date(x), Value::Date(y)) => Some(x == y),
            (Value::Time(x), Value::Time(y)) => Some(x == y),
            (Value::DateTimeTz(x), Value::DateTimeTz(y)) => Some(x == y),
            (Value::DaysTimeDuration(x), Value::DaysTimeDuration(y)) => Some(x == y),
            (Value::YearsMonthsDuration(x), Value::YearsMonthsDuration(y)) => Some(x == y),
            (Value::Date(d), Value::DateTimeTz(dtv)) | (Value::DateTimeTz(dtv), Value::Date(d)) => {
                // §10.3.2.3.5: a date ≡ a datetime at UTC midnight on that date.
                date_equals_datetime_z(d, dtv)
            }
            (Value::Array(x), Value::Array(y)) => list_equals(x, y), // element-wise
            (Value::Object(x), Value::Object(y)) => context_equals(x, y),
            // Type mismatch ("a" = 1, true = 1, etc.) → None (null), NOT false.
            _ => None,
        }
    }

    /// FEEL ordering — Option<Ordering> (null-propagating). NEVER reuse Rust Ord.
    pub fn feel_compare(a: &Value, b: &Value) -> Option<Ordering> {
        match (a, b) {
            (Value::Null, _) | (_, Value::Null) => None, // null < 3 → null
            (Value::Number(x), Value::Number(y)) => x.partial_cmp(y),
            (Value::Number(x), Value::Integer(i)) | (Value::Integer(i), Value::Number(x)) => {
                rust_decimal::Decimal::from(*i).partial_cmp(x)
            }
            (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
            (Value::Date(x), Value::Date(y)) => Some(x.cmp(y)),
            (Value::Time(x), Value::Time(y)) => time_cmp(x, y),
            (Value::DateTimeTz(x), Value::DateTimeTz(y)) => datetime_cmp(x, y),
            (Value::DaysTimeDuration(x), Value::DaysTimeDuration(y)) => x.partial_cmp(y),
            (Value::YearsMonthsDuration(x), Value::YearsMonthsDuration(y)) => Some(x.months.cmp(&y.months)),
            // Type mismatch or unordered types → None.
            _ => None,
        }
    }

    /// Explicit three-valued AND. false is dominant; null propagates otherwise.
    pub fn and_3vl(a: Option<bool>, b: Option<bool>) -> Option<bool> {
        match (a, b) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None, // null present, no false
        }
    }

    /// Explicit three-valued OR. true is dominant; null propagates otherwise.
    pub fn or_3vl(a: Option<bool>, b: Option<bool>) -> Option<bool> {
        match (a, b) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None, // null present, no true
        }
    }

    /// Explicit three-valued NOT. null → null; else negate.
    pub fn not_3vl(a: Option<bool>) -> Option<bool> {
        match a {
            Some(true) => Some(false),
            Some(false) => Some(true),
            None => None, // not(null) → null
        }
    }
}
```

### Pattern 3: Flat Scope + explicit dependency tracking (DMN-14 / DMN-15, user override)
**What:** A `Scope` is a flat `HashMap<String, Value>` of bindings plus a `deps: HashMap<String, Vec<String>>` recording which names each binding depends on. Nested scope resolution is **not** a parent-chain walk — it is an explicit dependency lookup. The same `deps` map drives boxed-context topological evaluation.
**When to use:** Always for this phase — the user overrode the chained-Scope option.
**Example:**
```rust
// Source: CONTEXT.md Grey Area 4 (user override: flat Scope + explicit deps)
use crate::data_types::Value;
use std::collections::HashMap;

/// Flat FEEL evaluation scope. A context is a flat map; nested resolution
/// uses the `deps` adjacency map (NOT a parent chain).
pub struct Scope {
    /// Name → value bindings (input data, decision results, iteration vars).
    pub bindings: HashMap<String, Value>,
    /// Name → names it depends on (drives boxed-context topo sort + QN resolution).
    pub deps: HashMap<String, Vec<String>>,
}

impl Scope {
    pub fn get(&self, name: &str) -> Option<&Value> { self.bindings.get(name) }
    pub fn bind(&mut self, name: String, v: Value, deps: Vec<String>) {
        self.bindings.insert(name, v);
        self.deps.insert(name, deps);
    }
    /// Resolve a qualified name `a.b.c` against the flat scope: look up `a`,
    /// then path-navigate `.b.c`. (Longest-match QN resolution, ROADMAP note.)
    pub fn resolve_qn(&self, qn: &[String]) -> Value {
        // Try the longest prefix that resolves, then path-navigate the rest.
        for i in (1..=qn.len()).rev() {
            if let Some(v) = self.bindings.get(&qn[..i].join(".")) {
                return qn[i..].iter().fold(v.clone(), path_nav);
            }
        }
        Value::Null
    }
}

/// Boxed-context topological evaluation with DFS cycle detection (DMN-15).
///
/// Returns a map of evaluated entries in dependency order. If a cycle is
/// detected, the cyclic entries evaluate to Value::Null and an error is
/// recorded (carrying source location per CONTEXT.md eval-error propagation).
pub fn eval_context_topo(
    entries: &[(String, FeelExpression)],
    scope: &Scope,
) -> Result<HashMap<String, Value>, ContextCycleError> {
    // 1. Build the dependency graph: for each entry, free-vars of its expr
    //    that are also entry keys → edges.
    // 2. DFS color-marking (White/Gray/Black) cycle detection; Gray-on-Gray = cycle.
    // 3. DFS post-order gives topological order.
    // 4. Evaluate in that order, binding each result into the working scope.
    //    A cyclic entry → Value::Null + error recorded.
}
```

### Pattern 4: AST cache keyed by cell source hash (DMN-18)
**What:** A `DashMap<String, Arc<FeelExpression>>` keyed by a canonical hash of the cell source text. The DMN IR loader walks every FEEL-bearing IR field and populates the cache at load time. The evaluator takes `&FeelExpression` — it is *impossible* to call it with raw source text, enforcing the no-re-parse contract at the type level.
**When to use:** Always — this is the headline performance property.
**Example:**
```rust
// Source: CONTEXT.md locked decisions (DMN-18) + Phase 29 IR shape
use dashmap::DashMap;
use std::sync::Arc;

pub struct FeelAstCache {
    map: DashMap<String, Arc<FeelExpression>>,
}

impl FeelAstCache {
    pub fn get_or_parse(&self, src: &str, entry: ParseEntry) -> Arc<FeelExpression> {
        let key = cell_key(src); // canonical hash; preserves source for error spans
        self.map.entry(key).or_insert_with(|| {
            Arc::new(match entry {
                ParseEntry::Expression => parse_expression(src),
                ParseEntry::SimpleUnaryTests => parse_simple_unary_tests(src),
            })
        }).clone()
    }
}

pub enum ParseEntry { Expression, SimpleUnaryTests }

/// The evaluator takes a BORROWED AST — never &str. This is the type-level
/// contract that prevents the 4–6× re-parse performance bug.
pub fn evaluate(expr: &FeelExpression, scope: &Scope) -> Value { /* dispatch */ }
```

### Anti-Patterns to Avoid
- **Reusing Rust `==` / `Ord` / `compare_values` for FEEL equality/ordering.** This is Pitfall 1 — the single most likely source of silent three-valued-logic bugs. The mitigation is a private `feel::eval::cmp` module + the `feel_equals`/`feel_compare` → `Option<_>` signature. The evaluator MUST call these, never `a == b`.
- **Routing FEEL through the Rete `Expression` AST or `EngineRegistry`.** The `FeelExpression` enum is distinct (CONTEXT.md locked). Mixing them regresses v1.0–v5.0 rule execution and violates the stateless/three-valued vs forward-chaining/two-valued separation.
- **Using `f64` for FEEL numbers.** Phase 30 Anti-Pattern — `0.1_f64 + 0.2_f64 = 0.30000000000000004`. Always `Value::Number(Decimal)`.
- **Implicit number↔string conversion in operators.** `1 + "2"` yields `null` (Phase 30 Pitfall 6). number↔string is the explicit `string()`/`number()` built-ins, NOT a lattice or operator coercion.
- **Left-associating `**`.** `2 ** 3 ** 2` must parse as `2 ** (3 ** 2) = 512`, not `(2 ** 3) ** 2 = 64`. Right-associative in the Pratt loop.
- **`unwrap()` on `Option`/`Result` from untrusted FEEL text.** Division by zero, overflow, malformed `@""`, DST gaps → `Value::Null`, never panic (Phase 30 Pitfall 5).
- **Fail-fast parsing on a single bad cell.** The user chose error-recovery producing a partial AST (`Err` nodes carry spans) so the Phase 34 editor can show per-cell errors while the rest of the table loads. Do NOT `?`-propagate a parse error out of a cell — emit `FeelExpression::Err(span, msg)`.
- **Building a parent/child Scope chain.** The user overrode this — flat Scope + explicit dependency tracking is the locked model.
- **Marking `Value` or `FeelExpression` `#[non_exhaustive]`.** Exhaustive match is the safety net that catches missing arms at consumer sites (Phase 30 Pattern 1).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Decimal128 arithmetic (`+ - * / **`) | Fixed-point / `f64` | `rust_decimal::Decimal` operators + `checked_div` | Decimal semantics; `maths` feature for `sqrt`/`ln`/`exp`. Div-by-zero → `Err` → map to `Value::Null`. |
| Regex (FEEL `replace`/`matches`/`split`) | Hand-rolled matcher | `regex` crate | FEEL regex = XQuery 1.0 subset; `regex` covers it. Battle-tested, no ReDoS (bounded execution). |
| Concurrent AST cache | `RwLock<HashMap>` | `DashMap<String, Arc<FeelExpression>>` | Sharded, lock-free reads after load; consistent with v5.0 `SessionManager`. |
| Date-time arithmetic (DST-correct) | `seconds + 30*86400` | Phase 30 `feel::temporal::add_months_zoned` / `add_days_time_zoned` | Already handle Jan 31 + 1 month → Feb 28/29 and DST gap→null/overlap→earliest. |
| Topological sort + cycle detection | Custom graph lib | DFS post-order + DFS color-marking (White/Gray/Black) | The dependency graph is small (a boxed context's entries); a 30-line DFS is clearer than a dep. |
| Pratt precedence parsing | Operator-precedence table generator | Hand-written Pratt loop | The grammar is small (65 rules); a generator adds a build step and obscures error-recovery. |
| `instance of` type checking | Custom type-compat table | Phase 30 `feel::types::conforms` + `type_of` + `resolve_type_ref` | The lattice is already implemented and property-tested. |
| `@""` literal parsing | New tokenizer | Phase 30 `feel::temporal::parse_at_literal` | Already dispatches by first separator; handles all 5 temporal types. |

**Key insight:** Phase 31 is largely an *integration* of Phase 30's value domain + lattice + temporal helpers into a parser/evaluator shell. The genuinely new code is the grammar, the three-valued-logic operators, the boxed-context topological evaluation, and the built-in function bodies — everything else is dispatch over `FeelExpression` variants.

## Runtime State Inventory

> Phase 31 is a greenfield additive extension (new modules under `engine/src/dmn/feel/`). No rename/migration. This section is included for completeness.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — no persisted records reference `FeelExpression` ASTs yet (Phase 33 persists evaluated `Value`s, not ASTs). | None |
| Live service config | None — no running services depend on the FEEL parser/evaluator yet (Phase 33 wires the API). | None |
| OS-registered state | None | None |
| Secrets/env vars | None | None |
| Build artifacts | `engine/target/` will recompile on `cargo add dashmap regex`. No stale artifacts. | `cargo build` after adding deps. |

## Common Pitfalls

### Pitfall 1: Three-valued-logic leak (ROADMAP-flagged, DMN-12)
**What goes wrong:** A FEEL comparison `a = b` is implemented as `a == b` in Rust, which yields `false` (not `null`) for type-mismatched operands like `"a" = 1`, and panics or yields `false` for `null` operands. This silently violates the DMN-12 conformance cases.
**Why it happens:** Rust's `PartialEq` is two-valued; FEEL is three-valued. `Value` derives `PartialEq`, so `a == b` compiles and looks correct.
**How to avoid:** (a) Put all FEEL comparison/equality in a **private** `feel::eval::cmp` module; do NOT re-export `PartialEq`/`PartialOrd` from it. (b) The evaluator calls `feel_equals`/`feel_compare` (returning `Option<_>`), never `==`/`Ord`. (c) Add a `#[deny(clippy::eq_op)]`-style lint or a unit-test that asserts `"a" = 1` → `Value::Null`, `null = null` → `Value::Bool(true)`, `null < 3` → `Value::Null`, `not(null)` → `Value::Null`. (d) Make `Value`'s derived `PartialEq` unreachable from the `feel::eval` module via visibility, OR add a `compile_error!`-guarded test that the evaluator module never imports `PartialEq` on `Value`.
**Warning signs:** A FEEL equality test returns `Value::Bool(false)` where the spec demands `Value::Null`. A `match` on `Value` in `feel::eval` that ends with `_ => false` instead of `_ => None`.

### Pitfall 2: `**` parsed left-associative (DMN-11)
**What goes wrong:** `2 ** 3 ** 2` evaluates to `64` instead of `512`.
**Why it happens:** The naive Pratt loop treats all binary ops as left-associative.
**How to avoid:** Give `**` right-associative binding power (recurse with `min_bp = bp` rather than `bp + 1` in the right operand). Add a unit test: `assert_eq!(eval("2 ** 3 ** 2"), Value::Number(dec!(512)))`.
**Warning signs:** A `**` test with nested exponents fails.

### Pitfall 3: Keyword-as-name disambiguation (DMN-11)
**What goes wrong:** A context entry like `{ for: 1 }` or a function parameter named `between` fails to parse, or `x in [1..10]` is mis-parsed as a quantified-expression `in` clause.
**Why it happens:** FEEL reserves keywords (`for`, `if`, `some`, `every`, `in`, `between`, `and`, `or`, `return`, `function`, `instance`, `of`, `true`, `false`, `null`) BUT also allows them as names in certain positions (grammar rule 25 `name` is permissive; rule 18 `qualified name = name, { ".", name }`).
**How to avoid:** The lexer emits a `Token::Name` for any identifier matching rule 25's character class, INCLUDING keyword-shaped strings. The parser disambiguates by *position*: at the start of a statement, `for`/`if`/`some`/`every`/`function` are keywords; after a `.` (path) or `:` (context entry key) or `(` (parameter list), they are names. Maintain a `is_keyword_in_position(token, position) -> bool` table. Add round-trip tests for `{ for: 1, if: 2 }` and `function(for, in, between) for + in + between`.
**Warning signs:** A context with a keyword key fails to parse.

### Pitfall 4: `null = null` → `true` vs `null = e` → `false` (DMN-12)
**What goes wrong:** The evaluator treats all `null`-involving equalities as `null` (unknown), missing the spec rule 49.a special case that `e = null` maps to `FEEL(e) is null` (a definite `true`/`false`, not `null`).
**Why it happens:** A naive "any null operand → null" rule is wrong for `=`/`!=`. The spec is precise: `null = null` → `true`; `5 = null` → `false` (because `FEEL(5) is null` is false); only `5 = "a"` (type mismatch) → `null`.
**How to avoid:** Implement `feel_equals` with the three explicit null arms (Pattern 2 above): `(Null, Null) → Some(true)`, `(Null, _) | (_, Null) → Some(false)`, then the same-kind arms, then `_ → None` for type mismatch. Add mandatory unit tests: `null = null` → `Bool(true)`, `5 = null` → `Bool(false)`, `"a" = 1` → `Null`.
**Warning signs:** A test asserts `null = null` → `Null` (wrong — it's `true`).

### Pitfall 5: Boxed-context cycle non-detection (DMN-15)
**What goes wrong:** A boxed context `{ a: b + 1, b: a + 1 }` (a and b depend on each other) evaluates to infinite recursion or a stack overflow.
**Why it happens:** No cycle detection in the dependency graph.
**How to avoid:** DFS color-marking (White = unvisited, Gray = on-stack, Black = done). A Gray-on-Gray edge is a cycle. On cycle detection, the cyclic entries evaluate to `Value::Null` and an error carrying source location is recorded (CONTEXT.md eval-error propagation). Add a unit test for the 2-cycle and a 3-cycle case.
**Warning signs:** A boxed-context test with cyclic dependencies hangs or overflows.

### Pitfall 6: `is` vs `=` confusion (DMN-12 / DMN-17)
**What goes wrong:** `is("a", 1)` is implemented to return `null` (like `=`), when the spec says `is` "never results in null" — `is("a", 1)` → `false`.
**Why it happens:** The `is` built-in and the `=` operator have different null semantics. `=` is three-valued; `is` is two-valued (identity, total).
**How to avoid:** Implement `is` as a separate built-in that returns `Value::Bool(true)` / `Value::Bool(false)` — never `Value::Null`. It checks structural identity across the FEEL semantic domain (including `is(null, null)` → `true`, `is("a", 1)` → `false`). Add a unit test: `is("a", 1)` → `Bool(false)`, `is(null, null)` → `Bool(true)`.
**Warning signs:** `is` returns `Value::Null` for any input.

### Pitfall 7: `regex` crate vs XQuery 1.0 regex dialect (DMN-17)
**What goes wrong:** A TCK `replace`/`matches`/`split` vector uses an XQuery 1.0 regex feature the Rust `regex` crate doesn't support (e.g. `\p{L}` Unicode category in some forms, or certain escape rules), causing a parse error or wrong match.
**Why it happens:** FEEL regex is specified as XQuery 1.0 / XPath 2.0 FO regex. The Rust `regex` crate covers most of it but is a subset.
**How to avoid:** Use `regex` for the implementation; document this as a known approximation. Flag any TCK divergence for Phase 35. Do NOT switch to `fancy-regex` unless a TCK vector demands it (it adds a backtracking engine and a dep). Add a Wave-0 task to scan the vendored TCK `replace`/`matches`/`split` vectors for patterns `regex` rejects.
**Warning signs:** A TCK regex vector fails with a `regex` parse error.

### Pitfall 8: AST cache keying on raw source (DMN-18)
**What goes wrong:** Two cells with identical text but different CDATA flags, or whitespace variants, cache-collide or cache-miss, causing either wrong-AST reuse or redundant re-parse.
**Why it happens:** The cache key must be canonical.
**How to avoid:** Key on a canonical hash of the *content* (post-CDATA-strip, post-trim) AND the entry-point kind (`Expression` vs `SimpleUnaryTests`) — the same text can parse differently under the two entry points. Use `format!("{entry:?}:{content}")` hashed. Add a unit test that two cells with the same content but different CDATA flags hit the same cache entry.
**Warning signs:** A cell parses twice, or a unary-test cell uses an `Expression` AST.

## Code Examples

### Three-valued-logic conformance cases (DMN-12 — mandatory unit tests)
```rust
// Source: DMN 1.5 spec rule 49.a + CONTEXT.md locked conformance cases.
// These are MANDATORY unit tests (CONTEXT.md: "the DMN-12 truth-table cases
// are mandatory unit tests").
#[test]
fn dmn12_a_equals_type_mismatch_yields_null() {
    // "a" = 1 → null (type mismatch, not false)
    let expr = parse_expression(r#""a" = 1"#);
    assert_eq!(evaluate(&expr, &Scope::empty()), Value::Null);
}

#[test]
fn dmn12_null_lt_three_yields_null() {
    // null < 3 → null
    let scope = Scope::empty(); // null is the value of any unbound name
    let expr = parse_expression("null < 3");
    assert_eq!(evaluate(&expr, &scope), Value::Null);
}

#[test]
fn dmn12_null_equals_null_yields_true() {
    // null = null → true (rule 49.a: e = null → FEEL(e) is null)
    let expr = parse_expression("null = null");
    assert_eq!(evaluate(&expr, &Scope::empty()), Value::Bool(true));
}

#[test]
fn dmn12_not_null_yields_null() {
    // not(null) → null (negation table: "otherwise" → null)
    let expr = parse_expression("not(null)");
    assert_eq!(evaluate(&expr, &Scope::empty()), Value::Null);
}

#[test]
fn dmn12_and_or_truth_tables() {
    // null and true → null; null or true → true; null and false → false; null or false → null.
    assert_eq!(evaluate(&parse_expression("null and true"), &Scope::empty()), Value::Null);
    assert_eq!(evaluate(&parse_expression("null or true"), &Scope::empty()), Value::Bool(true));
    assert_eq!(evaluate(&parse_expression("null and false"), &Scope::empty()), Value::Bool(false));
    assert_eq!(evaluate(&parse_expression("null or false"), &Scope::empty()), Value::Null);
}
```

### Date-time arithmetic (DMN-13)
```rust
// Source: DMN 1.5 §10.3.2.3.5 + Drools FEEL handbook arithmetic examples.
#[test]
fn date_plus_years_months_duration_yields_date() {
    // date + yearsMonthsDuration → date
    let expr = parse_expression(r#"date("2021-01-31") + duration("P1M")"#);
    let v = evaluate(&expr, &Scope::empty());
    // Jan 31 + 1 month → Feb 28 (last day of resulting month, spec-correct via checked_add_months)
    match v {
        Value::Date(d) => assert_eq!(d, chrono::NaiveDate::from_ymd_opt(2021, 2, 28).unwrap()),
        other => panic!("expected Date, got {:?}", other),
    }
}

#[test]
fn datetime_plus_days_time_duration_yields_datetime() {
    // date and time + daysTimeDuration → date and time
    let expr = parse_expression(r#"date and time("2021-01-01T10:00:00@Australia/Melbourne") + duration("P1D")"#);
    let v = evaluate(&expr, &Scope::empty());
    // IANA zone preserved (Phase 30 Pitfall 3 mitigation via add_days_time_zoned)
    match v {
        Value::DateTimeTz(crate::dmn::feel::temporal::DateTimeValue::Iana(dt)) => {
            assert_eq!(dt.timezone(), chrono_tz::Australia::Melbourne);
        }
        other => panic!("expected Iana datetime, got {:?}", other),
    }
}

#[test]
fn date_minus_date_yields_days_time_duration() {
    // date − date → days and time duration
    let expr = parse_expression(r#"date("2012-12-25") - date("2012-12-24")"#);
    let v = evaluate(&expr, &Scope::empty());
    match v {
        Value::DaysTimeDuration(td) => assert_eq!(td, chrono::TimeDelta::days(1)),
        other => panic!("expected DaysTimeDuration, got {:?}", other),
    }
}

#[test]
fn division_by_zero_yields_null() {
    // 1.0 / 0 → null (numeric edge case, CONTEXT.md locked: div by zero → null)
    let expr = parse_expression("1.0 / 0");
    assert_eq!(evaluate(&expr, &Scope::empty()), Value::Null);
}
```

### `instance of` against the Phase 30 lattice (DMN-14)
```rust
// Source: DMN 1.5 grammar rule 51 + Phase 30 feel::types::conforms.
#[test]
fn instance_of_checks_conformance() {
    // 5 instance of number → true
    assert_eq!(evaluate(&parse_expression("5 instance of number"), &Scope::empty()), Value::Bool(true));
    // 5 instance of string → false
    assert_eq!(evaluate(&parse_expression("5 instance of string"), &Scope::empty()), Value::Bool(false));
    // [1,2,3] instance of list<number> → true
    assert_eq!(evaluate(&parse_expression("[1,2,3] instance of list<number>"), &Scope::empty()), Value::Bool(true));
    // null instance of Any → true (Null <: Any)
    assert_eq!(evaluate(&parse_expression("null instance of Any"), &Scope::empty()), Value::Bool(true));
}
// Implementation: parse the `type` (rule 52) into a FeelType via
// feel::types::resolve_type_ref, then feel::types::conforms(type_of(value), target_type).
```

### Built-in function registry shape (DMN-17)
```rust
// Source: CONTEXT.md locked decision (HashMap<&str, BuiltinFn> registry).
use std::collections::HashMap;

/// A built-in function pointer. Null args propagate to Value::Null unless
/// the spec says otherwise (e.g. count([null]) = 1, string(null) = null).
pub type BuiltinFn = fn(&[Value]) -> Value;

pub fn builtin_registry() -> HashMap<&'static str, BuiltinFn> {
    let mut m = HashMap::new();
    // Conversion
    m.insert("date", date_built_in as BuiltinFn);
    m.insert("time", time_built_in);
    m.insert("date and time", datetime_built_in);
    m.insert("duration", duration_built_in);
    m.insert("number", number_built_in);
    m.insert("string", string_built_in);
    m.insert("years and months duration", ymd_built_in);
    // Numeric
    m.insert("decimal", decimal_built_in);
    m.insert("floor", floor_built_in);
    m.insert("ceiling", ceiling_built_in);
    m.insert("round up", round_up_built_in);
    m.insert("round down", round_down_built_in);
    m.insert("round half up", round_half_up_built_in);
    m.insert("round half down", round_half_down_built_in);
    m.insert("abs", abs_built_in);
    m.insert("modulo", modulo_built_in);
    m.insert("sqrt", sqrt_built_in);
    m.insert("log", log_built_in);
    m.insert("exp", exp_built_in);
    m.insert("odd", odd_built_in);
    m.insert("even", even_built_in);
    // String
    m.insert("substring", substring_built_in);
    m.insert("string length", string_length_built_in);
    m.insert("upper case", upper_case_built_in);
    m.insert("lower case", lower_case_built_in);
    m.insert("substring before", substring_before_built_in);
    m.insert("substring after", substring_after_built_in);
    m.insert("replace", replace_built_in); // uses `regex` crate
    m.insert("contains", contains_built_in);
    m.insert("starts with", starts_with_built_in);
    m.insert("ends with", ends_with_built_in);
    m.insert("matches", matches_built_in); // uses `regex` crate
    m.insert("split", split_built_in);     // uses `regex` crate
    m.insert("string join", string_join_built_in);
    // List
    m.insert("list contains", list_contains_built_in);
    m.insert("list replace", list_replace_built_in);
    m.insert("count", count_built_in);
    m.insert("min", min_built_in);
    m.insert("max", max_built_in);
    m.insert("sum", sum_built_in);
    m.insert("mean", mean_built_in);
    m.insert("all", all_built_in);
    m.insert("any", any_built_in);
    m.insert("sublist", sublist_built_in);
    m.insert("append", append_built_in);
    m.insert("concatenate", concatenate_built_in);
    m.insert("insert before", insert_before_built_in);
    m.insert("remove", remove_built_in);
    m.insert("reverse", reverse_built_in);
    m.insert("index of", index_of_built_in);
    m.insert("union", union_built_in);
    m.insert("distinct values", distinct_values_built_in);
    m.insert("flatten", flatten_built_in);
    m.insert("product", product_built_in);
    m.insert("median", median_built_in);
    m.insert("stddev", stddev_built_in);
    m.insert("mode", mode_built_in);
    m.insert("sort", sort_built_in);
    // Boolean
    m.insert("not", not_built_in); // not(null) → null
    // Context
    m.insert("context", context_built_in);
    m.insert("context put", context_put_built_in);
    m.insert("context merge", context_merge_built_in);
    m.insert("get value", get_value_built_in);
    m.insert("get entries", get_entries_built_in);
    // Temporal
    m.insert("now", now_built_in);
    m.insert("today", today_built_in);
    m.insert("day of year", day_of_year_built_in);
    m.insert("day of week", day_of_week_built_in);
    m.insert("month of year", month_of_year_built_in);
    m.insert("week of year", week_of_year_built_in);
    m.insert("is", is_built_in); // NEVER null (Pitfall 6)
    // Range
    m.insert("before", before_built_in);
    m.insert("after", after_built_in);
    m.insert("meets", meets_built_in);
    m.insert("met by", met_by_built_in);
    m.insert("overlaps", overlaps_built_in);
    m.insert("overlaps before", overlaps_before_built_in);
    m.insert("overlaps after", overlaps_after_built_in);
    m.insert("finishes", finishes_built_in);
    m.insert("finished by", finished_by_built_in);
    m.insert("includes", includes_built_in);
    m.insert("during", during_built_in);
    m.insert("starts", starts_built_in);
    m.insert("started by", started_by_built_in);
    m.insert("coincides", coincides_built_in);
    m
}
```

## FEEL Grammar 1–65 → Parser Method Mapping (DMN-11)

The OMG DMN 1.5 plain-text spec (§10.3.1) defines 65 grammar rules. The parser implements two entry points and a Pratt-precedence binary-operator ladder. The mapping below is prescriptive — the planner assigns each rule to a parser method.

### Dual entry points
- **`parse_simple_unary_tests(&str) -> FeelExpression`** (grammar rule 15 `unary tests`): for input-cell FEEL. Parses `positive unary tests` (rule 14, comma-separated list of rule-13 `positive unary test = expression`), the `not(...)` form, and the `-` (empty/any) form. Yields a `FeelExpression` representing the test (often a `Range`/`BinOp`/`Or`-chain over the input value).
- **`parse_expression(&str) -> FeelExpression`** (grammar rule 1 `expression`): for output-cell FEEL and all other expression contexts. Dispatches to `boxed expression` (rule 53) or `textual expression` (rule 2).

### Grammar-rule → parser-method table (verbatim rules, OMG plain-text spec)

| Rule | Production (verbatim) | Parser method | Notes |
|------|----------------------|---------------|-------|
| 1 | `expression = boxed expression \| textual expression` | `parse_expression` | Top entry; dispatch on first token (`[`/`{`/`function` → boxed, else textual). |
| 2 | `textual expression = for \| if \| quantified \| disjunction \| conjunction \| comparison \| arithmetic \| instance of \| path \| filter \| function invocation \| literal \| simple positive unary test \| name \| "(" expression ")"` | `parse_textual_expression` | **Precedence ladder** — alternatives listed lowest→highest. Pratt loop. |
| 3 | `textual expressions = textual expression, { "," textual expression }` | `parse_textual_expressions` | Comma-list (rare in cells). |
| 4 | `arithmetic expression = addition \| subtraction \| multiplication \| division \| exponentiation \| arithmetic negation` | `parse_arithmetic` (Pratt) | `+ -` < `* /` < `**` < unary `-`. |
| 7 | `simple positive unary test = ("<"\|"<="\|">"\|">=") , endpoint \| interval` | `parse_simple_positive_unary_test` | Input-cell test; endpoint is an expression (rule 16). |
| 8–12 | `interval = (open\|closed start), endpoint, "..", endpoint, (open\|closed end)` | `parse_interval` | Range syntax `[a..b]`, `(a..b]`, etc. Open-start can be `(` OR `]`; open-end `)` OR `[`. |
| 13 | `positive unary test = expression` | delegate to `parse_expression` | A unary test can be any expression (evaluated against the input value). |
| 15 | `unary tests = positive unary tests \| "not" "(" positive unary tests ")" \| "-"` | `parse_simple_unary_tests` (entry) | `-` means "any" (always true). `not(...)` is negation. |
| 16 | `endpoint = expression` | `parse_expression` | |
| 17 | `simple value = qualified name \| simple literal` | `parse_simple_value` | |
| 18 | `qualified name = name, { ".", name }` | `parse_qualified_name` | Multi-part `a.b.c`; disambiguate from path (rule 43) by position. |
| 19–23 | `addition`/`subtraction`/`multiplication`/`division`/`exponentiation = expression, OP, expression` | Pratt arms | `**` RIGHT-assoc (Pitfall 2). |
| 24 | `arithmetic negation = "-", expression` | Pratt prefix arm | |
| 25–30 | `name` / `name start` / `name part` / `name start char` / `name part char` / `additional name symbols` | Lexer | `name` includes `?`, letters, `_`, digits, and `.`/`/`/`-`/`'`/`+`/`*` as additional name symbols — these make keyword disambiguation subtle (Pitfall 3). |
| 31 | `literal = simple literal \| "null"` | `parse_literal` | `null` → `Value::Null`. |
| 32 | `simple literal = numeric \| string \| boolean \| date time` | `parse_simple_literal` | |
| 33 | `string literal = """ , { char } , """` | Lexer | Escape sequences rule 64. |
| 34 | `boolean literal = "true" \| "false"` | Lexer | |
| 35 | `numeric literal = ["-"], (digits ["." digits] \| "." digits)` | Lexer | → `Value::Number(Decimal)`; never `f64`. |
| 38 | `function invocation = expression, parameters` | Pratt postfix arm | High precedence (higher than `*`). |
| 39 | `parameters = "(", (named \| positional), ")"` | `parse_parameters` | Named: `name: expr`; positional: `expr`. |
| 40–42 | `named parameters` / `parameter name` / `positional parameters` | sub-parsers | |
| 43 | `path expression = expression, ".", name` | Pratt postfix arm | Disambiguate from QN (rule 18) by position. |
| 44 | `for expression = "for", name, "in", iteration context, { ",", name, "in", iteration context }, "return", expression` | `parse_for` | Multiple iteration contexts; `return` keyword. Body has iteration vars in scope. |
| 45 | `if expression = "if", expression, "then", expression, "else", expression` | `parse_if` | |
| 46 | `quantified expression = ("some"\|"every"), name, "in", expression, { ",", name, "in", expression }, "satisfies", expression` | `parse_quantified` | `some`/`every`; `satisfies` keyword. |
| 47 | `disjunction = expression, "or", expression` | Pratt arm (bp 10) | `or` |
| 48 | `conjunction = expression, "and", expression` | Pratt arm (bp 20) | `and` |
| 49 | `comparison = expression, ("="\|"!="\|"<"\|"<="\|">"\|">="), expression \| "between" \| "in"` | Pratt arm (bp 30) | `between` desugars to `a >= b and a <= c`. `in` desugars to a unary-test/interval check. |
| 50 | `filter expression = expression, "[", expression, "]"` | Pratt postfix arm | `[?...]` filter; higher than path? No — rule 2 lists `path` before `filter`, so filter is higher than path. Check: rule 2 order is `... path expression | filter expression | function invocation ...` → filter > path > invocation? Re-read: "lowest to highest" so `path < filter < invocation`. **Verify at Wave 0.** |
| 51 | `instance of = expression, "instance", "of", type` | Pratt arm | `instance of` — spec lists it in rule 2 between `comparison` and `arithmetic`, so precedence is between them. **Verify at Wave 0.** |
| 52 | `type = qualified name \| "range<"...>" \| "list<"...>" \| "context<"...>" \| "function<"...>" "->" type` | `parse_type` | → `FeelType` via `resolve_type_ref` + recursive parse. |
| 53 | `boxed expression = list \| function definition \| context` | `parse_boxed` | Textual boxed = 3 forms. The other 7 "boxed" types (literal, relation, invocation, conditional, filter, for, every/some) are textual-expression forms (rule 2) OR DMN XML boxed-expression elements (Phase 29 IR). The `FeelExpression` enum carries all 10 variants for the evaluator (DMN-15). |
| 54 | `list = "[", [expression, {"," expression}], "]"` | `parse_list` | |
| 55 | `function definition = "function", "(", [formal param, {"," formal param}], ")", ["external"], expression` | `parse_function_definition` | DMN-16. `external` flag is parsed but REJECTED (CONTEXT.md: non-external only) — emit `Err` node. |
| 56 | `formal parameter = parameter name, [":", type]` | sub-parser | Optional type annotation. |
| 57 | `context = "{", [context entry, {"," context entry}], "}"` | `parse_context` | DMN-15; topological evaluation. |
| 58 | `context entry = key, ":", expression` | sub-parser | |
| 59 | `key = name \| string literal` | sub-parser | String-literal keys allow keywords as keys (Pitfall 3). |
| 60 | `date time literal = at literal \| function invocation` | `parse_date_time_literal` | `@"..."` (rule 65) OR a function call like `date("...")`. |
| 61–62 | `white space` / `vertical space` | Lexer skip | |
| 63 | `iteration context = expression, ["..", expression]` | sub-parser | `for`/quantified iteration; `a..b` is a range iteration. |
| 64 | `string escape sequence = "\'" \| "\\"" \| "\\\\" \| "\n" \| "\r" \| "\t" \| code point` | Lexer | |
| 65 | `at literal = "@", string literal` | Lexer | Inner string → Phase 30 `parse_at_literal`. |

### Subtle / commonly misimplemented rules
- **Rule 23 (`**`) is right-associative** (Pitfall 2). All other binary ops are left-assoc.
- **Rule 49 `between`**: `a between b and c` desugars to `a >= b and a <= c`. The `and` here is the FEEL conjunction, NOT a syntax keyword — the parser must parse `b` and `c` as full expressions up to the `and` that ends `between`, not greedily consume `and` as the boolean operator.
- **Rule 49 `in`**: `a in <positive unary test>` and `a in (<positive unary tests>)`. The unary-test form is a single test (rule 13/14); the parenthesized form is a comma-list. This is distinct from the `in` in `for`/`some`/`every` (rule 44/46) — disambiguate by position (Pitfall 3).
- **Rule 50 vs 43 precedence**: re-confirm at Wave 0 whether `path < filter < invocation` (per rule 2 ordering) — this affects `a.b[i]` vs `(a.b)[i]` vs `a.(b[i])`. The spec says rule 2 alternatives are lowest→highest, so `path expression` (rule 43) is lower than `filter expression` (rule 50) is lower than `function invocation` (rule 38). This means `a.b[i]` parses as `filter(path(a, b), [i])` and `a.b(c)` parses as `invoke(path(a, b), (c))`. **Verify with a unit test.**
- **Rule 51 `instance of` precedence**: rule 2 lists `instance of` between `comparison` and `arithmetic expression` — so `instance of` is lower than arithmetic. `1 + 2 instance of number` parses as `(1 + 2) instance of number`. **Verify.**
- **Rule 55 `function` keyword**: at statement start, `function` is a boxed-expression keyword; as a name (rule 25), it could appear in path position. Disambiguate by position.
- **Rule 53 `boxed expression` = 3 forms, NOT 10**: the "10 boxed expressions" in CONTEXT.md refers to the *evaluator's* coverage (all 10 `FeelExpression` boxed variants evaluate recursively), NOT the textual grammar's `boxed expression` production. The other 7 (literal, relation, invocation, conditional, filter, for, every/some) arrive via rule 2 textual forms or DMN XML boxed-expression elements (Phase 29 IR). **The parser produces the textual subset; the evaluator handles all 10.** Flag this distinction in the plan.

## Three-Valued Logic Truth Tables (DMN-12)

### `=` (equality) — rule 49.a
| a | b | a = b |
|---|---|-------|
| null | null | **true** (rule 49.a: `e = null` → `FEEL(e) is null`) |
| null | non-null | **false** (`FEEL(e) is null` is false) |
| non-null | null | **false** |
| same-kind, equal | | **true** |
| same-kind, unequal | | **false** |
| **different kind** (e.g. `"a"`, `1`) | | **null** (type mismatch) |

### `!=` (inequality) — negation of `=`
| a | b | a != b |
|---|---|--------|
| null | null | **false** |
| null | non-null | **true** |
| non-null | null | **true** |
| same-kind, equal | | **false** |
| same-kind, unequal | | **true** |
| different kind | | **null** |

### `<`, `<=`, `>`, `>=` (ordering) — comparison table
| a | b | result |
|---|---|--------|
| null | _ | **null** |
| _ | null | **null** |
| same-kind, comparable | | `Some(Ordering)` → Bool |
| **different kind** | | **null** (input domain requires same kind) |

### `and` (conjunction) — false is dominant
| a | b | a and b |
|---|---|---------|
| false | _ | **false** |
| _ | false | **false** |
| true | true | **true** |
| true | null | **null** |
| null | true | **null** |
| null | null | **null** |

### `or` (disjunction) — true is dominant
| a | b | a or b |
|---|---|--------|
| true | _ | **true** |
| _ | true | **true** |
| false | false | **false** |
| false | null | **null** |
| null | false | **null** |
| null | null | **null** |

### `not` (negation)
| a | not(a) |
|---|--------|
| true | false |
| false | true |
| null | **null** (negation table: "otherwise" → null) |

### `is` built-in (DMN-17 equality family — NEVER null, Pitfall 6)
| a | b | is(a, b) |
|---|---|----------|
| null | null | **true** |
| null | non-null | **false** |
| different kind | | **false** (NOT null — `is` is total) |
| same kind, equal | | **true** |
| same kind, unequal | | **false** |

## Operators (DMN-13)

### Arithmetic
| Operator | Semantics | Null/edge behavior |
|----------|-----------|--------------------|
| `+` (Number) | `Decimal::add` | null operand → null. |
| `+` (String) | String concatenation (`"a" + "b" = "ab"`) | null operand → null. Number+String → null (no implicit conversion, Phase 30 Pitfall 6). |
| `-` (Number) | `Decimal::sub` | null operand → null. |
| `*` (Number) | `Decimal::mul` | null operand → null. |
| `/` (Number) | `Decimal::checked_div`; div-by-zero → `Value::Null` | null operand → null. Overflow → null. |
| `**` (Number) | Integer-exponent `Decimal` pow; for non-integer exponent, `exp(ln(base) * exp)` via `maths` | null operand → null. Overflow → null. |
| `-` (unary) | `Decimal::neg` | null → null. |

### Comparison
- `=`, `!=`, `<`, `<=`, `>`, `>=`: see truth tables above. All via `feel_equals`/`feel_compare` → `Option<_>` → `Value::Bool`/`Value::Null`.
- `a between b and c`: desugar to `a >= b and a <= c`.
- `a in <test>` / `a in (tests)`: evaluate the unary test(s) against `a`. `-` (empty unary tests) means "always true". `not(tests)` negates.

### Boolean
- `and`, `or`: explicit truth tables above (`and_3vl`/`or_3vl`).
- `not(x)`: built-in OR operator; `not_3vl`.

### Date-time arithmetic matrix (DMN-13, spec §10.3.2.3.5 + Drools handbook)
| LHS | RHS | Result | Implementation |
|-----|-----|--------|----------------|
| `date` | `years and months duration` | `date` | Phase 30 `add_months_zoned` (or `NaiveDate::checked_add_months` for the date-only form). |
| `date and time` | `days and time duration` | `date and time` | Phase 30 `add_days_time_zoned`. |
| `time` | `days and time duration` | `time` | `NaiveTime + TimeDelta` (wraps 24h); preserve offset/zone. |
| `date` | `date` | `days and time duration` | `TimeDelta::days((a - b).num_days())`. |
| `date and time` | `date and time` | `days and time duration` | `dt1 - dt2` → `TimeDelta`. |
| `time` | `time` | `days and time duration` | `TimeDelta` (may be > 24h if times cross midnight). |
| `days and time duration` | `days and time duration` | `days and time duration` | `TimeDelta + TimeDelta`. |
| `years and months duration` | `years and months duration` | `years and months duration` | `months + months` (i64 add). |
| `date` | `date` (years and months duration between) | `years and months duration` | `years and months duration(a, b)` built-in. |

**DST handling**: Phase 30's `add_months_zoned`/`add_days_time_zoned` already map DST gap → `None` (null) and overlap → earliest. Phase 31 reuses these — no new DST code. Confirm `temporal.rs` has these helpers (CONTEXT.md discretion: planner verifies; only add what's missing).

### Duration normalization
- `days and time duration`: `chrono::TimeDelta` (fixed seconds). Normalize via `TimeDelta::seconds`.
- `years and months duration`: `YearsMonthsDuration { months: i64 }` (total months, normalized so `months ∈ [0, 11]` at the value level). `P2Y2M = 26 months`.

## Control + Quantified + Path + Filter + `instance of` (DMN-14)

### `if` (rule 45)
`if cond then a else b` — evaluate `cond`; if `Value::Bool(true)` → `a`; if `Value::Bool(false)` → `b`; if `Value::Null` → `Value::Null` (null cond → null result). NOTE: the spec's three-valued `if` with a null condition is implementation-defined in some readings — recommend null → null (consistent with the rest of 3VL). Verify against TCK at Phase 35.

### `for` (rule 44)
`for x in iter1, y in iter2 return body` — iterate over each `iteration context` (rule 63: a list or `a..b` range); bind `x`, `y` into the flat Scope (with `deps = [iter-source names]`); evaluate `body` for each combination; collect results into a `Value::Array`. Iteration variables are scoped to the body — implement by extending the flat Scope with the iteration bindings (add/remove, or a child-flat-scope copy — the user's flat-Scope model permits a scoped copy).

### `some` / `every` (rule 46)
`some x in list satisfies pred` → `Value::Bool(true)` if any element satisfies; `false` if none do; `null` if `list` is null or the predicate yields null without a definite true/false. `every x in list satisfies pred` → `true` if all satisfy; `false` if any fails; `null` if null interferes. Use `or_3vl`/`and_3vl` to fold the predicate results.

### Path (rule 43)
`a.b.c` — path navigation. `a` is evaluated, then `.b` navigates into a context (`.b` → `get value(a, "b")`) or a list (`.b` → `[elem.b for elem in a]`, the "map" semantics of path on a list). Multi-level `.c` continues. Disambiguate from QN (rule 18) by position: at the start of an expression, `a.b.c` is a QN (resolve against scope); after an operator or `]`/`)`, it's a path.

### Filter (rule 50)
`list[?pred]` — filter `list` to elements where `pred` is true. The predicate is evaluated with the current element bound to a special variable (convention: `item`, or the positional `?`). For a non-list LHS, filter on a singleton list. `null[?...]` → `null`.

### `instance of` (rule 51)
`expr instance of T` — evaluate `expr`, compute `type_of(value)`, parse `T` (rule 52) into a `FeelType` via `resolve_type_ref` (extended for the recursive `list<...>`/`context<...>`/`function<...>`/`range<...>` forms), and return `Value::Bool(conforms(type_of(value), T))`. `null instance of Any` → `true` (Null <: Any).

## Boxed Expressions (DMN-15) — All 10

| # | Boxed type | `FeelExpression` variant | Source | Recursive? |
|---|-----------|--------------------------|--------|------------|
| 1 | Literal | `Literal(Value)` | Text (rule 31) or XML `<literalExpression>` | Leaf |
| 2 | Context | `Context(Vec<(String, FeelExpression)>)` | Text (rule 57) or XML `<context>` | Yes (entries) |
| 3 | List | `List(Vec<FeelExpression>)` | Text (rule 54) or XML `<list>` | Yes (elements) |
| 4 | Relation | `Relation { columns: Vec<String>, rows: Vec<Vec<FeelExpression>> }` | XML `<relation>` (no textual form) | Yes (cells) |
| 5 | Function invocation | `FunctionInvocation { callee: Box, args: Vec, named: Option<Vec<(String, FeelExpression)>> }` | Text (rule 38) or XML `<invocation>` | Yes (args) |
| 6 | Function definition | `FunctionDefinition { params: Vec<(String, Option<FeelType>)>, body: Box, external: bool }` | Text (rule 55) or XML `<functionDefinition>` | Yes (body) |
| 7 | Conditional | `If { cond: Box, then: Box, else_: Box }` (shared with rule 45) | Text (rule 45) or XML `<conditional>` | Yes |
| 8 | Filter | `Filter { target: Box, predicate: Box }` | Text (rule 50) or XML `<filter>` | Yes |
| 9 | For | `For { iters: Vec<(String, FeelExpression)>, body: Box }` | Text (rule 44) or XML `<for>` | Yes |
| 10 | Every/Some | `Quantified { kind: QuantKind, iters: Vec<(String, FeelExpression)>, satisfies: Box }` | Text (rule 46) or XML `<every>`/`<some>` | Yes |

**Boxed-context topological evaluation (DMN-15):**
1. For each entry `(key, expr)`, compute the free variables of `expr` (names it references).
2. Build the dependency graph: edge `key → dep` for each free var that is also a key.
3. **DFS color-marking cycle detection**: White (unvisited) → Gray (on-stack) → Black (done). A Gray→Gray edge is a cycle.
4. **DFS post-order** gives the topological order (reverse of finishing times).
5. Evaluate entries in topological order, binding each result into the working Scope. A cyclic entry → `Value::Null` + an error carrying source location.
6. Return the resolved context as `Value::Object(HashMap)` (or a `FeelContext` if the Phase 30 `Value::Object` legacy semantics clash — planner decides; `Value::Object` is the natural fit).

**Recommendation:** Implement `eval_context_topo` with an explicit `Color` enum and a `Vec<String>` stack for the Gray path. ~30 lines of Rust. Do NOT pull in `petgraph` — the dependency graph is a single context's entries.

## User-Defined Lambdas (DMN-16, grammar rule 55)

### Extended `FeelFunction` carrier
Phase 30's `FeelFunction` is a placeholder (arity + param/return `FeelType`s). Phase 31 extends it to carry the closure body and captured environment:
```rust
// Source: CONTEXT.md locked decision (DMN-16 non-external) + Phase 30 Value::Function doc comment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeelFunction {
    pub arity: usize,
    pub param_types: Vec<FeelType>,
    pub ret_type: Box<FeelType>,
    // --- Phase 31 additions ---
    /// Parameter names (for positional→named binding). Length == arity.
    pub param_names: Vec<String>,
    /// The closure body AST. Arc so the closure is cheap to clone (the AST
    /// is shared with the cache). None for built-ins (which are BuiltinFn ptrs).
    pub body: Option<Arc<FeelExpression>>,
    /// Captured lexical scope (flat Scope snapshot at function-definition time).
    /// For DMN-16 non-external lambdas, this is the defining context's bindings.
    pub captured: Option<Arc<Scope>>,
}
```

### Closure-capture model
**Lexical scope capture**: at function-definition evaluation, the current `Scope` is snapshotted into `FeelFunction::captured` (an `Arc<Scope>` clone — cheap because the bindings HashMap is shared via `Arc`). At invocation, a working Scope is constructed by merging the captured scope with the parameter bindings (params override captured names of the same key). The body is evaluated against this merged scope. This is **lexical capture** (the closure sees its definition site's bindings), NOT dynamic capture.

### Non-external constraint
Grammar rule 55 allows an optional `external` keyword. Per CONTEXT.md, DMN-16 covers only non-external lambdas. The parser RECOGNIZES `external` (parses it into `FeelFunction::external = true`) but the evaluator REJECTS it — evaluating a `FunctionDefinition` with `external: true` yields `Value::Null` + a logged warning. This forward-compatible shape lets a future phase add external integration without an AST migration.

## Built-in Function Library (~70+, DMN-17)

### Function families + counts
| Family | Count | Functions |
|--------|-------|-----------|
| Conversion | 8 | `date` (3 overloads), `time` (3), `date and time` (2), `duration`, `number`, `string`, `years and months duration` |
| Numeric | 12 | `decimal`, `floor` (2), `ceiling` (2), `round up`, `round down`, `round half up`, `round half down`, `abs`, `modulo`, `sqrt`, `log`, `exp`, `odd`, `even` |
| String | 13 | `substring`, `string length`, `upper case`, `lower case`, `substring before`, `substring after`, `replace`, `contains`, `starts with`, `ends with`, `matches`, `split`, `string join` |
| List | 24 | `list contains`, `list replace`, `count`, `min`, `max`, `sum`, `mean`, `all`, `any`, `sublist`, `append`, `concatenate`, `insert before`, `remove`, `reverse`, `index of`, `union`, `distinct values`, `flatten`, `product`, `median`, `stddev`, `mode`, `sort` |
| Boolean | 1 | `not` |
| Context | 5 | `context`, `context put` (2), `context merge`, `get value`, `get entries` |
| Date-time | 7 | `now`, `today`, `day of year`, `day of week`, `month of year`, `week of year`, `is` |
| Range | 14 | `before`, `after`, `meets`, `met by`, `overlaps`, `overlaps before`, `overlaps after`, `finishes`, `finished by`, `includes`, `during`, `starts`, `started by`, `coincides` |
| **Total** | **~84** | (overloads counted as one name; the ~70+ in the requirement is a floor, not a cap) |

### Null-propagation behavior (general rule)
- A null argument propagates to `Value::Null` UNLESS the spec says otherwise. Exceptions:
  - `count([null])` → `1` (count counts elements, including null).
  - `string(null)` → `null` (spec: `string` of null is null).
  - `is(a, b)` → NEVER null (Pitfall 6) — returns `Bool(true)`/`Bool(false)`.
  - `all([])` → `true`; `any([])` → `false` (empty-list base cases).
  - `sum([])` → `null` per spec (some readings: 0 — DMN 1.5 says null; DMN-V2-04 notes DMN 1.6 may change this; follow DMN 1.5: `sum([])` → null).
  - `mean([])`, `median([])`, `stddev([])`, `mode([])` → `null` (empty).
  - `min([])`, `max([])` → `null` (empty).
  - `product([])` → `1`? Follow spec: empty product → null (verify against TCK).
- Numeric edge cases (div-by-zero in `modulo`, overflow in `**`, sqrt of negative) → `Value::Null` (CONTEXT.md locked).

### Registry shape
`HashMap<&'static str, BuiltinFn>` where `BuiltinFn = fn(&[Value]) -> Value` (function pointer). For built-ins needing the `regex` crate (`replace`/`matches`/`split`), the function compiles the pattern on each call OR caches compiled patterns in a `OnceLock<HashMap<&str, Regex>>` — recommend the latter for performance (the TCK exercises these heavily). Initialize at first use (lazy) or at module init (eager). CONTEXT.md discretion: planner decides.

### Plan split (DMN-17 → 31-07 + 31-08)
- **31-07 (part 1):** Conversion + String + List + Numeric families + the `HashMap<&str, BuiltinFn>` registry skeleton.
- **31-08 (part 2):** Date-time + Context + Boolean + Range + Equality (`is`) families.

## AST Cache (DMN-18)

### `FeelAstCache`
```rust
use dashmap::DashMap;
use std::sync::Arc;
pub struct FeelAstCache {
    map: DashMap<String, Arc<FeelExpression>>,
}
```
Keying: `format!("{entry_point:?}:{content}")` hashed (SHA-256 or FxHash) — the entry-point kind (`Expression` vs `SimpleUnaryTests`) is part of the key because the same text can parse differently under the two entry points. Content is post-CDATA-strip, post-trim (Phase 30 `TextContent.content`).

### IR wiring (Phase 29 loader extension)
The DMN IR loader (`engine/src/dmn/parser.rs`) is extended to walk every FEEL-bearing IR field after the IR is built and populate the cache:
- `InputClause.input_expression: String` → `parse_expression` (entry point `Expression`).
- `RuleEntry.text: TextContent` — for **input** entries → `parse_simple_unary_tests`; for **output** entries → `parse_expression`; for **annotation** entries → `parse_expression`.
- `OutputClause.default_output_entry: Option<String>` → `parse_expression`.
- `AllowedValues::UnaryTests(String)` → `parse_simple_unary_tests`.
- Non-decision-table boxed expressions on a `Decision` (literal/context/list/relation/invocation/function-definition/conditional/filter/for/every/some XML elements) — Phase 29 currently carries FEEL as strings; if Phase 29 already parsed these into IR nodes, the cache stores the corresponding `FeelExpression`. Planner verifies the IR shape and wires accordingly.

### Evaluator contract (type-level enforcement)
`pub fn evaluate(expr: &FeelExpression, scope: &Scope) -> Value` — the evaluator takes a **borrowed** `FeelExpression`, never `&str`. There is no `evaluate(src: &str, ...)` overload. This makes it impossible to call the evaluator with raw source, preventing the 4–6× re-parse bug at the type level. The decision-table engine (Phase 32) and the decision evaluator (Phase 33) obtain `Arc<FeelExpression>` from the cache and pass `&*arc` to `evaluate`.

## Scope Model + QN Resolution (Grey Area 4, user override)

### Flat `Scope`
```rust
pub struct Scope {
    pub bindings: HashMap<String, Value>,
    pub deps: HashMap<String, Vec<String>>,
}
```
A context is a flat map. Nested-scope resolution (e.g. a boxed context inside a for-loop) uses explicit dependency tracking — the `deps` map records, for each binding, which other names it depends on. There is NO parent-chain walk.

### Qualified-name (QN) resolution — ROADMAP "longest-match" note
A qualified name `a.b.c` is resolved by trying the longest prefix that exists in `bindings`, then path-navigating the rest:
1. Look up `a.b.c` directly — if bound, return it.
2. Else look up `a.b` — if bound, path-navigate `.c`.
3. Else look up `a` — if bound, path-navigate `.b.c`.
4. Else `Value::Null`.

This "longest-match" rule handles both QN-as-single-binding (`a.b.c` is one variable name) and QN-as-path (`a` is a context, `.b.c` navigates into it). The ambiguity is resolved in favor of the longest bound prefix.

### How flat-Scope drives boxed-context topological evaluation (DMN-15)
The `deps` map IS the dependency graph for boxed-context topological sort. When a boxed context is evaluated, each entry's free variables (names referenced but not bound in the entry's own expression) are intersected with the context's keys to form the `deps` edges. DFS post-order over `deps` gives the topological order. This is the user's locked mechanism — flat Scope + explicit deps unifies QN resolution and boxed-context dependency evaluation.

## Parser Error Recovery (Grey Area 4, user override)

### Strategy: partial AST with `Err` nodes
The parser performs **error recovery**, not fail-fast. On a parse error:
1. Record the error with a `Span` (byte offset + length into the source).
2. Emit a `FeelExpression::Err(Span, String)` node at the error site.
3. Attempt to recover by skipping to the next synchronization token (`,`, `)`, `]`, `}`, `then`, `else`, `return`, `satisfies`, `in`, end-of-input).
4. Continue parsing the rest of the expression/table.

The rest of the table still loads — per-cell errors are isolated. The Phase 34 frontend editor can display per-cell parse errors (using the spans) while the rest of the table is editable.

### Source-span representation
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// Byte offset into the cell source where the error starts.
    pub start: usize,
    /// Byte length of the error span.
    pub len: usize,
    /// 1-based line number (for editor display).
    pub line: u32,
    /// 1-based column number (for editor display).
    pub col: u32,
}
```
Carry `Span` on every `FeelExpression` variant (or on a wrapper `Spanned<FeelExpression>`) so eval errors (CONTEXT.md: "eval errors carry source location") can point at the failing expression. Recommend a `Spanned<FeelExpression>` wrapper to avoid bloating every variant — planner discretion.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| FEEL equality via `==` (two-valued) | FEEL equality via `feel_equals -> Option<bool>` (three-valued) | DMN 1.5 §10.3.2.2 / rule 49.a | `null = null` → `true`; type mismatch → `null`, not `false`. |
| `rust_decimal` 1.36 | `rust_decimal` 1.42 (with `maths` feature) | 2024–2025 | `sqrt`/`ln`/`exp` available for numeric built-ins. |
| `chrono::Duration` (deprecated alias) | `chrono::TimeDelta` | chrono 0.4.x | Use `TimeDelta` in new code; `Duration` is an alias retained. |
| Hand-written regex in FEEL built-ins | `regex` crate (XQuery 1.0 subset) | Rust ecosystem standard | `replace`/`matches`/`split` delegate to `regex`. |
| Parent/child chained Scope | Flat Scope + explicit deps (user override) | Phase 31 design | Unifies QN resolution and boxed-context topo sort. |

**Deprecated/outdated:**
- `chrono::Duration` — use `TimeDelta`.
- `rust_decimal::Decimal::new(i64, u32)` — prefer `dec!` macro for literals.
- `compare_values` (Rete path) — NEVER reuse for FEEL equality (Pitfall 1).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The Rust `regex` crate covers all XQuery 1.0 regex features the TCK vectors exercise. | DMN-17 / Pitfall 7 | A TCK `replace`/`matches`/`split` vector uses a feature `regex` lacks (e.g. some Unicode class shorthands), causing a Phase 35 conformance failure. Mitigation: Wave-0 task scans the vendored TCK regex vectors for patterns `regex` rejects; if any, switch that built-in to `fancy-regex` (localized, not a wholesale dep change). |
| A2 | `path < filter < invocation` precedence (rule 2 ordering) is correct. | DMN-11 / Rule 50 | `a.b[i]` parses wrong. Mitigation: Wave-0 unit test `a.b[i]` vs `(a.b)[i]` vs `a.(b[i])` against a TCK vector. |
| A3 | `instance of` precedence is between `comparison` and `arithmetic` (rule 2 ordering). | DMN-11 / Rule 51 | `1 + 2 instance of number` parses wrong. Mitigation: Wave-0 unit test. |
| A4 | DMN 1.5 `if` with a `null` condition → `null` (not a specified value; recommendation consistent with 3VL). | DMN-14 | A TCK vector exercises `if null then ... else ...` and expects a different result. Mitigation: Phase 35 TCK run; if divergent, document as a known divergence per DMN-33. |
| A5 | `sum([])` → `null` per DMN 1.5 (DMN-V2-04 notes DMN 1.6 may change this to 0). | DMN-17 | A TCK vector expects `0`. Mitigation: follow DMN 1.5 (null) for v6.0; document as a known divergence candidate per DMN-33. |
| A6 | The Phase 29 IR carries non-decision-table boxed expressions (literal/context/list/relation/etc. XML elements) in a shape the Phase 31 cache can walk. | DMN-18 | Phase 29 may store them as strings only, requiring the cache to parse-on-demand for these forms. Mitigation: planner verifies the IR shape at Wave 0 and wires accordingly (the cache wiring is planner-discretion per CONTEXT.md). |
| A7 | `chrono-tz` 0.10.4's `LocalResult` API is unchanged (Phase 30 Pitfall 5 policy still holds). | DMN-13 | Phase 30 already verified; re-confirm at Wave 0. |

**If this table is empty:** N/A — 7 assumptions logged, each with a mitigation task.

## Open Questions

1. **Does Phase 29's IR carry non-decision-table boxed expressions (literal/context/list/relation/invocation/function-definition/conditional/filter/for/every/some XML elements) as structured nodes or as strings?**
   - What we know: `Decision.decision_table` is structured; `RuleEntry.text` carries FEEL as `TextContent` strings; the model.rs comment says "other boxed expressions land in Phase 31."
   - What's unclear: whether there's an IR enum for non-table boxed expressions or only strings.
   - Recommendation: Wave-0 task in 31-01 (or 31-09) reads `engine/src/dmn/model.rs` and `parser.rs` to confirm. If strings-only, the cache parses them on demand; if structured, the cache maps IR nodes to `FeelExpression` directly. Either way the evaluator handles all 10 variants.

2. **Exact precedence of `path` vs `filter` vs `invocation` (rule 2 ordering)?**
   - What we know: rule 2 lists `path expression | filter expression | function invocation` in that order (lowest→highest).
   - What's unclear: whether `a.b[i]` is `filter(path(a,b), [i])` or `path(filter(a,[i]), b)`.
   - Recommendation: Wave-0 unit test against a TCK vector; the spec ordering says `path < filter < invocation`, so `a.b[i]` = `filter(path(a, b), [i])` and `a.b(c)` = `invoke(path(a, b), (c))`. Verify.

3. **`is` vs `=` for `null` operands — confirm `is(null, null)` → `true` and `is("a", 1)` → `false` (never null)?**
   - What we know: spec says `is` "never results in null."
   - What's unclear: the exact `is` semantics for cross-type operands.
   - Recommendation: implement `is` as total structural identity (Pitfall 6); verify against TCK at Phase 35.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `rustc` | All Rust code | ✓ | 1.94.1 | — |
| `cargo` | Build | ✓ | (workspace) | — |
| `rust_decimal` | DMN-13 arithmetic, DMN-17 numeric built-ins | ✓ (in Cargo.toml) | 1.42 (features serde/macros/maths) | `bigdecimal` 0.4.10 (deferred — only if Phase 35 TCK demands scale > 28) |
| `chrono` | DMN-13 date-time arithmetic | ✓ (in Cargo.toml) | 0.4 (serde) | — |
| `chrono-tz` | DMN-13 IANA-zone datetime arithmetic | ✓ (in Cargo.toml) | 0.10.4 (serde) | — |
| `dashmap` | DMN-18 AST cache | ✗ (NOT in Cargo.toml) | 7.0.0-rc2 | None — must add (legitimacy gate OK) |
| `regex` | DMN-17 string built-ins (replace/matches/split) | ✗ (NOT in Cargo.toml) | 1.12.4 | `fancy-regex` (only if a TCK vector demands a feature `regex` lacks) |
| `proptest` | Validation (property tests) | ✓ (workspace dev-dep) | 1.11.0 | — |
| `bigdecimal` | DMN-17 rounding built-ins (scale > 28) | ✗ (deferred) | 0.4.10 | Only if Phase 35 TCK demands |
| OMG DMN TCK repo | DMN-17/DMN-12 validation vectors | ✓ (cloned in Phase 30; reuse the fixture home) | master | Camunda feel-scala test corpus (fallback per Phase 35 research) |

**Missing dependencies with no fallback:**
- `dashmap` — must be added in Wave 0. No alternative provides sharded concurrent HashMap with the v5.0-stack consistency.

**Missing dependencies with fallback:**
- `regex` — has a fallback (`fancy-regex`) if TCK demands; default to `regex`.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `proptest` 1.11.0 (workspace dev-dep) + snapshot/golden tests |
| Config file | `engine/Cargo.toml` `[dev-dependencies]` (proptest already present) |
| Quick run command | `cargo test -p engine --lib dmn::feel::` |
| Full suite command | `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DMN-11 | Grammar rules 1–65 parse (dual entry points, keyword-as-name) | unit + property | `cargo test -p engine --lib dmn::feel::parser` | ❌ Wave 0 (31-01) |
| DMN-11 | Error recovery: bad cell → `Err` node, rest loads | unit | `cargo test -p engine --lib dmn::feel::parser::error_recovery` | ❌ Wave 0 (31-01) |
| DMN-12 | `"a"=1`→null, `null<3`→null, `null=null`→true, `not(null)`→null | unit (MANDATORY) | `cargo test -p engine --lib dmn::feel::eval::dmn12_` | ❌ Wave 0 (31-02) |
| DMN-12 | `and`/`or`/`not` truth tables (explicit, no Rust `==`) | unit + property | `cargo test -p engine --lib dmn::feel::eval::truth_tables` | ❌ Wave 0 (31-02) |
| DMN-12 | No `==`/`Ord` reuse in feel::eval (lint guard) | smoke | `cargo test -p engine --lib dmn::feel::eval::no_rust_eq` | ❌ Wave 0 (31-02) |
| DMN-13 | Arithmetic (`+ - * / **`), `**` right-assoc | unit + property | `cargo test -p engine --lib dmn::feel::eval::arithmetic` | ❌ Wave 0 (31-03) |
| DMN-13 | Date-time arithmetic (date+ymd, dateTime+dtd, date−date) | unit + property | `cargo test -p engine --lib dmn::feel::eval::datetime_arith` | ❌ Wave 0 (31-03) |
| DMN-13 | Division by zero / overflow → null | unit | `cargo test -p engine --lib dmn::feel::eval::numeric_edge` | ❌ Wave 0 (31-03) |
| DMN-13 | String concat (`+`), `1 + "2"` → null | unit | `cargo test -p engine --lib dmn::feel::eval::string_concat` | ❌ Wave 0 (31-03) |
| DMN-14 | `if`/`for`/`some`/`every`/path/filter/`instance of` | unit + property | `cargo test -p engine --lib dmn::feel::eval::control` | ❌ Wave 0 (31-04) |
| DMN-15 | All 10 boxed expressions evaluate recursively | unit + snapshot | `cargo test -p engine --lib dmn::feel::eval::boxed` | ❌ Wave 0 (31-05) |
| DMN-15 | Boxed-context topological order + cycle detection | unit + property | `cargo test -p engine --lib dmn::feel::eval::context_topo` | ❌ Wave 0 (31-05) |
| DMN-16 | User-defined lambdas evaluate (grammar rule 55, non-external) | unit | `cargo test -p engine --lib dmn::feel::eval::lambdas` | ❌ Wave 0 (31-06) |
| DMN-16 | `external` function rejected → null + warning | unit | `cargo test -p engine --lib dmn::feel::eval::external_rejected` | ❌ Wave 0 (31-06) |
| DMN-17 (part 1) | Conversion/string/list/numeric builtins (~45 fns) | unit + property | `cargo test -p engine --lib dmn::feel::builtins::conversion dmn::feel::builtins::string dmn::feel::builtins::list dmn::feel::builtins::numeric` | ❌ Wave 0 (31-07) |
| DMN-17 (part 2) | Date-time/context/boolean/range/equality builtins (~30 fns) | unit | `cargo test -p engine --lib dmn::feel::builtins::datetime dmn::feel::builtins::context dmn::feel::builtins::boolean dmn::feel::builtins::range dmn::feel::builtins::equality` | ❌ Wave 0 (31-08) |
| DMN-17 | `is` never returns null (Pitfall 6) | unit | `cargo test -p engine --lib dmn::feel::builtins::is_never_null` | ❌ Wave 0 (31-08) |
| DMN-18 | AST cache: 30-cell table × 10k evals = ~30 parses, not 300k | bench | `cargo test -p engine --lib dmn::feel::cache::perf -- --nocapture` (or criterion) | ❌ Wave 0 (31-09) |
| DMN-18 | Evaluator takes `&FeelExpression`, never `&str` (type-level) | compile | `cargo build -p engine` (no `evaluate(&str,...)` overload exists) | ❌ Wave 0 (31-09) |
| DMN-18 | IR loader pre-parses all cells into the cache | integration | `cargo test -p engine --test feel_cache_wiring` | ❌ Wave 0 (31-09) |
| Regression | Rete path unchanged — `engine/src/engine/` compiles, existing tests pass | regression | `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` | ✅ (existing) |
| TCK subset | Vendored OMG TCK FEEL subset passes (loader + runner scaffold) | integration | `cargo test -p engine --test feel_tck_subset` | ❌ Wave 0 (defer full TCK to Phase 35) |

### Sampling Rate
- **Per task commit:** `cargo test -p engine --lib dmn::feel::`
- **Per wave merge:** `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak`
- **Phase gate:** Full suite green + DMN-12 truth-table unit tests green + vendored TCK subset green + Rete regression green before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `cargo add dashmap regex --package engine` — two dependency installs
- [ ] `engine/src/dmn/feel/ast.rs` — `FeelExpression` enum + `Span` + `QuantKind` (DMN-11)
- [ ] `engine/src/dmn/feel/parser.rs` — Lexer + recursive-descent/Pratt parser, dual entry points, error recovery (DMN-11)
- [ ] `engine/src/dmn/feel/eval.rs` — `evaluate(&FeelExpression, &Scope)`, private `cmp` module, `Scope` type (DMN-12/13/14/15)
- [ ] `engine/src/dmn/feel/builtins.rs` — `BuiltinFn` type + registry skeleton (DMN-17)
- [ ] `engine/src/dmn/feel/coercion.rs` — explicit FEEL operator/builtin coercions (DMN-13)
- [ ] `engine/src/dmn/feel/cache.rs` (or fold into `parser.rs`) — `FeelAstCache = DashMap<String, Arc<FeelExpression>>` (DMN-18)
- [ ] `engine/src/dmn/feel/types.rs` — extend `FeelFunction` with `body`/`captured`/`param_names` (DMN-16)
- [ ] `engine/tests/fixtures/tck/feel/` — vendored OMG TCK FEEL subset (loader + runner scaffold)
- [ ] `engine/tests/feel_tck_subset.rs` — TCK harness scaffold (full passage deferred to Phase 35)
- [ ] `engine/tests/feel_cache_wiring.rs` — IR loader pre-parse integration test

*(If no gaps: N/A — all infrastructure is new in Wave 0 across the 9 plans.)*

## Security Domain

> Phase 31 adds a parser (untrusted FEEL text from `.dmn` files), an evaluator (no I/O), and a built-in function library. `security_enforcement` is enabled by default (no `security_enforcement: false` key in `.planning/config.json`). The parser is the primary new untrusted-input boundary.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — (no auth surface) |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes | Parser is fail-closed: malformed FEEL → `FeelExpression::Err(span, msg)` (error recovery, not panic). Malformed `@""` → `Value::Null` (Phase 30 `parse_at_literal` returns `None`). No `unwrap()` on `Option`/`Result` from untrusted input. Lexer rejects oversized tokens (bound the token count per cell to prevent pathological-parse DoS — recommend a `max_tokens` cap). |
| V6 Cryptography | no | — |

### Known Threat Patterns for FEEL parser/evaluator

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malformed FEEL causing parser panic | Denial of Service | Error recovery (`Err` nodes); no `unwrap()`; proptest fuzzes malformed strings. |
| Pathological-parse DoS (deeply nested `((((...)))` ) | Denial of Service | A `max_depth` cap on the recursive-descent stack; reject with `Err` node past the cap. |
| Regex ReDoS (`replace`/`matches`/`split` built-ins) | Denial of Service | The `regex` crate uses bounded-execution finite automata (no catastrophic backtracking). Set a `Regex::set_compile_timeout`/size cap if available. |
| Decimal overflow / div-by-zero in arithmetic | Denial of Service | `checked_div`/`checked_mul` → `None` → `Value::Null`; no panic. |
| Infinite recursion in boxed-context cycle | Denial of Service | DFS color-marking cycle detection (DMN-15); cyclic entry → `Value::Null` + error. |
| Stack overflow in deeply recursive boxed expression | Denial of Service | A `max_eval_depth` cap on `evaluate` recursion; return `Value::Null` past the cap. |
| AST cache unbounded growth (many distinct cells) | Resource exhaustion | The cache is bounded by the IR's cell count (finite per `.dmn` file); no unbounded growth. A pathological `.dmn` with millions of cells is bounded by Phase 29's parser limits. |

## Sources

### Primary (HIGH confidence)
- OMG DMN 1.5 plain-text spec (`omg-dmn-taskforce/plain-text-spec`, `10-expression-language-FEEL.adoc`) — grammar rules 1–65 quoted verbatim; §10.3.2.2 (numeric domain + three-valued equality rule 49.a); §10.3.2.3.5 (date ≡ datetime@UTC); §10.3.2.9 (type lattice, consumed from Phase 30). [CITED: raw.githubusercontent.com/omg-dmn-taskforce/plain-text-spec/main/10-expression-language-FEEL.adoc]
- Drools DMN FEEL handbook (`kiegroup/dmn-feel-handbook`) — three-valued-logic `and`/`or`/`not` truth tables; date-time arithmetic examples (date+ymd, dateTime+dtd, date−date → dtd, years and months duration); string concat via `+`; `in` operator range semantics. [CITED: kiegroup.github.io/dmn-feel-handbook]
- Phase 30 RESEARCH (`30-RESEARCH.md`) — `rust_decimal`/`chrono`/`chrono-tz` standard stack + Pitfalls 1–6 + `feel_numeric_eq -> Option<bool>` template (the direct ancestor of DMN-12). [VERIFIED: codebase]
- Phase 30 source — `engine/src/dmn/feel/{mod,types,temporal,numeric}.rs`, `engine/src/data_types.rs` (Value enum), `engine/src/dmn/model.rs` (IR shape for DMN-18 wiring). [VERIFIED: codebase]
- Phase 29 source — `engine/src/dmn/parser.rs` (IR loader to extend for DMN-18). [VERIFIED: codebase]

### Secondary (MEDIUM confidence)
- WebSearch aggregate (Camunda 8 FEEL docs + Red Hat Decision Manager FEEL reference) — full ~70+ built-in function enumeration by family with signatures; `is` "never results in null" confirmed; `context`/`list replace` DMN 1.4 additions; `day of year`/`week of year` DMN 1.5 additions. [CITED: docs.camunda.io + docs.redhat.com — cross-checked against spec]
- `cargo search` + `gsd-tools query package-legitimacy check` — confirmed `dashmap 7.0.0-rc2` (OK, xacrimon/dashmap, ~5M/wk, since 2019) and `regex 1.12.4` (OK, rust-lang/regex, ~15M/wk, since 2014). [VERIFIED: registry + legitimacy gate]

### Tertiary (LOW confidence)
- None — all findings are spec-cited, registry-verified, or codebase-verified.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — `dashmap`/`regex` verified via `cargo search` + legitimacy gate OK; `rust_decimal`/`chrono`/`chrono-tz`/`proptest` already in deps (Phase 30).
- Grammar (DMN-11): HIGH — 65 rules quoted verbatim from the OMG plain-text spec; parser-method mapping is prescriptive; precedence ambiguities (A2/A3) flagged as Wave-0 verification tasks.
- Three-valued logic (DMN-12): HIGH — rule 49.a quoted verbatim; truth tables derived from spec + Drools handbook; `feel_equals`/`feel_compare` signature derived from Phase 30's `feel_numeric_eq` template.
- Operators (DMN-13): HIGH — arithmetic + date-time matrix cross-checked against Drools handbook examples; Decimal128 edge cases follow Phase 30 Pitfall 2.
- Control/quantified/boxed (DMN-14/15): HIGH — grammar rules verbatim; boxed-context topo-sort is a standard DFS; the flat-Scope mechanism is the user's locked choice.
- Lambdas (DMN-16): HIGH — grammar rule 55 verbatim; carrier extension is a natural Phase 30 `FeelFunction` evolution; non-external constraint from CONTEXT.md.
- Builtins (DMN-17): HIGH — ~84 functions enumerated across 8 families from Camunda + Red Hat references cross-checked against the spec; null-propagation rules and exceptions documented.
- AST cache (DMN-18): HIGH — `DashMap` legitimacy verified; IR wiring fields read from `model.rs`; type-level contract (`evaluate(&FeelExpression, ...)`) is a compile-time guarantee.
- Pitfalls: HIGH — each grounded in a spec rule, a Phase 30 pitfall, or a codebase line.

**Research date:** 2026-07-07
**Valid until:** 2026-08-06 (30 days — OMG DMN 1.5 spec is stable; `dashmap`/`regex` are mature. Re-verify `dashmap` 7.0.0-rc2 has a stable release before Phase 31 Wave 0, and re-confirm the TCK repo HEAD before Phase 35.)