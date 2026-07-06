//! DMN XML exporter — Plan 29-03 (RED phase: failing behavior tests + stubs).
//!
//! Serializes the canonical IR ([`DmnModel`]) back to `.dmn` XML using a
//! [`quick_xml::writer::Writer`], preserving annotations, descriptions, IDs,
//! CDATA flags, element order, and DMNDI passthrough. Round-trip fidelity is
//! structural (Pitfall 11): the exporter consumes ONLY the canonical IR and
//! never reconstructs DMN from a non-IR shape.
//!
//! This is the RED TDD phase: the public signatures and behavior tests are in
//! place, but the implementations are stubs returning `Err`. The GREEN phase
//! replaces them with the real `quick_xml::Writer` emitter.

use thiserror::Error;

use crate::dmn::model::DmnModel;

/// Errors produced by the DMN XML exporter.
#[derive(Debug, Clone, Error)]
pub enum DmnExportError {
    /// Low-level XML write error from `quick-xml`.
    #[error("XML write error: {0}")]
    XmlWrite(String),
    /// The IR model is invalid for export (e.g. a required `id` is missing).
    #[error("invalid DMN model: {0}")]
    InvalidModel(String),
    /// An I/O error writing the output.
    #[error("I/O error: {0}")]
    Io(String),
}

/// Serialize a [`DmnModel`] to a `.dmn` XML document string.
///
/// RED stub — returns `Err` until the GREEN phase implements the emitter.
pub fn export_dmn(_model: &DmnModel) -> Result<String, DmnExportError> {
    Err(DmnExportError::InvalidModel(
        "exporter not yet implemented (RED stub)".into(),
    ))
}

/// Serialize a [`DmnModel`] to `.dmn` XML bytes.
///
/// RED stub — returns `Err` until the GREEN phase implements the emitter.
pub fn export_dmn_to_bytes(_model: &DmnModel) -> Result<Vec<u8>, DmnExportError> {
    Err(DmnExportError::InvalidModel(
        "exporter not yet implemented (RED stub)".into(),
    ))
}

#[cfg(test)]
mod tests {
    //! Plan 29-03 Task 1 behavior tests (RED phase — these fail until the
    //! GREEN phase implements the real emitter).
    use super::*;
    use crate::dmn::model::{
        BuiltinAggregator, Decision, DecisionTable, Definitions, DefinitionsChild, HitPolicy,
        ItemDefinition, TextContent,
    };

    /// Build an empty `Decision` with a required `id` and no children.
    fn empty_decision(id: &str) -> Decision {
        Decision {
            id: id.into(),
            name: None,
            description: None,
            variable: None,
            question: None,
            allowed_answers: None,
            decision_table: None,
            required_decisions: Vec::new(),
            required_inputs: Vec::new(),
            required_knowledge: Vec::new(),
            annotations: Vec::new(),
        }
    }

    /// Build a minimal `ItemDefinition` with the given name.
    fn named_item(name: &str) -> ItemDefinition {
        ItemDefinition {
            id: None,
            name: name.into(),
            description: None,
            type_ref: None,
            is_collection: false,
            type_language: None,
            allowed_values: None,
            item_components: Vec::new(),
            annotations: Vec::new(),
        }
    }

    /// Test 1: `export_dmn(&model)` returns `Ok(String)` containing
    /// `<definitions` and `</definitions>`.
    #[test]
    fn export_dmn_returns_ok_with_definitions() {
        let model = minimal_model();
        let xml = export_dmn(&model).expect("export should succeed");
        assert!(
            xml.contains("<definitions"),
            "expected '<definitions' in output: {xml}"
        );
        assert!(
            xml.contains("</definitions>"),
            "expected '</definitions>' in output: {xml}"
        );
    }

    /// Test 2: a `TextContent { is_cdata: true, content: "x" }` is emitted as
    /// `<![CDATA[x]]>`.
    #[test]
    fn export_dmn_emits_cdata_for_cdata_text() {
        let mut model = minimal_model();
        model.definitions.description = Some(TextContent::cdata("x"));
        let xml = export_dmn(&model).expect("export should succeed");
        assert!(
            xml.contains("<![CDATA[x]]>"),
            "expected '<![CDATA[x]]>' in output: {xml}"
        );
    }

    /// Test 3: `HitPolicy::Collect` + `BuiltinAggregator::Sum` emits
    /// `hitPolicy="COLLECT" aggregation="SUM"`.
    #[test]
    fn export_dmn_emits_collect_and_aggregation() {
        let model = collect_sum_model();
        let xml = export_dmn(&model).expect("export should succeed");
        assert!(
            xml.contains(r#"hitPolicy="COLLECT""#),
            "expected hitPolicy=\"COLLECT\" in output: {xml}"
        );
        assert!(
            xml.contains(r#"aggregation="SUM""#),
            "expected aggregation=\"SUM\" in output: {xml}"
        );
    }

    /// Test 4: `definitions.children == [ItemDefinition, Decision]` is emitted
    /// with `<itemDefinition>` before `<decision>` (element order preserved).
    #[test]
    fn export_dmn_preserves_element_order() {
        let model = ordered_children_model();
        let xml = export_dmn(&model).expect("export should succeed");
        let item_pos = xml
            .find("<itemDefinition")
            .unwrap_or_else(|| panic!("<itemDefinition> not found in: {xml}"));
        let dec_pos = xml
            .find("<decision")
            .unwrap_or_else(|| panic!("<decision> not found in: {xml}"));
        assert!(
            item_pos < dec_pos,
            "itemDefinition must precede decision (Pitfall 11): {xml}"
        );
    }

    /// Build a minimal valid `DmnModel` (empty definitions + default namespace).
    fn minimal_model() -> DmnModel {
        DmnModel {
            definitions: Definitions {
                namespace: "https://www.omg.org/spec/DMN/20200524/MODEL/".into(),
                ..Definitions::default()
            },
            source_namespace: Some("https://www.omg.org/spec/DMN/20200524/MODEL/".into()),
        }
    }

    /// Build a `DmnModel` with one decision carrying a COLLECT + SUM table.
    fn collect_sum_model() -> DmnModel {
        let mut model = minimal_model();
        model.definitions.children.push(DefinitionsChild::Decision(Decision {
            id: "_d1".into(),
            name: Some("Total".into()),
            description: None,
            variable: None,
            question: None,
            allowed_answers: None,
            decision_table: Some(DecisionTable {
                hit_policy: HitPolicy::Collect,
                aggregation: Some(BuiltinAggregator::Sum),
                ..DecisionTable::default()
            }),
            required_decisions: Vec::new(),
            required_inputs: Vec::new(),
            required_knowledge: Vec::new(),
            annotations: Vec::new(),
        }));
        model
    }

    /// Build a `DmnModel` whose `children == [ItemDefinition, Decision]` to
    /// assert element-order preservation.
    fn ordered_children_model() -> DmnModel {
        let mut model = minimal_model();
        model
            .definitions
            .children
            .push(DefinitionsChild::ItemDefinition(named_item("Age")));
        model
            .definitions
            .children
            .push(DefinitionsChild::Decision(empty_decision("_d1")));
        model
    }
}