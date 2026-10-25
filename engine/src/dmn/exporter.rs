//! DMN XML exporter — Plan 29-03 (GREEN phase: full implementation).
//!
//! Serializes the canonical IR ([`DmnModel`]) back to `.dmn` XML using a
//! [`quick_xml::writer::Writer`], preserving annotations, descriptions, IDs,
//! CDATA flags, element order, and DMNDI passthrough. Round-trip fidelity is
//! structural (Pitfall 11): the exporter consumes ONLY the canonical IR and
//! never reconstructs DMN from a non-IR shape.
//!
//! ## Design (load-bearing — do not regress)
//!
//! - **Canonical IR is the sole store.** Every emitted byte is derived from
//!   [`DmnModel`]; no DMN is reconstructed from a non-IR shape.
//! - **Element order is preserved.** `Vec<DefinitionsChild>` is iterated in
//!   order; clause/rule/entry vectors keep their document order (Pitfall 11).
//! - **CDATA round-trips.** [`TextContent::is_cdata`] decides CDATA-vs-escaped
//!   emission, so the parser re-sets the same flag on re-import.
//! - **No serde-XML emitter.** The `quick_xml` serde serializer loses
//!   attribute order and CDATA fidelity (same rationale as the parser); we
//!   use the event `Writer` directly.
//! - **Threat T-29-05 (XML injection).** Non-CDATA text is emitted via
//!   [`quick_xml::events::BytesText`] which XML-escapes `&`/`<`/`>`/`"`; CDATA
//!   is emitted only where [`TextContent::is_cdata`] is set.

use quick_xml::events::{BytesCData, BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::writer::Writer;
use thiserror::Error;

use crate::dmn::model::{
    AllowedValues, Annotation, Association, BusinessKnowledgeModel, Decision, DecisionService,
    DecisionTable, Definitions, DefinitionsChild, DmnDiagram, DmnEdge, DmnLabel, DmnModel, DmnRule,
    DmnShape, HitPolicy, InformationItem, InputClause, InputData, ItemDefinition, KnowledgeSource,
    LiteralExpression, OutputClause, RuleEntry, TextAnnotation, TextContent,
};

/// The DMN MODEL 1.5 namespace used as the default `xmlns` when the IR's
/// `definitions.namespace` is empty and no `source_namespace` was captured.
const DEFAULT_DMN_NS: &str = "https://www.omg.org/spec/DMN/20200524/MODEL/";

/// DMNDI / DC / DI namespace URIs for DMNDI passthrough emission.
const NS_DMNDI: &str = "http://www.omg.org/spec/DMN/20180521/DMNDI/";
const NS_DC: &str = "http://www.omg.org/spec/DD/20180521/DC/";
const NS_DI: &str = "http://www.omg.org/spec/DD/20180521/DI/";

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

impl From<quick_xml::Error> for DmnExportError {
    fn from(e: quick_xml::Error) -> Self {
        Self::XmlWrite(e.to_string())
    }
}

impl From<std::io::Error> for DmnExportError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

// =========================================================================
// Public API
// =========================================================================

/// Serialize a [`DmnModel`] to a `.dmn` XML document as a UTF-8 `String`.
///
/// The document carries an `<?xml version="1.0" encoding="UTF-8"?>` declaration
/// and a root `<definitions>` element bound to the DMN MODEL namespace.
pub fn export_dmn(model: &DmnModel) -> Result<String, DmnExportError> {
    let bytes = export_dmn_to_bytes(model)?;
    String::from_utf8(bytes)
        .map_err(|e| DmnExportError::XmlWrite(format!("non-UTF-8 exporter output: {e}")))
}

/// Serialize a [`DmnModel`] to `.dmn` XML bytes (the bytes variant for file
/// writing). This is the real entry point; [`export_dmn`] wraps it with a
/// UTF-8 conversion.
pub fn export_dmn_to_bytes(model: &DmnModel) -> Result<Vec<u8>, DmnExportError> {
    let mut writer = Writer::new(Vec::<u8>::new());
    emit_document(&mut writer, model)?;
    Ok(writer.into_inner())
}

// =========================================================================
// Emission — top-level document
// =========================================================================

/// Drive the full document: XML declaration, `<definitions>` root, children,
/// DMNDI, then the closing `</definitions>`.
fn emit_document(writer: &mut Writer<Vec<u8>>, model: &DmnModel) -> Result<(), DmnExportError> {
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    emit_definitions(
        writer,
        &model.definitions,
        model.source_namespace.as_deref(),
    )?;
    Ok(())
}

/// Emit the `<definitions>` root element with its attributes and ordered
/// children (Pitfall 11), followed by DMNDI passthrough if present.
fn emit_definitions(
    writer: &mut Writer<Vec<u8>>,
    defs: &Definitions,
    source_namespace: Option<&str>,
) -> Result<(), DmnExportError> {
    // Resolve the xmlns value: prefer the parser-captured source namespace,
    // fall back to the `namespace` attribute, then the DMN MODEL default.
    let xmlns = if let Some(ns) = source_namespace.filter(|s| !s.is_empty()) {
        ns
    } else if !defs.namespace.is_empty() {
        defs.namespace.as_str()
    } else {
        DEFAULT_DMN_NS
    };

    let mut start = BytesStart::new("definitions");
    start.push_attribute(("xmlns", xmlns));
    if let Some(id) = defs.id.as_deref() {
        start.push_attribute(("id", id));
    }
    if let Some(name) = defs.name.as_deref() {
        start.push_attribute(("name", name));
    }
    if !defs.namespace.is_empty() {
        start.push_attribute(("namespace", defs.namespace.as_str()));
    }
    if let Some(v) = defs.type_language.as_deref() {
        start.push_attribute(("typeLanguage", v));
    }
    if let Some(v) = defs.expression_language.as_deref() {
        start.push_attribute(("expressionLanguage", v));
    }
    if let Some(v) = defs.exporter.as_deref() {
        start.push_attribute(("exporter", v));
    }
    if let Some(v) = defs.exporter_version.as_deref() {
        start.push_attribute(("exporterVersion", v));
    }
    writer.write_event(Event::Start(start))?;

    // <description> child (CDATA/escaped) — emitted first per DMN schema order.
    if let Some(desc) = defs.description.as_ref() {
        emit_text_element(writer, "description", desc)?;
    }

    // Ordered top-level children (Pitfall 11).
    for child in &defs.children {
        emit_definitions_child(writer, child)?;
    }

    // DMNDI passthrough (DMN-07 prep — emitted after model children).
    if !defs.diagrams.is_empty() {
        emit_dmndi(writer, &defs.diagrams)?;
    }

    writer.write_event(Event::End(BytesEnd::new("definitions")))?;
    Ok(())
}

/// Emit a single `DefinitionsChild` variant in document order.
fn emit_definitions_child(
    writer: &mut Writer<Vec<u8>>,
    child: &DefinitionsChild,
) -> Result<(), DmnExportError> {
    match child {
        DefinitionsChild::ItemDefinition(item) => {
            emit_item_definition(writer, item, "itemDefinition")
        }
        DefinitionsChild::Decision(dec) => emit_decision(writer, dec),
        DefinitionsChild::InputData(input) => emit_input_data(writer, input),
        DefinitionsChild::BusinessKnowledgeModel(bkm) => emit_bkm(writer, bkm),
        DefinitionsChild::DecisionService(ds) => emit_decision_service(writer, ds),
        DefinitionsChild::KnowledgeSource(ks) => emit_knowledge_source(writer, ks),
        DefinitionsChild::TextAnnotation(ta) => emit_text_annotation(writer, ta),
        DefinitionsChild::Association(a) => emit_association(writer, a),
    }
}

// =========================================================================
// Clause 7 — <itemDefinition> (recursive over item_components)
// =========================================================================

/// Emit an `<itemDefinition>` (or nested `<itemComponent>`) element. `tag` is
/// the element name to emit (`"itemDefinition"` for top-level,
/// `"itemComponent"` for nested).
fn emit_item_definition(
    writer: &mut Writer<Vec<u8>>,
    item: &ItemDefinition,
    tag: &str,
) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new(tag);
    if let Some(id) = item.id.as_deref() {
        start.push_attribute(("id", id));
    }
    start.push_attribute(("name", item.name.as_str()));
    if item.is_collection {
        start.push_attribute(("isCollection", "true"));
    }
    if let Some(v) = item.type_language.as_deref() {
        start.push_attribute(("typeLanguage", v));
    }
    writer.write_event(Event::Start(start))?;

    if let Some(desc) = item.description.as_ref() {
        emit_text_element(writer, "description", desc)?;
    }
    if let Some(tref) = item.type_ref.as_deref() {
        emit_string_element(writer, "typeRef", tref)?;
    }
    if let Some(av) = item.allowed_values.as_ref() {
        emit_allowed_values(writer, "allowedValues", av)?;
    }
    for comp in &item.item_components {
        emit_item_definition(writer, comp, "itemComponent")?;
    }
    for ann in &item.annotations {
        emit_annotation(writer, ann)?;
    }

    writer.write_event(Event::End(BytesEnd::new(tag)))?;
    Ok(())
}

/// Emit an `allowedValues` / `outputValues` / `inputValues` block.
/// `UnaryTests(s)` → `<text>escaped</text>`; `Enumeration(items)` → one
/// `<itemComponent><text>…</text></itemComponent>` per literal.
fn emit_allowed_values(
    writer: &mut Writer<Vec<u8>>,
    tag: &str,
    av: &AllowedValues,
) -> Result<(), DmnExportError> {
    writer.write_event(Event::Start(BytesStart::new(tag)))?;
    match av {
        AllowedValues::UnaryTests(text) => {
            // Synthesize a non-CDATA TextContent (the exporter escapes it).
            emit_text_element(writer, "text", &TextContent::plain(text.as_str()))?;
        }
        AllowedValues::Enumeration(items) => {
            for item in items {
                writer.write_event(Event::Start(BytesStart::new("itemComponent")))?;
                emit_text_element(writer, "text", item)?;
                writer.write_event(Event::End(BytesEnd::new("itemComponent")))?;
            }
        }
    }
    writer.write_event(Event::End(BytesEnd::new(tag)))?;
    Ok(())
}

// =========================================================================
// <decision> + <decisionTable>
// =========================================================================

/// Emit a `<decision>` element. The `id` attribute is required by the DMN
/// schema; a `None` id is an [`DmnExportError::InvalidModel`].
fn emit_decision(writer: &mut Writer<Vec<u8>>, dec: &Decision) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("decision");
    start.push_attribute(("id", dec.id.as_str()));
    if let Some(name) = dec.name.as_deref() {
        start.push_attribute(("name", name));
    }
    if let Some(q) = dec.question.as_deref() {
        start.push_attribute(("question", q));
    }
    if let Some(a) = dec.allowed_answers.as_deref() {
        start.push_attribute(("allowedAnswers", a));
    }
    writer.write_event(Event::Start(start))?;

    if let Some(desc) = dec.description.as_ref() {
        emit_text_element(writer, "description", desc)?;
    }
    // `question` and `allowedAnswers` are emitted as attributes on the start
    // tag above; the parser reads them from attributes (and a `<question>`
    // child would override). We emit the attribute form only, which round-trips
    // cleanly because the parser's attribute read populates the same IR field.
    if let Some(v) = dec.variable.as_ref() {
        emit_information_item(writer, v, "variable")?;
    }
    for rid in &dec.required_decisions {
        writer.write_event(Event::Start(BytesStart::new("informationRequirement")))?;
        let mut rd = BytesStart::new("requiredDecision");
        rd.push_attribute(("href", format!("#{rid}").as_str()));
        writer.write_event(Event::Empty(rd))?;
        writer.write_event(Event::End(BytesEnd::new("informationRequirement")))?;
    }
    for rid in &dec.required_inputs {
        writer.write_event(Event::Start(BytesStart::new("informationRequirement")))?;
        let mut ri = BytesStart::new("requiredInput");
        ri.push_attribute(("href", format!("#{rid}").as_str()));
        writer.write_event(Event::Empty(ri))?;
        writer.write_event(Event::End(BytesEnd::new("informationRequirement")))?;
    }
    for rid in &dec.required_knowledge {
        writer.write_event(Event::Start(BytesStart::new("knowledgeRequirement")))?;
        let mut rk = BytesStart::new("requiredKnowledge");
        rk.push_attribute(("href", format!("#{rid}").as_str()));
        writer.write_event(Event::Empty(rk))?;
        writer.write_event(Event::End(BytesEnd::new("knowledgeRequirement")))?;
    }
    if let Some(literal) = dec.literal_expression.as_ref() {
        emit_literal_expression(writer, literal)?;
    }
    if let Some(dt) = dec.decision_table.as_ref() {
        emit_decision_table(writer, dt)?;
    }
    for ann in &dec.annotations {
        emit_annotation(writer, ann)?;
    }

    writer.write_event(Event::End(BytesEnd::new("decision")))?;
    Ok(())
}

/// Emit a `<variable>` / `<informationItem>` element from an
/// [`InformationItem`]. `tag` is the element name (`"variable"` for decisions,
/// `"variable"` for input data / BKMs too — DMN uses `<variable>` inside
/// DRG nodes).
fn emit_information_item(
    writer: &mut Writer<Vec<u8>>,
    item: &InformationItem,
    tag: &str,
) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new(tag);
    if let Some(id) = item.id.as_deref() {
        start.push_attribute(("id", id));
    }
    if !item.name.is_empty() {
        start.push_attribute(("name", item.name.as_str()));
    }
    if let Some(tref) = item.type_ref.as_deref() {
        start.push_attribute(("typeRef", tref));
    }
    writer.write_event(Event::Start(start))?;
    if let Some(desc) = item.description.as_ref() {
        emit_text_element(writer, "description", desc)?;
    }
    for ann in &item.annotations {
        emit_annotation(writer, ann)?;
    }
    writer.write_event(Event::End(BytesEnd::new(tag)))?;
    Ok(())
}

/// Emit a `<literalExpression>` element (DMN 1.5 §10.3.2).
///
/// The expression text is written back through [`emit_text_element`], so a
/// CDATA-encoded expression stays CDATA and a plain one stays escaped.
fn emit_literal_expression(
    writer: &mut Writer<Vec<u8>>,
    literal: &LiteralExpression,
) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("literalExpression");
    if let Some(id) = literal.id.as_deref() {
        start.push_attribute(("id", id));
    }
    if let Some(type_ref) = literal.type_ref.as_deref() {
        start.push_attribute(("typeRef", type_ref));
    }
    writer.write_event(Event::Start(start))?;
    if let Some(description) = literal.description.as_ref() {
        emit_text_element(writer, "description", description)?;
    }
    emit_text_element(writer, "text", &literal.text)?;
    writer.write_event(Event::End(BytesEnd::new("literalExpression")))?;
    Ok(())
}

/// Emit a `<decisionTable>` element. `hitPolicy` is ALWAYS emitted (the
/// default `Unique` is emitted as `hitPolicy="UNIQUE"` so it round-trips
/// explicitly); `aggregation` is emitted only when `Some` (COLLECT tables).
fn emit_decision_table(
    writer: &mut Writer<Vec<u8>>,
    dt: &DecisionTable,
) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("decisionTable");
    if let Some(id) = dt.id.as_deref() {
        start.push_attribute(("id", id));
    }
    if let Some(name) = dt.name.as_deref() {
        start.push_attribute(("name", name));
    }
    // Always emit hitPolicy (default Unique -> "UNIQUE" round-trips cleanly).
    start.push_attribute(("hitPolicy", HitPolicy::as_str(&dt.hit_policy)));
    if let Some(agg) = dt.aggregation.as_ref() {
        start.push_attribute(("aggregation", agg.as_str()));
    }
    if let Some(v) = dt.preferred_orientation.as_deref() {
        start.push_attribute(("preferredOrientation", v));
    }
    if let Some(v) = dt.output_label.as_deref() {
        start.push_attribute(("outputLabel", v));
    }
    writer.write_event(Event::Start(start))?;

    for ic in &dt.input_clauses {
        emit_input_clause(writer, ic)?;
    }
    for oc in &dt.output_clauses {
        emit_output_clause(writer, oc)?;
    }
    for rule in &dt.rules {
        emit_rule(writer, rule)?;
    }

    writer.write_event(Event::End(BytesEnd::new("decisionTable")))?;
    Ok(())
}

/// Emit an `<input>` clause.
fn emit_input_clause(writer: &mut Writer<Vec<u8>>, ic: &InputClause) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("input");
    if let Some(id) = ic.id.as_deref() {
        start.push_attribute(("id", id));
    }
    if let Some(label) = ic.label.as_deref() {
        start.push_attribute(("label", label));
    }
    writer.write_event(Event::Start(start))?;

    if let Some(desc) = ic.description.as_ref() {
        emit_text_element(writer, "description", desc)?;
    }
    // <inputExpression><text>…</text></inputExpression>
    writer.write_event(Event::Start(BytesStart::new("inputExpression")))?;
    emit_string_element(writer, "text", ic.input_expression.as_str())?;
    writer.write_event(Event::End(BytesEnd::new("inputExpression")))?;
    if let Some(av) = ic.input_values.as_ref() {
        emit_allowed_values(writer, "inputValues", av)?;
    }

    writer.write_event(Event::End(BytesEnd::new("input")))?;
    Ok(())
}

/// Emit an `<output>` clause.
fn emit_output_clause(
    writer: &mut Writer<Vec<u8>>,
    oc: &OutputClause,
) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("output");
    if let Some(id) = oc.id.as_deref() {
        start.push_attribute(("id", id));
    }
    if let Some(label) = oc.label.as_deref() {
        start.push_attribute(("label", label));
    }
    if let Some(name) = oc.name.as_deref() {
        start.push_attribute(("name", name));
    }
    if let Some(tref) = oc.type_ref.as_deref() {
        start.push_attribute(("typeRef", tref));
    }
    writer.write_event(Event::Start(start))?;

    if let Some(desc) = oc.description.as_ref() {
        emit_text_element(writer, "description", desc)?;
    }
    if let Some(av) = oc.output_values.as_ref() {
        emit_allowed_values(writer, "outputValues", av)?;
    }
    if let Some(doe) = oc.default_output_entry.as_deref() {
        writer.write_event(Event::Start(BytesStart::new("defaultOutputEntry")))?;
        emit_string_element(writer, "text", doe)?;
        writer.write_event(Event::End(BytesEnd::new("defaultOutputEntry")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("output")))?;
    Ok(())
}

/// Emit a `<rule>` element: `<description>` (if any), one `<inputEntry>` per
/// input clause, one `<outputEntry>` per output clause, one
/// `<annotationEntry>` per annotation column. Clause/entry order is preserved.
fn emit_rule(writer: &mut Writer<Vec<u8>>, rule: &DmnRule) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("rule");
    if let Some(id) = rule.id.as_deref() {
        start.push_attribute(("id", id));
    }
    writer.write_event(Event::Start(start))?;

    if let Some(desc) = rule.description.as_ref() {
        emit_text_element(writer, "description", desc)?;
    }
    for entry in &rule.input_entries {
        emit_rule_entry(writer, "inputEntry", entry)?;
    }
    for entry in &rule.output_entries {
        emit_rule_entry(writer, "outputEntry", entry)?;
    }
    for entry in &rule.annotation_entries {
        emit_rule_entry(writer, "annotationEntry", entry)?;
    }

    writer.write_event(Event::End(BytesEnd::new("rule")))?;
    Ok(())
}

/// Emit a single `<inputEntry>` / `<outputEntry>` / `<annotationEntry>` with a
/// `<text>` child carrying the FEEL cell text (CDATA flag preserved).
fn emit_rule_entry(
    writer: &mut Writer<Vec<u8>>,
    tag: &str,
    entry: &RuleEntry,
) -> Result<(), DmnExportError> {
    writer.write_event(Event::Start(BytesStart::new(tag)))?;
    emit_text_element(writer, "text", &entry.text)?;
    writer.write_event(Event::End(BytesEnd::new(tag)))?;
    Ok(())
}

// =========================================================================
// DRG nodes: inputData / BKM / decisionService / knowledgeSource
// =========================================================================

/// Emit an `<inputData>` element.
fn emit_input_data(writer: &mut Writer<Vec<u8>>, input: &InputData) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("inputData");
    start.push_attribute(("id", input.id.as_str()));
    if let Some(name) = input.name.as_deref() {
        start.push_attribute(("name", name));
    }
    writer.write_event(Event::Start(start))?;
    if let Some(desc) = input.description.as_ref() {
        emit_text_element(writer, "description", desc)?;
    }
    if let Some(v) = input.variable.as_ref() {
        emit_information_item(writer, v, "variable")?;
    }
    for ann in &input.annotations {
        emit_annotation(writer, ann)?;
    }
    writer.write_event(Event::End(BytesEnd::new("inputData")))?;
    Ok(())
}

/// Emit a `<businessKnowledgeModel>` element.
fn emit_bkm(
    writer: &mut Writer<Vec<u8>>,
    bkm: &BusinessKnowledgeModel,
) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("businessKnowledgeModel");
    start.push_attribute(("id", bkm.id.as_str()));
    if let Some(name) = bkm.name.as_deref() {
        start.push_attribute(("name", name));
    }
    writer.write_event(Event::Start(start))?;
    if let Some(desc) = bkm.description.as_ref() {
        emit_text_element(writer, "description", desc)?;
    }
    if let Some(v) = bkm.variable.as_ref() {
        emit_information_item(writer, v, "variable")?;
    }
    for rid in &bkm.knowledge_requirement {
        writer.write_event(Event::Start(BytesStart::new("knowledgeRequirement")))?;
        let mut rk = BytesStart::new("requiredKnowledge");
        rk.push_attribute(("href", format!("#{rid}").as_str()));
        writer.write_event(Event::Empty(rk))?;
        writer.write_event(Event::End(BytesEnd::new("knowledgeRequirement")))?;
    }
    for ann in &bkm.annotations {
        emit_annotation(writer, ann)?;
    }
    writer.write_event(Event::End(BytesEnd::new("businessKnowledgeModel")))?;
    Ok(())
}

/// Emit a `<decisionService>` element.
fn emit_decision_service(
    writer: &mut Writer<Vec<u8>>,
    ds: &DecisionService,
) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("decisionService");
    start.push_attribute(("id", ds.id.as_str()));
    if let Some(name) = ds.name.as_deref() {
        start.push_attribute(("name", name));
    }
    writer.write_event(Event::Start(start))?;
    if let Some(desc) = ds.description.as_ref() {
        emit_text_element(writer, "description", desc)?;
    }
    for rid in &ds.output_decisions {
        let mut e = BytesStart::new("outputDecision");
        e.push_attribute(("href", format!("#{rid}").as_str()));
        writer.write_event(Event::Empty(e))?;
    }
    for rid in &ds.encapsulated_decisions {
        let mut e = BytesStart::new("encapsulatedDecision");
        e.push_attribute(("href", format!("#{rid}").as_str()));
        writer.write_event(Event::Empty(e))?;
    }
    for rid in &ds.input_data {
        let mut e = BytesStart::new("inputData");
        e.push_attribute(("href", format!("#{rid}").as_str()));
        writer.write_event(Event::Empty(e))?;
    }
    for ann in &ds.annotations {
        emit_annotation(writer, ann)?;
    }
    writer.write_event(Event::End(BytesEnd::new("decisionService")))?;
    Ok(())
}

/// Emit a `<knowledgeSource>` element.
fn emit_knowledge_source(
    writer: &mut Writer<Vec<u8>>,
    ks: &KnowledgeSource,
) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("knowledgeSource");
    start.push_attribute(("id", ks.id.as_str()));
    if let Some(name) = ks.name.as_deref() {
        start.push_attribute(("name", name));
    }
    if let Some(t) = ks.type_ref.as_deref() {
        start.push_attribute(("type", t));
    }
    writer.write_event(Event::Start(start))?;
    if let Some(desc) = ks.description.as_ref() {
        emit_text_element(writer, "description", desc)?;
    }
    for ann in &ks.annotations {
        emit_annotation(writer, ann)?;
    }
    writer.write_event(Event::End(BytesEnd::new("knowledgeSource")))?;
    Ok(())
}

// =========================================================================
// Artifacts: textAnnotation / association / annotation
// =========================================================================

/// Emit a `<textAnnotation>` element. The annotation text is emitted as a
/// `<text>` child (CDATA when `is_cdata`), which the parser reads back into
/// `TextContent` regardless of whether the source used a `text` attribute or
/// a `<text>` child.
fn emit_text_annotation(
    writer: &mut Writer<Vec<u8>>,
    ta: &TextAnnotation,
) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("textAnnotation");
    if let Some(id) = ta.id.as_deref() {
        start.push_attribute(("id", id));
    }
    if let Some(tf) = ta.text_format.as_deref() {
        start.push_attribute(("textFormat", tf));
    }
    writer.write_event(Event::Start(start))?;
    emit_text_element(writer, "text", &ta.text)?;
    writer.write_event(Event::End(BytesEnd::new("textAnnotation")))?;
    Ok(())
}

/// Emit an `<association>` element.
fn emit_association(writer: &mut Writer<Vec<u8>>, a: &Association) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("association");
    if let Some(id) = a.id.as_deref() {
        start.push_attribute(("id", id));
    }
    if let Some(s) = a.source_ref.as_deref() {
        start.push_attribute(("sourceRef", s));
    }
    if let Some(t) = a.target_ref.as_deref() {
        start.push_attribute(("targetRef", t));
    }
    if let Some(ty) = a.association_type.as_deref() {
        start.push_attribute(("type", ty));
    }
    writer.write_event(Event::Start(start))?;
    if let Some(ann) = a.annotation.as_ref() {
        emit_annotation(writer, ann)?;
    }
    writer.write_event(Event::End(BytesEnd::new("association")))?;
    Ok(())
}

/// Emit an `<annotation>` element (a child annotation on a DRG node).
fn emit_annotation(writer: &mut Writer<Vec<u8>>, ann: &Annotation) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("annotation");
    if let Some(id) = ann.id.as_deref() {
        start.push_attribute(("id", id));
    }
    if let Some(ty) = ann.annotation_type.as_deref() {
        start.push_attribute(("annotationType", ty));
    }
    if let Some(tf) = ann.text_format.as_deref() {
        start.push_attribute(("textFormat", tf));
    }
    writer.write_event(Event::Start(start))?;
    emit_text_element(writer, "text", &ann.text)?;
    writer.write_event(Event::End(BytesEnd::new("annotation")))?;
    Ok(())
}

// =========================================================================
// DMNDI passthrough (DMN-07 prep — emitted when diagrams is non-empty)
// =========================================================================

/// Emit a `<dmndi:DMNDI>` block carrying the [`DmnDiagram`] IR through
/// round-trip. Uses the `dmndi`/`dc`/`di` namespace prefixes. The parser is
/// prefix-agnostic (matches local names), so the prefix choice is cosmetic.
fn emit_dmndi(writer: &mut Writer<Vec<u8>>, diagrams: &[DmnDiagram]) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("dmndi:DMNDI");
    start.push_attribute(("xmlns:dmndi", NS_DMNDI));
    start.push_attribute(("xmlns:dc", NS_DC));
    start.push_attribute(("xmlns:di", NS_DI));
    writer.write_event(Event::Start(start))?;
    for diag in diagrams {
        emit_dmn_diagram(writer, diag)?;
    }
    writer.write_event(Event::End(BytesEnd::new("dmndi:DMNDI")))?;
    Ok(())
}

fn emit_dmn_diagram(writer: &mut Writer<Vec<u8>>, diag: &DmnDiagram) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("dmndi:DMNDiagram");
    if let Some(id) = diag.id.as_deref() {
        start.push_attribute(("id", id));
    }
    if let Some(name) = diag.name.as_deref() {
        start.push_attribute(("name", name));
    }
    writer.write_event(Event::Start(start))?;
    for shape in &diag.shapes {
        emit_dmn_shape(writer, shape)?;
    }
    for edge in &diag.edges {
        emit_dmn_edge(writer, edge)?;
    }
    writer.write_event(Event::End(BytesEnd::new("dmndi:DMNDiagram")))?;
    Ok(())
}

fn emit_dmn_shape(writer: &mut Writer<Vec<u8>>, shape: &DmnShape) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("dmndi:DMNShape");
    start.push_attribute(("dmnElementRef", shape.element_id.as_str()));
    writer.write_event(Event::Start(start))?;
    if let Some(b) = shape.bounds.as_ref() {
        emit_dc_bounds(writer, b)?;
    }
    for label in &shape.labels {
        emit_dmn_label(writer, label)?;
    }
    writer.write_event(Event::End(BytesEnd::new("dmndi:DMNShape")))?;
    Ok(())
}

fn emit_dmn_edge(writer: &mut Writer<Vec<u8>>, edge: &DmnEdge) -> Result<(), DmnExportError> {
    let mut start = BytesStart::new("dmndi:DMNEdge");
    if let Some(id) = edge.id.as_deref() {
        start.push_attribute(("id", id));
    }
    start.push_attribute(("source", edge.source_id.as_str()));
    start.push_attribute(("target", edge.target_id.as_str()));
    writer.write_event(Event::Start(start))?;
    for wp in &edge.waypoints {
        let mut e = BytesStart::new("di:waypoint");
        e.push_attribute(("x", format_f64(wp.x).as_str()));
        e.push_attribute(("y", format_f64(wp.y).as_str()));
        writer.write_event(Event::Empty(e))?;
    }
    writer.write_event(Event::End(BytesEnd::new("dmndi:DMNEdge")))?;
    Ok(())
}

fn emit_dc_bounds(
    writer: &mut Writer<Vec<u8>>,
    b: &crate::dmn::model::DcBounds,
) -> Result<(), DmnExportError> {
    let mut e = BytesStart::new("dc:Bounds");
    e.push_attribute(("x", format_f64(b.x).as_str()));
    e.push_attribute(("y", format_f64(b.y).as_str()));
    e.push_attribute(("width", format_f64(b.width).as_str()));
    e.push_attribute(("height", format_f64(b.height).as_str()));
    writer.write_event(Event::Empty(e))?;
    Ok(())
}

fn emit_dmn_label(writer: &mut Writer<Vec<u8>>, label: &DmnLabel) -> Result<(), DmnExportError> {
    let start = BytesStart::new("dmndi:DMNLabel");
    writer.write_event(Event::Start(start))?;
    if let Some(b) = label.bounds.as_ref() {
        emit_dc_bounds(writer, b)?;
    }
    if let Some(text) = label.text.as_deref() {
        emit_string_element(writer, "text", text)?;
    }
    writer.write_event(Event::End(BytesEnd::new("dmndi:DMNLabel")))?;
    Ok(())
}

// =========================================================================
// Shared text/element helpers
// =========================================================================

/// Emit a `<tag>` element whose body is a single [`TextContent`] value.
/// CDATA is emitted when `is_cdata == true`; otherwise the text is XML-escaped
/// via [`BytesText`] (T-29-05 mitigation).
fn emit_text_element(
    writer: &mut Writer<Vec<u8>>,
    tag: &str,
    text: &TextContent,
) -> Result<(), DmnExportError> {
    writer.write_event(Event::Start(BytesStart::new(tag)))?;
    emit_text_content(writer, text)?;
    writer.write_event(Event::End(BytesEnd::new(tag)))?;
    Ok(())
}

/// Emit a `<tag>` element whose body is a plain (non-CDATA) string. The string
/// is XML-escaped via [`BytesText`].
fn emit_string_element(
    writer: &mut Writer<Vec<u8>>,
    tag: &str,
    text: &str,
) -> Result<(), DmnExportError> {
    writer.write_event(Event::Start(BytesStart::new(tag)))?;
    writer.write_event(Event::Text(BytesText::new(text)))?;
    writer.write_event(Event::End(BytesEnd::new(tag)))?;
    Ok(())
}

/// Emit a [`TextContent`] value as either a CDATA section (`is_cdata == true`)
/// or XML-escaped text (`is_cdata == false`). This is the CDATA-vs-escaped
/// branch that preserves the distinction through round-trip (DMN-06).
fn emit_text_content(
    writer: &mut Writer<Vec<u8>>,
    text: &TextContent,
) -> Result<(), DmnExportError> {
    if text.is_cdata {
        writer.write_event(Event::CData(BytesCData::new(text.content.as_str())))?;
    } else {
        writer.write_event(Event::Text(BytesText::new(text.content.as_str())))?;
    }
    Ok(())
}

/// Format an `f64` coordinate for DMNDI emission (trim trailing zeros so
/// `100.0` becomes `"100"` and `1.5` stays `"1.5"`).
fn format_f64(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

// =========================================================================
// Tests (Plan 29-03 Task 1 behavior tests — GREEN phase)
// =========================================================================

#[cfg(test)]
mod tests {
    //! Plan 29-03 Task 1 behavior tests. These were RED against the
    //! `Err`-returning stubs; they pass against the real `quick_xml::Writer`
    //! emitter (GREEN phase).
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
            literal_expression: None,
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
        model
            .definitions
            .children
            .push(DefinitionsChild::Decision(Decision {
                id: "_d1".into(),
                name: Some("Total".into()),
                description: None,
                variable: None,
                question: None,
                allowed_answers: None,
                literal_expression: None,
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
}
