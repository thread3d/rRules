//! FEEL evaluator — `evaluate(&FeelExpression, &Scope) -> Value`.
//!
//! Plan 31-02 owns the real implementation: the private `cmp` module
//! (`feel_equals` / `feel_compare` returning `Option<_>`, the explicit
//! `and_3vl`/`or_3vl`/`not_3vl` truth tables — Pitfall 1 mitigation), the
//! `Scope` flat-map + `deps` type, and the dispatch over
//! [`super::ast::FeelExpression`].
//!
//! **Prohibition:** never reuse Rust `==` / `Ord` / `compare_values` for FEEL
//! equality/ordering. The private `cmp` module does NOT re-export
//! `PartialEq`/`PartialOrd` — accidental `a == b` reuse at evaluator
//! call-sites becomes a "method not found" or lint error
//! (31-RESEARCH.md Pitfall 1 / Pattern 2). The `no_rust_eq` test module
//! enforces this invariant at test time by grepping the source.
//!
//! **Evaluator contract (DMN-18):** `evaluate` takes a *borrowed*
//! `&FeelExpression`, never `&str`. There is no `evaluate(src: &str, …)`
//! overload. This is the type-level contract that prevents the 4–6×
//! re-parse performance bug (a 30-cell table × 10k evals ≈ 30 parses at
//! load, not 300k). The AST cache (31-09) hands the evaluator `&*arc`.

use crate::data_types::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::cmp::Ordering;

use super::ast::{FeelExpression, BinOpKind};
use super::coercion::{coerce_to_number, coerce_to_string};
use super::numeric::feel_numeric_eq;
use super::temporal::parse_at_literal;
use super::types::FeelRange;

/// Flat FEEL evaluation scope (CONTEXT.md user override: flat Scope +
/// explicit dependency tracking — NOT a parent/child chain).
///
/// A `Scope` is a flat `HashMap<String, Value>` of bindings (input data,
/// decision results, iteration variables) plus a `deps:
/// HashMap<String, Vec<String>>` recording which names each binding
/// depends on. **Nested scope resolution is NOT a parent-chain walk** —
/// it is an explicit dependency lookup. The same `deps` map drives
/// boxed-context topological evaluation in 31-05 (DMN-15).
///
/// `Serialize`/`Deserialize`/`PartialEq` are derived so the
/// [`super::types::FeelFunction`] carrier's `captured: Option<Arc<Scope>>`
/// field continues to satisfy its serde + equality bounds (Phase 30
/// D-30-01c; 31-01 carrier-shape decision).
///
/// 31-RESEARCH.md Pattern 3 / Scope Model + QN Resolution.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Scope {
    /// Name → value bindings (input data, decision results, iteration vars).
    pub bindings: HashMap<String, Value>,
    /// Name → names it depends on (drives boxed-context topo sort + QN
    /// resolution). NOT a parent-chain.
    pub deps: HashMap<String, Vec<String>>,
}

impl Scope {
    /// Construct an empty scope.
    pub fn empty() -> Self {
        Scope {
            bindings: HashMap::new(),
            deps: HashMap::new(),
        }
    }

    /// Look up a bound name.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.bindings.get(name)
    }

    /// Bind `name` to `v`, recording its dependency list in `deps`.
    pub fn bind(&mut self, name: String, v: Value, deps: Vec<String>) {
        self.deps.insert(name.clone(), deps);
        self.bindings.insert(name, v);
    }

    /// Resolve a qualified name `a.b.c` against the flat scope using the
    /// longest-match rule: try the longest bound prefix (`a.b.c`, then
    /// `a.b`, then `a`); if found, path-navigate the remaining components
    /// into the value (`.b.c` on a context). Returns [`Value::Null`] when
    /// no prefix resolves.
    ///
    /// Path navigation on a [`Value::Object`] looks up each component as a
    /// key; on a [`Value::Array`] it maps over the elements. Any
    /// non-navigable step yields [`Value::Null`].
    pub fn resolve_qn(&self, qn: &[String]) -> Value {
        // Longest-match: try the longest bound prefix first.
        for i in (1..=qn.len()).rev() {
            let prefix = qn[..i].join(".");
            if let Some(v) = self.bindings.get(&prefix) {
                let mut current = v.clone();
                for comp in &qn[i..] {
                    current = path_nav(current, comp);
                    if matches!(current, Value::Null) {
                        return Value::Null;
                    }
                }
                return current;
            }
        }
        Value::Null
    }
}

/// Path-navigate a single `.name` step into `v`.
///
/// - [`Value::Object`] → look up `name` key (returns [`Value::Null`] if
///   absent).
/// - [`Value::Array`] → map over elements, navigating each (FEEL list path
///   semantics, rule 43).
/// - anything else → [`Value::Null`].
fn path_nav(v: Value, name: &str) -> Value {
    match v {
        Value::Object(map) => map.get(name).cloned().unwrap_or(Value::Null),
        Value::Array(elems) => Value::Array(
            elems
                .into_iter()
                .map(|e| path_nav(e, name))
                .collect(),
        ),
        _ => Value::Null,
    }
}

// =====================================================================
// Private cmp module — the SOLE home of FEEL equality/ordering.
// =====================================================================

/// Private three-valued-logic comparison module.
///
/// This module is the **only** place in `feel::eval` that performs FEEL
/// equality/ordering. It does NOT re-export `PartialEq`/`PartialOrd` —
/// accidental `a == b` reuse on [`Value`] at evaluator call-sites is a
/// visibility error (31-RESEARCH.md Pitfall 1 / Pattern 2). The
/// `no_rust_eq` test module enforces the invariant at test time.
///
/// All operators return `Option<_>` so `None` maps to [`Value::Null`] at
/// the evaluator boundary (three-valued logic, DMN 1.5 rule 49.a).
// `#[allow(dead_code)]`: the cmp functions are exercised by the Task 1
// unit tests (dmn12_/truth_tables) and are wired into `evaluate`'s dispatch
// in Task 2 GREEN. The non-test build has no caller until that dispatch
// lands, so the lint is expected and transient.
#[allow(dead_code)]
mod cmp {
    use crate::data_types::Value;
    use std::cmp::Ordering;

    /// FEEL equality — `Option<bool>` (null-propagating). NEVER reuse Rust `==`.
    ///
    /// DMN 1.5 rule 49.a (verbatim):
    /// - `null = null` → `true` (rule 49.a: `e = null` maps to `FEEL(e) is
    ///   null`; `null is null` is `true`).
    /// - `null = e` (non-null) → `false` (`FEEL(e) is null` is `false`).
    /// - `e1 = e2` for same-kind operands → `true`/`false`.
    /// - **different kind** (e.g. `"a" = 1`) → `None` (null), NOT `false`
    ///   (Pitfall 4).
    ///
    /// Number/Integer cross-type promotion: `Number(5) = Integer(5)` →
    /// `true` (§10.3.2.2 — integers and numbers are one numeric domain).
    /// `Date = DateTimeTz` at UTC midnight → `true` (§10.3.2.3.5).
    pub fn feel_equals(a: &Value, b: &Value) -> Option<bool> {
        // Rule 49.a null arms (verbatim): null=null → true; null=non-null →
        // false (because `FEEL(e) is null` is false when e is non-null).
        match (a, b) {
            (Value::Null, Value::Null) => return Some(true),
            (Value::Null, _) | (_, Value::Null) => return Some(false),
            _ => {}
        }
        // Same-kind arms.
        match (a, b) {
            // Numeric domain: Number/Number, Number/Integer, Integer/Number
            // via feel_numeric_eq (Phase 30 consistency). (Integer, Integer)
            // is NOT handled by feel_numeric_eq (its `_` arm returns None),
            // so compare the integers directly.
            (Value::Number(_), Value::Number(_))
            | (Value::Number(_), Value::Integer(_))
            | (Value::Integer(_), Value::Number(_)) => super::feel_numeric_eq(a, b),
            (Value::Integer(x), Value::Integer(y)) => Some(*x == *y),
            (Value::String(x), Value::String(y)) => Some(x == y),
            (Value::Bool(x), Value::Bool(y)) => Some(x == y),
            (Value::Date(x), Value::Date(y)) => Some(x == y),
            // §10.3.2.3.5: a date ≡ a datetime at UTC midnight on that date.
            (Value::Date(d), Value::DateTimeTz(dtv)) | (Value::DateTimeTz(dtv), Value::Date(d)) => {
                date_equals_datetime_z(d, dtv)
            }
            (Value::Time(x), Value::Time(y)) => Some(x == y),
            (Value::DateTimeTz(x), Value::DateTimeTz(y)) => Some(x == y),
            (Value::DaysTimeDuration(x), Value::DaysTimeDuration(y)) => Some(x == y),
            (Value::YearsMonthsDuration(x), Value::YearsMonthsDuration(y)) => Some(x == y),
            (Value::Array(x), Value::Array(y)) => list_equals(x, y),
            (Value::Object(x), Value::Object(y)) => context_equals(x, y),
            // TODO(31-05): Value::Range equality (structural endpoint
            // comparison) lands when boxed-context evaluation needs it. For
            // now range equality yields None (null) — not a same-kind
            // scalar arm. No conformance case requires it in 31-02.
            // Type mismatch ("a" = 1, true = 1, etc.) → None (null), NOT
            // false (Pitfall 4).
            _ => None,
        }
    }

    /// FEEL ordering — `Option<Ordering>` (null-propagating). NEVER reuse Rust `Ord`.
    ///
    /// - Any `null` operand → `None` (`null < 3` → `null`).
    /// - Same-kind, comparable → `Some(Ordering)`.
    /// - **Different kind** → `None` (input domain requires same kind).
    pub fn feel_compare(a: &Value, b: &Value) -> Option<Ordering> {
        match (a, b) {
            (Value::Null, _) | (_, Value::Null) => None,
            // Numeric domain. (Number, Integer) and (Integer, Number) are
            // handled with the correct operand order (the research Pattern 2
            // code had the order reversed for (Number, Integer) — a latent
            // bug; this is the corrected version).
            (Value::Number(x), Value::Number(y)) => x.partial_cmp(y),
            (Value::Number(x), Value::Integer(i)) => x.partial_cmp(&rust_decimal::Decimal::from(*i)),
            (Value::Integer(i), Value::Number(x)) => rust_decimal::Decimal::from(*i).partial_cmp(x),
            (Value::Integer(x), Value::Integer(y)) => Some(x.cmp(y)),
            (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
            (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
            (Value::Date(x), Value::Date(y)) => Some(x.cmp(y)),
            (Value::Time(x), Value::Time(y)) => time_cmp(x, y),
            (Value::DateTimeTz(x), Value::DateTimeTz(y)) => datetime_cmp(x, y),
            (Value::DaysTimeDuration(x), Value::DaysTimeDuration(y)) => Some(x.cmp(y)),
            (Value::YearsMonthsDuration(x), Value::YearsMonthsDuration(y)) => Some(x.months.cmp(&y.months)),
            // Type mismatch or unordered types → None.
            _ => None,
        }
    }

    /// Explicit three-valued AND. `false` is dominant; `null` propagates
    /// otherwise. Truth table (31-RESEARCH.md):
    /// - `(Some(false), _) | (_, Some(false))` → `Some(false)`.
    /// - `(Some(true), Some(true))` → `Some(true)`.
    /// - `_` → `None` (null present, no false).
    pub fn and_3vl(a: Option<bool>, b: Option<bool>) -> Option<bool> {
        match (a, b) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        }
    }

    /// Explicit three-valued OR. `true` is dominant; `null` propagates
    /// otherwise. Truth table (31-RESEARCH.md):
    /// - `(Some(true), _) | (_, Some(true))` → `Some(true)`.
    /// - `(Some(false), Some(false))` → `Some(false)`.
    /// - `_` → `None` (null present, no true).
    pub fn or_3vl(a: Option<bool>, b: Option<bool>) -> Option<bool> {
        match (a, b) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        }
    }

    /// Explicit three-valued NOT. `null` → `null`; else negate.
    /// - `Some(true)` → `Some(false)`.
    /// - `Some(false)` → `Some(true)`.
    /// - `None` → `None` (`not(null)` → `null`).
    pub fn not_3vl(a: Option<bool>) -> Option<bool> {
        match a {
            Some(true) => Some(false),
            Some(false) => Some(true),
            None => None,
        }
    }

    /// Element-wise list equality. Two lists are equal iff same length and
    /// every pair is `feel_equals`-equal. A length mismatch → `Some(false)`
    /// (definite). Any pair returning `None` (type mismatch) → `None` (null
    /// propagates through the list comparison per 3VL).
    fn list_equals(x: &[Value], y: &[Value]) -> Option<bool> {
        if x.len() != y.len() {
            return Some(false);
        }
        let mut all_eq = true;
        for (a, b) in x.iter().zip(y.iter()) {
            match feel_equals(a, b) {
                Some(true) => {}
                Some(false) => all_eq = false,
                None => return None,
            }
        }
        Some(all_eq)
    }

    /// Context equality: same keys, and each key's value is `feel_equals`-equal.
    /// A missing key in the other context → `Some(false)` (definite). A
    /// value comparison returning `None` → `None` (null propagates).
    fn context_equals(
        x: &std::collections::HashMap<String, Value>,
        y: &std::collections::HashMap<String, Value>,
    ) -> Option<bool> {
        if x.len() != y.len() {
            return Some(false);
        }
        let mut all_eq = true;
        for (k, v) in x {
            match y.get(k) {
                None => return Some(false),
                Some(v2) => match feel_equals(v, v2) {
                    Some(true) => {}
                    Some(false) => all_eq = false,
                    None => return None,
                },
            }
        }
        Some(all_eq)
    }

    /// §10.3.2.3.5: a `date` equals a `date and time` at UTC midnight on
    /// that date. The datetime is compared in UTC: a Fixed/Iana datetime is
    /// converted to its UTC naive component; a Local datetime is treated as
    /// if its naive component were UTC (no offset carried).
    fn date_equals_datetime_z(
        d: &chrono::NaiveDate,
        dtv: &super::super::temporal::DateTimeValue,
    ) -> Option<bool> {
        let midnight = chrono::NaiveTime::from_hms_opt(0, 0, 0)?;
        let (utc_date, utc_time) = match dtv {
            super::super::temporal::DateTimeValue::Fixed(dt) => {
                let utc = dt.naive_utc();
                (utc.date(), utc.time())
            }
            super::super::temporal::DateTimeValue::Iana(dt) => {
                let utc = dt.naive_utc();
                (utc.date(), utc.time())
            }
            super::super::temporal::DateTimeValue::Local(ndt) => {
                (ndt.date(), ndt.time())
            }
        };
        Some(utc_date == *d && utc_time == midnight)
    }

    /// Compare two `TimeValue`s. Within-form comparisons use the natural
    /// ordering; cross-form comparisons (e.g. Offset vs Local) return `None`
    /// (incomparable without a shared reference frame).
    fn time_cmp(
        x: &super::super::temporal::TimeValue,
        y: &super::super::temporal::TimeValue,
    ) -> Option<Ordering> {
        use super::super::temporal::TimeValue;
        match (x, y) {
            (TimeValue::Fixed(a), TimeValue::Fixed(b)) => Some(a.cmp(b)),
            (TimeValue::Local(a), TimeValue::Local(b)) => Some(a.cmp(b)),
            (TimeValue::Offset(a), TimeValue::Offset(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }

    /// Compare two `DateTimeValue`s. Within-form comparisons use the natural
    /// ordering; cross-form comparisons (Fixed/Iana/Local) return `None`.
    fn datetime_cmp(
        x: &super::super::temporal::DateTimeValue,
        y: &super::super::temporal::DateTimeValue,
    ) -> Option<Ordering> {
        use super::super::temporal::DateTimeValue;
        match (x, y) {
            (DateTimeValue::Fixed(a), DateTimeValue::Fixed(b)) => Some(a.cmp(b)),
            (DateTimeValue::Iana(a), DateTimeValue::Iana(b)) => Some(a.cmp(b)),
            (DateTimeValue::Local(a), DateTimeValue::Local(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }
}

// =====================================================================
// Evaluator dispatch — Plan 31-02 Task 2.
// =====================================================================

/// The sole FEEL evaluator entry point (DMN-18 contract).
///
/// Takes a *borrowed* [`FeelExpression`] AST and a flat [`Scope`], returns
/// a [`Value`]. There is **no** `evaluate(src: &str, …)` overload — the
/// type-level contract that prevents the 4–6× re-parse performance bug
/// (a 30-cell table × 10k evals ≈ 30 parses at load via the AST cache, not
/// 300k).
///
/// Plan 31-02 implements the comparison/boolean/Literal/Name/QualifiedName/
/// AtLiteral/Range/Neg/Err arms and stubs the control/boxed/function arms
/// to [`Value::Null`] with `TODO(31-NN)` markers; subsequent plans fill
/// them in.
pub fn evaluate(expr: &FeelExpression, scope: &Scope) -> Value {
    match expr {
        // ----- Real arms (31-02) -----
        FeelExpression::Literal(v) => v.clone(),
        FeelExpression::Name(name) => scope.get(name).cloned().unwrap_or(Value::Null),
        FeelExpression::QualifiedName(qn) => scope.resolve_qn(qn),
        FeelExpression::AtLiteral(inner) => parse_at_literal(inner).unwrap_or(Value::Null),
        FeelExpression::Range {
            start,
            end,
            start_inclusive,
            end_inclusive,
        } => {
            let s = evaluate(start, scope);
            let e = evaluate(end, scope);
            Value::Range(Box::new(FeelRange {
                start: opt_endpoint(s),
                end: opt_endpoint(e),
                start_incl: *start_inclusive,
                end_incl: *end_inclusive,
            }))
        }
        FeelExpression::BinOp { op, lhs, rhs } => eval_binop(op, lhs, rhs, scope),
        FeelExpression::Neg(inner) => match evaluate(inner, scope) {
            Value::Number(x) => Value::Number(-x),
            Value::Integer(i) => Value::Integer(-i),
            _ => Value::Null,
        },
        FeelExpression::Err(_, _) => Value::Null,

        // ----- Stubbed arms (subsequent plans) -----
        // Control + quantified + path + filter + instance-of + boxed
        // expressions are stubbed to Value::Null here. The exhaustive
        // match (NO `_ =>` wildcard) is the safety net: adding a new
        // FeelExpression variant forces an evaluator-arm update.
        FeelExpression::If { .. } => {
            // TODO(31-05): 3VL condition dispatch.
            Value::Null
        }
        FeelExpression::For { .. } => {
            // TODO(31-05): cartesian-product iteration → Array.
            Value::Null
        }
        FeelExpression::Quantified { .. } => {
            // TODO(31-05): some/every fold via or_3vl/and_3vl.
            Value::Null
        }
        FeelExpression::Path { .. } => {
            // TODO(31-05): context/list path navigation via path_nav.
            Value::Null
        }
        FeelExpression::Filter { .. } => {
            // TODO(31-05): list filter predicate.
            Value::Null
        }
        FeelExpression::InstanceOf { .. } => {
            // TODO(31-04): feel::types::conforms(type_of(v), T).
            Value::Null
        }
        FeelExpression::List(_) => {
            // TODO(31-05): evaluate each element → Array.
            Value::Null
        }
        FeelExpression::Relation { .. } => {
            // TODO(31-05): relation → list of contexts.
            Value::Null
        }
        FeelExpression::Context(_) => {
            // TODO(31-05): dependency-ordered context evaluation.
            Value::Null
        }
        FeelExpression::FunctionInvocation { .. } => {
            // TODO(31-06/31-07): built-in + user-lambda invocation.
            Value::Null
        }
        FeelExpression::FunctionDefinition { .. } => {
            // TODO(31-06): closure construction (FeelFunction carrier).
            Value::Null
        }
    }
}

/// Evaluate a binary operator. Comparison (`=`/`!=`/`<`/`<=`/`>`/`>=`) and
/// boolean (`and`/`or`) operators are real in 31-02; `between` is real
/// (desugars to `a >= b and a <= c` via `feel_compare` + `and_3vl`).
/// Arithmetic (`+`/`-`/`*`/`/`/`**`) and `in` (positive unary test) are
/// stubbed to `Value::Null` — they land in 31-03 (coercion) where the
/// numeric/string coercion rules live.
fn eval_binop(op: &BinOpKind, lhs: &FeelExpression, rhs: &FeelExpression, scope: &Scope) -> Value {
    // `between` has a `List([b, c])` rhs (parser desugaring); handle it
    // before evaluating rhs as a value (the List arm is still stubbed).
    if matches!(op, BinOpKind::Between) {
        let l = evaluate(lhs, scope);
        let (b, c) = match rhs {
            FeelExpression::List(items) if items.len() == 2 => (&items[0], &items[1]),
            _ => return Value::Null, // malformed AST (parser invariant violation)
        };
        let bv = evaluate(b, scope);
        let cv = evaluate(c, scope);
        // `a between b and c` ≡ `a >= b and a <= c`.
        // `feel_compare(a, b)` returns the ordering of `a` relative to `b`,
        // so `a >= b` ↔ `o != Ordering::Less`; `a <= c` ↔ `o != Ordering::Greater`.
        // (Rule 1 fix: the prior code had the ge/le operands swapped —
        // `o != Greater` for ge and `o != Less` for le, which inverted the
        // `between` range check. Corrected here.)
        let ge = cmp::feel_compare(&l, &bv).map(|o| o != Ordering::Less);
        let le = cmp::feel_compare(&l, &cv).map(|o| o != Ordering::Greater);
        return opt_bool(cmp::and_3vl(ge, le));
    }
    let l = evaluate(lhs, scope);
    let r = evaluate(rhs, scope);
    match op {
        BinOpKind::Eq => opt_bool(cmp::feel_equals(&l, &r)),
        BinOpKind::NotEq => opt_bool(cmp::feel_equals(&l, &r).map(|b| !b)),
        BinOpKind::Lt => opt_bool(cmp::feel_compare(&l, &r).map(|o| o == Ordering::Less)),
        BinOpKind::Le => opt_bool(cmp::feel_compare(&l, &r).map(|o| o != Ordering::Greater)),
        BinOpKind::Gt => opt_bool(cmp::feel_compare(&l, &r).map(|o| o == Ordering::Greater)),
        BinOpKind::Ge => opt_bool(cmp::feel_compare(&l, &r).map(|o| o != Ordering::Less)),
        BinOpKind::And => opt_bool(cmp::and_3vl(to_bool(&l), to_bool(&r))),
        BinOpKind::Or => opt_bool(cmp::or_3vl(to_bool(&l), to_bool(&r))),
        // Arithmetic (DMN-13). All via Decimal128; div-by-zero / overflow →
        // Value::Null (CONTEXT.md locked numeric-edge rule; no Result/panic).
        // No implicit number↔string conversion: Number+String → Null
        // (Pitfall 6); the explicit string()/number() built-ins (31-07) are
        // the only such path.
        BinOpKind::Add => eval_add(l, r),
        BinOpKind::Sub => numeric_binop(l, r, |x, y| Some(x - y)),
        BinOpKind::Mul => numeric_binop(l, r, |x, y| Some(x * y)),
        BinOpKind::Div => numeric_binop(l, r, |x, y| {
            if y.is_zero() {
                None
            } else {
                x.checked_div(y)
            }
        }),
        BinOpKind::Pow => eval_pow(l, r),
        // `in` (DMN-13 range membership): `a in <range>` checks that `a`
        // lies within the range's endpoints per the inclusivity flags. The
        // full unary-test `in` semantics (`a in <positive unary test>`, `a
        // in (tests)`, `-`, `not(...)`) land in 31-04 alongside the
        // SimpleUnaryTests evaluator.
        BinOpKind::In => eval_in(l, r),
        // Between is handled above; this arm is unreachable but kept for
        // exhaustiveness (BinOpKind is a closed enum — no `_ =>` wildcard).
        BinOpKind::Between => unreachable!("between handled above"),
    }
}

/// `Option<bool>` → `Value`: `Some(b)` → `Bool(b)`, `None` → `Null`. This is
/// the evaluator-boundary 3VL rule — the `None` from null-propagating
/// comparisons/boolean-ops maps to `Value::Null` for the FEEL caller.
fn opt_bool(b: Option<bool>) -> Value {
    match b {
        Some(v) => Value::Bool(v),
        None => Value::Null,
    }
}

// =====================================================================
// Plan 31-03: arithmetic / string-concat / in-range helpers.
// =====================================================================

/// FEEL `+` (DMN-13). Two semantics, distinguished by operand kind:
/// - `String + String` → `String` (concatenation). Either operand `Null` →
///   `Null`.
/// - `Number`/`Integer` `+` `Number`/`Integer` → `Number` (Decimal128 add;
///   integers promoted via `Decimal::from`).
/// - `Number + String` / `String + Number` / any other mixed kind → `Null`
///   (NO implicit number↔string conversion — Phase 30 Pitfall 6; the explicit
///   `string()`/`number()` built-ins in 31-07 are the only such path).
fn eval_add(l: Value, r: Value) -> Value {
    // Null operand short-circuit (both the concat and numeric arms propagate
    // null for a null operand).
    if matches!(l, Value::Null) || matches!(r, Value::Null) {
        return Value::Null;
    }
    // String concatenation — both operands must be Strings. This is the ONLY
    // `+` overload for the String kind; Number+String falls through to the
    // numeric arm and yields Null (coerce_to_number(String) = None).
    if let (Some(a), Some(b)) = (coerce_to_string(&l), coerce_to_string(&r)) {
        let mut s = a;
        s.push_str(&b);
        return Value::String(s);
    }
    // Numeric add (Number/Integer → Decimal).
    numeric_binop(l, r, |x, y| Some(x + y))
}

/// Apply a Decimal128 binary op to two `Value`s. Either operand not coercible
/// to a number (String, Bool, Null, …) → `Value::Null`. The op closure
/// returns `None` for div-by-zero / overflow / invalid → `Value::Null`
/// (CONTEXT.md locked numeric-edge rule; no `Result`, no panic). No `f64` is
/// constructed in this path (prohibition 2).
fn numeric_binop<F>(l: Value, r: Value, op: F) -> Value
where
    F: FnOnce(rust_decimal::Decimal, rust_decimal::Decimal) -> Option<rust_decimal::Decimal>,
{
    let Some(x) = coerce_to_number(&l) else {
        return Value::Null;
    };
    let Some(y) = coerce_to_number(&r) else {
        return Value::Null;
    };
    match op(x, y) {
        Some(result) => Value::Number(result),
        None => Value::Null,
    }
}

/// FEEL `**` (DMN-13). Uses `rust_decimal::MathematicalOps::checked_powd`,
/// which: handles non-negative integer exponents exactly via
/// `checked_powu`; handles negative integer exponents via `checked_powi`
/// (1/base^n); and approximates non-integer exponents via
/// `exp(y·ln(x))`. Returns `None` on overflow → `Value::Null`. The
/// right-associativity is in the AST shape (the parser produced
/// `BinOp(Pow, 2, BinOp(Pow, 3, 2))`); eval just walks the tree, so
/// `2 ** 3 ** 2 = 2 ** 9 = 512` (Pitfall 2).
fn eval_pow(l: Value, r: Value) -> Value {
    use rust_decimal::MathematicalOps;
    let Some(base) = coerce_to_number(&l) else {
        return Value::Null;
    };
    let Some(exp) = coerce_to_number(&r) else {
        return Value::Null;
    };
    match base.checked_powd(exp) {
        Some(result) => Value::Number(result),
        None => Value::Null,
    }
}

/// FEEL `in` (DMN-13 range membership). For a `Value::Range` RHS, checks
/// that `l` lies within the range's endpoints per the inclusivity flags via
/// `feel_compare` + `and_3vl`. A null operand or incomparable endpoint →
/// `None` propagates to `Value::Null`. For any non-Range RHS, returns
/// `Value::Null` — the full unary-test `in` semantics (`a in <positive
/// unary test>`, `a in (tests)`, `-`, `not(...)`) land in 31-04.
fn eval_in(l: Value, r: Value) -> Value {
    let Value::Range(range) = r else {
        // TODO(31-04): full unary-test `in` semantics (positive unary test,
        // comma-list, `-`, `not(...)`).
        return Value::Null;
    };
    let mut ok: Option<bool> = Some(true);
    if let Some(start) = &range.start {
        let in_lower = match cmp::feel_compare(&l, start) {
            Some(o) => Some(if range.start_incl {
                o != Ordering::Less
            } else {
                o == Ordering::Greater
            }),
            None => None,
        };
        ok = cmp::and_3vl(ok, in_lower);
    }
    if let Some(end) = &range.end {
        let in_upper = match cmp::feel_compare(&l, end) {
            Some(o) => Some(if range.end_incl {
                o != Ordering::Greater
            } else {
                o == Ordering::Less
            }),
            None => None,
        };
        ok = cmp::and_3vl(ok, in_upper);
    }
    opt_bool(ok)
}

/// `Value` → `Option<bool>` for boolean operator operands: `Bool(b)` →
/// `Some(b)`, `Null` → `None`, any other kind → `None` (a non-boolean in a
/// boolean context is a type error → null, per 3VL).
fn to_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

/// `Value` → `Option<Value>` for range endpoints: `Null` → `None` (open
/// endpoint), any other value → `Some(v)`.
fn opt_endpoint(v: Value) -> Option<Value> {
    match v {
        Value::Null => None,
        other => Some(other),
    }
}

// =====================================================================
// Tests.
//
// These are declared FLAT at the `feel::eval` module level (not nested in
// a `mod tests` submodule) so the VALIDATION.md per-task filters
// (`cargo test -p engine --lib dmn::feel::eval::dmn12_`, `::truth_tables`,
// `::no_rust_eq`, `::eval_`) match the test paths directly. The
// `no_rust_eq` lint guard below scans the source above the first
// `#[cfg(test)]` test item, so keep all tests below this point.
// =====================================================================

#[cfg(test)]
use crate::dmn::feel::temporal::DateTimeValue;
#[cfg(test)]
use chrono::NaiveDate;
#[cfg(test)]
use rust_decimal::dec;
#[cfg(test)]
use crate::dmn::feel::parser::parse_expression;
#[cfg(test)]
use proptest::prop_assert_eq;
#[cfg(test)]
use proptest::strategy::Strategy;

// ----- MANDATORY DMN-12 conformance cases (the load-bearing tests) -----

/// DMN-12: `"a" = 1` → `null` (type mismatch → None → Value::Null).
#[cfg(test)]
#[test]
fn dmn12_a_equals_type_mismatch_yields_null() {
    assert_eq!(
        cmp::feel_equals(&Value::String("a".into()), &Value::Number(dec!(1))),
        None
    );
}

/// DMN-12: `null < 3` → `null` (any null operand in a comparison → null).
#[cfg(test)]
#[test]
fn dmn12_null_lt_three_yields_null() {
    assert_eq!(
        cmp::feel_compare(&Value::Null, &Value::Number(dec!(3))),
        None
    );
}

/// DMN-12: `null = null` → `true` (rule 49.a: `e = null` → `FEEL(e) is
/// null`; `null is null` is `true`).
#[cfg(test)]
#[test]
fn dmn12_null_equals_null_yields_true() {
    assert_eq!(cmp::feel_equals(&Value::Null, &Value::Null), Some(true));
}

/// DMN-12: `null = 5` → `false` (`e = null` → `FEEL(e) is null` → false
/// when e is non-null). Pitfall 4: this is a definite `false`, NOT null.
#[cfg(test)]
#[test]
fn dmn12_null_equals_nonnull_yields_false() {
    assert_eq!(
        cmp::feel_equals(&Value::Null, &Value::Number(dec!(5))),
        Some(false)
    );
}

/// DMN-12: `not(null)` → `null` (negation table: "otherwise" → null).
#[cfg(test)]
#[test]
fn dmn12_not_null() {
    assert_eq!(cmp::not_3vl(None), None);
}

/// DMN-12 and/or truth tables (the CONTEXT.md locked conformance cases).
#[cfg(test)]
#[test]
fn dmn12_and_truth_table() {
    // null and true → null
    assert_eq!(cmp::and_3vl(None, Some(true)), None);
    // false is dominant: Some(false) and _ → Some(false)
    assert_eq!(cmp::and_3vl(Some(false), None), Some(false));
    assert_eq!(cmp::and_3vl(None, Some(false)), Some(false));
    // true and true → true
    assert_eq!(cmp::and_3vl(Some(true), Some(true)), Some(true));
    // null and false → false (false dominant)
    assert_eq!(cmp::and_3vl(None, Some(false)), Some(false));
}

#[cfg(test)]
#[test]
fn dmn12_or_truth_table() {
    // true is dominant: Some(true) or _ → Some(true)
    assert_eq!(cmp::or_3vl(Some(true), None), Some(true));
    // null or true → true (true dominant)
    assert_eq!(cmp::or_3vl(None, Some(true)), Some(true));
    // false or false → false
    assert_eq!(cmp::or_3vl(Some(false), Some(false)), Some(false));
    // null or false → null (no true, null present)
    assert_eq!(cmp::or_3vl(None, Some(false)), None);
}

// ----- Same-kind equality / cross-type promotion -----

/// §10.3.2.2: `Number(5) = Integer(5)` → `true` (one numeric domain).
#[cfg(test)]
#[test]
fn number_integer_cross_type() {
    assert_eq!(
        cmp::feel_equals(&Value::Number(dec!(5)), &Value::Integer(5)),
        Some(true)
    );
    assert_eq!(
        cmp::feel_equals(&Value::Integer(5), &Value::Number(dec!(5))),
        Some(true)
    );
}

/// §10.3.2.3.5: a `date` equals a `date and time` at UTC midnight on
/// that date.
#[cfg(test)]
#[test]
fn date_equals_datetime_z() {
    let d = NaiveDate::from_ymd_opt(2021, 1, 31).unwrap();
    let midnight = chrono::DateTime::parse_from_rfc3339("2021-01-31T00:00:00+00:00").unwrap();
    let dtv = DateTimeValue::Fixed(midnight);
    assert_eq!(
        cmp::feel_equals(&Value::Date(d), &Value::DateTimeTz(dtv.clone())),
        Some(true)
    );
    // The reverse direction.
    assert_eq!(
        cmp::feel_equals(&Value::DateTimeTz(dtv), &Value::Date(d)),
        Some(true)
    );
}

// ----- Truth-table property tests -----

/// `and_3vl` truth table: `false` is dominant; `null` propagates.
#[cfg(test)]
#[test]
fn truth_tables_and() {
    // false is dominant in and.
    for b in [None, Some(true), Some(false)] {
        assert_eq!(cmp::and_3vl(Some(false), b), Some(false));
        assert_eq!(cmp::and_3vl(b, Some(false)), Some(false));
    }
    // true and true → true.
    assert_eq!(cmp::and_3vl(Some(true), Some(true)), Some(true));
    // null and true → null; true and null → null.
    assert_eq!(cmp::and_3vl(None, Some(true)), None);
    assert_eq!(cmp::and_3vl(Some(true), None), None);
    // null and null → null.
    assert_eq!(cmp::and_3vl(None, None), None);
}

/// `or_3vl` truth table: `true` is dominant; `null` propagates.
#[cfg(test)]
#[test]
fn truth_tables_or() {
    // true is dominant in or.
    for b in [None, Some(true), Some(false)] {
        assert_eq!(cmp::or_3vl(Some(true), b), Some(true));
        assert_eq!(cmp::or_3vl(b, Some(true)), Some(true));
    }
    // false or false → false.
    assert_eq!(cmp::or_3vl(Some(false), Some(false)), Some(false));
    // null or false → null; false or null → null.
    assert_eq!(cmp::or_3vl(None, Some(false)), None);
    assert_eq!(cmp::or_3vl(Some(false), None), None);
    // null or null → null.
    assert_eq!(cmp::or_3vl(None, None), None);
}

/// `not_3vl` truth table.
#[cfg(test)]
#[test]
fn truth_tables_not() {
    assert_eq!(cmp::not_3vl(Some(true)), Some(false));
    assert_eq!(cmp::not_3vl(Some(false)), Some(true));
    assert_eq!(cmp::not_3vl(None), None);
}

/// `feel_equals` same-kind arms.
#[cfg(test)]
#[test]
fn truth_tables_equals_same_kind() {
    assert_eq!(cmp::feel_equals(&Value::Number(dec!(5)), &Value::Number(dec!(5))), Some(true));
    assert_eq!(cmp::feel_equals(&Value::Number(dec!(5)), &Value::Number(dec!(6))), Some(false));
    assert_eq!(cmp::feel_equals(&Value::String("a".into()), &Value::String("a".into())), Some(true));
    assert_eq!(cmp::feel_equals(&Value::String("a".into()), &Value::String("b".into())), Some(false));
    assert_eq!(cmp::feel_equals(&Value::Bool(true), &Value::Bool(true)), Some(true));
    assert_eq!(cmp::feel_equals(&Value::Bool(true), &Value::Bool(false)), Some(false));
    // Type mismatch → None (Pitfall 4).
    assert_eq!(cmp::feel_equals(&Value::Bool(true), &Value::Number(dec!(1))), None);
}

/// `feel_compare` same-kind + null + type-mismatch.
#[cfg(test)]
#[test]
fn truth_tables_compare() {
    assert_eq!(cmp::feel_compare(&Value::Number(dec!(1)), &Value::Number(dec!(2))), Some(Ordering::Less));
    assert_eq!(cmp::feel_compare(&Value::Number(dec!(2)), &Value::Number(dec!(1))), Some(Ordering::Greater));
    assert_eq!(cmp::feel_compare(&Value::Number(dec!(2)), &Value::Number(dec!(2))), Some(Ordering::Equal));
    // null operand → None.
    assert_eq!(cmp::feel_compare(&Value::Null, &Value::Number(dec!(1))), None);
    assert_eq!(cmp::feel_compare(&Value::Number(dec!(1)), &Value::Null), None);
    // type mismatch → None.
    assert_eq!(cmp::feel_compare(&Value::String("a".into()), &Value::Number(dec!(1))), None);
}

// ----- no_rust_eq lint guard (Pitfall 1 enforcement) -----

/// Lint guard: the evaluator module does NOT reuse Rust `==`/`Ord` for
/// FEEL value equality outside the private `cmp` module. This is the
/// test-time enforcement of the "never reuse Rust ==/Ord for FEEL
/// equality" prohibition (31-RESEARCH.md Pitfall 1; plan prohibition #1).
/// A compile-time gate (newtype wrapper / clippy lint) is deferred to a
/// tech-debt phase; this grep guard catches regressions until then.
///
/// The guard forbids, outside `mod cmp`:
///   - `use …PartialEq` / `use …PartialOrd` imports,
///   - `impl …PartialEq/PartialOrd for …` blocks,
///   - `.partial_cmp(` method calls.
/// It ALLOWS: doc comments (`//`, `//!`, `///`) that mention the
/// prohibition by name, and `#[derive(…, PartialEq, …)]` on `Scope`
/// (a structural derive required by the `FeelFunction` carrier's serde
/// + equality bound — NOT FEEL value equality; the plan explicitly keeps
/// `Value`'s derives untouched, and `Scope`'s derive is the same kind).
#[cfg(test)]
#[test]
fn no_rust_eq() {
    let src = include_str!("eval.rs");
    // Find the `mod cmp {` block boundaries (first occurrence).
    let cmp_start = src.find("mod cmp {").expect("mod cmp must exist");
    // The closing `}` of the cmp module: scan from cmp_start balancing braces.
    let bytes = src.as_bytes();
    let mut depth: i32 = 0;
    let mut cmp_end = src.len();
    let mut i = cmp_start;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    cmp_end = i + 1;
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    // Outside-cmp source = before `mod cmp` + after `mod cmp`'s closing
    // brace up to the first test item (tests legitimately use `==` /
    // `assert_eq!` to assert expected `Option<bool>` values — that is test
    // assertion, not FEEL value equality).
    let outside_before = &src[..cmp_start];
    let after_cmp = &src[cmp_end..];
    let outside_after = match after_cmp.find("#[cfg(test)]\nuse crate::dmn::feel::parser") {
        Some(pos) => &after_cmp[..pos],
        None => match after_cmp.find("#[cfg(test)]") {
            Some(pos) => &after_cmp[..pos],
            None => after_cmp,
        },
    };

    // Strip comment lines (`//`, `//!`, `///`) and `#[derive(...)]`
    // attribute lines — these may legitimately mention `PartialEq` by
    // name (the prohibition doc + the structural `Scope` derive). What
    // remains is executable code; that is what must not reuse Rust eq/ord.
    fn strip(region: &str) -> Vec<&str> {
        region
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !(trimmed.starts_with("//") || trimmed.starts_with("#[derive("))
            })
            .collect()
    }
    let code_before = strip(outside_before);
    let code_after = strip(outside_after);

    // Forbidden outside `mod cmp`: PartialEq/PartialOrd imports, impl
    // blocks, and `.partial_cmp(` method calls. (`==` is permitted because
    // the evaluator legitimately compares `BinOpKind` variants with `==`;
    // the danger is Ord/partial_cmp reuse on `Value`, which `cmp` owns.)
    let check = |lines: &[&str], where_| {
        for line in lines {
            let has_partial_eq = line.contains("PartialEq");
            let has_partial_ord = line.contains("PartialOrd");
            // `use …PartialEq/PartialOrd` import.
            assert!(
                !(line.contains("use ") && (has_partial_eq || has_partial_ord)),
                "no_rust_eq: Rust eq/ord trait import outside mod cmp ({}): {}",
                where_, line
            );
            // `impl …PartialEq/PartialOrd for …` block.
            assert!(
                !(line.contains("impl ") && (has_partial_eq || has_partial_ord) && line.contains(" for ")),
                "no_rust_eq: Rust eq/ord impl block outside mod cmp ({}): {}",
                where_, line
            );
            // `.partial_cmp(` method call.
            assert!(
                !line.contains(".partial_cmp("),
                "no_rust_eq: `.partial_cmp(` call outside mod cmp ({}): {}",
                where_, line
            );
        }
    };
    check(&code_before, "before cmp");
    check(&code_after, "after cmp");
}

// ----- evaluate dispatch tests (Task 2) -----

#[cfg(test)]
fn scope_with_x_5() -> Scope {
    let mut s = Scope::empty();
    s.bind("x".to_string(), Value::Number(dec!(5)), vec![]);
    s
}

/// `null = null` → `Bool(true)` (rule 49.a via the Eq BinOp arm).
#[cfg(test)]
#[test]
fn eval_null_equals_null() {
    let e = parse_expression("null = null");
    assert_eq!(evaluate(&e, &Scope::empty()), Value::Bool(true));
}

/// `null < 3` → `Null` (null operand in comparison → None → Null).
#[cfg(test)]
#[test]
fn eval_null_lt_three() {
    let e = parse_expression("null < 3");
    assert_eq!(evaluate(&e, &Scope::empty()), Value::Null);
}

/// `true and false` → `Bool(false)`.
#[cfg(test)]
#[test]
fn eval_true_and_false() {
    let e = parse_expression("true and false");
    assert_eq!(evaluate(&e, &Scope::empty()), Value::Bool(false));
}

/// `true or false` → `Bool(true)`.
#[cfg(test)]
#[test]
fn eval_true_or_false() {
    let e = parse_expression("true or false");
    assert_eq!(evaluate(&e, &Scope::empty()), Value::Bool(true));
}

/// `not(null)` → `Null`. The `not` keyword parses to a
/// `FunctionInvocation(callee=Name("not"), args=[null])`. The function
/// invocation arm is stubbed in this plan, so the stub returns Null; the
/// behavior block requires `not(null)` → Null, which the stub satisfies.
/// 31-07 wires the real `not` built-in (which calls `not_3vl`).
#[cfg(test)]
#[test]
fn eval_not_null() {
    let e = parse_expression("not(null)");
    assert_eq!(evaluate(&e, &Scope::empty()), Value::Null);
}

/// `5` → `Number(5)` (Literal arm).
#[cfg(test)]
#[test]
fn eval_literal_number() {
    let e = parse_expression("5");
    assert_eq!(evaluate(&e, &Scope::empty()), Value::Number(dec!(5)));
}

/// `x` → bound value (Name arm via Scope::get); unbound → Null.
#[cfg(test)]
#[test]
fn eval_name() {
    let e = parse_expression("x");
    assert_eq!(evaluate(&e, &scope_with_x_5()), Value::Number(dec!(5)));
    // Unbound name → Null.
    assert_eq!(evaluate(&e, &Scope::empty()), Value::Null);
}

/// `@"2021-01-31"` → `Date(...)` (AtLiteral arm via parse_at_literal).
#[cfg(test)]
#[test]
fn eval_at_literal_date() {
    let e = parse_expression("@\"2021-01-31\"");
    assert_eq!(
        evaluate(&e, &Scope::empty()),
        Value::Date(NaiveDate::from_ymd_opt(2021, 1, 31).unwrap())
    );
}

/// `1 = 1` → `Bool(true)` (comparison BinOp arm via feel_equals).
#[cfg(test)]
#[test]
fn eval_one_equals_one() {
    let e = parse_expression("1 = 1");
    assert_eq!(evaluate(&e, &Scope::empty()), Value::Bool(true));
}

/// `"a" = 1` → `Null` (type mismatch → None → Null).
#[cfg(test)]
#[test]
fn eval_string_equals_number_yields_null() {
    let e = parse_expression("\"a\" = 1");
    assert_eq!(evaluate(&e, &Scope::empty()), Value::Null);
}

/// `Err(_, _)` → `Null`.
#[cfg(test)]
#[test]
fn eval_err_yields_null() {
    let e = FeelExpression::Err(
        super::ast::Span::zero(0, 1, 1),
        "expected expression".to_string(),
    );
    assert_eq!(evaluate(&e, &Scope::empty()), Value::Null);
}

/// No `evaluate(src: &str, ...)` overload exists (type-level no-re-parse
/// contract). The sole public evaluator signature takes `&FeelExpression`.
#[cfg(test)]
#[test]
fn eval_takes_borrowed_ast_not_str() {
    fn _accepts(expr: &FeelExpression, scope: &Scope) -> Value {
        evaluate(expr, scope)
    }
    let e = parse_expression("1");
    let _ = _accepts(&e, &Scope::empty());
}


// =====================================================================
// Plan 31-03 Task 1: arithmetic + string-concat + numeric-edge tests.
// =====================================================================

#[cfg(test)]
fn eval_src(src: &str) -> Value {
    evaluate(&parse_expression(src), &Scope::empty())
}

#[cfg(test)]
fn eval_src_scope(src: &str, scope: &Scope) -> Value {
    evaluate(&parse_expression(src), scope)
}

// ----- arithmetic -----

#[cfg(test)]
#[test]
fn arithmetic_add() {
    assert_eq!(eval_src("1 + 2"), Value::Number(dec!(3)));
}

#[cfg(test)]
#[test]
fn arithmetic_sub() {
    assert_eq!(eval_src("1 - 2"), Value::Number(dec!(-1)));
}

#[cfg(test)]
#[test]
fn arithmetic_mul() {
    assert_eq!(eval_src("2 * 3"), Value::Number(dec!(6)));
}

#[cfg(test)]
#[test]
fn arithmetic_div() {
    assert_eq!(eval_src("6 / 2"), Value::Number(dec!(3)));
}

#[cfg(test)]
#[test]
fn arithmetic_div_decimal() {
    // 7 / 2 = 3.5 — Decimal division, NOT integer truncation.
    assert_eq!(eval_src("7 / 2"), Value::Number(dec!(3.5)));
}

#[cfg(test)]
#[test]
fn arithmetic_pow_right_associative_512() {
    // Pitfall 2: 2 ** 3 ** 2 = 2 ** (3 ** 2) = 2 ** 9 = 512 (NOT 64).
    assert_eq!(eval_src("2 ** 3 ** 2"), Value::Number(dec!(512)));
}

#[cfg(test)]
#[test]
fn arithmetic_pow_2_to_10() {
    assert_eq!(eval_src("2 ** 10"), Value::Number(dec!(1024)));
}

#[cfg(test)]
#[test]
fn arithmetic_unary_neg() {
    assert_eq!(eval_src("- 5"), Value::Number(dec!(-5)));
}

#[cfg(test)]
#[test]
fn arithmetic_unary_neg_integer_value() {
    // An Integer-typed value (via a bound name) negates to Integer.
    let mut s = Scope::empty();
    s.bind("i".to_string(), Value::Integer(7), vec![]);
    assert_eq!(eval_src_scope("- i", &s), Value::Integer(-7));
}

// ----- numeric_edge (div-by-zero, overflow → Null) -----

#[cfg(test)]
#[test]
fn numeric_edge_div_by_zero_integer() {
    assert_eq!(eval_src("1.0 / 0"), Value::Null);
}

#[cfg(test)]
#[test]
fn numeric_edge_div_by_zero_decimal() {
    assert_eq!(eval_src("1.0 / 0.0"), Value::Null);
}

#[cfg(test)]
#[test]
fn numeric_edge_null_operand() {
    assert_eq!(eval_src("null + 1"), Value::Null);
    assert_eq!(eval_src("null - 1"), Value::Null);
    assert_eq!(eval_src("null * 1"), Value::Null);
    assert_eq!(eval_src("null / 1"), Value::Null);
    assert_eq!(eval_src("1 + null"), Value::Null);
}

#[cfg(test)]
#[test]
fn numeric_edge_type_mismatch_yields_null() {
    // Number + Bool → Null (no implicit conversion).
    assert_eq!(eval_src("1 + true"), Value::Null);
}

// ----- string_concat -----

#[cfg(test)]
#[test]
fn string_concat_two_strings() {
    assert_eq!(eval_src("\"a\" + \"b\""), Value::String("ab".to_string()));
}

#[cfg(test)]
#[test]
fn string_concat_number_plus_string_yields_null() {
    // Pitfall 6: NO implicit number<->string conversion; 1 + "2" → null.
    assert_eq!(eval_src("1 + \"2\""), Value::Null);
    assert_eq!(eval_src("\"2\" + 1"), Value::Null);
}

#[cfg(test)]
#[test]
fn string_concat_null_operand() {
    assert_eq!(eval_src("null + \"a\""), Value::Null);
    assert_eq!(eval_src("\"a\" + null"), Value::Null);
}

// ----- between (desugaring — already real in 31-02; reaffirm here) -----

#[cfg(test)]
#[test]
fn between_in_range_true() {
    assert_eq!(eval_src("5 between 1 and 10"), Value::Bool(true));
}

#[cfg(test)]
#[test]
fn between_out_of_range_false() {
    assert_eq!(eval_src("0 between 1 and 10"), Value::Bool(false));
}

// ----- in (range-membership desugaring; full unary-test `in` → 31-04) -----

#[cfg(test)]
#[test]
fn in_range_inclusive_true() {
    // 5 in [1..10] → true (inclusive range).
    assert_eq!(eval_src("5 in [1..10]"), Value::Bool(true));
}

#[cfg(test)]
#[test]
fn in_range_inclusive_boundary() {
    assert_eq!(eval_src("1 in [1..10]"), Value::Bool(true));
    assert_eq!(eval_src("10 in [1..10]"), Value::Bool(true));
}

#[cfg(test)]
#[test]
fn in_range_exclusive_false() {
    // 5 in (1..10) → true (interior); 1 in (1..10) → false (excl start).
    assert_eq!(eval_src("5 in (1..10)"), Value::Bool(true));
    assert_eq!(eval_src("1 in (1..10)"), Value::Bool(false));
    assert_eq!(eval_src("10 in (1..10)"), Value::Bool(false));
}

#[cfg(test)]
#[test]
fn in_range_outside_false() {
    assert_eq!(eval_src("0 in [1..10]"), Value::Bool(false));
    assert_eq!(eval_src("11 in [1..10]"), Value::Bool(false));
}

#[cfg(test)]
#[test]
fn in_range_null_operand_yields_null() {
    assert_eq!(eval_src("null in [1..10]"), Value::Null);
}

// ----- property test: Decimal128 division invariants -----

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 64,
        ..proptest::test_runner::Config::default()
    })]

    /// For any non-zero Decimal `d`, `d / d == Number(1)` — no f64 drift,
    /// no div-by-zero panic (the div-by-zero guard returns Null when d==0,
    /// excluded here). Regression seed committed by the proptest harness.
    #[test]
    fn arithmetic_div_identity(d in (-100_000_000i64..100_000_000i64).prop_filter("nonzero", |x| *x != 0)) {
        let dec = rust_decimal::Decimal::from(d);
        let mut s = Scope::empty();
        s.bind("d".to_string(), Value::Number(dec), vec![]);
        let r = eval_src_scope("d / d", &s);
        prop_assert_eq!(r, Value::Number(rust_decimal::Decimal::ONE));
    }
}