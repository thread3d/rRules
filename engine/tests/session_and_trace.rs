//! Two settings that were stored and never read, now that they do something.
//!
//! `EngineBuilder::enable_debug_trace` asked for a trace that the evaluator was never told to collect,
//! and `Session`'s `stateful` flag was never consulted — so both were words in a signature. These tests
//! are what stops them going back to being words.

use engine::data_types::Value;
use engine::environment::{EngineBuilder, Session};
use engine::fact::Fact;
use std::collections::HashMap;

const RULES: &str = r#"{"rules": [{
    "id": "gold",
    "name": "Gold customers get a discount",
    "salience": 0,
    "no_loop": false,
    "lock_on_active": false,
    "condition": {"Pattern": {"fact_type": "customer", "field": "tier",
                              "value": {"String": "Gold"}, "op": "Eq"}},
    "actions": [{"LogMessage": {"level": "Info", "message": "gold"}}]
}]}"#;

fn gold_customer() -> Fact {
    let mut payload = HashMap::new();
    payload.insert("tier".to_string(), Value::String("Gold".to_string()));
    Fact::new("customer", payload)
}

#[test]
fn tracing_is_off_until_it_is_asked_for() {
    let mut engine = EngineBuilder::new().build();
    engine.load_rules(RULES).expect("rules load");
    engine
        .insert_fact_for_simulation(gold_customer())
        .expect("fact");
    engine.execute_forward();

    assert!(
        engine.last_trace().is_none(),
        "a default engine does not record a trace"
    );
}

#[test]
fn asking_for_a_trace_produces_one() {
    let mut engine = EngineBuilder::new().enable_debug_trace(true).build();
    engine.load_rules(RULES).expect("rules load");
    engine
        .insert_fact_for_simulation(gold_customer())
        .expect("fact");
    let result = engine.execute_forward();

    assert!(
        result.is_success(),
        "the run succeeded: {}",
        result.message()
    );
    let trace = engine
        .last_trace()
        .expect("`enable_debug_trace(true)` must produce a trace");
    assert!(
        !trace.events.is_empty(),
        "the trace records the run: {trace:?}"
    );
}

#[test]
fn a_stateless_session_clears_its_facts_after_running() {
    let mut engine = EngineBuilder::new().build();
    engine.load_rules(RULES).expect("rules load");
    let mut session = Session::new(&engine, false, None);
    session.insert_fact(gold_customer()).expect("fact");

    session.execute().expect("first execute");

    assert!(
        session.engine().working_memory().get_all_facts().is_empty(),
        "a single-use session must not keep the facts it ran on"
    );
}

#[test]
fn a_stateful_session_keeps_its_facts() {
    let mut engine = EngineBuilder::new().build();
    engine.load_rules(RULES).expect("rules load");
    let mut session = Session::new(&engine, true, None);
    session.insert_fact(gold_customer()).expect("fact");

    session.execute().expect("first execute");
    assert_eq!(
        session.engine().working_memory().get_all_facts().len(),
        1,
        "a long-lived session keeps what it was given"
    );

    // And the kept fact is still there for the next run.
    session.execute().expect("second execute");
    assert_eq!(session.engine().working_memory().get_all_facts().len(), 1);
}
