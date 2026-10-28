//! Plan 29-02 Task 2: DMN parser integration tests (import corpus).
//!
//! Embeds the six fixture `.dmn` files under `fixtures/dmn/` via `include_str!`
//! and exercises the four Phase 29 success criteria against the canonical IR:
//!
//! 1. Multi-version import (DMN 1.1 / 1.3 / 1.5) — `test_parse_dmn_1_5_imports`,
//!    `test_parse_dmn_1_3_namespace`, `test_parse_dmn_1_1_namespace`.
//! 2. XXE rejection + CDATA preservation — `test_xxe_rejected`,
//!    `test_cdata_description_preserved`.
//! 3. `hitPolicy` absent → `HitPolicy::Unique` — `test_no_hitpolicy_defaults_to_unique`.
//! 4. Clause 7 item definitions + element order — `test_item_definition_clause_7`,
//!    `test_element_order_preserved`.
//!
//! Filter: `cargo test -p engine --test dmn_parser`

use engine::dmn::model::{
    AllowedValues, BuiltinAggregator, DefinitionsChild, DmnParseError, HitPolicy,
};
use engine::dmn::parser::parse_dmn;

const DMN_1_5: &str = include_str!("fixtures/dmn/decision_1_5.dmn");
const DMN_1_3: &str = include_str!("fixtures/dmn/decision_1_3.dmn");
const DMN_1_2: &str = include_str!("fixtures/dmn/decision_1_2.dmn");
const DMN_1_1: &str = include_str!("fixtures/dmn/decision_1_1.dmn");
const DMN_NO_HITPOLICY: &str = include_str!("fixtures/dmn/no_hitpolicy.dmn");
const DMN_CDATA: &str = include_str!("fixtures/dmn/cdata_description.dmn");
const DMN_XXE: &str = include_str!("fixtures/dmn/xxe_payload.dmn");

/// Success criterion 1 (DMN 1.5 import) + element-order/ID/annotation
/// preservation. Parses the 1.5 fixture and asserts the IR shape, clause
/// counts, rule entry text, description preservation, and element order.
#[test]
fn test_parse_dmn_1_5_imports() {
    let model = parse_dmn(DMN_1_5).expect("DMN 1.5 should parse");
    let children = &model.definitions.children;
    assert!(
        children.len() >= 4,
        "expected >= 4 top-level children, got {}",
        children.len()
    );

    // First child: ItemDefinition name "Age".
    let first = &children[0];
    let item = match first {
        DefinitionsChild::ItemDefinition(i) => i,
        other => panic!("expected first child to be ItemDefinition, got {:?}", other),
    };
    assert_eq!(item.name, "Age");

    // Second child: Decision id "_decEligibility".
    let dec = match &children[1] {
        DefinitionsChild::Decision(d) => d,
        other => panic!("expected second child to be Decision, got {:?}", other),
    };
    assert_eq!(dec.id, "_decEligibility");

    // Decision table shape: UNIQUE, 2 inputs, 1 output, 2 rules.
    let dt = dec
        .decision_table
        .as_ref()
        .expect("decision must have a decision table");
    assert_eq!(dt.hit_policy, HitPolicy::Unique);
    assert_eq!(dt.input_clauses.len(), 2);
    assert_eq!(dt.output_clauses.len(), 1);
    assert_eq!(dt.rules.len(), 2);

    // First rule's first output entry text == "\"Eligible\"" (with quotes).
    let r0 = &dt.rules[0];
    assert_eq!(r0.output_entries[0].text.content, "\"Eligible\"");

    // At least one rule has a description.
    assert!(
        dt.rules.iter().any(|r| r.description.is_some()),
        "at least one rule must have a description"
    );
}

/// Every DMN fixture the project ships declares a namespace the parser recognises, including the
/// `20230324` URI that was missing from the allow-list until 9.0.66.
#[test]
fn every_shipped_fixture_declares_a_recognised_namespace() {
    for (version, source) in [
        ("1.1", DMN_1_1),
        ("1.2", DMN_1_2),
        ("1.3", DMN_1_3),
        ("1.5", DMN_1_5),
    ] {
        let model = parse_dmn(source).unwrap_or_else(|e| panic!("DMN {version} should parse: {e}"));
        assert!(
            model.namespace_is_recognised(),
            "DMN {version} fixture declares {:?}, which the parser does not recognise",
            model.source_namespace
        );
    }
}

/// An unrecognised root namespace parses anyway — foreign and vendor namespaces are how real documents
/// travel, and refusing them would refuse valid models. It is reported, not rejected.
#[test]
fn a_foreign_namespace_parses_and_is_reported_as_unrecognised() {
    let source = DMN_1_5.replace(
        "https://www.omg.org/spec/DMN/20230324/MODEL/",
        "https://example.com/vendor/model/",
    );
    let model = parse_dmn(&source).expect("a foreign namespace must still parse");
    assert!(!model.namespace_is_recognised());
    assert_eq!(
        model.source_namespace.as_deref(),
        Some("https://example.com/vendor/model/")
    );
}

/// PRIORITY hit policy + enumeration `allowedValues` on the output clause, on the DMN **1.2**
/// namespace fixture. This test used to be called `test_parse_dmn_1_3_namespace` while parsing the
/// `20180521` document — `20200524` is 1.3, `20180521` is 1.2 (spec, gap 20).
#[test]
fn test_parse_dmn_1_2_namespace() {
    let model = parse_dmn(DMN_1_2).expect("DMN 1.2 should parse");
    assert_eq!(
        model.source_namespace.as_deref(),
        Some("https://www.omg.org/spec/DMN/20180521/MODEL/")
    );
    let dec = match &model.definitions.children[1] {
        DefinitionsChild::Decision(d) => d,
        other => panic!("expected second child to be Decision, got {:?}", other),
    };
    let dt = dec.decision_table.as_ref().expect("decision table present");
    assert_eq!(dt.hit_policy, HitPolicy::Priority);
    let oc = &dt.output_clauses[0];
    assert!(
        matches!(oc.output_values, Some(AllowedValues::Enumeration(_))),
        "expected Enumeration output_values, got {:?}",
        oc.output_values
    );
}

/// Success criterion 1 (DMN 1.1 namespace acceptance) + COLLECT + SUM
/// aggregator parsing.
#[test]
fn test_parse_dmn_1_1_namespace() {
    let model = parse_dmn(DMN_1_1).expect("DMN 1.1 should parse");
    let dec = match &model.definitions.children[1] {
        DefinitionsChild::Decision(d) => d,
        other => panic!("expected second child to be Decision, got {:?}", other),
    };
    let dt = dec.decision_table.as_ref().expect("decision table present");
    assert_eq!(dt.hit_policy, HitPolicy::Collect);
    assert_eq!(dt.aggregation, Some(BuiltinAggregator::Sum));
}

/// Success criterion 3: a decision table with NO `hitPolicy` attribute
/// defaults to `HitPolicy::Unique` (DMN-04).
#[test]
fn test_no_hitpolicy_defaults_to_unique() {
    let model = parse_dmn(DMN_NO_HITPOLICY).expect("no-hitpolicy fixture should parse");
    let dec = match &model.definitions.children[0] {
        DefinitionsChild::Decision(d) => d,
        other => panic!("expected first child to be Decision, got {:?}", other),
    };
    let dt = dec.decision_table.as_ref().expect("decision table present");
    assert_eq!(dt.hit_policy, HitPolicy::Unique);
}

/// Success criterion 2 (CDATA half): CDATA sections in a decision description
/// and a rule description are preserved with `is_cdata == true` and the
/// content is intact (including the literal `&` that CDATA protects).
#[test]
fn test_cdata_description_preserved() {
    let model = parse_dmn(DMN_CDATA).expect("CDATA fixture should parse");
    let dec = match &model.definitions.children[0] {
        DefinitionsChild::Decision(d) => d,
        other => panic!("expected first child to be Decision, got {:?}", other),
    };
    let desc = dec
        .description
        .as_ref()
        .expect("decision has a description");
    assert!(desc.is_cdata, "decision description must be CDATA-flagged");
    assert!(
        desc.content.contains("age >= 18"),
        "description content must contain 'age >= 18', got {:?}",
        desc.content
    );

    let dt = dec.decision_table.as_ref().expect("decision table present");
    let cdata_rule = dt
        .rules
        .iter()
        .find(|r| r.description.as_ref().is_some_and(|d| d.is_cdata))
        .expect("at least one rule must have a CDATA description");
    let rdesc = cdata_rule.description.as_ref().unwrap();
    assert!(
        rdesc.content.contains("age >= 18"),
        "rule description must contain 'age >= 18', got {:?}",
        rdesc.content
    );
}

/// Success criterion 2 (XXE half): a payload containing
/// `<!ENTITY xxe SYSTEM "file:///etc/passwd">` is rejected with
/// `DmnParseError::XxeRejected` and no file is read.
#[test]
fn test_xxe_rejected() {
    let res = parse_dmn(DMN_XXE);
    assert!(
        matches!(res, Err(DmnParseError::XxeRejected(_))),
        "expected Err(XxeRejected), got {:?}",
        res
    );
}

/// DMN-05 (Clause 7): the `ItemDefinition` "Age" has `is_collection == false`
/// and `allowed_values == Some(AllowedValues::UnaryTests(s))` where the
/// unary-tests text contains `>= 18` (entity-decoded from `&gt;= 18`).
#[test]
fn test_item_definition_clause_7() {
    let model = parse_dmn(DMN_1_5).expect("DMN 1.5 should parse");
    let item = match &model.definitions.children[0] {
        DefinitionsChild::ItemDefinition(i) => i,
        other => panic!("expected first child to be ItemDefinition, got {:?}", other),
    };
    assert_eq!(item.name, "Age");
    assert!(!item.is_collection, "is_collection must be false");
    let av = item
        .allowed_values
        .as_ref()
        .expect("itemDefinition must have allowed_values");
    match av {
        AllowedValues::UnaryTests(s) => {
            assert!(
                s.contains(">= 18"),
                "unary-tests must contain '>= 18', got {:?}",
                s
            );
        }
        other => panic!("expected UnaryTests, got {:?}", other),
    }
}

/// Success criterion 1 / Pitfall 11: the `definitions.children` variant
/// sequence matches the document order exactly —
/// `[ItemDefinition, Decision, TextAnnotation, Association]`.
#[test]
fn test_element_order_preserved() {
    let model = parse_dmn(DMN_1_5).expect("DMN 1.5 should parse");
    let seq: Vec<&str> = model
        .definitions
        .children
        .iter()
        .map(|c| match c {
            DefinitionsChild::ItemDefinition(_) => "ItemDefinition",
            DefinitionsChild::Decision(_) => "Decision",
            DefinitionsChild::InputData(_) => "InputData",
            DefinitionsChild::BusinessKnowledgeModel(_) => "BusinessKnowledgeModel",
            DefinitionsChild::DecisionService(_) => "DecisionService",
            DefinitionsChild::KnowledgeSource(_) => "KnowledgeSource",
            DefinitionsChild::TextAnnotation(_) => "TextAnnotation",
            DefinitionsChild::Association(_) => "Association",
        })
        .collect();
    assert_eq!(
        seq,
        [
            "ItemDefinition",
            "Decision",
            "TextAnnotation",
            "Association"
        ],
        "element order must be preserved (Pitfall 11)"
    );
}
