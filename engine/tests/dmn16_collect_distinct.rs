//! `COLLECT` with the DMN 1.6 `DISTINCT` aggregator.
//!
//! **The semantics tested here are derived, not copied.** The vendored TCK corpus is DMN 1.5, so nothing in
//! this repository can check a reading of the 1.6 specification text. What these tests assert follows from two
//! things this codebase already does and already tests: `COLLECT` returns the matching rules' outputs in rule
//! order, and FEEL's `distinct values()` removes duplicates by **FEEL equality**. `DISTINCT` is that
//! deduplication applied to the collected outputs, keeping the first occurrence.
//!
//! Stating the provenance matters: a reader deserves to know which of these is a spec claim and which is a
//! consistent extension of 1.5 behaviour. This file is the latter, and the module comment is the difference
//! between a documented decision and an invented one.

use engine::data_types::Value;
use engine::dmn::evaluator::{DecisionEvaluator, EvaluateRequest, SharedDmnModel};
use engine::dmn::model::BuiltinAggregator;
use engine::dmn::parser::parse_dmn;
use std::collections::HashMap;

/// A COLLECT table whose two rules both match `tier = "gold"` and output `name`.
fn table_with(output_entries: &[&str]) -> String {
    let rules = output_entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            format!(
                r#"<rule id="r{index}">
                     <inputEntry id="i{index}"><text>"gold"</text></inputEntry>
                     <outputEntry id="o{index}"><text>{entry}</text></outputEntry>
                   </rule>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<?xml version="1.0"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20230324/MODEL/" id="d" name="collect-distinct">
  <decision id="tier-names" name="Tier names">
    <decisionTable id="t" hitPolicy="COLLECT" aggregation="DISTINCT">
      <input id="in1" label="tier"><inputExpression id="ie1" typeRef="string">
        <text>tier</text></inputExpression></input>
      <output id="out1" name="name" typeRef="string"/>
      {rules}
    </decisionTable>
  </decision>
</definitions>"#
    )
}

fn evaluate(source: &str) -> Value {
    let model = parse_dmn(source).expect("the fixture parses");
    let evaluator = DecisionEvaluator::new(std::sync::Arc::new(std::sync::RwLock::new(
        SharedDmnModel::new(model, engine::dmn::feel::FeelAstCache::new()),
    )));
    let mut inputs = HashMap::new();
    inputs.insert("tier".to_string(), Value::String("gold".to_string()));
    let result = evaluator
        .evaluate(
            "tier-names",
            EvaluateRequest {
                inputs,
                trace: false,
            },
        )
        .expect("the decision evaluates");
    result.outputs.get("name").cloned().unwrap_or(Value::Null)
}

#[test]
fn duplicate_outputs_are_collapsed_and_order_is_kept() {
    let value = evaluate(&table_with(&["\"a\"", "\"b\"", "\"a\""]));

    assert_eq!(
        value,
        Value::Array(vec![
            Value::String("a".to_string()),
            Value::String("b".to_string()),
        ]),
        "first occurrence of each value, in rule order"
    );
}

#[test]
fn every_output_the_same_leaves_one_element() {
    let value = evaluate(&table_with(&["\"same\"", "\"same\"", "\"same\""]));

    assert_eq!(value, Value::Array(vec![Value::String("same".to_string())]));
}

#[test]
fn distinct_outputs_are_untouched() {
    let value = evaluate(&table_with(&["\"a\"", "\"b\""]));

    assert_eq!(
        value,
        Value::Array(vec![
            Value::String("a".to_string()),
            Value::String("b".to_string()),
        ])
    );
}

#[test]
fn equality_is_feel_equality_not_rust_equality() {
    // FEEL says `1 = 1.0`, so a NUMBER list collapses them; and `1` is not `"1"`, so a string stays. This is
    // the same comparison `distinct values()` uses, which is why the two cannot disagree.
    let value = evaluate(&table_with(&["1", "1.0", "\"1\""]));

    assert_eq!(
        value,
        Value::Array(vec![
            Value::Number(engine::Decimal::from(1)),
            Value::String("1".to_string()),
        ]),
        "1 and 1.0 are one value; the string \"1\" is another"
    );
}

#[test]
fn no_matching_rule_answers_an_empty_list() {
    // Unlike SUM/MIN/MAX (null for an empty set), DISTINCT's result stays a list, so a consumer can iterate
    // it without a type check.
    let source = table_with(&["\"a\""]).replace("\"gold\"", "\"plat\"");
    let value = evaluate(&source);

    assert_eq!(value, Value::Array(Vec::new()));
}

#[test]
fn the_document_round_trips_through_the_exporter() {
    // The aggregator has to survive XML round-tripping, or a stored 1.6 document would lose it on save.
    let model = parse_dmn(&table_with(&["\"a\"", "\"a\""])).expect("parses");
    let table = model
        .definitions
        .children
        .iter()
        .find_map(|child| match child {
            engine::dmn::model::DefinitionsChild::Decision(decision) => {
                decision.decision_table.clone()
            }
            _ => None,
        })
        .expect("the fixture has a table");
    assert_eq!(table.aggregation, Some(BuiltinAggregator::Distinct));

    let exported = engine::dmn::exporter::export_dmn(&model).expect("the model exports");
    assert!(
        exported.contains("aggregation=\"DISTINCT\""),
        "the exporter must write the aggregator back: {exported}"
    );

    let reparsed = parse_dmn(&exported).expect("the export parses");
    let table = reparsed
        .definitions
        .children
        .iter()
        .find_map(|child| match child {
            engine::dmn::model::DefinitionsChild::Decision(decision) => {
                decision.decision_table.clone()
            }
            _ => None,
        })
        .expect("the re-parsed fixture has a table");
    assert_eq!(table.aggregation, Some(BuiltinAggregator::Distinct));
}
