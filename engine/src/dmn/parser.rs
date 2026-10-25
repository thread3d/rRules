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
//! - **Zero-copy event handling (PERF-07).** The default quick-xml idiom
//!   `let e = e.into_owned();` re-copies the element name and every attribute
//!   byte into a fresh heap allocation on *every* start event — defeating
//!   zero-copy and duplicating the work `collect_attrs` then does again. We
//!   drop it from every dispatch loop below the root. Each loop reads a
//!   borrowed `Event::Start(e)`, extracts only what it needs — the local name
//!   as an owned `String` and, when a sub-parse needs it, an owned `Attrs` map
//!   — and then drops the borrow. `read_event_into` ties the event's lifetime
//!   to `&mut self.reader`, so non-lexical lifetimes end `e`'s borrow at its
//!   last use (`collect_attrs(&e)` or `local_name_str(e.name())`), and the
//!   subsequent recursive `&mut self` parse is sound with no owned event copy.
//!   Sub-parsers take the pre-collected `Attrs` / `parent` / `end_local`
//!   (owned, no buffer borrow) rather than a `&BytesStart`, which is what
//!   makes the borrow release possible. The single exception is the root
//!   `<definitions>` element, which still uses `into_owned()` once per document
//!   because resolving its namespace requires `&self.reader` while the qname
//!   is live — one allocation per document, negligible.
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

use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::name::{QName, ResolveResult};
use quick_xml::reader::NsReader;

use crate::dmn::model::{
    AllowedValues, Annotation, Association, BuiltinAggregator, BusinessKnowledgeModel, Decision,
    DecisionService, DecisionTable, Definitions, DefinitionsChild, DmnDiagram, DmnEdge, DmnLabel,
    DmnModel, DmnParseError, DmnRule, DmnShape, HitPolicy, InformationItem, InputClause, InputData,
    ItemDefinition, KnowledgeSource, OutputClause, RuleEntry, TextAnnotation, TextContent,
};

// FEEL AST cache (Phase 31-09, DMN-18): pre-parse every FEEL cell into the
// `FeelAstCache` at load time. The cache is a sidecar to `DmnModel` — never
// mutated by the IR loader beyond `get_or_parse` calls during the walk.
use crate::dmn::feel::{FeelAstCache, ParseEntry};

/// Owned attribute map for a start element, keyed by local name (namespace
/// declarations excluded). PERF-07 zero-copy: collected directly from the
/// buffer-backed `BytesStart` while its borrow is live, then carried as an
/// owned value into the recursive parse so no `BytesStart::into_owned()` copy
/// of the element name + attribute bytes is ever made.
type Attrs = HashMap<String, String>;

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
    let content = std::fs::read_to_string(path).map_err(|e| DmnParseError::Io(e.to_string()))?;
    parse_dmn(&content)
}

/// Parse a DMN document with FEEL AST pre-parsing (DMN-18, Phase 31-09).
///
/// Returns `(DmnModel, FeelAstCache)` — the `FeelAstCache` is a sidecar that
/// holds every parsed FEEL cell from decision-table IR fields. The cache is
/// populated during the walk below and read-only thereafter; it is owned by the
/// caller (the Phase 32 decision-table engine will borrow it for evaluation).
///
/// **Performance property:** a 30-cell table evaluated 10k times performs ~30
/// parses at load time, not 300k (each `get_or_parse` call is O(1) amortized
/// after the first parse per unique key).
pub fn parse_dmn_with_cache(input: &str) -> Result<(DmnModel, FeelAstCache), DmnParseError> {
    let model = parse_dmn_bytes(input.as_bytes())?;
    let cache = FeelAstCache::new();
    walk_ir_and_populate_cache(&model, &cache);
    Ok((model, cache))
}

/// Walk every FEEL-bearing field of an IR and pre-parse into the cache.
///
/// This is called once after `parse_dmn_bytes` returns a complete `DmnModel`.
/// It walks:
/// - Each [`Decision`] → each [`DecisionTable`] → input/output/annotation
///   clauses, rule entries, allowed values (DMN-18 wiring).
///
/// **Open Question A6 resolution:** Phase 29's `Decision` struct carries only
/// `decision_table: Option<DecisionTable>` — there are no non-table boxed
/// expressions in the IR yet. The cache parses them on demand (via
/// `get_or_parse`) when Phase 33 adds them; this walk covers the decision-table
/// path that exists today.
fn walk_ir_and_populate_cache(model: &DmnModel, cache: &FeelAstCache) {
    for child in &model.definitions.children {
        if let DefinitionsChild::Decision(decision) = child {
            walk_decision(decision, cache);
        }
    }
}

fn walk_decision(decision: &Decision, cache: &FeelAstCache) {
    if let Some(table) = &decision.decision_table {
        walk_decision_table(table, cache);
    }
    // A6 resolution (Phase 29): `Decision` carries only `decision_table`. Non-table
    // boxed expressions are NOT in the IR yet — they will be parsed on demand by
    // the evaluator when Phase 33 adds them. Documented: plan 31-09 Open Question A6.
}

fn walk_decision_table(table: &DecisionTable, cache: &FeelAstCache) {
    // Input clauses: `input_expression` is a FEEL Expression; `input_values` may be UnaryTests.
    for input in &table.input_clauses {
        if !input.input_expression.is_empty() {
            cache.get_or_parse(&input.input_expression, ParseEntry::Expression);
        }
        if let Some(AllowedValues::UnaryTests(ref text)) = input.input_values {
            if !text.is_empty() {
                cache.get_or_parse(text, ParseEntry::SimpleUnaryTests);
            }
        }
    }

    // Output clauses: `default_output_entry` is a FEEL Expression.
    for output in &table.output_clauses {
        if let Some(ref default) = output.default_output_entry {
            if !default.is_empty() {
                cache.get_or_parse(default, ParseEntry::Expression);
            }
        }
        if let Some(AllowedValues::UnaryTests(ref text)) = output.output_values {
            if !text.is_empty() {
                cache.get_or_parse(text, ParseEntry::SimpleUnaryTests);
            }
        }
    }

    // Rules: input_entries (SimpleUnaryTests), output_entries (Expression),
    // annotation_entries (Expression).
    for rule in &table.rules {
        for entry in &rule.input_entries {
            let text = entry.text.content.trim().to_string();
            if !text.is_empty() {
                cache.get_or_parse(&text, ParseEntry::SimpleUnaryTests);
            }
        }
        for entry in &rule.output_entries {
            let text = entry.text.content.trim().to_string();
            if !text.is_empty() {
                cache.get_or_parse(&text, ParseEntry::Expression);
            }
        }
        for entry in &rule.annotation_entries {
            let text = entry.text.content.trim().to_string();
            if !text.is_empty() {
                cache.get_or_parse(&text, ParseEntry::Expression);
            }
        }
    }
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
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    if ln == "definitions" {
                        // PERF-07: this is the ONE element per document that
                        // still uses `into_owned()`. Resolving the root namespace
                        // requires `self.resolve_element_ns` (`&self.reader`),
                        // but `read_event_into` ties the borrowed event's
                        // lifetime to `&mut self.reader` — so we cannot touch the
                        // reader while the borrowed event is held. `into_owned()`
                        // severs that borrow. This is a single allocation per
                        // *document* (negligible); every element below the root
                        // stays zero-copy.
                        let e = e.into_owned();
                        source_ns = self.resolve_element_ns(e.name());
                        let attrs = collect_attrs(&e)?;
                        // Lenient namespace check: record but do not hard-reject
                        // unrecognized roots (T-29-04 accept). The three recognized
                        // DMN MODEL namespaces (1.1/1.3/1.5) are all accepted;
                        // unknown namespaces parse by local-name (foreign).
                        if let Some(ref ns) = source_ns {
                            let _recognized = DMN_MODEL_NAMESPACES.contains(&ns.as_str());
                        }
                        definitions = Some(self.parse_definitions(attrs)?);
                    }
                    // Non-definitions start elements outside the root are
                    // ignored (foreign content / comments).
                }
                Ok(Event::Eof) => break,
                // Ignore stray text/comments outside the root.
                Ok(Event::End(_))
                | Ok(Event::Text(_))
                | Ok(Event::CData(_))
                | Ok(Event::Empty(_))
                | Ok(Event::Comment(_)) => {}
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }
        let definitions = definitions
            .ok_or_else(|| DmnParseError::MissingElement("definitions root element".into()))?;
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

    fn parse_definitions(&mut self, attrs: Attrs) -> Result<Definitions, DmnParseError> {
        let mut defs = Definitions::default();
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
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => defs.description = Some(self.read_text_content(&ln)?),
                        "itemDefinition" => {
                            let attrs = collect_attrs(&e)?;
                            let item = self.parse_item_definition_body(attrs, "itemDefinition")?;
                            defs.children.push(DefinitionsChild::ItemDefinition(item));
                        }
                        "decision" => {
                            let attrs = collect_attrs(&e)?;
                            let d = self.parse_decision(attrs)?;
                            defs.children.push(DefinitionsChild::Decision(d));
                        }
                        "inputData" => {
                            let attrs = collect_attrs(&e)?;
                            let d = self.parse_input_data(attrs)?;
                            defs.children.push(DefinitionsChild::InputData(d));
                        }
                        "businessKnowledgeModel" => {
                            let attrs = collect_attrs(&e)?;
                            let b = self.parse_bkm(attrs)?;
                            defs.children
                                .push(DefinitionsChild::BusinessKnowledgeModel(b));
                        }
                        "decisionService" => {
                            let attrs = collect_attrs(&e)?;
                            let ds = self.parse_decision_service(attrs)?;
                            defs.children.push(DefinitionsChild::DecisionService(ds));
                        }
                        "knowledgeSource" => {
                            let attrs = collect_attrs(&e)?;
                            let ks = self.parse_knowledge_source(attrs)?;
                            defs.children.push(DefinitionsChild::KnowledgeSource(ks));
                        }
                        "textAnnotation" => {
                            let attrs = collect_attrs(&e)?;
                            let ta = self.parse_text_annotation(attrs)?;
                            defs.children.push(DefinitionsChild::TextAnnotation(ta));
                        }
                        "association" => {
                            let attrs = collect_attrs(&e)?;
                            let a = self.parse_association(attrs)?;
                            defs.children.push(DefinitionsChild::Association(a));
                        }
                        "DMNDI" => {
                            let diags = self.parse_dmndi_block()?;
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
        attrs: Attrs,
        end_local: &str,
    ) -> Result<ItemDefinition, DmnParseError> {
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
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            item.description = Some(self.read_text_content(&ln)?);
                        }
                        "typeRef" => {
                            item.type_ref = Some(self.read_text_string(&ln)?);
                        }
                        "allowedValues" => {
                            item.allowed_values = Some(self.parse_allowed_values(&ln)?);
                        }
                        "itemComponent" => {
                            // Nested item definition — recurse with its own
                            // closing tag.
                            let attrs = collect_attrs(&e)?;
                            let comp = self.parse_item_definition_body(attrs, "itemComponent")?;
                            item.item_components.push(comp);
                        }
                        "annotation" => {
                            let attrs = collect_attrs(&e)?;
                            item.annotations.push(self.parse_annotation(attrs)?);
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
    fn parse_allowed_values(&mut self, parent: &str) -> Result<AllowedValues, DmnParseError> {
        let mut unary: Option<String> = None;
        let mut enum_items: Vec<TextContent> = Vec::new();
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
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

    fn parse_decision(&mut self, attrs: Attrs) -> Result<Decision, DmnParseError> {
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
            literal_expression: None,
            required_decisions: Vec::new(),
            required_inputs: Vec::new(),
            required_knowledge: Vec::new(),
            annotations: Vec::new(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
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
                            let attrs = collect_attrs(&e)?;
                            dec.variable = Some(self.parse_information_item(attrs, &ln)?);
                        }
                        "informationRequirement" => {
                            self.parse_information_requirement(&ln, &mut dec)?;
                        }
                        "knowledgeRequirement" => {
                            self.parse_knowledge_requirement(&ln, &mut dec)?;
                        }
                        "decisionTable" => {
                            let attrs = collect_attrs(&e)?;
                            dec.decision_table = Some(self.parse_decision_table(attrs)?);
                        }
                        "literalExpression" => {
                            let attrs = collect_attrs(&e)?;
                            dec.literal_expression = Some(self.parse_literal_expression(attrs)?);
                        }
                        "annotation" => {
                            let attrs = collect_attrs(&e)?;
                            dec.annotations.push(self.parse_annotation(attrs)?);
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
        attrs: Attrs,
        end_local: &str,
    ) -> Result<InformationItem, DmnParseError> {
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
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            item.description = Some(self.read_text_content(&ln)?);
                        }
                        "typeRef" => {
                            item.type_ref = Some(self.read_text_string(&ln)?);
                        }
                        "annotation" => {
                            let attrs = collect_attrs(&e)?;
                            item.annotations.push(self.parse_annotation(attrs)?);
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
        parent: &str,
        dec: &mut Decision,
    ) -> Result<(), DmnParseError> {
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
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
        parent: &str,
        dec: &mut Decision,
    ) -> Result<(), DmnParseError> {
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
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

    /// Parse `<literalExpression>` (DMN 1.5 §10.3.2): an id/typeRef plus the
    /// `<text>` child holding the FEEL expression.
    fn parse_literal_expression(
        &mut self,
        attrs: Attrs,
    ) -> Result<crate::dmn::model::LiteralExpression, DmnParseError> {
        let mut literal = crate::dmn::model::LiteralExpression {
            id: attrs.get("id").cloned(),
            type_ref: attrs.get("typeRef").cloned(),
            text: crate::dmn::model::TextContent::plain(String::new()),
            description: None,
        };

        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            literal.description = Some(self.read_text_content(&ln)?);
                        }
                        "text" => {
                            literal.text = self.read_text_content(&ln)?;
                        }
                        _ => self.skip_element(&ln)?,
                    }
                }
                Ok(Event::End(e)) => {
                    if local_name_str(e.name()) == "literalExpression" {
                        break;
                    }
                    return Err(DmnParseError::UnexpectedElement(format!(
                        "unexpected end </{}> while parsing <literalExpression>",
                        local_name_str(e.name())
                    )));
                }
                Ok(Event::Eof) => {
                    return Err(DmnParseError::UnexpectedElement(
                        "unexpected end of document inside <literalExpression>".into(),
                    ))
                }
                Ok(_) => {}
                Err(e) => return Err(DmnParseError::XmlError(e.to_string())),
            }
        }

        Ok(literal)
    }

    fn parse_decision_table(&mut self, attrs: Attrs) -> Result<DecisionTable, DmnParseError> {
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
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "input" => {
                            let attrs = collect_attrs(&e)?;
                            dt.input_clauses.push(self.parse_input_clause(attrs)?);
                        }
                        "output" => {
                            let attrs = collect_attrs(&e)?;
                            dt.output_clauses.push(self.parse_output_clause(attrs)?);
                        }
                        "rule" => {
                            let n_in = dt.input_clauses.len();
                            let n_out = dt.output_clauses.len();
                            let attrs = collect_attrs(&e)?;
                            dt.rules.push(self.parse_rule(attrs, n_in, n_out)?);
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

    fn parse_input_clause(&mut self, attrs: Attrs) -> Result<InputClause, DmnParseError> {
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
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            ic.description = Some(self.read_text_content(&ln)?);
                        }
                        "inputExpression" => {
                            ic.input_expression = self.parse_feel_text_child(&ln)?;
                        }
                        "inputValues" => {
                            ic.input_values = Some(self.parse_allowed_values(&ln)?);
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

    fn parse_output_clause(&mut self, attrs: Attrs) -> Result<OutputClause, DmnParseError> {
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
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            oc.description = Some(self.read_text_content(&ln)?);
                        }
                        "outputValues" => {
                            oc.output_values = Some(self.parse_allowed_values(&ln)?);
                        }
                        "defaultOutputEntry" => {
                            oc.default_output_entry = Some(self.parse_feel_text_child(&ln)?);
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
        attrs: Attrs,
        n_inputs: usize,
        n_outputs: usize,
    ) -> Result<DmnRule, DmnParseError> {
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
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            rule.description = Some(self.read_text_content(&ln)?);
                        }
                        "inputEntry" => {
                            rule.input_entries
                                .push(self.parse_rule_entry("inputEntry")?);
                        }
                        "outputEntry" => {
                            rule.output_entries
                                .push(self.parse_rule_entry("outputEntry")?);
                        }
                        "annotationEntry" => {
                            rule.annotation_entries
                                .push(self.parse_rule_entry("annotationEntry")?);
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

    fn parse_rule_entry(&mut self, end_local: &str) -> Result<RuleEntry, DmnParseError> {
        let mut text = TextContent::plain(String::new());
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
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

    fn parse_input_data(&mut self, attrs: Attrs) -> Result<InputData, DmnParseError> {
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
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            input.description = Some(self.read_text_content(&ln)?);
                        }
                        "variable" | "informationItem" => {
                            let attrs = collect_attrs(&e)?;
                            input.variable = Some(self.parse_information_item(attrs, &ln)?);
                        }
                        "annotation" => {
                            let attrs = collect_attrs(&e)?;
                            input.annotations.push(self.parse_annotation(attrs)?);
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

    fn parse_bkm(&mut self, attrs: Attrs) -> Result<BusinessKnowledgeModel, DmnParseError> {
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
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            bkm.description = Some(self.read_text_content(&ln)?);
                        }
                        "variable" | "informationItem" => {
                            let attrs = collect_attrs(&e)?;
                            bkm.variable = Some(self.parse_information_item(attrs, &ln)?);
                        }
                        "knowledgeRequirement" => {
                            self.collect_required_knowledge(&ln, &mut bkm.knowledge_requirement)?;
                        }
                        "annotation" => {
                            let attrs = collect_attrs(&e)?;
                            bkm.annotations.push(self.parse_annotation(attrs)?);
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

    fn parse_decision_service(&mut self, attrs: Attrs) -> Result<DecisionService, DmnParseError> {
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
                Ok(Event::Start(e)) => {
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
                            let attrs = collect_attrs(&e)?;
                            ds.annotations.push(self.parse_annotation(attrs)?);
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

    fn parse_knowledge_source(&mut self, attrs: Attrs) -> Result<KnowledgeSource, DmnParseError> {
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
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "description" => {
                            ks.description = Some(self.read_text_content(&ln)?);
                        }
                        "annotation" => {
                            let attrs = collect_attrs(&e)?;
                            ks.annotations.push(self.parse_annotation(attrs)?);
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

    fn parse_text_annotation(&mut self, attrs: Attrs) -> Result<TextAnnotation, DmnParseError> {
        let mut ta = TextAnnotation {
            id: attrs.get("id").cloned(),
            text: TextContent::plain(attrs.get("text").cloned().unwrap_or_default()),
            text_format: attrs.get("textFormat").cloned(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
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

    fn parse_association(&mut self, attrs: Attrs) -> Result<Association, DmnParseError> {
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
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "annotation" => {
                            let attrs = collect_attrs(&e)?;
                            a.annotation = Some(self.parse_annotation(attrs)?);
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

    fn parse_annotation(&mut self, attrs: Attrs) -> Result<Annotation, DmnParseError> {
        let mut ann = Annotation {
            id: attrs.get("id").cloned(),
            text: TextContent::plain(String::new()),
            annotation_type: attrs.get("annotationType").cloned(),
            text_format: attrs.get("textFormat").cloned(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
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

    fn parse_dmndi_block(&mut self) -> Result<Vec<DmnDiagram>, DmnParseError> {
        let parent = "DMNDI";
        let mut diagrams = Vec::new();
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "DMNDiagram" => {
                            let attrs = collect_attrs(&e)?;
                            diagrams.push(self.parse_dmndi_diagram(attrs)?);
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

    fn parse_dmndi_diagram(&mut self, attrs: Attrs) -> Result<DmnDiagram, DmnParseError> {
        let mut diag = DmnDiagram {
            id: attrs.get("id").cloned(),
            name: attrs.get("name").cloned(),
            shapes: Vec::new(),
            edges: Vec::new(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "DMNShape" => {
                            let attrs = collect_attrs(&e)?;
                            diag.shapes.push(self.parse_dmn_shape(attrs)?);
                        }
                        "DMNEdge" => {
                            let attrs = collect_attrs(&e)?;
                            diag.edges.push(self.parse_dmn_edge(attrs)?);
                        }
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

    fn parse_dmn_shape(&mut self, attrs: Attrs) -> Result<DmnShape, DmnParseError> {
        let element_id = attrs.get("dmnElementRef").cloned().unwrap_or_default();
        let mut bounds = None;
        let mut labels = Vec::new();
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "Bounds" => {
                            let attrs = collect_attrs(&e)?;
                            bounds = Some(self.parse_dc_bounds(attrs)?);
                        }
                        "DMNLabel" | "label" => {
                            labels.push(self.parse_dmn_label(&ln)?);
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

    fn parse_dmn_edge(&mut self, attrs: Attrs) -> Result<DmnEdge, DmnParseError> {
        let mut edge = DmnEdge {
            id: attrs.get("id").cloned(),
            source_id: attrs.get("source").cloned().unwrap_or_default(),
            target_id: attrs.get("target").cloned().unwrap_or_default(),
            waypoints: Vec::new(),
        };
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "waypoint" => {
                            let attrs = collect_attrs(&e)?;
                            edge.waypoints.push(self.parse_dc_point(attrs)?);
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

    fn parse_dc_bounds(
        &mut self,
        attrs: Attrs,
    ) -> Result<crate::dmn::model::DcBounds, DmnParseError> {
        let x = attrs.get("x").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let y = attrs.get("y").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let width = attrs
            .get("width")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        let height = attrs
            .get("height")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        self.skip_element("Bounds")?;
        Ok(crate::dmn::model::DcBounds {
            x,
            y,
            width,
            height,
        })
    }

    fn parse_dc_point(
        &mut self,
        attrs: Attrs,
    ) -> Result<crate::dmn::model::DcPoint, DmnParseError> {
        let x = attrs.get("x").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let y = attrs.get("y").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        self.skip_element("waypoint")?;
        Ok(crate::dmn::model::DcPoint { x, y })
    }

    fn parse_dmn_label(&mut self, parent: &str) -> Result<DmnLabel, DmnParseError> {
        let mut bounds = None;
        let mut text = None;
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
                    let ln = local_name_str(e.name());
                    match ln.as_str() {
                        "Bounds" => {
                            let attrs = collect_attrs(&e)?;
                            bounds = Some(self.parse_dc_bounds(attrs)?);
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

    /// Collect `<requiredKnowledge>` hrefs into a BKM's knowledge_requirement.
    fn collect_required_knowledge(
        &mut self,
        parent: &str,
        out: &mut Vec<String>,
    ) -> Result<(), DmnParseError> {
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
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
    fn parse_feel_text_child(&mut self, end_local: &str) -> Result<String, DmnParseError> {
        let mut text = String::new();
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
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
    fn read_text_or_text_child(&mut self, end_local: &str) -> Result<TextContent, DmnParseError> {
        let mut content = String::new();
        let mut is_cdata = false;
        let mut saw_text_child = false;
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => {
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
    /// Resolve a general/character entity reference (`&gt;`, `&amp;`, `&#62;`,
    /// ...) emitted as a separate [`Event::GeneralRef`] by quick-xml 0.40.
    ///
    /// quick-xml's streaming pull parser does NOT inline-decode entity
    /// references into adjacent `Event::Text` segments — it emits each
    /// `&name;` / `&#NN;` as its own `GeneralRef` event. If that event is
    /// dropped (as a naive `Ok(_) => {}` catch-all does), the resolved
    /// character silently vanishes from the text, e.g. `&gt;= 18` becomes
    /// `= 18`. This helper resolves the 5 predefined XML 1.0 entities plus
    /// numeric character references (`&#decimal;` / `&#xHEX;`). Because the
    /// parser rejects `<!DOCTYPE>`/`<!ENTITY>` (XXE hardening, T-29-01), no
    /// custom named entities can ever be defined, so any unknown named
    /// reference is an error rather than a silent drop.
    fn resolve_general_ref(r: &BytesRef) -> Result<String, DmnParseError> {
        // Numeric character reference (`&#62;` / `&#x3E;`).
        if r.is_char_ref() {
            return match r.resolve_char_ref() {
                Ok(Some(ch)) => Ok(ch.to_string()),
                Ok(None) => Err(DmnParseError::XmlError(format!(
                    "unresolvable character reference {:?}",
                    String::from_utf8_lossy(r.as_ref())
                ))),
                Err(e) => Err(DmnParseError::XmlError(e.to_string())),
            };
        }
        // Named entity — only the 5 predefined XML 1.0 entities are valid
        // (DOCTYPE/ENTITY declarations are rejected, so no custom entities
        // can be in scope).
        let name = r
            .decode()
            .map_err(|e| DmnParseError::XmlError(e.to_string()))?;
        let resolved = match name.as_ref() {
            "gt" => ">",
            "lt" => "<",
            "amp" => "&",
            "quot" => "\"",
            "apos" => "'",
            other => {
                return Err(DmnParseError::XmlError(format!(
                    "undefined named entity reference '&{};' (DOCTYPE/ENTITY declarations \
                     are rejected by XXE hardening; only gt/lt/amp/quot/apos are predefined)",
                    other
                )));
            }
        };
        Ok(resolved.to_string())
    }

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
                Ok(Event::GeneralRef(r)) => {
                    let e = r.into_owned();
                    content.push_str(&Self::resolve_general_ref(&e)?);
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
                        ln, end_local
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

/// The local name (post-`:` part) of a qualified name, as an owned `String`.
///
/// PERF-07 note: this is the one owned `String` we still allocate per start
/// element. It is unavoidable for the `match ln.as_str()` dispatch (a `&str`
/// borrowing the reusable read buffer would be invalidated by the next
/// `read_event_into`). The allocation is a single short `String` per element —
/// far cheaper than the `BytesStart::into_owned()` it replaces, which
/// re-copied the element name *and every attribute byte*.
fn local_name_str(qname: QName) -> String {
    String::from_utf8_lossy(qname.local_name().as_ref()).into_owned()
}

/// Collect a start element's attributes into a [`Attrs`] map keyed by local
/// name. XML namespace declarations (`xmlns` / `xmlns:*`) are skipped.
///
/// PERF-07: called while the `BytesStart` event still borrows the reusable read
/// buffer; returns owned `String`s so the result outlives the buffer borrow and
/// can be carried into a recursive `&mut self` parse.
#[allow(deprecated)] // `unescape_value` is deprecated in quick-xml 0.40 but
                     // still functional; the replacement needs an `XmlVersion`
                     // context not available without the reader. Behavior is
                     // identical per the changelog.
fn collect_attrs(e: &BytesStart) -> Result<Attrs, DmnParseError> {
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
        assert!(
            matches!(res, Err(DmnParseError::XxeRejected(_))),
            "got {:?}",
            res
        );
    }

    /// PERF-07 regression guard: a decision table with attributes, a
    /// `hitPolicy`, CDATA cell text, and a numeric entity reference (`&gt;`)
    /// must all survive the zero-copy refactor intact. Exercises every
    /// refactored path: definitions attrs → decision attrs → decisionTable
    /// attrs/hitPolicy → input/output clause attrs → rule attrs → ruleEntry
    /// text (CDATA + GeneralRef) → FEEL text child.
    #[test]
    fn parser_zero_copy_preserves_attrs_cdata_and_entities() {
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20200524/MODEL/" id="_d1" name="Eligibility" namespace="https://example.com/elig">
  <decision id="_dec" name="Approve">
    <decisionTable id="_dt" hitPolicy="UNIQUE">
      <input id="_in" label="Applicant age">
        <inputExpression><text>applicant.age</text></inputExpression>
      </input>
      <output id="_out" name="result" typeRef="feel:string"/>
      <rule id="_r1">
        <inputEntry><text><![CDATA[>= 18]]></text></inputEntry>
        <outputEntry><text>"adult"</text></outputEntry>
      </rule>
      <rule id="_r2">
        <inputEntry><text>&lt; 18</text></inputEntry>
        <outputEntry><text>"minor"</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#;
        let model = parse_dmn(doc).expect("decision-table doc should parse");
        let decision = model
            .definitions
            .children
            .iter()
            .find_map(|c| match c {
                DefinitionsChild::Decision(d) => Some(d),
                _ => None,
            })
            .expect("a decision exists");
        assert_eq!(decision.id, "_dec");
        assert_eq!(decision.name.as_deref(), Some("Approve"));
        let table = decision.decision_table.as_ref().expect("has table");
        assert_eq!(table.id.as_deref(), Some("_dt"));
        assert_eq!(table.hit_policy, HitPolicy::Unique);
        assert_eq!(table.input_clauses.len(), 1);
        assert_eq!(
            table.input_clauses[0].label.as_deref(),
            Some("Applicant age")
        );
        assert_eq!(table.input_clauses[0].input_expression, "applicant.age");
        assert_eq!(table.output_clauses.len(), 1);
        assert_eq!(table.output_clauses[0].name.as_deref(), Some("result"));
        assert_eq!(table.rules.len(), 2);
        // CDATA cell must carry the is_cdata flag and the raw text.
        assert_eq!(table.rules[0].input_entries[0].text.content, ">= 18");
        assert!(table.rules[0].input_entries[0].text.is_cdata);
        // Numeric-less named entity `&lt;` must resolve (GeneralRef path).
        assert_eq!(table.rules[1].input_entries[0].text.content, "< 18");
        assert_eq!(table.rules[0].output_entries[0].text.content, "\"adult\"");
    }
}
