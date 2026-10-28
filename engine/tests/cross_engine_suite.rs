//! One functional suite, run against every engine the registry reports.
//!
//! `ENG-002` says the four Rete-family implementations "must pass the same functional tests
//! regardless of algorithm". That was true only by inspection: each engine had its own keystone
//! test with a *copied* fixture, so nothing failed when one engine stopped behaving like the others,
//! and behaviour only one engine implemented was never compared at all.
//!
//! These tests take the engine names from `EngineRegistry::available_engines()` and run the same
//! scenario through each, so an engine that diverges fails here regardless of which one it is, and an
//! engine that is compiled in but broken cannot hide behind its neighbours.

use engine::data_types::Value;
use engine::engine::registry::{EngineRegistry, ExecutionEngine};
use engine::fact::Fact;
use engine::rules::{Action, ComparisonOp, Condition, Expression, Rule, RuleSet};
use std::collections::HashMap;

/// A fixed id, so the rule's `UpdateFact` target and the retraction can name the same fact.
fn card_id() -> uuid::Uuid {
    uuid::Uuid::from_u128(0x5ca4_d000_0000_0000_0000_0000_0000_0001)
}

/// The rule set every engine is judged by: `r1` fires when `score == 0` and adds 10.
fn score_rule_set() -> RuleSet {
    let mut fields = HashMap::new();
    fields.insert(
        "score".to_string(),
        Expression::Add(
            Box::new(Expression::FieldRef {
                fact_type: "scorecard".to_string(),
                field: "score".to_string(),
            }),
            Box::new(Expression::Literal(Value::Integer(10))),
        ),
    );

    let rule = Rule::builder("r1", "Score plus ten")
        .condition(Condition::Pattern {
            fact_type: "scorecard".to_string(),
            field: "score".to_string(),
            value: Value::Integer(0),
            op: ComparisonOp::Eq,
        })
        .action(Action::UpdateFact {
            // The fact is keyed by its id, so the action has to name that same id; the
            // stringified UUID is what `insert_fact` stores it under.
            id: card_id().to_string(),
            fields,
        })
        .build();

    let mut ruleset = RuleSet::new();
    ruleset.add_rules(vec![rule]).expect("valid rule set");
    ruleset
}

fn scorecard(score: i64) -> Fact {
    let mut payload = HashMap::new();
    payload.insert("score".to_string(), Value::Integer(score));
    Fact::with_id(card_id(), "scorecard", payload)
}

/// Every engine name the registry reports, so a newly compiled engine joins the suite automatically.
///
/// `EngineRegistry::new()` is empty — engines are registered explicitly — so this registers the
/// defaults exactly as the server does, and panics if the list comes back empty. A loop over an empty
/// list passes every assertion it contains, which is how five of these tests passed before this guard.
fn engine_names() -> Vec<&'static str> {
    let mut registry = EngineRegistry::new();
    registry.register_default_engines();
    let mut names = registry.available_engines();
    names.sort_unstable();
    assert!(
        !names.is_empty(),
        "the registry reports no engines; every assertion in this suite would be vacuous"
    );
    names
}

fn engine(name: &str) -> Box<dyn ExecutionEngine> {
    let mut registry = EngineRegistry::new();
    registry.register_default_engines();
    registry
        .create_engine_by_name(name)
        .unwrap_or_else(|| panic!("{name} should be constructible"))
        .unwrap_or_else(|error| panic!("{name} should construct cleanly: {error}"))
}

fn score_of(engine: &dyn ExecutionEngine) -> Option<Value> {
    engine
        .get_facts()
        .into_iter()
        .find(|fact| fact.id == card_id())
        .and_then(|fact| fact.payload.get("score").cloned())
}

#[test]
fn every_engine_reports_the_documented_names() {
    let names = engine_names();
    for expected in ["rete", "treat", "leaps", "phreak"] {
        assert!(
            names.contains(&expected),
            "{expected} should be compiled in and addressable; registry reports {names:?}"
        );
    }
}

#[test]
fn an_unknown_engine_name_is_an_error() {
    let mut registry = EngineRegistry::new();
    registry.register_default_engines();
    if let Some(Ok(_)) = registry.create_engine_by_name("no-such-engine") {
        panic!("an unknown name must not construct an engine");
    }
}

#[test]
fn every_engine_fires_the_same_rule() {
    for name in engine_names() {
        let mut engine = engine(name);
        engine
            .load_ruleset(&score_rule_set())
            .unwrap_or_else(|error| panic!("{name} should load a rule set: {error}"));
        engine
            .insert_fact(scorecard(0))
            .unwrap_or_else(|error| panic!("{name} should insert a fact: {error}"));

        let result = engine.execute();
        assert!(
            result.success,
            "{name} reported failure: {}",
            result.message
        );
        assert!(result.rules_fired >= 1, "{name} should fire the rule");
        assert_eq!(
            score_of(engine.as_ref()),
            Some(Value::Integer(10)),
            "{name} should have applied `score := score + 10`"
        );
    }
}

#[test]
fn every_engine_retracts_a_fact_and_reports_the_second_attempt_as_a_no_op() {
    for name in engine_names() {
        let mut engine = engine(name);
        engine
            .load_ruleset(&score_rule_set())
            .unwrap_or_else(|error| panic!("{name} should load a rule set: {error}"));
        engine
            .insert_fact(scorecard(0))
            .unwrap_or_else(|error| panic!("{name} should insert a fact: {error}"));

        let id = card_id().to_string();
        assert_eq!(
            engine.retract_fact(&id),
            Ok(true),
            "{name} should retract a fact it holds"
        );
        assert!(
            score_of(engine.as_ref()).is_none(),
            "{name} should have removed the fact from working memory"
        );
        assert_eq!(
            engine.retract_fact(&id),
            Ok(false),
            "{name} should report a second retract as a no-op, not an error"
        );
        assert_eq!(
            engine.retract_fact("never-existed"),
            Ok(false),
            "{name} should report an unknown id as a no-op"
        );
    }
}

#[test]
fn every_engine_resets_to_an_empty_state() {
    for name in engine_names() {
        let mut engine = engine(name);
        engine
            .load_ruleset(&score_rule_set())
            .unwrap_or_else(|error| panic!("{name} should load a rule set: {error}"));
        engine
            .insert_fact(scorecard(0))
            .unwrap_or_else(|error| panic!("{name} should insert a fact: {error}"));

        engine.reset();
        assert!(
            engine.get_facts().is_empty(),
            "{name} should hold no facts after reset"
        );
        let result = engine.execute();
        assert_eq!(
            result.rules_fired, 0,
            "{name} should not fire anything after a reset"
        );
    }
}

#[test]
fn every_engine_is_freshly_constructed_per_execution() {
    // Two engines built from the same registry share no state: loading rules into one cannot be
    // observed in the other.
    for name in engine_names() {
        let mut registry = EngineRegistry::new();
        registry.register_default_engines();
        let mut first = registry
            .create_engine_by_name(name)
            .expect("known name")
            .expect("constructs");
        let mut second = registry
            .create_engine_by_name(name)
            .expect("known name")
            .expect("constructs");

        first
            .load_ruleset(&score_rule_set())
            .unwrap_or_else(|error| panic!("{name} should load a rule set: {error}"));
        assert!(
            second.get_facts().is_empty(),
            "{name}: a second instance must start empty"
        );
        assert_eq!(
            second.execute().rules_fired,
            0,
            "{name}: a second instance must not inherit the first one's rules"
        );
    }
}

#[test]
fn every_engine_holds_two_facts_of_the_same_type() {
    // Inserting a second fact of a type the engine has already seen must not be swallowed. (The TREAT
    // network's join cache had exactly that defect — keyed by node alone, so the second fact found the
    // first one's entry and was never propagated — but execution goes through the shared evaluator
    // rather than that network, so the cache is tested where it lives: `treat::mod`'s unit tests.)
    for name in engine_names() {
        let mut engine = engine(name);
        engine
            .load_ruleset(&score_rule_set())
            .unwrap_or_else(|error| panic!("{name} should load a rule set: {error}"));

        let mut first = HashMap::new();
        first.insert("score".to_string(), Value::Integer(0));
        engine
            .insert_fact(Fact::with_id(uuid::Uuid::from_u128(1), "scorecard", first))
            .unwrap_or_else(|error| panic!("{name} should insert the first fact: {error}"));

        let mut second = HashMap::new();
        second.insert("score".to_string(), Value::Integer(0));
        engine
            .insert_fact(Fact::with_id(uuid::Uuid::from_u128(2), "scorecard", second))
            .unwrap_or_else(|error| panic!("{name} should insert the second fact: {error}"));

        let facts = engine.get_facts();
        assert_eq!(
            facts.len(),
            2,
            "{name} should hold both facts, not just the one that reached the cache first"
        );
        assert!(
            facts.iter().any(|fact| fact.id == uuid::Uuid::from_u128(1)),
            "{name} lost the first fact"
        );
        assert!(
            facts.iter().any(|fact| fact.id == uuid::Uuid::from_u128(2)),
            "{name} lost the second fact"
        );
    }
}
