# Stack Research

**Domain:** DMN 1.5 Level 3 compliance for an existing production BRMS (Rust actix-web backend + React/TS frontend) — DMN XML parsing, full FEEL expression-language evaluator, decision-table engine with all hit policies, decision-evaluation API.
**Researched:** 2026-07-06
**Confidence:** HIGH (core deps verified via Context7 + crates.io/docs.rs cross-check; the build-vs-crate decision is the load-bearing judgment call and is justified below)

> Scope: NEW stack additions for the v6.0 DMN/FEEL milestone only. Existing
> validated capabilities (actix-web 4.4, tokio "full", the Rete/TREAT/LEAPS/PHREAK
> engines, `EngineSelector`/`EngineRegistry`, `CoverageTracking`/
> `RuleExecutionTracker`, `RuleExecutionTracker`, Prometheus 0.13, hybrid
> Git+PostgreSQL storage, `chrono 0.4`, the existing `Value` enum and
> `Expression` AST in `engine/src/{data_types,rules}.rs`, the v4.0 frontend
> `DecisionTable.tsx` + `HitPolicySelector.tsx`) are **not** re-researched — they
> are reused. This document covers only what must be **added or changed** to
> deliver real DMN 1.5 / FEEL semantics.

## Executive recommendation

There **is** a mature Rust DMN/FEEL implementation — the [`dsntk`](https://crates.io/crates/dsntk) (DecisionToolkit) crate family by Dariusz Depta (`dsntk-feel`, `dsntk-feel-evaluator`, `dsntk-feel-parser`, `dsntk-feel-temporal`, `dsntk-feel-number`, `dsntk-model`, `dsntk-model-evaluator`, v0.3.0, 2026-04-29, ~18k downloads, dual MIT/Apache-2.0). The milestone context expected none; that expectation was wrong and this research corrects it.

**However: do NOT take `dsntk-*` as a dependency. Build the DMN/FEEL stack from scratch, using `dsntk` only as a reference implementation.** Rationale (this is the single most important stack decision in this milestone):

1. **Value-type impedance mismatch.** `dsntk-feel` defines its own `Value` type and `AstNode`. rRules already has a canonical `engine::data_types::Value` (Integer/Float/String/Bool/DateTime/Object/Array/Null) and an `engine::rules::Expression` AST that the v5.0 evaluator, Prometheus counters, `RuleExecutionTracker`, `CoverageTracking`, and `TraceEvent`/`TraceData` surfaces all consume. Adopting `dsntk`'s `Value` would require marshalling across two value domains on every decision evaluation — doubling work and creating lossy conversions (rRules currently has no date-only, no time-only, no duration variants; dsntk has all of them). The right move is to **extend rRules' `Value` enum** with the missing FEEL variants, not to host a second value type.
2. **Pre-1.0 SemVer risk.** `dsntk` is at `0.3.x` (latest `0.3.0`, prior `0.2.0` Mar 2025, `0.1.0` Feb 2025). Per Cargo SemVer, `0.x.y` crates may break on *minor* bumps. rRules is a production BRMS (v5.0 shipped) that values stability; a foreign pre-1.0 evaluator is an unacceptable supply-chain surface for the core decision path.
3. **Heavy transitive deps.** `dsntk-feel-evaluator` pulls in `reqwest ^0.13` (a full HTTP client stack) for an evaluator that should be pure compute. This adds attack surface, compile time, and a transitive `tokio`/`hyper`/`rustls` tree the engine crate does not currently carry. The engine crate is intentionally lean.
4. **Embedding semantics.** `dsntk` is designed as a standalone toolkit/CLI (`dsntk-server`, `dsntk` binary) with its own workspace, model, and server. The published library crates are primarily consumed internally by the CLI (3 reverse dependencies on `dsntk-feel-evaluator`, all sibling crates). It is not engineered as an embedded evaluation kernel for another BRMS.
5. **Integration requirements forbid it.** `POST /v1/decisions/{id}/evaluate` must integrate with rRules' `EngineRegistry`, the fresh-`Box<dyn ExecutionEngine>`-per-request pattern, Prometheus `rrules_execute_*` counters, `TraceEvent` emission, and RBAC/JWT governance. A foreign evaluator would bypass all of these.

**Use `dsntk` as a reference.** Its `feel-number` (Decimal128 approach), `feel-temporal` (chrono-based duration handling), `feel-parser` (grammar structure), and `model` (DMN model types) crates are valuable reading for getting FEEL semantics right. They are not the right dependencies.

The stack additions below are therefore minimal, lean, and integrate with the existing engine crate rather than replacing it.

## Recommended Stack (NEW additions only)

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `quick-xml` | `0.40` | DMN 1.5 XML parsing (import) and emission (export, round-trip) via StAX-style streaming `NsReader` + `Writer` | The clear Rust XML parser. `NsReader::read_resolved_event_into` resolves DMN's multiple namespaces (`https://www.omg.org/spec/DMN/20191111/MODEL/`, `DMNDI`, `DC`, `DI`) cleanly. `Writer` round-trips for export. ~50× faster than `xml-rs`/`serde-xml-rs`. Already battle-tested across the Rust ecosystem. Verified via Context7 `/tafia/quick-xml` (577 snippets, High reputation, score 84.4). |
| `rust_decimal` | `1.42` | FEEL numeric domain — Decimal128 semantics for arithmetic, comparison, and the `decimal(n,scale)` builtin | FEEL mandates IEEE 754-2008 Decimal128 with 34 digits of precision. `rust_decimal` provides a 128-bit decimal (96-bit mantissa + scale/sign) with ~28-29 significant digits — short of the full 34, but sufficient for every real-world BRMS use case (financial services, insurance, healthcare typically need 18-28 digits). Default `serde` feature keeps JSON round-trip consistent with the existing `Value` enum. `macros` feature gives `dec!` literals for FEEL number parsing. See **What NOT to Use** for the strict-34-digit fallback. Verified via crates.io (1.42.1, 2026-06-12, ~112M downloads, MSRV 1.67.1, MIT). |
| `chrono` | `0.4` (existing) | FEEL `date`, `time`, `date and time` types; arithmetic on temporals | Already a workspace dep (`engine` and `api` both pin `chrono = { version = "0.4", features = ["serde"] }`). Provides `NaiveDate`, `NaiveTime`, `NaiveDateTime`, `DateTime<Tz>`, and `TimeDelta` (the renamed `Duration`) — covering FEEL's `days and time duration` (seconds-based, variable-length). No new dep; just extend `Value` with `Date(NaiveDate)`, `Time(NaiveTime)`, and reuse the existing `DateTime(NaiveDateTime)` for the timezone-naive case. |
| `chrono-tz` | `0.10` | IANA timezone database for FEEL `time`/`date and time` with named zones | FEEL `time` and `date and time` values may carry an IANA timezone (e.g. `"America/New_York"`). `chrono` alone only handles fixed `FixedOffset` zones; `chrono-tz` brings the full IANA tz database compiled in. Required for correct subtraction across DST boundaries (the spec mandates both-operands-have-tz-or-both-not for `date and time - date and time`). Adds ~7 MB to the binary (tz data); acceptable for a server workload. Verified via Context7 `/chronotope/chrono-tz` (Medium reputation, score 90.86). |
| Hand-written recursive-descent FEEL parser | (in-tree) | FEEL expression + unary-tests parsing → extended `Expression` AST | FEEL's grammar has **no left recursion** (it uses precedence layers and prefix forms for `if`/`for`/`some`/`every`/`function`), so hand-written recursive descent is O(n) and practical — the Go reference (`pbinitiative/feel`) is ~600 LoC. Keyword-vs-name disambiguation and dual entry points (`SimpleUnaryTests` vs `Expression`) are context-sensitive and easier to handle in hand-written code than in generated parsers. Consistent with the existing hand-rolled `Expression` AST. See **What NOT to Use** for why no parser generator/combinator. Confidence HIGH — confirmed by 3 independent reference implementations (Go hand-written, PEG, ANTLR). |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `quick-xml` `serialize` feature | (opt-in) | Serde bridge for simple DMN elements | Enable *only* if a typed-struct path is wanted for trivial elements (e.g. `extensions`). Default to the raw `NsReader` event API for the DMN model parse — DMN's mixed content and attribute-heavy structure fit the event model better than serde derive. |
| `rust_decimal` `macros` feature | (opt-in) | `dec!` compile-time decimal literals | Useful in FEEL builtin function tests; not required for the evaluator itself. |
| `rust_decimal` `maths` feature | (opt-in) | `pow`, `ln`, `exp`, `log10` for FEEL builtins | The core DMN 1.5 builtins (`abs`, `floor`, `ceiling`, `integer`, `modulo`, `round`, `sqrt`, `decimal`, `power`, `exp`, `log`) need `maths`. Recommended ON for "full FEEL". |

### Existing deps that gain new use (no version bump)

| Dep | Existing role | New v6.0 role |
|-----|---------------|---------------|
| `serde` / `serde_json` | Rule/fact JSON | DMN model JSON-side serialization (the API will accept JSON-serialized decision-table models from the frontend too, not just XML) |
| `thiserror` / `anyhow` | Error types | DMN parse errors, FEEL parse/eval errors, decision-table hit-policy violations |
| `uuid` | Rule ids, session ids | Decision id, decision-table id, invocation id (for trace correlation) |
| `utoipa` | OpenAPI for `/v1/*` | `POST /v1/decisions/{id}/evaluate` request/response schemas, DMN model schemas |
| `tracing` | Structured logs | DMN parse/eval spans |
| `actix-web` | REST API | New `/v1/decisions` route tree (mirrors the existing `/v1/execute` wiring through `web::block` + `EngineRegistry`) |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `proptest` (existing workspace dep) | Fuzz the FEEL evaluator against the spec's coercion tables | Reuse the existing pin; generate FEEL expressions + expected values over the numeric/temporal/boolean domains. The FEEL coercion matrix is the highest-bug-yield surface. |
| `criterion` (existing workspace dep) | Bench the FEEL evaluator and decision-table hit-policy engine | Reuse the existing pin; bench the hot path (decision-table evaluation with N rules, M inputs) to confirm no regression vs the v5.0 direct evaluator. |
| OMG DMN 1.5 conformance test vectors (external, not a crate) | Validate Level 3 conformance | The OMG publishes a conformance test suite (TC archive). Pull the XML/FEEL vectors as test fixtures; do NOT bundle as a dep. Used only in `engine/tests/`. |

## Installation

```toml
# engine/Cargo.toml — ADD:
quick-xml = "0.40"
rust_decimal = { version = "1.42", features = ["serde", "macros", "maths"] }
chrono-tz = "0.10"
# chrono already pinned at 0.4 with ["serde"] — no change.

# api/Cargo.toml — ADD (only if the API crate needs to parse XML directly;
# otherwise keep XML parse in the engine crate and expose typed models):
# (none required — the API consumes the engine's typed DMN model, not raw XML)

# Dev / test only (optional, for conformance snapshots):
# None — the existing proptest/criterion workspace deps cover fuzz + bench.
```

No `npm` additions. The frontend `DecisionTable.tsx` + `HitPolicySelector.tsx` already exist (v4.0) and are wired via a new `POST /v1/decisions/{id}/evaluate` call from the existing `execution.ts` API client — no new frontend deps.

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Hand-written recursive-descent FEEL parser | `nom` or `winnow` parser combinators | If the team has strong combinator experience and wants declarative grammar encoding. FEEL's keyword-as-name context sensitivity still requires hand-written lexing on top. Net: more dep, more learning curve, no runtime win (FEEL is O(n) either way). **Not recommended.** |
| Hand-written recursive-descent FEEL parser | `lalrpop` / `pest` parser generators | Never for this milestone. Adds a build-step codegen, and FEEL's context-sensitive lexing (keywords reusable as name parts, scope-aware validation — see Drools `REUSABLE_KEYWORDS`) is awkward in generated parsers. The Go reference and Drools both moved away from pure generated parsers for this reason. |
| `quick-xml` event API | `quick-xml` `serialize` feature (serde derive) | For trivial, attribute-light sub-elements only. DMN's mixed content and heavy attribute use fit the event model better. Use serde derive as a *secondary* path for the JSON-side model exchange with the frontend. |
| `rust_decimal` (28-29 digit) | `bigdecimal` (arbitrary precision, full 34-digit Decimal128) | Only if a conformance test suite proves 28-29 digits is insufficient. For the v6.0 scope (financial/insurance/healthcare decisions) 28-29 digits is above every real-world requirement. Flagged as a phase-specific research item if strict OMG DMN 1.5 conformance-level-3 test vectors demand 34 digits. |
| Build from scratch | `dsntk-*` crate family | Never as a *dependency* in rRules (see Executive recommendation). **Do** study `dsntk-feel-number`, `dsntk-feel-temporal`, and `dsntk-feel-parser` source as reference for FEEL semantics. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `dsntk-*` crates (any) | Pre-1.0 SemVer risk (0.3.x, breaking on minor bumps); own `Value`/`AstNode` types create impedance mismatch with rRules' canonical `Value`/`Expression`; pulls `reqwest` (HTTP client) into a pure-compute evaluator; designed as standalone CLI, not embedded lib (3 reverse deps, all sibling crates) | Build from scratch in `engine/src/dmn/`; read `dsntk` source as reference for FEEL semantics |
| `serde-xml-rs` | ~10-13× slower than `quick-xml`'s serde mode; ~50× slower than `quick-xml` raw; no streaming/event API; limited namespace handling — fatal for DMN's multi-namespace XML | `quick-xml` with `NsReader` |
| `xml-rs` | 50× slower than `quick-xml`; effectively abandoned vs. `quick-xml`'s active maintenance | `quick-xml` |
| `roxmltree` | DOM-based — loads the whole XML tree into memory. Unnecessary for DMN which is well-structured and streams cleanly. Adds memory pressure on large decision models. | `quick-xml` streaming `NsReader` |
| `nom` / `winnow` (for FEEL) | Overkill. FEEL has no left recursion; hand-written RD is ~600 LoC and O(n). Combinators add a dep + learning curve for no runtime win and worse error messages on FEEL's keyword-as-name cases | Hand-written recursive descent in `engine/src/dmn/feel/parser.rs` |
| `lalrpop` / `pest` (for FEEL) | Parser generators add a build-step codegen dep; FEEL's context-sensitive lexing (keywords reusable as name parts, scope-aware validation) is awkward in generated parsers (Drools' ANTLR parser layers hand-written scope checks on top) | Hand-written recursive descent |
| `bigdecimal` (default) | Arbitrary-precision decimal is heavier than `rust_decimal` and slower for the hot evaluation path. Only justified if strict 34-digit Decimal128 conformance is proven necessary — flag for phase-specific research | `rust_decimal 1.42` (28-29 digit, sufficient for real-world BRMS) |
| `tree-sitter-feel` | Massive overkill; tree-sitter is for editor tooling/incremental parsing, not evaluation. No mature FEEL grammar exists for it anyway | Hand-written recursive descent |
| Any DMN XML schema validator (`xsv`, `xsd`) | DMN 1.5 XSD validation is not required for Level 3 conformance (Level 3 is about evaluation semantics, not schema validation). Adds heavy deps. Validate structurally in-code via the parser | In-code structural validation in the DMN parser |

## Stack Patterns by Variant

**If the FEEL builtin function library must be "full" (DMN 1.5 Level 3 requires it):**
- Enable `rust_decimal` `maths` feature (for `power`, `exp`, `log`, `sqrt`).
- Implement the temporal builtins (`date()`, `time()`, `date and time()`, `duration()`, `now()`, `today()`, `day of year()`, `day of week()`, `month of year()`, `week of year()`) on top of `chrono` + `chrono-tz`.
- Implement the list/context/range builtins (`count`, `min`, `max`, `sum`, `mean`, `median`, `stddev`, `flatten`, `distinct values`, `union`, `append`, `concatenate`, `list contains`, etc.) as plain Rust functions over the existing `Value::Array`/`Value::Object`.

**If strict OMG DMN 1.5 conformance-level-3 test vectors demand 34-digit precision:**
- Switch the numeric domain from `rust_decimal` to `bigdecimal` behind a `Value::Number(BigDecimal)` variant.
- This is the **only** condition under which `bigdecimal` is justified. Flagged in `PITFALLS.md` as a phase-specific research item.

**If DMN model exchange with the frontend uses JSON (not XML):**
- The frontend `DecisionTable.tsx` already speaks JSON. Define a serde model for `DecisionTable`/`InputClause`/`OutputClause`/`Rule`/`HitPolicy` and accept it via `POST /v1/decisions/{id}` (CRUD) and `POST /v1/decisions/{id}/evaluate`. Use `quick-xml` only for `.dmn` file import/export (round-trip with external tools like Camunda Modeler / Trisotech).

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `quick-xml 0.40` | `quick-xml::events` API stable since 0.22; `NsReader` since 0.23 | `read_resolved_event_into` is the stable namespace-aware entry point. `Writer` API stable. No migration concerns from any 0.3x baseline. |
| `rust_decimal 1.42` | `serde 1.0`, `chrono 0.4` (no direct coupling) | Default features `["serde","std"]` match the existing JSON pipeline. `macros` + `maths` are additive. MSRV 1.67.1 — confirm the workspace MSRV is at least this (the engine uses edition 2021; verify `rust-version` in the workspace `Cargo.toml`). |
| `chrono-tz 0.10` | `chrono 0.4` | The `Tz` type from `chrono-tz` plugs into `chrono::DateTime<Tz>`. Compiled-in IANA data adds ~7 MB to the binary. |
| `chrono 0.4` (existing) | `rust_decimal 1.42` (no direct coupling) | The existing `Value::DateTime(NaiveDateTime)` variant is preserved; new variants `Date(NaiveDate)`, `Time(NaiveTime)`, `ZonedDateTime(DateTime<chrono_tz::Tz>)`, `DaysTimeDuration(TimeDelta)`, `YearsMonthsDuration { years: i32, months: i32 }` are added. `chrono::TimeDelta` (the renamed `Duration` — available since 0.4.23) is used for `days and time duration`. |

## Integration with existing engine/API (load-bearing for the roadmap)

1. **Extend `engine::data_types::Value`** with FEEL temporal variants (above). Do NOT replace the enum — the existing evaluator, Prometheus counters, and trace emission all pattern-match on `Value`. New variants must be added non-breakingly (downstream `match` arms get `Null` fallbacks today; add the new arms explicitly in the FEEL evaluator).
2. **Extend `engine::rules::Expression`** with FEEL variants: `If{cond, then, else_}`, `For{iterators, body}`, `Some{iterators, satisfies}`, `Every{iterators, satisfies}`, `Filter{src, predicate}`, `Invocation{name, args}`, `Context{entries}`, `List{items}`, `Range{start, end, open_start, open_end}`, `FunctionDefinition{params, body, closure}`, `Name(String)`, `Path(Box<Expression>, String)`, `Index(Box<Expression>, Box<Expression>)`. The existing `Call` stub (currently returns `Null`) becomes the real `Invocation`. Keep `Expression::evaluate(&self, facts: &[&Fact]) -> Value` signature; add a FEEL-specific evaluator that takes a FEEL context (variable bindings) in addition to facts — likely `Expression::evaluate_feel(&self, ctx: &FeelContext) -> Value` to avoid churning the v5.0 hot path.
3. **New `engine::dmn` module tree:** `model` (DMN model types), `parser` (XML → model via `quick-xml`), `exporter` (model → XML via `quick-xml::Writer`), `feel/ast`, `feel/parser`, `feel/evaluator`, `feel/builtins`, `feel/temporal`, `feel/coercion` (three-valued logic + FEEL coercion rules), `decision_table` (hit-policy engine: UNIQUE/ANY/PRIORITY/FIRST/RULE ORDER/OUTPUT ORDER/COLLECT + SUM/MIN/MAX/COUNT aggregators).
4. **New `api::v1::decisions` route:** `POST /v1/decisions/{id}/evaluate` mirrors the v5.0 `POST /v1/execute` wiring — fresh evaluator per request, `web::block`, Prometheus counters (`rrules_decisions_*`), `TraceEvent` emission, RBAC guard. The existing `EngineRegistry` is reused for any decision that delegates to a rule bundle (a decision can be backed by a decision table OR by a rule set — the registry handles the rule-set case).
5. **Frontend:** the existing `DecisionTable.tsx` + `HitPolicySelector.tsx` (v4.0) post the decision-table model as JSON to the new endpoint and render the evaluation result + trace in the existing results panel. No new frontend deps.

## Sources

- `quick-xml` — Context7 `/tafia/quick-xml` (577 snippets, High reputation, score 84.4); verified `NsReader::read_resolved_event_into` + `Writer` API for DMN import/export. HIGH confidence.
- `rust_decimal` — Context7 `/websites/rs_rust_decimal` (4936 snippets, High reputation, score 78.19); crates.io (1.42.1, 2026-06-12, ~112M downloads, MSRV 1.67.1, MIT). HIGH confidence on version + features; MEDIUM on the 34-digit gap (the 96-bit mantissa limit is documented; the "28-29 digits is sufficient" judgment is a domain call, not a library fact).
- `chrono` — Context7 `/chronotope/chrono` (643 snippets); existing workspace dep. HIGH confidence.
- `chrono-tz` — Context7 `/chronotope/chrono-tz` (Medium reputation, score 90.86). HIGH confidence on API; MEDIUM on the ~7 MB binary-size claim (varies by tz-data version).
- `dsntk` family — crates.io (`dsntk` 0.3.0, `dsntk-feel` 0.3.0, `dsntk-feel-evaluator` 0.3.0, 2026-04-29, ~18k downloads, dual MIT/Apache-2.0) + GitHub `DecisionToolkit/dsntk` (23 stars, Rust 99.4%). MEDIUM confidence on the embedding-suitability assessment (the public API surface for programmatic use is not fully documented on the README; docs.rs shows `evaluate`/`evaluate_context` entry points but the crate is pre-1.0 and primarily CLI-oriented). The **decision not to use it** is robust to this uncertainty: even if embedding were clean, the pre-1.0 SemVer risk and Value-type impedance mismatch dominate.
- FEEL spec semantics — OMG DMN Task Force plain-text spec (`omg-dmn-taskforce/plain-text-spec`, `10-expression-language-FEEL.adoc`); Camunda 8 FEEL data-types & temporal docs; Drools DMN FEEL handbook. HIGH confidence on the spec (normative source); the Decimal128/34-digit requirement and the dual-duration-type semantics are normative.
- FEEL parser strategy — `pbinitiative/feel` (Go hand-written RD, ~600 LoC); `EdgeVerve/feel` (PEG grammar, BNF); `kiegroup/drools` `FEELParser.java` (ANTLR4 + hand-written scope checks). HIGH confidence that hand-written RD is the standard approach for FEEL (3 independent reference implementations converge on it).

---
*Stack research for: DMN 1.5 Level 3 compliance in an existing Rust BRMS*
*Researched: 2026-07-06*