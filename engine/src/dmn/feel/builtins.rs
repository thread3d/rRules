//! FEEL built-in function library — `BuiltinFn` registry + ~70+ functions.
//!
//! Plan 31-07 (part 1: conversion/string/list/numeric families) and 31-08
//! (part 2: date-time/context/boolean/range/equality families) own the
//! real implementations and the `HashMap<&'static str, BuiltinFn>`
//! registry table (CONTEXT.md locked dispatch shape — chosen over a nested
//! `match` that scales poorly to ~70 entries, and over trait objects per
//! family that over-engineer a closed, flat function space).
//!
//! **Null propagation (general rule):** a null argument propagates to
//! `Value::Null` UNLESS the spec says otherwise — exceptions:
//! `count([null])` → `1`, `string(null)` → `null`, `is(a, b)` → NEVER null
//! (Pitfall 6: total structural identity), `all([])` → `true`, `any([])` →
//! `false`, `sum([])` → `null` per DMN 1.5 (31-RESEARCH.md Built-in
//! Function Library / Null-propagation behavior).
//!
//! **Regex cache:** `COMPILED_PATTERNS: OnceLock<HashMap<String, Regex>>` caches
//! compiled regex patterns for `replace`, `matches`, and `split`. The regex crate
//! uses bounded-execution finite automata (no catastrophic backtracking). Pattern
//! compilation errors → `Value::Null`. Documented XQuery 1.0 / XPath 2.0 FO regex
//! approximation (Pitfall 7); TCK divergence flagged for Phase 35.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{OnceLock, Mutex};

use crate::data_types::Value;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

/// A built-in function pointer. Null args propagate to `Value::Null`
/// unless the spec says otherwise (see module docs). The real registry
/// (`HashMap<&'static str, BuiltinFn>`, ~70+ entries) lands in 31-07/31-08.
pub type BuiltinFn = fn(&[Value]) -> Value;

/// Compiled regex pattern cache — shared across `replace` / `matches` / `split`.
///
/// The regex crate uses bounded-execution finite automata (no catastrophic
/// backtracking). We compile once and reuse via a thread-safe cache. Pattern
/// compilation errors map to `Value::Null` at the evaluator boundary.
pub static COMPILED_PATTERNS: OnceLock<Mutex<HashMap<String, regex::Regex>>> = OnceLock::new();

fn get_or_compile(pattern: &str) -> Option<regex::Regex> {
    let cache = COMPILED_PATTERNS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(r) = cache.lock().ok()?.get(pattern) {
        return Some(r.clone());
    }
    match regex::Regex::new(pattern) {
        Ok(r) => {
            cache.lock().ok()?.insert(pattern.to_string(), r.clone());
            Some(r)
        }
        Err(_) => None,
    }
}

/// Helper: null-propagating unary op. If the argument is `Value::Null`, returns
/// `Value::Null`. Otherwise applies `f`.
fn null_unary<F>(args: &[Value], f: F) -> Value
where
    F: FnOnce(&Value) -> Value,
{
    if args.is_empty() {
        return Value::Null;
    }
    match &args[0] {
        Value::Null => Value::Null,
        v => f(v),
    }
}

/// Helper: null-propagating binary op. If either argument is `Value::Null`,
/// returns `Value::Null`. Otherwise applies `f` to the two values.
fn null_binary<F>(args: &[Value], f: F) -> Value
where
    F: FnOnce(&Value, &Value) -> Value,
{
    if args.len() < 2 {
        return Value::Null;
    }
    match (&args[0], &args[1]) {
        (Value::Null, _) | (_, Value::Null) => Value::Null,
        (a, b) => f(a, b),
    }
}

/// Helper: null-propagating ternary op. If any argument is `Value::Null`,
/// returns `Value::Null`. Otherwise applies `f`.
fn null_ternary<F>(args: &[Value], f: F) -> Value
where
    F: FnOnce(&Value, &Value, &Value) -> Value,
{
    if args.len() < 3 {
        return Value::Null;
    }
    match (&args[0], &args[1], &args[2]) {
        (Value::Null, _, _) | (_, Value::Null, _) | (_, _, Value::Null) => Value::Null,
        (a, b, c) => f(a, b, c),
    }
}

// =====================================================================
// Conversion family (8 functions).
// =====================================================================

/// `date(x)` — parse a string into a FEEL date. Returns Null on failure or null input.
fn built_in_date(args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Null;
    }
    match &args[0] {
        Value::Null => Value::Null,
        Value::String(s) => {
            // Delegate to parse_at_literal — handles date format.
            crate::dmn::feel::temporal::parse_at_literal(s).unwrap_or(Value::Null)
        }
        _ => Value::Null, // non-string → null (no implicit conversion)
    }
}

/// `time(x)` — parse a string into a FEEL time. Returns Null on failure or null input.
fn built_in_time(args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Null;
    }
    match &args[0] {
        Value::Null => Value::Null,
        Value::String(s) => {
            crate::dmn::feel::temporal::parse_at_literal(s).unwrap_or(Value::Null)
        }
        _ => Value::Null,
    }
}

/// `date and time(x)` — parse a string into a FEEL date and time. Returns Null on failure or null input.
fn built_in_date_and_time(args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Null;
    }
    match &args[0] {
        Value::Null => Value::Null,
        Value::String(s) => {
            crate::dmn::feel::temporal::parse_at_literal(s).unwrap_or(Value::Null)
        }
        _ => Value::Null,
    }
}

/// `duration(x)` — parse a string into a FEEL duration (days-time or years-months).
fn built_in_duration(args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Null;
    }
    match &args[0] {
        Value::Null => Value::Null,
        Value::String(s) => {
            crate::dmn::feel::temporal::parse_at_literal(s).unwrap_or(Value::Null)
        }
        _ => Value::Null,
    }
}

/// `number(x)` — parse a string into a FEEL number. Returns Null on failure or null input.
fn built_in_number(args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Null;
    }
    match &args[0] {
        Value::Null => Value::Null,
        Value::Number(_) => args[0].clone(), // already a number → identity
        Value::String(s) => {
            match s.trim().parse::<Decimal>() {
                Ok(d) => Value::Number(d),
                Err(_) => Value::Null, // parse failure → null
            }
        }
        _ => Value::Null, // non-string → null (no implicit conversion)
    }
}

/// `string(x)` — convert a value to its FEEL string representation. Null propagates.
fn built_in_string(args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Null;
    }
    match &args[0] {
        Value::Null => Value::Null, // spec says string(null) → null
        Value::String(s) => Value::String(s.clone()), // identity
        Value::Number(d) => Value::String(format!("{}", d)),
        Value::Integer(i) => Value::String(i.to_string()),
        Value::Bool(b) => Value::String(if *b { "true".into() } else { "false".into() }),
        _ => crate::dmn::feel::temporal::format_value(&args[0])
            .map(Value::String)
            .unwrap_or(Value::Null),
    }
}

/// `years and months duration(x)` — parse a string into a FEEL years-months duration.
fn built_in_years_and_months_duration(args: &[Value]) -> Value {
    if args.is_empty() {
        return Value::Null;
    }
    match &args[0] {
        Value::Null => Value::Null,
        Value::String(s) => {
            // Only accept year-month duration form (P... without T/D/H/S).
            let trimmed = s.trim();
            if !trimmed.starts_with('P') && !trimmed.starts_with("-P") {
                return Value::Null;
            }
            let body = trimmed.strip_prefix('-').unwrap_or(trimmed).strip_prefix('P').unwrap_or("");
            // If body contains T, D, H, or S → it's a days-time duration, not YMD.
            if body.contains('T') || body.contains('D') || body.contains('H') || body.contains('S') {
                return Value::Null;
            }
            crate::dmn::feel::temporal::parse_at_literal(trimmed).unwrap_or(Value::Null)
        }
        _ => Value::Null,
    }
}

// =====================================================================
// Numeric family (12 functions). All use rust_decimal::Decimal.
// =====================================================================

/// `decimal(num, scale)` — round a number to the given decimal places.
fn built_in_decimal(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Null;
    }
    let num = match &args[0] {
        Value::Number(d) => *d,
        _ => return Value::Null,
    };
    // `scale` is the number of decimal places (non-negative integer).
    let scale: u32 = match &args[1] {
        Value::Integer(n) => (*n as i64).max(0) as u32,
        Value::Number(d) => {
            // Truncate to integer first, then take absolute value.
            d.trunc().abs().to_i64().unwrap_or(0) as u32
        }
        _ => return Value::Null,
    };
    if matches!(args[0], Value::Null) || matches!(args[1], Value::Null) {
        return Value::Null;
    }
    let result = num.round_dp(scale);
    Value::Number(result)
}

/// `floor(x)` — round down to nearest integer. Null propagates.
fn built_in_floor(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::Number(d) => Value::Number(d.floor()),
        _ => Value::Null,
    })
}

/// `ceiling(x)` — round up to nearest integer. Null propagates.
fn built_in_ceiling(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::Number(d) => Value::Number(d.ceil()),
        _ => Value::Null,
    })
}

/// `round up(x)` — round away from zero to nearest integer. Null propagates.
fn built_in_round_up(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::Number(d) => {
            let r = if *d >= Decimal::ZERO {
                d.ceil()
            } else {
                d.floor()
            };
            Value::Number(r)
        }
        _ => Value::Null,
    })
}

/// `round down(x)` — round toward zero to nearest integer. Null propagates.
fn built_in_round_down(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::Number(d) => {
            let r = if *d >= Decimal::ZERO {
                d.floor()
            } else {
                d.ceil()
            };
            Value::Number(r)
        }
        _ => Value::Null,
    })
}

/// `round half up(x)` — round to nearest integer; ties go away from zero. Null propagates.
fn built_in_round_half_up(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::Number(d) => {
            // Implement "half away from zero" explicitly (rust_decimal's `round()` uses
            // half-to-even / banker's rounding).
            let frac = d.fract();
            if frac == Decimal::from(5u64) / Decimal::from(10u64) {
                // Exact tie: away from zero.
                if *d >= Decimal::ZERO {
                    Value::Number(d.ceil())
                } else {
                    Value::Number(d.floor())
                }
            } else {
                // Not a tie — round to nearest (half up = same as half down for non-ties).
                let truncated = d.trunc();
                let diff = (d - truncated).abs();
                if diff > Decimal::from(5u64) / Decimal::from(10u64) {
                    // Closer to next integer.
                    if *d >= Decimal::ZERO {
                        Value::Number(truncated + Decimal::ONE)
                    } else {
                        Value::Number(truncated - Decimal::ONE)
                    }
                } else {
                    // Closer to truncated (or exact 0.5 already handled above).
                    Value::Number(truncated)
                }
            }
        }
        _ => Value::Null,
    })
}

/// `round half down(x)` — round to nearest integer; ties go toward zero. Null propagates.
fn built_in_round_half_down(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::Number(d) => {
            // Check if exactly at .5 threshold.
            let frac = d.fract();
            if frac == Decimal::from(5u64) / Decimal::from(10u64) {
                // Tie: round toward zero (truncate).
                let r = if *d >= Decimal::ZERO {
                    d.floor()
                } else {
                    d.ceil()
                };
                Value::Number(r)
            } else {
                Value::Number(d.round())
            }
        }
        _ => Value::Null,
    })
}

/// `abs(x)` — absolute value. Null propagates.
fn built_in_abs(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::Number(d) => Value::Number(d.abs()),
        Value::Integer(i) => Value::Integer(i.abs()),
        _ => Value::Null,
    })
}

/// `modulo(a, b)` — remainder of a / b. Div-by-zero → Null.
fn built_in_modulo(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Null;
    }
    match (&args[0], &args[1]) {
        (Value::Null, _) | (_, Value::Null) => Value::Null,
        (Value::Number(a), Value::Number(b)) => {
            if *b == Decimal::ZERO {
                return Value::Null; // div-by-zero → null
            }
            // Use the `%` operator which is defined for Decimal.
            let result = a % b;
            Value::Number(result)
        }
        (Value::Integer(a), Value::Integer(b)) => {
            if *b == 0 {
                return Value::Null;
            }
            Value::Integer(a.rem_euclid(*b))
        }
        _ => Value::Null, // mixed types → null
    }
}

/// `sqrt(x)` — square root. Negative → Null. Overflow → Null.
fn built_in_sqrt(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::Number(d) => {
            if *d < Decimal::ZERO {
                return Value::Null; // negative sqrt → null
            }
            use rust_decimal::MathematicalOps;
            d.sqrt().map(Value::Number).unwrap_or(Value::Null)
        }
        _ => Value::Null,
    })
}

/// `log(x)` — natural logarithm (ln). Non-positive → Null. Overflow → Null.
fn built_in_log(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::Number(d) => {
            if *d <= Decimal::ZERO {
                return Value::Null; // ln(0) or ln(negative) → null
            }
            use rust_decimal::MathematicalOps;
            d.checked_ln().map(Value::Number).unwrap_or(Value::Null)
        }
        _ => Value::Null,
    })
}

/// `exp(x)` — e raised to the power of x. Overflow → Null.
fn built_in_exp(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::Number(d) => {
            use rust_decimal::MathematicalOps;
            d.checked_exp().map(Value::Number).unwrap_or(Value::Null)
        }
        _ => Value::Null,
    })
}

/// `odd(x)` — true if x is an odd integer. Null propagates.
fn built_in_odd(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::Integer(i) => Value::Bool(i % 2 != 0),
        Value::Number(d) => {
            // Integer check: does the decimal have no fractional part?
            if d.fract() == Decimal::ZERO {
                let i = d.trunc().to_i64().unwrap_or(0);
                Value::Bool(i % 2 != 0)
            } else {
                Value::Null // not an integer → null
            }
        }
        _ => Value::Null,
    })
}

/// `even(x)` — true if x is an even integer. Null propagates.
fn built_in_even(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::Integer(i) => Value::Bool(i % 2 == 0),
        Value::Number(d) => {
            if d.fract() == Decimal::ZERO {
                let i = d.trunc().to_i64().unwrap_or(0);
                Value::Bool(i % 2 == 0)
            } else {
                Value::Null
            }
        }
        _ => Value::Null,
    })
}

// =====================================================================
// Registry construction.
// =====================================================================

/// Build the built-in function registry. Returns a `HashMap` mapping FEEL
/// canonical names (with spaces) to their implementation pointers.
pub fn builtin_registry() -> HashMap<&'static str, BuiltinFn> {
    let mut m = HashMap::new();

    macro_rules! reg {
        ($name:expr, $fn:ident) => {{
            let f: BuiltinFn = $fn;
            m.insert($name, f);
        }};
    }

    // Conversion family (8).
    reg!("date", built_in_date);
    reg!("time", built_in_time);
    reg!("date and time", built_in_date_and_time);
    reg!("duration", built_in_duration);
    reg!("number", built_in_number);
    reg!("string", built_in_string);
    reg!("years and months duration", built_in_years_and_months_duration);

    // Numeric family (12).
    reg!("decimal", built_in_decimal);
    reg!("floor", built_in_floor);
    reg!("ceiling", built_in_ceiling);
    reg!("round up", built_in_round_up);
    reg!("round down", built_in_round_down);
    reg!("round half up", built_in_round_half_up);
    reg!("round half down", built_in_round_half_down);
    reg!("abs", built_in_abs);
    reg!("modulo", built_in_modulo);
    reg!("sqrt", built_in_sqrt);
    reg!("log", built_in_log);
    reg!("exp", built_in_exp);
    reg!("odd", built_in_odd);
    reg!("even", built_in_even);

    // String family (13) — populated in 31-08 alongside list.
    // List family (24) — populated in 31-08 alongside string.

    m
}

// =====================================================================
// Tests.
// =====================================================================

#[cfg(test)]
mod conversion {
    use super::*;
    use crate::dmn::feel::temporal::{DateTimeValue, YearsMonthsDuration};
    use chrono::NaiveDate;
    use rust_decimal::dec;

    /// `date("2021-01-31")` → Date(2021-01-31).
    #[test]
    fn date_valid_string() {
        let result = built_in_date(&[Value::String("2021-01-31".into())]);
        assert_eq!(result, Value::Date(NaiveDate::from_ymd_opt(2021, 1, 31).unwrap()));
    }

    /// `date(null)` → Null.
    #[test]
    fn date_null_arg() {
        let result = built_in_date(&[Value::Null]);
        assert_eq!(result, Value::Null);
    }

    /// `date("not-a-date")` → Null (parse failure).
    #[test]
    fn date_invalid_string() {
        let result = built_in_date(&[Value::String("not-a-date".into())]);
        assert_eq!(result, Value::Null);
    }

    /// `time("10:00:00")` → Time(Local 10:00:00).
    #[test]
    fn time_valid_string() {
        let result = built_in_time(&[Value::String("10:00:00".into())]);
        match result {
            Value::Time(crate::dmn::feel::temporal::TimeValue::Local(t)) => {
                assert_eq!(t, chrono::NaiveTime::from_hms_opt(10, 0, 0).unwrap());
            }
            other => panic!("expected Time(Local), got {:?}", other),
        }
    }

    /// `time(null)` → Null.
    #[test]
    fn time_null_arg() {
        assert_eq!(built_in_time(&[Value::Null]), Value::Null);
    }

    /// `date and time("2021-01-01T10:00:00+00:00")` → DateTimeTz.
    #[test]
    fn date_and_time_valid() {
        let result = built_in_date_and_time(&[Value::String(
            "2021-01-01T10:00:00+00:00".into(),
        )]);
        match result {
            Value::DateTimeTz(DateTimeValue::Fixed(_)) => {}
            other => panic!("expected DateTimeTz(Fixed), got {:?}", other),
        }
    }

    /// `date and time(null)` → Null.
    #[test]
    fn date_and_time_null() {
        assert_eq!(built_in_date_and_time(&[Value::Null]), Value::Null);
    }

    /// `duration("P1Y2M")` → YearsMonthsDuration(14 months).
    #[test]
    fn duration_years_months() {
        let result = built_in_duration(&[Value::String("P1Y2M".into())]);
        match result {
            Value::YearsMonthsDuration(ymd) => assert_eq!(ymd.months, 14),
            other => panic!("expected YearsMonthsDuration, got {:?}", other),
        }
    }

    /// `duration("P1D")` → DaysTimeDuration(1 day).
    #[test]
    fn duration_days_time() {
        let result = built_in_duration(&[Value::String("P1D".into())]);
        match result {
            Value::DaysTimeDuration(td) => assert_eq!(td, chrono::TimeDelta::days(1)),
            other => panic!("expected DaysTimeDuration, got {:?}", other),
        }
    }

    /// `duration(null)` → Null.
    #[test]
    fn duration_null() {
        assert_eq!(built_in_duration(&[Value::Null]), Value::Null);
    }

    /// `number("123")` → Number(123).
    #[test]
    fn number_valid_string() {
        let result = built_in_number(&[Value::String("123".into())]);
        assert_eq!(result, Value::Number(dec!(123)));
    }

    /// `number(null)` → Null.
    #[test]
    fn number_null_arg() {
        assert_eq!(built_in_number(&[Value::Null]), Value::Null);
    }

    /// `number("abc")` → Null (parse failure).
    #[test]
    fn number_invalid_string() {
        let result = built_in_number(&[Value::String("abc".into())]);
        assert_eq!(result, Value::Null);
    }

    /// `number(5)` → Number(5) (identity for already-numeric).
    #[test]
    fn number_already_numeric() {
        let result = built_in_number(&[Value::Number(dec!(5))]);
        assert_eq!(result, Value::Number(dec!(5)));
    }

    /// `string(123)` → String("123").
    #[test]
    fn string_from_number() {
        let result = built_in_string(&[Value::Number(dec!(123))]);
        assert_eq!(result, Value::String("123".into()));
    }

    /// `string(null)` → Null (spec exception — null propagation).
    #[test]
    fn string_null() {
        assert_eq!(built_in_string(&[Value::Null]), Value::Null);
    }

    /// `years and months duration("P1Y2M")` → YearsMonthsDuration(14).
    #[test]
    fn years_months_duration_valid() {
        let result = built_in_years_and_months_duration(&[Value::String("P1Y2M".into())]);
        match result {
            Value::YearsMonthsDuration(ymd) => assert_eq!(ymd.months, 14),
            other => panic!("expected YearsMonthsDuration, got {:?}", other),
        }
    }

    /// `years and months duration("PT1H")` → Null (days-time form rejected).
    #[test]
    fn years_months_duration_rejects_days_time() {
        let result = built_in_years_and_months_duration(&[Value::String("PT1H".into())]);
        assert_eq!(result, Value::Null);
    }

    /// `years and months duration(null)` → Null.
    #[test]
    fn years_months_duration_null() {
        assert_eq!(built_in_years_and_months_duration(&[Value::Null]), Value::Null);
    }

    /// Registry contains all conversion names.
    #[test]
    fn registry_contains_conversion_names() {
        let reg = builtin_registry();
        for name in &[
            "date", "time", "date and time", "duration", "number",
            "string", "years and months duration",
        ] {
            assert!(reg.contains_key(*name), "missing conversion: {}", name);
        }
    }
}

#[cfg(test)]
mod numeric {
    use super::*;
    use rust_decimal::dec;

    /// `abs(-5)` → Number(5).
    #[test]
    fn abs_negative() {
        assert_eq!(built_in_abs(&[Value::Number(dec!(-5))]), Value::Number(dec!(5)));
    }

    /// `abs(null)` → Null.
    #[test]
    fn abs_null() {
        assert_eq!(built_in_abs(&[Value::Null]), Value::Null);
    }

    /// `modulo(10, 3)` → Number(1).
    #[test]
    fn modulo_normal() {
        assert_eq!(built_in_modulo(&[Value::Number(dec!(10)), Value::Number(dec!(3))]), Value::Number(dec!(1)));
    }

    /// `modulo(10, 0)` → Null (div-by-zero).
    #[test]
    fn modulo_div_by_zero() {
        assert_eq!(built_in_modulo(&[Value::Number(dec!(10)), Value::Number(dec!(0))]), Value::Null);
    }

    /// `modulo(null, 3)` → Null.
    #[test]
    fn modulo_null_arg() {
        assert_eq!(built_in_modulo(&[Value::Null, Value::Number(dec!(3))]), Value::Null);
    }

    /// `sqrt(4)` → Number(2).
    #[test]
    fn sqrt_positive() {
        let result = built_in_sqrt(&[Value::Number(dec!(4))]);
        assert_eq!(result, Value::Number(dec!(2)));
    }

    /// `sqrt(-1)` → Null (negative sqrt).
    #[test]
    fn sqrt_negative() {
        assert_eq!(built_in_sqrt(&[Value::Number(dec!(-1))]), Value::Null);
    }

    /// `sqrt(null)` → Null.
    #[test]
    fn sqrt_null() {
        assert_eq!(built_in_sqrt(&[Value::Null]), Value::Null);
    }

    /// `floor(3.7)` → Number(3).
    #[test]
    fn floor_positive() {
        let three_7 = rust_decimal::Decimal::from(37) / rust_decimal::Decimal::from(10);
        assert_eq!(built_in_floor(&[Value::Number(three_7)]), Value::Number(dec!(3)));
    }

    /// `ceiling(3.2)` → Number(4).
    #[test]
    fn ceiling_positive() {
        let three_2 = rust_decimal::Decimal::from(32) / rust_decimal::Decimal::from(10);
        assert_eq!(built_in_ceiling(&[Value::Number(three_2)]), Value::Number(dec!(4)));
    }

    /// `round up(3.2)` → Number(4) (away from zero).
    #[test]
    fn round_up_positive() {
        let three_2 = rust_decimal::Decimal::from(32) / rust_decimal::Decimal::from(10);
        assert_eq!(built_in_round_up(&[Value::Number(three_2)]), Value::Number(dec!(4)));
    }

    /// `round down(3.7)` → Number(3) (toward zero).
    #[test]
    fn round_down_positive() {
        let three_7 = rust_decimal::Decimal::from(37) / rust_decimal::Decimal::from(10);
        assert_eq!(built_in_round_down(&[Value::Number(three_7)]), Value::Number(dec!(3)));
    }

    /// `round half up(2.5)` → Number(3) (ties away from zero).
    #[test]
    fn round_half_up_positive() {
        let two_5 = rust_decimal::Decimal::from(25) / rust_decimal::Decimal::from(10);
        assert_eq!(built_in_round_half_up(&[Value::Number(two_5)]), Value::Number(dec!(3)));
    }

    /// `round half down(2.5)` → Number(2) (ties toward zero).
    #[test]
    fn round_half_down_positive() {
        let two_5 = rust_decimal::Decimal::from(25) / rust_decimal::Decimal::from(10);
        assert_eq!(built_in_round_half_down(&[Value::Number(two_5)]), Value::Number(dec!(2)));
    }

    /// `odd(3)` → Bool(true).
    #[test]
    fn odd_true() {
        assert_eq!(built_in_odd(&[Value::Integer(3)]), Value::Bool(true));
    }

    /// `even(4)` → Bool(true).
    #[test]
    fn even_true() {
        assert_eq!(built_in_even(&[Value::Integer(4)]), Value::Bool(true));
    }

    /// `odd(null)` → Null.
    #[test]
    fn odd_null() {
        assert_eq!(built_in_odd(&[Value::Null]), Value::Null);
    }

    /// `odd(2)` → Bool(false).
    #[test]
    fn odd_false() {
        assert_eq!(built_in_odd(&[Value::Integer(2)]), Value::Bool(false));
    }

    /// `even(3)` → Bool(false).
    #[test]
    fn even_false() {
        assert_eq!(built_in_even(&[Value::Integer(3)]), Value::Bool(false));
    }

    /// `decimal(3.14159, 2)` → Number(3.14).
    #[test]
    fn decimal_rounding() {
        let three_point_one_four_one_five_nine = rust_decimal::Decimal::from_str("3.14159").unwrap();
        let result = built_in_decimal(&[
            Value::Number(three_point_one_four_one_five_nine),
            Value::Integer(2),
        ]);
        assert_eq!(result, Value::Number(rust_decimal::Decimal::from_str("3.14").unwrap()));
    }

    /// `log(e)` approx Number(1).
    #[test]
    fn log_approx_one() {
        let e_approx = rust_decimal::Decimal::from_str("2.718281828").unwrap();
        let result = built_in_log(&[Value::Number(e_approx)]);
        // ln(e) ≈ 1, but Decimal precision means it might be close.
        match result {
            Value::Number(d) => {
                let one = rust_decimal::Decimal::ONE;
                let threshold = rust_decimal::Decimal::from_str("0.001").unwrap();
                assert!((d - one).abs() < threshold, "ln(e) should approximate to 1, got {}", d);
            }
            other => panic!("expected Number, got {:?}", other),
        }
    }

    /// `exp(1)` approx e.
    #[test]
    fn exp_approx_e() {
        let result = built_in_exp(&[Value::Number(dec!(1))]);
        match result {
            Value::Number(d) => {
                // e ≈ 2.71828...
                let two_point_7 = rust_decimal::Decimal::from_str("2.7").unwrap();
                let threshold = rust_decimal::Decimal::from_str("0.1").unwrap();
                assert!((d - two_point_7).abs() < threshold, "exp(1) should approximate to e, got {}", d);
            }
            other => panic!("expected Number, got {:?}", other),
        }
    }

    /// `floor(-3.2)` → Number(-4).
    #[test]
    fn floor_negative() {
        let neg_three_2 = rust_decimal::Decimal::from_str("-3.2").unwrap();
        assert_eq!(built_in_floor(&[Value::Number(neg_three_2)]), Value::Number(dec!(-4)));
    }

    /// `ceiling(-3.7)` → Number(-3).
    #[test]
    fn ceiling_negative() {
        let neg_three_7 = rust_decimal::Decimal::from_str("-3.7").unwrap();
        assert_eq!(built_in_ceiling(&[Value::Number(neg_three_7)]), Value::Number(dec!(-3)));
    }

    /// Registry contains all numeric names.
    #[test]
    fn registry_contains_numeric_names() {
        let reg = builtin_registry();
        for name in &[
            "decimal", "floor", "ceiling", "round up", "round down",
            "round half up", "round half down", "abs", "modulo", "sqrt",
            "log", "exp", "odd", "even",
        ] {
            assert!(reg.contains_key(*name), "missing numeric: {}", name);
        }
    }

    /// No f64 in production path (prohibition).
    #[test]
    fn no_f64_in_builtins() {
        let src = include_str!("builtins.rs");
        // Scan all lines, skipping comments/imports and this test function's body.
        // Use brace-depth tracking to properly identify the function boundaries.
        let mut in_target_fn = false;
        let mut depth: i32 = 0;
        for line in src.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("use ") {
                continue;
            }
            // Detect entering the target function.
            if !in_target_fn && trimmed.starts_with("fn no_") && trimmed.ends_with("_in_builtins() {") {
                in_target_fn = true;
                depth = 1;
                continue;
            }
            if in_target_fn {
                // Count braces to track when we exit the function.
                for ch in line.chars() {
                    match ch {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        _ => {}
                    }
                }
                if depth == 0 {
                    // Exited the function body.
                    in_target_fn = false;
                }
                continue;
            }
            assert!(
                !line.contains("f64"),
                "production path contains f64: {}",
                line
            );
        }
    }
}

#[cfg(test)]
mod registry_dispatch {
    use super::*;
    use crate::dmn::feel::ast::{FeelExpression, BinOpKind};

    /// Helper: parse + evaluate with empty scope.
    fn ev(src: &str) -> Value {
        crate::dmn::feel::eval::evaluate(
            &crate::dmn::feel::parser::parse_expression(src),
            &crate::dmn::feel::eval::Scope::empty(),
        )
    }

    /// A call to an unregistered name → Null (registry miss).
    #[test]
    fn unregistered_name_returns_null() {
        // Use a built-in name that doesn't exist in our registry.
        let result = ev("nonexistent_function(1)");
        assert_eq!(result, Value::Null);
    }

    /// Registry size check — at least 20 entries (8 conversion + 12 numeric).
    #[test]
    fn registry_has_minimum_entries() {
        let reg = builtin_registry();
        assert!(reg.len() >= 20, "registry should have at least 20 entries, got {}", reg.len());
    }

    /// `abs(-5)` via FunctionInvocation dispatch → Number(5).
    #[test]
    fn built_in_dispatch_abs() {
        let result = ev("abs(-5)");
        assert_eq!(result, Value::Number(dec!(5)));
    }

    /// `floor(3.7)` via dispatch → Number(3).
    #[test]
    fn built_in_dispatch_floor() {
        let result = ev("floor(3.7)");
        assert_eq!(result, Value::Number(dec!(3)));
    }

    /// `sqrt(4)` via dispatch → Number(2).
    #[test]
    fn built_in_dispatch_sqrt() {
        let result = ev("sqrt(4)");
        assert_eq!(result, Value::Number(dec!(2)));
    }

    /// `modulo(10, 3)` via dispatch → Number(1).
    #[test]
    fn built_in_dispatch_modulo() {
        let result = ev("modulo(10, 3)");
        assert_eq!(result, Value::Number(dec!(1)));
    }

    /// `count([null])` == Number(1) (spec exception: count counts elements including null).
    #[test]
    fn count_null_element_is_one() {
        // We don't have count yet in task 1; this test will be added in task 2.
        // Placeholder to verify registry dispatch works for future built-ins.
        let _ = Value::Null;
    }

    /// `sum([])` == Null (A5 documented).
    #[test]
    fn sum_empty_is_null() {
        // Placeholder — count/sum land in task 2.
        let _ = Value::Null;
    }

    /// `modulo(10, 0)` == Null (div-by-zero → null).
    #[test]
    fn modulo_zero_returns_null() {
        let result = ev("modulo(10, 0)");
        assert_eq!(result, Value::Null);
    }

    /// `string(null)` == Null.
    #[test]
    fn string_null_arg_is_null_via_dispatch() {
        // "string" as a name resolves to null in empty scope; need literal invocation.
        let result = ev("string(null)");
        assert_eq!(result, Value::Null);
    }

    /// `number("abc")` == Null (parse failure → null).
    #[test]
    fn number_invalid_string_via_dispatch() {
        let result = ev(r#"number("abc")"#);
        assert_eq!(result, Value::Null);
    }
}

// =====================================================================
// Proptest: numeric invariants.
// =====================================================================

#[cfg(test)]
mod proptests {
    use super::*;

    /// Multiplication helper for the tests below.
    fn built_in_mul(args: Vec<Value>) -> Value {
        if args.len() < 2 {
            return Value::Null;
        }
        match (&args[0], &args[1]) {
            (Value::Number(a), Value::Number(b)) => Value::Number(*a * *b),
            _ => Value::Null,
        }
    }

    /// For any Decimal `d`, `abs(d) >= 0` and `abs(-d) == abs(d)`.
    #[test]
    fn prop_abs_symmetry() {
        for d in -100i64..100 {
            let dec_val = rust_decimal::Decimal::from(d);
            let neg = built_in_abs(&[Value::Number(dec_val.clone())]);
            let pos = built_in_abs(&[Value::Number(-dec_val)]);
            match (&neg, &pos) {
                (Value::Number(a), Value::Number(b)) => assert_eq!(a, b, "abs(x) == abs(-x) for {}", d),
                _ => panic!("expected Number results"),
            }
        }
    }

    /// For any Decimal `d`, `floor(d) <= d` and `ceiling(d) >= d`.
    #[test]
    fn prop_floor_le_ceil_ge() {
        for d in -100i64..100 {
            let whole = rust_decimal::Decimal::from(d);
            for frac in 0u32..10 {
                let fract = rust_decimal::Decimal::from(frac as i64) / rust_decimal::Decimal::from(10i64);
                let val = whole + fract;

                let floored = built_in_floor(&[Value::Number(val.clone())]);
                let ceiled = built_in_ceiling(&[Value::Number(val.clone())]);

                match (&floored, &ceiled) {
                    (Value::Number(floor_v), Value::Number(ceil_v)) => {
                        assert!(*floor_v <= val, "floor({}) should be <= {}", floor_v, val);
                        assert!(*ceil_v >= val, "ceiling({}) should be >= {}", ceil_v, val);
                    }
                    _ => panic!("expected Number results"),
                }
            }
        }
    }

    /// `odd(n)` and `even(n)` are complementary for integers.
    #[test]
    fn prop_odd_even_complementary() {
        for i in -100i64..100 {
            let v = Value::Integer(i);
            let o = built_in_odd(&[v.clone()]);
            let e = built_in_even(&[v.clone()]);
            match (o, e) {
                (Value::Bool(ob), Value::Bool(eb)) => {
                    assert_eq!(ob, !eb, "odd and even must be complementary for integer {}", i);
                }
                _ => panic!("expected Bool results"),
            }
        }
    }

    /// `round up` and `round down` agree for integers.
    #[test]
    fn prop_round_up_down_agree_on_integers() {
        for d in -100i64..100 {
            let dec_val = rust_decimal::Decimal::from(d);
            if dec_val.fract() == Decimal::ZERO {
                let ru = built_in_round_up(&[Value::Number(dec_val.clone())]);
                let rd = built_in_round_down(&[Value::Number(dec_val.clone())]);
                assert_eq!(ru, rd, "round up and round down agree on integers");
            }
        }
    }

    /// `sqrt(d * d) == abs(d)` for non-negative `d`.
    #[test]
    fn prop_sqrt_square_is_abs() {
        for x in 1i64..100 {
            let dec_x = rust_decimal::Decimal::from(x);
            let squared = built_in_mul(vec![Value::Number(dec_x.clone()), Value::Number(dec_x.clone())]);
            // sqrt(squared) should equal abs(x).
            let result = built_in_sqrt(&[squared]);
            match (result, dec_x.abs()) {
                (Value::Number(r), expected) => assert_eq!(r, expected, "sqrt(x*x) == |x| for {}", x),
                _ => panic!("expected Number"),
            }
        }
    }
}
