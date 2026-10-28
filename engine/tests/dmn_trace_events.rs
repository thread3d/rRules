//! What the DMN evaluation trace actually records.
//!
//! The module's own description claimed events for `evaluate_table`, `match_rule`, `aggregate` and
//! `resolve_reference`, served by a `GET /v1/decisions/{id}/trace` route. Two of those events exist, the
//! other two were never emitted, and the route was never written. These tests pin the real behaviour, so
//! the next reader can believe the description.

use engine::data_types::Value;
use engine::dmn::evaluator::{DecisionEvaluator, EvaluateRequest, SharedDmnModel};
use engine::dmn::parser::parse_dmn;
use std::collections::HashMap;

const SINGLE_DECISION: &str = r#"<?xml version="1.0"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20230324/MODEL/" id="d" name="single">
  <decision id="greeting" name="Greeting">
    <decisionTable id="t" hitPolicy="UNIQUE">
      <input id="i1" label="name"><inputExpression id="e1" typeRef="string">
        <text>name</text></inputExpression></input>
      <output id="o1" name="greeting" typeRef="string"/>
      <rule id="r1">
        <inputEntry id="ie1"><text>"Ada"</text></inputEntry>
        <outputEntry id="oe1"><text>"hello"</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#;

fn evaluate(trace: bool) -> engine::dmn::evaluator::EvaluateResult {
    let model = parse_dmn(SINGLE_DECISION).expect("the fixture parses");
    let shared = SharedDmnModel::new(model, engine::dmn::feel::FeelAstCache::new());
    let evaluator = DecisionEvaluator::new(std::sync::Arc::new(std::sync::RwLock::new(shared)));
    let mut inputs = HashMap::new();
    inputs.insert("name".to_string(), Value::String("Ada".to_string()));
    evaluator
        .evaluate("greeting", EvaluateRequest { inputs, trace })
        .expect("evaluation succeeds")
}

#[test]
fn a_single_decision_evaluation_records_the_table_and_the_scope_merge() {
    // Measured, not assumed: even with no upstream decisions to resolve, the target decision's table
    // evaluation and its scope merge are recorded, and the `evaluate_table` event carries the output it
    // produced. The first version of this test asserted "no events" and was wrong — which is the reason
    // it exists.
    let result = evaluate(true);
    let trace = result
        .trace
        .expect("a trace is returned when it is asked for");
    let kinds: Vec<&str> = trace.events.iter().map(|e| e.event_type.as_str()).collect();
    assert_eq!(
        kinds,
        ["evaluate_table", "merge_scope"],
        "recorded: {:?}",
        trace.events
    );
    assert_eq!(
        trace.events[0].output,
        Some(Value::String("hello".to_string())),
        "the table event carries the value it produced"
    );
    assert!(trace.duration_ms() >= 0.0);
}

#[test]
fn no_match_rule_or_aggregate_event_is_ever_emitted() {
    // The module's description used to list them. Nothing emits them, so a consumer waiting for them
    // waits forever; the test states the vocabulary that exists.
    let result = evaluate(true);
    let trace = result.trace.expect("trace");
    for event in &trace.events {
        assert!(
            !matches!(
                event.event_type.as_str(),
                "match_rule" | "aggregate" | "resolve_reference"
            ),
            "'{}' was never emitted before and is not emitted now",
            event.event_type
        );
    }
}

#[test]
fn asking_without_trace_records_nothing() {
    let result = evaluate(false);
    assert!(
        result.trace.is_none(),
        "a trace is only built when requested"
    );
}
