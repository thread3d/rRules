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

#[test]
fn a_session_holds_the_facts_it_is_given_and_runs_the_rules_over_them() {
    // The session used to keep a raw pointer it never dereferenced: `insert_fact` discarded its
    // argument and `execute` answered "Not implemented". It owns the engine now, so both are real.
    let mut engine = EngineBuilder::new().build();
    engine
        .load_rules(RULE_JSON)
        .expect("the rule document loads");

    let mut session = engine.create_session(true, Some("s-1".to_string()));
    assert_eq!(session.id(), Some("s-1"));
    session
        .insert_fact(card())
        .expect("the session accepts a fact");
    assert_eq!(
        session
            .engine()
            .working_memory()
            .get_facts_by_type("scorecard")
            .len(),
        1,
        "the fact is in the session's working memory, not discarded"
    );

    let result = session.execute().expect("the session executes");
    assert!(result.is_success(), "{}", result.message());
    assert!(
        result.message().contains("1 rule(s) fired"),
        "the session runs the rules it holds: {}",
        result.message()
    );

    // The session's own memory reflects the action's effect…
    let score = session
        .engine()
        .working_memory()
        .get_facts_by_type("scorecard")
        .first()
        .and_then(|fact| fact.payload.get("score").cloned());
    assert_eq!(score, Some(engine::data_types::Value::Integer(10)));

    // …and the engine it was created from is untouched, which is what owning a clone means.
    assert!(engine
        .working_memory()
        .get_facts_by_type("scorecard")
        .is_empty());
}

#[test]
fn a_destroyed_session_refuses_to_work() {
    let engine = EngineBuilder::new().build();
    let mut session = engine.create_session(true, None);
    session.destroy();
    assert!(!session.is_valid());
    assert!(session.insert_fact(card()).is_err());
    assert!(session.execute().is_err());
}

#[test]
fn backward_chaining_explains_how_a_field_is_produced() {
    // The goal names a field; the search finds the rule that writes it and checks that rule's
    // conditions against working memory.
    let mut engine = EngineBuilder::new().build();
    engine
        .load_rules(RULE_JSON)
        .expect("the rule document loads");
    engine
        .working_memory_mut()
        .insert_fact(card())
        .expect("the fact is inserted");

    let session = engine.create_session(false, None);
    let trace = engine::backward_chaining::execute_backward(&engine, &session, "scorecard.score");
    assert!(trace.success, "the goal is reachable: {}", trace.explain());
    assert!(
        trace
            .path
            .iter()
            .any(|step| step.rule_id.as_deref() == Some("r1")),
        "the rule that writes the field should appear: {}",
        trace.explain()
    );
    assert!(
        trace.explain().contains("Score plus ten"),
        "the explanation should name the rule: {}",
        trace.explain()
    );
    assert!(
        !trace.fact_matches.is_empty(),
        "the fact that satisfied the condition should be listed: {}",
        trace.explain()
    );
}

#[test]
fn backward_chaining_says_so_when_nothing_can_produce_the_goal() {
    let mut engine = EngineBuilder::new().build();
    engine
        .load_rules(RULE_JSON)
        .expect("the rule document loads");

    let trace = engine.execute_backward("scorecard.unknown_field");
    assert!(!trace.is_success());
    assert!(
        trace.path().contains("No rule can produce"),
        "the explanation should say why: {}",
        trace.path()
    );
}

#[test]
fn backward_chaining_reports_a_goal_that_names_no_field() {
    let mut engine = EngineBuilder::new().build();
    engine
        .load_rules(RULE_JSON)
        .expect("the rule document loads");
    let trace = engine.execute_backward("scorecard");
    assert!(!trace.is_success());
    assert!(
        trace.path().contains("<fact_type>.<field>"),
        "the explanation should say what a goal looks like: {}",
        trace.path()
    );
}

#[test]
fn backward_chaining_follows_a_chain_through_sub_goals() {
    // `r2` produces `total`, but only from a `line` fact that nothing has inserted; `r3` produces the
    // `line` fact. The search must find the supplier rather than give up at the first unmet
    // condition.
    let rules = r#"{
      "rules": [
        {
          "id": "r2", "name": "Compute total", "salience": 0,
          "no_loop": false, "lock_on_active": false,
          "condition": {"Pattern": {"fact_type": "line", "field": "amount", "value": {"Integer": 100}, "op": "Ge"}},
          "actions": [{"UpdateFact": {"id": "order", "fields": {"total": {"Literal": {"Integer": 1}}}}}]
        },
        {
          "id": "r3", "name": "Build a line", "salience": 0,
          "no_loop": false, "lock_on_active": false,
          "condition": {"Pattern": {"fact_type": "order", "field": "id", "value": {"String": "o-1"}, "op": "Eq"}},
          "actions": [{"InsertFact": {"fact_type": "line", "fields": {"amount": {"Integer": 100}}}}]
        }
      ]
    }"#;
    let mut engine = EngineBuilder::new().build();
    engine.load_rules(rules).expect("the rules load");
    // The chain's last link: the order the `Build a line` rule reads.
    let mut order = HashMap::new();
    order.insert(
        "id".to_string(),
        engine::data_types::Value::String("o-1".to_string()),
    );
    engine
        .working_memory_mut()
        .insert_fact(Fact::new("order", order))
        .expect("the order fact is inserted");

    let session = engine.create_session(false, None);
    let trace = engine::backward_chaining::execute_backward(&engine, &session, "order.total");
    let explanation = trace.explain();
    assert!(trace.success, "the chain should be found: {explanation}");
    assert!(
        explanation.contains("Sub-goal: line.amount"),
        "the search should name the condition it had to satisfy: {explanation}"
    );
    assert!(
        explanation.contains("Build a line"),
        "the supplying rule should appear: {explanation}"
    );
}
