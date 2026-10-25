//! Rule-semantics integration tests: firing order, numeric comparison across
//! the value carriers, and the once-per-rule fixpoint contract.
//!
//! These are the assertions behind `specs/requirements.md` ENG-001, ENG-004 and
//! ENG-006. They run against the shared direct evaluator (TREAT) and Rete, which
//! keep separate fixpoint loops, so a divergence between the two shows up here.

use engine::data_types::Value;
use engine::engine::registry::ExecutionEngine;
use engine::fact::Fact;
use engine::rules::{Action, ComparisonOp, Condition, Expression, Rule, RuleSet};
use std::collections::HashMap;

fn scorecard_engine(kind: &str) -> Box<dyn ExecutionEngine> {
    match kind {
        "rete" => Box::new(engine::engine::rete::ReteEngine::new()),
        "treat" => Box::new(engine::engine::treat::TreatEngine::new()),
        "leaps" => Box::new(engine::engine::leaps::LeapsEngine::new()),
        "phreak" => Box::new(engine::engine::phreak::PhreakEngine::new()),
        other => panic!("unknown engine {other}"),
    }
}

fn insert_scorecard(engine: &mut dyn ExecutionEngine, initial: i64) {
    let mut payload = HashMap::new();
    payload.insert("score".to_string(), Value::Integer(initial));
    engine
        .insert_fact(Fact::new("scorecard", payload))
        .expect("insert fact");
}

fn set_score_rule(id: &str, salience: i32, value: i64) -> Rule {
    let mut rule = Rule::new(id, id);
    rule.salience = salience;
    let mut fields = HashMap::new();
    fields.insert(
        "score".to_string(),
        Expression::Literal(Value::Integer(value)),
    );
    rule.actions = vec![Action::UpdateFact {
        id: "scorecard".to_string(),
        fields,
    }];
    rule
}

fn score_of(engine: &dyn ExecutionEngine) -> Option<Value> {
    engine
        .get_facts()
        .into_iter()
        .find(|fact| fact.fact_type_id == "scorecard")
        .and_then(|fact| fact.payload.get("score").cloned())
}

/// ENG-006: a rule with higher salience is considered before a rule with lower
/// salience, regardless of load order.
///
/// Load order is `r1` (salience 0) then `r2` (salience 10); both write the same
/// field. If salience decides the order, `r2` runs first and `r1` writes last,
/// so the surviving value is 1. If load order decided it, the value would be 2.
#[test]
fn eng_006_salience_orders_firing() {
    for kind in ["rete", "treat", "leaps", "phreak"] {
        let mut engine = scorecard_engine(kind);
        insert_scorecard(engine.as_mut(), 0);

        let mut set = RuleSet::new();
        set.add_rule(set_score_rule("r1", 0, 1)).expect("rule r1");
        set.add_rule(set_score_rule("r2", 10, 2)).expect("rule r2");
        engine.load_ruleset(&set).expect("load rules");

        engine.execute();
        assert_eq!(
            score_of(engine.as_ref()),
            Some(Value::Integer(1)),
            "{kind}: higher salience must fire first"
        );
    }
}

/// ENG-001: every rule fires at most once per execution, so two rules that add
/// to the same field compose additively rather than looping.
#[test]
fn eng_001_rules_fire_once_to_fixpoint() {
    for kind in ["rete", "treat", "leaps", "phreak"] {
        let mut engine = scorecard_engine(kind);
        insert_scorecard(engine.as_mut(), 0);

        let mut set = RuleSet::new();
        for (id, delta) in [("r1", 1), ("r2", 2)] {
            let mut rule = Rule::new(id, id);
            let mut fields = HashMap::new();
            fields.insert(
                "score".to_string(),
                Expression::Add(
                    Box::new(Expression::FieldRef {
                        fact_type: "scorecard".to_string(),
                        field: "score".to_string(),
                    }),
                    Box::new(Expression::Literal(Value::Integer(delta))),
                ),
            );
            rule.actions = vec![Action::UpdateFact {
                id: "scorecard".to_string(),
                fields,
            }];
            set.add_rule(rule).expect("rule");
        }
        engine.load_ruleset(&set).expect("load rules");

        let result = engine.execute();
        assert_eq!(result.rules_fired, 2, "{kind}: both rules fire");
        assert_eq!(result.fired_rule_ids.len(), 2, "{kind}: fired ids reported");
        assert_eq!(
            score_of(engine.as_ref()),
            Some(Value::Integer(3)),
            "{kind}: each rule contributes exactly once"
        );
    }
}

/// ENG-004: a `Pattern` condition compares the fact's field against the
/// condition value for every numeric carrier, including FEEL `number` values.
#[test]
fn eng_004_conditions_compare_all_numeric_carriers() {
    let mut payload = HashMap::new();
    payload.insert("age".to_string(), Value::Integer(30));
    payload.insert(
        "ratio".to_string(),
        Value::Number("0.75".parse::<engine::Decimal>().expect("decimal")),
    );
    let fact = Fact::new("customer", payload);
    let facts = vec![&fact];

    let cases = [
        ("age", Value::Integer(18), ComparisonOp::Ge, true),
        ("age", Value::Integer(30), ComparisonOp::Eq, true),
        ("age", Value::Integer(31), ComparisonOp::Lt, true),
        (
            "ratio",
            Value::Number("0.5".parse::<engine::Decimal>().expect("decimal")),
            ComparisonOp::Gt,
            true,
        ),
        ("ratio", Value::Integer(1), ComparisonOp::Lt, true),
        ("ratio", Value::Integer(0), ComparisonOp::Gt, true),
        ("missing", Value::Integer(1), ComparisonOp::Eq, false),
        ("age", Value::String("30".to_string()), ComparisonOp::Eq, false),
    ];

    for (field, value, op, expected) in cases {
        let condition = Condition::Pattern {
            fact_type: "customer".to_string(),
            field: field.to_string(),
            value,
            op: op.clone(),
        };
        assert_eq!(
            condition.evaluate(&facts),
            expected,
            "condition {field} {op:?} did not evaluate to {expected}"
        );
    }
}

/// ENG-003: retraction is idempotent and removes the fact from working memory
/// for every engine, including PHREAK (which previously reported every
/// retraction as unsuccessful).
#[test]
fn eng_003_retraction_is_idempotent_in_every_engine() {
    for kind in ["rete", "treat", "leaps", "phreak"] {
        let mut engine = scorecard_engine(kind);
        let fact = Fact::new("scorecard", HashMap::new());
        let id = fact.id.to_string();
        engine.insert_fact(fact).expect("insert");

        assert!(engine.retract_fact(&id).unwrap(), "{kind}: first retract");
        assert!(!engine.retract_fact(&id).unwrap(), "{kind}: second retract");
        assert!(
            !engine
                .get_facts()
                .iter()
                .any(|f| f.id.to_string() == id),
            "{kind}: working memory must drop the fact"
        );
    }
}
