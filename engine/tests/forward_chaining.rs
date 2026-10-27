//! Forward chaining through the engine's own entry points.
//!
//! `Engine::load_rules` was a placeholder that parsed nothing and returned `Ok(())`, and
//! `Engine::execute_forward` answered "Not implemented" — so the pair could not fire a rule however
//! they were used. Both are real now: rules load from the same JSON the API stores, and the chaining
//! pass runs the shared fixpoint evaluator (salience, agenda groups, once-per-rule firing, the
//! `RRULES_MAX_CHAIN_ITERATIONS` cap) over the engine's working memory.
//!
//! Backward chaining is still a placeholder, and `specs/rule-engine.md` says so rather than implying
//! otherwise.

use engine::environment::EngineBuilder;
use engine::fact::Fact;
use std::collections::HashMap;

/// The same rule shape `engine/tests/cross_engine_suite.rs` uses: `score` starts at 0 and gains 10.
const RULE_JSON: &str = r#"{
  "rules": [
    {
      "id": "r1",
      "name": "Score plus ten",
      "salience": 0,
      "no_loop": false,
      "lock_on_active": false,
      "condition": {"Pattern": {"fact_type": "scorecard", "field": "score", "value": {"Integer": 0}, "op": "Eq"}},
      "actions": [
        {"UpdateFact": {"id": "5ca4d000-0000-0000-0000-000000000001",
                        "fields": {"score": {"Add": [
                          {"FieldRef": {"fact_type": "scorecard", "field": "score"}},
                          {"Literal": {"Integer": 10}}
                        ]}}}}
      ]
    }
  ]
}"#;

fn card() -> Fact {
    let mut payload = HashMap::new();
    payload.insert("score".to_string(), engine::data_types::Value::Integer(0));
    Fact::with_id(
        uuid::Uuid::from_u128(0x5ca4_d000_0000_0000_0000_0000_0000_0001),
        "scorecard",
        payload,
    )
}

#[test]
fn a_rule_loads_from_json_and_fires_through_the_engine() {
    let mut engine = EngineBuilder::new().build();
    engine
        .load_rules(RULE_JSON)
        .expect("the rule document loads");
    engine
        .working_memory_mut()
        .insert_fact(card())
        .expect("the fact is inserted");

    let result = engine.execute_forward();
    assert!(result.is_success(), "{}", result.message());
    assert!(
        result.message().contains("1 rule(s) fired"),
        "the rule should have fired: {}",
        result.message()
    );

    let score = engine
        .working_memory()
        .get_facts_by_type("scorecard")
        .first()
        .and_then(|fact| fact.payload.get("score").cloned());
    assert_eq!(
        score,
        Some(engine::data_types::Value::Integer(10)),
        "the action's effect should be visible in working memory afterwards"
    );
}

#[test]
fn a_single_rule_object_loads_too() {
    // A caller holding one rule passes it bare; a rules directory passes `{ "rules": [...] }`.
    let single = r#"{
      "id": "r1",
      "name": "Score plus ten",
      "salience": 0,
      "no_loop": false,
      "lock_on_active": false,
      "condition": {"Pattern": {"fact_type": "scorecard", "field": "score", "value": {"Integer": 0}, "op": "Eq"}},
      "actions": [{"LogMessage": {"level": "Info", "message": "fired"}}]
    }"#;
    let mut engine = EngineBuilder::new().build();
    engine.load_rules(single).expect("a bare rule loads");
    engine
        .working_memory_mut()
        .insert_fact(card())
        .expect("the fact is inserted");

    let result = engine.execute_forward();
    assert!(
        result.message().contains("1 rule(s) fired"),
        "{}",
        result.message()
    );
}

#[test]
fn invalid_rule_text_is_an_error_rather_than_a_silent_success() {
    let mut engine = EngineBuilder::new().build();
    let error = engine
        .load_rules("this is not JSON")
        .expect_err("invalid rules must not load silently");
    assert!(
        error.contains("JSON"),
        "the error should say what is wrong: {error}"
    );
}

#[test]
fn an_engine_with_no_rules_fires_nothing_and_says_so() {
    let mut engine = EngineBuilder::new().build();
    engine
        .working_memory_mut()
        .insert_fact(card())
        .expect("the fact is inserted");

    let result = engine.execute_forward();
    assert!(result.is_success());
    assert!(
        result.message().contains("0 rule(s) fired"),
        "an empty rule set is a result, not a failure: {}",
        result.message()
    );
}

#[test]
fn the_session_entry_point_and_the_engine_entry_point_agree() {
    // Two ways to run the same engine must not answer differently: both delegate to one loop.
    let mut engine = EngineBuilder::new().build();
    engine
        .load_rules(RULE_JSON)
        .expect("the rule document loads");
    engine
        .working_memory_mut()
        .insert_fact(card())
        .expect("the fact is inserted");

    let session = engine.create_session(false, None);
    let direct = engine::forward_chaining::execute_forward(&mut engine, &session);
    assert_eq!(direct.rules_fired, 1);
}
