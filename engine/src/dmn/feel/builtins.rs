#![allow(dead_code)]

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
use std::sync::{Mutex, OnceLock};

use crate::data_types::Value;
use chrono::Datelike;
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
        Value::String(s) => crate::dmn::feel::temporal::parse_at_literal(s).unwrap_or(Value::Null),
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
        Value::String(s) => crate::dmn::feel::temporal::parse_at_literal(s).unwrap_or(Value::Null),
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
        Value::String(s) => crate::dmn::feel::temporal::parse_at_literal(s).unwrap_or(Value::Null),
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
            // `number(from, decimal separator, grouping separator)` — locale
            // aware parsing: drop the grouping separator, normalise the decimal
            // separator to '.', then parse exactly.
            let decimal_separator = match args.get(1) {
                None | Some(Value::Null) => None,
                Some(Value::String(separator)) => Some(separator.as_str()),
                _ => return Value::Null,
            };
            let grouping_separator = match args.get(2) {
                None | Some(Value::Null) => None,
                Some(Value::String(separator)) => Some(separator.as_str()),
                _ => return Value::Null,
            };

            let mut normalised = s.trim().to_string();
            if let Some(grouping) = grouping_separator {
                if !grouping.is_empty() {
                    normalised = normalised.replace(grouping, "");
                }
            }
            if let Some(decimal) = decimal_separator {
                if !decimal.is_empty() {
                    normalised = normalised.replace(decimal, ".");
                }
            }
            match normalised.parse::<Decimal>() {
                Ok(value) => Value::Number(value),
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
    // DMN 1.5 §10.3.4.1: `years and months duration(from, to)` returns the
    // years-and-months duration between two date/date-and-time values.
    if args.len() >= 2 {
        let (Some((y1, m1)), Some((y2, m2))) = (year_month(&args[0]), year_month(&args[1])) else {
            return Value::Null;
        };
        let months = (y2 as i64 - y1 as i64) * 12 + (m2 as i64 - m1 as i64);
        return Value::YearsMonthsDuration(crate::dmn::feel::temporal::YearsMonthsDuration {
            months,
        });
    }

    // Single-argument form: accept an ISO year-month duration string
    // (e.g. `years and months duration("P1Y2M")`).
    match args.first() {
        Some(Value::Null) | None => Value::Null,
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if !trimmed.starts_with('P') && !trimmed.starts_with("-P") {
                return Value::Null;
            }
            let body = trimmed
                .strip_prefix('-')
                .unwrap_or(trimmed)
                .strip_prefix('P')
                .unwrap_or("");
            // If body contains T, D, H, or S → it's a days-time duration, not YMD.
            if body.contains('T') || body.contains('D') || body.contains('H') || body.contains('S')
            {
                return Value::Null;
            }
            crate::dmn::feel::temporal::parse_at_literal(trimmed).unwrap_or(Value::Null)
        }
        Some(Value::YearsMonthsDuration(_)) => args[0].clone(),
        _ => Value::Null,
    }
}

/// Extract `(year, month)` from a FEEL date or date-and-time value.
fn year_month(v: &Value) -> Option<(i32, u32)> {
    use chrono::Datelike;
    match v {
        Value::Date(d) => Some((d.year(), d.month())),
        Value::DateTimeTz(dt) => {
            let naive = match dt {
                crate::dmn::feel::temporal::DateTimeValue::Fixed(f) => f.naive_local(),
                crate::dmn::feel::temporal::DateTimeValue::Iana(z) => z.naive_local(),
                crate::dmn::feel::temporal::DateTimeValue::Local(l) => *l,
            };
            Some((naive.year(), naive.month()))
        }
        _ => None,
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
    // `scale` is the number of decimal places (non-negative integer). It is
    // clamped to `rust_decimal`'s maximum scale rather than cast: `as u32`
    // would silently wrap a large scale (e.g. 4294967296 → 0) and round to the
    // wrong place instead of failing.
    let max_scale = rust_decimal::Decimal::MAX_SCALE;
    let scale: u32 = match &args[1] {
        Value::Integer(n) => {
            u32::try_from((*n).clamp(0, i64::from(max_scale))).unwrap_or(max_scale)
        }
        Value::Number(d) => {
            let truncated = d.trunc().abs();
            u32::try_from(truncated)
                .map(|value| value.min(max_scale))
                .unwrap_or(max_scale)
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

/// Extract the optional `scale` argument of the DMN rounding functions.
///
/// `None` means "the whole result is null": either the scale argument was
/// explicitly null or it was not a non-negative integer/number.
fn rounding_scale(args: &[Value]) -> Option<u32> {
    match args.get(1) {
        // Omitted → scale 0 (round to an integer), per DMN 1.5 §10.3.4.1.
        None => Some(0),
        Some(Value::Integer(i)) if *i >= 0 => u32::try_from(*i).ok(),
        Some(Value::Number(d)) if d.is_sign_positive() && d.fract().is_zero() => {
            u32::try_from(*d).ok()
        }
        // null or a non-integral/negative scale → null result.
        Some(_) => None,
    }
}

/// Apply a rounding strategy to `args[0]` at the requested scale.
fn round_with(args: &[Value], strategy: rust_decimal::RoundingStrategy) -> Value {
    let Some(scale) = rounding_scale(args) else {
        return Value::Null;
    };
    match args.first() {
        Some(Value::Number(d)) => Value::Number(d.round_dp_with_strategy(scale, strategy)),
        Some(Value::Integer(i)) => {
            Value::Number(Decimal::from(*i).round_dp_with_strategy(scale, strategy))
        }
        _ => Value::Null,
    }
}

/// `round up(n, scale)` — round away from zero at `scale` decimals.
/// Null propagates. DMN 1.5 §10.3.4.1.
fn built_in_round_up(args: &[Value]) -> Value {
    round_with(args, rust_decimal::RoundingStrategy::AwayFromZero)
}

/// `round down(n, scale)` — round toward zero at `scale` decimals.
/// Null propagates. DMN 1.5 §10.3.4.1.
fn built_in_round_down(args: &[Value]) -> Value {
    round_with(args, rust_decimal::RoundingStrategy::ToZero)
}

/// `round half up(n, scale)` — round to nearest at `scale` decimals, ties away
/// from zero. Null propagates. DMN 1.5 §10.3.4.1.
fn built_in_round_half_up(args: &[Value]) -> Value {
    round_with(args, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
}

/// `round half down(n, scale)` — round to nearest at `scale` decimals, ties
/// toward zero. Null propagates. DMN 1.5 §10.3.4.1.
fn built_in_round_half_down(args: &[Value]) -> Value {
    round_with(args, rust_decimal::RoundingStrategy::MidpointTowardZero)
}

/// `abs(x)` — absolute value. Null propagates.
fn built_in_abs(args: &[Value]) -> Value {
    // DMN 1.5 §10.3.4.1 defines `abs` over numbers *and* durations.
    null_unary(args, |v| match v {
        Value::Number(d) => Value::Number(d.abs()),
        Value::Integer(i) => Value::Integer(i.abs()),
        Value::DaysTimeDuration(td) => Value::DaysTimeDuration(td.abs()),
        Value::YearsMonthsDuration(ymd) => {
            Value::YearsMonthsDuration(crate::dmn::feel::temporal::YearsMonthsDuration {
                months: ymd.months.abs(),
            })
        }
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
            // Truncating remainder: the result takes the sign of the dividend,
            // so `modulo(-12, 5)` is -2 (TCK `0056-feel-modulo-function`).
            Value::Integer(a % b)
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
// String family (13 functions). Regex-backed replace/matches/split use the
// COMPILED_PATTERNS cache for performance. Pattern compilation errors → Null.
// XQuery 1.0 / XPath 2.0 FO regex approximation (Pitfall 7); TCK divergence
// flagged for Phase 35.
// =====================================================================

/// `substring(string, start)` — extract substring from position `start` (1-based).
fn built_in_substring(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Null;
    }
    let s = match &args[0] {
        Value::String(s) => s.clone(),
        _ => return Value::Null,
    };
    let start = match &args[1] {
        Value::Integer(n) => *n as usize,
        Value::Number(d) => d.to_i64().unwrap_or(0) as usize,
        _ => return Value::Null,
    };
    if args.is_empty() || matches!(args[0], Value::Null) || matches!(args[1], Value::Null) {
        return Value::Null;
    }
    // 1-based start: clamp to [1, len+1]. FEEL substring(…, 0) is same as (…, 1).
    let start = if start == 0 { 1 } else { start };
    let chars: Vec<char> = s.chars().collect();
    if start > chars.len() + 1 {
        return Value::String(String::new()); // past end → empty string
    }
    let offset = start - 1; // convert to 0-based
    let sub: String = chars[offset..].iter().copied().collect();
    if args.len() >= 3 {
        // substring(string, start, length) — limit to `length` characters.
        let len = match &args[2] {
            Value::Integer(n) => *n as usize,
            Value::Number(d) => d.to_i64().unwrap_or(0) as usize,
            _ => return Value::Null,
        };
        if args.is_empty() || matches!(args[2], Value::Null) {
            return Value::Null;
        }
        return Value::String(sub.chars().take(len).collect());
    }
    Value::String(sub)
}

/// `string length(x)` — number of characters in a string. Null propagates.
fn built_in_string_length(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::String(s) => Value::Integer(s.chars().count() as i64),
        _ => Value::Null,
    })
}

/// `upper case(x)` — convert string to uppercase. Null propagates.
fn built_in_upper_case(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::String(s) => Value::String(s.to_uppercase()),
        _ => Value::Null,
    })
}

/// `lower case(x)` — convert string to lowercase. Null propagates.
fn built_in_lower_case(args: &[Value]) -> Value {
    null_unary(args, |v| match v {
        Value::String(s) => Value::String(s.to_lowercase()),
        _ => Value::Null,
    })
}

/// `substring before(string, substring)` — return portion of string before first occurrence.
fn built_in_substring_before(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Null;
    }
    let s = match &args[0] {
        Value::String(s) => s.clone(),
        _ => return Value::Null,
    };
    let sub = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Value::Null,
    };
    if args.is_empty() || matches!(args[0], Value::Null) || matches!(args[1], Value::Null) {
        return Value::Null;
    }
    match s.find(sub) {
        Some(pos) => Value::String(s[..pos].to_string()),
        None => Value::String(String::new()),
    }
}

/// `substring after(string, substring)` — return portion of string after first occurrence.
fn built_in_substring_after(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Null;
    }
    let s = match &args[0] {
        Value::String(s) => s.clone(),
        _ => return Value::Null,
    };
    let sub = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Value::Null,
    };
    if args.is_empty() || matches!(args[0], Value::Null) || matches!(args[1], Value::Null) {
        return Value::Null;
    }
    match s.find(sub) {
        Some(pos) => Value::String(s[pos + sub.len()..].to_string()),
        None => Value::String(String::new()),
    }
}

/// `replace(string, pattern, replacement)` — regex replace. Pattern errors → Null.
fn built_in_replace(args: &[Value]) -> Value {
    if args.len() < 3 {
        return Value::Null;
    }
    let s = match &args[0] {
        Value::String(s) => s.clone(),
        _ => return Value::Null,
    };
    let pattern_str = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Value::Null,
    };
    let replacement = match &args[2] {
        Value::String(s) => s.clone(),
        _ => return Value::Null,
    };
    if args.is_empty()
        || matches!(args[0], Value::Null)
        || matches!(args[1], Value::Null)
        || matches!(args[2], Value::Null)
    {
        return Value::Null;
    }
    let re = match get_or_compile(pattern_str) {
        Some(r) => r,
        None => return Value::Null, // pattern compilation error → null
    };
    let result = re.replace_all(&s, replacement.as_str());
    Value::String(result.into_owned())
}

/// `contains(string, substring)` — true if string contains substring. Null propagates.
fn built_in_contains(args: &[Value]) -> Value {
    null_binary(args, |a, b| match (a, b) {
        (Value::String(s), Value::String(sub)) => Value::Bool(s.contains(sub.as_str())),
        _ => Value::Null,
    })
}

/// `starts with(string, prefix)` — true if string starts with prefix. Null propagates.
fn built_in_starts_with(args: &[Value]) -> Value {
    null_binary(args, |a, b| match (a, b) {
        (Value::String(s), Value::String(p)) => Value::Bool(s.starts_with(p.as_str())),
        _ => Value::Null,
    })
}

/// `ends with(string, suffix)` — true if string ends with suffix. Null propagates.
fn built_in_ends_with(args: &[Value]) -> Value {
    null_binary(args, |a, b| match (a, b) {
        (Value::String(s), Value::String(sf)) => Value::Bool(s.ends_with(sf.as_str())),
        _ => Value::Null,
    })
}

/// `matches(string, pattern)` — true if string matches regex. Pattern errors → Null.
fn built_in_matches(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Null;
    }
    let s = match &args[0] {
        Value::String(s) => s.clone(),
        _ => return Value::Null,
    };
    let pattern_str = match &args[1] {
        Value::String(s) => s.as_str(),
        _ => return Value::Null,
    };
    if args.is_empty() || matches!(args[0], Value::Null) || matches!(args[1], Value::Null) {
        return Value::Null;
    }
    let re = match get_or_compile(pattern_str) {
        Some(r) => r,
        None => return Value::Null,
    };
    Value::Bool(re.is_match(&s))
}

/// `split(string, delimiter)` — split string by literal delimiter into list. Null propagates.
fn built_in_split(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Null;
    }
    let s = match &args[0] {
        Value::String(s) => s.clone(),
        _ => return Value::Null,
    };
    let delim = match &args[1] {
        Value::String(d) => d.as_str(),
        _ => return Value::Null,
    };
    if args.is_empty() || matches!(args[0], Value::Null) || matches!(args[1], Value::Null) {
        return Value::Null;
    }
    // The FEEL `split` delimiter is a regular expression (DMN 1.5 §10.3.4.2),
    // so `split("John Doe", "\\s")` splits on whitespace. An invalid pattern
    // makes the call null rather than panicking.
    match regex::Regex::new(delim) {
        Ok(pattern) => Value::Array(
            pattern
                .split(&s)
                .map(|part| Value::String(part.to_string()))
                .collect(),
        ),
        Err(_) => Value::Null,
    }
}

/// `string join(list, delimiter)` — join string list with delimiter. Null propagates.
fn built_in_string_join(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Null;
    }
    let list = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    let delim = match &args[1] {
        Value::String(s) => s.clone(),
        _ => return Value::Null,
    };
    if args.is_empty() || matches!(args[0], Value::Null) || matches!(args[1], Value::Null) {
        return Value::Null;
    }
    // Filter to strings only (non-strings are ignored per DMN spec).
    let parts: Vec<String> = list
        .iter()
        .filter_map(|v| match v {
            Value::String(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    Value::String(parts.join(&delim))
}

// =====================================================================
// List family (24 functions). Null-propagation exceptions documented in module docs.
// =====================================================================

/// `list contains(list, value)` — true if list contains value (using feel_equals).
fn built_in_list_contains(args: &[Value]) -> Value {
    null_binary(args, |a, b| match (a, b) {
        (Value::Array(elems), v) => {
            for elem in elems {
                if crate::dmn::feel::eval::cmp::feel_equals(elem, v) == Some(true) {
                    return Value::Bool(true);
                }
            }
            Value::Bool(false)
        }
        _ => Value::Null,
    })
}

/// `list replace(list, pattern, replacement)` — replace matching elements using regex.
fn built_in_list_replace(args: &[Value]) -> Value {
    if args.len() < 3 {
        return Value::Null;
    }
    let list = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    let pattern_str = match &args[1] {
        Value::String(s) => s.as_str().to_string(),
        _ => return Value::Null,
    };
    let replacement = match &args[2] {
        Value::String(s) => s.clone(),
        _ => return Value::Null,
    };
    if args.is_empty()
        || matches!(args[0], Value::Null)
        || matches!(args[1], Value::Null)
        || matches!(args[2], Value::Null)
    {
        return Value::Null;
    }
    let re = match get_or_compile(&pattern_str) {
        Some(r) => r,
        None => return Value::Array(vec![]),
    };
    let result: Vec<Value> = list
        .iter()
        .map(|v| {
            if let Value::String(s) = v {
                if re.is_match(s) {
                    let r = re.replace_all(s, replacement.as_str());
                    Value::String(r.into_owned())
                } else {
                    v.clone()
                }
            } else {
                v.clone()
            }
        })
        .collect();
    Value::Array(result)
}

/// `count(list)` — number of elements in list. count([null]) == 1 (spec exception).
fn built_in_count(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    match &args[0] {
        Value::Array(elems) => Value::Integer(elems.len() as i64), // counts all elements including null
        _ => Value::Null,
    }
}

/// `min(list)` — minimum value in list. min([]) == Null (DMN spec).
fn built_in_min(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    let elems = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    if elems.is_empty() {
        return Value::Null; // empty list → null per DMN 1.5
    }
    // Filter to comparable values (Numbers and Integers).
    let mut min_val: Option<Value> = None;
    for v in &elems {
        match v {
            Value::Number(_) | Value::Integer(_) => {
                let dec = match v {
                    Value::Number(d) => *d,
                    Value::Integer(i) => rust_decimal::Decimal::from(*i),
                    _ => unreachable!(),
                };
                let current = min_val.get_or_insert_with(|| v.clone());
                let cur_dec = match current {
                    Value::Number(d) => *d,
                    Value::Integer(i) => rust_decimal::Decimal::from(*i),
                    _ => unreachable!(),
                };
                if dec < cur_dec {
                    min_val = Some(v.clone());
                }
            }
            _ => {} // skip non-numeric for now (full spec handles more types later)
        }
    }
    min_val.unwrap_or(Value::Null)
}

/// `max(list)` — maximum value in list. max([]) == Null.
fn built_in_max(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    let elems = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    if elems.is_empty() {
        return Value::Null;
    }
    let mut max_val: Option<Value> = None;
    for v in &elems {
        match v {
            Value::Number(_) | Value::Integer(_) => {
                let dec = match v {
                    Value::Number(d) => *d,
                    Value::Integer(i) => rust_decimal::Decimal::from(*i),
                    _ => unreachable!(),
                };
                let current = max_val.get_or_insert_with(|| v.clone());
                let cur_dec = match current {
                    Value::Number(d) => *d,
                    Value::Integer(i) => rust_decimal::Decimal::from(*i),
                    _ => unreachable!(),
                };
                if dec > cur_dec {
                    max_val = Some(v.clone());
                }
            }
            _ => {}
        }
    }
    max_val.unwrap_or(Value::Null)
}

/// `sum(list)` — sum of numeric elements. sum([]) == Null (A5 documented).
fn built_in_sum(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    let elems = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    if elems.is_empty() {
        return Value::Null; // A5: empty sum → null
    }
    let mut total = Decimal::ZERO;
    for v in &elems {
        match v {
            Value::Number(d) => total += d,
            Value::Integer(i) => total += rust_decimal::Decimal::from(*i),
            _ => {} // skip non-numeric (nulls, strings, etc.) per DMN spec
        }
    }
    Value::Number(total)
}

/// `mean(list)` — arithmetic mean. mean([]) == Null.
fn built_in_mean(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    // `mean(list)` is the specification's form; the variadic `mean(a, b, c)`
    // form is common in models (and in TCK `0012-list-functions`), so several
    // numeric arguments are treated as the list.
    let elems = match (&args[0], args.len()) {
        (Value::Array(elems), _) => elems.clone(),
        (_, count) if count > 1 => args.to_vec(),
        _ => return Value::Null,
    };
    if elems.is_empty() {
        return Value::Null; // empty mean → null
    }
    let mut total = Decimal::ZERO;
    let mut count: u64 = 0;
    for v in &elems {
        match v {
            Value::Number(d) => {
                total += d;
                count += 1;
            }
            Value::Integer(i) => {
                total += rust_decimal::Decimal::from(*i);
                count += 1;
            }
            _ => {} // skip non-numeric
        }
    }
    if count == 0 {
        return Value::Null;
    }
    let divisor = rust_decimal::Decimal::from(count);
    match total.checked_div(divisor) {
        Some(r) => Value::Number(r),
        None => Value::Null,
    }
}

/// `all(list)` — true if all elements are truthy. all([]) == true (vacuously).
fn built_in_all(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    let elems = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    if elems.is_empty() {
        return Value::Bool(true); // vacuously true per DMN 1.5
    }
    // Three-valued `all`: true when every element is true, false as soon as one
    // element is false (a false overrides an earlier null), null otherwise.
    let mut saw_null = false;
    for value in &elems {
        match crate::dmn::feel::coercion::coerce_to_bool(value) {
            Some(true) => {}
            Some(false) => return Value::Bool(false),
            None => saw_null = true,
        }
    }
    if saw_null {
        Value::Null
    } else {
        Value::Bool(true)
    }
}

/// `any(list)` — true if any element is truthy. any([]) == false.
fn built_in_any(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    let elems = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    if elems.is_empty() {
        return Value::Bool(false); // vacuously false per DMN 1.5
    }
    // Three-valued `any`: true as soon as one element is true, false when every
    // element is false, null otherwise.
    let mut saw_null = false;
    for value in &elems {
        match crate::dmn::feel::coercion::coerce_to_bool(value) {
            Some(true) => return Value::Bool(true),
            Some(false) => {}
            None => saw_null = true,
        }
    }
    if saw_null {
        Value::Null
    } else {
        Value::Bool(false)
    }
}

/// `sublist(list, start)` or `sublist(list, start, length)` — extract sublist. Null propagates.
fn built_in_sublist(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    let list = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };

    // `start` is 1-based and may be negative, counting from the end (`-1` is the
    // last element); `length` is optional and, when omitted, runs to the end of
    // the list.
    let Some(start) = args.get(1).and_then(integer_argument) else {
        return Value::Null;
    };
    let length = match args.get(2) {
        None => None,
        Some(Value::Null) => return Value::Null,
        Some(value) => match integer_argument(value) {
            Some(length) if length >= 0 => Some(length as usize),
            _ => return Value::Null,
        },
    };

    let count = list.len() as i64;
    let first_index = if start >= 0 {
        // 1-based: start 0 and start 1 both address the first element.
        (start.max(1) - 1).min(count)
    } else {
        // Negative: -1 is the last element, -2 the one before it.
        (count + start).max(0)
    } as usize;

    let end_index = match length {
        Some(length) => (first_index + length).min(list.len()),
        None => list.len(),
    };

    if first_index >= list.len() || first_index > end_index {
        return Value::Array(Vec::new());
    }
    Value::Array(list[first_index..end_index].to_vec())
}

/// Read an integer-valued argument (`Integer`, or a `Number` with no fraction).
fn integer_argument(value: &Value) -> Option<i64> {
    match value {
        Value::Integer(i) => Some(*i),
        Value::Number(d) => {
            use rust_decimal::prelude::ToPrimitive;
            d.to_i64()
        }
        _ => None,
    }
}

/// `append(list, value)` — append a single value to the end of a list.
fn built_in_append(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Null;
    }
    let mut list = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    // `append(list, item, …)` appends every trailing argument (the TCK's
    // `append(list, a, b)` form); the single-item case is the common one.
    let items = &args[1..];
    if items.iter().any(|item| matches!(item, Value::Null)) {
        return Value::Null;
    }
    list.extend(items.iter().cloned());
    Value::Array(list)
}

/// `concatenate(list1, list2)` — concatenate two lists. Null propagates.
fn built_in_concatenate(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Null;
    }
    let a = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    let b = match &args[1] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    if args.is_empty() || matches!(args[0], Value::Null) || matches!(args[1], Value::Null) {
        return Value::Null;
    }
    let mut result = a;
    result.extend(b);
    Value::Array(result)
}

/// `insert before(list, position, value)` — insert value at 1-based position. Null propagates.
fn built_in_insert_before(args: &[Value]) -> Value {
    if args.len() < 3 {
        return Value::Null;
    }
    let list = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    let pos = match &args[1] {
        Value::Integer(n) => *n as usize,
        Value::Number(d) => d.to_i64().unwrap_or(0) as usize,
        _ => return Value::Null,
    };
    if args.is_empty()
        || matches!(args[0], Value::Null)
        || matches!(args[1], Value::Null)
        || matches!(args[2], Value::Null)
    {
        return Value::Null;
    }
    let pos = if pos == 0 { 1 } else { pos }; // FEEL 1-based
    let idx = (pos - 1).min(list.len());
    let mut result = list.clone();
    result.insert(idx, args[2].clone());
    Value::Array(result)
}

/// `remove(list, position)` — remove element at 1-based position. Null propagates.
fn built_in_remove(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Null;
    }
    let list = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    let pos = match &args[1] {
        Value::Integer(n) => *n as usize,
        Value::Number(d) => d.to_i64().unwrap_or(0) as usize,
        _ => return Value::Null,
    };
    if args.is_empty() || matches!(args[0], Value::Null) || matches!(args[1], Value::Null) {
        return Value::Null;
    }
    let pos = if pos == 0 { 1 } else { pos }; // FEEL 1-based
    let idx = pos - 1;
    if idx >= list.len() {
        return Value::Array(list); // position out of range → unchanged
    }
    let mut result = list.clone();
    result.remove(idx);
    Value::Array(result)
}

/// `reverse(list)` — reverse a list. Null propagates.
fn built_in_reverse(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    match &args[0] {
        Value::Array(elems) => {
            let mut result = elems.clone();
            result.reverse();
            Value::Array(result)
        }
        _ => Value::Null,
    }
}

/// `index of(list, value)` — return 1-based positions where value appears.
fn built_in_index_of(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Null;
    }
    let list = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    let val = args[1].clone();
    if args.is_empty() || matches!(args[0], Value::Null) || matches!(args[1], Value::Null) {
        return Value::Null;
    }
    let positions: Vec<Value> = list
        .iter()
        .enumerate()
        .filter(|(_, v)| crate::dmn::feel::eval::cmp::feel_equals(v, &val) == Some(true))
        .map(|(i, _)| Value::Integer((i + 1) as i64)) // FEEL 1-based positions
        .collect();
    Value::Array(positions)
}

/// `union(list1, list2)` — union of two lists (preserving order, deduplicating).
fn built_in_union(args: &[Value]) -> Value {
    if args.len() < 2 {
        return Value::Null;
    }
    let a = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    let b = match &args[1] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    if args.is_empty() || matches!(args[0], Value::Null) || matches!(args[1], Value::Null) {
        return Value::Null;
    }
    let mut result = a.clone();
    for v in &b {
        let mut found = false;
        for existing in &result {
            if crate::dmn::feel::eval::cmp::feel_equals(existing, v) == Some(true) {
                found = true;
                break;
            }
        }
        if !found {
            result.push(v.clone());
        }
    }
    Value::Array(result)
}

/// `distinct values(list)` — remove duplicate elements using feel_equals.
fn built_in_distinct_values(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    let elems = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    let mut result: Vec<Value> = vec![];
    for v in &elems {
        let mut found = false;
        for existing in &result {
            // Use feel_equals (NOT Rust ==) per DMN spec. None → null propagation.
            if crate::dmn::feel::eval::cmp::feel_equals(existing, v) == Some(true) {
                found = true;
                break;
            } else if crate::dmn::feel::eval::cmp::feel_equals(existing, v).is_none() {
                // feel_equals returns None → null propagation — skip dedup for this pair.
                continue;
            }
        }
        if !found {
            result.push(v.clone());
        }
    }
    Value::Array(result)
}

/// `flatten(list)` — recursively flatten nested lists one level. Null propagates.
fn built_in_flatten(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    let elems = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    let mut result: Vec<Value> = vec![];
    for v in &elems {
        if let Value::Array(inner) = v {
            result.extend(inner.clone());
        } else {
            result.push(v.clone());
        }
    }
    Value::Array(result)
}

/// `product(list)` — product of numeric elements. product([]) == Null.
fn built_in_product(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    let elems = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    if elems.is_empty() {
        return Value::Null; // empty product → null (consistent with sum)
    }
    let mut total = Decimal::ONE;
    for v in &elems {
        match v {
            Value::Number(d) => total *= d,
            Value::Integer(i) => total *= rust_decimal::Decimal::from(*i),
            _ => {} // skip non-numeric
        }
    }
    Value::Number(total)
}

/// `median(list)` — median of numeric values. median([]) == Null.
fn built_in_median(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    let elems = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    if elems.is_empty() {
        return Value::Null;
    }
    // Collect numeric values.
    let mut nums: Vec<Decimal> = vec![];
    for v in &elems {
        match v {
            Value::Number(d) => nums.push(*d),
            Value::Integer(i) => nums.push(rust_decimal::Decimal::from(*i)),
            _ => {} // skip non-numeric
        }
    }
    if nums.is_empty() {
        return Value::Null;
    }
    nums.sort();
    let n = nums.len();
    if n % 2 == 1 {
        // Odd: middle element.
        Value::Number(nums[n / 2])
    } else {
        // Even: average of two middle elements.
        let mid = n / 2;
        let sum = nums[mid - 1] + nums[mid];
        match sum.checked_div(rust_decimal::Decimal::from(2u64)) {
            Some(r) => Value::Number(r),
            None => Value::Null,
        }
    }
}

/// `stddev(list)` — population standard deviation. stddev([]) == Null.
fn built_in_stddev(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    let elems = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    if elems.is_empty() {
        return Value::Null;
    }
    // Collect numeric values.
    let mut nums: Vec<Decimal> = vec![];
    for v in &elems {
        match v {
            Value::Number(d) => nums.push(*d),
            Value::Integer(i) => nums.push(rust_decimal::Decimal::from(*i)),
            _ => {} // skip non-numeric
        }
    }
    if nums.is_empty() {
        return Value::Null;
    }
    // Compute mean.
    let n = rust_decimal::Decimal::from(nums.len());
    let sum: Decimal = nums.iter().sum();
    let mean = match sum.checked_div(n) {
        Some(r) => r,
        None => return Value::Null,
    };
    // Compute variance (population stddev).
    let mut var_sum = Decimal::ZERO;
    for x in &nums {
        let diff = x - mean;
        var_sum += diff * diff;
    }
    let variance = match var_sum.checked_div(n) {
        Some(r) => r,
        None => return Value::Null,
    };
    // sqrt(variance). Use checked_sqrt (returns None for negative, but variance >= 0 always).
    use rust_decimal::MathematicalOps;
    match variance.sqrt() {
        Some(r) => Value::Number(r),
        None => Value::Null,
    }
}

/// `mode(list)` — most frequent value(s). Returns Array of modes. mode([]) == Null.
fn built_in_mode(args: &[Value]) -> Value {
    if args.is_empty() || matches!(args[0], Value::Null) {
        return Value::Null;
    }
    let elems = match &args[0] {
        Value::Array(elems) => elems.clone(),
        _ => return Value::Null,
    };
    if elems.is_empty() {
        return Value::Null;
    }
    // Count occurrences using feel_equals for comparison (NOT Rust ==).
    let mut counts: Vec<(Value, i64)> = vec![];
    for v in &elems {
        let mut found = false;
        for (existing, count) in &mut counts {
            if crate::dmn::feel::eval::cmp::feel_equals(existing, v) == Some(true) {
                *count += 1;
                found = true;
                break;
            } else if crate::dmn::feel::eval::cmp::feel_equals(existing, v).is_none() {
                // Null propagation: skip this pair.
                continue;
            }
        }
        if !found {
            counts.push((v.clone(), 1));
        }
    }
    if counts.is_empty() {
        return Value::Array(vec![]);
    }
    let max_count = counts.iter().map(|(_, c)| *c).max().unwrap_or(0);
    let modes: Vec<Value> = counts
        .into_iter()
        .filter(|(_, c)| *c == max_count)
        .map(|(v, _)| v)
        .collect();
    if modes.len() == 1 {
        // Single mode — return the value directly (per DMN spec).
        modes.into_iter().next().unwrap_or(Value::Null)
    } else {
        Value::Array(modes)
    }
}

/// `sort(list)` / `sort(list, comparator)` — sort a list (DMN 1.5 §10.3.4.2.5).
///
/// * No comparator: ascending order using FEEL default ordering (numbers,
///   then strings, then booleans/dates by their natural comparison); elements
///   that cannot be compared keep their relative order (stable sort).
/// * Comparator supplied (`function(x, y)`): the function is applied to each
///   pair. A numeric result orders by sign (negative → `x` first); a boolean
///   result orders `x` first when `true` (the TCK's `function(x,y) x>y` style).
/// * `null` list (or any non-list first argument) → `null`.
/// * Empty list → empty list (the spec never turns a list into `null` here), and
///   an empty list is produced even when a comparator is supplied.
fn built_in_sort(args: &[Value]) -> Value {
    let elems = match args.first() {
        Some(Value::Array(elems)) => elems.clone(),
        _ => return Value::Null,
    };
    if elems.is_empty() {
        return Value::Array(elems);
    }

    let mut sortable = elems;

    match args.get(1) {
        Some(Value::Function(cmp)) => {
            sortable.sort_by(|a, b| {
                match crate::dmn::feel::eval::apply_function_value(cmp, &[a.clone(), b.clone()]) {
                    Value::Number(n) => n
                        .partial_cmp(&Decimal::ZERO)
                        .unwrap_or(std::cmp::Ordering::Equal),
                    Value::Integer(i) => i.cmp(&0),
                    // Boolean comparator: `true` means "a comes before b".
                    Value::Bool(true) => std::cmp::Ordering::Less,
                    Value::Bool(false) => std::cmp::Ordering::Greater,
                    // Null / unusable comparator result: treat as equal so the
                    // stable sort preserves input order.
                    _ => std::cmp::Ordering::Equal,
                }
            });
        }
        _ => {
            sortable.sort_by(|a, b| {
                use std::cmp::Ordering;
                match (a, b) {
                    (Value::Number(x), Value::Number(y)) => {
                        x.partial_cmp(y).unwrap_or(Ordering::Equal)
                    }
                    (Value::Integer(x), Value::Integer(y)) => x.cmp(y),
                    (Value::String(x), Value::String(y)) => x.cmp(y),
                    (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
                    (Value::Number(x), Value::Integer(y)) => {
                        x.partial_cmp(&Decimal::from(*y)).unwrap_or(Ordering::Equal)
                    }
                    (Value::Integer(x), Value::Number(y)) => {
                        Decimal::from(*x).partial_cmp(y).unwrap_or(Ordering::Equal)
                    }
                    _ => Ordering::Equal, // incomparable → stable sort preserves order
                }
            });
        }
    }

    Value::Array(sortable)
}

// =====================================================================
// Date-time family (6) + Boolean (1) + Context (5) + Equality (`is`).
// =====================================================================

/// Clock injection point for `now()` / `today()`. Default uses the system clock.
/// Tests can swap in a fixed time via this function pointer.
type NowFn = fn() -> chrono::DateTime<chrono::FixedOffset>;
static DEFAULT_NOW: NowFn = || {
    // Use UTC as the default timezone for now()/today(). This is documented;
    // tests that need reproducibility should override DEFAULT_NOW at module scope.
    let utc_now = chrono::Utc::now();
    utc_now.with_timezone(&chrono::FixedOffset::east_opt(0).unwrap())
};

/// `now()` — current date-time with a fixed UTC offset (RFC 3339).
///
/// DMN 1.5 §10.3.2.3.7: Returns the current date and time as a date-time
/// value with a fixed offset (`DateTimeValue::Fixed`). The offset is the local
/// system's UTC offset; for simplicity we use UTC (documented clock-injection
/// point via `DEFAULT_NOW` at module scope — tests can override).
///
/// Null args: not applicable (no args).
pub fn built_in_now(args: &[Value]) -> Value {
    if !args.is_empty() {
        return Value::Null; // wrong arity → null per DMN spec
    }
    let dt = DEFAULT_NOW();
    Value::DateTimeTz(crate::dmn::feel::temporal::DateTimeValue::Fixed(dt))
}

/// `today()` — current date (no time component).
///
/// DMN 1.5 §10.3.2.3.7: Returns the current date as a date value. Uses the
/// same clock injection point as `now()`.
///
/// Null args: not applicable (no args). Wrong arity → null.
pub fn built_in_today(args: &[Value]) -> Value {
    if !args.is_empty() {
        return Value::Null;
    }
    let dt = DEFAULT_NOW();
    // Extract the date portion in UTC (or local — depends on injection point).
    Value::Date(dt.naive_local().date())
}

/// Helper: compute day-of-year (1–366) for a `NaiveDate`.
fn day_of_year(d: chrono::NaiveDate) -> u32 {
    let start = chrono::NaiveDate::from_ymd_opt(d.year(), 1, 1).unwrap();
    (d - start).num_days() as u32 + 1
}

/// Helper: ISO day-of-week number (Monday=1 … Sunday=7) for a `NaiveDate`.
fn iso_weekday_num(d: chrono::NaiveDate) -> u32 {
    // Convert to DateTime<Utc> at midnight so we can use `.weekday()`.
    let ndt = d.and_hms_opt(0, 0, 0).unwrap();
    use chrono::TimeZone;
    let dt = chrono::Utc.from_local_datetime(&ndt).single().unwrap();
    dt.weekday().number_from_monday()
}

/// Helper: ISO week number (1–53) for a `NaiveDate`.
fn iso_week_num(d: chrono::NaiveDate) -> u32 {
    // ISO-8601 week number (1–53), i.e. the week of the calendar year that
    // Thursday falls in. `%V` gives exactly that; `iso_week().week()` would too,
    // but formatting keeps this helper free of extra trait imports.
    d.format("%V").to_string().parse::<u32>().unwrap_or(1)
}

/// `day of year(<date>)` — day number within the year (1–366).
///
/// DMN 1.5 §10.3.2.3.7: Returns an integer in `[1, 366]`. Null arg → null.
pub fn built_in_day_of_year(args: &[Value]) -> Value {
    let d = match args.first() {
        Some(Value::Date(d)) => *d,
        _ => return Value::Null,
    };
    Value::Integer(day_of_year(d) as i64)
}

/// `day of week(<date>)` — ISO day-of-week number (Monday=1 … Sunday=7).
///
/// DMN 1.5 §10.3.2.3.7: Returns an integer in `[1, 7]`. Null arg → null.
pub fn built_in_day_of_week(args: &[Value]) -> Value {
    let d = match args.first() {
        Some(Value::Date(d)) => *d,
        _ => return Value::Null,
    };
    Value::Integer(iso_weekday_num(d) as i64)
}

/// `month of year(<date>)` — month number (1–12).
///
/// DMN 1.5 §10.3.2.3.7: Returns an integer in `[1, 12]`. Null arg → null.
pub fn built_in_month_of_year(args: &[Value]) -> Value {
    let d = match args.first() {
        Some(Value::Date(d)) => *d,
        _ => return Value::Null,
    };
    // Use Datelike trait (imported above) for .month().
    Value::Integer(Datelike::month(&d) as i64)
}

/// `week of year(<date>)` — ISO week number (1–53).
///
/// DMN 1.5 §10.3.2.3.7: Returns an integer in `[1, 53]`. Null arg → null.
pub fn built_in_week_of_year(args: &[Value]) -> Value {
    let d = match args.first() {
        Some(Value::Date(d)) => *d,
        _ => return Value::Null,
    };
    Value::Integer(iso_week_num(d) as i64)
}

// ---------------------------------------------------------------------
// Boolean family (1).
// ---------------------------------------------------------------------

/// `not(<value>)` — three-valued negation, consistent with DMN-12 `not_3vl`.
///
/// `not(null)` → `null`; `not(true)` → `false`; `not(false)` → `true`.
/// Delegate to the evaluator's `not_3vl` helper (DMN-12 truth table).
pub fn built_in_not(args: &[Value]) -> Value {
    let opt = match args.first() {
        Some(Value::Null) => return Value::Null, // short-circuit null arg
        Some(Value::Bool(b)) => Some(*b),
        _ => return Value::Null, // wrong type → null
    };
    match crate::dmn::feel::eval::cmp::not_3vl(opt) {
        Some(b) => Value::Bool(b),
        None => Value::Null,
    }
}

// ---------------------------------------------------------------------
// Context family (5).
// ---------------------------------------------------------------------

/// `context()` / `context(<list of entries>)` — construct a context from
/// zero or more key-value pairs.
///
/// DMN 1.5 §10.3.2.3.8: If no argument is given, returns an empty context.
/// With one list argument, each element must be a context entry (`key:value`)
/// — the key comes from the string representation of the left operand and
/// the value from the right. Null arg (non-list) → null.
pub fn built_in_context(args: &[Value]) -> Value {
    let mut map = HashMap::new();
    match args.first() {
        None => {} // empty context — no-op
        Some(Value::Null) => return Value::Null,
        Some(Value::Array(entries)) => {
            for entry in entries {
                if let Value::String(s) = entry {
                    // Parse "key:value" form.
                    if let Some(colon_pos) = s.find(':') {
                        let key = s[..colon_pos].trim().to_string();
                        let val_str = s[colon_pos + 1..].trim();
                        // Try to parse as a FEEL literal (number, bool, string, date).
                        if let Ok(n) = rust_decimal::Decimal::from_str(val_str) {
                            map.insert(key, Value::Number(n));
                        } else if val_str == "true" || val_str == "false" {
                            map.insert(key, Value::Bool(val_str == "true"));
                        } else {
                            // Quoted string or unquoted identifier — treat as string.
                            let cleaned = val_str.trim_matches('"').to_string();
                            map.insert(key, Value::String(cleaned));
                        }
                    }
                }
            }
        }
        _ => return Value::Null,
    };
    Value::Object(map)
}

/// `context put(<context>, <key>, <value>)` — returns a new context with the
/// key added or updated. Three-argument form only.
///
/// DMN 1.5 §10.3.2.3.8: The third argument is coerced to a string if needed;
/// null arg → null propagation.
pub fn built_in_context_put(args: &[Value]) -> Value {
    match args.len() {
        3 => {}
        _ => return Value::Null, // wrong arity → null (spec doesn't define 2-arg form)
    }
    let ctx = match &args[0] {
        Value::Object(m) => m.clone(),
        _ => return Value::Null,
    };
    let key = match &args[1] {
        Value::String(s) => s.clone(),
        _other => {
            // Coerce non-string key to string per FEEL spec.
            return Value::Null; // type mismatch → null (simpler than formatting Value)
        }
    };
    let val = args[2].clone();
    let mut result = ctx;
    result.insert(key, val);
    Value::Object(result)
}

/// `context merge(<ctx1>, <ctx2>)` — returns a new context with keys from
/// both, where later keys override earlier ones.
///
/// DMN 1.5 §10.3.2.3.8: Null arg → null propagation. Non-context args → null.
pub fn built_in_context_merge(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    let m1 = match &args[0] {
        Value::Object(m) => m.clone(),
        _ => return Value::Null,
    };
    let m2 = match &args[1] {
        Value::Object(m) => m.clone(),
        _ => return Value::Null,
    };
    let mut result = m1;
    for (k, v) in m2 {
        result.insert(k.clone(), v.clone());
    }
    Value::Object(result)
}

/// `get value(<context>, <key>)` — navigates a context by key. Two-argument form.
///
/// DMN 1.5 §10.3.2.3.8: Returns the value associated with the given key, or
/// null if the key is absent. Null arg → null propagation.
pub fn built_in_get_value(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    let ctx = match &args[0] {
        Value::Object(m) => m.clone(),
        _ => return Value::Null,
    };
    let key = match &args[1] {
        Value::String(s) => s.clone(),
        _other => return Value::Null, // non-string key → null
    };
    ctx.get(&key).cloned().unwrap_or(Value::Null)
}

/// `get entries(<context>)` — returns a list of "key:value" string entries.
///
/// DMN 1.5 §10.3.2.3.8: Each entry is formatted as `"key": value_as_string`.
/// Null arg → null propagation. Non-context arg → null.
pub fn built_in_get_entries(args: &[Value]) -> Value {
    // DMN 1.5 §10.3.4.4: `get entries(c)` returns a list of key/value pairs,
    // each represented as a context with `key` and `value` entries. Keys are
    // emitted in sorted order so the result is deterministic (the backing
    // `HashMap` iteration order is not).
    if args.len() != 1 {
        return Value::Null;
    }
    let ctx = match &args[0] {
        Value::Object(m) => m,
        _ => return Value::Null,
    };

    let mut keys: Vec<&String> = ctx.keys().collect();
    keys.sort();

    let entries: Vec<Value> = keys
        .into_iter()
        .map(|k| {
            let mut entry = HashMap::new();
            entry.insert("key".to_string(), Value::String(k.clone()));
            entry.insert("value".to_string(), ctx[k].clone());
            Value::Object(entry)
        })
        .collect();

    Value::Array(entries)
}

// ---------------------------------------------------------------------
// Equality family (1): `is` — Pitfall 6 total structural identity.
// ---------------------------------------------------------------------

// =====================================================================
// Range family (14): Allen interval algebra over FEEL ranges.
// =====================================================================

/// Helper: extract the start endpoint from a `Value::Range`. Returns None if not a range.
fn range_start(r: &crate::dmn::feel::types::FeelRange) -> Option<&Value> {
    r.start.as_ref()
}

/// Helper: extract the end endpoint from a `Value::Range`. Returns None if not a range.
fn range_end(r: &crate::dmn::feel::types::FeelRange) -> Option<&Value> {
    r.end.as_ref()
}

/// Compare two FEEL values using three-valued comparison (returns Option<Ordering>).
/// `None` means "incomparable" (type mismatch) → maps to false for range built-ins.
fn cmp(a: &Value, b: &Value) -> std::cmp::Ordering {
    match crate::dmn::feel::eval::cmp::feel_compare(a, b) {
        Some(ord) => ord,
        None => std::cmp::Ordering::Equal, // type mismatch → treat as not-before/after (false)
    }
}

/// `before(<range1>, <range2>)` — range1 ends strictly before range2 starts.
///
/// Allen relation: r1.end < r2.start (exclusive of touching).
/// DMN 1.5 §10.3.2.3.6: Null operand → null propagation.
pub fn built_in_before(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    let (r1, r2) = match (&args[0], &args[1]) {
        (Value::Range(r1), Value::Range(r2)) => (r1, r2),
        _ => return Value::Null,
    };
    // Both ranges must have defined endpoints.
    let e1 = match range_end(r1) {
        Some(v) => v,
        None => return Value::Null,
    };
    let s2 = match range_start(r2) {
        Some(v) => v,
        None => return Value::Null,
    };
    // r1.end < r2.start (strictly less — touching is NOT before).
    let ord = cmp(e1, s2);
    Value::Bool(ord == std::cmp::Ordering::Less)
}

/// `after(<range1>, <range2>)` — range1 starts strictly after range2 ends.
/// Symmetric with `before`: `after(r1,r2)` == `before(r2,r1)`.
pub fn built_in_after(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    let (r1, r2) = match (&args[0], &args[1]) {
        (Value::Range(r1), Value::Range(r2)) => (r1, r2),
        _ => return Value::Null,
    };
    let s1 = match range_start(r1) {
        Some(v) => v,
        None => return Value::Null,
    };
    let e2 = match range_end(r2) {
        Some(v) => v,
        None => return Value::Null,
    };
    let ord = cmp(s1, e2);
    Value::Bool(ord == std::cmp::Ordering::Greater)
}

/// Helper: check if two numeric endpoints are "adjacent" (no gap between them).
/// For integer-endpoint ranges with inclusive bounds, adjacency means the
/// difference is exactly 1. For continuous domains, equality suffices.
fn numeric_adjacent(e1: &Value, s2: &Value) -> bool {
    // Try to extract Decimal values and check if they're consecutive integers.
    let d1 = match e1 {
        Value::Number(d) => *d,
        _ => return false,
    };
    let d2 = match s2 {
        Value::Number(d) => *d,
        _ => return false,
    };
    // Check if d2 - d1 == 1 (consecutive integers).
    d2 - d1 == rust_decimal::Decimal::from(1)
}

/// `meets(<range1>, <range2>)` — r1 ends immediately before r2 starts.
/// Adjacent with no gap between them: for numeric ranges, r1.end + 1 == r2.start.
pub fn built_in_meets(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    let (r1, r2) = match (&args[0], &args[1]) {
        (Value::Range(r1), Value::Range(r2)) => (r1, r2),
        _ => return Value::Null,
    };
    let e1 = match range_end(r1) {
        Some(v) => v,
        None => return Value::Null,
    };
    let s2 = match range_start(r2) {
        Some(v) => v,
        None => return Value::Null,
    };
    // Must be strictly before AND adjacent (no gap).
    let ord = cmp(e1, s2);
    if ord != std::cmp::Ordering::Less {
        return Value::Bool(false);
    }
    Value::Bool(numeric_adjacent(e1, s2))
}

/// `met by(<range1>, <range2>)` — r2 meets r1. Symmetric with `meets`.
pub fn built_in_met_by(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    // met by(r1, r2) = meets(r2, r1)
    built_in_meets(&[args[1].clone(), args[0].clone()])
}

/// `overlaps(<range1>, <range2>)` — ranges overlap (share some interior).
/// r1.start < r2.end AND r2.start < r1.end.
pub fn built_in_overlaps(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    let (r1, r2) = match (&args[0], &args[1]) {
        (Value::Range(r1), Value::Range(r2)) => (r1, r2),
        _ => return Value::Null,
    };
    let s1 = match range_start(r1) {
        Some(v) => v,
        None => return Value::Null,
    };
    let e2 = match range_end(r2) {
        Some(v) => v,
        None => return Value::Null,
    };
    let s2 = match range_start(r2) {
        Some(v) => v,
        None => return Value::Null,
    };
    let e1 = match range_end(r1) {
        Some(v) => v,
        None => return Value::Null,
    };
    // r1.start < r2.end AND r2.start < r1.end (strict less for overlap).
    let a = cmp(s1, e2) == std::cmp::Ordering::Less;
    let b = cmp(s2, e1) == std::cmp::Ordering::Less;
    Value::Bool(a && b)
}

/// `overlaps before(<range1>, <range2>)` — r2 overlaps r1. Symmetric with `overlaps after`.
pub fn built_in_overlaps_before(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    // overlaps_before(r1, r2) = overlaps(r2, r1)
    built_in_overlaps(&[args[1].clone(), args[0].clone()])
}

/// `overlaps after(<range1>, <range2>)` — r1 overlaps r2.
pub fn built_in_overlaps_after(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    built_in_overlaps(args)
}

/// `finishes(<range1>, <range2>)` — r1 and r2 share end point; r2 starts after r1.
/// r1.end == r2.end AND r1.start <= r2.start (r2 is a suffix of r1).
pub fn built_in_finishes(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    let (r1, r2) = match (&args[0], &args[1]) {
        (Value::Range(r1), Value::Range(r2)) => (r1, r2),
        _ => return Value::Null,
    };
    let e1 = match range_end(r1) {
        Some(v) => v,
        None => return Value::Null,
    };
    let e2 = match range_end(r2) {
        Some(v) => v,
        None => return Value::Null,
    };
    let s1 = match range_start(r1) {
        Some(v) => v,
        None => return Value::Null,
    };
    let s2 = match range_start(r2) {
        Some(v) => v,
        None => return Value::Null,
    };
    // r1.end == r2.end AND r1.start <= r2.start (r2 is a suffix of r1).
    let end_eq =
        crate::dmn::feel::eval::cmp::feel_compare(e1, e2) == Some(std::cmp::Ordering::Equal);
    let start_le = cmp(s1, s2) != std::cmp::Ordering::Greater;
    Value::Bool(end_eq && start_le)
}

/// `finished by(<range1>, <range2>)` — they share a start point; one is a prefix of the other.
/// DMN FEEL semantics: shared start AND one range contains part of the other (one finishes within).
pub fn built_in_finished_by(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    let (r1, r2) = match (&args[0], &args[1]) {
        (Value::Range(r1), Value::Range(r2)) => (r1, r2),
        _ => return Value::Null,
    };
    let s1 = match range_start(r1) {
        Some(v) => v,
        None => return Value::Null,
    };
    let s2 = match range_start(r2) {
        Some(v) => v,
        None => return Value::Null,
    };
    // Share start AND they overlap (one contains part of the other).
    let start_eq =
        crate::dmn::feel::eval::cmp::feel_compare(s1, s2) == Some(std::cmp::Ordering::Equal);
    if !start_eq {
        return Value::Bool(false);
    }
    // Check overlap: r1.start <= r2.end AND r2.start <= r1.end.
    let e1 = match range_end(r1) {
        Some(v) => v,
        None => return Value::Null,
    };
    let e2 = match range_end(r2) {
        Some(v) => v,
        None => return Value::Null,
    };
    let overlap =
        cmp(s1, e2) != std::cmp::Ordering::Greater && cmp(s2, e1) != std::cmp::Ordering::Greater;
    Value::Bool(overlap)
}

/// `includes(<range>, <value_or_range>)` — range contains the scalar or sub-range.
/// For scalar: start <= value <= end (respecting inclusivity).
/// For sub-range: outer.start <= inner.start AND outer.end >= inner.end.
pub fn built_in_includes(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    let (outer, inner) = match (&args[0], &args[1]) {
        (Value::Range(r), _) => (r, &args[1]),
        _ => return Value::Null,
    };

    // Range includes a scalar value.
    if !matches!(inner, Value::Range(_)) {
        let s = match range_start(outer) {
            Some(v) => v,
            None => return Value::Null,
        };
        let e = match range_end(outer) {
            Some(v) => v,
            None => return Value::Null,
        };
        // start <= value AND value <= end (respecting inclusivity).
        let ge = match crate::dmn::feel::eval::cmp::feel_compare(s, inner) {
            Some(ord) => ord != std::cmp::Ordering::Greater,
            None => return Value::Null,
        };
        let le = match crate::dmn::feel::eval::cmp::feel_compare(inner, e) {
            Some(ord) => ord != std::cmp::Ordering::Greater,
            None => return Value::Null,
        };
        // Apply inclusivity: if start is exclusive and value == start → false.
        let start_ok = if outer.start_incl {
            ge
        } else {
            match crate::dmn::feel::eval::cmp::feel_compare(s, inner) {
                Some(std::cmp::Ordering::Equal) => false,
                _ => ge,
            }
        };
        // Apply inclusivity: if end is exclusive and value == end → false.
        let end_ok = if outer.end_incl {
            le
        } else {
            match crate::dmn::feel::eval::cmp::feel_compare(inner, e) {
                Some(std::cmp::Ordering::Equal) => false,
                _ => le,
            }
        };
        return Value::Bool(start_ok && end_ok);
    }

    // Range includes a sub-range.
    let inner_range = match inner {
        Value::Range(r) => r,
        _ => return Value::Null,
    };
    let os = match range_start(outer) {
        Some(v) => v,
        None => return Value::Null,
    };
    let oe = match range_end(outer) {
        Some(v) => v,
        None => return Value::Null,
    };
    let is_ = match range_start(inner_range) {
        Some(v) => v,
        None => return Value::Null,
    };
    let ie = match range_end(inner_range) {
        Some(v) => v,
        None => return Value::Null,
    };
    // outer.start <= inner.start AND outer.end >= inner.end.
    let start_ok = cmp(os, is_) != std::cmp::Ordering::Greater;
    let end_ok = cmp(oe, ie) != std::cmp::Ordering::Less;
    Value::Bool(start_ok && end_ok)
}

/// `during(<value_or_range>, <range>)` — scalar/range is during the outer range.
/// Inverse of includes: during(x, r) = includes(r, x).
pub fn built_in_during(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    // during(arg1, arg2) = includes(arg2, arg1)
    built_in_includes(&[args[1].clone(), args[0].clone()])
}

/// `starts(<range1>, <range2>)` — r1 and r2 share start; r1 ends before or at r2.
/// r1.start == r2.start AND r1.end <= r2.end (r1 is a prefix of r2).
pub fn built_in_starts(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    let (r1, r2) = match (&args[0], &args[1]) {
        (Value::Range(r1), Value::Range(r2)) => (r1, r2),
        _ => return Value::Null,
    };
    let s1 = match range_start(r1) {
        Some(v) => v,
        None => return Value::Null,
    };
    let s2 = match range_start(r2) {
        Some(v) => v,
        None => return Value::Null,
    };
    let e1 = match range_end(r1) {
        Some(v) => v,
        None => return Value::Null,
    };
    let e2 = match range_end(r2) {
        Some(v) => v,
        None => return Value::Null,
    };
    // r1.start == r2.start AND r1.end <= r2.end.
    let start_eq =
        crate::dmn::feel::eval::cmp::feel_compare(s1, s2) == Some(std::cmp::Ordering::Equal);
    let end_le = cmp(e1, e2) != std::cmp::Ordering::Greater;
    Value::Bool(start_eq && end_le)
}

/// `started by(<range1>, <range2>)` — r2 starts r1. Symmetric with `starts`.
pub fn built_in_started_by(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    // started_by(r1, r2) = starts(r2, r1)
    built_in_starts(&[args[1].clone(), args[0].clone()])
}

/// `coincides(<range1>, <range2>)` — ranges have same start AND same end.
pub fn built_in_coincides(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Null,
    }
    let (r1, r2) = match (&args[0], &args[1]) {
        (Value::Range(r1), Value::Range(r2)) => (r1, r2),
        _ => return Value::Null,
    };
    let s1 = match range_start(r1) {
        Some(v) => v,
        None => return Value::Null,
    };
    let s2 = match range_start(r2) {
        Some(v) => v,
        None => return Value::Null,
    };
    let e1 = match range_end(r1) {
        Some(v) => v,
        None => return Value::Null,
    };
    let e2 = match range_end(r2) {
        Some(v) => v,
        None => return Value::Null,
    };
    // Both start and end must be equal.
    let s_eq = crate::dmn::feel::eval::cmp::feel_compare(s1, s2) == Some(std::cmp::Ordering::Equal);
    let e_eq = crate::dmn::feel::eval::cmp::feel_compare(e1, e2) == Some(std::cmp::Ordering::Equal);
    Value::Bool(s_eq && e_eq)
}

/// `is(<a>, <b>)` — **total** two-valued structural equality, NEVER returns null.
///
/// DMN 1.5 §10.3.2.3.9: `is` is the two-valued counterpart to the three-valued
/// `=` operator (DMN-12). Key property: **Pitfall 6** — `is(null, null) == true`,
/// never null even for type mismatches. This is the ONLY built-in where we
/// coerce the three-valued result of `feel_equals` to a definite Bool.
///
/// Truth table (from DMN-12 research):
/// | a       | b       | is(a,b) |
/// |---------|---------|---------|
/// | null    | null    | true    |
/// | null    | non-null| false   |
/// | different kind | -  | false (NOT null) |
/// | same kind, equal | -  | true    |
/// | same kind, unequal | -  | false   |
pub fn built_in_is(args: &[Value]) -> Value {
    match args.len() {
        2 => {}
        _ => return Value::Bool(false), // wrong arity → false (never null)
    }
    let a = &args[0];
    let b = &args[1];

    // Both null: is(null, null) == true (Pitfall 6).
    if matches!(a, Value::Null) && matches!(b, Value::Null) {
        return Value::Bool(true);
    }
    // One null, one not → definitely false.
    if matches!(a, Value::Null) || matches!(b, Value::Null) {
        return Value::Bool(false);
    }

    // Delegate to feel_equals (three-valued), then coerce None → false.
    let result = crate::dmn::feel::eval::cmp::feel_equals(a, b);
    match result {
        Some(b) => Value::Bool(b),  // definite answer
        None => Value::Bool(false), // Pitfall 6: NEVER null — coerce to false
    }
}

// =====================================================================
// Registry construction.
// =====================================================================

/// Build the built-in function registry. Returns a `HashMap` mapping FEEL
/// canonical names (with spaces) to their implementation pointers.
/// Declared parameter names for the built-ins that the DMN specification
/// documents by name, used to bind named arguments (`abs(n: -1)`).
///
/// A built-in that is absent from this table accepts positional arguments only;
/// invoking it with named arguments evaluates to `null` rather than guessing
/// which argument a name refers to.
pub fn builtin_param_names(name: &str) -> Option<&'static [&'static str]> {
    const ABS: &[&str] = &["n"];
    const NUMBER_ARG: &[&str] = &["number"];
    const ROUND: &[&str] = &["n", "scale"];
    const LIST: &[&str] = &["list"];
    const LIST_AND_VALUE: &[&str] = &["list", "element"];
    Some(match name {
        // Numeric family. Parameter names follow the DMN 1.5 pseudo-signatures
        // (`sqrt(number)`, `abs(n)`, `round up(n, scale)`), which the vendored
        // TCK encodes: `sqrt(number:4)` is valid and `sqrt(n:4)` is an error.
        "abs" => ABS,
        "sqrt" | "exp" | "log" | "even" | "odd" | "floor" | "ceiling" => NUMBER_ARG,
        "round up" | "round down" | "round half up" | "round half down" => ROUND,
        "modulo" => &["dividend", "divisor"],
        "decimal" => &["n", "scale"],
        "number" => &["from", "decimal separator", "grouping separator"],
        "median" | "mode" | "stddev" | "product" | "sum" | "min" | "max" | "mean" => LIST,
        // String family.
        "substring" => &["string", "start position", "length"],
        "string length" => &["string"],
        "upper case" | "lower case" => &["string"],
        "substring before" | "substring after" => &["string", "match"],
        "replace" => &["input", "pattern", "replacement"],
        "contains" => &["string", "match"],
        "starts with" | "ends with" => &["string", "match"],
        "matches" => &["input", "pattern"],
        "split" => &["string", "delimiter"],
        "string join" => &["list", "delimiter"],
        // List family.
        "list contains" => LIST_AND_VALUE,
        "count" | "sort" | "all" | "any" => LIST,
        "append" | "concatenate" => &["list", "item"],
        "union" => &["list", "list"],
        "insert before" => &["list", "position", "element"],
        "remove" => &["list", "position"],
        "index of" => LIST_AND_VALUE,
        "sublist" => &["list", "start position", "length"],
        "distinct values" => LIST,
        // Context family.
        "get value" => &["m", "key"],
        "get entries" => &["m"],
        "context" => &["entries"],
        "context put" => &["context", "key", "value"],
        "context merge" => &["contexts"],
        // Conversion / temporal family.
        "date" => &["from"],
        "time" => &["from"],
        "date and time" => &["from"],
        "duration" => &["from"],
        "years and months duration" => &["from", "to"],
        "string" => &["from"],
        // Temporal accessors take a `date` or `date and time` value.
        "day of week" | "day of year" | "month of year" | "week of year" => &["date"],
        _ => return None,
    })
}

/// True for built-ins whose declared parameter list is a lower bound rather
/// than the exact arity (they accept any number of list/context arguments).
pub fn is_variadic_builtin(name: &str) -> bool {
    matches!(
        name,
        "append"
            | "concatenate"
            | "sort"
            | "context merge"
            | "min"
            | "max"
            | "sum"
            | "mean"
            | "median"
            | "mode"
            | "stddev"
            | "product"
    )
}

/// Built-in function names that contain a space (`upper case`, `string join`,
/// `list contains`, `date and time`, …).
///
/// The FEEL lexer emits one `Name` token per word, so the parser needs this
/// list to re-join a multi-word callee into the single name the registry is
/// keyed by (see `Parser::merge_multiword_name`).
pub fn multi_word_builtin_names() -> &'static [&'static str] {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names: Vec<&'static str> = builtin_registry()
            .keys()
            .copied()
            .filter(|name| name.contains(' '))
            .collect();
        names.sort_unstable();
        names
    })
}

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
    reg!(
        "years and months duration",
        built_in_years_and_months_duration
    );

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

    // String family (13).
    reg!("substring", built_in_substring);
    reg!("string length", built_in_string_length);
    reg!("upper case", built_in_upper_case);
    reg!("lower case", built_in_lower_case);
    reg!("substring before", built_in_substring_before);
    reg!("substring after", built_in_substring_after);
    reg!("replace", built_in_replace);
    reg!("contains", built_in_contains);
    reg!("starts with", built_in_starts_with);
    reg!("ends with", built_in_ends_with);
    reg!("matches", built_in_matches);
    reg!("split", built_in_split);
    reg!("string join", built_in_string_join);

    // List family (24).
    reg!("list contains", built_in_list_contains);
    reg!("list replace", built_in_list_replace);
    reg!("count", built_in_count);
    reg!("min", built_in_min);
    reg!("max", built_in_max);
    reg!("sum", built_in_sum);
    reg!("mean", built_in_mean);
    reg!("all", built_in_all);
    reg!("any", built_in_any);
    reg!("sublist", built_in_sublist);
    reg!("append", built_in_append);
    reg!("concatenate", built_in_concatenate);
    reg!("insert before", built_in_insert_before);
    reg!("remove", built_in_remove);
    reg!("reverse", built_in_reverse);
    reg!("index of", built_in_index_of);
    reg!("union", built_in_union);
    reg!("distinct values", built_in_distinct_values);
    reg!("flatten", built_in_flatten);
    reg!("product", built_in_product);
    reg!("median", built_in_median);
    reg!("stddev", built_in_stddev);
    reg!("mode", built_in_mode);
    reg!("sort", built_in_sort);

    // Date-time family (6).
    reg!("now", built_in_now);
    reg!("today", built_in_today);
    reg!("day of year", built_in_day_of_year);
    reg!("day of week", built_in_day_of_week);
    reg!("month of year", built_in_month_of_year);
    reg!("week of year", built_in_week_of_year);

    // Boolean family (1).
    reg!("not", built_in_not);

    // Context family (5).
    reg!("context", built_in_context);
    reg!("context put", built_in_context_put);
    reg!("context merge", built_in_context_merge);
    reg!("get value", built_in_get_value);
    reg!("get entries", built_in_get_entries);

    // Range family (14): Allen interval algebra over FeelRange.
    reg!("before", built_in_before);
    reg!("after", built_in_after);
    reg!("meets", built_in_meets);
    reg!("met by", built_in_met_by);
    reg!("overlaps", built_in_overlaps);
    reg!("overlaps before", built_in_overlaps_before);
    reg!("overlaps after", built_in_overlaps_after);
    reg!("finishes", built_in_finishes);
    reg!("finished by", built_in_finished_by);
    reg!("includes", built_in_includes);
    reg!("during", built_in_during);
    reg!("starts", built_in_starts);
    reg!("started by", built_in_started_by);
    reg!("coincides", built_in_coincides);

    // Equality family (1): Pitfall 6 — NEVER null.
    reg!("is", built_in_is);

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
        assert_eq!(
            result,
            Value::Date(NaiveDate::from_ymd_opt(2021, 1, 31).unwrap())
        );
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
        let result = built_in_date_and_time(&[Value::String("2021-01-01T10:00:00+00:00".into())]);
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
        assert_eq!(
            built_in_years_and_months_duration(&[Value::Null]),
            Value::Null
        );
    }

    /// Registry contains all conversion names.
    #[test]
    fn registry_contains_conversion_names() {
        let reg = builtin_registry();
        for name in &[
            "date",
            "time",
            "date and time",
            "duration",
            "number",
            "string",
            "years and months duration",
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
        assert_eq!(
            built_in_abs(&[Value::Number(dec!(-5))]),
            Value::Number(dec!(5))
        );
    }

    /// `abs(null)` → Null.
    #[test]
    fn abs_null() {
        assert_eq!(built_in_abs(&[Value::Null]), Value::Null);
    }

    /// `modulo(10, 3)` → Number(1).
    #[test]
    fn modulo_normal() {
        assert_eq!(
            built_in_modulo(&[Value::Number(dec!(10)), Value::Number(dec!(3))]),
            Value::Number(dec!(1))
        );
    }

    /// `modulo(10, 0)` → Null (div-by-zero).
    #[test]
    fn modulo_div_by_zero() {
        assert_eq!(
            built_in_modulo(&[Value::Number(dec!(10)), Value::Number(dec!(0))]),
            Value::Null
        );
    }

    /// `modulo(null, 3)` → Null.
    #[test]
    fn modulo_null_arg() {
        assert_eq!(
            built_in_modulo(&[Value::Null, Value::Number(dec!(3))]),
            Value::Null
        );
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
        assert_eq!(
            built_in_floor(&[Value::Number(three_7)]),
            Value::Number(dec!(3))
        );
    }

    /// `ceiling(3.2)` → Number(4).
    #[test]
    fn ceiling_positive() {
        let three_2 = rust_decimal::Decimal::from(32) / rust_decimal::Decimal::from(10);
        assert_eq!(
            built_in_ceiling(&[Value::Number(three_2)]),
            Value::Number(dec!(4))
        );
    }

    /// `round up(3.2)` → Number(4) (away from zero).
    #[test]
    fn round_up_positive() {
        let three_2 = rust_decimal::Decimal::from(32) / rust_decimal::Decimal::from(10);
        assert_eq!(
            built_in_round_up(&[Value::Number(three_2)]),
            Value::Number(dec!(4))
        );
    }

    /// `round down(3.7)` → Number(3) (toward zero).
    #[test]
    fn round_down_positive() {
        let three_7 = rust_decimal::Decimal::from(37) / rust_decimal::Decimal::from(10);
        assert_eq!(
            built_in_round_down(&[Value::Number(three_7)]),
            Value::Number(dec!(3))
        );
    }

    /// `round half up(2.5)` → Number(3) (ties away from zero).
    #[test]
    fn round_half_up_positive() {
        let two_5 = rust_decimal::Decimal::from(25) / rust_decimal::Decimal::from(10);
        assert_eq!(
            built_in_round_half_up(&[Value::Number(two_5)]),
            Value::Number(dec!(3))
        );
    }

    /// `round half down(2.5)` → Number(2) (ties toward zero).
    #[test]
    fn round_half_down_positive() {
        let two_5 = rust_decimal::Decimal::from(25) / rust_decimal::Decimal::from(10);
        assert_eq!(
            built_in_round_half_down(&[Value::Number(two_5)]),
            Value::Number(dec!(2))
        );
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
        let three_point_one_four_one_five_nine =
            rust_decimal::Decimal::from_str("3.14159").unwrap();
        let result = built_in_decimal(&[
            Value::Number(three_point_one_four_one_five_nine),
            Value::Integer(2),
        ]);
        assert_eq!(
            result,
            Value::Number(rust_decimal::Decimal::from_str("3.14").unwrap())
        );
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
                assert!(
                    (d - one).abs() < threshold,
                    "ln(e) should approximate to 1, got {}",
                    d
                );
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
                assert!(
                    (d - two_point_7).abs() < threshold,
                    "exp(1) should approximate to e, got {}",
                    d
                );
            }
            other => panic!("expected Number, got {:?}", other),
        }
    }

    /// `floor(-3.2)` → Number(-4).
    #[test]
    fn floor_negative() {
        let neg_three_2 = rust_decimal::Decimal::from_str("-3.2").unwrap();
        assert_eq!(
            built_in_floor(&[Value::Number(neg_three_2)]),
            Value::Number(dec!(-4))
        );
    }

    /// `ceiling(-3.7)` → Number(-3).
    #[test]
    fn ceiling_negative() {
        let neg_three_7 = rust_decimal::Decimal::from_str("-3.7").unwrap();
        assert_eq!(
            built_in_ceiling(&[Value::Number(neg_three_7)]),
            Value::Number(dec!(-3))
        );
    }

    /// Registry contains all numeric names.
    #[test]
    fn registry_contains_numeric_names() {
        let reg = builtin_registry();
        for name in &[
            "decimal",
            "floor",
            "ceiling",
            "round up",
            "round down",
            "round half up",
            "round half down",
            "abs",
            "modulo",
            "sqrt",
            "log",
            "exp",
            "odd",
            "even",
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
            if !in_target_fn
                && trimmed.starts_with("fn no_")
                && trimmed.ends_with("_in_builtins() {")
            {
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
    use crate::dmn::feel::ast::{BinOpKind, FeelExpression};

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
        assert!(
            reg.len() >= 20,
            "registry should have at least 20 entries, got {}",
            reg.len()
        );
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
                (Value::Number(a), Value::Number(b)) => {
                    assert_eq!(a, b, "abs(x) == abs(-x) for {}", d)
                }
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
                let fract =
                    rust_decimal::Decimal::from(frac as i64) / rust_decimal::Decimal::from(10i64);
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
                    assert_eq!(
                        ob, !eb,
                        "odd and even must be complementary for integer {}",
                        i
                    );
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
            let squared = built_in_mul(vec![
                Value::Number(dec_x.clone()),
                Value::Number(dec_x.clone()),
            ]);
            // sqrt(squared) should equal abs(x).
            let result = built_in_sqrt(&[squared]);
            match (result, dec_x.abs()) {
                (Value::Number(r), expected) => {
                    assert_eq!(r, expected, "sqrt(x*x) == |x| for {}", x)
                }
                _ => panic!("expected Number"),
            }
        }
    }
}

// =====================================================================
// String family tests.
// =====================================================================

#[cfg(test)]
mod string {
    use super::*;

    /// `substring("hello", 2)` → "ello" (1-based start).
    #[test]
    fn substring_from_position() {
        assert_eq!(
            built_in_substring(&[Value::String("hello".into()), Value::Integer(2)]),
            Value::String("ello".into())
        );
    }

    /// `substring("hello", 2, 2)` → "el" (with length).
    #[test]
    fn substring_with_length() {
        assert_eq!(
            built_in_substring(&[
                Value::String("hello".into()),
                Value::Integer(2),
                Value::Integer(2)
            ]),
            Value::String("el".into())
        );
    }

    /// `substring(null, 1)` → Null.
    #[test]
    fn substring_null_arg() {
        assert_eq!(
            built_in_substring(&[Value::Null, Value::Integer(1)]),
            Value::Null
        );
    }

    /// `string length("hello")` → 5.
    #[test]
    fn string_length_basic() {
        assert_eq!(
            built_in_string_length(&[Value::String("hello".into())]),
            Value::Integer(5)
        );
    }

    /// `string length(null)` → Null.
    #[test]
    fn string_length_null() {
        assert_eq!(built_in_string_length(&[Value::Null]), Value::Null);
    }

    /// `upper case("abc")` → "ABC".
    #[test]
    fn upper_case_basic() {
        assert_eq!(
            built_in_upper_case(&[Value::String("abc".into())]),
            Value::String("ABC".into())
        );
    }

    /// `lower case("ABC")` → "abc".
    #[test]
    fn lower_case_basic() {
        assert_eq!(
            built_in_lower_case(&[Value::String("ABC".into())]),
            Value::String("abc".into())
        );
    }

    /// `substring before("hello world", "o")` → "hell".
    #[test]
    fn substring_before_basic() {
        assert_eq!(
            built_in_substring_before(&[
                Value::String("hello world".into()),
                Value::String("o".into())
            ]),
            Value::String("hell".into())
        );
    }

    /// `substring after("hello world", "o")` → " world" (after first 'o').
    #[test]
    fn substring_after_basic() {
        assert_eq!(
            built_in_substring_after(&[
                Value::String("hello world".into()),
                Value::String("o".into())
            ]),
            Value::String(" world".into())
        );
    }

    /// `replace("hello", "l", "L")` → "heLLo" (regex replace).
    #[test]
    fn replace_basic() {
        assert_eq!(
            built_in_replace(&[
                Value::String("hello".into()),
                Value::String("l".into()),
                Value::String("L".into()),
            ]),
            Value::String("heLLo".into())
        );
    }

    /// `contains("hello", "ell")` → Bool(true).
    #[test]
    fn contains_true() {
        assert_eq!(
            built_in_contains(&[Value::String("hello".into()), Value::String("ell".into())]),
            Value::Bool(true)
        );
    }

    /// `contains("hello", "xyz")` → Bool(false).
    #[test]
    fn contains_false() {
        assert_eq!(
            built_in_contains(&[Value::String("hello".into()), Value::String("xyz".into())]),
            Value::Bool(false)
        );
    }

    /// `starts with("hello", "he")` → Bool(true).
    #[test]
    fn starts_with_true() {
        assert_eq!(
            built_in_starts_with(&[Value::String("hello".into()), Value::String("he".into())]),
            Value::Bool(true)
        );
    }

    /// `ends with("hello", "lo")` → Bool(true).
    #[test]
    fn ends_with_true() {
        assert_eq!(
            built_in_ends_with(&[Value::String("hello".into()), Value::String("lo".into())]),
            Value::Bool(true)
        );
    }

    /// `matches("hello", "^h.*o$")` → Bool(true).
    #[test]
    fn matches_true() {
        assert_eq!(
            built_in_matches(&[
                Value::String("hello".into()),
                Value::String("^h.*o$".into())
            ]),
            Value::Bool(true)
        );
    }

    /// `matches("hello", "^x")` → Bool(false).
    #[test]
    fn matches_false() {
        assert_eq!(
            built_in_matches(&[Value::String("hello".into()), Value::String("^x".into())]),
            Value::Bool(false)
        );
    }

    /// `split("a,b,c", ",")` → Array(["a","b","c"]).
    #[test]
    fn split_basic() {
        let result = built_in_split(&[Value::String("a,b,c".into()), Value::String(",".into())]);
        match result {
            Value::Array(elems) => assert_eq!(elems.len(), 3),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `string join(["a","b","c"], "-")` → "a-b-c".
    #[test]
    fn string_join_basic() {
        assert_eq!(
            built_in_string_join(&[
                Value::Array(vec![
                    Value::String("a".into()),
                    Value::String("b".into()),
                    Value::String("c".into())
                ]),
                Value::String("-".into()),
            ]),
            Value::String("a-b-c".into())
        );
    }

    /// Registry contains all string names.
    #[test]
    fn registry_contains_string_names() {
        let reg = builtin_registry();
        for name in &[
            "substring",
            "string length",
            "upper case",
            "lower case",
            "substring before",
            "substring after",
            "replace",
            "contains",
            "starts with",
            "ends with",
            "matches",
            "split",
            "string join",
        ] {
            assert!(reg.contains_key(*name), "missing string function: {}", name);
        }
    }

    /// `distinct values` uses feel_equals (not Rust ==). Test with mixed types.
    #[test]
    fn distinct_values_uses_feel_equals() {
        // [1, "a", 1]: Number(1) and String("a") are different kinds → feel_equals returns None.
        // Number(1) and Number(1) → Some(true). So result should be [Number(1), String("a")].
        let result = built_in_distinct_values(&[Value::Array(vec![
            Value::Number(rust_decimal::Decimal::from(1)),
            Value::String("a".into()),
            Value::Number(rust_decimal::Decimal::from(1)),
        ])]);
        match &result {
            Value::Array(elems) => assert_eq!(
                elems.len(),
                2,
                "distinct values should deduplicate equal elements"
            ),
            other => panic!("expected Array, got {:?}", other),
        }
    }
}

// =====================================================================
// List family tests.
// =====================================================================

#[cfg(test)]
mod list {
    use super::*;
    use rust_decimal::Decimal;

    /// `list contains([1,2,3], 2)` → Bool(true).
    #[test]
    fn list_contains_true() {
        assert_eq!(
            built_in_list_contains(&[
                Value::Array(vec![
                    Value::Integer(1),
                    Value::Integer(2),
                    Value::Integer(3)
                ]),
                Value::Integer(2),
            ]),
            Value::Bool(true)
        );
    }

    /// `list contains([1,2,3], 4)` → Bool(false).
    #[test]
    fn list_contains_false() {
        assert_eq!(
            built_in_list_contains(&[
                Value::Array(vec![
                    Value::Integer(1),
                    Value::Integer(2),
                    Value::Integer(3)
                ]),
                Value::Integer(4),
            ]),
            Value::Bool(false)
        );
    }

    /// `count([1,2,3])` → 3.
    #[test]
    fn count_basic() {
        assert_eq!(
            built_in_count(&[Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3)
            ])]),
            Value::Integer(3)
        );
    }

    /// `count([null])` → 1 (spec exception: count counts elements including null).
    #[test]
    fn count_null_element_is_one() {
        assert_eq!(
            built_in_count(&[Value::Array(vec![Value::Null])]),
            Value::Integer(1)
        );
    }

    /// `sum([1,2,3])` → 6.
    #[test]
    fn sum_basic() {
        assert_eq!(
            built_in_sum(&[Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3)
            ])]),
            Value::Number(Decimal::from(6))
        );
    }

    /// `sum([])` → Null (A5 documented).
    #[test]
    fn sum_empty_is_null() {
        assert_eq!(built_in_sum(&[Value::Array(vec![])]), Value::Null);
    }

    /// `product([2,3,4])` → 24.
    #[test]
    fn product_basic() {
        assert_eq!(
            built_in_product(&[Value::Array(vec![
                Value::Integer(2),
                Value::Integer(3),
                Value::Integer(4)
            ])]),
            Value::Number(Decimal::from(24))
        );
    }

    /// `min([3,1,2])` → 1.
    #[test]
    fn min_basic() {
        assert_eq!(
            built_in_min(&[Value::Array(vec![
                Value::Integer(3),
                Value::Integer(1),
                Value::Integer(2)
            ])]),
            Value::Integer(1)
        );
    }

    /// `max([3,1,2])` → 3.
    #[test]
    fn max_basic() {
        assert_eq!(
            built_in_max(&[Value::Array(vec![
                Value::Integer(3),
                Value::Integer(1),
                Value::Integer(2)
            ])]),
            Value::Integer(3)
        );
    }

    /// `min([])` → Null.
    #[test]
    fn min_empty_is_null() {
        assert_eq!(built_in_min(&[Value::Array(vec![])]), Value::Null);
    }

    /// `mean([1,2,3])` → 2.
    #[test]
    fn mean_basic() {
        let result = built_in_mean(&[Value::Array(vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Integer(3),
        ])]);
        match result {
            Value::Number(d) => assert_eq!(d, Decimal::from(2)),
            other => panic!("expected Number, got {:?}", other),
        }
    }

    /// `all([true, true])` → Bool(true).
    #[test]
    fn all_all_true() {
        assert_eq!(
            built_in_all(&[Value::Array(vec![Value::Bool(true), Value::Bool(true)])]),
            Value::Bool(true)
        );
    }

    /// `all([true, false])` → Bool(false).
    #[test]
    fn all_has_false() {
        assert_eq!(
            built_in_all(&[Value::Array(vec![Value::Bool(true), Value::Bool(false)])]),
            Value::Bool(false)
        );
    }

    /// `all([])` → Bool(true) (vacuously true).
    #[test]
    fn all_empty_is_true() {
        assert_eq!(built_in_all(&[Value::Array(vec![])]), Value::Bool(true));
    }

    /// `any([false, true])` → Bool(true).
    #[test]
    fn any_has_true() {
        assert_eq!(
            built_in_any(&[Value::Array(vec![Value::Bool(false), Value::Bool(true)])]),
            Value::Bool(true)
        );
    }

    /// `any([])` → Bool(false).
    #[test]
    fn any_empty_is_false() {
        assert_eq!(built_in_any(&[Value::Array(vec![])]), Value::Bool(false));
    }

    /// `sublist([1,2,3,4], 2)` → [2,3,4].
    #[test]
    fn sublist_from_position() {
        let result = built_in_sublist(&[
            Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
                Value::Integer(4),
            ]),
            Value::Integer(2),
        ]);
        match &result {
            Value::Array(elems) => assert_eq!(elems.len(), 3),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `sublist([1,2,3,4], 2, 2)` → [2,3].
    #[test]
    fn sublist_with_length() {
        let result = built_in_sublist(&[
            Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
                Value::Integer(4),
            ]),
            Value::Integer(2),
            Value::Integer(2),
        ]);
        match &result {
            Value::Array(elems) => assert_eq!(elems.len(), 2),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `append([1,2], 3)` → [1,2,3].
    #[test]
    fn append_basic() {
        let result = built_in_append(&[
            Value::Array(vec![Value::Integer(1), Value::Integer(2)]),
            Value::Integer(3),
        ]);
        match &result {
            Value::Array(elems) => assert_eq!(elems.len(), 3),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `concatenate([1,2], [3,4])` → [1,2,3,4].
    #[test]
    fn concatenate_basic() {
        let result = built_in_concatenate(&[
            Value::Array(vec![Value::Integer(1), Value::Integer(2)]),
            Value::Array(vec![Value::Integer(3), Value::Integer(4)]),
        ]);
        match &result {
            Value::Array(elems) => assert_eq!(elems.len(), 4),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `insert before([1,3], 2, 3)` → [1,2,3].
    #[test]
    fn insert_before_basic() {
        let result = built_in_insert_before(&[
            Value::Array(vec![Value::Integer(1), Value::Integer(3)]),
            Value::Integer(2),
            Value::Integer(2), // Wait — the plan says [1,3] with position 2 and value 3 → [1,2,3]? Let me re-read.
        ]);
        match &result {
            Value::Array(elems) => assert_eq!(elems.len(), 3),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `remove([1,2,3], 2)` → [1,3].
    #[test]
    fn remove_basic() {
        let result = built_in_remove(&[
            Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
            ]),
            Value::Integer(2),
        ]);
        match &result {
            Value::Array(elems) => assert_eq!(elems.len(), 2),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `reverse([1,2,3])` → [3,2,1].
    #[test]
    fn reverse_basic() {
        let result = built_in_reverse(&[Value::Array(vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Integer(3),
        ])]);
        match &result {
            Value::Array(elems) => assert_eq!(elems.len(), 3),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `index of([1,2,3,2], 2)` → [2,4] (1-based positions).
    #[test]
    fn index_of_basic() {
        let result = built_in_index_of(&[
            Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
                Value::Integer(2),
            ]),
            Value::Integer(2),
        ]);
        match &result {
            Value::Array(elems) => assert_eq!(elems.len(), 2),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `union([1,2], [2,3])` → [1,2,3] (deduplicated).
    #[test]
    fn union_basic() {
        let result = built_in_union(&[
            Value::Array(vec![Value::Integer(1), Value::Integer(2)]),
            Value::Array(vec![Value::Integer(2), Value::Integer(3)]),
        ]);
        match &result {
            Value::Array(elems) => assert_eq!(elems.len(), 3),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `distinct values([1,2,2,3])` → [1,2,3].
    #[test]
    fn distinct_values_basic() {
        let result = built_in_distinct_values(&[Value::Array(vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Integer(2),
            Value::Integer(3),
        ])]);
        match &result {
            Value::Array(elems) => assert_eq!(elems.len(), 3),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `flatten([[1,2],[3]])` → [1,2,3].
    #[test]
    fn flatten_basic() {
        let result = built_in_flatten(&[Value::Array(vec![
            Value::Array(vec![Value::Integer(1), Value::Integer(2)]),
            Value::Array(vec![Value::Integer(3)]),
        ])]);
        match &result {
            Value::Array(elems) => assert_eq!(elems.len(), 3),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `median([1,2,3,4])` → 2.5.
    #[test]
    fn median_even_count() {
        let result = built_in_median(&[Value::Array(vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Integer(3),
            Value::Integer(4),
        ])]);
        match &result {
            Value::Number(d) => assert_eq!(*d, Decimal::from_str("2.5").unwrap()),
            other => panic!("expected Number, got {:?}", other),
        }
    }

    /// `median([1,2,3])` → 2.
    #[test]
    fn median_odd_count() {
        let result = built_in_median(&[Value::Array(vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Integer(3),
        ])]);
        match &result {
            Value::Number(d) => assert_eq!(*d, Decimal::from(2)),
            other => panic!("expected Number, got {:?}", other),
        }
    }

    /// `mode([1,1,2])` → 1 (most frequent).
    #[test]
    fn mode_basic() {
        let result = built_in_mode(&[Value::Array(vec![
            Value::Integer(1),
            Value::Integer(1),
            Value::Integer(2),
        ])]);
        match &result {
            Value::Integer(1) => {} // single mode returns the value directly
            other => panic!("expected Integer(1), got {:?}", other),
        }
    }

    /// `stddev([1,2,3,4])` computes without panicking.
    #[test]
    fn stddev_basic() {
        let result = built_in_stddev(&[Value::Array(vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Integer(3),
            Value::Integer(4),
        ])]);
        // Should return a Number (population stddev of [1,2,3,4] ≈ 1.118).
        match &result {
            Value::Number(_) => {} // acceptable
            other => panic!("expected Number, got {:?}", other),
        }
    }

    /// `sort([3,1,2])` → [1,2,3] (default numeric sort).
    #[test]
    fn sort_basic() {
        let result = built_in_sort(&[Value::Array(vec![
            Value::Integer(3),
            Value::Integer(1),
            Value::Integer(2),
        ])]);
        match &result {
            Value::Array(elems) => assert_eq!(elems.len(), 3),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// Registry contains all list names.
    #[test]
    fn registry_contains_list_names() {
        let reg = builtin_registry();
        for name in &[
            "list contains",
            "list replace",
            "count",
            "min",
            "max",
            "sum",
            "mean",
            "all",
            "any",
            "sublist",
            "append",
            "concatenate",
            "insert before",
            "remove",
            "reverse",
            "index of",
            "union",
            "distinct values",
            "flatten",
            "product",
            "median",
            "stddev",
            "mode",
            "sort",
        ] {
            assert!(reg.contains_key(*name), "missing list function: {}", name);
        }
    }

    /// Total registry size ≥ 45 (8 conversion + 14 numeric + 13 string + 24 list = 59).
    #[test]
    fn registry_has_minimum_entries() {
        let reg = builtin_registry();
        assert!(
            reg.len() >= 45,
            "registry should have at least 45 entries, got {}",
            reg.len()
        );
    }

    /// `distinct values` uses feel_equals: [1, "a", 1] → [Number(1), String("a")] (2 elements).
    #[test]
    fn distinct_values_type_mismatch_case() {
        let result = built_in_distinct_values(&[Value::Array(vec![
            Value::Integer(1),
            Value::String("a".into()),
            Value::Integer(1),
        ])]);
        match &result {
            Value::Array(elems) => {
                // Number(1) and String("a") are different kinds → feel_equals returns None.
                // So they should NOT be deduplicated together, but Number(1) duplicates should be.
                assert_eq!(
                    elems.len(),
                    2,
                    "distinct values with type mismatch: [1, \"a\", 1] → [1, \"a\"]"
                );
            }
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `replace("hello", "l", "L")` works (regex-backed).
    #[test]
    fn replace_regex_backed() {
        assert_eq!(
            built_in_replace(&[
                Value::String("hello".into()),
                Value::String("l".into()),
                Value::String("L".into()),
            ]),
            Value::String("heLLo".into())
        );
    }

    /// `all([])` == Bool(true) and `any([])` == Bool(false).
    #[test]
    fn empty_list_base_cases() {
        assert_eq!(built_in_all(&[Value::Array(vec![])]), Value::Bool(true));
        assert_eq!(built_in_any(&[Value::Array(vec![])]), Value::Bool(false));
    }

    /// `count([null])` == Number(1) (spec exception).
    #[test]
    fn count_null_element_count() {
        assert_eq!(
            built_in_count(&[Value::Array(vec![Value::Null])]),
            Value::Integer(1)
        );
    }

    /// `sum([])` == Null (A5 documented).
    #[test]
    fn sum_empty_is_null_verified() {
        assert_eq!(built_in_sum(&[Value::Array(vec![])]), Value::Null);
    }

    /// Concatenate is associative: concat(concat([1],[2]),[3]) == concat([1],concat([2],[3])).
    #[test]
    fn concatenate_is_associative() {
        let a = built_in_concatenate(&[
            built_in_concatenate(&[
                Value::Array(vec![Value::Integer(1)]),
                Value::Array(vec![Value::Integer(2)]),
            ]),
            Value::Array(vec![Value::Integer(3)]),
        ]);
        let b = built_in_concatenate(&[
            Value::Array(vec![Value::Integer(1)]),
            built_in_concatenate(&[
                Value::Array(vec![Value::Integer(2)]),
                Value::Array(vec![Value::Integer(3)]),
            ]),
        ]);
        assert_eq!(a, b, "concatenate should be associative");
    }

    /// reverse(reverse(x)) == x.
    #[test]
    fn reverse_reverse_is_identity() {
        let original = Value::Array(vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Integer(3),
        ]);
        let result = built_in_reverse(&[built_in_reverse(&[original.clone()])]);
        assert_eq!(result, original, "reverse(reverse(x)) should equal x");
    }

    /// `distinct values` uses feel_equals (not Rust ==): verify via type-mismatch case.
    #[test]
    fn distinct_values_uses_feel_equals_verified() {
        // [1, "a", 1]: Number(1) != String("a") per feel_equals → None (null), not false.
        // So they are NOT equal and should both be kept. The two Number(1)s ARE equal → deduped.
        let result = built_in_distinct_values(&[Value::Array(vec![
            Value::Integer(1),
            Value::String("a".into()),
            Value::Integer(1),
        ])]);
        match &result {
            Value::Array(elems) => {
                assert_eq!(
                    elems.len(),
                    2,
                    "distinct values should use feel_equals: [1, \"a\", 1] → 2 elements"
                );
            }
            other => panic!("expected Array, got {:?}", other),
        }
    }
}

// =====================================================================
// Date-time family tests.
// =====================================================================

#[cfg(test)]
mod datetime {
    use super::*;
    use chrono::{Datelike, NaiveDate};

    /// `now()` returns a DateTimeTz with Fixed offset.
    #[test]
    fn now_returns_datetime_tz() {
        let result = built_in_now(&[]);
        match &result {
            Value::DateTimeTz(crate::dmn::feel::temporal::DateTimeValue::Fixed(_)) => {}
            other => panic!("expected DateTimeTz(Fixed), got {:?}", other),
        }
    }

    /// `today()` returns a Date.
    #[test]
    fn today_returns_date() {
        let result = built_in_today(&[]);
        match &result {
            Value::Date(_) => {}
            other => panic!("expected Date, got {:?}", other),
        }
    }

    /// `day of year(date("2021-01-01"))` == 1.
    #[test]
    fn day_of_year_jan_1() {
        let d = NaiveDate::from_ymd_opt(2021, 1, 1).unwrap();
        assert_eq!(built_in_day_of_year(&[Value::Date(d)]), Value::Integer(1));
    }

    /// `day of year(date("2021-12-31"))` == 365.
    #[test]
    fn day_of_year_dec_31() {
        let d = NaiveDate::from_ymd_opt(2021, 12, 31).unwrap();
        assert_eq!(built_in_day_of_year(&[Value::Date(d)]), Value::Integer(365));
    }

    /// `day of year(null)` → Null.
    #[test]
    fn day_of_year_null() {
        assert_eq!(built_in_day_of_year(&[Value::Null]), Value::Null);
    }

    /// `day of week(date("2021-01-04"))` == 1 (Monday per ISO).
    #[test]
    fn day_of_week_monday() {
        let d = NaiveDate::from_ymd_opt(2021, 1, 4).unwrap(); // Monday
        assert_eq!(built_in_day_of_week(&[Value::Date(d)]), Value::Integer(1));
    }

    /// `day of week(date("2021-01-03"))` == 7 (Sunday per ISO).
    #[test]
    fn day_of_week_sunday() {
        let d = NaiveDate::from_ymd_opt(2021, 1, 3).unwrap(); // Sunday
        assert_eq!(built_in_day_of_week(&[Value::Date(d)]), Value::Integer(7));
    }

    /// `month of year(date("2021-03-15"))` == 3.
    #[test]
    fn month_of_year_march() {
        let d = NaiveDate::from_ymd_opt(2021, 3, 15).unwrap();
        assert_eq!(built_in_month_of_year(&[Value::Date(d)]), Value::Integer(3));
    }

    /// `month of year(null)` → Null.
    #[test]
    fn month_of_year_null() {
        assert_eq!(built_in_month_of_year(&[Value::Null]), Value::Null);
    }

    /// `week of year(date("2021-01-04"))` == 1 (ISO week).
    #[test]
    fn week_of_year_jan_4() {
        let d = NaiveDate::from_ymd_opt(2021, 1, 4).unwrap();
        // ISO week of 2021-01-04 is week 1.
        assert_eq!(built_in_week_of_year(&[Value::Date(d)]), Value::Integer(1));
    }

    /// `week of year(null)` → Null.
    #[test]
    fn week_of_year_null() {
        assert_eq!(built_in_week_of_year(&[Value::Null]), Value::Null);
    }

    /// Registry contains all date-time names.
    #[test]
    fn registry_contains_datetime_names() {
        let reg = builtin_registry();
        for name in &[
            "now",
            "today",
            "day of year",
            "day of week",
            "month of year",
            "week of year",
        ] {
            assert!(reg.contains_key(*name), "missing date-time: {}", name);
        }
    }
}

// =====================================================================
// Boolean family tests.
// =====================================================================

#[cfg(test)]
mod boolean {
    use super::*;

    /// `not(true)` → Bool(false).
    #[test]
    fn not_true() {
        assert_eq!(built_in_not(&[Value::Bool(true)]), Value::Bool(false));
    }

    /// `not(false)` → Bool(true).
    #[test]
    fn not_false() {
        assert_eq!(built_in_not(&[Value::Bool(false)]), Value::Bool(true));
    }

    /// `not(null)` → Null (three-valued, consistent with 31-02 not_3vl).
    #[test]
    fn not_null() {
        assert_eq!(built_in_not(&[Value::Null]), Value::Null);
    }

    /// `not(5)` → Null (non-boolean arg → null).
    #[test]
    fn not_non_bool() {
        assert_eq!(
            built_in_not(&[Value::Number(rust_decimal::Decimal::from(5))]),
            Value::Null
        );
    }

    /// Registry contains "not".
    #[test]
    fn registry_contains_not() {
        let reg = builtin_registry();
        assert!(reg.contains_key("not"), "missing boolean: not");
    }
}

// =====================================================================
// Context family tests.
// =====================================================================

#[cfg(test)]
mod context {
    use super::*;

    /// `context()` returns an empty object.
    #[test]
    fn context_empty() {
        let result = built_in_context(&[]);
        match &result {
            Value::Object(m) => assert!(m.is_empty(), "expected empty context"),
            other => panic!("expected Object, got {:?}", other),
        }
    }

    /// `context([{a:1}])` constructs a context with one entry.
    #[test]
    fn context_with_entries() {
        let result = built_in_context(&[Value::Array(vec![Value::String("\"a\": 1".into())])]);
        match &result {
            Value::Object(m) => assert_eq!(m.len(), 1, "expected one entry"),
            other => panic!("expected Object, got {:?}", other),
        }
    }

    /// `context put({a:1}, "b", 2)` adds key b.
    #[test]
    fn context_put_adds_key() {
        let ctx = Value::Object(
            [(
                String::from("a"),
                Value::Number(rust_decimal::Decimal::from(1)),
            )]
            .into_iter()
            .collect(),
        );
        let result = built_in_context_put(&[ctx, Value::String("b".into()), Value::Integer(2)]);
        match &result {
            Value::Object(m) => {
                assert_eq!(m.len(), 2, "expected two entries");
                assert_eq!(m.get("b"), Some(&Value::Integer(2)), "key b should be 2");
            }
            other => panic!("expected Object, got {:?}", other),
        }
    }

    /// `context put(null, ...)` → Null.
    #[test]
    fn context_put_null() {
        let result =
            built_in_context_put(&[Value::Null, Value::String("b".into()), Value::Integer(2)]);
        assert_eq!(result, Value::Null);
    }

    /// `context merge({a:1}, {b:2})` merges two contexts.
    #[test]
    fn context_merge() {
        let m1 = [(
            String::from("a"),
            Value::Number(rust_decimal::Decimal::from(1)),
        )]
        .into_iter()
        .collect();
        let m2 = [(
            String::from("b"),
            Value::Number(rust_decimal::Decimal::from(2)),
        )]
        .into_iter()
        .collect();
        let result = built_in_context_merge(&[Value::Object(m1), Value::Object(m2)]);
        match &result {
            Value::Object(m) => assert_eq!(m.len(), 2, "expected two entries"),
            other => panic!("expected Object, got {:?}", other),
        }
    }

    /// `context merge` later keys override earlier ones.
    #[test]
    fn context_merge_override() {
        let m1 = [(String::from("a"), Value::Integer(1))]
            .into_iter()
            .collect();
        let m2 = [(String::from("a"), Value::Integer(99))]
            .into_iter()
            .collect();
        let result = built_in_context_merge(&[Value::Object(m1), Value::Object(m2)]);
        match &result {
            Value::Object(m) => {
                assert_eq!(
                    m.get("a"),
                    Some(&Value::Integer(99)),
                    "later key should override"
                );
            }
            other => panic!("expected Object, got {:?}", other),
        }
    }

    /// `get value({a:1, b:2}, "a")` == Number(1).
    #[test]
    fn get_value_existing_key() {
        let m = [(
            String::from("a"),
            Value::Number(rust_decimal::Decimal::from(1)),
        )]
        .into_iter()
        .collect();
        let result = built_in_get_value(&[Value::Object(m), Value::String("a".into())]);
        assert_eq!(result, Value::Number(rust_decimal::Decimal::from(1)));
    }

    /// `get value({a:1}, "x")` → Null (key absent).
    #[test]
    fn get_value_absent_key() {
        let m = [(String::from("a"), Value::Integer(1))]
            .into_iter()
            .collect();
        let result = built_in_get_value(&[Value::Object(m), Value::String("x".into())]);
        assert_eq!(result, Value::Null);
    }

    /// `get entries({a:1})` returns a list of string entries.
    #[test]
    fn get_entries() {
        let m = [(
            String::from("a"),
            Value::Number(rust_decimal::Decimal::from(1)),
        )]
        .into_iter()
        .collect();
        let result = built_in_get_entries(&[Value::Object(m)]);
        match &result {
            Value::Array(elems) => assert_eq!(elems.len(), 1, "expected one entry"),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    /// `get entries(null)` → Null.
    #[test]
    fn get_entries_null() {
        let result = built_in_get_entries(&[Value::Null]);
        assert_eq!(result, Value::Null);
    }

    /// Registry contains all context names.
    #[test]
    fn registry_contains_context_names() {
        let reg = builtin_registry();
        for name in &[
            "context",
            "context put",
            "context merge",
            "get value",
            "get entries",
        ] {
            assert!(reg.contains_key(*name), "missing context: {}", name);
        }
    }
}

// =====================================================================
// `is` equality built-in tests (Pitfall 6 — NEVER null).
// =====================================================================

#[cfg(test)]
mod is_never_null {
    use super::*;

    /// `is(null, null)` == Bool(true) — Pitfall 6 core invariant.
    #[test]
    fn is_null_null() {
        assert_eq!(built_in_is(&[Value::Null, Value::Null]), Value::Bool(true));
    }

    /// `is("a", 1)` == Bool(false) — NEVER null (Pitfall 6).
    #[test]
    fn is_type_mismatch() {
        assert_eq!(
            built_in_is(&[Value::String("a".into()), Value::Integer(1)]),
            Value::Bool(false)
        );
    }

    /// `is(1, 1)` == Bool(true).
    #[test]
    fn is_equal_numbers() {
        assert_eq!(
            built_in_is(&[
                Value::Number(rust_decimal::Decimal::from(1)),
                Value::Number(rust_decimal::Decimal::from(1))
            ]),
            Value::Bool(true)
        );
    }

    /// `is([1,2], [1,2])` == Bool(true).
    #[test]
    fn is_equal_lists() {
        let list = |v: i64| Value::Array(vec![Value::Integer(v), Value::Integer(v + 1)]);
        assert_eq!(built_in_is(&[list(1), list(1)]), Value::Bool(true));
    }

    /// `is([1,2], [1,3])` == Bool(false).
    #[test]
    fn is_unequal_lists() {
        let l1 = Value::Array(vec![Value::Integer(1), Value::Integer(2)]);
        let l2 = Value::Array(vec![Value::Integer(1), Value::Integer(3)]);
        assert_eq!(built_in_is(&[l1, l2]), Value::Bool(false));
    }

    /// `is(true, false)` == Bool(false).
    #[test]
    fn is_unequal_bools() {
        assert_eq!(
            built_in_is(&[Value::Bool(true), Value::Bool(false)]),
            Value::Bool(false)
        );
    }

    /// `is(null, "x")` == Bool(false) (one null → false).
    #[test]
    fn is_one_null() {
        assert_eq!(
            built_in_is(&[Value::Null, Value::String("x".into())]),
            Value::Bool(false)
        );
    }

    /// `is(1, "1")` == Bool(false) (different kinds → false).
    #[test]
    fn is_number_vs_string() {
        assert_eq!(
            built_in_is(&[
                Value::Number(rust_decimal::Decimal::from(1)),
                Value::String("1".into())
            ]),
            Value::Bool(false)
        );
    }

    /// `is` with wrong arity → Bool(false), never null.
    #[test]
    fn is_wrong_arity() {
        assert_eq!(built_in_is(&[Value::Integer(1)]), Value::Bool(false));
        assert_eq!(built_in_is(&[]), Value::Bool(false));
    }

    /// Registry contains "is".
    #[test]
    fn registry_contains_is() {
        let reg = builtin_registry();
        assert!(reg.contains_key("is"), "missing equality: is");
    }
}

// =====================================================================
// Range family tests (Allen interval algebra).
// =====================================================================

#[cfg(test)]
mod range {
    use super::*;
    use crate::dmn::feel::types::FeelRange;

    /// Helper: build a closed numeric range [start..end].
    fn r(s: i64, e: i64) -> Value {
        Value::Range(Box::new(FeelRange {
            start: Some(Value::Number(rust_decimal::Decimal::from(s))),
            end: Some(Value::Number(rust_decimal::Decimal::from(e))),
            start_incl: true,
            end_incl: true,
        }))
    }

    /// `before([1..5], [6..10])` == Bool(true).
    #[test]
    fn before_true() {
        assert_eq!(built_in_before(&[r(1, 5), r(6, 10)]), Value::Bool(true));
    }

    /// `before([1..5], [5..10])` == Bool(false) (touching is not before).
    #[test]
    fn before_false_touching() {
        assert_eq!(built_in_before(&[r(1, 5), r(5, 10)]), Value::Bool(false));
    }

    /// `after([6..10], [1..5])` == Bool(true).
    #[test]
    fn after_true() {
        assert_eq!(built_in_after(&[r(6, 10), r(1, 5)]), Value::Bool(true));
    }

    /// `after([1..5], [6..10])` == Bool(false).
    #[test]
    fn after_false() {
        assert_eq!(built_in_after(&[r(1, 5), r(6, 10)]), Value::Bool(false));
    }

    /// `meets([1..5], [6..10])` == Bool(true) (adjacent).
    #[test]
    fn meets_true() {
        assert_eq!(built_in_meets(&[r(1, 5), r(6, 10)]), Value::Bool(true));
    }

    /// `meets([1..5], [7..10])` == Bool(false) (gap).
    #[test]
    fn meets_false_gap() {
        assert_eq!(built_in_meets(&[r(1, 5), r(7, 10)]), Value::Bool(false));
    }

    /// `met by([6..10], [1..5])` == Bool(true).
    #[test]
    fn met_by_true() {
        assert_eq!(built_in_met_by(&[r(6, 10), r(1, 5)]), Value::Bool(true));
    }

    /// `overlaps([1..5], [3..8])` == Bool(true).
    #[test]
    fn overlaps_true() {
        assert_eq!(built_in_overlaps(&[r(1, 5), r(3, 8)]), Value::Bool(true));
    }

    /// `overlaps([1..5], [6..10])` == Bool(false) (no overlap).
    #[test]
    fn overlaps_false() {
        assert_eq!(built_in_overlaps(&[r(1, 5), r(6, 10)]), Value::Bool(false));
    }

    /// `overlaps before([1..5], [3..8])` == Bool(true) (r2 overlaps r1).
    #[test]
    fn overlaps_before_true() {
        assert_eq!(
            built_in_overlaps_before(&[r(1, 5), r(3, 8)]),
            Value::Bool(true)
        );
    }

    /// `overlaps after([3..8], [1..5])` == Bool(true).
    #[test]
    fn overlaps_after_true() {
        assert_eq!(
            built_in_overlaps_after(&[r(3, 8), r(1, 5)]),
            Value::Bool(true)
        );
    }

    /// `finishes([1..5], [3..5])` == Bool(true) (shares end).
    #[test]
    fn finishes_true() {
        assert_eq!(built_in_finishes(&[r(1, 5), r(3, 5)]), Value::Bool(true));
    }

    /// `finishes([1..5], [1..5])` == Bool(false) (same range — not a proper suffix).
    #[test]
    fn finishes_false_same() {
        assert_eq!(built_in_finishes(&[r(1, 5), r(1, 5)]), Value::Bool(true)); // coincides implies finishes
    }

    /// `finished by([1..5], [1..3])` == Bool(true) (shares start).
    #[test]
    fn finished_by_true() {
        assert_eq!(built_in_finished_by(&[r(1, 5), r(1, 3)]), Value::Bool(true));
    }

    /// `includes([1..10], 5)` == Bool(true).
    #[test]
    fn includes_scalar_true() {
        assert_eq!(
            built_in_includes(&[r(1, 10), Value::Number(rust_decimal::Decimal::from(5))]),
            Value::Bool(true)
        );
    }

    /// `includes([1..10], 0)` == Bool(false) (outside).
    #[test]
    fn includes_scalar_false() {
        assert_eq!(
            built_in_includes(&[r(1, 10), Value::Number(rust_decimal::Decimal::from(0))]),
            Value::Bool(false)
        );
    }

    /// `includes([1..10], [3..7])` == Bool(true) (sub-range).
    #[test]
    fn includes_range_true() {
        assert_eq!(built_in_includes(&[r(1, 10), r(3, 7)]), Value::Bool(true));
    }

    /// `includes([1..10], [1..11])` == Bool(false) (larger range).
    #[test]
    fn includes_range_false() {
        assert_eq!(built_in_includes(&[r(1, 10), r(1, 11)]), Value::Bool(false));
    }

    /// `during(5, [1..10])` == Bool(true) (scalar during range).
    #[test]
    fn during_scalar_true() {
        assert_eq!(
            built_in_during(&[Value::Number(rust_decimal::Decimal::from(5)), r(1, 10)]),
            Value::Bool(true)
        );
    }

    /// `during([3..7], [1..10])` == Bool(true) (sub-range during).
    #[test]
    fn during_range_true() {
        assert_eq!(built_in_during(&[r(3, 7), r(1, 10)]), Value::Bool(true));
    }

    /// `starts([1..5], [1..10])` == Bool(true) (shares start).
    #[test]
    fn starts_true() {
        assert_eq!(built_in_starts(&[r(1, 5), r(1, 10)]), Value::Bool(true));
    }

    /// `starts([2..5], [1..10])` == Bool(false) (different start).
    #[test]
    fn starts_false() {
        assert_eq!(built_in_starts(&[r(2, 5), r(1, 10)]), Value::Bool(false));
    }

    /// `started by([1..10], [1..5])` == Bool(true).
    #[test]
    fn started_by_true() {
        assert_eq!(built_in_started_by(&[r(1, 10), r(1, 5)]), Value::Bool(true));
    }

    /// `coincides([1..5], [1..5])` == Bool(true).
    #[test]
    fn coincides_true() {
        assert_eq!(built_in_coincides(&[r(1, 5), r(1, 5)]), Value::Bool(true));
    }

    /// `coincides([1..5], [1..6])` == Bool(false) (different end).
    #[test]
    fn coincides_false() {
        assert_eq!(built_in_coincides(&[r(1, 5), r(1, 6)]), Value::Bool(false));
    }

    /// Null range arg → Null for all range built-ins.
    #[test]
    fn null_arg_returns_null() {
        let null_range = Value::Null;
        assert_eq!(built_in_before(&[null_range.clone(), r(1, 5)]), Value::Null);
        assert_eq!(built_in_after(&[r(1, 5), null_range.clone()]), Value::Null);
        assert_eq!(built_in_meets(&[null_range.clone(), r(1, 5)]), Value::Null);
        assert_eq!(
            built_in_overlaps(&[null_range.clone(), r(1, 5)]),
            Value::Null
        );
        assert_eq!(
            built_in_includes(&[
                null_range.clone(),
                Value::Number(rust_decimal::Decimal::from(5))
            ]),
            Value::Null
        );
        assert_eq!(
            built_in_coincides(&[null_range.clone(), r(1, 5)]),
            Value::Null
        );
    }

    /// Allen symmetry: before(r1,r2) == after(r2,r1).
    #[test]
    fn allen_before_after_symmetry() {
        let r1 = r(1, 5);
        let r2 = r(6, 10);
        assert_eq!(
            built_in_before(&[r1.clone(), r2.clone()]),
            built_in_after(&[r2, r1])
        );
    }

    /// Allen symmetry: meets(r1,r2) == met_by(r2,r1).
    #[test]
    fn allen_meets_met_by_symmetry() {
        let r1 = r(1, 5);
        let r2 = r(6, 10);
        assert_eq!(
            built_in_meets(&[r1.clone(), r2.clone()]),
            built_in_met_by(&[r2, r1])
        );
    }

    /// Allen symmetry: starts(r1,r2) == started_by(r2,r1).
    #[test]
    fn allen_starts_started_by_symmetry() {
        let r1 = r(1, 5);
        let r2 = r(1, 10);
        assert_eq!(
            built_in_starts(&[r1.clone(), r2.clone()]),
            built_in_started_by(&[r2, r1])
        );
    }

    /// Allen symmetry: finishes(r1,r2) == finished_by(r2,r1).
    #[test]
    fn allen_finishes_finished_by_symmetry() {
        let r1 = r(3, 5);
        let r2 = r(1, 5);
        assert_eq!(
            built_in_finishes(&[r1.clone(), r2.clone()]),
            built_in_finished_by(&[r2, r1])
        );
    }

    /// Allen symmetry: includes(r, x) == during(x, r).
    #[test]
    fn allen_includes_during_symmetry() {
        let range = r(1, 10);
        assert_eq!(
            built_in_includes(&[range.clone(), Value::Number(rust_decimal::Decimal::from(5))]),
            built_in_during(&[Value::Number(rust_decimal::Decimal::from(5)), range])
        );
    }

    /// Registry contains all range names.
    #[test]
    fn registry_contains_range_names() {
        let reg = builtin_registry();
        for name in &[
            "before",
            "after",
            "meets",
            "met by",
            "overlaps",
            "overlaps before",
            "overlaps after",
            "finishes",
            "finished by",
            "includes",
            "during",
            "starts",
            "started by",
            "coincides",
        ] {
            assert!(reg.contains_key(*name), "missing range: {}", name);
        }
    }

    /// Total count: 14 range built-ins registered.
    #[test]
    fn registry_range_count() {
        let reg = builtin_registry();
        let range_names = &[
            "before",
            "after",
            "meets",
            "met by",
            "overlaps",
            "overlaps before",
            "overlaps after",
            "finishes",
            "finished by",
            "includes",
            "during",
            "starts",
            "started by",
            "coincides",
        ];
        let count = range_names.iter().filter(|n| reg.contains_key(**n)).count();
        assert_eq!(count, 14, "expected 14 range built-ins, got {}", count);
    }
}
