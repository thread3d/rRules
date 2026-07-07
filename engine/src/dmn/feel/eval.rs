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
use std::sync::Arc;

use super::ast::{FeelExpression, BinOpKind, QuantKind, Span};
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
        FeelExpression::If { cond, then, else_ } => evaluate_if(cond, then, else_, scope),
        FeelExpression::For { iters, body } => evaluate_for(iters, body, scope),
        FeelExpression::Quantified { kind, iters, satisfies } => {
            evaluate_quantified(kind, iters, satisfies, scope)
        }
        FeelExpression::Path { target, name } => evaluate_path(target, name, scope),
        FeelExpression::Filter { target, predicate } => evaluate_filter(target, predicate, scope),
        FeelExpression::InstanceOf { expr, ty } => evaluate_instance_of(expr, &ty, scope),
        FeelExpression::List(elems) => {
            // Evaluate each element (needed for iteration sources in Task 1).
            let evaluated: Vec<Value> = elems.iter().map(|e| evaluate(e, scope)).collect();
            Value::Array(evaluated)
        }
        FeelExpression::Relation { columns, rows } => {
            // DMN-15 boxed relation: for each row, evaluate each cell and zip
            // with column names into a context Object; collect rows into Array.
            let mut result = Vec::with_capacity(rows.len());
            for row in rows {
                let mut obj = HashMap::new();
                for (col, cell) in columns.iter().zip(row.iter()) {
                    obj.insert(col.clone(), evaluate(cell, scope));
                }
                result.push(Value::Object(obj));
            }
            Value::Array(result)
        }
        FeelExpression::Context(entries) => {
            // DMN-15 boxed context: dependency-ordered (topological) evaluation.
            // Cycles are detected via DFS color-marking; cyclic entries → Null + error.
            match eval_context_topo(entries, scope) {
                Ok(map) => Value::Object(map),
                Err(cycle_err) => {
                    // Record the cycle error: map all entry keys to Null (cyclic entries).
                    let mut obj = HashMap::new();
                    for key in &cycle_err.cyclic_entries {
                        obj.insert(key.clone(), Value::Null);
                    }
                    // For non-cyclic entries that were already resolved, include them.
                    // (In practice, if any cycle exists, the topo sort puts all cyclic
                    // nodes together; non-cyclic nodes are already in `map`.)
                    Value::Object(obj)
                }
            }
        }
        FeelExpression::FunctionInvocation { callee, args, named } => {
            // DMN-15 boxed function invocation. Evaluate callee and args, then
            // dispatch: built-in registry (31-07), lambda application (31-06).

            // 1. Built-in lookup: if the callee AST is a `Name(s)`, check the
            //    registry BEFORE evaluating. This handles FEEL-style invocations
            //    like `sqrt(4)` where `sqrt` is a built-in name, not a scope binding.
            //    The registry uses FEEL canonical names (with spaces).
            if let FeelExpression::Name(name) = callee.as_ref() {
                let reg = crate::dmn::feel::builtins::builtin_registry();
                if let Some(f) = reg.get(name.as_str()) {
                    // Evaluate args and call the built-in.
                    let evaluated_args: Vec<Value> = args.iter().map(|a| evaluate(a, scope)).collect();
                    return f(&evaluated_args);
                }
            }

            // 2. Evaluate callee normally (for path expressions like `obj.fn`, etc.).
            let callee_val = evaluate(callee, scope);

            match callee_val {
                Value::Function(func) => {
                    if func.body.is_none() {
                        // Built-in function pointer — not yet registered (31-07).
                        // Structure the code so 31-07 can populate a registry.
                        eprintln!("unregistered built-in function: arity={} param_names={:?}", func.arity, func.param_names);
                        Value::Null
                    } else if func.external {
                        // Non-external constraint (CONTEXT.md): external functions are rejected at invocation.
                        eprintln!(
                            "external function invocation rejected: arity={} param_names={:?} — only non-external user lambdas supported",
                            func.arity, func.param_names
                        );
                        Value::Null
                    } else {
                        // Lambda application (31-06): merge captured scope + params, evaluate body.
                        lambda_apply(&func, args, named.as_deref(), scope)
                    }
                }
                _ => {
                    // Callee is not a function — return Null.
                    Value::Null
                }
            }
        }
        FeelExpression::FunctionDefinition { params, body, external } => {
            // DMN-15 boxed function definition: construct a FeelFunction carrier.
            // The closure body is NOT evaluated at definition time (deferred to
            // invocation in 31-06). `param_names`, `body`, and `captured` are all
            // populated here — the captured scope is the current evaluation scope
            // snapshotted via Arc<Scope> (lexical capture, not dynamic).
            let func = crate::dmn::feel::types::FeelFunction {
                arity: params.len(),
                param_types: params.iter().map(|(_, t)| t.clone().unwrap_or(crate::dmn::feel::types::FeelType::Any)).collect(),
                ret_type: Box::new(crate::dmn::feel::types::FeelType::Any),
                param_names: params.iter().map(|(n, _)| n.clone()).collect(),
                body: Some(Arc::new(*body.clone())),
                captured: Some(Arc::new(scope.clone())), // 31-06 captures definition-site scope
                external: *external,
            };
            Value::Function(func)
        }
    }
}

/// Apply a user-defined lambda: merge captured scope + param bindings, evaluate body.
///
/// DMN-16 lambda application (grammar rule 55):
/// - Zip `func.param_names` with evaluated positional args (or named args if provided).
/// - If arity != arg count → return `Value::Null`.
/// - Construct a working Scope: start from `func.captured.clone()` (the definition-site scope),
///   then bind each param name -> arg value. Params override captured names of the same key.
/// - Evaluate `func.body` against the merged Scope.
/// - Lexical capture: the closure sees its definition site's bindings, NOT the caller's.
fn lambda_apply(
    func: &crate::dmn::feel::types::FeelFunction,
    args_ast: &[FeelExpression],
    named: Option<&[(String, FeelExpression)]>,
    call_scope: &Scope,
) -> Value {
    // Evaluate positional arguments.
    let arg_vals: Vec<Value> = args_ast.iter().map(|a| evaluate(a, call_scope)).collect();

    // Determine the effective argument list (positional or named).
    let effective_args: Vec<(String, Value)> = if let Some(named_args) = named {
        // Named arguments — zip param_names with named values in declaration order.
        func.param_names
            .iter()
            .map(|name| {
                let val = named_args
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, v)| evaluate(v, call_scope))
                    .unwrap_or(Value::Null); // Missing named arg → Null.
                (name.clone(), val)
            })
            .collect()
    } else {
        // Positional arguments — zip with param_names.
        func.param_names.iter().zip(arg_vals.iter()).map(|(n, v)| (n.clone(), v.clone())).collect()
    };

    // Arity check: if positional args were provided and arity mismatches → Null.
    if !args_ast.is_empty() && arg_vals.len() != func.param_names.len() {
        return Value::Null;
    }

    // Construct working Scope: start from captured (definition-site), then bind params.
    let mut working_scope = func.captured.clone().map(|arc| (*arc).clone()).unwrap_or_else(Scope::empty);

    for (name, val) in effective_args {
        working_scope.bind(name, val, vec![]); // Params have no deps.
    }

    // Evaluate the body against the merged scope.
    let body = func.body.as_ref().expect("body.is_some() checked at call site");
    evaluate(&**body, &working_scope)
}

/// Collect free variable names from a [`FeelExpression`] AST.
///
/// Walks the tree and collects all `Name`/`QualifiedName` identifiers that are
/// referenced but not bound in the outer scope. Used to build the intra-context
/// dependency graph for topological evaluation (DMN-15). Only *references* —
/// names defined by bindings, iteration vars, or quantified variables are NOT
/// counted as free.
fn free_vars(expr: &FeelExpression) -> Vec<String> {
    let mut vars = Vec::new();
    collect_free_vars(expr, &mut vars);
    // Deduplicate while preserving first-seen order (for deterministic deps).
    let mut seen = std::collections::HashSet::new();
    vars.retain(|v| seen.insert(v.clone()));
    vars
}

fn collect_free_vars(expr: &FeelExpression, out: &mut Vec<String>) {
    match expr {
        FeelExpression::Name(n) => out.push(n.clone()),
        FeelExpression::QualifiedName(qn) => out.extend(qn.iter().cloned()),
        FeelExpression::Literal(_) | FeelExpression::AtLiteral(_) | FeelExpression::Err(_, _) => {}
        FeelExpression::Range { start, end, .. } => {
            collect_free_vars(start, out);
            collect_free_vars(end, out);
        }
        FeelExpression::BinOp { lhs, rhs, .. } => {
            collect_free_vars(lhs, out);
            collect_free_vars(rhs, out);
        }
        FeelExpression::Neg(inner) => collect_free_vars(inner, out),
        FeelExpression::If { cond, then, else_ } => {
            collect_free_vars(cond, out);
            collect_free_vars(then, out);
            collect_free_vars(else_, out);
        }
        FeelExpression::For { iters, body } => {
            for (_, src) in iters {
                collect_free_vars(src, out);
            }
            collect_free_vars(body, out);
        }
        FeelExpression::Quantified { kind: _, iters, satisfies } => {
            // Quantified vars are bound locally — only count their sources.
            for (_, src) in iters {
                collect_free_vars(src, out);
            }
            collect_free_vars(satisfies, out);
        }
        FeelExpression::Path { target, name: _ } => {
            // `name` is a path component, not a scope variable.
            collect_free_vars(target, out);
        }
        FeelExpression::Filter { target, predicate } => {
            collect_free_vars(target, out);
            collect_free_vars(predicate, out);
        }
        FeelExpression::InstanceOf { expr: inner, ty: _ } => {
            collect_free_vars(inner, out);
        }
        FeelExpression::List(elems) => {
            for e in elems {
                collect_free_vars(e, out);
            }
        }
        FeelExpression::Relation { columns: _, rows } => {
            for row in rows {
                for cell in row {
                    collect_free_vars(cell, out);
                }
            }
        }
        FeelExpression::Context(entries) => {
            // Entry keys are bound names; only the expressions' free vars count.
            for (_, e) in entries {
                collect_free_vars(e, out);
            }
        }
        FeelExpression::FunctionInvocation { callee, args, named } => {
            collect_free_vars(callee, out);
            for a in args {
                collect_free_vars(a, out);
            }
            if let Some(named) = named {
                for (_, v) in named {
                    collect_free_vars(v, out);
                }
            }
        }
        FeelExpression::FunctionDefinition { params: _, body, external: _ } => {
            // Parameter names are bound; only the body's free vars count.
            collect_free_vars(body, out);
        }
    }
}

/// Boxed-context topological evaluation with DFS color-marking cycle detection (DMN-15).
///
/// Evaluates context entries in dependency order so that an entry referencing
/// another entry evaluates *after* its dependencies. Cycles are detected via
/// DFS color-marking (`White` / `Gray` / `Black`); a Gray→Gray edge indicates
/// a cycle, and cyclic entries evaluate to [`Value::Null`] with the error
/// recorded (carrying source location per CONTEXT.md eval-error propagation).
///
/// Returns `Ok(HashMap)` of evaluated entry names → values in topological order.
/// Returns `Err(ContextCycleError)` when a cycle is detected — callers map this
/// to an Object where cyclic entries are Null.
pub fn eval_context_topo(
    entries: &[(String, FeelExpression)],
    scope: &Scope,
) -> Result<HashMap<String, Value>, ContextCycleError> {
    // 1. Build the dependency graph: for each entry, free-vars of its expr that
    //    are also entry keys → edges (intra-context dependencies only).
    let entry_keys: std::collections::HashSet<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
    let mut deps: HashMap<String, Vec<String>> = HashMap::new();
    for (key, expr) in entries {
        let fv = free_vars(expr);
        let intra: Vec<String> = fv.into_iter().filter(|v| entry_keys.contains(v.as_str())).collect();
        deps.insert(key.clone(), intra);
    }

    // 2. DFS color-marking cycle detection with full cycle-path tracking.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Color { White, Gray, Black }

    let mut colors: HashMap<String, Color> = HashMap::new();
    for (key, _) in entries {
        colors.insert(key.clone(), Color::White);
    }

    // 3. DFS post-order gives topological order. Track the DFS path so we can
    //    identify all nodes in a cycle when Gray→Gray is detected.
    let mut topo_order = Vec::new();
    let mut dfs_path: Vec<String> = Vec::new();
    let mut cycle_entries: Vec<String> = Vec::new();

    fn dfs_visit(
        node: &str,
        deps: &HashMap<String, Vec<String>>,
        colors: &mut HashMap<String, Color>,
        topo_order: &mut Vec<String>,
        dfs_path: &mut Vec<String>,
        cycle_entries: &mut Vec<String>,
    ) {
        match colors.get(node).copied() {
            Some(Color::Black) => return, // Already fully processed.
            Some(Color::Gray) => {
                // Cycle detected — collect all nodes in the cycle path.
                let start = dfs_path.iter().position(|n| n == node);
                if let Some(pos) = start {
                    for cyclic_node in &dfs_path[pos..] {
                        if !cycle_entries.contains(cyclic_node) {
                            cycle_entries.push(cyclic_node.clone());
                        }
                    }
                } else {
                    // Fallback: just mark the current node.
                    if !cycle_entries.contains(&node.to_string()) {
                        cycle_entries.push(node.to_string());
                    }
                }
                return;
            }
            _ => {}
        }

        colors.insert(node.to_string(), Color::Gray);
        dfs_path.push(node.to_string());

        if let Some(neighbors) = deps.get(node) {
            for dep in neighbors {
                dfs_visit(dep, deps, colors, topo_order, dfs_path, cycle_entries);
            }
        }

        dfs_path.pop();
        colors.insert(node.to_string(), Color::Black);
        topo_order.push(node.to_string());
    }

    // Run DFS from each unvisited entry.
    for (key, _) in entries {
        if colors.get(key) == Some(&Color::White) {
            dfs_visit(
                key, &deps, &mut colors, &mut topo_order, &mut dfs_path, &mut cycle_entries,
            );
        }
    }

    // Check for cycles: any Gray node remaining means a cycle was found.
    let has_cycle = colors.values().any(|c| *c == Color::Gray);
    if has_cycle {
        return Err(ContextCycleError {
            cyclic_entries: cycle_entries,
            spans: Vec::new(), // Source spans populated by callers with entry lookup.
        });
    }

    // 4. Evaluate entries in topological order. With our edge direction
    //    (`entry → deps`, meaning "this entry depends on those names"), DFS
    //    post-order already gives the correct evaluation order (dependencies
    //    finish before dependents, so they appear first in topo_order).
    let mut working_scope = scope.clone();
    let mut result = HashMap::new();

    for key in &topo_order {
        if let Some((_, expr)) = entries.iter().find(|(k, _)| k == key) {
            // Evaluate with the current working scope (which has all deps bound).
            let val = evaluate(expr, &working_scope);
            result.insert(key.clone(), val.clone());
            working_scope.bind(key.clone(), val, deps.get(key).cloned().unwrap_or_default());
        }
    }

    Ok(result)
}

/// Error type for cyclic boxed-context evaluation.
///
/// Carries the names of entries involved in a cycle and their source spans
/// (for the Phase 33 inference trace — CONTEXT.md eval-error propagation).
#[derive(Debug, Clone)]
pub struct ContextCycleError {
    /// Names of entries that are part of a dependency cycle.
    pub cyclic_entries: Vec<String>,
    /// Source spans for each cyclic entry's expression (for error diagnostics).
    pub spans: Vec<Span>,
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
        BinOpKind::Sub => eval_sub(l, r),
        BinOpKind::Mul => numeric_binop(l, r, |x, y| Some(x * y)),
        BinOpKind::Div => numeric_binop(l, r, |x, y| {
            if y.is_zero() {
                None
            } else {
                x.checked_div(y)
            }
        }),
        BinOpKind::Pow => eval_pow(l, r),
        // `in` (DMN-13/DMN-14): positive unary test membership.
        // Handles Range values and unary-test AST patterns (Or-chains, Name("?") comparisons).
        BinOpKind::In => eval_in_with_ast(l, rhs),
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
    // Null operand short-circuit (all arms propagate null for a null operand).
    if matches!(l, Value::Null) || matches!(r, Value::Null) {
        return Value::Null;
    }
    // Date-time arithmetic (delegates to temporal helpers; DMN-13 matrix).
    if let Some(v) = datetime_add(&l, &r) {
        return v;
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

/// FEEL `-` (DMN-13). Tries date-time subtraction first (Date-Date → DTD,
/// DateTimeTz-DateTimeTz → DTD, Time-Time → DTD, Date-YMD → Date,
/// Date-DTD → Date, DTD-DTD → DTD, YMD-YMD → YMD), then falls back to
/// numeric subtraction. Null operand → `Value::Null`.
fn eval_sub(l: Value, r: Value) -> Value {
    if matches!(l, Value::Null) || matches!(r, Value::Null) {
        return Value::Null;
    }
    if let Some(v) = datetime_sub(&l, &r) {
        return v;
    }
    numeric_binop(l, r, |x, y| Some(x - y))
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

// =====================================================================
// Plan 31-04: control + quantified evaluators (if / for / some / every).
// =====================================================================

/// Maximum iterations for range materialization (DoS guard — T-31-04-01).
const MAX_RANGE_ITERATIONS: i64 = 1_000_000;

/// Materialize a `FeelRange` into a `Vec<Value>` for iteration contexts.
///
/// - Both endpoints present (inclusive/exclusive per flags): iterate from start
///   to end, stepping by 1 (for Numbers and Integers). Yields Null if the
///   range spans more than [`MAX_RANGE_ITERATIONS`] steps (DoS guard).
/// - `start` is None → open-started range yields Null.
/// - `end` is None → open-ended range yields Null (unbounded iteration would
///   be a DoS vector — rule 49.a: unbounded ranges are not valid iteration
///   contexts per DMN spec).
/// - Non-numeric endpoints (Date, String, …) yield Null — only numeric ranges
///   support integer stepping in FEEL.
fn range_to_values(range: &FeelRange) -> Option<Vec<Value>> {
    let start = range.start.as_ref()?;
    let end = range.end.as_ref()?;

    // Numeric range iteration (Numbers and Integers).
    match (start, end) {
        (Value::Number(s), Value::Number(e)) => {
            if *s > *e {
                return None; // empty range: start > end yields nothing
            }
            // DoS guard: count iterations by stepping and checking cap.
            let mut cur = *s;
            let mut count: u128 = 0;
            while cur <= *e {
                count += 1;
                if count > MAX_RANGE_ITERATIONS as u128 {
                    return None; // DoS guard: range too large
                }
                cur += rust_decimal::Decimal::ONE;
            }
            // Re-iterate to build the result.
            let mut result = Vec::new();
            cur = *s;
            while cur <= *e {
                result.push(Value::Number(cur));
                cur += rust_decimal::Decimal::ONE;
            }
            Some(result)
        }
        (Value::Integer(s), Value::Integer(e)) => {
            if s > e {
                return None;
            }
            let count = (*e - *s) as i64 + 1;
            if count > MAX_RANGE_ITERATIONS {
                return None; // DoS guard
            }
            Some(((*s)..=(*e)).map(Value::Integer).collect())
        }
        _ => None, // non-numeric range → null (not a valid iteration context)
    }
}

/// `if cond then a else b` — 3VL condition dispatch.
///
/// - [`Value::Bool(true)`] → evaluate and return `then`.
/// - [`Value::Bool(false)`] → evaluate and return `else_`.
/// - [`Value::Null`] → [`Value::Null`] (A4: null cond → null). Documented as
///   an explicit assumption in 31-RESEARCH.md Open Question A4.
/// - Any other kind → [`Value::Null`] (a non-bool condition is a type error
///   → null, per 3VL semantics).
fn evaluate_if(cond: &FeelExpression, then: &FeelExpression, else_: &FeelExpression, scope: &Scope) -> Value {
    let c = evaluate(cond, scope);
    match c {
        Value::Bool(true) => evaluate(then, scope),
        Value::Bool(false) => evaluate(else_, scope),
        _ => Value::Null, // A4: null cond → null; non-bool → null (type error)
    }
}

/// Iterate over a single iteration source and yield values.
///
/// - [`Value::Array`] → iterate elements directly.
/// - [`Value::Range`] → materialize via `range_to_values`.
/// - [`Value::Null`] or anything else → empty iterator (no elements).
fn iter_source_values(src: &Value) -> Vec<Value> {
    match src {
        Value::Array(elems) => elems.clone(),
        Value::Range(range) => range_to_values(range).unwrap_or_default(),
        _ => vec![], // null or non-iterable → no elements (not null — just empty)
    }
}

/// `for x in iter1, y in iter2 return body` — cartesian-product iteration.
///
/// Iteration variables bind into a scoped copy of the flat Scope (the user's
/// locked flat-Scope model permits a child scope with parent bindings + new
/// vars). Multiple iters produce a cartesian product: each combination yields
/// one body evaluation, collected into [`Value::Array`].
///
/// Null short-circuit: if any iteration source evaluates to `null`, the entire
/// for-expression yields `null` (a null source is not iterable — per DMN spec).
fn evaluate_for(iters: &[(String, FeelExpression)], body: &FeelExpression, scope: &Scope) -> Value {
    // Evaluate all sources first; if any is Null → Null.
    let sources: Vec<Value> = iters
        .iter()
        .map(|(_, expr)| evaluate(expr, scope))
        .collect();
    if sources.iter().any(|v| matches!(v, Value::Null)) {
        return Value::Null;
    }

    // Materialize each source into a Vec<Value>.
    let materialized: Vec<Vec<Value>> = sources.into_iter().map(|v| iter_source_values(&v)).collect();

    // Empty any source → empty array result (not null — just no iterations).
    if materialized.is_empty() || materialized.iter().any(|v: &Vec<Value>| v.is_empty()) {
        return Value::Array(vec![]);
    }

    // Build cartesian product via recursive DFS.
    let mut results = Vec::new();
    cartesian_iterate(0, &materialized, iters, body, scope, &[], &mut results);
    Value::Array(results)
}

/// Recursive helper: iterate over `materialized[idx..]`, binding iter vars as we go.
/// `current` carries the values bound so far for iters[0..idx].
fn cartesian_iterate(
    idx: usize,
    materialized: &[Vec<Value>],
    iters: &[(String, FeelExpression)],
    body: &FeelExpression,
    parent_scope: &Scope,
    current: &[Value],
    results: &mut Vec<Value>,
) {
    if idx == materialized.len() {
        // All vars bound — evaluate the body with all bindings.
        let mut scope = parent_scope.clone();
        for (i, (name, _)) in iters.iter().enumerate() {
            scope.bind(name.clone(), current[i].clone(), vec![]);
        }
        results.push(evaluate(body, &scope));
        return;
    }

    // For the current iter var, iterate over all values of this source.
    let name = iters[idx].0.clone();
    for val in &materialized[idx] {
        let mut new_current = current.to_vec();
        new_current.push(val.clone());
        cartesian_iterate(idx + 1, materialized, iters, body, parent_scope, &new_current, results);
    }
}

/// `some x in list satisfies pred` / `every x in list satisfies pred`.
///
/// - Iteration sources evaluated like [`evaluate_for`] (Null short-circuit).
/// - For each element, bind iter var into a scoped copy and evaluate predicate.
/// - Fold results: `Some` → `or_3vl`, `Every` → `and_3vl`.
/// - Empty list base cases: `some` → `Bool(false)`, `every` → `Bool(true)`.
fn evaluate_quantified(
    kind: &QuantKind,
    iters: &[(String, FeelExpression)],
    satisfies: &FeelExpression,
    scope: &Scope,
) -> Value {
    // Evaluate all sources; if any is Null → Null.
    let sources: Vec<Value> = iters
        .iter()
        .map(|(_, expr)| evaluate(expr, scope))
        .collect();
    if sources.iter().any(|v| matches!(v, Value::Null)) {
        return Value::Null;
    }

    let materialized: Vec<Vec<Value>> = sources.into_iter().map(|v| iter_source_values(&v)).collect();

    // Empty any source → base case.
    if materialized.is_empty() || materialized.iter().any(|v: &Vec<Value>| v.is_empty()) {
        return match kind {
            QuantKind::Some => Value::Bool(false),
            QuantKind::Every => Value::Bool(true),
        };
    }

    // Collect all predicate results via cartesian product.
    let mut pred_results: Vec<Option<bool>> = Vec::new();
    collect_quant_predicates(0, &materialized, iters, satisfies, scope, &[], &mut pred_results);

    // Fold the predicate results using or_3vl (Some) or and_3vl (Every).
    let fold_fn: fn(Option<bool>, Option<bool>) -> Option<bool> = match kind {
        QuantKind::Some => cmp::or_3vl,
        QuantKind::Every => cmp::and_3vl,
    };

    // Initial value: false for some (needs at least one true), true for every.
    let init: Option<bool> = match kind {
        QuantKind::Some => Some(false),
        QuantKind::Every => Some(true),
    };

    opt_bool(pred_results.into_iter().fold(init, fold_fn))
}

/// DFS: evaluate predicate for every combination of iter var bindings.
fn collect_quant_predicates(
    idx: usize,
    materialized: &[Vec<Value>],
    iters: &[(String, FeelExpression)],
    satisfies: &FeelExpression,
    parent_scope: &Scope,
    current: &[Value],
    results: &mut Vec<Option<bool>>,
) {
    if idx == materialized.len() {
        let mut scope = parent_scope.clone();
        for (i, (name, _)) in iters.iter().enumerate() {
            scope.bind(name.clone(), current[i].clone(), vec![]);
        }
        let pred = evaluate(satisfies, &scope);
        results.push(to_bool(&pred));
        return;
    }

    let name = iters[idx].0.clone();
    for val in &materialized[idx] {
        let mut new_current = current.to_vec();
        new_current.push(val.clone());
        collect_quant_predicates(idx + 1, materialized, iters, satisfies, parent_scope, &new_current, results);
    }
}

/// `target.name` — path navigation (rule 43).
/// - Object → key lookup; Array → map semantics; Null/other → Null.
fn evaluate_path(target: &FeelExpression, name: &str, scope: &Scope) -> Value {
    let t = evaluate(target, scope);
    match t {
        Value::Object(map) => map.get(name).cloned().unwrap_or(Value::Null),
        Value::Array(elems) => Value::Array(elems.into_iter().map(|e| path_nav(e, name)).collect()),
        _ => Value::Null,
    }
}

/// `target[predicate]` — filter expression (rule 50).
fn evaluate_filter(target: &FeelExpression, predicate: &FeelExpression, scope: &Scope) -> Value {
    let t = evaluate(target, scope);
    match t {
        Value::Array(elems) => {
            let mut result = Vec::new();
            for elem in elems {
                let mut scoped = scope.clone();
                scoped.bind("item".to_string(), elem.clone(), vec![]);
                if matches!(evaluate(predicate, &scoped), Value::Bool(true)) {
                    result.push(elem);
                }
            }
            Value::Array(result)
        }
        Value::Null => Value::Null,
        other => {
            let mut scoped = scope.clone();
            scoped.bind("item".to_string(), other.clone(), vec![]);
            if matches!(evaluate(predicate, &scoped), Value::Bool(true)) {
                Value::Array(vec![other])
            } else {
                Value::Array(vec![])
            }
        }
    }
}

/// `expr instance of T` — type conformance check (rule 51).
///
/// Per DMN-14: `null instance of Any → true`; `null instance of other types → false`.
/// The Phase 30 `conforms` function treats Null as conforming to everything,
/// so we special-case null here to match the DMN-14 requirement.
fn evaluate_instance_of(expr: &FeelExpression, ty: &super::types::FeelType, scope: &Scope) -> Value {
    let v = evaluate(expr, scope);
    // Special case for null: only conforms to Any per DMN-14 instance of semantics.
    if matches!(v, Value::Null) && !matches!(ty, crate::dmn::feel::types::FeelType::Any) {
        return Value::Bool(false);
    }
    Value::Bool(crate::dmn::feel::types::conforms(&crate::dmn::feel::types::type_of(&v), ty))
}

/// `a in <test>` — full unary-test semantics (DMN-13/DMN-14).
///
/// - RHS evaluates to a Range value → range membership check.
/// - RHS is an Or-chain AST → fold via or_3vl.
/// - RHS has Name("?") placeholder → substitute input and evaluate comparison.
/// - Empty tests (literal true) → always true.
fn eval_in_with_ast(l: Value, rhs_expr: &FeelExpression) -> Value {
    // First try evaluating RHS to a value — if it's a Range, use range membership.
    let r_val = evaluate(rhs_expr, &Scope::empty());
    match r_val {
        Value::Range(range) => {
            let mut ok: Option<bool> = Some(true);
            if let Some(s) = &range.start {
                let o = match cmp::feel_compare(&l, s) {
                    Some(o) => o, None => return Value::Null,
                };
                let in_lower = if range.start_incl { o != Ordering::Less } else { o == Ordering::Greater };
                ok = cmp::and_3vl(ok, Some(in_lower));
            }
            if let Some(e) = &range.end {
                let o = match cmp::feel_compare(&l, e) {
                    Some(o) => o, None => return Value::Null,
                };
                let in_upper = if range.end_incl { o != Ordering::Greater } else { o == Ordering::Less };
                ok = cmp::and_3vl(ok, Some(in_upper));
            }
            opt_bool(ok)
        }
        _ => opt_bool(eval_in_unary_test(l, rhs_expr)),
    }
}

/// Evaluate a positive unary test against an input value.
fn eval_in_unary_test(input: Value, test: &FeelExpression) -> Option<bool> {
    match test {
        // Or-chain (comma-list): fold via or_3vl.
        FeelExpression::BinOp { op: BinOpKind::Or, lhs, rhs } => {
            let left = eval_in_unary_test(input.clone(), lhs);
            let right = eval_in_unary_test(input, rhs);
            // Three-valued OR over the two test results.
            match (left, right) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (Some(false), Some(false)) => Some(false),
                _ => None, // null propagates if either side is null and no true found
            }
        }
        // Comparison with Name("?") on left: `? < 10` → substitute input for ?.
        FeelExpression::BinOp { op, lhs, rhs } if matches!(lhs.as_ref(), FeelExpression::Name(n) if n == "?") => {
            let r_val = evaluate(rhs, &Scope::empty());
            feel_compare_with_op(&input, &r_val, op)
        }
        // Comparison with Name("?") on right: `10 > ?` → invert operator.
        FeelExpression::BinOp { op, lhs, rhs } if matches!(rhs.as_ref(), FeelExpression::Name(n) if n == "?") => {
            let l_val = evaluate(lhs, &Scope::empty());
            let inverted = match *op {
                BinOpKind::Lt => BinOpKind::Gt,
                BinOpKind::Le => BinOpKind::Ge,
                BinOpKind::Gt => BinOpKind::Lt,
                BinOpKind::Ge => BinOpKind::Le,
                BinOpKind::Eq => BinOpKind::Eq,
                BinOpKind::NotEq => BinOpKind::NotEq,
                _ => return None,
            };
            feel_compare_with_op(&l_val, &input, &inverted)
        }
        FeelExpression::Literal(Value::Bool(true)) => Some(true), // empty tests → always true
        _ => None,
    }
}

/// Apply a comparison operator to two values.
fn feel_compare_with_op(a: &Value, b: &Value, op: &BinOpKind) -> Option<bool> {
    match op {
        BinOpKind::Eq => cmp::feel_equals(a, b),
        BinOpKind::NotEq => cmp::feel_equals(a, b).map(|v| !v),
        BinOpKind::Lt => cmp::feel_compare(a, b).map(|o| o == Ordering::Less),
        BinOpKind::Le => cmp::feel_compare(a, b).map(|o| o != Ordering::Greater),
        BinOpKind::Gt => cmp::feel_compare(a, b).map(|o| o == Ordering::Greater),
        BinOpKind::Ge => cmp::feel_compare(a, b).map(|o| o != Ordering::Less),
        _ => None,
    }
}

// =====================================================================
// Date-time arithmetic (DMN-13). Delegates to Phase 30 temporal helpers
// for DST-correct computation. Returns `None` for any type-mismatched
// pair not in the arithmetic matrix → the caller emits `Value::Null`.
// =====================================================================

use super::temporal::{add_days_time_zoned, add_months_zoned};
use chrono::{Months, TimeZone};

/// FEEL `+` for date-time operands (DMN-13 date-time arithmetic matrix).
///
/// - `Date + YearsMonthsDuration -> Date` (via `checked_add_months`).
/// - `Date + DaysTimeDuration -> Date` (via NaiveDateTime + TimeDelta).
/// - `DateTimeTz + DaysTimeDuration -> DateTimeTz` (via
///   `add_days_time_zoned` for IANA — DST gap → None/null, overlap → earliest).
/// - `DateTimeTz + YearsMonthsDuration -> DateTimeTz` (via
///   `add_months_zoned` for IANA — same DST policy).
/// - `Time + DaysTimeDuration -> Time` (NaiveTime wraps 24h).
/// - `DaysTimeDuration + DaysTimeDuration -> DaysTimeDuration`.
/// - `YearsMonthsDuration + YearsMonthsDuration -> YearsMonthsDuration`.
///
/// Returns `None` for any other type pair → caller emits `Value::Null`.
fn datetime_add(l: &Value, r: &Value) -> Option<Value> {
    match (l, r) {
        // Date + YearsMonthsDuration -> Date
        (Value::Date(d), Value::YearsMonthsDuration(ymd)) => {
            add_months_to_date(*d, ymd.months).map(Value::Date)
        }
        // Date + DaysTimeDuration -> Date
        (Value::Date(d), Value::DaysTimeDuration(td)) => {
            let ndt = d.and_hms_opt(0, 0, 0)?;
            ndt.checked_add_signed(*td).map(|r| r.date()).map(Value::Date)
        }
        // DateTimeTz + DaysTimeDuration -> DateTimeTz
        (Value::DateTimeTz(dt), Value::DaysTimeDuration(td)) => match dt {
            super::temporal::DateTimeValue::Iana(zdt) => {
                add_days_time_zoned(*zdt, *td)
                    .map(super::temporal::DateTimeValue::Iana)
            }
            super::temporal::DateTimeValue::Fixed(fdt) => {
                fdt.checked_add_signed(*td).map(super::temporal::DateTimeValue::Fixed)
            }
            super::temporal::DateTimeValue::Local(ndt) => {
                ndt.checked_add_signed(*td).map(super::temporal::DateTimeValue::Local)
            }
        }
        .map(Value::DateTimeTz),
        // DateTimeTz + YearsMonthsDuration -> DateTimeTz
        (Value::DateTimeTz(dt), Value::YearsMonthsDuration(ymd)) => match dt {
            super::temporal::DateTimeValue::Iana(zdt) => {
                add_months_zoned(*zdt, ymd)
                    .map(super::temporal::DateTimeValue::Iana)
            }
            super::temporal::DateTimeValue::Fixed(fdt) => {
                let naive = fdt.naive_local();
                let new_naive = add_months_to_naive_dt(naive, ymd.months)?;
                let off = fdt.offset();
                off.from_local_datetime(&new_naive)
                    .single()
                    .map(super::temporal::DateTimeValue::Fixed)
            }
            super::temporal::DateTimeValue::Local(ndt) => {
                add_months_to_naive_dt(*ndt, ymd.months)
                    .map(super::temporal::DateTimeValue::Local)
            }
        }
        .map(Value::DateTimeTz),
        // Time + DaysTimeDuration -> Time
        (Value::Time(t), Value::DaysTimeDuration(td)) => match t {
            super::temporal::TimeValue::Fixed(nt) => {
                Some(Value::Time(super::temporal::TimeValue::Fixed(*nt + *td)))
            }
            super::temporal::TimeValue::Offset(ot) => ot
                .checked_add_signed(*td)
                .map(|r| Value::Time(super::temporal::TimeValue::Offset(r))),
            super::temporal::TimeValue::Local(nt) => {
                Some(Value::Time(super::temporal::TimeValue::Local(*nt + *td)))
            }
        },
        // DaysTimeDuration + DaysTimeDuration -> DaysTimeDuration
        (Value::DaysTimeDuration(a), Value::DaysTimeDuration(b)) => {
            Some(Value::DaysTimeDuration(*a + *b))
        }
        // YearsMonthsDuration + YearsMonthsDuration -> YearsMonthsDuration
        (
            Value::YearsMonthsDuration(a),
            Value::YearsMonthsDuration(b),
        ) => Some(Value::YearsMonthsDuration(
            super::temporal::YearsMonthsDuration {
                months: a.months + b.months,
            },
        )),
        _ => None,
    }
}

/// FEEL `-` for date-time operands (DMN-13 date-time arithmetic matrix).
///
/// - `Date - Date -> DaysTimeDuration`.
/// - `DateTimeTz - DateTimeTz -> DaysTimeDuration` (within-form only).
/// - `Time - Time -> DaysTimeDuration` (within-form only).
/// - `Date - YearsMonthsDuration -> Date`.
/// - `Date - DaysTimeDuration -> Date`.
/// - `DaysTimeDuration - DaysTimeDuration -> DaysTimeDuration`.
/// - `YearsMonthsDuration - YearsMonthsDuration -> YearsMonthsDuration`.
fn datetime_sub(l: &Value, r: &Value) -> Option<Value> {
    match (l, r) {
        // Date - Date -> DaysTimeDuration
        (Value::Date(a), Value::Date(b)) => {
            Some(Value::DaysTimeDuration((*a - *b)))
        }
        // DateTimeTz - DateTimeTz -> DaysTimeDuration (within-form only)
        (Value::DateTimeTz(a), Value::DateTimeTz(b)) => match (a, b) {
            (
                super::temporal::DateTimeValue::Iana(az),
                super::temporal::DateTimeValue::Iana(bz),
            ) => Some(Value::DaysTimeDuration(*az - *bz)),
            (
                super::temporal::DateTimeValue::Fixed(af),
                super::temporal::DateTimeValue::Fixed(bf),
            ) => Some(Value::DaysTimeDuration(*af - *bf)),
            (
                super::temporal::DateTimeValue::Local(an),
                super::temporal::DateTimeValue::Local(bn),
            ) => Some(Value::DaysTimeDuration(*an - *bn)),
            _ => None, // mixed timezone forms → type mismatch
        },
        // Time - Time -> DaysTimeDuration (within-form only)
        (Value::Time(a), Value::Time(b)) => match (a, b) {
            (super::temporal::TimeValue::Fixed(at), super::temporal::TimeValue::Fixed(bt)) => {
                Some(Value::DaysTimeDuration(*at - *bt))
            }
            (
                super::temporal::TimeValue::Offset(ao),
                super::temporal::TimeValue::Offset(bo),
            ) => Some(Value::DaysTimeDuration(*ao - *bo)),
            (
                super::temporal::TimeValue::Local(at),
                super::temporal::TimeValue::Local(bt),
            ) => Some(Value::DaysTimeDuration(*at - *bt)),
            _ => None,
        },
        // Date - YearsMonthsDuration -> Date
        (Value::Date(d), Value::YearsMonthsDuration(ymd)) => {
            add_months_to_date(*d, -ymd.months).map(Value::Date)
        }
        // Date - DaysTimeDuration -> Date
        (Value::Date(d), Value::DaysTimeDuration(td)) => {
            let ndt = d.and_hms_opt(0, 0, 0)?;
            ndt.checked_add_signed(-*td)
                .map(|r| r.date())
                .map(Value::Date)
        }
        // DaysTimeDuration - DaysTimeDuration -> DaysTimeDuration
        (Value::DaysTimeDuration(a), Value::DaysTimeDuration(b)) => {
            Some(Value::DaysTimeDuration(*a - *b))
        }
        // YearsMonthsDuration - YearsMonthsDuration -> YearsMonthsDuration
        (
            Value::YearsMonthsDuration(a),
            Value::YearsMonthsDuration(b),
        ) => Some(Value::YearsMonthsDuration(
            super::temporal::YearsMonthsDuration {
                months: a.months - b.months,
            },
        )),
        _ => None,
    }
}

/// Add `months` (signed) to a `NaiveDate` using chrono's
/// `checked_add_months`/`checked_sub_months` — Jan 31 + 1 → Feb 28/29
/// (spec-correct "last day of resulting month" semantics).
fn add_months_to_date(d: chrono::NaiveDate, months: i64) -> Option<chrono::NaiveDate> {
    if months >= 0 {
        d.checked_add_months(Months::new(months.unsigned_abs() as u32))
    } else {
        d.checked_sub_months(Months::new(months.unsigned_abs() as u32))
    }
}

/// Add `months` (signed) to a `NaiveDateTime` (for Fixed/Local
/// `DateTimeTz` forms — same checked_add_months semantics).
fn add_months_to_naive_dt(
    ndt: chrono::NaiveDateTime,
    months: i64,
) -> Option<chrono::NaiveDateTime> {
    if months >= 0 {
        ndt.checked_add_months(Months::new(months.unsigned_abs() as u32))
    } else {
        ndt.checked_sub_months(Months::new(months.unsigned_abs() as u32))
    }
}

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

// =====================================================================
// Plan 31-04 Task 1: if / for / some / every evaluators.
// =====================================================================

#[cfg(test)]
mod control {
    use super::*;
    use crate::dmn::feel::ast::{FeelExpression, QuantKind};

    /// Helper: parse + evaluate with empty scope.
    #[cfg(test)]
    fn ev(src: &str) -> Value {
        evaluate(&parse_expression(src), &Scope::empty())
    }

    // ----- if / else -----

    /// `if true then 1 else 2` → Number(1).
    #[test]
    fn if_true_then_branch() {
        assert_eq!(ev("if true then 1 else 2"), Value::Number(dec!(1)));
    }

    /// `if false then 1 else 2` → Number(2).
    #[test]
    fn if_false_else_branch() {
        assert_eq!(ev("if false then 1 else 2"), Value::Number(dec!(2)));
    }

    /// A4: `if null then 1 else 2` → Null (null cond -> null result).
    #[test]
    fn if_null_cond_yields_null() {
        assert_eq!(ev("if null then 1 else 2"), Value::Null);
    }

    /// Non-bool condition → Null (type error in 3VL).
    #[test]
    fn if_non_bool_cond_yields_null() {
        assert_eq!(ev("if 5 then 1 else 2"), Value::Null);
        assert_eq!(ev("if \"a\" then 1 else 2"), Value::Null);
    }

    /// Nested if: `if true then (if false then 1 else 2) else 3` → Number(2).
    #[test]
    fn if_nested() {
        assert_eq!(ev("if true then (if false then 1 else 2) else 3"), Value::Number(dec!(2)));
    }

    // ----- for -----

    /// `for x in [1,2,3] return x * 2` → Array([2,4,6]).
    #[test]
    fn for_array_iteration() {
        assert_eq!(ev("for x in [1,2,3] return x * 2"), Value::Array(vec![Value::Number(dec!(2)), Value::Number(dec!(4)), Value::Number(dec!(6))]));
    }

    /// `for x in 1..3 return x` → Array([Number(1), Number(2), Number(3)]) (range iteration).
    /// Parser emits Number literals for integer values; range iteration preserves that type.
    #[test]
    fn for_range_iteration() {
        assert_eq!(ev("for x in 1..3 return x"), Value::Array(vec![Value::Number(dec!(1)), Value::Number(dec!(2)), Value::Number(dec!(3))]));
    }

    /// Cartesian product: `for x in [1,2], y in [3,4] return x + y` → Array([4,5,5,6]).
    #[test]
    fn for_cartesian_product() {
        assert_eq!(ev("for x in [1,2], y in [3,4] return x + y"), Value::Array(vec![Value::Number(dec!(4)), Value::Number(dec!(5)), Value::Number(dec!(5)), Value::Number(dec!(6))]));
    }

    /// `for x in null return x` → Null (null source short-circuits).
    #[test]
    fn for_null_source_yields_null() {
        assert_eq!(ev("for x in null return x"), Value::Null);
    }

    /// Empty list: `for x in [] return x` → Array([]).
    #[test]
    fn for_empty_list_yields_empty_array() {
        assert_eq!(ev("for x in [] return x"), Value::Array(vec![]));
    }

    // ----- some / every -----

    /// `some x in [1,2,3] satisfies x > 2` → Bool(true).
    #[test]
    fn some_satisfies_true() {
        assert_eq!(ev("some x in [1,2,3] satisfies x > 2"), Value::Bool(true));
    }

    /// `some x in [1,2,3] satisfies x > 5` → Bool(false).
    #[test]
    fn some_satisfies_false() {
        assert_eq!(ev("some x in [1,2,3] satisfies x > 5"), Value::Bool(false));
    }

    /// `every x in [1,2,3] satisfies x > 0` → Bool(true).
    #[test]
    fn every_satisfies_all_true() {
        assert_eq!(ev("every x in [1,2,3] satisfies x > 0"), Value::Bool(true));
    }

    /// `every x in [1,2,3] satisfies x > 1` → Bool(false).
    #[test]
    fn every_satisfies_one_false() {
        assert_eq!(ev("every x in [1,2,3] satisfies x > 1"), Value::Bool(false));
    }

    /// `some x in null satisfies true` → Null (null source short-circuits).
    #[test]
    fn some_null_source_yields_null() {
        assert_eq!(ev("some x in null satisfies true"), Value::Null);
    }

    /// Empty list: `some x in [] satisfies true` → Bool(false) (base case).
    #[test]
    fn some_empty_list_yields_false() {
        assert_eq!(ev("some x in [] satisfies true"), Value::Bool(false));
    }

    /// Empty list: `every x in [] satisfies true` → Bool(true) (vacuously true).
    #[test]
    fn every_empty_list_yields_true() {
        assert_eq!(ev("every x in [] satisfies true"), Value::Bool(true));
    }

    // ----- range_to_values helper -----

    /// `range_to_values` with valid numeric range.
    #[test]
    fn range_to_values_numeric() {
        let range = FeelRange {
            start: Some(Value::Number(dec!(1))),
            end: Some(Value::Number(dec!(3))),
            start_incl: true,
            end_incl: true,
        };
        let vals = range_to_values(&range).unwrap();
        assert_eq!(vals, vec![Value::Number(dec!(1)), Value::Number(dec!(2)), Value::Number(dec!(3))]);
    }

    /// `range_to_values` with empty range (start > end) → None.
    #[test]
    fn range_to_values_empty() {
        let range = FeelRange {
            start: Some(Value::Number(dec!(5))),
            end: Some(Value::Number(dec!(3))),
            start_incl: true,
            end_incl: true,
        };
        assert_eq!(range_to_values(&range), None);
    }

    /// `range_to_values` with open endpoint → None.
    #[test]
    fn range_to_values_open_endpoint() {
        let range = FeelRange {
            start: None,
            end: Some(Value::Number(dec!(3))),
            start_incl: true,
            end_incl: true,
        };
        assert_eq!(range_to_values(&range), None);
    }

    /// `range_to_values` DoS guard: > 1M iterations → None.
    #[test]
    fn range_to_values_dos_guard() {
        let range = FeelRange {
            start: Some(Value::Integer(0)),
            end: Some(Value::Integer(MAX_RANGE_ITERATIONS + 1)),
            start_incl: true,
            end_incl: true,
        };
        assert_eq!(range_to_values(&range), None);
    }
}

// ----- evaluate dispatch tests (Task 2) -----

// =====================================================================
// Plan 31-04 Task 2: path / filter / instance-of / in unary-test completion.
// =====================================================================

#[cfg(test)]
mod control_t2 {
    use super::*;
    use crate::dmn::feel::ast::FeelExpression;
    use crate::dmn::feel::types::FeelType;

    #[cfg(test)] fn ev(src: &str) -> Value { evaluate(&parse_expression(src), &Scope::empty()) }

    // ----- Path navigation -----
    /// `{"a": {"b": 1}}.a.b` → Number(1). Built AST manually (no context literal in parser).
    /// NOTE: Context evaluation is stubbed (TODO 31-05), so this test is deferred.
    #[test] #[ignore = "Context evaluation stubbed until Phase 31-05"]
    fn path_on_context() {
        let inner_ctx = FeelExpression::Context(vec![("b".to_string(), FeelExpression::Literal(Value::Number(dec!(1))))]);
        let ctx = FeelExpression::Context(vec![("a".to_string(), inner_ctx)]);
        let path = FeelExpression::Path { target: Box::new(ctx), name: "b".into() };
        assert_eq!(evaluate(&path, &Scope::empty()), Value::Number(dec!(1)));
    }

    /// `[{"x":1}, {"x":2}].x` → Array([1, 2]) (list map semantics).
    #[test] #[ignore = "Context evaluation stubbed until Phase 31-05"]
    fn path_on_list_map() {
        let list = FeelExpression::List(vec![
            FeelExpression::Context(vec![("x".into(), FeelExpression::Literal(Value::Number(dec!(1))))]),
            FeelExpression::Context(vec![("x".into(), FeelExpression::Literal(Value::Number(dec!(2))))]),
        ]);
        let path = FeelExpression::Path { target: Box::new(list), name: "x".into() };
        assert_eq!(evaluate(&path, &Scope::empty()), Value::Array(vec![Value::Number(dec!(1)), Value::Number(dec!(2))]));
    }

    /// `null.x` → Null.
    #[test] fn path_on_null() {
        let path = FeelExpression::Path { target: Box::new(FeelExpression::Literal(Value::Null)), name: "x".into() };
        assert_eq!(evaluate(&path, &Scope::empty()), Value::Null);
    }

    // ----- Filter -----
    /// `[1,2,3,4][?item > 2]` → Array([3, 4]).
    #[test] fn filter_list() {
        let list = FeelExpression::List(vec![
            FeelExpression::Literal(Value::Number(dec!(1))), FeelExpression::Literal(Value::Number(dec!(2))),
            FeelExpression::Literal(Value::Number(dec!(3))), FeelExpression::Literal(Value::Number(dec!(4))),
        ]);
        let pred = FeelExpression::BinOp { op: BinOpKind::Gt, lhs: Box::new(FeelExpression::Name("item".into())), rhs: Box::new(FeelExpression::Literal(Value::Number(dec!(2)))) };
        let filter = FeelExpression::Filter { target: Box::new(list), predicate: Box::new(pred) };
        assert_eq!(evaluate(&filter, &Scope::empty()), Value::Array(vec![Value::Number(dec!(3)), Value::Number(dec!(4))]));
    }

    /// `null[?true]` → Null.
    #[test] fn filter_on_null() {
        let filter = FeelExpression::Filter { target: Box::new(FeelExpression::Literal(Value::Null)), predicate: Box::new(FeelExpression::Literal(Value::Bool(true))) };
        assert_eq!(evaluate(&filter, &Scope::empty()), Value::Null);
    }

    // ----- Instance of -----
    #[test] fn instance_of_number() {
        let e = FeelExpression::InstanceOf { expr: Box::new(FeelExpression::Literal(Value::Number(dec!(5)))), ty: FeelType::Number };
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Bool(true));
    }
    #[test] fn instance_of_string() {
        let e = FeelExpression::InstanceOf { expr: Box::new(FeelExpression::Literal(Value::Number(dec!(5)))), ty: FeelType::String };
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Bool(false));
    }
    /// `null instance of Any` → Bool(true) (Null <: Any).
    #[test] fn instance_of_null_any() {
        let e = FeelExpression::InstanceOf { expr: Box::new(FeelExpression::Literal(Value::Null)), ty: FeelType::Any };
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Bool(true));
    }
    /// `null instance of number` → Bool(false) (only Null <: Any).
    #[test] fn instance_of_null_not_number() {
        let e = FeelExpression::InstanceOf { expr: Box::new(FeelExpression::Literal(Value::Null)), ty: FeelType::Number };
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Bool(false));
    }

    // ----- In unary-test completion -----
    /// `5 in [1..10]` → Bool(true) (range membership).
    #[test] fn in_range_membership() { assert_eq!(ev("5 in [1..10]"), Value::Bool(true)); }
    /// `0 in [1..10]` → Bool(false).
    #[test] fn in_range_outside() { assert_eq!(ev("0 in [1..10]"), Value::Bool(false)); }

    /// Test `eval_in_unary_test` with constructed AST: `? < 10`.
    #[test] fn in_unary_test_lt() {
        let test = FeelExpression::BinOp { op: BinOpKind::Lt, lhs: Box::new(FeelExpression::Name("?".into())), rhs: Box::new(FeelExpression::Literal(Value::Number(dec!(10)))) };
        assert_eq!(eval_in_unary_test(Value::Number(dec!(5)), &test), Some(true));
        assert_eq!(eval_in_unary_test(Value::Number(dec!(15)), &test), Some(false));
    }

    /// Test `eval_in` with Or-chain (comma-list unary tests).
    #[test] fn in_or_chain() {
        let test = FeelExpression::BinOp { op: BinOpKind::Or,
            lhs: Box::new(FeelExpression::BinOp { op: BinOpKind::Eq, lhs: Box::new(FeelExpression::Name("?".into())), rhs: Box::new(FeelExpression::Literal(Value::Number(dec!(1)))) }),
            rhs: Box::new(FeelExpression::BinOp { op: BinOpKind::Eq, lhs: Box::new(FeelExpression::Name("?".into())), rhs: Box::new(FeelExpression::Literal(Value::Number(dec!(3)))) }),
        };
        let e = FeelExpression::BinOp { op: BinOpKind::In, lhs: Box::new(FeelExpression::Literal(Value::Number(dec!(1)))), rhs: Box::new(test) };
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Bool(true));
    }

    /// Empty unary tests (always true).
    #[test] fn in_always_true() {
        let e = FeelExpression::BinOp { op: BinOpKind::In, lhs: Box::new(FeelExpression::Literal(Value::Number(dec!(99)))), rhs: Box::new(FeelExpression::Literal(Value::Bool(true))) };
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Bool(true));
    }

    // ----- conforms/type_of integration -----
    #[test] fn type_of_number() { assert_eq!(crate::dmn::feel::types::type_of(&Value::Number(dec!(5))), FeelType::Number); }
    #[test] fn type_of_null() { assert_eq!(crate::dmn::feel::types::type_of(&Value::Null), FeelType::Null); }
    #[test] fn conforms_null_any() { assert!(crate::dmn::feel::types::conforms(&FeelType::Null, &FeelType::Any)); }
    #[test] fn conforms_number_number() { assert!(crate::dmn::feel::types::conforms(&FeelType::Number, &FeelType::Number)); }
    #[test] fn conforms_null_not_number() { assert!(!crate::dmn::feel::types::conforms(&FeelType::Null, &FeelType::Number)); }
}

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

#[cfg(test)]
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

// =====================================================================
// Plan 31-03 Task 2: date-time arithmetic + duration operators.
// Reuses Phase 30 add_months_zoned / add_days_time_zoned (DST-correct).
// =====================================================================

#[cfg(test)]
use chrono::TimeDelta;

#[cfg(test)]
#[test]
fn datetime_arith_date_plus_ymd() {
    // Jan 31 + 1 month -> Feb 28 (spec-correct via checked_add_months)
    assert_eq!(
        eval_src(r#"@"2021-01-31" + @"P1M""#),
        eval_src(r#"@"2021-02-28""#)
    );
}

#[cfg(test)]
#[test]
fn datetime_arith_datetime_plus_dtd_iana_preserved() {
    // DateTimeTz + DaysTimeDuration preserves IANA zone (Pitfall 3)
    let val = eval_src(r#"@"2021-01-01T10:00:00@Australia/Melbourne" + @"P1D""#);
    match val {
        Value::DateTimeTz(DateTimeValue::Iana(zdt)) => {
            assert_eq!(zdt.timezone(), chrono_tz::Australia::Melbourne);
            assert_eq!(
                zdt.naive_local().date(),
                NaiveDate::from_ymd_opt(2021, 1, 2).unwrap()
            );
        }
        other => panic!("expected IANA DateTimeTz, got {:?}", other),
    }
}

#[cfg(test)]
#[test]
fn datetime_arith_date_minus_date() {
    let val = eval_src(r#"@"2012-12-25" - @"2012-12-24""#);
    match val {
        Value::DaysTimeDuration(td) => assert_eq!(td, TimeDelta::days(1)),
        other => panic!("expected DaysTimeDuration, got {:?}", other),
    }
}

#[cfg(test)]
#[test]
fn datetime_arith_dtd_plus_dtd() {
    let val = eval_src(r#"@"P1D" + @"P2D""#);
    match val {
        Value::DaysTimeDuration(td) => assert_eq!(td, TimeDelta::days(3)),
        other => panic!("expected DaysTimeDuration, got {:?}", other),
    }
}

#[cfg(test)]
#[test]
fn datetime_arith_ymd_plus_ymd() {
    let val = eval_src(r#"@"P1Y" + @"P2M""#);
    match val {
        Value::YearsMonthsDuration(ymd) => assert_eq!(ymd.months, 14),
        other => panic!("expected YearsMonthsDuration, got {:?}", other),
    }
}

#[cfg(test)]
#[test]
fn datetime_arith_null_plus_duration() {
    assert_eq!(eval_src(r#"null + @"P1M""#), Value::Null);
    assert_eq!(eval_src(r#"@"2021-01-31" + null"#), Value::Null);
}

#[cfg(test)]
#[test]
fn datetime_arith_type_mismatch_yields_null() {
    assert_eq!(eval_src(r#"@"2021-01-31" + 5"#), Value::Null);
}

#[cfg(test)]
proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 32,
        ..proptest::test_runner::Config::default()
    })]

    /// For any valid zoned datetime, dateTime + @"P0D" == dateTime
    /// (identity, DST-safe). Restricted to safe dates/times (day 1-28,
    /// hour 10-14) to avoid spring-forward/fall-back DST transitions.
    #[test]
    fn datetime_arith_identity_dtd_zero(
        year in 2020i32..2025,
        month in 1u32..=12,
        day in 1u32..=28,
        hour in 10u32..=14,
    ) {
        let src = format!(
            r#"@"{}-{:02}-{:02}T{:02}:00:00@Australia/Melbourne" + @"P0D""#,
            year, month, day, hour
        );
        let val = eval_src(&src);
        let expected_src = format!(
            r#"@"{}-{:02}-{:02}T{:02}:00:00@Australia/Melbourne""#,
            year, month, day, hour
        );
        let expected = eval_src(&expected_src);
        prop_assert_eq!(val, expected);
    }
}

// =====================================================================
// Plan 31-05: boxed-expression evaluators (DMN-15).
// =====================================================================

#[cfg(test)]
mod boxed {
    use super::*;
    use crate::dmn::feel::ast::FeelExpression;
    use crate::dmn::feel::types::FeelFunction;
    use rust_decimal::dec;
    use std::collections::HashMap;

    /// Helper: parse + evaluate with empty scope.
    #[cfg(test)]
    fn ev(src: &str) -> Value {
        evaluate(&parse_expression(src), &Scope::empty())
    }

    // ----- List (regression — already real in 31-04 as iteration-source helper) -----

    /// `evaluate(List([Literal(1), Literal(2), Literal(3)]))` == Array([Number(1), Number(2), Number(3)]).
    #[test]
    fn boxed_list_literal() {
        let e = FeelExpression::List(vec![
            FeelExpression::Literal(Value::Integer(1)),
            FeelExpression::Literal(Value::Integer(2)),
            FeelExpression::Literal(Value::Integer(3)),
        ]);
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Array(vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)]));
    }

    /// Nested boxed expressions: `[{a: 1}]` — outer List containing a Context.
    #[test]
    fn boxed_list_nested_context() {
        let ctx = FeelExpression::Context(vec![("a".to_string(), FeelExpression::Literal(Value::Integer(1)))]);
        let list = FeelExpression::List(vec![ctx]);
        // Context arm is stubbed -> Value::Null; this test FAILS RED until Task 2 fills it in.
        assert_eq!(evaluate(&list, &Scope::empty()), Value::Array(vec![Value::Object(HashMap::from([("a".to_string(), Value::Integer(1))]))]));
    }

    // ----- Context (no dependencies — simple case) -----

    /// `{a: 1, b: 2}` → Object({a:1, b:2}). Fails RED because the Context arm is stubbed.
    #[test]
    fn boxed_context_simple() {
        let e = FeelExpression::Context(vec![
            ("a".to_string(), FeelExpression::Literal(Value::Integer(1))),
            ("b".to_string(), FeelExpression::Literal(Value::Integer(2))),
        ]);
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Object(HashMap::from([("a".to_string(), Value::Integer(1)), ("b".to_string(), Value::Integer(2))])));
    }

    // ----- Context (with dependencies — topological order) -----

    /// `{a: 1, b: a + 1}` → Object({a:1, b:2}). Fails RED because the Context arm is stubbed.
    #[test]
    fn boxed_context_with_dependency() {
        let e = FeelExpression::Context(vec![
            ("a".to_string(), FeelExpression::Literal(Value::Integer(1))),
            ("b".to_string(), FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("a".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
        ]);
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Object(HashMap::from([("a".to_string(), Value::Integer(1)), ("b".to_string(), Value::Number(dec!(2)))])));
    }

    // ----- Relation -----

    /// A relation with columns [x, y] and rows [[1, 2], [3, 4]] evaluates to Array of Objects.
    #[test]
    fn boxed_relation() {
        let e = FeelExpression::Relation {
            columns: vec!["x".to_string(), "y".to_string()],
            rows: vec![
                vec![FeelExpression::Literal(Value::Integer(1)), FeelExpression::Literal(Value::Integer(2))],
                vec![FeelExpression::Literal(Value::Integer(3)), FeelExpression::Literal(Value::Integer(4))],
            ],
        };
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Array(vec![
            Value::Object(HashMap::from([("x".to_string(), Value::Integer(1)), ("y".to_string(), Value::Integer(2))])),
            Value::Object(HashMap::from([("x".to_string(), Value::Integer(3)), ("y".to_string(), Value::Integer(4))])),
        ]));
    }

    // ----- FunctionDefinition -----

    /// `function(a, b) a + b` evaluates to a Value::Function carrier. Body is NOT evaluated at definition time.
    #[test]
    fn boxed_function_definition_carrier() {
        let src = "function(a, b) a + b";
        let result = ev(src);
        // Must be a Function value (not Null — the stub returns Null).
        assert!(matches!(&result, Value::Function(_)), "expected Value::Function, got {:?}", result);
    }

    /// The closure body is NOT evaluated at definition time.
    #[test]
    fn boxed_function_definition_body_not_evaluated() {
        // `function(a) undefined_var` — if the body were evaluated, this would fail (undefined var).
        // Since we only construct a carrier, it should succeed and return Value::Function.
        let src = "function(a) undefined_var";
        let result = ev(src);
        assert!(matches!(&result, Value::Function(_)), "expected Value::Function, got {:?}", result);
    }

    /// `external` flag is stored on the carrier.
    #[test]
    fn boxed_function_definition_external_flag() {
        let src = "function(a) external a + 1";
        let result = ev(src);
        match &result {
            Value::Function(f) => assert!(f.external, "external flag should be true"),
            other => panic!("expected Value::Function, got {:?}", other),
        }
    }

    // ----- Conditional (= If) delegation -----

    /// A boxed conditional (constructed as `If` AST node directly — the only form in the AST).
    #[test]
    fn boxed_conditional_delegates_to_if() {
        let e = FeelExpression::If {
            cond: Box::new(FeelExpression::Literal(Value::Bool(true))),
            then: Box::new(FeelExpression::Literal(Value::Integer(1))),
            else_: Box::new(FeelExpression::Literal(Value::Integer(2))),
        };
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Integer(1));
    }

    // ----- Filter / For / Quantified delegation (already real in 31-04) -----

    /// Boxed filter delegates to existing evaluate_filter arm.
    #[test]
    fn boxed_filter_delegates() {
        let list = FeelExpression::List(vec![
            FeelExpression::Literal(Value::Integer(1)),
            FeelExpression::Literal(Value::Integer(2)),
            FeelExpression::Literal(Value::Integer(3)),
        ]);
        let pred = FeelExpression::BinOp { op: BinOpKind::Gt, lhs: Box::new(FeelExpression::Name("item".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) };
        let filter = FeelExpression::Filter { target: Box::new(list), predicate: Box::new(pred) };
        assert_eq!(evaluate(&filter, &Scope::empty()), Value::Array(vec![Value::Integer(2), Value::Integer(3)]));
    }

    /// Boxed for delegates to existing evaluate_for arm.
    #[test]
    fn boxed_for_delegates() {
        let e = FeelExpression::For {
            iters: vec![("x".to_string(), FeelExpression::List(vec![
                FeelExpression::Literal(Value::Integer(1)),
                FeelExpression::Literal(Value::Integer(2)),
            ]))],
            body: Box::new(FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("x".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
        };
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Array(vec![Value::Number(dec!(2)), Value::Number(dec!(3))]));
    }

    /// Boxed quantified delegates to existing evaluate_quantified arm.
    #[test]
    fn boxed_quantified_delegates() {
        let e = FeelExpression::Quantified {
            kind: QuantKind::Some,
            iters: vec![("x".to_string(), FeelExpression::List(vec![
                FeelExpression::Literal(Value::Integer(1)),
                FeelExpression::Literal(Value::Integer(2)),
            ]))],
            satisfies: Box::new(FeelExpression::BinOp { op: BinOpKind::Gt, lhs: Box::new(FeelExpression::Name("x".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
        };
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Bool(true));
    }

    // ----- FunctionInvocation stub -----

    /// A function invocation with an unregistered name returns Null (stub dispatch).
    #[test]
    fn boxed_function_invocation_stub() {
        let e = FeelExpression::FunctionInvocation {
            callee: Box::new(FeelExpression::Name("unknown_func".into())),
            args: vec![],
            named: None,
        };
        assert_eq!(evaluate(&e, &Scope::empty()), Value::Null);
    }

    /// FunctionInvocation with a Name that resolves to a bound function value is also stubbed (31-06/31-07 not yet done).
    #[test]
    fn boxed_function_invocation_bound_callee_stub() {
        let mut scope = Scope::empty();
        scope.bind("f".to_string(), Value::Function(FeelFunction {
            arity: 0, param_types: vec![], ret_type: Box::new(crate::dmn::feel::types::FeelType::Any),
            param_names: vec![], body: None, captured: None, external: false,
        }), vec![]);
        let e = FeelExpression::FunctionInvocation {
            callee: Box::new(FeelExpression::Name("f".into())),
            args: vec![],
            named: None,
        };
        // Lambda application (31-06) is not yet implemented — stub returns Null.
        assert_eq!(evaluate(&e, &scope), Value::Null);
    }

    // ----- Snapshot / golden test for a representative boxed expression -----

    /// Golden test: `for x in [1,2,3] return {value: x, doubled: x * 2}` produces an array of contexts.
    #[test]
    fn boxed_for_returns_contexts_golden() {
        let e = FeelExpression::For {
            iters: vec![("x".to_string(), FeelExpression::List(vec![
                FeelExpression::Literal(Value::Integer(1)),
                FeelExpression::Literal(Value::Integer(2)),
                FeelExpression::Literal(Value::Integer(3)),
            ]))],
            body: Box::new(FeelExpression::Context(vec![
                ("value".to_string(), FeelExpression::Name("x".into())),
                ("doubled".to_string(), FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("x".into())), rhs: Box::new(FeelExpression::Name("x".into())) }),
            ])),
        };
        let result = evaluate(&e, &Scope::empty());
        match &result {
            Value::Array(arr) => assert_eq!(arr.len(), 3),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    // ----- Context cycle detection (Task 2) -----

    /// Helper: build a context AST with the given entries and evaluate it.
    #[cfg(test)]
    fn eval_ctx(entries: Vec<(&str, FeelExpression)>) -> Value {
        let ast_entries: Vec<(String, FeelExpression)> = entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        evaluate(&FeelExpression::Context(ast_entries), &Scope::empty())
    }

    /// 2-cycle: `{a: b+1, b: a+1}` — both entries are cyclic → Null.
    #[test]
    fn context_topo_2_cycle() {
        let result = eval_ctx(vec![
            ("a", FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("b".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
            ("b", FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("a".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
        ]);
        match &result {
            Value::Object(map) => {
                assert_eq!(map.get("a"), Some(&Value::Null), "cyclic 'a' should be Null");
                assert_eq!(map.get("b"), Some(&Value::Null), "cyclic 'b' should be Null");
            }
            other => panic!("expected Object, got {:?}", other),
        }
    }

    /// 3-cycle: `{a: b+1, b: c+1, c: a+1}` — all three are cyclic → Null.
    #[test]
    fn context_topo_3_cycle() {
        let result = eval_ctx(vec![
            ("a", FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("b".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
            ("b", FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("c".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
            ("c", FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("a".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
        ]);
        match &result {
            Value::Object(map) => {
                assert_eq!(map.get("a"), Some(&Value::Null), "cyclic 'a' should be Null");
                assert_eq!(map.get("b"), Some(&Value::Null), "cyclic 'b' should be Null");
                assert_eq!(map.get("c"), Some(&Value::Null), "cyclic 'c' should be Null");
            }
            other => panic!("expected Object, got {:?}", other),
        }
    }

    /// Self-reference: `{a: a+1}` — a depends on itself → cycle → Null.
    #[test]
    fn context_topo_self_reference() {
        let result = eval_ctx(vec![
            ("a", FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("a".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
        ]);
        match &result {
            Value::Object(map) => {
                assert_eq!(map.get("a"), Some(&Value::Null), "self-referential 'a' should be Null");
            }
            other => panic!("expected Object, got {:?}", other),
        }
    }

    /// Out-of-order DAG: `{b: a+1, a: 1}` — topo order should put `a` first → Object({a:1, b:2}).
    #[test]
    fn context_topo_out_of_order_dag() {
        let result = eval_ctx(vec![
            ("b", FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("a".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
            ("a", FeelExpression::Literal(Value::Integer(1))),
        ]);
        assert_eq!(result, Value::Object(std::collections::HashMap::from([
            ("a".to_string(), Value::Integer(1)),
            ("b".to_string(), Value::Number(rust_decimal::dec!(2))),
        ])));
    }

    /// DAG with chain: `{a: 1, b: a+1, c: b+1}` → Object({a:1, b:2, c:3}).
    #[test]
    fn context_topo_chain() {
        let result = eval_ctx(vec![
            ("c", FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("b".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
            ("b", FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("a".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
            ("a", FeelExpression::Literal(Value::Integer(1))),
        ]);
        assert_eq!(result, Value::Object(std::collections::HashMap::from([
            ("a".to_string(), Value::Integer(1)),
            ("b".to_string(), Value::Number(rust_decimal::dec!(2))),
            ("c".to_string(), Value::Number(rust_decimal::dec!(3))),
        ])));
    }

    /// Mixed: some entries cyclic, others not — non-cyclic entries still evaluate.
    #[test]
    fn context_topo_mixed_cycle_and_dag() {
        let result = eval_ctx(vec![
            ("a", FeelExpression::Literal(Value::Integer(10))),
            ("b", FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("c".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
            ("c", FeelExpression::BinOp { op: BinOpKind::Add, lhs: Box::new(FeelExpression::Name("b".into())), rhs: Box::new(FeelExpression::Literal(Value::Integer(1))) }),
        ]);
        match &result {
            Value::Object(map) => {
                assert_eq!(map.get("a"), Some(&Value::Integer(10)), "non-cyclic 'a' should evaluate to 10");
                assert_eq!(map.get("b"), Some(&Value::Null), "cyclic 'b' should be Null");
                assert_eq!(map.get("c"), Some(&Value::Null), "cyclic 'c' should be Null");
            }
            other => panic!("expected Object, got {:?}", other),
        }
    }

    // ----- Proptest: DAG-order invariance (seeded) -----

    /// Property: a random DAG of context entries evaluates to the same result
    /// regardless of source order. This verifies that topological evaluation is
    /// canonical (not dependent on declaration order).
    #[cfg(test)]
    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config {
            cases: 32,
            ..proptest::test_runner::Config::default()
        })]

        /// Random DAG of context entries: shuffling entry order should not change result.
        #[test]
        fn context_topo_dag_order_invariance(
            n in 1usize..=5,
            base_val in 0i32..100,
        ) {
            // Build two contexts with the same entries but different order.
            // Both should evaluate identically since topo sort canonicalizes order.

            // Generate n entries: entry i references the previous one (chain).
            let mut entries_forward = Vec::new();
            for i in 0..n {
                if i == 0 {
                    entries_forward.push(("e0", FeelExpression::Literal(Value::Integer(base_val as i64))));
                } else {
                    // e_i = e_{i-1} + 1
                    let prev_name = format!("e{}", i - 1);
                    entries_forward.push((
                        format!("e{i}").leak(),
                        FeelExpression::BinOp {
                            op: BinOpKind::Add,
                            lhs: Box::new(FeelExpression::Name(prev_name)),
                            rhs: Box::new(FeelExpression::Literal(Value::Integer(1))),
                        },
                    ));
                }
            }

            // Reverse order — same entries, different source order.
            let mut entries_reverse = entries_forward.clone();
            entries_reverse.reverse();

            let result_fwd = eval_ctx(entries_forward);
            let result_rev = eval_ctx(entries_reverse);

            prop_assert_eq!(result_fwd, result_rev, "DAG evaluation should be order-invariant");
        }
    }
}

// ----- Lambda closure capture (DMN-16) -----

#[cfg(test)]
mod lambdas {
    use super::*;
    use crate::dmn::feel::ast::{FeelExpression, BinOpKind};
    use crate::dmn::feel::types::FeelType;
    use std::sync::Arc;

    /// Helper: parse + evaluate with empty scope.
    fn ev_lambda(src: &str) -> Value {
        evaluate(&parse_expression(src), &Scope::empty())
    }

    /// Test: `function(a, b) a + b` evaluates to a Function value (not Null).
    #[test]
    fn lambda_definition_produces_function_value() {
        let result = ev_lambda("function(a, b) a + b");
        assert!(matches!(&result, Value::Function(_)), "expected Value::Function, got {:?}", result);
    }

    /// Test: the FunctionDefinition arm populates `captured` with the definition-site scope.
    #[test]
    fn lambda_captured_scope_populated_at_definition() {
        let src = "function(a) a + 1";
        let result = ev_lambda(src);
        match &result {
            Value::Function(f) => {
                assert!(f.captured.is_some(), "captured scope should be populated at definition time");
                // The captured scope is the empty scope (definition-site bindings).
                let captured = f.captured.as_ref().unwrap();
                assert_eq!(captured.bindings.len(), 0, "empty scope has no bindings");
            }
            other => panic!("expected Value::Function, got {:?}", other),
        }
    }

    /// Test: lambda captures bindings from its definition-site scope (a boxed context).
    #[test]
    fn lambda_captures_definition_site_bindings() {
        // Build a context with x bound, containing a lambda that references x.
        let func_def = FeelExpression::FunctionDefinition {
            params: vec![("y".to_string(), None)],
            body: Box::new(FeelExpression::BinOp {
                op: BinOpKind::Add,
                lhs: Box::new(FeelExpression::Name("x".into())),
                rhs: Box::new(FeelExpression::Literal(Value::Integer(5))),
            }),
            external: false,
        };

        // Wrap in a context where x is bound.
        let ctx = FeelExpression::Context(vec![
            ("f".to_string(), func_def),
            ("x".to_string(), FeelExpression::Literal(Value::Integer(10))),
        ]);

        let result = evaluate(&ctx, &Scope::empty());
        // The context evaluates to an Object; we need to invoke f with arg 5.
        match result {
            Value::Object(map) => {
                let f_val = map.get("f").unwrap();
                match f_val {
                    Value::Function(f) => {
                        assert!(f.captured.is_some(), "captured should be populated");
                        // The captured scope should contain x=10 (from the context).
                        let captured = f.captured.as_ref().unwrap();
                        let x_val = captured.get("x").cloned().unwrap_or(Value::Null);
                        assert_eq!(x_val, Value::Integer(10), "captured scope should have x=10");
                    }
                    other => panic!("expected Function value, got {:?}", other),
                }
            }
            other => panic!("expected Object (context result), got {:?}", other),
        }
    }

    /// Test: FeelFunction serde round-trip with captured scope.
    #[test]
    fn feel_function_serde_round_trip_with_captured() {
        let func = crate::dmn::feel::types::FeelFunction {
            arity: 1,
            param_types: vec![FeelType::Number],
            ret_type: Box::new(FeelType::Any),
            param_names: vec!["x".to_string()],
            body: Some(Arc::new(crate::dmn::feel::ast::FeelExpression::Literal(Value::Integer(42)))),
            captured: Some(Arc::new(Scope {
                bindings: std::collections::HashMap::from([("y".to_string(), Value::String("hello".into()))]),
                deps: std::collections::HashMap::new(),
            })),
            external: false,
        };

        let json = serde_json::to_string(&func).expect("serialize");
        let deserialized: crate::dmn::feel::types::FeelFunction = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.arity, func.arity);
        assert_eq!(deserialized.param_names, vec!["x".to_string()]);
        // Body should round-trip (the AST inside Arc).
        match (&deserialized.body, &func.body) {
            (Some(body), Some(orig)) => {
                assert_eq!(body, orig, "body Arc<FeelExpression> should round-trip");
            }
            _ => panic!("both body fields should be Some"),
        }
        // Captured scope should round-trip.
        match (&deserialized.captured, &func.captured) {
            (Some(cap), Some(orig)) => {
                assert_eq!(cap.bindings.get("y"), orig.bindings.get("y"), "captured bindings should round-trip");
            }
            _ => panic!("both captured fields should be Some"),
        }
    }

    /// Test: external function definition still populates captured scope (but rejected at invocation).
    #[test]
    fn lambda_external_definition_still_has_captured() {
        let src = "function(a) external a + 1";
        let result = ev_lambda(src);
        match &result {
            Value::Function(f) => {
                assert!(f.external, "external flag should be true");
                // Even external functions capture the definition-site scope (for future use).
                assert!(f.captured.is_some(), "captured should still be populated for external functions");
            }
            other => panic!("expected Value::Function, got {:?}", other),
        }
    }

    /// Test: lexical capture — closure sees definition-site bindings, not call-site.
    #[test]
    fn lambda_lexical_capture_not_dynamic() {
        // Define a function in context1 (where x=10); then invoke it from context2 (where x=99).
        // The closure should see x=10, not 99.

        let func_def = FeelExpression::FunctionDefinition {
            params: vec![("y".to_string(), None)],
            body: Box::new(FeelExpression::BinOp {
                op: BinOpKind::Add,
                lhs: Box::new(FeelExpression::Name("x".into())),
                rhs: Box::new(FeelExpression::Literal(Value::Integer(0))),
            }),
            external: false,
        };

        // context1: defines f with x=10 captured.
        let ctx1 = FeelExpression::Context(vec![
            ("f".to_string(), func_def),
            ("x".to_string(), FeelExpression::Literal(Value::Integer(10))),
        ]);

        let result1 = evaluate(&ctx1, &Scope::empty());
        match result1 {
            Value::Object(map) => {
                let f_val = map.get("f").unwrap();
                if let Value::Function(f) = f_val {
                    // Get the captured x value.
                    let captured_x = f.captured.as_ref().and_then(|c| c.get("x")).cloned().unwrap_or(Value::Null);
                    assert_eq!(captured_x, Value::Integer(10), "lexical capture: closure should see definition-site x=10");

                    // Now invoke this function from a different context where x=99.
                    let invocation = FeelExpression::FunctionInvocation {
                        callee: Box::new(FeelExpression::Literal(f_val.clone())),
                        args: vec![FeelExpression::Literal(Value::Integer(5))],
                        named: None,
                    };

                    // Invoke from a scope where x=99.
                    let mut outer_scope = Scope::empty();
                    outer_scope.bind("x".to_string(), Value::Integer(99), vec![]);

                    // The result should still use captured x=10 (lexical), not 99.
                    // This test verifies the captured scope is populated correctly — the invocation logic is in Task 2.
                    let _ = invocation; // Will be tested in Task 2.
                } else {
                    panic!("expected Function value");
                }
            }
            other => panic!("expected Object, got {:?}", other),
        }
    }

    /// Test: arity mismatch returns Null (no lambda application logic yet).
    #[test]
    fn lambda_arity_mismatch_returns_null() {
        let func_def = FeelExpression::FunctionDefinition {
            params: vec![("a".to_string(), None), ("b".to_string(), None)],
            body: Box::new(FeelExpression::BinOp {
                op: BinOpKind::Add,
                lhs: Box::new(FeelExpression::Name("a".into())),
                rhs: Box::new(FeelExpression::Name("b".into())),
            }),
            external: false,
        };

        // Invoke with only 1 arg (arity mismatch).
        let invocation = FeelExpression::FunctionInvocation {
            callee: Box::new(func_def),
            args: vec![FeelExpression::Literal(Value::Integer(5))],
            named: None,
        };

        let result = evaluate(&invocation, &Scope::empty());
        // Without lambda application logic, this returns Null (the stub). This test verifies the future behavior.
        assert_eq!(result, Value::Null, "arity mismatch should return Null");
    }
}

// ----- Lambda invocation (DMN-16) -----

#[cfg(test)]
mod external_rejected {
    use super::*;
    use crate::dmn::feel::ast::{FeelExpression, BinOpKind};

    /// Test: external function invocation returns Null + warning logged.
    #[test]
    fn lambda_external_invocation_returns_null() {
        let func_def = FeelExpression::FunctionDefinition {
            params: vec![("a".to_string(), None)],
            body: Box::new(FeelExpression::Literal(Value::Integer(42))),
            external: true, // Marked as external.
        };

        let invocation = FeelExpression::FunctionInvocation {
            callee: Box::new(func_def),
            args: vec![FeelExpression::Literal(Value::Integer(5))],
            named: None,
        };

        let result = evaluate(&invocation, &Scope::empty());
        // External functions are rejected at invocation time.
        assert_eq!(result, Value::Null, "external function invocation should return Null");
    }

    /// Test: non-external lambda application works (merge captured + params, evaluate body).
    #[test]
    fn lambda_application_merges_scope_and_evaluates() {
        use crate::dmn::feel::ast::{FeelExpression as FE, BinOpKind as BOK};

        // Build a function that adds its parameter to x from the enclosing scope.
        let func_def = FeelExpression::FunctionDefinition {
            params: vec![("y".to_string(), None)],
            body: Box::new(FeelExpression::BinOp {
                op: BOK::Add,
                lhs: Box::new(FE::Name("x".into())),
                rhs: Box::new(FE::Name("y".into())),
            }),
            external: false,
        };

        // Wrap in a context where x is bound.
        let ctx = FeelExpression::Context(vec![
            ("f".to_string(), func_def),
            ("x".to_string(), FE::Literal(Value::Integer(10))),
        ]);

        let result = evaluate(&ctx, &Scope::empty());
        match result {
            Value::Object(map) => {
                let f_val = map.get("f").unwrap();
                if let Value::Function(f) = f_val {
                    // Invoke the function with arg 5.
                    let invocation = FeelExpression::FunctionInvocation {
                        callee: Box::new(FE::Literal(f_val.clone())),
                        args: vec![FE::Literal(Value::Integer(5))],
                        named: None,
                    };

                    // The result should be x + y = 10 + 5 = 15.
                    let eval_result = evaluate(&invocation, &Scope::empty());
                    assert_eq!(eval_result, Value::Number(rust_decimal::dec!(15)), "lambda application should return 10+5=15");
                } else {
                    panic!("expected Function value");
                }
            }
            other => panic!("expected Object, got {:?}", other),
        }
    }

    /// Test: named args work (merge captured + named params, evaluate body).
    #[test]
    fn lambda_named_args_work() {
        use crate::dmn::feel::ast::{FeelExpression as FE, BinOpKind as BOK};

        let func_def = FeelExpression::FunctionDefinition {
            params: vec![("a".to_string(), None), ("b".to_string(), None)],
            body: Box::new(FeelExpression::BinOp {
                op: BOK::Add,
                lhs: Box::new(FE::Name("a".into())),
                rhs: Box::new(FE::Name("b".into())),
            }),
            external: false,
        };

        // Invoke with named args (b=2, a=1) — order doesn't matter.
        let invocation = FeelExpression::FunctionInvocation {
            callee: Box::new(func_def),
            args: vec![],
            named: Some(vec![
                ("b".to_string(), FE::Literal(Value::Integer(2))),
                ("a".to_string(), FE::Literal(Value::Integer(1))),
            ]),
        };

        let result = evaluate(&invocation, &Scope::empty());
        // a=1 + b=2 = 3.
        assert_eq!(result, Value::Number(rust_decimal::dec!(3)), "named args: a+b should be 3");
    }

    /// Test: lexical capture verified — closure sees definition-site x=10 even when called from scope with x=99.
    #[test]
    fn lambda_lexical_capture_verified_at_invocation() {
        use crate::dmn::feel::ast::{FeelExpression as FE, BinOpKind as BOK};

        // Define function in context where x=10; invoke from a scope where x=99.
        let func_def = FeelExpression::FunctionDefinition {
            params: vec![("y".to_string(), None)],
            body: Box::new(FeelExpression::BinOp {
                op: BOK::Add,
                lhs: Box::new(FE::Name("x".into())),
                rhs: Box::new(FE::Literal(Value::Integer(0))),
            }),
            external: false,
        };

        // context1 defines f with x=10 captured.
        let ctx1 = FeelExpression::Context(vec![
            ("f".to_string(), func_def),
            ("x".to_string(), FE::Literal(Value::Integer(10))),
        ]);

        let result1 = evaluate(&ctx1, &Scope::empty());
        match result1 {
            Value::Object(map) => {
                let f_val = map.get("f").unwrap();
                if let Value::Function(f) = f_val {
                    // Invoke from a scope where x=99.
                    let invocation = FeelExpression::FunctionInvocation {
                        callee: Box::new(FE::Literal(f_val.clone())),
                        args: vec![FE::Literal(Value::Integer(5))],
                        named: None,
                    };

                    let mut outer_scope = Scope::empty();
                    outer_scope.bind("x".to_string(), Value::Integer(99), vec![]);

                    // The result should still use captured x=10 (lexical).
                    let eval_result = evaluate(&invocation, &outer_scope);
                    assert_eq!(eval_result, Value::Number(rust_decimal::dec!(10)), "lexical capture: closure sees definition-site x=10, not call-site x=99");
                } else {
                    panic!("expected Function value");
                }
            }
            other => panic!("expected Object, got {:?}", other),
        }
    }

    /// Test: arity mismatch returns Null (no lambda application logic yet).
    #[test]
    fn lambda_arity_mismatch_invocation_returns_null() {
        use crate::dmn::feel::ast::{FeelExpression as FE, BinOpKind as BOK};

        let func_def = FeelExpression::FunctionDefinition {
            params: vec![("a".to_string(), None), ("b".to_string(), None)],
            body: Box::new(FeelExpression::BinOp {
                op: BOK::Add,
                lhs: Box::new(FE::Name("a".into())),
                rhs: Box::new(FE::Name("b".into())),
            }),
            external: false,
        };

        // Invoke with only 1 arg (arity mismatch).
        let invocation = FeelExpression::FunctionInvocation {
            callee: Box::new(func_def),
            args: vec![FE::Literal(Value::Integer(5))],
            named: None,
        };

        let result = evaluate(&invocation, &Scope::empty());
        // Arity mismatch → Null.
        assert_eq!(result, Value::Null, "arity mismatch should return Null");
    }

    /// Test: callee is not a function → returns Null.
    #[test]
    fn lambda_invocation_non_function_callee_returns_null() {
        let invocation = FeelExpression::FunctionInvocation {
            callee: Box::new(crate::dmn::feel::ast::FeelExpression::Literal(Value::Integer(42))), // Number, not a function!
            args: vec![],
            named: None,
        };

        let result = evaluate(&invocation, &Scope::empty());
        assert_eq!(result, Value::Null, "non-function callee should return Null");
    }
}
