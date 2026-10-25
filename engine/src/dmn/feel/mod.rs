//! FEEL (Friendly Enough Expression Language) module — sibling to
//! [`crate::dmn::model`], [`crate::dmn::parser`], and [`crate::dmn::exporter`].
//!
//! Phase 30 landed the value-domain foundation; Phase 31 builds the parser,
//! evaluator, and built-in function library on top:
//!
//! - [`types`] — the `FeelType` enum (the DMN 1.5 type lattice nodes) and the
//!   `FeelRange` / `FeelFunction` carrier structs used by
//!   [`crate::data_types::Value`]'s `Range` and `Function` variants. Lattice
//!   relations (`conforms`, `equivalent`, `implicit_convert`,
//!   `resolve_type_ref`) landed in 30-04. Phase 31 extends `FeelFunction`
//!   with `param_names`/`body`/`captured` fields (31-01 data-model decision;
//!   populated by 31-05/31-06).
//! - [`temporal`] — the five FEEL temporal carrier types: `TimeValue`,
//!   `DateTimeValue` (carrying fixed-offset, IANA-zone, or local forms), and
//!   `YearsMonthsDuration`. `@""` parse/format and DST-correct duration
//!   arithmetic helpers landed in 30-03; Phase 31 reuses them for DMN-13.
//! - [`numeric`] — the FEEL Decimal128 numeric helpers (`feel_numeric_eq`,
//!   `0.1 + 0.2 = 0.3`); the `Option<bool>` null-propagating template for
//!   the Phase 31 three-valued-logic operators (30-02).
//! - [`ast`] — the `FeelExpression` enum + `Span` + `QuantKind` +
//!   `BinOpKind` (Phase 31-01). Distinct from the rule engine's
//!   `Expression` AST; the FEEL path is never routed through
//!   `EngineRegistry` / the Rete evaluators under `engine/src/engine/`.
//! - [`parser`] — the hand-written recursive-descent + Pratt-precedence
//!   parser (grammar rules 1–65), dual entry points
//!   `parse_expression` / `parse_simple_unary_tests`, keyword-as-name
//!   disambiguation, error recovery (Phase 31-01).
//! - [`eval`] — the evaluator: `evaluate(&FeelExpression, &Scope) -> Value`,
//!   private `cmp` module (`feel_equals`/`feel_compare` → `Option<_>`),
//!   explicit `and`/`or`/`not` truth tables, `Scope` flat-map + deps
//!   (Phase 31-02 onwards).
//! - [`builtins`] — the `BuiltinFn` type + `HashMap<&str, BuiltinFn>`
//!   registry (~70+ functions across 8 families — Phase 31-07/31-08).
//! - [`coercion`] — the explicit FEEL operator/built-in coercions (distinct
//!   from the Phase 30 lattice `implicit_convert`; Phase 31-03).
//! - [`cache`] — the `DashMap<String, Arc<FeelExpression>>` AST cache +
//!   IR-loader pre-parse wiring (Phase 31-09).
//!
//! ## Four prohibitions (CONTEXT.md locked, enforced in each sub-module)
//!
//! 1. **Never reuse Rust `==`/`Ord`/`compare_values`** for FEEL
//!    equality/ordering — the private `feel::eval::cmp` module is the sole
//!    home of `feel_equals`/`feel_compare` (returns `Option<_>`).
//! 2. **Never use `f64`** in the FEEL production path — numeric literals
//!    construct `Value::Number(Decimal)` (`rust_decimal`, never `f64`).
//! 3. **No implicit number↔string conversion** — that is the explicit
//!    `string()` / `number()` built-ins (31-07), NOT a lattice or operator
//!    coercion (`1 + "2"` → `Value::Null`).
//! 4. **The FEEL path is never routed through `EngineRegistry` /
//!    `engine/src/engine/`** — `FeelExpression` is distinct from the rule
//!    engine's `Expression` AST; mixing them regresses v1.0–v5.0 rule
//!    execution and violates the stateless/three-valued vs
//!    forward-chaining/two-valued separation.

pub mod ast;
pub mod builtins;
pub mod cache;
pub mod coercion;
pub mod eval;
pub mod numeric;
pub mod parser;
pub mod temporal;
pub mod types;

// Re-exports for the IR loader (Phase 29 `engine/src/dmn/parser.rs`) and
// downstream consumers. The cache is a sidecar to the DMN model — it lives in
// `feel::cache` but the IR loader needs it accessible as `feel::FeelAstCache`.
pub use cache::{FeelAstCache, ParseEntry};
// (sub-module wiring — `pub mod ast; pub mod parser; pub mod eval; pub mod builtins; pub mod coercion; pub mod cache;` declared above alongside the Phase 30 `numeric`/`temporal`/`types`.)
