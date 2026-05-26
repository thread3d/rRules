---
phase: 06-dmn-compliance
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/dmn/parser/xml_reader.rs
  - src/dmn/parser/validator.rs
  - src/dmn/parser/model.rs
  - src/dmn/lib.rs
autonomous: true
requirements: [DMN-01]
user_setup: []

must_haves:
  truths:
    - "DMN XML files can be parsed according to OMG DMN 1.5 specification"
    - "All required elements (Decision, DecisionTable, InformationRequirement) are recognized"
    - "All required attributes (id, name, hitPolicy, outputLabel) are validated"
    - "Cross-references between components are resolved correctly"
    - "Validation errors provide clear error messages with location info"
  artifacts:
    - path: "src/dmn/parser/xml_reader.rs"
      provides: "DMN XML parsing with DOM-like traversal"
      exports: ["parse_dmn_document", "parse_decision", "parse_decision_table"]
    - path: "src/dmn/parser/validator.rs"
      provides: "Spec compliance validation"
      exports: ["validate_element", "validate_hit_policy", "resolve_cross_references"]
    - path: "src/dmn/parser/model.rs"
      provides: "Rust data structures for all DMN 1.5 elements"
      contains: [
        "pub struct DmnDocument",
        "pub struct Decision",
        "pub struct DecisionTable",
        "pub struct InputClause",
        "pub struct OutputClause",
        "pub struct Rule",
        "pub enum HitPolicy"
      ]
    - path: "src/dmn/lib.rs"
      provides: "Module entry point and public API re-exports"
      exports: ["parser", "DMN_VERSION"]
  key_links:
    - from: "src/dmn/parser/xml_reader.rs"
      to: "src/dmn/parser/validator.rs"
      via: "Validation calls during parsing"
      pattern: "validate_element|resolve_cross_references"
---

## Phase Goal

**As a** developer, **I want to** load and validate DMN 1.5 files, **so that** I can use standardized business decision models in rRules.

Purpose: Implement the complete DMN Level 3 XML parser following the OMG specification before any execution logic. This foundational layer must parse all DMN constructs (Decision, DecisionTable, InformationRequirement, InputClause, OutputClause, Rule) and validate them against spec requirements.
Output: Full DMN 1.5 XML parser with validation, error reporting, and cross-reference resolution.

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md

**Canonical References (D-06):**
- Full spec compliance first - Implement complete DMN Level 3 spec before any execution
- XML-based DMN 1.5 parser following OMG specification
- Validation of all required elements and attributes
- Cross-reference resolution between Decision Tables, Contexts, Literals
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md (Phase 6 goal, requirements)
@.planning/phases/06-dmn-compliance/06-CONTEXT.md (implementation decisions D-06)

**Phase 1 Context (locked - hybrid approach):**
- Both API and file-based DSL for rule definition

**DMN Directory Structure (from CONTEXT.md):**
```
src/dmn/
├── Cargo.toml
├── lib.rs
├── parser/              # DMN XML parser
│   ├── mod.rs
│   ├── xml_reader.rs   # Parse DMN 1.5 XML
│   └── validator.rs    # Validate against spec
├── feel/                # FEEL expression language
│   ├── mod.rs
│   ├── parser.rs       # FEEL grammar parser
│   ├── evaluator.rs    # Expression evaluation
│   └── builtins.rs     # All FEEL built-in functions
├── execution/           # DMN execution engine
│   ├── mod.rs
│   ├── tables.rs       # Decision Table execution with hit policies
│   ├── contexts.rs     # Context evaluation
│   └── literals.rs     # Literal expression handling
└── api/                 # REST endpoints
    ├── mod.rs
    └── dmns.rs         # DMN CRUD and execution
```
</context>

<tasks>

<task type="auto">
  <name>Task 1: Create DMN module structure and data models</name>
  <files>src/dmn/lib.rs, src/dmn/parser/model.rs</files>
  <action>Create the complete Rust data model for DMN 1.5 spec compliance:

1. **src/dmn/lib.rs** - Module declarations and re-exports:
   - Declare `pub mod parser;`
   - Define `DMN_VERSION = "1.5"` constant
   - Re-export parser module types

2. **src/dmn/parser/model.rs** - Complete data structures for all DMN 1.5 elements:
   - `DmnDocument`: Root element with namespace, id, name, definitions
   - `Definitions`: Contains decisions, items, requirements, etc.
   - `Decision`: id, name, typeRef, requirementReference
   - `DecisionTable`: hitPolicy (enum), aggregate, ruleOrder, input, output
   - `HitPolicy` enum: First, Any, Collect (with aggregation functions), Override, Priority, RuleOrder, Sum, Min, Max, Count
   - `InputClause`: id, label, inputExpression (typed expression)
   - `OutputClause`: id, label, typeRef, allowedValues
   - `Rule`: list of input entries and output entry
   - `InputEntry`/`OutputEntry`: text representation of rule conditions/actions
   - `InformationRequirement`/`Artifact`: cross-reference elements

All structs must use serde for XML serialization support.
</action>
  <verify>
    <automated>cd /home/threaded/projects/rRules && cargo check --package rrules 2>&1 | grep -E "(error|warning: unused)" | head -20</automated>
  </verify>
  <done>Data model compiles without errors, all DMN elements have corresponding Rust structs</done>
</task>

<task type="auto">
  <name>Task 2: Implement XML parsing with proper namespace handling</name>
  <files>src/dmn/parser/xml_reader.rs</files>
  <action>Implement `src/dmn/parser/xml_reader.rs` with complete DMN 1.5 XML parsing:

**Core Functions:**
- `parse_dmn_document(xml: &str) -> Result<DmnDocument, ParseError>` - Main entry point
- `parse_definitions(node: &Node) -> Result<Definitions, ParseError>`
- `parse_decision(node: &Node) -> Result<Decision, ParseError>`
- `parse_decision_table(node: &Node) -> Result<DecisionTable, ParseError>`
- `parse_input_clause(node: &Node) -> Result<InputClause, ParseError>`
- `parse_output_clause(node: &Node) -> Result<OutputClause, ParseError>`
- `parse_rule(node: &Node) -> Result<Rule, ParseError>`

**Requirements:**
1. Use quick_xml or similar XML parser with namespace support
2. Handle DMN namespaces: urn:omg.org:dmn:1.5, http://www.omg.org/spec/DMN/20191111/DMN
3. Parse hitPolicy attribute into HitPolicy enum (First, Any, Collect, Override, Priority, RuleOrder, Sum, Min, Max, Count)
4. Extract typeRef attributes for typed expressions
5. Handle optional attributes with Option<T> types
6. Preserve XML source position info in error reporting

**Error Handling:**
- ParseError struct with: message, location (line/column), context (element name)
- Invalid hitPolicy returns specific error variant
- Missing required attribute returns detailed error with attribute name
</action>
  <verify>
    <automated>cd /home/threaded/projects/rRules && cargo test --package rrules parse_dmn 2>&1 | grep -E "(test result|running)"</automated>
  </verify>
  <done>Parsing functions correctly handle all DMN 1.5 element types and produce DmnDocument structs</done>
</task>

<task type="auto">
  <name>Task 3: Implement validation engine with error reporting</name>
  <files>src/dmn/parser/validator.rs</files>
  <action>Implement `src/dmn/parser/validator.rs` with spec compliance validation:

**Core Functions:**
- `validate_element(element: &DmnElement) -> Result<(), ValidationError>` - Type-specific validation
- `validate_hit_policy(table: &DecisionTable) -> Result<(), ValidationError>`
- `resolve_cross_references(document: &mut DmnDocument) -> Result<(), ValidationError>`
- `validate_decision_table(table: &DecisionTable, definitions: &Definitions) -> Result<(), ValidationError>`

**Validation Rules (per DMN 1.5 spec):**
1. **Element Required Attributes**: Check id, name, hitPolicy presence
2. **Cross-reference Validation**: 
   - InformationRequirement references must point to existing Definitions elements
   - InputClause inputExpression typeRef must resolve to ItemDefinition or built-in type
3. **Hit Policy Validity**:
   - First/Any/Override: No aggregation allowed
   - Collect: Requires aggregation function (Sum/Min/Max/Count)
   - Priority: Requires ruleOrder attribute
4. **Input/Output Consistency**:
   - Number of input entries must match number of inputs
   - Output entry count must match output clauses
   - Rule output values must conform to output typeRef

**Error Reporting:**
- ValidationError with detailed context (element id, line number)
- Group related errors (collect all issues before failing)
- Provide helpful suggestions for common mistakes
</action>
  <verify>
    <automated>cd /home/threaded/projects/rRules && cargo test --package rrules validate_ 2>&1 | grep -E "(test result|running)"</automated>
  </verify>
  <done>All validation rules from DMN spec implemented, errors provide clear location and description</done>
</task>

<task type="auto">
  <name>Task 4: Create example DMN files for testing</name>
  <files>tests/fixtures/dmn/simple_decision.dmn, tests/fixtures/dmn/complex_table.dmn</files>
  <action>Create test fixtures that validate against OMG DMN 1.5 specification:

**tests/fixtures/dmn/simple_decision.dmn:**
- Simple decision with single hit policy (First)
- One input and one output clause
- Basic rule with condition and action

**tests/fixtures/dmn/complex_table.dmn:**
- Decision table with Collect hit policy and Sum aggregation
- Multiple inputs and outputs
- Several rules to test ordering and aggregation

Both files must:
1. Be valid XML with proper DMN namespace declarations
2. Include id attributes on all required elements
3. Use correct typeRef values for expressions
4. Follow OMG example patterns

Also create a test file `src/dmn/parser/tests.rs` that includes these as tests.
</action>
  <verify>
    <automated>cd /home/threaded/projects/rRules && cargo test --package rrules xml_reader::tests 2>&1 | grep -E "(test result|running)"</automated>
  </verify>
  <done>Example DMN files parse correctly and validation passes</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| XML input → Parser | Untrusted user-provided XML must not cause panics or memory corruption |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-06-01 | Tampering | XML parsing with quick_xml | mitigate | Validate all element/attribute names against spec; reject unknown elements gracefully |
| T-06-02 | Denial of Service | Large XML files | accept | No DoS vector in parsing logic; file size limits handled by caller (REST API) |
| T-06-03 | Spoofing | Element IDs | mitigate | Validate all ID references during cross-reference resolution; reject duplicates |
| T-06-SC | Tampering | Cargo installs | mitigate | Blocking human checkpoint for [ASSUMED] packages before `cargo add` |

## Mitigation Details

**XML Injection Prevention:**
- Use quick_xml's default secure parser settings (no external entities)
- Validate all element names against expected DMN 1.5 namespace elements
- Reject any XML with DOCTYPE declarations that could enable XXE attacks
</threat_model>

<verification>
1. Parse test fixtures: `cargo test --package rrules parse_dmn_document`
2. Validation tests: `cargo test --package rrules validate_`
3. Cross-reference resolution: `cargo test --package rrules resolve_`

## Success Criteria

What must be TRUE:
1. DMN 1.5 XML files with proper namespace can be parsed into Rust structs
2. All required elements (Decision, DecisionTable, InputClause, OutputClause, Rule) are recognized
3. Hit policy values are correctly parsed and validated
4. Cross-references between components resolve without errors
5. Validation errors include element name, attribute, and location information

## Output

Create `.planning/phases/06-dmn-compliance/06-01-SUMMARY.md` when done