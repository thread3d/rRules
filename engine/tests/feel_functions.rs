//! Function values in FEEL: defining one, holding it, and calling it.
//!
//! The machinery — lexical capture, positional and named application, argument-count checks — has
//! existed since the DMN evaluator was built, but a comment in `eval.rs` said "lambda application is
//! not yet implemented — stub returns Null", which is what a reader (and this loop) took at face
//! value. These tests state what actually works and what a wrong call returns, so the next reader
//! does not have to discover it by experiment.
//!
//! **Not covered here:** `let x = … return …`. That is a FEEL **1.6** construct; the DMN 1.5 grammar
//! this evaluator implements has no `let` expression, so its absence is a version boundary rather
//! than a defect.

use engine::data_types::Value;
use engine::dmn::feel::eval::{evaluate, Scope};
use engine::dmn::feel::parser::parse_expression;

fn feel(source: &str) -> Value {
    evaluate(&parse_expression(source), &Scope::empty())
}

#[test]
fn a_function_expression_can_be_applied_immediately() {
    assert_eq!(
        feel("(function(x) x * 2)(4)"),
        Value::Number(engine::Decimal::from(8))
    );
    assert_eq!(
        feel("(function(a, b) a + b)(2, 3)"),
        Value::Number(engine::Decimal::from(5))
    );
}

#[test]
fn a_function_held_in_a_scope_binding_can_be_called() {
    // Bound by an iteration, then called with one argument.
    assert_eq!(
        feel("for f in [function(x) x * 2] return f(3)"),
        Value::Array(vec![Value::Number(engine::Decimal::from(6))]),
        "a function reached through a binding is callable"
    );

    // Reached through a context member, then called.
    assert_eq!(
        feel("{ f: function(x) x * 2 }.f(3)"),
        Value::Number(engine::Decimal::from(6))
    );
}

#[test]
fn a_comparator_function_orders_a_list() {
    assert_eq!(
        feel("sort([3, 1, 2], function(a, b) a < b)"),
        Value::Array(vec![
            Value::Number(engine::Decimal::from(1)),
            Value::Number(engine::Decimal::from(2)),
            Value::Number(engine::Decimal::from(3)),
        ])
    );
    assert_eq!(
        feel("sort([3, 1, 2], function(a, b) a > b)"),
        Value::Array(vec![
            Value::Number(engine::Decimal::from(3)),
            Value::Number(engine::Decimal::from(2)),
            Value::Number(engine::Decimal::from(1)),
        ])
    );
}

#[test]
fn calling_with_the_wrong_number_of_arguments_is_null_not_a_guess() {
    // Too few, and too many: FEEL has no default arguments and no partial application, so the answer
    // is null. (This is the behaviour the stale "stub" comment was describing.)
    assert_eq!(feel("(function(x) x * 2)()"), Value::Null);
    assert_eq!(feel("(function(a, b) a + b)(1)"), Value::Null);
}

#[test]
fn a_function_captures_its_definition_scope() {
    // `n` is bound where the function is written, not where it is called: the closure sees `n = 10`.
    assert_eq!(
        feel("for n in [10] return (function(x) x + n)(5)"),
        Value::Array(vec![Value::Number(engine::Decimal::from(15))])
    );
}
