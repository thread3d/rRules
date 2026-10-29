//! The text form of a rule, against the grammar in `specs/rule-syntax.md`.
//!
//! The contract that matters is the **round trip**: for every rule the interface can build,
//! `parse_rule_text(render_rule_text(rule)) == rule`. If that holds, text and structure are two spellings of
//! one object; if it does not, one of them is lying about the other, which is exactly what `dsl_body` did for
//! several releases.

use engine::data_types::Value;
use engine::rule_syntax::{parse_rule_text, parse_rules_text, render_rule_text};
use engine::rules::{Action, ComparisonOp, Condition, LogLevel, Rule};

fn pattern(field: &str, op: ComparisonOp, value: Value) -> Condition {
    Condition::Pattern {
        fact_type: "customer".to_string(),
        field: field.to_string(),
        value,
        op,
    }
}

fn rule(condition: Condition) -> Rule {
    Rule {
        id: "high-status".to_string(),
        name: "high-status".to_string(),
        salience: 0,
        no_loop: false,
        lock_on_active: false,
        agenda_group: None,
        condition,
        actions: vec![Action::LogMessage {
            level: LogLevel::Info,
            message: "High status customer detected".to_string(),
        }],
    }
}

#[test]
fn parses_the_body_the_form_renders() {
    let text = "rule \"high-status\"\nwhen customer.status = \"premium\"\nthen log(\"fired\")";

    let parsed = parse_rule_text(text).expect("the form's own output must parse");

    assert_eq!(parsed.name, "high-status");
    assert_eq!(
        parsed.id, "high-status",
        "the id is the name, as a placeholder"
    );
    assert_eq!(
        parsed.condition,
        pattern(
            "status",
            ComparisonOp::Eq,
            Value::String("premium".to_string())
        )
    );
    assert_eq!(
        parsed.actions,
        vec![Action::LogMessage {
            level: LogLevel::Info,
            message: "fired".to_string()
        }]
    );
}

#[test]
fn every_value_kind_survives_a_round_trip() {
    for value in [
        Value::String("premium".to_string()),
        Value::String("42".to_string()),
        Value::Integer(42),
        Value::Integer(-7),
        Value::Float(1.5),
        Value::Float(1.0),
        Value::Bool(true),
        Value::Bool(false),
    ] {
        let original = rule(pattern("field", ComparisonOp::Eq, value.clone()));
        let text = render_rule_text(&original);
        let parsed = parse_rule_text(&text)
            .unwrap_or_else(|error| panic!("{value:?} rendered as {text:?} must parse: {error}"));
        assert_eq!(
            parsed, original,
            "{value:?} did not survive the round trip through {text:?}"
        );
    }
}

#[test]
fn every_operator_survives_a_round_trip() {
    for op in [
        ComparisonOp::Eq,
        ComparisonOp::Ne,
        ComparisonOp::Gt,
        ComparisonOp::Ge,
        ComparisonOp::Lt,
        ComparisonOp::Le,
    ] {
        let original = rule(pattern("score", op, Value::Integer(100)));
        let text = render_rule_text(&original);
        assert_eq!(parse_rule_text(&text).expect(&text), original, "{text}");
    }
}

#[test]
fn a_conjunction_and_a_disjunction_round_trip() {
    let conjunction = rule(Condition::And(vec![
        pattern("status", ComparisonOp::Eq, Value::String("premium".into())),
        pattern("score", ComparisonOp::Ge, Value::Integer(100)),
    ]));
    let text = render_rule_text(&conjunction);
    assert!(text.contains("and customer.score >= 100"), "{text}");
    assert_eq!(parse_rule_text(&text).expect(&text), conjunction);

    let disjunction = rule(Condition::Or(vec![
        pattern("status", ComparisonOp::Eq, Value::String("gold".into())),
        pattern("status", ComparisonOp::Eq, Value::String("platinum".into())),
    ]));
    let text = render_rule_text(&disjunction);
    assert!(text.contains("or customer.status = \"platinum\""), "{text}");
    assert_eq!(parse_rule_text(&text).expect(&text), disjunction);
}

#[test]
fn a_negation_round_trips() {
    let negated = rule(Condition::Not(Box::new(pattern(
        "status",
        ComparisonOp::Eq,
        Value::String("blocked".into()),
    ))));
    let text = render_rule_text(&negated);
    assert_eq!(
        text.lines().nth(1),
        Some("when not(customer.status = \"blocked\")")
    );
    assert_eq!(parse_rule_text(&text).expect(&text), negated);
}

#[test]
fn a_message_with_quotes_and_backslashes_round_trips() {
    let mut original = rule(pattern("a", ComparisonOp::Eq, Value::Integer(1)));
    original.actions = vec![Action::LogMessage {
        level: LogLevel::Info,
        message: "he said \"stop\" \\ now".to_string(),
    }];
    let text = render_rule_text(&original);
    assert_eq!(parse_rule_text(&text).expect(&text), original);
}

#[test]
fn a_quoted_number_stays_a_string() {
    // The form's type selector exists for this: `"42"` is text, `42` is a number, and the parser must not
    // decide otherwise on the author's behalf.
    let quoted =
        parse_rule_text("rule \"r\"\nwhen customer.code = \"42\"\nthen log(\"x\")").unwrap();
    assert_eq!(
        quoted.condition,
        pattern("code", ComparisonOp::Eq, Value::String("42".into()))
    );

    let bare = parse_rule_text("rule \"r\"\nwhen customer.code = 42\nthen log(\"x\")").unwrap();
    assert_eq!(
        bare.condition,
        pattern("code", ComparisonOp::Eq, Value::Integer(42))
    );
}

#[test]
fn comments_and_blank_lines_are_ignored() {
    let text = "// what this rule is for\nrule \"r\"\n\nwhen customer.a = 1\n// and why\n\nthen log(\"x\")\n";
    let parsed = parse_rule_text(text).expect("comments are prose, not syntax");
    assert_eq!(parsed.name, "r");
}

#[test]
fn errors_name_the_line_and_what_was_expected() {
    let cases = [
        (
            "rule \"r\"\nwhen customer.a = 1\nwhen customer.b = 2\nthen log(\"x\")",
            3,
            "one `when`",
        ),
        (
            "rule \"r\"\nand customer.a = 1\nthen log(\"x\")",
            2,
            "before any `when`",
        ),
        (
            "rule \"r\"\nwhen customer.a = 1\nand customer.b = 2\nor customer.c = 3\nthen log(\"x\")",
            4,
            "ambiguous",
        ),
        (
            "rule \"r\"\nwhen customer.a ~= 1\nthen log(\"x\")",
            2,
            "not a comparison operator",
        ),
        (
            "rule \"r\"\nwhen customer.a = unquoted\nthen log(\"x\")",
            2,
            "is not a value",
        ),
        ("rule \"r\"\nwhen customer.a = 1", 0, "no `then` line"),
        ("when customer.a = 1\nthen log(\"x\")", 0, "no `rule"),
        ("rule \"r\"\nthen log(\"x\")", 0, "no `when` line"),
        (
            "rule \"r\"\nwhen customer.a = 1\nthen shout(\"x\")",
            3,
            "not an action this syntax carries",
        ),
        (
            "rule \"r\"\nwhen customer.a = 1\nthen log(\"unterminated)",
            3,
            "unterminated string",
        ),
    ];

    for (text, line, expected) in cases {
        let error = match parse_rule_text(text) {
            Ok(rule) => panic!("{text:?} must be rejected, but it parsed as {rule:?}"),
            Err(error) => error,
        };
        assert_eq!(
            error.line,
            line,
            "{} was reported on line {}, expected {line}: {}",
            text.escape_debug(),
            error.line,
            error.message
        );
        assert!(
            error.message.contains(expected),
            "error should mention {expected:?}, said: {}",
            error.message
        );
    }
}

#[test]
fn a_file_with_several_rules_parses() {
    let text = "rule \"first\"\nwhen customer.a = 1\nthen log(\"one\")\n\n\
                rule \"second\"\nwhen customer.b = 2\nthen log(\"two\")\n";

    let rules = parse_rules_text(text).expect("a rules file holds several rules");

    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].name, "first");
    assert_eq!(rules[1].name, "second");
}

#[test]
fn an_error_in_the_second_rule_names_a_line_in_the_file() {
    let text = "rule \"first\"\nwhen customer.a = 1\nthen log(\"one\")\n\n\
                rule \"second\"\nwhen customer.b ~= 2\nthen log(\"two\")\n";

    let error = parse_rules_text(text).expect_err("the second rule is wrong");

    assert_eq!(error.line, 6, "the line in the file, not in the rule");
}

#[test]
fn a_body_larger_than_the_budget_is_refused_rather_than_parsed() {
    // A rules file is data. Parsing it must not be able to spend unbounded time.
    let huge = format!(
        "rule \"r\"\nwhen customer.a = 1\n// {}\nthen log(\"x\")",
        "x".repeat(70_000)
    );

    let error = parse_rule_text(&huge).expect_err("oversized bodies are refused");

    assert!(error.message.contains("exceeds"), "{}", error.message);
}
