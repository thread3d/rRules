//! Explicit FEEL coercions used by operators and built-in functions.
//!
//! Distinct from the Phase 30 lattice `implicit_convert`
//! (`feel::types::implicit_convert` — exactly the three §10.3.2.9.4
//! conversions: to-singleton-list, from-singleton-list, conforms-to). This
//! module carries the **explicit** coercions the operators and built-ins
//! perform, e.g. number/integer cross-type promotion in arithmetic,
//! singleton-list unwrapping for `+`/`-` on a list of one, etc.
//!
//! **Prohibition (carried from Phase 30 Pitfall 6):** there is NO
//! number↔string conversion here — that is the explicit `string()` /
//! `number()` built-in (31-07), NOT an operator coercion. `1 + "2"` yields
//! `Value::Null`.
//!
//! Plan 31-03 (this file) implements the operator/builtin coercions:
//! - [`coerce_to_number`] — `Number`/`Integer` → `Decimal`; `String`/`Bool`/
//!   `Null` → `None` (NO implicit number↔string; the explicit `number()`
//!   built-in in 31-07 is the only such path).
//! - [`coerce_to_string`] — `String` → `Some`; `Number`/`Bool`/`Null` → `None`.
//! - [`coerce_to_bool`] — `Bool` → `Some`; everything else → `None`.
//!
//! These are the ONLY explicit coercions the FEEL operators use. Number↔string
//! is NOT here (that is the 31-07 `string()`/`number()` built-ins per the
//! DMN-13 prohibition).

use crate::data_types::Value;
use rust_decimal::Decimal;

/// Coerce a [`Value`] to a FEEL number (`Decimal`).
///
/// - [`Value::Number`] → `Some(inner)`.
/// - [`Value::Integer`] → `Some(Decimal::from(i))` (the FEEL numeric domain
///   treats integers and numbers as one domain per §10.3.2.2; arithmetic
///   promotes integers to `Decimal`).
/// - [`Value::String`] / [`Value::Bool`] / [`Value::Null`] / anything else →
///   `None`. **There is NO implicit number↔string conversion** (Phase 30
///   Pitfall 6; the explicit `number()` built-in in 31-07 is the only such
///   path).
pub fn coerce_to_number(v: &Value) -> Option<Decimal> {
    match v {
        Value::Number(x) => Some(*x),
        Value::Integer(i) => Some(Decimal::from(*i)),
        // NO implicit number<->string conversion — Pitfall 6.
        _ => None,
    }
}

/// Coerce a [`Value`] to a FEEL string.
///
/// - [`Value::String`] → `Some(inner)`.
/// - [`Value::Number`] / [`Value::Bool`] / [`Value::Null`] / anything else →
///   `None`. The explicit `string()` built-in (31-07) is the only
///   number→string / bool→string path; operators NEVER perform it.
pub fn coerce_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// Coerce a [`Value`] to a FEEL boolean.
///
/// - [`Value::Bool`] → `Some(inner)`.
/// - [`Value::Null`] → `None` (null in a boolean context is null per 3VL).
/// - Any other kind → `None` (a non-boolean in a boolean context is a type
///   error → null, per 3VL).
pub fn coerce_to_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

// =====================================================================
// Tests — flat at the module level so cargo test filter substrings
// (`dmn::feel::coercion::coerce_`) match directly (per the 31-02 test-
// flattening decision).
// =====================================================================

#[cfg(test)]
use rust_decimal::dec;

#[cfg(test)]
#[test]
fn coerce_to_number_number() {
    assert_eq!(coerce_to_number(&Value::Number(dec!(5))), Some(dec!(5)));
}

#[cfg(test)]
#[test]
fn coerce_to_number_integer_promotes() {
    // §10.3.2.2: integers and numbers are one numeric domain.
    assert_eq!(coerce_to_number(&Value::Integer(5)), Some(dec!(5)));
    assert_eq!(coerce_to_number(&Value::Integer(-3)), Some(dec!(-3)));
}

#[cfg(test)]
#[test]
fn coerce_to_number_string_is_none_no_implicit() {
    // Pitfall 6: NO implicit number<->string conversion.
    assert_eq!(coerce_to_number(&Value::String("5".into())), None);
}

#[cfg(test)]
#[test]
fn coerce_to_number_bool_is_none() {
    assert_eq!(coerce_to_number(&Value::Bool(true)), None);
}

#[cfg(test)]
#[test]
fn coerce_to_number_null_is_none() {
    assert_eq!(coerce_to_number(&Value::Null), None);
}

#[cfg(test)]
#[test]
fn coerce_to_string_string() {
    assert_eq!(
        coerce_to_string(&Value::String("abc".into())),
        Some("abc".to_string())
    );
}

#[cfg(test)]
#[test]
fn coerce_to_string_number_is_none_no_implicit() {
    // Pitfall 6: NO implicit number->string conversion.
    assert_eq!(coerce_to_string(&Value::Number(dec!(5))), None);
}

#[cfg(test)]
#[test]
fn coerce_to_string_null_is_none() {
    assert_eq!(coerce_to_string(&Value::Null), None);
}

#[cfg(test)]
#[test]
fn coerce_to_bool_bool() {
    assert_eq!(coerce_to_bool(&Value::Bool(true)), Some(true));
    assert_eq!(coerce_to_bool(&Value::Bool(false)), Some(false));
}

#[cfg(test)]
#[test]
fn coerce_to_bool_number_is_none() {
    assert_eq!(coerce_to_bool(&Value::Number(dec!(1))), None);
}

#[cfg(test)]
#[test]
fn coerce_to_bool_null_is_none() {
    assert_eq!(coerce_to_bool(&Value::Null), None);
}
