//! DMN 1.5 XML parser — Plan 29-02 (GREEN phase: full implementation).
//!
//! Streaming, namespace-aware, XXE-hardened, CDATA-handled parser using
//! [`quick_xml::reader::NsReader`]. Imports DMN 1.1 / 1.3 / 1.5 `.dmn` files
//! into the canonical IR from [`crate::dmn::model`].
//!
//! ## Design (load-bearing — do not regress)
//!
//! - **Streaming pull parser.** A `NsReader` event loop drives the parse; the
//!   whole DMN DOM is never loaded. We do NOT use the `quick_xml::de` serde-XML
//!   deserializer (serde-xml loses attribute order and CDATA fidelity, which
//!   breaks round-trip — Pitfall 11).
//! - **Namespace-aware, prefix-agnostic.** Elements are matched by local name
//!   (the part after `:`), so any prefix works. The recognized DMN MODEL
//!   namespace URIs (1.1 / 1.3 / 1.5) are accepted on `<definitions>`; the
//!   observed namespace is stored on [`DmnModel::source_namespace`] for the
//!   exporter to re-emit equivalent `xmlns:*` declarations (Plan 29-03).
//! - **XXE hardening (T-29-01 mitigation).** Any `<!DOCTYPE>` or processing
//!   instruction is rejected with [`DmnParseError::XxeRejected`]; no entity
//!   resolver is called, no DTD subset is enabled, no external resources are
//!   fetched. The `expand_empty_elements` config is enabled ONLY to split
//!   `<x/>` into `<x></x>` for uniform child parsing — it does NOT expand
//!   entities.
//! - **CDATA handling (DMN-04).** `Event::Text` and `Event::CData` events are
//!   accumulated into [`TextContent`]; if any `CData` segment was seen,
//!   `is_cdata` is set so the exporter (Plan 29-03) re-emits CDATA.
//! - **`hitPolicy` default = UNIQUE (DMN-04).** [`DecisionTable::default`]
//!   sets `hit_policy = HitPolicy::Unique`; the parser keeps that default
//!   when the `hitPolicy` attribute is absent and adds an explicit guard.
//! - **Element order (Pitfall 11).** `Vec` insertion order is preserved for
//!   `definitions.children`, `input_clauses`, `output_clauses`, `rules`,
//!   `item_components`, and rule entries — never sorted.
//! - **FEEL is NOT parsed here.** All FEEL cell / expression text is carried
//!   as `String` / `TextContent`; Phase 31 parses it into a distinct
//!   `FeelExpression` AST.

use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{QName, ResolveResult};
use quick_xml::reader::NsReader;

use crate::dmn::model::{
    AllowedValues, Annotation, Association, BuiltinAggregator, BusinessKnowledgeModel, Decision,
    DecisionService, DecisionTable, Definitions, DefinitionsChild, DmnDiagram, DmnEdge, DmnLabel,
    DmnModel, DmnParseError, DmnRule, DmnShape, HitPolicy, InformationItem, InputClause, InputData,
    ItemDefinition, KnowledgeSource, OutputClause, RuleEntry, TextAnnotation, TextContent,
};

/// Recognized DMN MODEL namespace URIs (DMN 1.1 / 1.3 / 1.5 — DMN-04).
const DMN_MODEL_NAMESPACES: [&str; 3] = [
    // DMN 1.5
    "https://www.omg.org/spec/DMN/20200524/MODEL/",
    // DMN 1.3
    "https://www.omg.org/spec/DMN/20180521/MODEL/",
    // DMN 1.1 (historical XSD-style namespace)
    "http://www.omg.org/spec/DMN/20151101/dmn.xsd",
];

/// Parse a DMN document from a string. Wraps [`parse_dmn_bytes`].
pub fn parse_dmn(input: &str) -> Result<DmnModel, DmnParseError> {
    parse_dmn_bytes(input.as_bytes())
}

/// Parse a DMN document from bytes — the real implementation.
pub fn parse_dmn_bytes(input: &[u8]) -> Result<DmnModel, DmnParseError> {
    Parser::new(input)?.parse()
}

/// Parse a DMN document from a file path.
pub fn parse_dmn_file(path: &str) -> Result<DmnModel, DmnParseError> {
    let content =
        std::fs::read_to_string(path).map_err(|e| DmnParseError::Io(e.to_string()))?;
    parse_dmn(&content)
}

// =========================================================================
// Internal streaming parser
// =========================================================================

/// Streaming pull-parser state over a `&[u8]` DMN source.
struct Parser<'a> {
    reader: NsReader<&'a [u8]>,
    buf: Vec<u8>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8]) -> Result<Self, DmnParseError> {
        let mut reader = NsReader::from_reader(input);
        // Preserve CDATA/text fidelity — do not trim whitespace inside text.
        reader.config_mut().trim_text(false);
        // Split <x/> into <x></x> so child-parsing logic is uniform. This ONLY
        // affects empty-element splitting; it does NOT expand entities (XXE
        // hardening is via DOCTYPE/PI rejection below).
        reader.config_mut().expand_empty_elements = true;
        Ok(Self {
            reader,
            buf: Vec::new(),
        })
    }

    /// Drive the document: skip the XML declaration, reject DOCTYPE/PI,
    /// locate `<definitions>`, parse it, return the model.
    fn parse(mut self) -> Result<DmnModel, DmnParseError> {
        let mut definitions: Option<Definitions> = None;
        let mut source_ns: Option<String> = None;
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Decl(_)) => {}
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    if local_name_str(e.name()) == "definitions" {
                        source_ns = self.resolve_element_ns(e.name());
                        // Lenient namespace check: record but do not hard-reject
                        // unrecognized roots (T-29-04 accept). The three recognized
                        // DMN MODEL namespaces (1.1/1.3/1.5) are all accepted;
                        // unknown namespaces parse by local-name (foreign).
                        if let Some(ref ns) = source_ns {
                            let _recognized = DMN_MODEL_NAMESPACES.contains(&ns.as_str());
                        }
                        definitions = Some(self.parse_definitions(&e)?);
                    }
                    // Non-definitions start elements outside the root are
                    // ignored (foreign content / comments).
                }
                Ok(Event::Eof) => break,
                // Ignore stray text/comments outside the root.
                Ok(Event::End(_)) | Ok(Event::Text(_)) | Ok(Event::CData(_)) |
                Ok(Event::Empty(_)) | Ok(Event::Comment(_)) => {}
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        let definitions = definitions.ok_or_else(|| {
            DmnParseError::MissingElement("definitions root element".into())
        })?;
        Ok(DmnModel {
            definitions,
            source_namespace: source_ns,
        })
    }

    /// Resolve the namespace URI of a qualified element name, if bound.
    fn resolve_element_ns(&self, qname: QName) -> Option<String> {
        let (res, _local) = self.reader.resolver().resolve_element(qname);
        match res {
            ResolveResult::Bound(ns) => Some(String::from_utf8_lossy(ns.as_ref()).into_owned()),
            _ => None,
        }
    }

    // ---- <definitions> root + ordered children ---------------------------

    fn parse_definitions(&mut self, start: &BytesStart) -> Result<Definitions, DmnParseError> {
        let mut defs = Definitions::default();
        let attrs = collect_attrs(start)?;
        if let Some(v) = attrs.get("id") {
            defs.id = Some(v.clone());
        }
        if let Some(v) = attrs.get("name") {
            defs.name = Some(v.clone());
        }
        if let Some(v) = attrs.get("namespace") {
            defs.namespace = v.clone();
        }
        if let Some(v) = attrs.get("typeLanguage") {
            defs.type_language = Some(v.clone());
        }
        if let Some(v) = attrs.get("expressionLanguage") {
            defs.expression_language = Some(v.clone());
        }
        if let Some(v) = attrs.get("exporter") {
            defs.exporter = Some(v.clone());
        }
        if let Some(v) = attrs.get("exporterVersion") {
            defs.exporter_version = Some(v.clone());
        }
        let parent = "definitions";
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => defs.description = Some(self.read_text_content(&ln)?),
                        "itemDefinition" => {
                            let item = self.parse_item_definition_body(&e, "itemDefinition")?;
                            defs.children.push(DefinitionsChild::ItemDefinition(item));
                        }
                        "decision" => {
                            let d = self.parse_decision(&e)?;
                            defs.children.push(DefinitionsChild::Decision(d));
                        }
                        "inputData" => {
                            let d = self.parse_input_data(&e)?;
                            defs.children.push(DefinitionsChild::InputData(d));
                        }
                        "businessKnowledgeModel" => {
                            let b = self.parse_bkm(&e)?;
                            defs.children.push(DefinitionsChild::BusinessKnowledgeModel(b));
                        }
                        "decisionService" => {
                            let ds = self.parse_decision_service(&e)?;
                            defs.children.push(DefinitionsChild::DecisionService(ds));
                        }
                        "knowledgeSource" => {
                            let ks = self.parse_knowledge_source(&e)?;
                            defs.children.push(DefinitionsChild::KnowledgeSource(ks));
                        }
                        "textAnnotation" => {
                            let ta = self.parse_text_annotation(&e)?;
                            defs.children.push(DefinitionsChild::TextAnnotation(ta));
                        }
                        "association" => {
                            let a = self.parse_association(&e)?;
                            defs.children.push(DefinitionsChild::Association(a));
                        }
                        "DMNDI" => {
                            let diags = self.parse_dmndi_block(&e)?;
                            defs.diagrams.extend(diags);
                        }
                        _ => {
                            self.skip_element(&ln)?;
                        }
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == parent {
                        break;
                    }
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected end </{}> while parsing <{}>",
                        local_name_str(e.name()),
                        parent
                    )));
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::MissingElement(
                        "closing </definitions>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(defs)
    }

    // ---- Clause 7: <itemDefinition> + nested <itemComponent> -------------

    /// Parse an `<itemDefinition>` (or `<itemComponent>`) body. `end_local`
    /// distinguishes the two closing tags so recursion works.
    fn parse_item_definition_body(
        &mut self,
        start: &BytesStart,
        end_local: &str,
    ) -> Result<ItemDefinition, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let mut item = ItemDefinition {
            id: attrs.get("id").cloned(),
            name: attrs.get("name").cloned().unwrap_or_default(),
            description: None,
            type_ref: attrs.get("typeRef").cloned(),
            is_collection: attrs
                .get("isCollection")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            type_language: attrs.get("typeLanguage").cloned(),
            allowed_values: None,
            item_components: Vec::new(),
            annotations: Vec::new(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            item.description = Some(self.read_text_content(&ln)?);
                        }
                        "typeRef" => {
                            item.type_ref = Some(self.read_text_string(&ln)?);
                        }
                        "allowedValues" => {
                            item.allowed_values = Some(self.parse_allowed_values(&e)?);
                        }
                        "itemComponent" => {
                            // Nested item definition — recurse with its own
                            // closing tag.
                            let comp = self.parse_item_definition_body(&e, "itemComponent")?;
                            item.item_components.push(comp);
                        }
                        "annotation" => {
                            item.annotations.push(self.parse_annotation(&e)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == end_local {
                        break;
                    }
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected end </{}> while parsing <{}>",
                        local_name_str(e.name()),
                        end_local
                    )));
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected EOF while parsing <{}>",
                        end_local
                    )));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(item)
    }

    /// Parse an `<allowedValues>` / `<outputValues>` / `<inputValues>` block.
    ///
    /// A single `<text>` child → [`AllowedValues::UnaryTests`]; one or more
    /// `<itemComponent>` children → [`AllowedValues::Enumeration`] (each
    /// itemComponent's text content preserved as [`TextContent`]).
    fn parse_allowed_values(
        &mut self,
        start: &BytesStart,
    ) -> Result<AllowedValues, DmnParseError> {
        let parent = local_name_str(start.name());
        let mut unary: Option<String> = None;
        let mut enum_items: Vec<TextContent> = Vec::new();
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "text" => {
                            unary = Some(self.read_text_string(&ln)?);
                        }
                        "itemComponent" => {
                            // Each enumeration literal: read the text content
                            // inside the itemComponent — either a `<text>`
                            // child (the standard DMN form) or raw text.
                            let tc = self.read_text_or_text_child(&ln)?;
                            enum_items.push(tc);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == parent {
                        break;
                    }
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected end </{}> while parsing <{}>",
                        local_name_str(e.name()),
                        parent
                    )));
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected EOF while parsing <{}>",
                        parent
                    )));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        if let Some(u) = unary {
            Ok(AllowedValues::UnaryTests(u))
        } else {
            Ok(AllowedValues::Enumeration(enum_items))
        }
    }

    // ---- <decision> + <decisionTable> -----------------------------------

    fn parse_decision(&mut self, start: &BytesStart) -> Result<Decision, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let id = attrs
            .get("id")
            .cloned()
            .ok_or_else(|| DmnParseError::MissingElement("decision id attribute".into()))?;
        let mut dec = Decision {
            id,
            name: attrs.get("name").cloned(),
            description: None,
            variable: None,
            question: attrs.get("question").cloned(),
            allowed_answers: attrs.get("allowedAnswers").cloned(),
            decision_table: None,
            required_decisions: Vec::new(),
            required_inputs: Vec::new(),
            required_knowledge: Vec::new(),
            annotations: Vec::new(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            dec.description = Some(self.read_text_content(&ln)?);
                        }
                        "question" => {
                            dec.question = Some(self.read_text_string(&ln)?);
                        }
                        "allowedAnswers" => {
                            dec.allowed_answers = Some(self.read_text_string(&ln)?);
                        }
                        "variable" | "informationItem" => {
                            dec.variable = Some(self.parse_information_item(&e, &ln)?);
                        }
                        "informationRequirement" => {
                            self.parse_information_requirement(&e, &mut dec)?;
                        }
                        "knowledgeRequirement" => {
                            self.parse_knowledge_requirement(&e, &mut dec)?;
                        }
                        "decisionTable" => {
                            dec.decision_table = Some(self.parse_decision_table(&e)?);
                        }
                        "annotation" => {
                            dec.annotations.push(self.parse_annotation(&e)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "decision" {
                        break;
                    }
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected end </{}> while parsing <decision>",
                        local_name_str(e.name())
                    )));
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <decision>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(dec)
    }

    fn parse_information_item(
        &mut self,
        start: &BytesStart,
        end_local: &str,
    ) -> Result<InformationItem, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let mut item = InformationItem {
            id: attrs.get("id").cloned(),
            name: attrs.get("name").cloned().unwrap_or_default(),
            description: None,
            type_ref: attrs.get("typeRef").cloned(),
            annotations: Vec::new(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            item.description = Some(self.read_text_content(&ln)?);
                        }
                        "typeRef" => {
                            item.type_ref = Some(self.read_text_string(&ln)?);
                        }
                        "annotation" => {
                            item.annotations.push(self.parse_annotation(&e)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == end_local {
                        break;
                    }
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected end </{}> while parsing <{}>",
                        local_name_str(e.name()),
                        end_local
                    )));
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected EOF while parsing <{}>",
                        end_local
                    )));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(item)
    }

    fn parse_information_requirement(
        &mut self,
        start: &BytesStart,
        dec: &mut Decision,
    ) -> Result<(), DmnParseError> {
        let parent = local_name_str(start.name());
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "requiredDecision" => {
                            let href = read_href_attribute(&e)?;
                            if let Some(h) = href {
                                dec.required_decisions.push(strip_href(&h));
                            }
                            self.skip_element(&ln)?;
                        }
                        "requiredInput" => {
                            let href = read_href_attribute(&e)?;
                            if let Some(h) = href {
                                dec.required_inputs.push(strip_href(&h));
                            }
                            self.skip_element(&ln)?;
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == parent {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected EOF while parsing <{}>",
                        parent
                    )));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(())
    }

    fn parse_knowledge_requirement(
        &mut self,
        start: &BytesStart,
        dec: &mut Decision,
    ) -> Result<(), DmnParseError> {
        let parent = local_name_str(start.name());
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "requiredKnowledge" => {
                            let href = read_href_attribute(&e)?;
                            if let Some(h) = href {
                                dec.required_knowledge.push(strip_href(&h));
                            }
                            self.skip_element(&ln)?;
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == parent {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected EOF while parsing <{}>",
                        parent
                    )));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(())
    }

    fn parse_decision_table(
        &mut self,
        start: &BytesStart,
    ) -> Result<DecisionTable, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let mut dt = DecisionTable::default();
        if let Some(v) = attrs.get("id") {
            dt.id = Some(v.clone());
        }
        if let Some(v) = attrs.get("name") {
            dt.name = Some(v.clone());
        }
        let mut hit_policy_seen = false;
        if let Some(v) = attrs.get("hitPolicy") {
            dt.hit_policy = parse_hit_policy(v);
            hit_policy_seen = true;
        }
        // The aggregation attribute is only meaningful for COLLECT (spec
        // §8.2.12). Parse it but only assign when hitPolicy == COLLECT.
        let raw_aggregation = attrs.get("aggregation").cloned();
        if let Some(v) = attrs.get("preferredOrientation") {
            dt.preferred_orientation = Some(v.clone());
        }
        if let Some(v) = attrs.get("outputLabel") {
            dt.output_label = Some(v.clone());
        }
        // DMN-04: an absent hitPolicy attribute defaults to HitPolicy::Unique.
        if !hit_policy_seen {
            dt.hit_policy = HitPolicy::Unique;
        }
        if dt.hit_policy == HitPolicy::Collect {
            if let Some(raw) = raw_aggregation {
                dt.aggregation = parse_aggregator(&raw);
            }
        }
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "input" => dt.input_clauses.push(self.parse_input_clause(&e)?),
                        "output" => dt.output_clauses.push(self.parse_output_clause(&e)?),
                        "rule" => {
                            let n_in = dt.input_clauses.len();
                            let n_out = dt.output_clauses.len();
                            dt.rules.push(self.parse_rule(&e, n_in, n_out)?);
                        }
                        "description" => {
                            // decisionTable-level description (rare) — skip.
                            self.skip_element(&ln)?;
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "decisionTable" {
                        break;
                    }
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected end </{}> while parsing <decisionTable>",
                        local_name_str(e.name())
                    )));
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <decisionTable>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(dt)
    }

    fn parse_input_clause(&mut self, start: &BytesStart) -> Result<InputClause, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let mut ic = InputClause {
            id: attrs.get("id").cloned(),
            label: attrs.get("label").cloned(),
            input_expression: String::new(),
            input_values: None,
            description: None,
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            ic.description = Some(self.read_text_content(&ln)?);
                        }
                        "inputExpression" => {
                            ic.input_expression = self.parse_feel_text_child(&e, &ln)?;
                        }
                        "inputValues" => {
                            ic.input_values = Some(self.parse_allowed_values(&e)?);
                        }
                        "label" => {
                            ic.label = Some(self.read_text_string(&ln)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "input" {
                        break;
                    }
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected end </{}> while parsing <input>",
                        local_name_str(e.name())
                    )));
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <input>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(ic)
    }

    fn parse_output_clause(&mut self, start: &BytesStart) -> Result<OutputClause, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let mut oc = OutputClause {
            id: attrs.get("id").cloned(),
            label: attrs.get("label").cloned(),
            name: attrs.get("name").cloned(),
            type_ref: attrs.get("typeRef").cloned(),
            output_values: None,
            default_output_entry: None,
            description: None,
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            oc.description = Some(self.read_text_content(&ln)?);
                        }
                        "outputValues" => {
                            oc.output_values = Some(self.parse_allowed_values(&e)?);
                        }
                        "defaultOutputEntry" => {
                            oc.default_output_entry = Some(self.parse_feel_text_child(&e, &ln)?);
                        }
                        "typeRef" => {
                            oc.type_ref = Some(self.read_text_string(&ln)?);
                        }
                        "label" => {
                            oc.label = Some(self.read_text_string(&ln)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "output" {
                        break;
                    }
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected end </{}> while parsing <output>",
                        local_name_str(e.name())
                    )));
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <output>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(oc)
    }

    fn parse_rule(
        &mut self,
        start: &BytesStart,
        n_inputs: usize,
        n_outputs: usize,
    ) -> Result<DmnRule, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let mut rule = DmnRule {
            id: attrs.get("id").cloned(),
            description: None,
            input_entries: Vec::new(),
            output_entries: Vec::new(),
            annotation_entries: Vec::new(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            rule.description = Some(self.read_text_content(&ln)?);
                        }
                        "inputEntry" => {
                            rule.input_entries.push(self.parse_rule_entry(&e, "inputEntry")?);
                        }
                        "outputEntry" => {
                            rule.output_entries.push(self.parse_rule_entry(&e, "outputEntry")?);
                        }
                        "annotationEntry" => {
                            rule
                                .annotation_entries
                                .push(self.parse_rule_entry(&e, "annotationEntry")?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "rule" {
                        break;
                    }
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected end </{}> while parsing <rule>",
                        local_name_str(e.name())
                    )));
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <rule>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        // Validate entry counts against clause counts (spec content model).
        if rule.input_entries.len() != n_inputs {
            return Err(DmnParseError::UnexpectedElement(format!(
                "rule {} has {} inputEntries but expected {} (input clause count)",
                rule.id.as_deref().unwrap_or("?"),
                rule.input_entries.len(),
                n_inputs
            )));
        }
        if rule.output_entries.len() != n_outputs {
            return Err(DmnParseError::UnexpectedElement(format!(
                "rule {} has {} outputEntries but expected {} (output clause count)",
                rule.id.as_deref().unwrap_or("?"),
                rule.output_entries.len(),
                n_outputs
            )));
        }
        Ok(rule)
    }

    fn parse_rule_entry(
        &mut self,
        _start: &BytesStart,
        end_local: &str,
    ) -> Result<RuleEntry, DmnParseError> {
        let mut text = TextContent::plain(String::new());
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "text" => {
                            text = self.read_text_content(&ln)?;
                        }
                        "description" => {
                            // entry descriptions are not modeled on RuleEntry;
                            // skip to preserve forward-compat without loss.
                            self.skip_element(&ln)?;
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == end_local {
                        break;
                    }
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected end </{}> while parsing <{}>",
                        local_name_str(e.name()),
                        end_local
                    )));
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected EOF while parsing <{}>",
                        end_local
                    )));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(RuleEntry { text })
    }

    // ---- DRG nodes: inputData / BKM / decisionService / knowledgeSource ---

    fn parse_input_data(&mut self, start: &BytesStart) -> Result<InputData, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let id = attrs
            .get("id")
            .cloned()
            .ok_or_else(|| DmnParseError::MissingElement("inputData id attribute".into()))?;
        let mut input = InputData {
            id,
            name: attrs.get("name").cloned(),
            description: None,
            variable: None,
            annotations: Vec::new(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            input.description = Some(self.read_text_content(&ln)?);
                        }
                        "variable" | "informationItem" => {
                            input.variable = Some(self.parse_information_item(&e, &ln)?);
                        }
                        "annotation" => {
                            input.annotations.push(self.parse_annotation(&e)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "inputData" {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <inputData>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(input)
    }

    fn parse_bkm(&mut self, start: &BytesStart) -> Result<BusinessKnowledgeModel, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let id = attrs.get("id").cloned().ok_or_else(|| {
            DmnParseError::MissingElement("businessKnowledgeModel id attribute".into())
        })?;
        let mut bkm = BusinessKnowledgeModel {
            id,
            name: attrs.get("name").cloned(),
            description: None,
            variable: None,
            knowledge_requirement: Vec::new(),
            annotations: Vec::new(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            bkm.description = Some(self.read_text_content(&ln)?);
                        }
                        "variable" | "informationItem" => {
                            bkm.variable = Some(self.parse_information_item(&e, &ln)?);
                        }
                        "knowledgeRequirement" => {
                            self.collect_required_knowledge(&e, &mut bkm.knowledge_requirement)?;
                        }
                        "annotation" => {
                            bkm.annotations.push(self.parse_annotation(&e)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "businessKnowledgeModel" {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <businessKnowledgeModel>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(bkm)
    }

    fn parse_decision_service(&mut self, start: &BytesStart) -> Result<DecisionService, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let id = attrs
            .get("id")
            .cloned()
            .ok_or_else(|| DmnParseError::MissingElement("decisionService id attribute".into()))?;
        let mut ds = DecisionService {
            id,
            name: attrs.get("name").cloned(),
            description: None,
            output_decisions: Vec::new(),
            encapsulated_decisions: Vec::new(),
            input_data: Vec::new(),
            annotations: Vec::new(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            ds.description = Some(self.read_text_content(&ln)?);
                        }
                        "outputDecision" => {
                            if let Some(h) = read_href_attribute(&e)? {
                                ds.output_decisions.push(strip_href(&h));
                            }
                            self.skip_element(&ln)?;
                        }
                        "encapsulatedDecision" => {
                            if let Some(h) = read_href_attribute(&e)? {
                                ds.encapsulated_decisions.push(strip_href(&h));
                            }
                            self.skip_element(&ln)?;
                        }
                        "inputData" => {
                            if let Some(h) = read_href_attribute(&e)? {
                                ds.input_data.push(strip_href(&h));
                            }
                            self.skip_element(&ln)?;
                        }
                        "annotation" => {
                            ds.annotations.push(self.parse_annotation(&e)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "decisionService" {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <decisionService>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(ds)
    }

    fn parse_knowledge_source(&mut self, start: &BytesStart) -> Result<KnowledgeSource, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let id = attrs
            .get("id")
            .cloned()
            .ok_or_else(|| DmnParseError::MissingElement("knowledgeSource id attribute".into()))?;
        let mut ks = KnowledgeSource {
            id,
            name: attrs.get("name").cloned(),
            description: None,
            type_ref: attrs.get("type").cloned(),
            annotations: Vec::new(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            ks.description = Some(self.read_text_content(&ln)?);
                        }
                        "annotation" => {
                            ks.annotations.push(self.parse_annotation(&e)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "knowledgeSource" {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <knowledgeSource>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(ks)
    }

    // ---- Artifacts: textAnnotation / association / annotation ------------

    fn parse_text_annotation(&mut self, start: &BytesStart) -> Result<TextAnnotation, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let mut ta = TextAnnotation {
            id: attrs.get("id").cloned(),
            text: TextContent::plain(attrs.get("text").cloned().unwrap_or_default()),
            text_format: attrs.get("textFormat").cloned(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "text" => {
                            ta.text = self.read_text_content(&ln)?;
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "textAnnotation" {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <textAnnotation>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(ta)
    }

    fn parse_association(&mut self, start: &BytesStart) -> Result<Association, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let mut a = Association {
            id: attrs.get("id").cloned(),
            source_ref: attrs.get("sourceRef").cloned(),
            target_ref: attrs.get("targetRef").cloned(),
            association_type: attrs.get("type").cloned(),
            annotation: None,
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "annotation" => {
                            a.annotation = Some(self.parse_annotation(&e)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "association" {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <association>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(a)
    }

    fn parse_annotation(&mut self, start: &BytesStart) -> Result<Annotation, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let mut ann = Annotation {
            id: attrs.get("id").cloned(),
            text: TextContent::plain(String::new()),
            annotation_type: attrs.get("annotationType").cloned(),
            text_format: attrs.get("textFormat").cloned(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "text" => {
                            ann.text = self.read_text_content(&ln)?;
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    let ln = local_name_str(e.name());
                    if ln == "annotation" {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <annotation>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(ann)
    }

    // ---- DMNDI passthrough (opaque structural carriers) -----------------

    fn parse_dmndi_block(&mut self, _start: &BytesStart) -> Result<Vec<DmnDiagram>, DmnParseError> {
        let parent = "DMNDI";
        let mut diagrams = Vec::new();
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "DMNDiagram" => {
                            diagrams.push(self.parse_dmndi_diagram(&e)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == parent {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected EOF while parsing <{}>",
                        parent
                    )));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(diagrams)
    }

    fn parse_dmndi_diagram(&mut self, start: &BytesStart) -> Result<DmnDiagram, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let mut diag = DmnDiagram {
            id: attrs.get("id").cloned(),
            name: attrs.get("name").cloned(),
            shapes: Vec::new(),
            edges: Vec::new(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "DMNShape" => diag.shapes.push(self.parse_dmn_shape(&e)?),
                        "DMNEdge" => diag.edges.push(self.parse_dmn_edge(&e)?),
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "DMNDiagram" {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <DMNDiagram>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(diag)
    }

    fn parse_dmn_shape(&mut self, start: &BytesStart) -> Result<DmnShape, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let element_id = attrs
            .get("dmnElementRef")
            .cloned()
            .unwrap_or_default();
        let mut bounds = None;
        let mut labels = Vec::new();
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "Bounds" => {
                            bounds = Some(self.parse_dc_bounds(&e)?);
                        }
                        "DMNLabel" | "label" => {
                            labels.push(self.parse_dmn_label(&e)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "DMNShape" {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <DMNShape>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(DmnShape {
            element_id,
            bounds,
            labels,
        })
    }

    fn parse_dmn_edge(&mut self, start: &BytesStart) -> Result<DmnEdge, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let mut edge = DmnEdge {
            id: attrs.get("id").cloned(),
            source_id: attrs.get("source").cloned().unwrap_or_default(),
            target_id: attrs.get("target").cloned().unwrap_or_default(),
            waypoints: Vec::new(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "waypoint" => {
                            edge.waypoints.push(self.parse_dc_point(&e)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "DMNEdge" {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while parsing <DMNEdge>".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(edge)
    }

    fn parse_dc_bounds(&mut self, start: &BytesStart) -> Result<crate::dmn::model::DcBounds, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let x = attrs.get("x").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let y = attrs.get("y").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let width = attrs.get("width").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let height = attrs.get("height").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        self.skip_element("Bounds")?;
        Ok(crate::dmn::model::DcBounds {
            x,
            y,
            width,
            height,
        })
    }

    fn parse_dc_point(&mut self, start: &BytesStart) -> Result<crate::dmn::model::DcPoint, DmnParseError> {
        let attrs = collect_attrs(start)?;
        let x = attrs.get("x").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let y = attrs.get("y").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        self.skip_element("waypoint")?;
        Ok(crate::dmn::model::DcPoint { x, y })
    }

    fn parse_dmn_label(&mut self, start: &BytesStart) -> Result<DmnLabel, DmnParseError> {
        let mut bounds = None;
        let mut text = None;
        let parent = local_name_str(start.name());
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "Bounds" => {
                            bounds = Some(self.parse_dc_bounds(&e)?);
                        }
                        "text" => {
                            text = Some(self.read_text_string(&ln)?);
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == parent {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected EOF while parsing <{}>",
                        parent
                    )));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(DmnLabel { bounds, text })
    }

    // ---- Shared helpers -------------------------------------------------

    /// Collect the attributes of a start element into a map keyed by local
    /// name (namespace declarations skipped). Escapes are decoded.

    /// Collect `<requiredKnowledge>` hrefs into a BKM's knowledge_requirement.
    fn collect_required_knowledge(
        &mut self,
        start: &BytesStart,
        out: &mut Vec<String>,
    ) -> Result<(), DmnParseError> {
        let parent = local_name_str(start.name());
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    if ln == "requiredKnowledge" {
                        if let Some(h) = read_href_attribute(&e)? {
                            out.push(strip_href(&h));
                        }
                        self.skip_element(&ln)?;
                    } else {
                        self.skip_element(&ln)?;
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == parent {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected EOF while parsing <{}>",
                        parent
                    )));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(())
    }

    /// Parse an element whose body is a single `<text>` child carrying FEEL
    /// text (e.g. `<inputExpression>`, `<defaultOutputEntry>`). Returns the
    /// unescaped text content.
    fn parse_feel_text_child(
        &mut self,
        _start: &BytesStart,
        end_local: &str,
    ) -> Result<String, DmnParseError> {
        let mut text = String::new();
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    if ln == "text" {
                        text = self.read_text_string(&ln)?;
                    } else {
                        self.skip_element(&ln)?;
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == end_local {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected EOF while parsing <{}>",
                        end_local
                    )));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(text)
    }

    /// Read the text content of an element that may carry it either as raw
    /// text/CDATA or as a `<text>` child (the standard DMN enumeration form
    /// for `itemComponent`). If a `<text>` child is present, its content (and
    /// CDATA flag) is used; otherwise raw `Text`/`CData` events are accumulated.
    fn read_text_or_text_child(
        &mut self,
        end_local: &str,
    ) -> Result<TextContent, DmnParseError> {
        let mut content = String::new();
        let mut is_cdata = false;
        let mut saw_text_child = false;
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => { let e = e.into_owned();
                    let ln = local_name_str(e.name());
                    if ln == "text" {
                        saw_text_child = true;
                        let tc = self.read_text_content("text")?;
                        content.push_str(&tc.content);
                        if tc.is_cdata {
                            is_cdata = true;
                        }
                    } else {
                        self.skip_element(&ln)?;
                    }
                }
                Ok(Event::Text(t)) => {
                    if !saw_text_child {
                        let s = t
                            .decode()
                            .map_err(|e| DmnParseError::XmlError(e.to_string()))?;
                        content.push_str(&s);
                    }
                }
                Ok(Event::CData(c)) => {
                    if !saw_text_child {
                        is_cdata = true;
                        content.push_str(&String::from_utf8_lossy(c.as_ref()));
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == end_local {
                        break;
                    }
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected end </{}> while parsing <{}>",
                        local_name_str(e.name()),
                        end_local
                    )));
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF in text content".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(TextContent { content, is_cdata })
    }

    /// Read text/CDATA content until the closing End event matching `end_local`.
    /// Sets `is_cdata = true` if any `Event::CData` segment was seen.
    fn read_text_content(&mut self, end_local: &str) -> Result<TextContent, DmnParseError> {
        let mut content = String::new();
        let mut is_cdata = false;
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Text(t)) => {
                    let s = t
                        .decode()
                        .map_err(|e| DmnParseError::XmlError(e.to_string()))?;
                    content.push_str(&s);
                }
                Ok(Event::CData(c)) => {
                    is_cdata = true;
                    content.push_str(&String::from_utf8_lossy(c.as_ref()));
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == end_local {
                        break;
                    }
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected end </{}> while reading text content of <{}>",
                        local_name_str(e.name()),
                        end_local
                    )));
                }
                Ok(Event::Start(s)) => {
                    let ln = local_name_str(s.name());
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected child <{}> inside text content of <{}>",
                        ln,
                        end_local
                    )));
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF in text content".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(TextContent {
            content,
            is_cdata,
        })
    }

    /// Read text/CDATA content and return just the string (CDATA flag discarded).
    fn read_text_string(&mut self, end_local: &str) -> Result<String, DmnParseError> {
        Ok(self.read_text_content(end_local)?.content)
    }

    /// Skip one balanced element (depth 1 → 0). Used for unknown/foreign
    /// elements so unknown namespaces pass through without error (T-29-04).
    fn skip_element(&mut self, _local: &str) -> Result<(), DmnParseError> {
        let mut depth = 1;
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(_)) => depth += 1,
                Ok(Event::End(_)) => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Ok(Event::DocType(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "DOCTYPE declarations are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::PI(_)) => {
                    return Err(DmnParseError::XxeRejected(
                        "processing instructions are not permitted (XXE hardening)".into(),
                    ));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected EOF while skipping element".into(),
                    ));
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        Ok(())
    }
}

// =========================================================================
// Free helpers
// =========================================================================

/// The local name (post-`:` part) of a qualified name, as a `String`.
fn local_name_str(qname: QName) -> String {
    String::from_utf8_lossy(qname.local_name().as_ref()).into_owned()
}

/// Collect a start element's attributes into a `HashMap<String, String>`
/// keyed by local name. XML namespace declarations (`xmlns` / `xmlns:*`)
/// are skipped.
#[allow(deprecated)] // `unescape_value` is deprecated in quick-xml 0.40 but
                     // still functional; the replacement needs an `XmlVersion`
                     // context not available without the reader. Behavior is
                     // identical per the changelog.
fn collect_attrs(e: &BytesStart) -> Result<HashMap<String, String>, DmnParseError> {
    let mut map = HashMap::new();
    for attr in e.attributes() {
        let attr = attr.map_err(|err| DmnParseError::XmlError(err.to_string()))?;
        // Skip namespace declarations (xmlns / xmlns:prefix).
        if attr.key.as_ref().starts_with(b"xmlns") {
            continue;
        }
        let key = local_name_str(attr.key);
        let val = attr
            .unescape_value()
            .map_err(|err| DmnParseError::XmlError(err.to_string()))?
            .into_owned();
        map.insert(key, val);
    }
    Ok(map)
}

/// Read the `href` attribute of a start element (used for DRD edge refs).
#[allow(deprecated)]
fn read_href_attribute(e: &BytesStart) -> Result<Option<String>, DmnParseError> {
    for attr in e.attributes() {
        let attr = attr.map_err(|err| DmnParseError::XmlError(err.to_string()))?;
        if local_name_str(attr.key) == "href" {
            let v = attr
                .unescape_value()
                .map_err(|err| DmnParseError::XmlError(err.to_string()))?;
            return Ok(Some(v.into_owned()));
        }
    }
    Ok(None)
}

/// Map a DMN `hitPolicy` attribute string to a [`HitPolicy`].
fn parse_hit_policy(s: &str) -> HitPolicy {
    match s {
        "UNIQUE" | "" => HitPolicy::Unique,
        "ANY" => HitPolicy::Any,
        "PRIORITY" => HitPolicy::Priority,
        "FIRST" => HitPolicy::First,
        "RULE ORDER" => HitPolicy::RuleOrder,
        "OUTPUT ORDER" => HitPolicy::OutputOrder,
        "COLLECT" => HitPolicy::Collect,
        _ => HitPolicy::Unique, // unknown -> spec default
    }
}

/// Map a DMN `aggregation` attribute string to a [`BuiltinAggregator`].
fn parse_aggregator(s: &str) -> Option<BuiltinAggregator> {
    match s {
        "SUM" => Some(BuiltinAggregator::Sum),
        "MIN" => Some(BuiltinAggregator::Min),
        "MAX" => Some(BuiltinAggregator::Max),
        "COUNT" => Some(BuiltinAggregator::Count),
        _ => None,
    }
}

/// Strip a leading `#` from an `href` reference (DRD edges use `#_id`).
fn strip_href(href: &str) -> String {
    href.strip_prefix('#').unwrap_or(href).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GREEN driver: a minimal valid DMN 1.5 document parses to a non-empty IR.
    /// (This was the RED test; against the full implementation it passes.)
    const MINIMAL_1_5: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20200524/MODEL/" id="_d1" name="Min" namespace="https://www.omg.org/spec/DMN/20200524/MODEL/">
  <itemDefinition id="_itemAge" name="Age" isCollection="false">
    <typeRef>feel:number</typeRef>
  </itemDefinition>
</definitions>"#;

    #[test]
    fn parser_smoke_red() {
        let model = parse_dmn(MINIMAL_1_5).expect("minimal DMN 1.5 should parse");
        assert!(
            !model.definitions.children.is_empty(),
            "definitions.children must not be empty"
        );
    }

    /// XXE payload with `<!DOCTYPE … <!ENTITY xxe SYSTEM …>>` MUST be rejected
    /// with `DmnParseError::XxeRejected` and MUST NOT read `/etc/passwd`.
    #[test]
    fn parser_rejects_doctype_inline() {
        let xxe = r#"<?xml version="1.0"?>
<!DOCTYPE definitions [
  <!ENTITY xxe SYSTEM "file:///etc/passwd">
]>
<definitions xmlns="https://www.omg.org/spec/DMN/20200524/MODEL/" id="_d1" namespace="https://www.omg.org/spec/DMN/20200524/MODEL/">
  <decision id="_dec"><description>&xxe;</description></decision>
</definitions>"#;
        let res = parse_dmn(xxe);
        assert!(matches!(res, Err(DmnParseError::XxeRejected(_))), "got {:?}", res);
    }
}