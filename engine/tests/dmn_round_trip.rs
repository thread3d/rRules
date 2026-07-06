//! Plan 29-03 Task 2: Round-trip DOM-compare tests against the import corpus.
//!
//! The round-trip gate (DMN-06) is a DOM-compare: parse a fixture into IR,
//! export the IR back to XML, parse the exported XML into IR again, and
//! assert the two IRs are equal (`PartialEq`). This proves the IR carries
//! enough information to regenerate the document and that the exporter
//! preserves annotations/descriptions/IDs/element-order/CDATA (Pitfall 11).
//!
//! Filter: `cargo test -p engine --test dmn_round_trip`

use engine::dmn::exporter::export_dmn;
use engine::dmn::model::{DefinitionsChild, HitPolicy};
use engine::dmn::parser::parse_dmn;

// Reuse the import-corpus fixtures from Plan 29-02 (same paths).
const DMN_1_5: &str = include_str!("fixtures/dmn/decision_1_5.dmn");
const DMN_1_3: &str = include_str!("fixtures/dmn/decision_1_3.dmn");
const DMN_1_1: &str = include_str!("fixtures/dmn/decision_1_1.dmn");
const DMN_NO_HITPOLICY: &str = include_str!("fixtures/dmn/no_hitpolicy.dmn");
const DMN_CDATA: &str = include_str!("fixtures/dmn/cdata_description.dmn");

/// Round-trip a DMN source: parse → export → parse, returning both IRs.
/// The canonical IR-equality gate is `assert_eq!(ir1, ir2)`.
fn round_trip(src: &str) -> (engine::dmn::model::DmnModel, engine::dmn::model::DmnModel) {
    let ir1 = parse_dmn(src).expect("first parse should succeed");
    let xml = export_dmn(&ir1).expect("export should succeed");
    let ir2 = parse_dmn(&xml).expect("re-parse of exported XML should succeed");
    (ir1, ir2)
}

/// Round-trip the DMN 1.5 fixture: `parse(export(parse(x))) == parse(x)`.
#[test]
fn test_round_trip_decision_1_5() {
    let (ir1, ir2) = round_trip(DMN_1_5);
    assert_eq!(ir1, ir2, "DMN 1.5 IR must round-trip equal (Pitfall 11)");
}

/// Round-trip the DMN 1.3 fixture (PRIORITY hit policy + enumeration
/// `outputValues`).
#[test]
fn test_round_trip_decision_1_3() {
    let (ir1, ir2) = round_trip(DMN_1_3);
    assert_eq!(ir1, ir2, "DMN 1.3 IR must round-trip equal (Pitfall 11)");
}

/// Round-trip the DMN 1.1 fixture (COLLECT + SUM aggregator).
#[test]
fn test_round_trip_decision_1_1() {
    let (ir1, ir2) = round_trip(DMN_1_1);
    assert_eq!(ir1, ir2, "DMN 1.1 IR must round-trip equal (Pitfall 11)");
}

/// Round-trip the no-hitpolicy fixture. Additionally asserts the exported
/// `<decisionTable>` carries `hitPolicy="UNIQUE"` (explicit) and the
/// re-parsed `hit_policy == HitPolicy::Unique` — the default is structural on
/// export (DMN-04).
#[test]
fn test_round_trip_no_hitpolicy() {
    let ir1 = parse_dmn(DMN_NO_HITPOLICY).expect("first parse should succeed");
    let xml = export_dmn(&ir1).expect("export should succeed");
    assert!(
        xml.contains(r#"hitPolicy="UNIQUE""#),
        "exported decisionTable must carry hitPolicy=\"UNIQUE\" (explicit default): {xml}"
    );
    let ir2 = parse_dmn(&xml).expect("re-parse should succeed");
    // Re-parsed hit policy must be Unique (the default, now structural).
    let dec = match &ir2.definitions.children[0] {
        DefinitionsChild::Decision(d) => d,
        other => panic!("expected first child to be Decision, got {:?}", other),
    };
    let dt = dec
        .decision_table
        .as_ref()
        .expect("decision table present");
    assert_eq!(
        dt.hit_policy,
        HitPolicy::Unique,
        "re-parsed hitPolicy must be Unique (structural default)"
    );
    assert_eq!(ir1, ir2, "no-hitpolicy IR must round-trip equal");
}

/// Round-trip the CDATA fixture. Additionally asserts the exported XML
/// contains `<![CDATA[` (CDATA preserved through round-trip) and the re-parsed
/// decision description has `is_cdata == true`.
#[test]
fn test_round_trip_cdata_description() {
    let ir1 = parse_dmn(DMN_CDATA).expect("first parse should succeed");
    let xml = export_dmn(&ir1).expect("export should succeed");
    assert!(
        xml.contains("<![CDATA["),
        "exported XML must contain CDATA sections (CDATA preserved): {xml}"
    );
    let ir2 = parse_dmn(&xml).expect("re-parse should succeed");
    let dec = match &ir2.definitions.children[0] {
        DefinitionsChild::Decision(d) => d,
        other => panic!("expected first child to be Decision, got {:?}", other),
    };
    let desc = dec
        .description
        .as_ref()
        .expect("decision has a description after re-parse");
    assert!(
        desc.is_cdata,
        "re-parsed decision description must be CDATA-flagged (is_cdata == true)"
    );
    assert_eq!(ir1, ir2, "CDATA fixture IR must round-trip equal");
}

/// Element-order round-trip: the `definitions.children` variant discriminant
/// sequence is identical between `ir1` and `ir2` (Pitfall 11).
#[test]
fn test_round_trip_element_order() {
    let (ir1, ir2) = round_trip(DMN_1_5);
    let seq1: Vec<std::mem::Discriminant<DefinitionsChild>> =
        ir1.definitions.children.iter().map(std::mem::discriminant).collect();
    let seq2: Vec<std::mem::Discriminant<DefinitionsChild>> =
        ir2.definitions.children.iter().map(std::mem::discriminant).collect();
    assert_eq!(
        seq1, seq2,
        "children variant sequence must be identical after round-trip (Pitfall 11)"
    );
    // Full IR equality is the stronger gate; element order is the focus here.
    assert_eq!(ir1, ir2, "DMN 1.5 IR must round-trip equal (element order)");
}

/// Well-formedness sanity: the exported XML starts with `<?xml` and contains
/// `<definitions` and `</definitions>`.
#[test]
fn test_export_produces_valid_xml() {
    let ir = parse_dmn(DMN_1_5).expect("parse should succeed");
    let xml = export_dmn(&ir).expect("export should succeed");
    assert!(
        xml.starts_with("<?xml"),
        "exported XML must start with <?xml declaration: {xml}"
    );
    assert!(
        xml.contains("<definitions"),
        "exported XML must contain <definitions: {xml}"
    );
    assert!(
        xml.contains("</definitions>"),
        "exported XML must contain </definitions>: {xml}"
    );
}

/// The exporter consumes only the canonical IR (`&DmnModel`). This is a
/// documentation-style compile-time gate: calling `export_dmn(&ir1)` with a
/// `DmnModel` proves the exporter uses the canonical IR, not a non-IR shape
/// (success criterion 4: IR is the sole store).
#[test]
fn test_exporter_uses_canonical_ir_only() {
    let ir1 = parse_dmn(DMN_1_5).expect("parse should succeed");
    // If this compiles, the exporter consumes &DmnModel (the canonical IR).
    let result = export_dmn(&ir1);
    assert!(
        result.is_ok(),
        "export_dmn(&DmnModel) must succeed — exporter consumes the canonical IR"
    );
}