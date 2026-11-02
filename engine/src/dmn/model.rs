//! Canonical DMN 1.5 in-memory IR (Plan 29-01 Task 3).
//!
//! This is the sole DMN data shape for every downstream phase: Plans 29-02
//! (parser) and 29-03 (exporter) read/write these types, and Phases 30–35
//! parse FEEL text carried on the IR or evaluate decision tables against it.
//! Round-trip fidelity is structural from day one (Pitfall 11): IDs,
//! descriptions, annotations, CDATA distinctions, and exact document element
//! order are preserved in the IR so the exporter can re-emit them without loss.
//!
//! ## Scope rules (load-bearing — do NOT regress)
//!
//! - **FEEL text is carried as `String` / [`TextContent`].** Phase 31 parses
//!   it into a distinct `FeelExpression` AST. The IR does NOT parse FEEL.
//! - **The IR does NOT reuse the rule-engine [`crate::data_types::Value`] or
//!   the rule-engine `Expression` AST.** DMN is stateless, side-effect-free,
//!   and three-valued; the Rete family is forward-chaining, working-memory-
//!   mutating, and two-valued. The two paths share neither an execution engine
//!   nor an expression AST (milestone research SUMMARY, "Anti-Pattern").
//! - **No `f64` in the FEEL-numeric path.** DMNDI geometry ([`DcBounds`] /
//!   [`DcPoint`]) legitimately uses `f64` — that is coordinate data, not FEEL
//!   numerics. Where a typed numeric literal is modeled in future phases, it
//!   uses `rust_decimal::Decimal` (Decimal128), never `f64`.
//! - **`[`DmnParseError`] lives here** so `model.rs` is the single error-type
//!   home; `parser.rs` (Plan 29-02) returns `Result<_, DmnParseError>`.
//!
//! Naming is load-bearing: Plans 29-02/29-03 and Phases 30–35 reference these
//! types by name.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// =========================================================================
// Root + ordering
// =========================================================================

/// Root wrapper for a parsed DMN 1.5 document.
///
/// Carries the [`Definitions`] element plus any namespace-prefix metadata
/// needed for round-trip (e.g. the prefix map observed by the parser, so the
/// exporter can re-emit equivalent `xmlns:*` declarations).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DmnModel {
    /// The `<definitions>` element — single root of a DMN document.
    pub definitions: Definitions,
    /// Observed XML namespace-prefix metadata for round-trip fidelity. The
    /// parser stores the prefix map here; the exporter re-emits equivalent
    /// `xmlns:*` declarations. `None` when no prefix metadata was captured.
    pub source_namespace: Option<String>,
}

impl DmnModel {
    /// Whether the document's root namespace is one of the DMN model namespaces this project knows.
    ///
    /// A document with an unrecognised root namespace parses fine — foreign and vendor namespaces are
    /// accepted on purpose, because rejecting them would refuse valid documents — so this is
    /// information for a caller, not a validity check. Until 9.0.66 the parser computed it and threw it
    /// away, which is why `DmnParseError::InvalidNamespace` could never be produced.
    pub fn namespace_is_recognised(&self) -> bool {
        self.source_namespace
            .as_deref()
            .map(crate::dmn::parser::is_known_dmn_namespace)
            .unwrap_or(false)
    }
}

/// The `<definitions>` element — the single root of a DMN document.
///
/// `children` preserves exact document element order across heterogeneous
/// child types (item definitions, decisions, input data, BKMs, …) so the
/// exporter can re-emit them in the original sequence (Pitfall 11).
/// `diagrams` carries DMNDI layout through the IR (populated in Plan 29-02;
/// emitted in Plan 29-03 / Phase 35).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Definitions {
    /// `id` attribute (optional on `<definitions>`).
    pub id: Option<String>,
    /// `name` attribute.
    pub name: Option<String>,
    /// `<description>` child (preserved as [`TextContent`] for CDATA round-trip).
    pub description: Option<TextContent>,
    /// Target/default namespace (`namespace` attribute) — required on `<definitions>`.
    pub namespace: String,
    /// `typeLanguage` attribute (URI, defaults to FEEL in DMN 1.5).
    pub type_language: Option<String>,
    /// `expressionLanguage` attribute (URI, defaults to FEEL in DMN 1.5).
    pub expression_language: Option<String>,
    /// `exporter` attribute (set by authoring tools on export).
    pub exporter: Option<String>,
    /// `exporterVersion` attribute.
    pub exporter_version: Option<String>,
    /// Ordered top-level children, preserving exact cross-type document order
    /// for round-trip (Pitfall 11).
    pub children: Vec<DefinitionsChild>,
    /// DMNDI diagrams carried through the IR (populated by the parser in
    /// Plan 29-02; emitted by the exporter in Plan 29-03 / Phase 35).
    pub diagrams: Vec<DmnDiagram>,
}

/// Ordered top-level child of [`Definitions`]. Each variant carries the
/// element so `Vec<DefinitionsChild>` preserves cross-type document order.
///
/// Variants follow the DMN 1.5 `definitionsChild` substitution group. The
/// annotation/association variants ([`Self::TextAnnotation`], [`Self::Association`])
/// are also valid top-level children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// The IR mirrors the DMN XML element tree directly; boxing the large variants
// would push that noise onto every producer/consumer for a one-off parse cost.
#[allow(clippy::large_enum_variant)]
pub enum DefinitionsChild {
    /// `<itemDefinition>` — Clause 7 item definition (DMN-05).
    ItemDefinition(ItemDefinition),
    /// `<decision>` — a decision in the DRD.
    Decision(Decision),
    /// `<inputData>` — input data node.
    InputData(InputData),
    /// `<businessKnowledgeModel>` — boxed function/knowledge.
    BusinessKnowledgeModel(BusinessKnowledgeModel),
    /// `<decisionService>` — decision service (§10.4).
    DecisionService(DecisionService),
    /// `<knowledgeSource>` — external knowledge source.
    KnowledgeSource(KnowledgeSource),
    /// `<textAnnotation>` — top-level text annotation.
    TextAnnotation(TextAnnotation),
    /// `<association>` — association between elements.
    Association(Association),
}

/// Preserves whether text was CDATA or plain (for round-trip).
///
/// Used for descriptions, annotations, and rule annotation text. The parser
/// records `is_cdata = true` when the source used `<![CDATA[…]]>`; the exporter
/// re-emits CDATA when the flag is set, plain escaped text otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextContent {
    /// The text payload (already entity-decoded by the parser; the exporter
    /// re-escapes on emission).
    pub content: String,
    /// `true` iff the source represented this text as a CDATA section.
    pub is_cdata: bool,
}

impl TextContent {
    /// Construct plain (non-CDATA) text content.
    pub fn plain(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_cdata: false,
        }
    }

    /// Construct CDATA-flagged text content.
    pub fn cdata(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_cdata: true,
        }
    }
}

// =========================================================================
// Clause 7 — item definitions + information items (DMN-05)
// =========================================================================

/// `<itemDefinition>` — a typed item definition (Clause 7).
///
/// Item definitions may nest via `item_components` (ordered), preserving the
/// document's component order for round-trip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemDefinition {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<TextContent>,
    /// `typeRef` — the referenced type (e.g. `feel:number`, `feel:string`).
    pub type_ref: Option<String>,
    /// `isCollection` attribute.
    pub is_collection: bool,
    /// `typeLanguage` override for this item definition.
    pub type_language: Option<String>,
    /// `allowedValues` — unary-tests or enumeration constraint.
    pub allowed_values: Option<AllowedValues>,
    /// Nested item components, preserving document order.
    pub item_components: Vec<ItemDefinition>,
    /// Annotations on this item definition.
    pub annotations: Vec<Annotation>,
}

/// `<informationItem>` / `variable` — a typed variable bound to a DRG node.
///
/// Carried as text (FEEL type names are strings); Phase 30 resolves
/// `type_ref` against the FEEL type lattice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InformationItem {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<TextContent>,
    /// `typeRef` — the referenced FEEL type.
    pub type_ref: Option<String>,
    pub annotations: Vec<Annotation>,
}

// =========================================================================
// Decisions + DRD nodes
// =========================================================================

/// `<decision>` — a decision in the DRD.
///
/// `required_decisions` / `required_inputs` / `required_knowledge` hold the
/// IDs of upstream DRG nodes (populated by the parser from
/// `informationRequirement` / `knowledgeRequirement`). `decision_table` is
/// `None` when the decision's expression is not a decision table (other boxed
/// expressions land in Phase 31).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<TextContent>,
    /// The decision's variable (`<variable>` child).
    pub variable: Option<InformationItem>,
    /// `question` attribute (human-readable decision question).
    pub question: Option<String>,
    /// `allowedAnswers` attribute.
    pub allowed_answers: Option<String>,
    /// The decision's decision table, if its expression is a `<decisionTable>`.
    pub decision_table: Option<DecisionTable>,
    /// The decision's literal expression, if its expression is a
    /// `<literalExpression>` (DMN 1.5 §10.3.2). A decision has at most one of
    /// `decision_table` / `literal_expression`.
    pub literal_expression: Option<LiteralExpression>,
    /// The decision's boxed `<context>`, when that is its body.
    pub boxed_context: Option<BoxedContext>,
    /// The decision's boxed `<list>`, when that is its body.
    pub boxed_list: Option<BoxedList>,
    /// The decision's boxed `<relation>`, when that is its body.
    pub boxed_relation: Option<BoxedRelation>,
    /// IDs of upstream decisions (DRD edges from `informationRequirement`).
    pub required_decisions: Vec<String>,
    /// IDs of upstream input data.
    pub required_inputs: Vec<String>,
    /// IDs of upstream business knowledge models.
    pub required_knowledge: Vec<String>,
    pub annotations: Vec<Annotation>,
}

/// `<inputData>` — input data node in the DRD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputData {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<TextContent>,
    pub variable: Option<InformationItem>,
    pub annotations: Vec<Annotation>,
}

/// `<businessKnowledgeModel>` — a boxed function / knowledge node.
///
/// The FEEL function body text is stored as `String` when present (Phase 31
/// parses it into a `FeelExpression` AST).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BusinessKnowledgeModel {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<TextContent>,
    pub variable: Option<InformationItem>,
    /// IDs of required knowledge (from `knowledgeRequirement`).
    pub knowledge_requirement: Vec<String>,
    /// The function's formal parameters, in declaration order.
    pub parameters: Vec<InformationItem>,
    /// The function body (`<encapsulatedLogic>`), when the model defines one.
    pub body: Option<LiteralExpression>,
    pub annotations: Vec<Annotation>,
}

/// `<decisionService>` — a decision service (§10.4). Multi-output decision
/// services are the v6.0 MVP evaluation scope (per roadmap scope lock).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionService {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<TextContent>,
    /// IDs of the output decisions the service exposes.
    pub output_decisions: Vec<String>,
    /// IDs of decisions enclosed by the service.
    pub encapsulated_decisions: Vec<String>,
    /// IDs of input data the service accepts.
    pub input_data: Vec<String>,
    /// IDs of decisions the service accepts as parameters, in declaration order.
    pub input_decisions: Vec<String>,
    /// Every input parameter (input decisions and input data) in document order —
    /// the order positional arguments bind in.
    pub input_parameters: Vec<String>,
    /// The service's child element ids (outputs, encapsulated decisions and
    /// parameters) in document order, so the XML re-emits exactly as it was read.
    pub child_order: Vec<String>,
    pub annotations: Vec<Annotation>,
}

/// `<knowledgeSource>` — an external knowledge source in the DRD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeSource {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<TextContent>,
    /// `type` attribute (e.g. reference to an external source kind).
    pub type_ref: Option<String>,
    pub annotations: Vec<Annotation>,
}

/// `<textAnnotation>` — a free-text annotation attached to an element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextAnnotation {
    pub id: Option<String>,
    /// The annotation text (CDATA flag preserved for round-trip).
    pub text: TextContent,
    /// `textFormat` attribute (e.g. `text/plain`).
    pub text_format: Option<String>,
}

/// `<association>` — an association between two DRG elements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Association {
    pub id: Option<String>,
    /// Source element ID.
    pub source_ref: Option<String>,
    /// Target element ID.
    pub target_ref: Option<String>,
    /// `type` attribute (e.g. `Association` / `InformationRequirement`).
    pub association_type: Option<String>,
    /// Optional annotation describing the association.
    pub annotation: Option<Annotation>,
}

// =========================================================================
// Decision table (Clause 8 structure — hit-policy evaluation is Phase 32)
// =========================================================================

/// `<decisionTable>` — the Clause 8 decision-table structure.
///
/// Hit-policy *evaluation* is Phase 32; this struct holds only the IR shape.
/// `hit_policy` defaults to [`HitPolicy::Unique`] when the parser sees no
/// explicit `hitPolicy` attribute (Phase 29 success criterion 3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DecisionTable {
    pub id: Option<String>,
    pub name: Option<String>,
    /// Hit policy. Defaults to `Unique` (UNIQUE) per DMN spec.
    pub hit_policy: HitPolicy,
    /// Aggregator; only set for `Collect` hit policy.
    pub aggregation: Option<BuiltinAggregator>,
    /// `preferredOrientation` attribute (`Rule-as-Row` / `Rule-as-Column` /
    /// `CrossTable`).
    pub preferred_orientation: Option<String>,
    /// `outputLabel` attribute.
    pub output_label: Option<String>,
    /// Input clauses, ordered (one per input column).
    pub input_clauses: Vec<InputClause>,
    /// Output clauses, ordered (one per output column).
    pub output_clauses: Vec<OutputClause>,
    /// Rules, ordered — preserving document order (Pitfall 11).
    pub rules: Vec<DmnRule>,
}

/// `<literalExpression>` — a decision whose value is one FEEL expression.
///
/// A boxed `<context>` expression — a literal, ordered set of named entries.
///
/// Used as a decision's body, where each entry is a named sub-expression. Entries
/// evaluate in document order, so a later entry may refer to an earlier one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxedContext {
    pub id: Option<String>,
    /// Entries in document order: the context key and its value expression.
    pub entries: Vec<BoxedContextEntry>,
    pub description: Option<TextContent>,
}

/// One `<contextEntry>` of a [`BoxedContext`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxedContextEntry {
    /// The `<variable name="…"/>` of the entry — the context key.
    pub key: String,
    /// The entry's value expression.
    pub value: BoxedValue,
}

/// A boxed `<list>` expression — an ordered list of sub-expressions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxedList {
    pub id: Option<String>,
    /// The items in document order.
    pub items: Vec<BoxedValue>,
    pub description: Option<TextContent>,
}

/// A boxed `<relation>` expression — named columns and their rows.
///
/// A relation is a list of contexts: each row binds every column name to the
/// row's value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxedRelation {
    pub id: Option<String>,
    /// Column names, in order.
    pub columns: Vec<String>,
    /// Column ids, in order (the XML gives each column an id).
    pub column_ids: Vec<Option<String>>,
    /// Rows, each with one value expression per column.
    pub rows: Vec<Vec<BoxedValue>>,
    /// Row ids, in order.
    pub row_ids: Vec<Option<String>>,
    pub description: Option<TextContent>,
}

/// The value expression inside a boxed construct.
///
/// Only the nests that appear in real models are modelled: a literal expression,
/// a nested list, a nested context. Anything else is kept as
/// [`BoxedValue::Unsupported`] with its element name, so the document still
/// round-trips instead of silently losing an element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BoxedValue {
    /// `<literalExpression><text>…</text></literalExpression>`.
    Literal(BoxedLiteral),
    /// `<list>…</list>`.
    List(BoxedList),
    /// `<context>…</context>`.
    Context(BoxedContext),
    /// `<relation>…</relation>`.
    Relation(BoxedRelation),
    /// An element this IR does not model; its name is preserved.
    Unsupported(String),
}

/// A literal expression inside a boxed value (text plus its attributes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxedLiteral {
    pub id: Option<String>,
    pub type_ref: Option<String>,
    pub text: TextContent,
}

/// The IR keeps the expression text verbatim (with its CDATA flag) rather than
/// re-shaping it: the evaluator parses it on demand through the AST cache, and
/// the exporter writes it back unchanged, so a literal decision round-trips.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiteralExpression {
    pub id: Option<String>,
    /// `typeRef` attribute, when the model states one.
    pub type_ref: Option<String>,
    /// The FEEL expression text.
    pub text: TextContent,
    pub description: Option<TextContent>,
}

/// `<input>` — an input clause (column) of a decision table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputClause {
    pub id: Option<String>,
    pub label: Option<String>,
    /// `<inputExpression>` — the FEEL input expression text.
    pub input_expression: String,
    /// `<inputValues>` — allowed values constraint on the input.
    pub input_values: Option<AllowedValues>,
    pub description: Option<TextContent>,
}

/// `<output>` — an output clause (column) of a decision table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputClause {
    pub id: Option<String>,
    pub label: Option<String>,
    pub name: Option<String>,
    /// `typeRef` for the output.
    pub type_ref: Option<String>,
    /// `<outputValues>` — allowed values constraint on the output.
    pub output_values: Option<AllowedValues>,
    /// `<defaultOutputEntry>` — FEEL text for the default output.
    pub default_output_entry: Option<String>,
    pub description: Option<TextContent>,
}

/// `<rule>` — a single rule (row) of a decision table.
///
/// `input_entries` / `output_entries` / `annotation_entries` are ordered, one
/// per input clause / output clause / annotation column respectively.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DmnRule {
    pub id: Option<String>,
    pub description: Option<TextContent>,
    /// One per input clause, in clause order.
    pub input_entries: Vec<RuleEntry>,
    /// One per output clause, in clause order.
    pub output_entries: Vec<RuleEntry>,
    /// Annotation-column entries (rule annotations), in column order.
    pub annotation_entries: Vec<RuleEntry>,
}

/// A single cell entry (input / output / annotation) of a [`DmnRule`].
///
/// Carries the FEEL cell text (or annotation text) as [`TextContent`] so the
/// CDATA flag round-trips.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleEntry {
    /// The FEEL cell text or annotation text (CDATA flag preserved).
    pub text: TextContent,
}

// =========================================================================
// Hit policy + aggregator enums (canonical — Phase 32 evaluates;
// frontend maps TO these per Phase 34)
// =========================================================================

/// DMN hit policy (Clause 8.2.12). Canonical enum — Phase 32 evaluates; the
/// Phase 34 frontend maps its hit-policy vocabulary TO these variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HitPolicy {
    /// `UNIQUE` — at most one rule matches; multiple matches are a violation.
    #[serde(rename = "UNIQUE")]
    Unique,
    /// `ANY` — multiple matching rules must yield the same output.
    #[serde(rename = "ANY")]
    Any,
    /// `PRIORITY` — select the match whose output is highest in the output
    /// column's `allowedValues` enumeration order (NOT first match by row).
    #[serde(rename = "PRIORITY")]
    Priority,
    /// `FIRST` — first matching rule in document order wins.
    #[serde(rename = "FIRST")]
    First,
    /// `RULE ORDER` — all matching rules' outputs, in document order.
    #[serde(rename = "RULE ORDER")]
    RuleOrder,
    /// `OUTPUT ORDER` — all matching rules' outputs, ordered by the output
    /// column's `allowedValues` enumeration.
    #[serde(rename = "OUTPUT ORDER")]
    OutputOrder,
    /// `COLLECT` — all matching rules' outputs, optionally aggregated.
    #[serde(rename = "COLLECT")]
    Collect,
}

#[cfg(test)]
mod enum_wire_tests {
    //! The JSON the IR endpoint emits and the XML the exporter writes must use one vocabulary — DMN's.
    //!
    //! They did not: serde emitted Rust variant names (`"RuleOrder"`) while `as_str()`, which the
    //! exporter writes into `<decisionTable hitPolicy="…">`, is `"RULE ORDER"`. A client comparing the
    //! IR's `hit_policy` against the DMN string — as `frontend/src/api/decisions.ts` does — never
    //! matched, and would have shown a blank or wrong hit policy had anything read it.

    use super::{BuiltinAggregator, HitPolicy};

    #[test]
    fn hit_policies_serialise_as_their_dmn_names() {
        for policy in [
            HitPolicy::Unique,
            HitPolicy::Any,
            HitPolicy::Priority,
            HitPolicy::First,
            HitPolicy::RuleOrder,
            HitPolicy::OutputOrder,
            HitPolicy::Collect,
        ] {
            assert_eq!(
                serde_json::to_value(policy).unwrap(),
                serde_json::Value::String(policy.as_str().to_string()),
                "{policy:?} should serialise as `{}`",
                policy.as_str()
            );
        }
    }

    #[test]
    fn aggregators_serialise_as_their_dmn_names() {
        for aggregator in [
            BuiltinAggregator::Sum,
            BuiltinAggregator::Min,
            BuiltinAggregator::Max,
            BuiltinAggregator::Count,
        ] {
            assert_eq!(
                serde_json::to_value(aggregator).unwrap(),
                serde_json::Value::String(aggregator.as_str().to_string()),
                "{aggregator:?} should serialise as `{}`",
                aggregator.as_str()
            );
        }
    }

    #[test]
    fn the_dmn_names_round_trip_back() {
        // A value read from JSON must be the same policy the exporter would write and the parser would
        // read from XML.
        for policy in [
            HitPolicy::RuleOrder,
            HitPolicy::Collect,
            HitPolicy::Priority,
        ] {
            let json = serde_json::to_string(&policy).unwrap();
            let back: HitPolicy = serde_json::from_str(&json).unwrap();
            assert_eq!(back, policy, "{json} should parse back to {policy:?}");
        }
    }
}

impl Default for HitPolicy {
    /// DMN default hit policy is `UNIQUE` (spec §8.2.12).
    fn default() -> Self {
        Self::Unique
    }
}

impl HitPolicy {
    /// The DMN attribute string used in `<decisionTable hitPolicy="…">`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unique => "UNIQUE",
            Self::Any => "ANY",
            Self::Priority => "PRIORITY",
            Self::First => "FIRST",
            Self::RuleOrder => "RULE ORDER",
            Self::OutputOrder => "OUTPUT ORDER",
            Self::Collect => "COLLECT",
        }
    }
}

/// Builtin COLLECT aggregator (Clause 8.2.12). Only meaningful when the hit
/// policy is [`HitPolicy::Collect`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuiltinAggregator {
    /// `SUM` — sum of outputs.
    #[serde(rename = "SUM")]
    Sum,
    /// `MIN` — minimum output.
    #[serde(rename = "MIN")]
    Min,
    /// `MAX` — maximum output.
    #[serde(rename = "MAX")]
    Max,
    /// `COUNT` — number of matching rules.
    #[serde(rename = "COUNT")]
    Count,
    /// `DISTINCT` — the matching rules' outputs with duplicates removed.
    ///
    /// A DMN 1.6 aggregator. The semantics here are **derived**, not copied: the vendored corpus is DMN 1.5,
    /// so nothing in this repository could check a reading of the 1.6 text, and the honest thing is to say so.
    /// They follow from two things this codebase already does — COLLECT returns the matching rules' outputs in
    /// rule order, and FEEL's own `distinct values()` removes duplicates by **FEEL equality** (`1` and `1.0`
    /// are equal; a string is not a number) — so `DISTINCT` is that deduplication applied to the collected
    /// outputs, keeping the first occurrence of each value.
    #[serde(rename = "DISTINCT")]
    Distinct,
}

impl BuiltinAggregator {
    /// The DMN aggregation attribute string used in
    /// `<decisionTable aggregation="…">`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sum => "SUM",
            Self::Min => "MIN",
            Self::Max => "MAX",
            Self::Count => "COUNT",
            Self::Distinct => "DISTINCT",
        }
    }
}

// =========================================================================
// Allowed values + annotations
// =========================================================================

/// `allowedValues` — a constraint on a typed value.
///
/// Stored as text (FEEL unary-tests or enumeration literals); Phase 31 parses
/// `UnaryTests` into a FEEL AST. Enumeration literals preserve CDATA flags via
/// [`TextContent`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AllowedValues {
    /// `<unaryTests>` — the FEEL unary-tests text (e.g. `> 5, < 100`).
    UnaryTests(String),
    /// Enumeration of literal values, preserving CDATA flags.
    Enumeration(Vec<TextContent>),
}

impl AllowedValues {
    /// Debug rendering for log messages / error output.
    pub fn as_str_or_items(&self) -> String {
        match self {
            Self::UnaryTests(t) => t.clone(),
            Self::Enumeration(items) => {
                let parts: Vec<String> = items
                    .iter()
                    .map(|t| {
                        if t.is_cdata {
                            format!("<![CDATA[{}]]>", t.content)
                        } else {
                            t.content.clone()
                        }
                    })
                    .collect();
                format!("[{}]", parts.join(", "))
            }
        }
    }
}

/// An annotation (e.g. `<textAnnotation>`) attached to a DRG element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub id: Option<String>,
    /// The annotation text (CDATA flag preserved for round-trip).
    pub text: TextContent,
    /// `annotationType` / the kind of annotation (e.g. `"TextAnnotation"`).
    pub annotation_type: Option<String>,
    /// `textFormat` attribute (e.g. `text/plain`).
    pub text_format: Option<String>,
}

// =========================================================================
// DMNDI passthrough (DMN-07 prep — Phase 35 renders; parser populates in
// Plan 29-02, exporter emits in Plan 29-03 / Phase 35). These are opaque
// structural carriers — no interpretation here; Phase 35 may enrich.
// =========================================================================

/// A DMNDI diagram (layout carrier). Geometry is coordinate data, NOT FEEL
/// numerics — `f64` is correct here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DmnDiagram {
    pub id: Option<String>,
    pub name: Option<String>,
    pub shapes: Vec<DmnShape>,
    pub edges: Vec<DmnEdge>,
}

/// A DMNDI shape (layout carrier for a DRG node).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DmnShape {
    /// ID of the DRG element this shape renders.
    pub element_id: String,
    /// Bounds of the shape on the diagram canvas.
    pub bounds: Option<DcBounds>,
    /// Labels attached to the shape.
    pub labels: Vec<DmnLabel>,
}

/// A DMNDI edge (layout carrier for a DRG edge/association).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DmnEdge {
    pub id: Option<String>,
    /// Source element ID.
    pub source_id: String,
    /// Target element ID.
    pub target_id: String,
    /// Waypoints defining the edge geometry.
    pub waypoints: Vec<DcPoint>,
}

/// `dc:Bounds` — a rectangle on the diagram canvas. Coordinate data (`f64`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DcBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// `dc:Point` — a point on the diagram canvas. Coordinate data (`f64`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DcPoint {
    pub x: f64,
    pub y: f64,
}

/// A DMNDI label (layout carrier for element labels).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DmnLabel {
    pub bounds: Option<DcBounds>,
    pub text: Option<String>,
}

// =========================================================================
// Errors — single home for the DMN error type (parser.rs returns these in
// Plan 29-02; declared here so model.rs owns the canonical error surface).
// =========================================================================

/// Errors produced by the DMN XML parser (Plan 29-02) and any IR-level
/// validation. Declared here so `model.rs` is the single error-type home.
#[derive(Debug, Clone, Error)]
pub enum DmnParseError {
    /// Low-level XML parse error from `quick-xml`.
    #[error("XML parse error: {0}")]
    XmlError(String),
    /// A required DMN element was missing.
    #[error("missing required element: {0}")]
    MissingElement(String),
    /// An unexpected element was encountered (e.g. a child not valid for its
    /// parent per the DMN 1.5 content model).
    #[error("unexpected element: {0}")]
    UnexpectedElement(String),
    /// An XXE external-entity reference was rejected by the hardened parser.
    #[error("XXE rejected: {0}")]
    XxeRejected(String),
    /// An I/O error reading the DMN source.
    #[error("I/O error: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: defaults align with DMN spec (hitPolicy = UNIQUE).
    /// Guards the Phase 29 success criterion "hitPolicy defaults to UNIQUE"
    /// at the IR level (the parser enforces it at parse time in 29-02).
    #[test]
    fn decision_table_default_hit_policy_is_unique() {
        let dt = DecisionTable::default();
        assert_eq!(dt.hit_policy, HitPolicy::Unique);
        assert_eq!(dt.hit_policy.as_str(), "UNIQUE");
        assert!(dt.aggregation.is_none());
        assert!(dt.input_clauses.is_empty());
        assert!(dt.output_clauses.is_empty());
        assert!(dt.rules.is_empty());
    }

    /// Smoke test: HitPolicy::as_str round-trips all 7 variants (the
    /// parser/exporter rely on these exact strings).
    #[test]
    fn hit_policy_as_str_all_variants() {
        let cases = [
            (HitPolicy::Unique, "UNIQUE"),
            (HitPolicy::Any, "ANY"),
            (HitPolicy::Priority, "PRIORITY"),
            (HitPolicy::First, "FIRST"),
            (HitPolicy::RuleOrder, "RULE ORDER"),
            (HitPolicy::OutputOrder, "OUTPUT ORDER"),
            (HitPolicy::Collect, "COLLECT"),
        ];
        for (hp, expected) in cases {
            assert_eq!(hp.as_str(), expected);
        }
    }

    /// Smoke test: BuiltinAggregator::as_str round-trips all 4 variants.
    #[test]
    fn builtin_aggregator_as_str_all_variants() {
        assert_eq!(BuiltinAggregator::Sum.as_str(), "SUM");
        assert_eq!(BuiltinAggregator::Min.as_str(), "MIN");
        assert_eq!(BuiltinAggregator::Max.as_str(), "MAX");
        assert_eq!(BuiltinAggregator::Count.as_str(), "COUNT");
    }

    /// Smoke test: TextContent constructors set the CDATA flag correctly.
    #[test]
    fn text_content_constructors() {
        let plain = TextContent::plain("hello");
        assert!(!plain.is_cdata);
        let cdata = TextContent::cdata("<b>html</b>");
        assert!(cdata.is_cdata);
    }

    /// Smoke test: AllowedValues debug rendering.
    #[test]
    fn allowed_values_debug_render() {
        let u = AllowedValues::UnaryTests("> 5".to_string());
        assert_eq!(u.as_str_or_items(), "> 5");
        let e = AllowedValues::Enumeration(vec![TextContent::plain("A"), TextContent::cdata("B")]);
        let rendered = e.as_str_or_items();
        assert!(rendered.contains("A"));
        assert!(rendered.contains("<![CDATA[B]]>"));
    }

    /// Smoke test: DmnModel + Definitions default-construct cleanly.
    #[test]
    fn model_defaults() {
        let m = DmnModel::default();
        assert!(m.source_namespace.is_none());
        assert!(m.definitions.children.is_empty());
        assert!(m.definitions.diagrams.is_empty());
        assert!(m.definitions.namespace.is_empty());
    }
}
