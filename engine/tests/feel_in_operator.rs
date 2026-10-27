//! The FEEL `in` operator, in the forms DMN 1.5 defines for it.
//!
//! `x in [1..10]` always worked. `x in (1, 2, 3)` — the list form, and the one a decision table's
//! input entry looks like — did not: the parser treated `(1, 2, 3)` as a parenthesised *expression*,
//! which it is not, produced an error node, and the operator evaluated to **null for every input**.
//! A null where a user expects a boolean is the worst kind of wrong: a rule written against it simply
//! never fires, with no error to explain why.

use engine::data_types::Value;
use engine::dmn::feel::eval::{evaluate, Scope};
use engine::dmn::feel::parser::parse_expression;

fn feel(expression: &str) -> Value {
    evaluate(&parse_expression(expression), &Scope::empty())
}

#[test]
fn membership_in_a_parenthesised_list_is_a_boolean() {
    assert_eq!(feel("2 in (1, 2, 3)"), Value::Bool(true));
    assert_eq!(feel("5 in (1, 2, 3)"), Value::Bool(false));
    assert_eq!(feel("\"b\" in (\"a\", \"b\")"), Value::Bool(true));
    assert_eq!(feel("2 in (2)"), Value::Bool(true), "a one-element list");
}

#[test]
fn membership_in_a_range_still_works() {
    assert_eq!(feel("2 in [1..3]"), Value::Bool(true));
    assert_eq!(feel("5 in [1..3]"), Value::Bool(false));
    assert_eq!(
        feel("1 in [1..3]"),
        Value::Bool(true),
        "inclusive lower bound"
    );
    assert_eq!(
        feel("3 in [1..3]"),
        Value::Bool(true),
        "inclusive upper bound"
    );
    assert_eq!(
        feel("1 in (1..3]"),
        Value::Bool(false),
        "exclusive lower bound"
    );
}

#[test]
fn a_comparison_is_a_unary_test_inside_the_list() {
    assert_eq!(feel("2 in (< 5)"), Value::Bool(true));
    assert_eq!(feel("7 in (< 5)"), Value::Bool(false));
    assert_eq!(
        feel("7 in (> 5, < 2)"),
        Value::Bool(true),
        "a comma list is an or-chain"
    );
    assert_eq!(feel("3 in (> 5, < 2)"), Value::Bool(false));
}

#[test]
fn not_inverts_the_list() {
    // `not(...)` is the negative unary-test form.
    assert_eq!(feel("2 in not(1, 2, 3)"), Value::Bool(false));
    assert_eq!(feel("9 in not(1, 2, 3)"), Value::Bool(true));
}
