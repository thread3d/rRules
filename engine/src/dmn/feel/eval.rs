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

#[allow(unused_imports)]
use crate::data_types::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[allow(unused_imports)]
use std::cmp::Ordering;

#[allow(unused_imports)]
use super::ast::FeelExpression;
#[allow(unused_imports)]
use super::numeric::feel_numeric_eq;
#[allow(unused_imports)]
use super::temporal::parse_at_literal;
#[allow(unused_imports)]
use super::types::FeelRange;

// During the RED phase some of these imports are not yet consumed by the
// stub bodies; the GREEN implementation (Task 1 / Task 2) uses them all.
// `#[allow(unused_imports)]` is scoped here rather than scattered across
// the stubs.

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
        // RED-phase stub: returns None for everything. GREEN replaces this
        // with the full same-kind dispatch.
        let _ = (a, b);
        None
    }

    /// FEEL ordering — `Option<Ordering>` (null-propagating). NEVER reuse Rust `Ord`.
    ///
    /// - Any `null` operand → `None` (`null < 3` → `null`).
    /// - Same-kind, comparable → `Some(Ordering)`.
    /// - **Different kind** → `None` (input domain requires same kind).
    pub fn feel_compare(a: &Value, b: &Value) -> Option<Ordering> {
        let _ = (a, b);
        None
    }

    /// Explicit three-valued AND. `false` is dominant; `null` propagates
    /// otherwise. Truth table (31-RESEARCH.md):
    /// - `(Some(false), _) | (_, Some(false))` → `Some(false)`.
    /// - `(Some(true), Some(true))` → `Some(true)`.
    /// - `_` → `None` (null present, no false).
    pub fn and_3vl(a: Option<bool>, b: Option<bool>) -> Option<bool> {
        let _ = (a, b);
        None
    }

    /// Explicit three-valued OR. `true` is dominant; `null` propagates
    /// otherwise. Truth table (31-RESEARCH.md):
    /// - `(Some(true), _) | (_, Some(true))` → `Some(true)`.
    /// - `(Some(false), Some(false))` → `Some(false)`.
    /// - `_` → `None` (null present, no true).
    pub fn or_3vl(a: Option<bool>, b: Option<bool>) -> Option<bool> {
        let _ = (a, b);
        None
    }

    /// Explicit three-valued NOT. `null` → `null`; else negate.
    /// - `Some(true)` → `Some(false)`.
    /// - `Some(false)` → `Some(true)`.
    /// - `None` → `None` (`not(null)` → `null`).
    pub fn not_3vl(a: Option<bool>) -> Option<bool> {
        let _ = a;
        None
    }

    /// Element-wise list equality. Two lists are equal iff same length and
    /// every pair is `feel_equals`-equal (with no `None`).
    #[allow(dead_code)] // populated in GREEN; used by feel_equals's Array arm.
    fn list_equals(x: &[Value], y: &[Value]) -> Option<bool> {
        let _ = (x, y);
        None
    }

    /// Context equality: same keys, and each key's value is `feel_equals`-equal.
    #[allow(dead_code)] // populated in GREEN; used by feel_equals's Object arm.
    fn context_equals(x: &std::collections::HashMap<String, Value>, y: &std::collections::HashMap<String, Value>) -> Option<bool> {
        let _ = (x, y);
        None
    }

    /// §10.3.2.3.5: a `date` equals a `date and time` at UTC midnight on
    /// that date.
    #[allow(dead_code)] // populated in GREEN; used by feel_equals's Date/DateTimeTz arm.
    fn date_equals_datetime_z(d: &chrono::NaiveDate, dtv: &super::super::temporal::DateTimeValue) -> Option<bool> {
        let _ = (d, dtv);
        None
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
    // RED-phase stub: returns Value::Null for everything. GREEN replaces
    // this with the exhaustive dispatch.
    let _ = (expr, scope);
    Value::Null
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

// Task 2 dispatch tests are added in the Task 2 RED commit.