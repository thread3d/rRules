//! Plan 29-03 Task 2: Round-trip DOM-compare tests against the import corpus.
//! Filter: `cargo test -p engine --test dmn_round_trip`

use std::path::Path;

use engine::dmn::exporter::export_dmn;
use engine::dmn::model::{DefinitionsChild, HitPolicy};
use engine::dmn::parser::parse_dmn;

const DMN_1_5: &str = include_str!("fixtures/dmn/decision_1_5.dmn");
const DMN_1_3: &str = include_str!("fixtures/dmn/decision_1_3.dmn");
const DMN_1_1: &str = include_str!("fixtures/dmn/decision_1_1.dmn");
const DMN_NO_HITPOLICY: &str = include_str!("fixtures/dmn/no_hitpolicy.dmn");
const DMN_CDATA: &str = include_str!("fixtures/dmn/cdata_description.dmn");

fn round_trip(src: &str) -> (engine::dmn::model::DmnModel, engine::dmn::model::DmnModel) {
    let ir1 = parse_dmn(src).expect("first parse ok");
    let xml = export_dmn(&ir1).expect("export ok");
    let ir2 = parse_dmn(&xml).expect("re-parse ok");
    (ir1, ir2)
}

#[test]
fn test_round_trip_decision_1_5() {
    let (a, b) = round_trip(DMN_1_5);
    assert_eq!(a, b, "DMN 1.5 IR must round-trip equal");
}
#[test]
fn test_round_trip_decision_1_3() {
    let (a, b) = round_trip(DMN_1_3);
    assert_eq!(a, b, "DMN 1.3 IR must round-trip equal");
}
#[test]
fn test_round_trip_decision_1_1() {
    let (a, b) = round_trip(DMN_1_1);
    assert_eq!(a, b, "DMN 1.1 IR must round-trip equal");
}

#[test]
fn test_round_trip_no_hitpolicy() {
    let ir1 = parse_dmn(DMN_NO_HITPOLICY).expect("parse ok");
    let xml = export_dmn(&ir1).expect("export ok");
    assert!(
        xml.contains(r#"hitPolicy="UNIQUE""#),
        "must carry hitPolicy UNIQUE: {xml}"
    );
    let ir2 = parse_dmn(&xml).expect("re-parse ok");
    let dec = match &ir2.definitions.children[0] {
        DefinitionsChild::Decision(d) => d,
        o => panic!("expected Decision, got {:?}", o),
    };
    assert_eq!(
        dec.decision_table.as_ref().unwrap().hit_policy,
        HitPolicy::Unique
    );
    assert_eq!(ir1, ir2, "no-hitpolicy IR must round-trip equal");
}

#[test]
fn test_round_trip_cdata_description() {
    let ir1 = parse_dmn(DMN_CDATA).expect("parse ok");
    let xml = export_dmn(&ir1).expect("export ok");
    assert!(xml.contains("<![CDATA["), "must preserve CDATA: {xml}");
    let ir2 = parse_dmn(&xml).expect("re-parse ok");
    let dec = match &ir2.definitions.children[0] {
        DefinitionsChild::Decision(d) => d,
        o => panic!("expected Decision, got {:?}", o),
    };
    assert!(
        dec.description.as_ref().unwrap().is_cdata,
        "must be CDATA-flagged"
    );
    assert_eq!(ir1, ir2, "CDATA fixture IR must round-trip equal");
}

#[test]
fn test_round_trip_element_order() {
    let (ir1, ir2) = round_trip(DMN_1_5);
    let seq1: Vec<_> = ir1
        .definitions
        .children
        .iter()
        .map(std::mem::discriminant)
        .collect();
    let seq2: Vec<_> = ir2
        .definitions
        .children
        .iter()
        .map(std::mem::discriminant)
        .collect();
    assert_eq!(seq1, seq2, "children variant sequence must be identical");
    assert_eq!(ir1, ir2);
}

#[test]
fn test_export_produces_valid_xml() {
    let xml = export_dmn(&parse_dmn(DMN_1_5).expect("parse ok")).expect("export ok");
    assert!(xml.starts_with("<?xml"), "must start with XML declaration");
    assert!(xml.contains("<definitions"), "must contain <definitions>");
    assert!(xml.contains("</definitions>"), "must close </definitions>");
}

#[test]
fn test_exporter_uses_canonical_ir_only() {
    let ir1 = parse_dmn(DMN_1_5).expect("parse ok");
    assert!(
        export_dmn(&ir1).is_ok(),
        "export_dmn(&DmnModel) must succeed"
    );
}

// =========================================================================
// DMNDI Round-Trip Tests (Wave 2 Task 2, DMN-07)
// =========================================================================

const DMN_WITH_DMNDI: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20180913/MODEL/"
             xmlns:dmndi="http://www.omg.org/spec/DMN/20180521/DMNDI/"
             xmlns:dc="http://www.omg.org/spec/DD/20180521/DC/"
             xmlns:di="http://www.omg.org/spec/DD/20180521/DI/"
             id="_test" name="Test DMN with DMNDI">
    <decision id="d_1" name="Simple Decision">
        <literalExpression><text>"hello"</text></literalExpression>
    </decision>
    <dmndi:DMNDI>
        <dmndi:DMNDiagram id="_diagram_1">
            <dmndi:Size height="400.0" width="300.0"/>
            <dmndi:DMNShape dmnElementRef="d_1" id="_shape_1">
                <dc:Bounds height="60.0" width="120.0" x="90.0" y="170.0"/>
            </dmndi:DMNShape>
        </dmndi:DMNDiagram>
    </dmndi:DMNDI>
</definitions>"#;

const DMN_NO_DMNDI: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20180913/MODEL/"
             id="_test_no_di" name="Test DMN No Diagrams">
    <decision id="d_2" name="Another Decision">
        <literalExpression><text>"world"</text></literalExpression>
    </decision>
</definitions>"#;

#[test]
fn test_dmndi_preserving_round_trip() {
    let ir1 = parse_dmn(DMN_WITH_DMNDI).expect("parse ok");
    assert_eq!(ir1.definitions.diagrams.len(), 1);
    let xml = export_dmn(&ir1).expect("export ok");
    let ir2 = parse_dmn(&xml).expect("re-parse ok");
    assert_eq!(ir2.definitions.diagrams.len(), 1, "diagram count preserved");
    assert_eq!(ir1, ir2);
}

#[test]
fn test_no_dmndi_added_when_absent() {
    let ir1 = parse_dmn(DMN_NO_DMNDI).expect("parse ok");
    assert!(ir1.definitions.diagrams.is_empty());
    let xml = export_dmn(&ir1).expect("export ok");
    assert!(!xml.contains("<dmndi:DMNDI>"), "no spurious DMNDI block");
    let ir2 = parse_dmn(&xml).expect("re-parse ok");
    assert_eq!(ir1, ir2);
}

#[test]
fn test_dmndi_namespace_preservation() {
    let xml = export_dmn(&parse_dmn(DMN_WITH_DMNDI).expect("parse ok")).expect("export ok");
    assert!(
        xml.contains(r#"xmlns:dmndi="http://www.omg.org/spec/DMN/20180521/DMNDI/""#)
            || xml.contains("xmlns:dmndi=\""),
        "dmndi NS"
    );
    assert!(
        xml.contains(r#"xmlns:dc="http://www.omg.org/spec/DD/20180521/DC/""#)
            || xml.contains("xmlns:dc=\""),
        "DC NS"
    );
    assert!(
        xml.contains(r#"xmlns:di="http://www.omg.org/spec/DD/20180521/DI/""#)
            || xml.contains("xmlns:di=\""),
        "DI NS"
    );
    assert!(xml.contains("DMNDI>"), "must contain DMNDI element marker");
}

#[test]
fn test_dmndi_element_completeness() {
    let xml = export_dmn(&parse_dmn(DMN_WITH_DMNDI).expect("parse ok")).expect("export ok");
    assert!(
        xml.contains(r#"dmnElementRef="d_1""#),
        "shape must reference decision element"
    );
    assert!(
        (xml.contains("x=\"90\"") || xml.contains("x=\"90.0\"")),
        "must preserve x=90"
    );
    assert!(
        (xml.contains("y=\"170\"") || xml.contains("y=\"170.0\"")),
        "must preserve y=170"
    );
    assert!(
        xml.contains(r#"width="120""#) || xml.contains(r#"width="120.0""#),
        "must preserve width"
    );
    assert!(
        xml.contains(r#"height="60""#) || xml.contains(r#"height="60.0""#),
        "must preserve height"
    );
}

#[test]
fn test_dmndi_round_trip_tck_fixture() {
    let tck_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("tck")
        .join("0002-input-data-number");
    let dmn_path = tck_root.join("0002-input-data-number.dmn");
    if !dmn_path.is_file() {
        return;
    }

    let content = std::fs::read_to_string(&dmn_path).expect("read fixture");
    let ir1 = parse_dmn(&content).expect("parse OK");
    if ir1.definitions.diagrams.is_empty() {
        return;
    }

    let xml = export_dmn(&ir1).expect("export OK");
    assert!(
        xml.contains("DMNDI"),
        "TCK fixture export must contain DMNDI marker"
    );

    let ir2 = parse_dmn(&xml).expect("re-parse OK");
    assert_eq!(ir1, ir2, "TCK fixture with DMNDI must round-trip equal");
}

#[test]
fn test_exported_xml_structure_verification() {
    let xml = export_dmn(&parse_dmn(DMN_1_5).expect("parse ok")).expect("export ok");
    assert!(xml.starts_with("<?xml"), "must have XML declaration");
    assert!(
        xml.contains("<definitions"),
        "must have <definitions> root element"
    );
    assert!(xml.contains("</definitions>"), "must close </definitions>");
    let has_ns = xml.contains("xmlns=\"https://www.omg.org/spec/DMN/")
        || xml.contains(r#"xmlns="http://www.omg.org/spec/DMN/"#);
    assert!(has_ns, "must declare DMN MODEL namespace");
    let has_decision = xml.contains("<decision") && xml.contains("</decision>");
    assert!(
        has_decision || xml.contains("<decisionTable"),
        "must have decision or decisionTable child"
    );
}
