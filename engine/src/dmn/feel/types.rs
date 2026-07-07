//! FEEL type lattice nodes, carrier structs, and lattice relations.
//!
//! Plan 30-01 landed the `FeelType` enum, `FeelRange`, and `FeelFunction`
//! placeholder carriers. Plan 30-04 (this file) fills in the lattice relations:
//!
//! - [`equivalent`] — DMN 1.5 §10.3.2.9.1 (reflexive, symmetric, transitive).
//! - [`conforms`] — DMN 1.5 §10.3.2.9.2 (subtype relation; `Null <: T <: Any`).
//! - [`ConvertCtx`] / [`implicit_convert`] — DMN 1.5 §10.3.2.9.4, EXACTLY the
//!   three spec-defined implicit conversions (to-singleton-list,
//!   from-singleton-list, conforms-to). There is **no** `number ↔ string`
//!   conversion here — that is the Phase 31 explicit `string()` / `number()`
//!   built-in (30-RESEARCH.md Pitfall 6).
//! - [`type_of`] — map a [`Value`] to its [`FeelType`] lattice node
//!   (exhaustive over all 16 `Value` variants — no wildcard).
//! - [`resolve_type_ref`] — map IR `feel:` type_ref strings to [`FeelType`]
//!   nodes (consumed by Phase 31 IR type resolution).
//!
//! The lattice is modelled as a closed enum + free functions (per
//! 30-RESEARCH.md Pattern 2), not a trait graph: FEEL's type space is small
//! and closed, so a table/function is clearer and property-testable.

use crate::data_types::Value;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::ast::FeelExpression;
use super::eval::Scope;

/// The FEEL type lattice nodes (DMN 1.5 §10.3.2.9).
///
/// `Null` and `Any` are the lattice bounds: `Null <: T <: Any` for every `T`.
/// `List`/`Context`/`Range` are recursive (carry inner `FeelType`s); `Function`
/// carries parameter and return types. The closure *body* of a FEEL function
/// value is a Phase 31 concern — here we carry only the type shape, enough for
/// `conforms`/`equivalent` (research Open Question 3 resolution).
///
/// `Serialize`/`Deserialize` are derived because [`FeelFunction`] (which holds
/// `Vec<FeelType>` / `Box<FeelType>`) derives serde and the derive propagates
/// the bound to the inner type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeelType {
    /// The null type — bottom of the lattice. `Null <: T` for every `T`.
    Null,
    /// The top of the lattice. `T <: Any` for every `T`.
    Any,
    /// FEEL `number` (Decimal128). Backed by `rust_decimal::Decimal`.
    Number,
    /// FEEL `string`.
    String,
    /// FEEL `boolean`.
    Boolean,
    /// FEEL `date`.
    Date,
    /// FEEL `time`.
    Time,
    /// FEEL `date and time`.
    DateTime,
    /// FEEL `days and time duration` (ISO 8601 `PnYnMnDTnHnMnS` day-time part).
    DaysTimeDuration,
    /// FEEL `years and months duration` (ISO 8601 `PnYnM` year-month part).
    YearsMonthsDuration,
    /// FEEL `list<T>`.
    List(Box<FeelType>),
    /// FEEL `context<k1:T1..kn:Tn>` — an ordered list of `(key, type)` pairs.
    Context(Vec<(String, FeelType)>),
    /// FEEL `range<T>`.
    Range(Box<FeelType>),
    /// FEEL `function (T1..Tn) -> U`. Contravariant params, covariant return.
    Function {
        /// Parameter types, positional.
        params: Vec<FeelType>,
        /// Return type.
        ret: Box<FeelType>,
    },
}

/// FEEL range value carrier — the payload of [`Value::Range`].
///
/// `start`/`end` are `Option<Value>` so an open endpoint (`None`) means
/// "unbounded" on that side (FEEL `..]` / `[..`). `start_incl`/`end_incl`
/// carry the bracket sense: `true` = inclusive (`[`/`]`), `false` = exclusive
/// `(`/`)`.
///
/// `Value::Range(Box<FeelRange>)` boxes the range because `FeelRange` holds
/// `Option<Value>` and `Value` holds `Box<FeelRange>` — the `Box` breaks the
/// infinite size cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeelRange {
    /// Start endpoint, or `None` if unbounded below.
    pub start: Option<Value>,
    /// End endpoint, or `None` if unbounded above.
    pub end: Option<Value>,
    /// Whether the start endpoint is inclusive.
    pub start_incl: bool,
    /// Whether the end endpoint is inclusive.
    pub end_incl: bool,
}

/// FEEL function value carrier — the payload of [`Value::Function`].
///
/// Plan 30-01 shipped a *placeholder* carrying only arity + parameter/return
/// `FeelType`s — enough for the lattice `conforms`/`equivalent` relations.
/// Phase 31-01 (this file) extends the carrier with the closure body and
/// captured-environment fields as a **data-model decision** (CONTEXT.md
/// FeelFunction carrier extension guidance; 31-RESEARCH.md §DMN-16): the
/// fields are DECLARED here with safe defaults so that existing Phase 30
/// construction sites still compile. 31-05 POPULATES `body` (the closure
/// body AST); 31-06 POPULATES `captured` (the definition-site scope
/// snapshot) and adds the lambda invocation logic. Neither 31-05 nor 31-06
/// re-declares these fields.
///
/// The `body`/`captured` fields use `Arc<…>` so that cloning a closure is
/// cheap (the AST and the scope snapshot are shared, not deep-copied). This
/// mirrors the AST cache (`Arc<FeelExpression>`, DMN-18) and the
/// `FeelRange` carrier's `Box<FeelRange>` cycle-break pattern (the
/// `FeelFunction` ⇄ `FeelExpression` ⇄ `FeelFunction` cycle is broken by
/// `Arc` + `Box`: `FeelExpression::FunctionInvocation` holds
/// `Box<FeelExpression>` for the callee, and `FeelFunction.body` holds
/// `Option<Arc<FeelExpression>>` for the closure body).
///
/// `Serialize`/`Deserialize` are derived (Phase 30 D-30-01c serde bound
/// propagates): `Arc<T>` is serde-serializable when `T: Serialize`, which
/// holds for both `FeelExpression` (this Phase 31-01) and the `Scope` stub
/// (31-02 retains the derives).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeelFunction {
    /// Number of parameters the function accepts.
    pub arity: usize,
    /// Parameter types, positional.
    pub param_types: Vec<FeelType>,
    /// Return type.
    pub ret_type: Box<FeelType>,
    // --- Phase 31 additions (declared here as a data-model decision;
    // populated by 31-05/31-06; safe defaults so Phase 30 construction
    // sites still compile) ---
    /// Parameter names (for positional→named binding in 31-06). Length ==
    /// `arity`. Defaults to `Vec::new()` for built-in function pointers
    /// (which have no parameter names) and for legacy Phase 30 construction
    /// sites that do not yet populate the carrier.
    pub param_names: Vec<String>,
    /// The closure body AST. `None` for built-ins (which are `BuiltinFn`
    /// pointers resolved via the `builtins` registry in 31-07/31-08);
    /// `Some(Arc<FeelExpression>)` for user-defined lambdas — populated in
    /// 31-05 when a `FunctionDefinition` evaluates to a `FeelFunction`
    /// value. The `Arc` shares the AST with the cache (31-09).
    pub body: Option<Arc<FeelExpression>>,
    /// Captured lexical scope (flat `Scope` snapshot at function-definition
    /// time). `None` for built-ins; `Some(Arc<Scope>)` for user-defined
    /// lambdas — populated in 31-06 (lexical capture, NOT dynamic capture:
    /// the closure sees its definition site's bindings). The `Arc` shares
    /// the snapshot cheaply across closure clones.
    pub captured: Option<Arc<Scope>>,
    /// Whether this function was defined with the `external` keyword (rule 55).
    /// `true` for externally-defined functions (rejected at invocation time in
    /// 31-06 — only non-external user lambdas are supported); `false` for
    /// regular closures. Built-in functions always carry `external: false`.
    pub external: bool,
}

impl FeelFunction {
    /// Construct a placeholder carrier with the given arity + param/return
    /// types and safe defaults for the Phase 31 fields (`param_names: []`,
    /// `body: None`, `captured: None`). Used by Phase 30 construction sites
    /// that do not yet populate the closure fields.
    pub fn placeholder(arity: usize, param_types: Vec<FeelType>, ret_type: FeelType) -> Self {
        Self {
            arity,
            param_types,
            ret_type: Box::new(ret_type),
            param_names: Vec::new(),
            body: None,
            captured: None,
            external: false,
        }
    }
}

/// The three DMN 1.5 §10.3.2.9.4 conversion contexts.
///
/// - `Filter` → *to singleton list* (a value `T` is wrapped in a singleton
///   `List<T>` when the target is `List<T>`).
/// - `Invocation` → *from singleton list* (a singleton `List<T>` is unwrapped
///   to its element `T` when the target is `T`).
/// - `Binding` → *conforms to* (a value of type `T1` is bound to a target of
///   type `T2` iff `T1 <: T2`; otherwise the binding yields null).
///
/// There is NO `number ↔ string` conversion in any context — that is the
/// Phase 31 explicit `string()` / `number()` built-in (Pitfall 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertCtx {
    /// §10.3.2.9.4 *to singleton list* — `T → List<T>`.
    Filter,
    /// §10.3.2.9.4 *from singleton list* — singleton `List<T> → T`.
    Invocation,
    /// §10.3.2.9.4 *conforms to* — `T1` binds to `T2` iff `T1 <: T2`.
    Binding,
}

/// DMN 1.5 §10.3.2.9.1 — type equivalence (≡).
///
/// Reflexive, symmetric, transitive. At the *type* level (not the value
/// level), two [`FeelType`]s are equivalent iff they are structurally equal
/// (`PartialEq`). The spec's value-level equivalences (e.g. a `date` value
/// equivalent to a `date and time` at UTC midnight, §10.3.2.3.5) are a Phase
/// 31 value-equality concern; the *type-level* relation `Date ≡ DateTime`
/// does NOT hold (it would wrongly make `conforms(DateTime, Date)` true via
/// the equivalence-implies-conformance rule). The one-directional
/// `Date <: DateTime` rule lives in [`conforms`].
pub fn equivalent(a: &FeelType, b: &FeelType) -> bool {
    // §10.3.2.9.1: reflexive, symmetric, transitive. At the *type* level,
    // equivalence is structural equality (PartialEq). The spec's value-level
    // equivalences (e.g. a specific date ≡ a datetime at UTC midnight) are a
    // Phase 31 value-equality concern — at the type level `Date ≡ DateTime`
    // must NOT hold, or `conforms(DateTime, Date)` would wrongly be true via
    // the equivalence-implies-conformance rule. The one-directional
    // `Date <: DateTime` rule lives in [`conforms`] below.
    a == b
}

/// DMN 1.5 §10.3.2.9.2 — type conformance (`<:`, the subtype relation).
///
/// Verbatim per spec (see 30-RESEARCH.md Code Examples):
///
/// - `T <: Any` for every `T`.
/// - `Null <: T` for every `T`.
/// - `(_, Null)` is false (nothing conforms to `Null` except `Null` itself,
///   handled by the equivalence arm).
/// - If `equivalent(a, b)` then `a <: b`.
/// - `Date <: DateTime` (§10.3.2.3.5 — a date is a date-time at UTC midnight).
/// - `list<T> <: list<S>` iff `T <: S`.
/// - `range<T> <: range<S>` iff `T <: S`.
/// - `context<k1:T1..kn:Tn> <: context<l1:S1..lm:Sm>` iff `n ≥ m` and for
///   every `(li, Si)` there is a unique `(kj, Tj)` with `li = kj` and
///   `Tj <: Si`.
/// - `function (T1..Tn)->U <: (S1..Sm)->V` iff `n = m`, `Si <: Ti`
///   (contravariant args), and `U <: V` (covariant return).
/// - Conformance is transitive (the recursive calls give transitivity).
pub fn conforms(a: &FeelType, b: &FeelType) -> bool {
    // §10.3.2.9.2 verbatim (see 30-RESEARCH.md Code Examples). Transitivity
    // comes for free from the recursive calls.
    match (a, b) {
        // T <: Any for every T.
        (_, FeelType::Any) => true,
        // Equivalence implies conformance (T ≡ S ⇒ T <: S). Must come BEFORE
        // the Null-exclusion arm so that `conforms(Null, Null)` returns true.
        (x, y) if equivalent(x, y) => true,
        // Nothing conforms to Null except itself (handled above via equivalence).
        (_, FeelType::Null) => false,
        // §10.3.2.3.5: a date is a date-time at UTC midnight → Date <: DateTime.
        (FeelType::Date, FeelType::DateTime) => true,
        // list<T> <: list<S> iff T <: S.
        (FeelType::List(t), FeelType::List(s)) => conforms(t, s),
        // range<T> <: range<S> iff T <: S.
        (FeelType::Range(t), FeelType::Range(s)) => conforms(t, s),
        // context<k1:T1..kn:Tn> <: context<l1:S1..lm:Sm> iff n ≥ m and for
        // every (li, Si) there is a unique (kj, Tj) with li=kj and Tj <: Si.
        (FeelType::Context(ae), FeelType::Context(be)) => context_conforms(ae, be),
        // function (T1..Tn)->U <: (S1..Sm)->V iff n=m, Si <: Ti (contravariant
        // args), and U <: V (covariant return). Here `a` is the source
        // (params=Ti, ret=U), `b` is the target (params=Si, ret=V).
        (FeelType::Function { params: ap, ret: ar }, FeelType::Function { params: bp, ret: br }) => {
            ap.len() == bp.len()
                && bp.iter().zip(ap).all(|(s, t)| conforms(s, t))
                && conforms(ar, br)
        }
        _ => false,
    }
}

/// §10.3.2.9.2 context conformance helper.
///
/// `ae` is the source context's `(key, type)` entries (n of them), `be` is the
/// target's (m of them). Conformance holds iff `n ≥ m` and for every
/// `(li, Si)` in `be` there is a *unique* `(kj, Tj)` in `ae` with `kj == li`
/// and `Tj <: Si` (i.e. `conforms(Tj, Si)`).
fn context_conforms(ae: &[(String, FeelType)], be: &[(String, FeelType)]) -> bool {
    if ae.len() < be.len() {
        return false;
    }
    be.iter().all(|(l, s)| {
        let matches: Vec<&(String, FeelType)> = ae
            .iter()
            .filter(|(k, _)| k == l)
            .filter(|(_, t)| conforms(t, s))
            .collect();
        matches.len() == 1
    })
}

/// DMN 1.5 §10.3.2.9.4 — the THREE implicit conversions.
///
/// Returns `None` when the conversion is impossible (the Phase 31 evaluator
/// will then yield `null`). There is **no** `number → string` and **no**
/// `string → number` conversion in ANY [`ConvertCtx`] — those are the Phase
/// 31 explicit `string()` / `number()` built-in functions (Pitfall 6). This
/// is the load-bearing prohibition: `1 + "2"` therefore yields `null` (the
/// `+` operator is Phase 31; the lattice provides no escape hatch).
pub fn implicit_convert(v: Value, target: &FeelType, ctx: ConvertCtx) -> Option<Value> {
    match ctx {
        // §10.3.2.9.4 to-singleton-list: T → List<T>. Wrap `v` in a singleton
        // list when `type_of(v) <: target.inner`.
        ConvertCtx::Filter => to_singleton_list(v, target),
        // §10.3.2.9.4 from-singleton-list: singleton List<T> → T. Unwrap the
        // single element when it conforms to `target`.
        ConvertCtx::Invocation => from_singleton_list(v, target),
        // §10.3.2.9.4 conforms-to: T1 binds to T2 iff T1 <: T2; else None.
        ConvertCtx::Binding => {
            if conforms(&type_of(&v), target) {
                Some(v)
            } else {
                None
            }
        }
    }
}

/// §10.3.2.9.4 *to singleton list* (Filter context).
///
/// If `target` is `List(inner)` and `type_of(v) <: inner`, wrap `v` in a
/// singleton `Value::Array`. Otherwise return `None`. There is no
/// number↔string conversion — `conforms` returns false for those pairings, so
/// the wrapping is rejected (Pitfall 6).
fn to_singleton_list(v: Value, target: &FeelType) -> Option<Value> {
    match target {
        FeelType::List(inner) if conforms(&type_of(&v), inner) => Some(Value::Array(vec![v])),
        _ => None,
    }
}

/// §10.3.2.9.4 *from singleton list* (Invocation context).
///
/// If `v` is a singleton `Value::Array([elem])` and `type_of(elem) <: target`,
/// unwrap to `elem`. Otherwise return `None`. No number↔string conversion —
/// the `conforms` check rejects cross-type unwrapping (Pitfall 6).
fn from_singleton_list(v: Value, target: &FeelType) -> Option<Value> {
    match v {
        Value::Array(ref xs) if xs.len() == 1 => {
            let elem = &xs[0];
            if conforms(&type_of(elem), target) {
                Some(elem.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Map a [`Value`] to its [`FeelType`] lattice node.
///
/// Exhaustive over all 16 `Value` variants (8 legacy Rete + 8 FEEL) — NO
/// wildcard. The compiler enforces a new arm if `Value` gains a variant (the
/// exhaustive-match safety net, 30-RESEARCH.md Pitfall 1).
pub fn type_of(v: &Value) -> FeelType {
    // Exhaustive over all 16 Value variants — NO wildcard. The compiler
    // forces a new arm if Value gains a variant (Pitfall 1 safety net).
    match v {
        // Legacy Rete variants.
        Value::Bool(_) => FeelType::Boolean,
        // FEEL treats integers and numbers as one numeric domain (§10.3.2.2);
        // the legacy f64 Float maps to Number here too (FEEL path should not
        // produce Float, but if it appears it is a number).
        Value::Integer(_) => FeelType::Number,
        Value::Float(_) => FeelType::Number,
        Value::String(_) => FeelType::String,
        // Legacy DateTime (NaiveDateTime, no offset) → DateTime type.
        Value::DateTime(_) => FeelType::DateTime,
        Value::Object(obj) => FeelType::Context(
            obj.iter()
                .map(|(k, val)| (k.clone(), type_of(val)))
                .collect(),
        ),
        Value::Array(xs) => {
            let inner = if xs.is_empty() {
                FeelType::Any
            } else {
                type_of(&xs[0])
            };
            FeelType::List(Box::new(inner))
        }
        Value::Null => FeelType::Null,
        // Phase 30 FEEL variants.
        Value::Number(_) => FeelType::Number,
        Value::Date(_) => FeelType::Date,
        Value::Time(_) => FeelType::Time,
        Value::DateTimeTz(_) => FeelType::DateTime,
        Value::DaysTimeDuration(_) => FeelType::DaysTimeDuration,
        Value::YearsMonthsDuration(_) => FeelType::YearsMonthsDuration,
        Value::Range(r) => FeelType::Range(Box::new(range_inner_type(r))),
        Value::Function(f) => FeelType::Function {
            params: f.param_types.clone(),
            ret: f.ret_type.clone(),
        },
    }
}

/// Infer the inner element type of a [`FeelRange`] from its endpoints.
///
/// Uses the start endpoint's type, falling back to the end endpoint, then
/// [`FeelType::Any`] when both are unbounded.
fn range_inner_type(r: &FeelRange) -> FeelType {
    r.start
        .as_ref()
        .or(r.end.as_ref())
        .map(type_of)
        .unwrap_or(FeelType::Any)
}

/// Map an IR `feel:` type_ref string to a [`FeelType`] lattice node.
///
/// Recognised refs (DMN 1.5 §10.3.2.9 type names): `feel:number`,
/// `feel:string`, `feel:boolean`, `feel:date`, `feel:date and time`,
/// `feel:time`, `feel:days and time duration`, `feel:years and months
/// duration`, `feel:Any`, `feel:Null`, plus the recursive `feel:list<T>` and
/// `feel:context<k1:T1, ...>` forms. Returns `None` for unrecognized strings
/// (fail-closed — T-30-04-01 mitigation).
pub fn resolve_type_ref(s: &str) -> Option<FeelType> {
    let s = s.trim();
    let inner = s.strip_prefix("feel:")?;
    // Recursive nested forms first (list<...> / context<...>), then flat names.
    if let Some(rest) = inner.strip_prefix("list<").and_then(|r| r.strip_suffix('>')) {
        let inner_t = resolve_type_ref(rest)?;
        return Some(FeelType::List(Box::new(inner_t)));
    }
    if let Some(rest) = inner
        .strip_prefix("context<")
        .and_then(|r| r.strip_suffix('>'))
    {
        let entries = parse_context_entries(rest)?;
        return Some(FeelType::Context(entries));
    }
    match inner {
        "number" => Some(FeelType::Number),
        "string" => Some(FeelType::String),
        "boolean" => Some(FeelType::Boolean),
        "date" => Some(FeelType::Date),
        "date and time" => Some(FeelType::DateTime),
        "time" => Some(FeelType::Time),
        "days and time duration" => Some(FeelType::DaysTimeDuration),
        "years and months duration" => Some(FeelType::YearsMonthsDuration),
        "Any" | "any" => Some(FeelType::Any),
        "Null" | "null" => Some(FeelType::Null),
        _ => None, // fail-closed (T-30-04-01)
    }
}

/// Parse the `k1:T1, k2:T2, ...` body of a `feel:context<...>` ref.
///
/// Splits on top-level commas (depth-aware so a nested `context<...>` or
/// `list<...>` containing commas/`<` is not split prematurely). Each entry is
/// `key : type_ref` where `type_ref` is forwarded to [`resolve_type_ref`].
/// Returns `None` on any malformed entry (fail-closed).
fn parse_context_entries(body: &str) -> Option<Vec<(String, FeelType)>> {
    let mut entries = Vec::new();
    for raw in split_top_level_commas(body) {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let (key, type_ref) = raw.split_once(':')?;
        let key = key.trim().to_string();
        let t = resolve_type_ref(type_ref.trim())?;
        entries.push((key, t));
    }
    Some(entries)
}

/// Split `s` on top-level commas, respecting `<` / `>` nesting depth so that
/// commas inside a nested `list<...>` / `context<...>` are not split.
fn split_top_level_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut buf = String::new();
    for c in s.chars() {
        match c {
            '<' => {
                depth += 1;
                buf.push(c);
            }
            '>' => {
                depth -= 1;
                buf.push(c);
            }
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut buf));
            }
            other => buf.push(other),
        }
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_types::Value;
    use proptest::prop_assert;
    use rust_decimal::dec;

    // ---- Property tests (§10.3.2.9.1 / §10.3.2.9.2 lattice laws) ----

    proptest::proptest! {
        /// §10.3.2.9.2: conforms is reflexive (T <: T), transitive, and
        /// `Null <: Any` only (DMN-14 instance of semantics).
        #[test]
        fn conformance_proptest(depth in 0u32..=3) {
            let ts = arb_feeltypes(depth);
            for t in &ts {
                // Reflexive: T <: T.
                prop_assert!(conforms(t, t), "reflexivity failed for {:?}", t);
                // Null <: Any only (DMN-14 instance of semantics), plus reflexive.
                prop_assert!(conforms(&FeelType::Null, &FeelType::Any), "Null <: Any failed");
                prop_assert!(!conforms(&FeelType::Null, t) || *t == FeelType::Any || equivalent(&FeelType::Null, t),
                    "Null should only conform to Any (or itself via equivalence), not {:?}", t);
                // T <: Any for every T.
                prop_assert!(conforms(t, &FeelType::Any), "T <: Any failed for {:?}", t);
            }
            // Transitive: T <: S and S <: U implies T <: U.
            for t in &ts {
                for s in &ts {
                    if conforms(t, s) {
                        for u in &ts {
                            if conforms(s, u) {
                                prop_assert!(conforms(t, u), "transitivity failed: {:?} <: {:?} <: {:?}", t, s, u);
                            }
                        }
                    }
                }
            }
        }

        /// §10.3.2.9.1: equivalent is reflexive, symmetric, transitive.
        #[test]
        fn equivalence_proptest(depth in 0u32..=3) {
            let ts = arb_feeltypes(depth);
            for t in &ts {
                // Reflexive.
                prop_assert!(equivalent(t, t), "reflexivity failed for {:?}", t);
            }
            for t in &ts {
                for s in &ts {
                    if equivalent(t, s) {
                        // Symmetric.
                        prop_assert!(equivalent(s, t), "symmetry failed: {:?} ≡ {:?} but not reverse", t, s);
                        // Equivalence implies conformance (both directions).
                        prop_assert!(conforms(t, s), "equivalence-implies-conformance failed: {:?} ≡ {:?}", t, s);
                        // Transitive.
                        for u in &ts {
                            if equivalent(s, u) {
                                prop_assert!(equivalent(t, u), "transitivity failed: {:?} ≡ {:?} ≡ {:?}", t, s, u);
                            }
                        }
                    }
                }
            }
        }
    }

    // ---- Specific conformance rules ----

    #[test]
    fn test_conforms_date_datetime() {
        // §10.3.2.3.5: a date is a date-time at UTC midnight → Date <: DateTime.
        assert!(conforms(&FeelType::Date, &FeelType::DateTime));
        // The reverse does NOT hold: DateTime is not a Date.
        assert!(!conforms(&FeelType::DateTime, &FeelType::Date));
    }

    #[test]
    fn test_conforms_list_covariance() {
        // list<T> <: list<S> iff T <: S.
        let list_num = FeelType::List(Box::new(FeelType::Number));
        let list_any = FeelType::List(Box::new(FeelType::Any));
        let list_str = FeelType::List(Box::new(FeelType::String));
        assert!(conforms(&list_num, &list_any), "List<Number> <: List<Any>");
        assert!(!conforms(&list_num, &list_str), "List<Number> !<: List<String>");
    }

    #[test]
    fn test_conforms_function_contravariance() {
        // DMN 1.5 §10.3.2.9.2: function (T1..Tn)->U conforms to (S1..Sm)->V
        // iff n=m, Si <: Ti (contravariant args), and U <: V (covariant return).
        //
        // A function that accepts Any and returns Number conforms to a function
        // that accepts Number and returns Any:
        //   source = (Any)->Number, target = (Number)->Any
        //   target param (Number) <: source param (Any)  ✓ contravariant
        //   source ret  (Number) <: target ret  (Any)   ✓ covariant
        let source = FeelType::Function {
            params: vec![FeelType::Any],
            ret: Box::new(FeelType::Number),
        };
        let target = FeelType::Function {
            params: vec![FeelType::Number],
            ret: Box::new(FeelType::Any),
        };
        assert!(
            conforms(&source, &target),
            "(Any)->Number should conform to (Number)->Any (contravariant args, covariant return)"
        );

        // The reverse does NOT conform: source=(Number)->Any, target=(Any)->Number.
        //   target param (Any) <: source param (Number) ✗ — Any is not <: Number.
        let source2 = FeelType::Function {
            params: vec![FeelType::Number],
            ret: Box::new(FeelType::Any),
        };
        let target2 = FeelType::Function {
            params: vec![FeelType::Any],
            ret: Box::new(FeelType::Number),
        };
        assert!(
            !conforms(&source2, &target2),
            "(Number)->Any should NOT conform to (Any)->Number (Any is not <: Number)"
        );
    }

    // ---- implicit_convert: exactly the three §10.3.2.9.4 conversions ----

    #[test]
    fn test_implicit_convert_to_singleton_list() {
        // Filter: to-singleton-list — T → List<T>.
        let v = Value::Number(dec!(5));
        let target = FeelType::List(Box::new(FeelType::Number));
        let out = implicit_convert(v.clone(), &target, ConvertCtx::Filter);
        assert_eq!(
            out,
            Some(Value::Array(vec![Value::Number(dec!(5))])),
            "Filter should wrap a Number in a singleton List<Number>"
        );
    }

    #[test]
    fn test_implicit_convert_from_singleton_list() {
        // Invocation: from-singleton-list — singleton List<T> → T.
        let v = Value::Array(vec![Value::Number(dec!(5))]);
        let target = FeelType::Number;
        let out = implicit_convert(v, &target, ConvertCtx::Invocation);
        assert_eq!(
            out,
            Some(Value::Number(dec!(5))),
            "Invocation should unwrap a singleton List<Number> to Number"
        );
    }

    #[test]
    fn test_implicit_convert_conforms_to() {
        // Binding: conforms-to — T1 binds to T2 iff T1 <: T2.
        // Number <: Any → unchanged.
        let v = Value::Number(dec!(5));
        let out = implicit_convert(v.clone(), &FeelType::Any, ConvertCtx::Binding);
        assert_eq!(out, Some(v), "Binding: Number <: Any → value unchanged");
        // String !<: Number → None (no conversion).
        let s = Value::String("x".to_string());
        let out2 = implicit_convert(s.clone(), &FeelType::Number, ConvertCtx::Binding);
        assert_eq!(out2, None, "Binding: String !<: Number → None");
    }

    // ---- THE PROHIBITION: no number↔string conversion in any context ----

    #[test]
    fn test_no_number_string_conversion() {
        // Pitfall 6: there is NO number→string and NO string→number implicit
        // conversion in ANY ConvertCtx. The Phase 31 evaluator will therefore
        // produce null for `1 + "2"` (no escape hatch from the lattice).
        let num = Value::Number(dec!(1));
        let str_val = Value::String("2".to_string());

        // Binding: number→string and string→number both yield None.
        assert_eq!(
            implicit_convert(num.clone(), &FeelType::String, ConvertCtx::Binding),
            None,
            "number→string must NOT convert in Binding context"
        );
        assert_eq!(
            implicit_convert(str_val.clone(), &FeelType::Number, ConvertCtx::Binding),
            None,
            "string→number must NOT convert in Binding context"
        );

        // Filter: to-singleton-list only wraps when target is List<inner>;
        // targeting List<String> with a Number does NOT convert number→string.
        let list_str = FeelType::List(Box::new(FeelType::String));
        assert_eq!(
            implicit_convert(num.clone(), &list_str, ConvertCtx::Filter),
            None,
            "number→List<String> must NOT convert in Filter context (no number→string)"
        );
        let list_num = FeelType::List(Box::new(FeelType::Number));
        assert_eq!(
            implicit_convert(str_val.clone(), &list_num, ConvertCtx::Filter),
            None,
            "string→List<Number> must NOT convert in Filter context (no string→number)"
        );

        // Invocation: from-singleton-list unwraps a singleton list, but the
        // element must conform to the target. A singleton List<Number> unwraps
        // to a Number targeting Number (conforms); targeting String yields None.
        let singleton_num = Value::Array(vec![Value::Number(dec!(1))]);
        assert_eq!(
            implicit_convert(singleton_num.clone(), &FeelType::String, ConvertCtx::Invocation),
            None,
            "singleton List<Number>→String must NOT convert in Invocation context"
        );
        let singleton_str = Value::Array(vec![Value::String("2".to_string())]);
        assert_eq!(
            implicit_convert(singleton_str, &FeelType::Number, ConvertCtx::Invocation),
            None,
            "singleton List<String>→Number must NOT convert in Invocation context"
        );
    }

    // ---- Null propagation through the lattice ----

    #[test]
    fn test_null_propagation() {
        // Null <: Any only (DMN-14 instance of semantics).
        assert!(conforms(&FeelType::Null, &FeelType::Any));
        // Per DMN-14: null does NOT conform to other types.
        // (The Phase 30 implementation had a bug where it treated Null as
        // conforming to everything; evaluate_instance_of special-cases this.)
        assert!(!conforms(&FeelType::Null, &FeelType::Number));
        assert!(!conforms(
            &FeelType::Null,
            &FeelType::List(Box::new(FeelType::String))
        ));
        assert!(!conforms(&FeelType::Null, &FeelType::DateTime));
    }

    // ---- resolve_type_ref ----

    #[test]
    fn test_resolve_type_ref() {
        assert_eq!(resolve_type_ref("feel:number"), Some(FeelType::Number));
        assert_eq!(resolve_type_ref("feel:string"), Some(FeelType::String));
        assert_eq!(resolve_type_ref("feel:boolean"), Some(FeelType::Boolean));
        assert_eq!(resolve_type_ref("feel:date"), Some(FeelType::Date));
        assert_eq!(
            resolve_type_ref("feel:date and time"),
            Some(FeelType::DateTime)
        );
        assert_eq!(resolve_type_ref("feel:time"), Some(FeelType::Time));
        assert_eq!(
            resolve_type_ref("feel:days and time duration"),
            Some(FeelType::DaysTimeDuration)
        );
        assert_eq!(
            resolve_type_ref("feel:years and months duration"),
            Some(FeelType::YearsMonthsDuration)
        );
        assert_eq!(resolve_type_ref("feel:Any"), Some(FeelType::Any));
        assert_eq!(resolve_type_ref("feel:any"), Some(FeelType::Any));
        assert_eq!(resolve_type_ref("feel:Null"), Some(FeelType::Null));
        assert_eq!(resolve_type_ref("feel:null"), Some(FeelType::Null));
        // Nested list form.
        assert_eq!(
            resolve_type_ref("feel:list<feel:number>"),
            Some(FeelType::List(Box::new(FeelType::Number)))
        );
        // Unrecognized → fail-closed (None).
        assert_eq!(resolve_type_ref("feel:garbage"), None);
        assert_eq!(resolve_type_ref("not-a-feel-ref"), None);
    }

    // ---- type_of ----

    #[test]
    fn test_type_of() {
        assert_eq!(type_of(&Value::Number(dec!(5))), FeelType::Number);
        assert_eq!(type_of(&Value::Integer(5)), FeelType::Number);
        assert_eq!(type_of(&Value::Float(5.0)), FeelType::Number);
        assert_eq!(type_of(&Value::Bool(true)), FeelType::Boolean);
        assert_eq!(
            type_of(&Value::String("x".to_string())),
            FeelType::String
        );
        assert_eq!(type_of(&Value::Null), FeelType::Null);
        assert_eq!(
            type_of(&Value::Date(chrono::NaiveDate::from_ymd_opt(2021, 1, 1).unwrap())),
            FeelType::Date
        );
        assert_eq!(
            type_of(&Value::Array(vec![Value::Number(dec!(5))])),
            FeelType::List(Box::new(FeelType::Number))
        );
        assert_eq!(
            type_of(&Value::Array(vec![])),
            FeelType::List(Box::new(FeelType::Any))
        );
    }

    // ---- Helpers: a small recursive FeelType generator for proptest ----

    /// Generate a fixed, varied set of `FeelType` values up to `depth` nesting.
    fn arb_feeltypes(depth: u32) -> Vec<FeelType> {
        let mut base = vec![
            FeelType::Null,
            FeelType::Any,
            FeelType::Number,
            FeelType::String,
            FeelType::Boolean,
            FeelType::Date,
            FeelType::Time,
            FeelType::DateTime,
            FeelType::DaysTimeDuration,
            FeelType::YearsMonthsDuration,
        ];
        if depth > 0 {
            let inner = arb_feeltypes(depth - 1);
            // A representative subset of the inner types for nesting (keep the
            // generated set bounded so the triple-nested transitivity loop
            // stays cheap).
            for t in [&FeelType::Number, &FeelType::Any, &FeelType::String] {
                base.push(FeelType::List(Box::new(t.clone())));
                base.push(FeelType::Range(Box::new(t.clone())));
            }
            // A nested list<list<...>> for depth coverage.
            if let Some(first) = inner.first() {
                base.push(FeelType::List(Box::new(FeelType::List(Box::new(
                    first.clone(),
                )))));
            }
            base.push(FeelType::Function {
                params: vec![FeelType::Any],
                ret: Box::new(FeelType::Number),
            });
            base.push(FeelType::Function {
                params: vec![FeelType::Number],
                ret: Box::new(FeelType::Any),
            });
            base.push(FeelType::Context(vec![
                ("k".to_string(), FeelType::Number),
            ]));
        }
        base
    }

    // ---- Retained Plan 01 skeleton tests (still valid) ----

    #[test]
    fn feeltype_number_variant_constructs() {
        let _t = FeelType::Number;
    }

    #[test]
    fn feeltype_list_and_function_construct() {
        let list = FeelType::List(Box::new(FeelType::Number));
        let f = FeelType::Function {
            params: vec![FeelType::Number],
            ret: Box::new(FeelType::Boolean),
        };
        assert_eq!(list.clone(), FeelType::List(Box::new(FeelType::Number)));
        let FeelType::Function { params, ret } = &f else {
            panic!("expected FeelType::Function");
        };
        assert_eq!(params.len(), 1);
        assert_eq!(**ret, FeelType::Boolean);
    }

    #[test]
    fn feel_range_carrier_round_trips() {
        let r = FeelRange {
            start: Some(Value::Null),
            end: None,
            start_incl: true,
            end_incl: false,
        };
        assert!(r.start_incl);
        assert!(r.end.is_none());
        let cloned = r.clone();
        assert_eq!(r, cloned);
    }

    #[test]
    fn feel_function_carrier_constructs() {
        let f = FeelFunction {
            arity: 1,
            param_types: vec![FeelType::Number],
            ret_type: Box::new(FeelType::Boolean),
            param_names: vec!["x".to_string()],
            body: None,
            captured: None,
            external: false,
        };
        assert_eq!(f.arity, 1);
        assert_eq!(f.param_names, vec!["x".to_string()]);
        assert!(f.body.is_none());
        assert!(f.captured.is_none());
        assert_eq!(f, f.clone());
        // The placeholder constructor yields safe defaults.
        let p = FeelFunction::placeholder(0, vec![], FeelType::Any);
        assert!(p.param_names.is_empty());
        assert!(p.body.is_none());
        assert!(p.captured.is_none());
    }

    #[test]
    fn feeltype_has_all_lattice_nodes() {
        let _ = FeelType::Null;
        let _ = FeelType::Any;
        let _ = FeelType::Number;
        let _ = FeelType::String;
        let _ = FeelType::Boolean;
        let _ = FeelType::Date;
        let _ = FeelType::Time;
        let _ = FeelType::DateTime;
        let _ = FeelType::DaysTimeDuration;
        let _ = FeelType::YearsMonthsDuration;
        let _ = FeelType::List(Box::new(FeelType::Any));
        let _ = FeelType::Context(vec![("k".to_string(), FeelType::Any)]);
        let _ = FeelType::Range(Box::new(FeelType::Any));
        let _ = FeelType::Function {
            params: vec![],
            ret: Box::new(FeelType::Any),
        };
    }
}