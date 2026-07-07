//! FEEL Decimal128 numeric helpers.
//!
//! Implements the FEEL numeric domain (DMN-08) backed by
//! `rust_decimal::Decimal` (per 30-RESEARCH.md Standard Stack). The headline
//! proof — `0.1 + 0.2 == 0.3` holds in `Decimal` because it is a *decimal*
//! (not binary-float) type — lives in [`tests::test_decimal_add`].
//!
//! Plan 30-02 delivers:
//! - [`feel_numeric_eq`] — null-propagating FEEL numeric equality with
//!   number/integer cross-type promotion (DMN 1.5 §10.3.2.2 numeric domain).
//!   Returns `Option<bool>` so the Phase 31 ternary-logic operators can
//!   consume the `None` (null-propagated) result directly.
//!
//! The arithmetic operators (`+`, `-`, `*`, `/`, `**`) are Phase 31 (DMN-13);
//! this module only proves the value domain and equality.

use crate::data_types::Value;

/// FEEL numeric equality — `Option<bool>` (null-propagating).
///
/// **RED STUB** — implementation lands in the GREEN commit of Plan 30-02
/// Task 1. The body is intentionally wrong (always `None`) so the tests
/// below fail and the RED gate is genuine.
pub fn feel_numeric_eq(_a: &Value, _b: &Value) -> Option<bool> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    /// Headline DMN-08 proof: `0.1 + 0.2 == 0.3` holds in
    /// `rust_decimal::Decimal` (decimal, not binary float).
    #[test]
    fn test_decimal_add() {
        assert_eq!(dec!(0.1) + dec!(0.2), dec!(0.3));
    }

    /// Negative-contrast: `f64` gives `0.30000000000000004` for `0.1 + 0.2`,
    /// demonstrating WHY the FEEL numeric path uses `Number(Decimal)` and
    /// never `Float(f64)`. This is not a FEEL behavior — it is the binary
    /// float problem FEEL avoids.
    #[test]
    fn test_float_not_equal() {
        let f: f64 = 0.1 + 0.2;
        assert_ne!(f, 0.3_f64);
    }

    /// `feel_numeric_eq` on two `Value::Number` — equal and unequal cases.
    #[test]
    fn test_feel_numeric_eq_number() {
        assert_eq!(
            feel_numeric_eq(&Value::Number(dec!(5)), &Value::Number(dec!(5))),
            Some(true)
        );
        assert_eq!(
            feel_numeric_eq(&Value::Number(dec!(5)), &Value::Number(dec!(6))),
            Some(false)
        );
    }

    /// Cross-type: `Value::Number(5)` equals `Value::Integer(5)` (FEEL numeric
    /// domain treats integer-valued numbers and integers as the same domain
    /// per spec §10.3.2.2). Number↔String returns `None` (NO implicit
    /// number→string conversion — that is a Phase 31 built-in).
    #[test]
    fn test_feel_numeric_eq_cross_type() {
        assert_eq!(
            feel_numeric_eq(&Value::Number(dec!(5)), &Value::Integer(5)),
            Some(true)
        );
        assert_eq!(
            feel_numeric_eq(&Value::Number(dec!(5)), &Value::String("5".into())),
            None
        );
    }

    /// Null propagates: `feel_numeric_eq(Value::Null, Value::Number(5))`
    /// returns `None` (ternary logic; Phase 31 consumes this `Option`).
    #[test]
    fn test_feel_numeric_eq_null() {
        assert_eq!(
            feel_numeric_eq(&Value::Null, &Value::Number(dec!(5))),
            None
        );
    }
}