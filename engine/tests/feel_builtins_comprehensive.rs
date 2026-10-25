//! FEEL built-in function tests (v8.0 scope: FEEL-01..FEEL-24).
//!
//! Every case goes through the real text front end — `parse_expression` then
//! `evaluate` — rather than constructing ASTs by hand, so the parser's handling
//! of multi-word function names (`upper case`, `list contains`, `get value`,
//! `date and time`, …) is exercised alongside the evaluator.
//!
//! Expectations follow DMN 1.5 §10.3.4; where the spec and an older in-repo
//! assumption disagreed, the spec wins and the test records the spec behaviour.

use engine::data_types::Value;
use engine::dmn::feel::eval::{evaluate, Scope};
use engine::dmn::feel::parser::parse_expression;
use rust_decimal::Decimal;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ev(expr: &str) -> Value {
    let ast = parse_expression(expr);
    evaluate(&ast, &Scope::empty())
}

fn num(n: i64) -> Value {
    Value::Number(Decimal::from(n))
}

/// Several built-ins (count, string length, the temporal accessors) return the
/// engine's `Integer` representation rather than a FEEL `Number`.
fn int(n: i64) -> Value {
    Value::Integer(n)
}

fn dec(s: &str) -> Value {
    Value::Number(Decimal::from_str(s).expect("valid decimal literal"))
}

fn arr(items: Vec<Value>) -> Value {
    Value::Array(items)
}

fn s(text: &str) -> Value {
    Value::String(text.to_string())
}

/// Assert the expression evaluates to exactly `expected`.
#[track_caller]
fn assert_ev(expr: &str, expected: Value) {
    let actual = ev(expr);
    assert_eq!(actual, expected, "expression: {expr}");
}

#[track_caller]
fn assert_null(expr: &str) {
    let actual = ev(expr);
    assert_eq!(actual, Value::Null, "expression: {expr} should be null");
}

// ---------------------------------------------------------------------------
// Multi-word function names (parser + evaluator)
// ---------------------------------------------------------------------------

#[test]
fn multiword_names_are_parsed_as_single_callees() {
    assert_ev("upper case(\"hello\")", s("HELLO"));
    assert_ev("lower case(\"HELLO\")", s("hello"));
    assert_ev("string length(\"hello\")", int(5));
    assert_ev("string join([\"a\", \"b\"], \"-\")", s("a-b"));
    assert_ev("list contains([1, 2, 3], 2)", Value::Bool(true));
    assert_ev("get value({a: 1}, \"a\")", num(1));
    assert_ev("substring before(\"hello world\", \" world\")", s("hello"));
    assert_ev("substring after(\"hello world\", \"hello \")", s("world"));
    assert_ev("starts with(\"abc\", \"a\")", Value::Bool(true));
    assert_ev("ends with(\"abc\", \"c\")", Value::Bool(true));
    assert_ev("round half up(1.25, 1)", dec("1.3"));
}

#[test]
fn date_and_time_is_a_multiword_callee() {
    let value = ev("date and time(\"2020-05-06T10:00:00\")");
    assert!(
        matches!(value, Value::DateTimeTz(_)),
        "expected a date-and-time value, got {value:?}"
    );
}

#[test]
fn operator_words_do_not_merge_into_function_names() {
    // `A and contains(...)` must stay an `and` expression, not a call to a
    // function named "A and contains".
    let ast = parse_expression("A and contains(\"abc\", \"b\")");
    let mut scope = Scope::empty();
    scope.bind("A".to_string(), Value::Bool(true), vec![]);
    assert_eq!(evaluate(&ast, &scope), Value::Bool(true));
}

// ---------------------------------------------------------------------------
// String built-ins (FEEL-11)
// ---------------------------------------------------------------------------

#[test]
fn string_functions_follow_spec() {
    assert_ev("substring(\"abcdef\", 2, 3)", s("bcd"));
    assert_ev("substring(\"abcdef\", 2)", s("bcdef"));
    assert_ev("replace(\"abcabc\", \"a\", \"x\")", s("xbcxbc"));
    assert_ev("contains(\"abc\", \"b\")", Value::Bool(true));
    assert_ev("contains(\"abc\", \"z\")", Value::Bool(false));
    assert_ev("string length(\"\")", int(0));
    assert_ev("split(\"a,b,c\", \",\")", arr(vec![s("a"), s("b"), s("c")]));
}

#[test]
fn string_functions_propagate_null() {
    assert_null("upper case(null)");
    assert_null("string length(null)");
    assert_null("split(null, \",\")");
    assert_null("substring(null, 1, 2)");
}

// ---------------------------------------------------------------------------
// List built-ins (FEEL-12..FEEL-17)
// ---------------------------------------------------------------------------

#[test]
fn list_functions_follow_spec() {
    assert_ev("count([1, 2, 3])", int(3));
    assert_ev("append([1, 2], 3)", arr(vec![num(1), num(2), num(3)]));
    assert_ev("concatenate([1], [2, 3])", arr(vec![num(1), num(2), num(3)]));
    assert_ev("union([1, 2], [2, 3])", arr(vec![num(1), num(2), num(3)]));
    assert_ev("sublist([1, 2, 3, 4], 2, 2)", arr(vec![num(2), num(3)]));
    assert_ev(
        "insert before([1, 3], 2, 2)",
        arr(vec![num(1), num(2), num(3)]),
    );
    assert_ev("remove([1, 2, 3], 2)", arr(vec![num(1), num(3)]));
    assert_ev("index of([1, 2, 3], 2)", arr(vec![int(2)]));
    assert_ev("all([true, true])", Value::Bool(true));
    assert_ev("any([false, true])", Value::Bool(true));
    assert_ev("all([])", Value::Bool(true));
}

#[test]
fn filter_expression_selects_matching_items() {
    assert_ev("[1, 2, 3][item > 1]", arr(vec![num(2), num(3)]));
    assert_ev("[1, 2, 3][item > 10]", arr(vec![]));
}

#[test]
fn sort_default_is_ascending() {
    assert_ev("sort([3, 1, 2])", arr(vec![num(1), num(2), num(3)]));
    assert_ev(
        "sort([\"c\", \"a\", \"b\"])",
        arr(vec![s("a"), s("b"), s("c")]),
    );
}

#[test]
fn sort_accepts_comparator_lambda() {
    // Descending via a boolean comparator (`true` means "x comes first").
    assert_ev(
        "sort([3, 1, 2], function(x, y) x > y)",
        arr(vec![num(3), num(2), num(1)]),
    );
    // Numeric comparator result.
    assert_ev(
        "sort([3, 1, 2], function(x, y) x - y)",
        arr(vec![num(1), num(2), num(3)]),
    );
    // Comparator over strings.
    assert_ev(
        "sort([\"b\", \"a\"], function(x, y) x < y)",
        arr(vec![s("a"), s("b")]),
    );
}

#[test]
fn sort_of_empty_list_is_an_empty_list() {
    // DMN 1.5 §10.3.4.2.5: sort never turns a list into null.
    assert_ev("sort([])", arr(vec![]));
    assert_ev("sort([], function(x, y) x > y)", arr(vec![]));
    assert_null("sort(null)");
}

#[test]
fn list_functions_propagate_null() {
    assert_null("count(null)");
    assert_null("append(null, 1)");
    assert_null("list contains(null, 1)");
    assert_null("sublist(null, 1, 1)");
}

// ---------------------------------------------------------------------------
// Numeric built-ins (FEEL-01..FEEL-10)
// ---------------------------------------------------------------------------

#[test]
fn numeric_functions_follow_spec() {
    assert_ev("abs(-3)", num(3));
    assert_ev("ceiling(1.2)", num(2));
    assert_ev("floor(1.7)", num(1));
    assert_ev("even(4)", Value::Bool(true));
    assert_ev("odd(4)", Value::Bool(false));
    assert_ev("modulo(5, 3)", num(2));
    assert_ev("product([1, 2, 3, 4])", num(24));
    assert_ev("median([1, 2, 3])", num(2));
    assert_ev("mode([1, 2, 2, 3])", num(2));
    assert_ev("stddev([2, 4, 4, 4, 5, 5, 7, 9])", num(2));
    assert_ev("decimal(1/3, 2)", dec("0.33"));
}

#[test]
fn sqrt_log_and_exp_edge_cases() {
    assert_ev("sqrt(9)", num(3));
    assert_null("sqrt(-1)");
    assert_null("log(0)");
    assert_null("log(-1)");
    assert_ev("exp(0)", num(1));
}

#[test]
fn empty_list_aggregates_are_null() {
    assert_null("sum([])");
    assert_null("min([])");
    assert_null("max([])");
}

// ---------------------------------------------------------------------------
// Rounding with an explicit scale (DMN 1.5 §10.3.4.1)
// ---------------------------------------------------------------------------

#[test]
fn rounding_respects_scale_argument() {
    assert_ev("round up(1.121, 2)", dec("1.13"));
    assert_ev("round down(1.129, 2)", dec("1.12"));
    assert_ev("round half up(1.25, 1)", dec("1.3"));
    assert_ev("round half down(1.25, 1)", dec("1.2"));
    assert_ev("round half up(-1.25, 1)", dec("-1.3"));
    assert_ev("round half down(-1.25, 1)", dec("-1.2"));
    // Scale 0 (and omitted scale) round to whole numbers.
    assert_ev("round up(1.121)", num(2));
    assert_ev("round down(1.9)", num(1));
    assert_null("round up(1.5, null)");
}

// ---------------------------------------------------------------------------
// Context built-ins (FEEL-18..FEEL-20)
// ---------------------------------------------------------------------------

#[test]
fn get_value_reads_a_context_entry() {
    assert_ev("get value({a: 1, b: 2}, \"b\")", num(2));
    assert_null("get value({a: 1}, \"missing\")");
    assert_null("get value(null, \"a\")");
}

#[test]
fn get_entries_returns_key_value_contexts() {
    let entries = ev("get entries({a: \"foo\", b: \"bar\"})");
    let Value::Array(items) = entries else {
        panic!("get entries must return a list");
    };
    assert_eq!(items.len(), 2);

    let mut seen = Vec::new();
    for item in &items {
        let Value::Object(map) = item else {
            panic!("each entry must be a context, got {item:?}");
        };
        let key = match map.get("key") {
            Some(Value::String(k)) => k.clone(),
            other => panic!("entry key must be a string, got {other:?}"),
        };
        let value = map.get("value").cloned().unwrap_or(Value::Null);
        seen.push((key, value));
    }
    seen.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        seen,
        vec![("a".to_string(), s("foo")), ("b".to_string(), s("bar"))]
    );

    assert_ev("get entries({})", arr(vec![]));
    assert_null("get entries(null)");
}

// ---------------------------------------------------------------------------
// Temporal built-ins (FEEL-23..FEEL-24)
// ---------------------------------------------------------------------------

#[test]
fn years_and_months_duration_between_dates() {
    let value = ev("years and months duration(date(\"2020-01-01\"), date(\"2021-03-01\"))");
    match value {
        Value::YearsMonthsDuration(d) => assert_eq!(d.months, 14),
        other => panic!("expected a years-and-months duration, got {other:?}"),
    }

    let negative = ev("years and months duration(date(\"2021-03-01\"), date(\"2020-01-01\"))");
    match negative {
        Value::YearsMonthsDuration(d) => assert_eq!(d.months, -14),
        other => panic!("expected a negative years-and-months duration, got {other:?}"),
    }
}

#[test]
fn temporal_accessors_follow_spec() {
    assert_ev("day of week(date(\"2020-01-06\"))", int(1));
    assert_ev("month of year(date(\"2020-03-15\"))", int(3));
    assert_ev("day of year(date(\"2020-02-01\"))", int(32));
    assert_ev("week of year(date(\"2020-01-01\"))", int(1));
}

// ---------------------------------------------------------------------------
// Relational + equality families (FEEL-21..FEEL-22)
// ---------------------------------------------------------------------------

#[test]
fn between_and_deep_equality() {
    assert_ev("3 between 1 and 5", Value::Bool(true));
    assert_ev("7 between 1 and 5", Value::Bool(false));
    assert_ev("[1, 2] = [1, 2]", Value::Bool(true));
    assert_ev("[1, 2] = [2, 1]", Value::Bool(false));
    assert_ev("{a: 1} = {a: 1}", Value::Bool(true));
    assert_ev("{a: 1} != {a: 2}", Value::Bool(true));
}
