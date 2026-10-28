//! Behaviour the specification listed as missing, pinned so it cannot silently regress.
//!
//! Three entries in `specs/dmn-feel.md`'s gap list were false or fixed in 9.0.64: range equality was
//! said to be a `TODO` that answered null (it works), `get entries`' description contradicted its own
//! body, and `list replace` answered an empty list where every other regex built-in answers null. The
//! first is the interesting one — a *stale gap*, a specification asserting a defect that no longer
//! existed, which is the mirror image of code keeping an old bug.

use engine::data_types::Value;
use engine::dmn::feel::eval::{evaluate, Scope};
use engine::dmn::feel::parser::parse_expression;

fn feel(source: &str) -> Value {
    evaluate(&parse_expression(source), &Scope::empty())
}

#[test]
fn ranges_compare_structurally() {
    // Same bounds and inclusivity: equal. The written form is part of a range's identity, because the
    // TCK distinguishes `(=10)` from `[10..10]`.
    assert_eq!(feel("[1..2] = [1..2]"), Value::Bool(true));
    assert_eq!(feel("[1..2] = [1..3]"), Value::Bool(false));
    assert_eq!(
        feel("[1..2] = (1..2]"),
        Value::Bool(false),
        "inclusivity differs"
    );
    assert_eq!(feel("[1..2] != [1..3]"), Value::Bool(true));
    assert_eq!(feel("[1..2] = [1..2]"), Value::Bool(true));
}

#[test]
fn get_entries_returns_contexts_not_strings() {
    // DMN §10.3.4.4: each entry is a context with `key` and `value`, sorted by key.
    let entries = feel("get entries({b: 2, a: 1})");
    let Value::Array(items) = entries else {
        panic!("expected a list of entries, got {entries:?}");
    };
    assert_eq!(items.len(), 2);
    let key_of = |entry: &Value| match entry {
        Value::Object(fields) => fields.get("key").cloned(),
        other => panic!("expected a context entry, got {other:?}"),
    };
    let value_of = |entry: &Value| match entry {
        Value::Object(fields) => fields.get("value").cloned(),
        other => panic!("expected a context entry, got {other:?}"),
    };
    assert_eq!(key_of(&items[0]), Some(Value::String("a".to_string())));
    assert_eq!(
        value_of(&items[0]),
        Some(Value::Number(engine::Decimal::from(1))),
        "a FEEL literal is a Decimal, not an Integer"
    );
    assert_eq!(key_of(&items[1]), Some(Value::String("b".to_string())));
}

#[test]
fn an_invalid_pattern_answers_null_consistently() {
    // `matches`, `replace` and `split` all answer null for a bad pattern; `list replace` used to answer
    // an empty list, which says something else entirely.
    assert_eq!(feel("list replace([\"a\"], \"[\", \"x\")"), Value::Null);
    assert_eq!(
        feel("list replace([\"ab\", \"cd\"], \"b\", \"X\")"),
        Value::Array(vec![
            Value::String("aX".to_string()),
            Value::String("cd".to_string()),
        ]),
        "a valid pattern still replaces"
    );
}

#[test]
fn matches_honours_its_flags_argument() {
    // DMN 1.5 §10.3.4.5: `matches(input, pattern, flags?)`, the flags being the XPath set.
    assert_eq!(feel("matches(\"abc\", \"ABC\")"), Value::Bool(false));
    assert_eq!(
        feel("matches(\"abc\", \"ABC\", \"i\")"),
        Value::Bool(true),
        "case-insensitive"
    );
    assert_eq!(
        feel("matches(\"a\\nb\", \"a.b\")"),
        Value::Bool(false),
        "`.` stops at a newline"
    );
    assert_eq!(
        feel("matches(\"a\\nb\", \"a.b\", \"s\")"),
        Value::Bool(true),
        "dot-all"
    );
    assert_eq!(
        feel("matches(\"ab\", \"a b\", \"x\")"),
        Value::Bool(true),
        "extended: whitespace ignored"
    );
    // An unknown flag is an error, and an error is null — not a silently unflagged match.
    assert_eq!(feel("matches(\"abc\", \"ABC\", \"q\")"), Value::Null);
}

#[test]
fn replace_honours_its_flags_argument() {
    assert_eq!(
        feel("replace(\"aBc\", \"b\", \"X\")"),
        Value::String("aBc".to_string())
    );
    assert_eq!(
        feel("replace(\"aBc\", \"B\", \"X\", \"i\")"),
        Value::String("aXc".to_string()),
        "case-insensitive replacement"
    );
    assert_eq!(feel("replace(\"abc\", \"b\", \"X\", \"q\")"), Value::Null);
}

#[test]
fn a_flagged_pattern_does_not_reuse_the_unflagged_cache_entry() {
    // The compiled-pattern cache is keyed by pattern *and* flags. Keying on the pattern alone would
    // answer this case-insensitive call from the case-sensitive entry compiled on the line above —
    // the same defect class as a rule cache keyed by node rather than by fact.
    assert_eq!(
        feel("matches(\"ABC\", \"abc\")"),
        Value::Bool(false),
        "compiles the plain pattern"
    );
    assert_eq!(
        feel("matches(\"ABC\", \"abc\", \"i\")"),
        Value::Bool(true),
        "must not reuse it"
    );
    assert_eq!(
        feel("matches(\"ABC\", \"abc\")"),
        Value::Bool(false),
        "and the plain entry is intact"
    );
}
