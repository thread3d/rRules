//! FEEL expression AST — the `FeelExpression` enum and supporting types.
//!
//! This module is the **sole** AST the Phase 31 evaluator operates on. It is
//! distinct from the rule engine's `Expression` AST (the Rete path under
//! `engine/src/engine/`): the FEEL path is never routed through
//! `EngineRegistry` or the Rete evaluators (CONTEXT.md locked decision;
//! 31-RESEARCH.md Anti-Pattern 2). The FEEL path is three-valued, stateless,
//! and Decimal128-native; the Rete path is two-valued, forward-chaining, and
//! carries `f64`. Mixing them regresses v1.0–v5.0 rule execution.
//!
//! Plan 31-01 ships the enum skeleton + `Span` + `QuantKind` + `BinOpKind`.
//! Downstream plans fill in the evaluator (31-02..31-06), built-in registry
//! (31-07/31-08), and the AST cache (31-09).
//!
//! ## Why a single enum (not a `BoxedExpr` sub-enum)
//!
//! The user's locked choice (CONTEXT.md Grey Area 4) is a single
//! `FeelExpression` enum carrying *all* textual-grammar forms (rules 1–65)
//! *and* all 10 boxed-expression forms (DMN-15). An exhaustive match over
//! this enum is the safety net that catches missing arms at consumer sites
//! (Phase 30 Pattern 1; 31-RESEARCH.md Anti-Pattern 5 — the enum is NOT
//! marked `#[non_exhaustive]`).
//!
//! ## Carrier-shape note
//!
//! `FeelFunction` (in [`super::types`]) gains `param_names`, `body`, and
//! `captured` fields in this plan as a *data-model* decision so that 31-05
//! and 31-06 can populate them without re-declaring the carrier. Those fields
//! reference [`FeelExpression`] (via `Arc<FeelExpression>`) and the
//! [`super::eval::Scope`] stub; the circular module reference
//! (`types` ⇄ `eval` ⇄ `ast`) is broken with `Arc`/`Box` exactly as
//! `FeelRange` already uses `Box<FeelRange>` to break the `Value` cycle.

use super::types::FeelType;
use crate::data_types::Value;
use serde::{Deserialize, Serialize};

/// A source span — byte offset + length + 1-based line/column.
///
/// Carried on every `FeelExpression::Err` node so the Phase 34 frontend
/// editor can display per-cell parse errors while the rest of the table
/// loads (CONTEXT.md user override: error recovery, not fail-fast). Eval
/// errors in later plans also carry `Span` so the Phase 33 inference trace
/// can point at the failing expression.
///
/// 31-RESEARCH.md "Source-span representation": `start`/`len` are byte
/// offsets into the cell source (post-CDATA-strip, post-trim); `line`/`col`
/// are 1-based for editor display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Byte offset into the cell source where the span starts.
    pub start: usize,
    /// Byte length of the span (0-width spans mark insertion points).
    pub len: usize,
    /// 1-based line number (for editor display).
    pub line: u32,
    /// 1-based column number (for editor display).
    pub col: u32,
}

impl Span {
    /// Construct a zero-length span at `start` (an insertion-point marker).
    pub fn zero(start: usize, line: u32, col: u32) -> Self {
        Self {
            start,
            len: 0,
            line,
            col,
        }
    }
}

/// Quantifier kind for the [`FeelExpression::Quantified`] variant
/// (grammar rule 46: `some`/`every`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantKind {
    /// `some x in list satisfies pred` — existential; folds via `or_3vl`.
    Some,
    /// `every x in list satisfies pred` — universal; folds via `and_3vl`.
    Every,
}

/// Binary operator tag for the [`FeelExpression::BinOp`] variant.
///
/// Covers grammar rules 19–23 (arithmetic), 47–49 (boolean / comparison),
/// the `between`/`in` desugaring targets, and the `**` exponentiation that
/// is RIGHT-associative (31-RESEARCH.md Pitfall 2). The Pratt loop in
/// `parser.rs` keys off this enum's binding powers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOpKind {
    /// `or` (rule 47) — bp 10, left-assoc.
    Or,
    /// `and` (rule 48) — bp 20, left-assoc.
    And,
    /// `=` (rule 49) — bp 30, three-valued via `feel_equals`.
    Eq,
    /// `!=` (rule 49) — bp 30, negation of `=`.
    NotEq,
    /// `<` (rule 49) — bp 30, null-propagating via `feel_compare`.
    Lt,
    /// `<=` (rule 49) — bp 30.
    Le,
    /// `>` (rule 49) — bp 30.
    Gt,
    /// `>=` (rule 49) — bp 30.
    Ge,
    /// `+` (rule 19) — bp 40, left-assoc; also string concatenation when
    /// both operands are `String` (DMN-13).
    Add,
    /// `-` (rule 20) — bp 40, left-assoc.
    Sub,
    /// `*` (rule 21) — bp 50, left-assoc.
    Mul,
    /// `/` (rule 22) — bp 50, left-assoc; div-by-zero → `Value::Null`.
    Div,
    /// `**` (rule 23) — bp 60, **RIGHT-associative** (Pitfall 2:
    /// `2 ** 3 ** 2 = 2 ** 9 = 512`, not `8 ** 2 = 64`).
    Pow,
    /// `between` (rule 49) — desugars to `a >= b and a <= c`; the parser
    /// emits this tag and the evaluator performs the two comparisons joined
    /// by `and_3vl` (the `and` is NOT consumed as a boolean operator by
    /// the Pratt loop — Pitfall: greedy-`and` consumption).
    Between,
    /// `in` (rule 49) — `a in <positive unary test>` / `a in (tests)`;
    /// distinct from the `in` in `for`/`some`/`every` (rules 44/46), which
    /// is disambiguated by position (Pitfall 3).
    In,
}

/// The FEEL expression AST.
///
/// A single enum carrying all textual-grammar forms (rules 1–65) and all
/// 10 boxed-expression forms (DMN-15). Distinct from the rule engine's
/// `Expression` AST — the FEEL path never reaches `EngineRegistry`.
///
/// **NOT `#[non_exhaustive]`**: exhaustive match is the safety net that
/// catches missing arms at evaluator/built-in call-sites (31-RESEARCH.md
/// Anti-Pattern 5).
///
/// Recursive variants use `Box<Self>` (cheap move, breaks the
/// self-reference cycle at the type level); `FunctionDefinition.body` is
/// `Box<Self>` for the same reason. The `FeelFunction` carrier (in
/// `types.rs`) holds `Option<Arc<FeelExpression>>` for the closure body so
/// a cloned closure shares the AST cheaply (the AST cache in 31-09 also
/// stores `Arc<FeelExpression>`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeelExpression {
    // ----- Literals & names (rules 17, 25, 31, 32) -----
    /// A literal `Value` (numeric, string, boolean, null, date-time, …).
    /// Numeric literals are `Value::Number(Decimal)` — NEVER `f64`
    /// (prohibition 2; 31-RESEARCH.md Anti-Pattern 4).
    Literal(Value),
    /// A bare name reference (rule 25) — resolved against the flat `Scope`
    /// at eval time. May be a keyword-shaped string (`for`, `if`, `in`, …);
    /// disambiguation is the parser's job, not the AST's (Pitfall 3).
    Name(String),
    /// A qualified name `a.b.c` (rule 18) — resolved via the flat-Scope
    /// "longest-match" rule (31-RESEARCH.md Scope Model + QN Resolution).
    QualifiedName(Vec<String>),
    /// An `@"..."` at-literal (rule 65). Carries the *inner* string; the
    /// temporal dispatch to `feel::temporal::parse_at_literal` happens at
    /// evaluation time, not parse time (Phase 30 owns the parser).
    AtLiteral(String),

    // ----- Ranges (rules 7–12) -----
    /// A range `start..end` with inclusive/exclusive endpoints (rule 8).
    /// `Range { start, end, start_inclusive: true, end_inclusive: false }`
    /// is `[start..end)`. Both endpoints are `Box<FeelExpression>` so the
    /// AST is uniformly recursive.
    Range {
        /// Start endpoint expression.
        start: Box<FeelExpression>,
        /// End endpoint expression.
        end: Box<FeelExpression>,
        /// Whether the start endpoint is inclusive (`[`).
        start_inclusive: bool,
        /// Whether the end endpoint is inclusive (`]`).
        end_inclusive: bool,
        /// True for the singleton form `= value`, which the TCK distinguishes
        /// from the equivalent-but-differently-written `[value..value]`.
        point: bool,
    },

    // ----- Binary & unary operators (rules 19–24, 47–49) -----
    /// A binary operator application. The `op` tag (see [`BinOpKind`])
    /// determines the precedence/associativity at parse time and the
    /// three-valued-logic semantics at eval time.
    BinOp {
        /// The operator.
        op: BinOpKind,
        /// Left-hand operand.
        lhs: Box<FeelExpression>,
        /// Right-hand operand.
        rhs: Box<FeelExpression>,
    },
    /// Arithmetic negation `-expr` (rule 24). Distinct from `BinOp(Sub, 0, e)`
    /// so the evaluator can preserve the type of the inner expression.
    Neg(Box<FeelExpression>),

    // ----- Control + quantified (rules 44–46) -----
    /// `if cond then a else b` (rule 45). A null condition → `Value::Null`
    /// (31-RESEARCH.md recommendation, consistent with 3VL — verify at
    /// Phase 35).
    If {
        /// Condition.
        cond: Box<FeelExpression>,
        /// Then-branch.
        then: Box<FeelExpression>,
        /// Else-branch.
        else_: Box<FeelExpression>,
    },
    /// `for x in iter1, y in iter2 return body` (rule 44). Multiple
    /// iteration contexts (rule 63) are nested cartesian-product-style; the
    /// body is evaluated once per combination and results are collected
    /// into `Value::Array`.
    For {
        /// `(iter_var, iteration_source)` pairs — the iteration source is
        /// itself a `FeelExpression` (a list or an `a..b` range).
        iters: Vec<(String, FeelExpression)>,
        /// The body, evaluated per iteration with the iter vars bound.
        body: Box<FeelExpression>,
    },
    /// `some x in list satisfies pred` / `every x in list satisfies pred`
    /// (rule 46). `Some` folds via `or_3vl`; `Every` folds via `and_3vl`.
    Quantified {
        /// `Some` or `Every`.
        kind: QuantKind,
        /// `(iter_var, iteration_source)` pairs.
        iters: Vec<(String, FeelExpression)>,
        /// The predicate, evaluated per element.
        satisfies: Box<FeelExpression>,
    },

    // ----- Path, filter, instance-of (rules 43, 50, 51) -----
    /// `target.name` (rule 43). Path navigation: into a context
    /// (`.foo` → `get value`) or a list (`.foo` → map over elements).
    /// Disambiguated from `QualifiedName` by position (rule 18 vs 43).
    Path {
        /// The navigation target.
        target: Box<FeelExpression>,
        /// The path component name.
        name: String,
    },
    /// `target[predicate]` (rule 50). For a list LHS, filter to elements
    /// where the predicate is true. For a non-list LHS, filter on a
    /// singleton list. `null[?…]` → `null`.
    Filter {
        /// The list/context being filtered.
        target: Box<FeelExpression>,
        /// The filter predicate.
        predicate: Box<FeelExpression>,
    },
    /// `expr instance of T` (rule 51). Eval calls
    /// `feel::types::conforms(type_of(value), T)` where `T` is parsed via
    /// `resolve_type_ref` (extended for recursive `list<…>`/`context<…>`/
    /// `function<…>`/`range<…>` forms in 31-04).
    InstanceOf {
        /// The expression whose value's type is checked.
        expr: Box<FeelExpression>,
        /// The target type lattice node.
        ty: FeelType,
    },

    // ----- 10 boxed expressions (DMN-15) -----
    /// `[e1, e2, …]` (rule 54). Recursive in the elements.
    List(Vec<FeelExpression>),
    /// A relation — DMN XML `<relation>` form (no textual grammar form).
    /// Recursive in the cell expressions of each row.
    Relation {
        /// Column names.
        columns: Vec<String>,
        /// Rows; each row is a `Vec<FeelExpression>` of length `columns.len()`.
        rows: Vec<Vec<FeelExpression>>,
    },
    /// `{ k1: e1, k2: e2, … }` (rule 57). The evaluator (31-05) evaluates
    /// entries in **dependency (topological) order** with DFS color-marking
    /// cycle detection, using the flat `Scope.deps` map.
    Context(Vec<(String, FeelExpression)>),
    /// `callee(args)` (rule 38). `named` carries `name: value` pairs for
    /// named-parameter invocation; positional args are in `args`. The
    /// callee resolves to a `Value::Function` (built-in or user-defined
    /// lambda) at eval time.
    FunctionInvocation {
        /// The callable expression (usually a `Name` or `Path`).
        callee: Box<FeelExpression>,
        /// Positional arguments.
        args: Vec<FeelExpression>,
        /// Optional named arguments (`name: value`).
        named: Option<Vec<(String, FeelExpression)>>,
    },
    /// `function(p1: T1, p2: T2) body` (rule 55). `external: true` is
    /// parsed but the evaluator REJECTS it (CONTEXT.md: non-external only)
    /// — evaluating an external `FunctionDefinition` yields `Value::Null` +
    /// a logged warning. The closure body and captured scope live on the
    /// `FeelFunction` carrier (populated in 31-05/31-06).
    FunctionDefinition {
        /// `(name, optional type annotation)` pairs.
        params: Vec<(String, Option<FeelType>)>,
        /// The closure body.
        body: Box<FeelExpression>,
        /// Whether the `external` keyword was present (rule 55).
        external: bool,
    },

    // ----- Error recovery (CONTEXT.md user override) -----
    /// A parse error carrying a [`Span`] and a message. The parser emits
    /// `Err` nodes rather than failing fast so the Phase 34 frontend
    /// editor can display per-cell errors while the rest of the table
    /// loads (31-RESEARCH.md Parser Error Recovery). The evaluator maps
    /// `Err(_, _)` to `Value::Null`.
    Err(Span, String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    /// Smoke test: the headline variants construct and the enum is not
    /// `#[non_exhaustive]` (exhaustive match is the safety net).
    #[test]
    fn feel_expression_variants_construct() {
        let _lit = FeelExpression::Literal(Value::Number(dec!(5)));
        let _name = FeelExpression::Name("x".to_string());
        let _qn = FeelExpression::QualifiedName(vec!["a".to_string(), "b".to_string()]);
        let _at = FeelExpression::AtLiteral("2021-01-31".to_string());
        let _range = FeelExpression::Range {
            start: Box::new(FeelExpression::Literal(Value::Number(dec!(1)))),
            end: Box::new(FeelExpression::Literal(Value::Number(dec!(10)))),
            start_inclusive: true,
            end_inclusive: false,
            point: false,
        };
        let _binop = FeelExpression::BinOp {
            op: BinOpKind::Add,
            lhs: Box::new(FeelExpression::Literal(Value::Number(dec!(1)))),
            rhs: Box::new(FeelExpression::Literal(Value::Number(dec!(2)))),
        };
        let _neg = FeelExpression::Neg(Box::new(FeelExpression::Literal(Value::Number(dec!(1)))));
        let _if = FeelExpression::If {
            cond: Box::new(FeelExpression::Literal(Value::Bool(true))),
            then: Box::new(FeelExpression::Literal(Value::Number(dec!(1)))),
            else_: Box::new(FeelExpression::Literal(Value::Number(dec!(2)))),
        };
        let _for = FeelExpression::For {
            iters: vec![(
                "x".to_string(),
                FeelExpression::Literal(Value::Number(dec!(1))),
            )],
            body: Box::new(FeelExpression::Name("x".to_string())),
        };
        let _some = FeelExpression::Quantified {
            kind: QuantKind::Some,
            iters: vec![(
                "x".to_string(),
                FeelExpression::Literal(Value::Number(dec!(1))),
            )],
            satisfies: Box::new(FeelExpression::Literal(Value::Bool(true))),
        };
        let _path = FeelExpression::Path {
            target: Box::new(FeelExpression::Name("a".to_string())),
            name: "b".to_string(),
        };
        let _filter = FeelExpression::Filter {
            target: Box::new(FeelExpression::Name("a".to_string())),
            predicate: Box::new(FeelExpression::Literal(Value::Bool(true))),
        };
        let _iof = FeelExpression::InstanceOf {
            expr: Box::new(FeelExpression::Literal(Value::Number(dec!(5)))),
            ty: FeelType::Number,
        };
        let _list = FeelExpression::List(vec![FeelExpression::Literal(Value::Number(dec!(1)))]);
        let _rel = FeelExpression::Relation {
            columns: vec!["x".to_string()],
            rows: vec![vec![FeelExpression::Literal(Value::Number(dec!(1)))]],
        };
        let _ctx = FeelExpression::Context(vec![(
            "a".to_string(),
            FeelExpression::Literal(Value::Number(dec!(1))),
        )]);
        let _inv = FeelExpression::FunctionInvocation {
            callee: Box::new(FeelExpression::Name("f".to_string())),
            args: vec![FeelExpression::Literal(Value::Number(dec!(1)))],
            named: None,
        };
        let _fdef = FeelExpression::FunctionDefinition {
            params: vec![("a".to_string(), None)],
            body: Box::new(FeelExpression::Name("a".to_string())),
            external: false,
        };
        let _err = FeelExpression::Err(Span::zero(0, 1, 1), "expected expression".to_string());
    }

    /// `Span` is `Copy` + `Eq` — required for cheap error-site propagation
    /// and for the Phase 34 editor's per-cell span comparison.
    #[test]
    fn span_is_copy_eq() {
        let s = Span {
            start: 3,
            len: 2,
            line: 1,
            col: 4,
        };
        let copy = s;
        assert_eq!(s, copy);
    }

    /// `BinOpKind` covers every binary operator in grammar rules 19–23, 47–49.
    #[test]
    fn binop_kind_has_all_operators() {
        let _ = BinOpKind::Or;
        let _ = BinOpKind::And;
        let _ = BinOpKind::Eq;
        let _ = BinOpKind::NotEq;
        let _ = BinOpKind::Lt;
        let _ = BinOpKind::Le;
        let _ = BinOpKind::Gt;
        let _ = BinOpKind::Ge;
        let _ = BinOpKind::Add;
        let _ = BinOpKind::Sub;
        let _ = BinOpKind::Mul;
        let _ = BinOpKind::Div;
        let _ = BinOpKind::Pow;
        let _ = BinOpKind::Between;
        let _ = BinOpKind::In;
    }

    /// `QuantKind` covers both quantifier keywords.
    #[test]
    fn quant_kind_some_every() {
        let _ = QuantKind::Some;
        let _ = QuantKind::Every;
        assert_ne!(QuantKind::Some, QuantKind::Every);
    }
}
