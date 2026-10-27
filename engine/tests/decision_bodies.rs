//! Decision bodies that express themselves as a call.
//!
//! DMN lets a decision invoke a model instead of holding a literal expression, and DMN 1.5 renamed
//! the business knowledge model's body element from `<encapsulatedLogic>` to `<functionDefinition>`.
//! Both were being dropped by the parser without a word: the decision kept no expression, and the
//! BKM lost its body. These tests pin the forms the parser now reads.

use engine::dmn::evaluator::{DecisionEvaluator, EvaluateRequest, SharedDmnModel};
use engine::dmn::parser::parse_dmn_with_cache;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

fn evaluator(xml: &str) -> DecisionEvaluator {
    let (model, cache) = parse_dmn_with_cache(xml).expect("the document parses");
    DecisionEvaluator::new(Arc::new(RwLock::new(SharedDmnModel::new(model, cache))))
}

fn evaluate(xml: &str, decision: &str) -> HashMap<String, engine::data_types::Value> {
    evaluator(xml)
        .evaluate(
            decision,
            EvaluateRequest {
                inputs: HashMap::new(),
                trace: false,
            },
        )
        .unwrap_or_else(|error| panic!("{decision} should evaluate: {error}"))
        .outputs
}

const BKM_WITH_FUNCTION_DEFINITION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20180913/MODEL/" id="bkm-doc" name="BKM"
             namespace="http://example.com/bkm">
  <businessKnowledgeModel id="double_model" name="double">
    <variable name="double" typeRef="feel:number"/>
    <functionDefinition kind="FEEL">
      <formalParameter name="value" typeRef="feel:number"/>
      <literalExpression><text>value * 2</text></literalExpression>
    </functionDefinition>
  </businessKnowledgeModel>
  <decision id="doubled" name="Doubled">
    <variable name="result" typeRef="feel:number"/>
    <literalExpression><text>double(21)</text></literalExpression>
  </decision>
</definitions>"#;

const DECISION_WITH_INVOCATION: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20180913/MODEL/" id="inv-doc" name="Invocation"
             namespace="http://example.com/inv">
  <businessKnowledgeModel id="triple_model" name="triple">
    <variable name="triple" typeRef="feel:number"/>
    <functionDefinition kind="FEEL">
      <formalParameter name="value" typeRef="feel:number"/>
      <literalExpression><text>value * 3</text></literalExpression>
    </functionDefinition>
  </businessKnowledgeModel>
  <decision id="tripled" name="Tripled">
    <variable name="result" typeRef="feel:number"/>
    <invocation href="#triple_model">
      <literalExpression><text>triple(14)</text></literalExpression>
    </invocation>
  </decision>
</definitions>"##;

#[test]
fn a_bkm_body_written_as_function_definition_is_read() {
    let outputs = evaluate(BKM_WITH_FUNCTION_DEFINITION, "doubled");
    assert_eq!(
        outputs.get("result"),
        Some(&engine::data_types::Value::Number(engine::Decimal::from(
            42
        ))),
        "the BKM's body should have been read from <functionDefinition>"
    );
}

#[test]
fn a_decision_written_as_an_invocation_is_read() {
    let outputs = evaluate(DECISION_WITH_INVOCATION, "tripled");
    assert_eq!(
        outputs.get("result"),
        Some(&engine::data_types::Value::Number(engine::Decimal::from(
            42
        ))),
        "the invocation's expression should be the decision's body"
    );
}

const INVOCATION_WITH_BINDINGS: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20180913/MODEL/" id="bind-doc" name="Bindings"
             namespace="http://example.com/bind">
  <businessKnowledgeModel id="quadruple_model" name="quadruple">
    <variable name="quadruple" typeRef="feel:number"/>
    <functionDefinition kind="FEEL">
      <formalParameter name="value" typeRef="feel:number"/>
      <literalExpression><text>value * 4</text></literalExpression>
    </functionDefinition>
  </businessKnowledgeModel>
  <decision id="quadrupled" name="Quadrupled">
    <variable name="result" typeRef="feel:number"/>
    <invocation href="#quadruple_model">
      <binding>
        <parameter name="value" typeRef="feel:number"/>
        <literalExpression><text>10</text></literalExpression>
      </binding>
    </invocation>
  </decision>
</definitions>"##;

const FOR_BODY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20180913/MODEL/" id="for-doc" name="For"
             namespace="http://example.com/for">
  <decision id="squares" name="Squares">
    <variable name="result" typeRef="feel:list"/>
    <for>
      <in>
        <context>
          <contextEntry>
            <variable name="x" typeRef="feel:number"/>
            <literalExpression><text>[1, 2, 3]</text></literalExpression>
          </contextEntry>
        </context>
      </in>
      <return><literalExpression><text>x * 2</text></literalExpression></return>
    </for>
  </decision>
</definitions>"#;

#[test]
fn a_decision_written_as_an_invocation_with_bindings_is_read() {
    let outputs = evaluate(INVOCATION_WITH_BINDINGS, "quadrupled");
    assert_eq!(
        outputs.get("result"),
        Some(&engine::data_types::Value::Number(engine::Decimal::from(
            40
        ))),
        "the binding should become the call's argument"
    );
}

#[test]
fn a_decision_written_as_a_for_body_is_read() {
    let outputs = evaluate(FOR_BODY, "squares");
    // The list's shape is the FEEL evaluator's to decide; what matters here is that the body was read
    // at all rather than dropped, so the decision produced its variable.
    assert_eq!(
        outputs.get("result"),
        Some(&engine::data_types::Value::Array(vec![
            engine::data_types::Value::Number(engine::Decimal::from(2)),
            engine::data_types::Value::Number(engine::Decimal::from(4)),
            engine::data_types::Value::Number(engine::Decimal::from(6)),
        ])),
        "the for body should double each element, got {outputs:?}"
    );
}
