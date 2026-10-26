# DMN 1.5 and FEEL

This document specifies the DMN/FEEL surface of the `rRules` engine as implemented, not as
intended. Every non-obvious statement cites `path:line`. Nothing here is routed through the
rule-engine path.

## 1. Purpose and scope

The DMN surface is a **parallel evaluation surface**: a stateless, side-effect-free,
three-valued evaluator that shares no execution engine and no expression AST with the
forward-chaining Rete family. This is a locked architectural decision, stated in
`engine/src/dmn/mod.rs:1-12` and repeated in `engine/src/dmn/feel/ast.rs:3-9` and
`engine/src/dmn/decision_table.rs:8-11`.

Four prohibitions constrain the whole surface (`engine/src/dmn/feel/mod.rs:38-57`):

| # | Prohibition | Where it is structurally enforced |
|---|---|---|
| 1 | Never use Rust `==`/`Ord`/`compare_values` for FEEL equality/ordering | Only `feel::eval::cmp` may compare (`engine/src/dmn/feel/eval.rs:131-148`); the module deliberately does not re-export `PartialEq`/`PartialOrd` (`eval.rs:9-14`) |
| 2 | Never use `f64` in the FEEL numeric path | Numeric literals lex to `Decimal` (`engine/src/dmn/feel/parser.rs:408-431`); FEEL numerics are `Value::Number(Decimal)` |
| 3 | No implicit number↔string conversion | `coerce_to_number`/`coerce_to_string` reject the other kind (`engine/src/dmn/feel/coercion.rs:39-59`); therefore `1 + "2"` → `Null` (`engine/src/dmn/feel/eval.rs:982-1009`) |
| 4 | The FEEL path never reaches `EngineRegistry` / `engine/src/engine/` | `FeelExpression` is a distinct AST (`engine/src/dmn/feel/ast.rs:141-304`) |

`f64` *is* used for DMNDI geometry (`engine/src/dmn/model.rs:550-564`); that is coordinate
data, explicitly not FEEL numerics (`model.rs:19-22`).

In scope: XML parsing/export of DMN documents, the canonical IR, the FEEL lexer/parser,
the FEEL evaluator and its built-in registry, decision-table evaluation for all seven hit
policies, DRD resolution with level-parallel execution, and the conformance harness.
Out of scope: the Rete/expression engine, the REST layer, and the frontend editors.

## 2. Canonical IR

`engine/src/dmn/model.rs` is the sole DMN data shape. FEEL text is carried as `String` /
`TextContent`; the IR does **not** parse FEEL (`model.rs:10-18`).

| Type | Shape summary | Cite |
|---|---|---|
| `DmnModel` | `{ definitions: Definitions, source_namespace: Option<String> }` | `model.rs:43-50` |
| `Definitions` | `id`, `name`, `description: Option<TextContent>`, `namespace: String`, `type_language`, `expression_language`, `exporter`, `exporter_version`, `children: Vec<DefinitionsChild>`, `diagrams: Vec<DmnDiagram>` | `model.rs:62-85` |
| `DefinitionsChild` | 8 variants: `ItemDefinition`, `Decision`, `InputData`, `BusinessKnowledgeModel`, `DecisionService`, `KnowledgeSource`, `TextAnnotation`, `Association` | `model.rs:98-115` |
| `TextContent` | `{ content: String, is_cdata: bool }` — the only carrier of CDATA-ness | `model.rs:122-147` |
| `ItemDefinition` | `id`, `name`, `description`, `type_ref`, `is_collection`, `type_language`, `allowed_values`, `item_components: Vec<ItemDefinition>`, `annotations` | `model.rs:158-174` |
| `InformationItem` | `id`, `name`, `description`, `type_ref`, `annotations` | `model.rs:181-188` |
| `Decision` | `id: String` (required), `name`, `description`, `variable: Option<InformationItem>`, `question`, `allowed_answers`, `decision_table: Option<DecisionTable>`, `required_decisions`/`required_inputs`/`required_knowledge: Vec<String>`, `annotations` | `model.rs:202-221` |
| `BusinessKnowledgeModel` | `id`, `name`, `description`, `variable`, `knowledge_requirement: Vec<String>`, `annotations` — **no body field** | `model.rs:238-246` |
| `DecisionService` | `id`, `output_decisions`, `encapsulated_decisions`, `input_data` (all `Vec<String>`), `annotations` | `model.rs:251-262` |
| `DecisionTable` | `id`, `name`, `hit_policy: HitPolicy` (default `Unique`), `aggregation: Option<BuiltinAggregator>`, `preferred_orientation`, `output_label`, `input_clauses`, `output_clauses`, `rules` | `model.rs:310-328` |
| `InputClause` | `id`, `label`, `input_expression: String`, `input_values: Option<AllowedValues>`, `description` | `model.rs:333-341` |
| `OutputClause` | `id`, `label`, `name`, `type_ref`, `output_values`, `default_output_entry: Option<String>`, `description` | `model.rs:345-356` |
| `DmnRule` | `id`, `description`, `input_entries`/`output_entries`/`annotation_entries: Vec<RuleEntry>` | `model.rs:363-372` |
| `RuleEntry` | `{ text: TextContent }` — a single cell | `model.rs:379-382` |
| `AllowedValues` | `UnaryTests(String)` \| `Enumeration(Vec<TextContent>)` | `model.rs:470-475` |
| `DmnDiagram` / `DmnShape` / `DmnEdge` / `DcBounds` / `DcPoint` / `DmnLabel` | opaque DMNDI layout carriers | `model.rs:520-571` |

Invariants:

- `Decision.id` and `InputData.id`/`BusinessKnowledgeModel.id`/`DecisionService.id`/`KnowledgeSource.id`
  are non-optional `String`; `Definitions.id`, `ItemDefinition.id`, `InputClause.id`,
  `RuleEntry` (no id), and `DmnShape` (no id) are optional (`model.rs:64`, `160`, `204`, `226`, `239`, `252`, `267`, `334`).
- `HitPolicy` has exactly 7 variants whose `as_str` values are the DMN attribute strings,
  including the space-bearing `"RULE ORDER"` / `"OUTPUT ORDER"` (`model.rs:392-431`).
- `BuiltinAggregator` has exactly 4 variants — `Sum`, `Min`, `Max`, `Count`. There is **no**
  `distinct` variant (`model.rs:436-458`).
- `DmnParseError` lives in `model.rs` and is the single DMN error surface (`model.rs:580-601`);
  variants: `XmlError`, `InvalidNamespace`, `MissingElement`, `UnexpectedElement`,
  `XxeRejected`, `Io`. `InvalidNamespace` is never constructed (see §10).

Preserved by the parser: element ids, names, descriptions (`TextContent`), the CDATA flag,
annotation order, and exact cross-type document order via `Vec<DefinitionsChild>`
(`model.rs:79-84`, `model.rs:6-9`).

Normalised or defaulted by the parser:

| Input | Normalised to | Cite |
|---|---|---|
| absent `hitPolicy`, `hitPolicy=""`, or an unrecognised value | `HitPolicy::Unique` | `parser.rs:2212-2223`, `model.rs:411-416` |
| `aggregation` when `hitPolicy` is not `COLLECT` | dropped (`None`) | `parser.rs:832-849` |
| unrecognised `aggregation` | `None` | `parser.rs:2226-2234` |
| `isCollection` | `true` only for the exact strings `"true"` / `"1"`; `"TRUE"` and `"yes"` are `false` | `parser.rs:428-431` |
| `<informationItem>` inside a decision/inputData/BKM | stored as `InformationItem`, re-emitted as `<variable>` | `parser.rs:598-601`, `1192-1195`; `exporter.rs:285` |
| `href="#_id"` on `requiredDecision`/`requiredInput`/`requiredKnowledge`/`outputDecision`/`encapsulatedDecision`/service `inputData` | leading `#` stripped; re-added on export | `parser.rs:2237-2239`, `728`, `1317`; `exporter.rs:290` |
| DMNDI `dmnElementRef` / `source` / `target` | `#` **not** stripped (inconsistent with the href rule above) | `parser.rs:1662-1665`, `1718-1719` |
| text | byte-exact: `trim_text(false)`, no trimming anywhere in the parse path | `parser.rs:216-217` |

Unknown attributes are dropped (only named attributes are read, `parser.rs:2178-2194`), and
`xmlns:*` declarations other than the root namespace are discarded (`parser.rs:2183-2185`).

## 3. Parsing and export

### 3.1 Entry points

| Function | Signature | Cite |
|---|---|---|
| `parse_dmn` | `(&str) -> Result<DmnModel, DmnParseError>` | `engine/src/dmn/parser.rs:90-92` |
| `parse_dmn_bytes` | `(&[u8]) -> Result<DmnModel, DmnParseError>` | `parser.rs:95-97` |
| `parse_dmn_file` | `(&str) -> Result<DmnModel, DmnParseError>` | `parser.rs:100-104` |
| `parse_dmn_with_cache` | `(&str) -> Result<(DmnModel, FeelAstCache), DmnParseError>` — pre-parses every FEEL cell | `parser.rs:116-121` |
| `export_dmn` | `(&DmnModel) -> Result<String, DmnExportError>` | `engine/src/dmn/exporter.rs:82-89` |
| `export_dmn_to_bytes` | `(&DmnModel) -> Result<Vec<u8>, DmnExportError>` | `exporter.rs:93-97` |

### 3.2 Accepted XML shapes and namespaces

Parsing is namespace-*aware* but namespace-*lenient*: the root namespace is resolved and
recorded, then only compared against a constant whose result is discarded — unknown
namespaces parse by local name (`parser.rs:260-268`). Consequently `DmnParseError::InvalidNamespace`
is unreachable from this path.

| Constant | URI | Cite |
|---|---|---|
| "DMN 1.5" | `https://www.omg.org/spec/DMN/20200524/MODEL/` | `parser.rs:82` |
| "DMN 1.3" | `https://www.omg.org/spec/DMN/20180521/MODEL/` | `parser.rs:84` |
| "DMN 1.1" | `http://www.omg.org/spec/DMN/20151101/dmn.xsd` | `parser.rs:86` |

(The labels are wrong: `20200524` is DMN 1.3 and `20180521` is DMN 1.2; the DMN 1.5 URI
`https://www.omg.org/spec/DMN/20230324/MODEL/` is absent from the list — harmless only
because of the lenient check above.)

Element matching is by **local name** (`local_name_str`), so prefixes do not matter
(`parser.rs:248`, `330`, `331`). Unprefixed documents therefore parse. Elements are mapped
as follows:

| XML element | IR target | Cite |
|---|---|---|
| `definitions` | `Definitions` (+ `source_namespace`) | `parser.rs:249-269`, `302-325` |
| `itemDefinition` | `ItemDefinition`, nested `itemComponents` | `parser.rs:333-335` |
| `decision` | `Decision` | `parser.rs:338-340`, `565-570` |
| `inputData` | `InputData` | `parser.rs:343-345`, `1327` |
| `businessKnowledgeModel` | `BusinessKnowledgeModel` | `parser.rs:348-350`, `1251` |
| `decisionService` | `DecisionService` | `parser.rs:353-355`, `1293-1300` |
| `knowledgeSource` | `KnowledgeSource` | `parser.rs:358-360`, `1196` |
| `textAnnotation` | `TextAnnotation` (top level) / `Annotation` (nested) | `parser.rs:363`, `459` |
| `association` | `Association` | `parser.rs:1443-1483` |
| `variable` / `informationItem` | `InformationItem` | `parser.rs:598`, `1192`, `1251` |
| `informationRequirement` → `requiredDecision` / `requiredInput` | `required_decisions` / `required_inputs` | `parser.rs:602`, `725-732` |
| `knowledgeRequirement` → `requiredKnowledge` | `required_knowledge` / `knowledge_requirement` | `parser.rs:605-607`, `781`, `1843` |
| `decisionTable` | `DecisionTable` | `parser.rs:608-611` |
| `input` / `inputExpression` / `inputValues` | `InputClause` | `parser.rs:856-858`, `925`, `928` |
| `output` / `outputValues` / `defaultOutputEntry` | `OutputClause` | `parser.rs:860-862`, `987`, `990` |
| `rule` / `inputEntry` / `outputEntry` / `annotationEntry` | `DmnRule`, `RuleEntry` | `parser.rs:864-866`, `1055-1063` |
| `unaryTests` / `list` / `text` | `AllowedValues::UnaryTests` / `Enumeration` | `parser.rs:928` |
| `description` / `annotation` | `TextContent` / `Annotation` | `parser.rs:332`, `612`, `2050-2107` |
| `dmndi:DMNDiagram` / `dmndi:DMNShape` / `dmndi:DMNEdge` / `dc:Bounds` / `di:waypoint` / `dmndi:DMNLabel` | `DmnDiagram` / `DmnShape` / `DmnEdge` / `DcBounds` / `DcPoint` / `DmnLabel` | `parser.rs:1575`, `1635` |

Silently dropped: every element not in the tables above, because each element loop's fallback
is `self.skip_element(&ln)?` (`parser.rs:613`, `2115-2140`, and the same `_ =>
self.skip_element(...)` arm in every other element loop). There is no `extensionElements` /
`otherElements` string anywhere in `engine/src/dmn/`. The concrete losses are:

| Dropped content | Cite |
|---|---|
| `literalExpression`, `<context>`, `<list>` and `<relation>` decision bodies (`Decision` carries `literal_expression`, `boxed_context`, `boxed_list`, `boxed_relation`); `<functionDefinition>`, `<invocation>` and `<for>` bodies are still unmodelled | `engine/src/dmn/model.rs` |
| `extensionElements`, `otherElements`, vendor metadata | `parser.rs:613` |
| `description` on `decisionTable` (explicitly skipped; `DecisionTable` has no such field) and on `textAnnotation` | `parser.rs:870-873`, `1435-1437`; `engine/src/dmn/model.rs:310-328`, `277-283` |
| `id` on entry elements (`inputEntry`/`outputEntry`/`annotationEntry`) — `RuleEntry` has no id field | `engine/src/dmn/model.rs:379-382` |
| `dmndi:Size`, `di:waypoint` on shapes, `@id` on `DMNShape`/`DMNLabel` | `engine/src/dmn/model.rs:529-571` |

Dropping is consistent in both directions, which is why `ir1 == ir2` still holds for these
documents: the exporter cannot emit what the IR never held, and the re-parse drops the same
content again.

### 3.3 Security posture

XXE hardening is a **deny-by-event** policy, applied in the document driver and repeated in
every element loop:

| Event | Action | Cite |
|---|---|---|
| `Event::DocType(_)` | `Err(DmnParseError::XxeRejected("DOCTYPE declarations are not permitted (XXE hardening)"))` | `parser.rs:237-241`; the same arm appears in every one of the 30 event loops (30 `Event::DocType` matches, 63 `XxeRejected` constructions — PI included) |
| `Event::PI(_)` | `Err(DmnParseError::XxeRejected("processing instructions are not permitted (XXE hardening)"))` | `parser.rs:242-246` |
| `Event::GeneralRef` | numeric char refs resolved; only `gt`/`lt`/`amp`/`quot`/`apos` accepted; any other name is a hard `XmlError` | `parser.rs:2012-2042` |
| `Event::Decl(_)` | skipped | `parser.rs:236` |
| `expand_empty_elements` | `true` (empty-element splitting only — does not expand entities) | `parser.rs:218-221` |

Because DOCTYPE is rejected, no custom entity can be in scope, so external-entity and
entity-expansion attacks are structurally impossible rather than filtered. There is **no**
document-size limit and **no** element-nesting limit in the parser; the only resource caps in
the DMN surface are on the FEEL lexer/parser (§4).

### 3.4 CDATA, entities, text

`read_text_content` (`parser.rs:2047-2107`) accumulates `Event::Text` (entity-decoded via
`t.decode()`), `Event::CData` (sets `is_cdata = true` once *any* segment was CDATA —
flag-by-any-segment, `parser.rs:2059-2062`), and `Event::GeneralRef`. A nested `Start` element
inside a text-carrying element is a hard `UnexpectedElement` error (`parser.rs:2077-2084`), so
mixed content is rejected rather than flattened.

The CDATA flag is kept only where the IR has a `TextContent`. Fields modelled as plain
`String` go through `read_text_string`, which **discards** the flag
(`parser.rs:2110-2113`) — this affects `inputExpression`, `defaultOutputEntry`,
`AllowedValues::UnaryTests`, and DMNDI label text (`engine/src/dmn/model.rs:337`, `354`, `472`,
`570`).

Export: `DmnExportError` = `XmlWrite` | `InvalidModel` | `Io` (`exporter.rs:43-56`). Non-CDATA
text is emitted through `BytesText` (escape of `& < > "`); CDATA is emitted only when
`is_cdata` is set (`exporter.rs:802-815`, threat note at `exporter.rs:20-22`). The emitted
document always carries `<?xml version="1.0" encoding="UTF-8"?>` (no `standalone`) and a DMN
MODEL default namespace resolved as `source_namespace` → `namespace` attribute → constant
(`exporter.rs:35-37`, `100`, `112-123`).

Formatting is deliberately canonical, not source-faithful:

| Aspect | Emission | Cite |
|---|---|---|
| indentation | none — `Writer::new` (single line, no pretty-printing) | `exporter.rs:88` |
| root attribute order | fixed: `xmlns, id, name, namespace, typeLanguage, expressionLanguage, exporter, exporterVersion` | `exporter.rs:123-144` |
| child order inside a parent | fixed schema-ish order per parent (e.g. decision: `description → variable → informationRequirements → knowledgeRequirement → decisionTable → annotations`) — **not** source order | `exporter.rs:277-313` |
| `hitPolicy` | always emitted, including the `UNIQUE` default | `exporter.rs:349-351` |
| `isCollection` | emitted only when `true` | `exporter.rs:200-202` |
| DMNDI coordinates | integral floats printed as integers (`100.0` → `"100"`) | `exporter.rs:819-825` |
| `question` / `allowedAnswers` | always attribute form, even when the source used child elements | `exporter.rs:280-283` |

### 3.5 Round-trip guarantees and their evidence

Guarantee: `parse(export(parse(x))) == parse(x)` at IR level, plus byte-level structural
equality after namespace normalisation. Evidence:

| Test | What it proves | Cite |
|---|---|---|
| `dmn_round_trip::test_round_trip_decision_1_5` / `_1_3` / `_1_1` | full `DmnModel` equality across a parse → export → re-parse for all three accepted namespaces | `engine/tests/dmn_round_trip.rs:23-25` |
| `test_round_trip_no_hitpolicy` | absent `hitPolicy` exports as `hitPolicy="UNIQUE"` and round-trips | `dmn_round_trip.rs:28-37` |
| `test_round_trip_cdata_description` | exported XML contains `<![CDATA[` and the re-parsed flag is `true` | `dmn_round_trip.rs:39-48` |
| `test_round_trip_element_order` | child-variant order survives the cycle | `dmn_round_trip.rs:50-57` |
| `test_dmndi_preserving_round_trip`, `test_no_dmndi_added_when_absent`, `test_dmndi_namespace_preservation`, `test_dmndi_element_completeness`, `test_dmndi_round_trip_tck_fixture` | diagram count, DMNDI/DC/DI namespace emission, `Bounds` fidelity, and a real TCK fixture round-trip | `dmn_round_trip.rs:104-160` |
| `dmn_parser::test_xxe_rejected` + inline `parser_rejects_doctype_inline` | a `<!DOCTYPE … <!ENTITY …SYSTEM "file:///etc/passwd">>` document is rejected with `XxeRejected` before any entity is resolved | `engine/tests/dmn_parser.rs:159-170`; `engine/src/dmn/parser.rs:2266-2283` |
| `parser_zero_copy_preserves_attrs_cdata_and_entities` | CDATA flag survives while `&lt; 18` is decoded to `< 18` through the `GeneralRef` path | `parser.rs:2285-2332` |
| `test_no_hitpolicy_defaults_to_unique`, `test_element_order_preserved`, `test_cdata_description_preserved` | hit-policy default, document order, CDATA descriptions | `dmn_parser.rs:113`, `127`, `200` |
| `tck_sweep::execute_parser_layer` / `execute_round_trip_layer` / `execute_dmndi_layer` | the same guarantees over all 51 vendored vectors, with `structurally_equal` (`engine/src/dmn/test_util.rs:114-186`) as the secondary gate | `engine/tests/tck_sweep.rs:208-238`, `409-439`, `441-507` |

The DMNDI guarantee is weaker than the rest: `TCK-COVERAGE-GAPS.md:56-59` records 21/50
fixtures preserved, attributed to non-standard fixture directory naming, and the harness
counts a DMNDI "loss" as a failure with a recorded reason (`tck_sweep.rs:467-505`).

Two asymmetries are known and untested:

| Asymmetry | Mechanism | Cite |
|---|---|---|
| A namespace-less root does not round-trip | `source_namespace = None` after the first parse, but the exporter always emits an `xmlns`, so the re-parse records `Some(uri)` | `parser.rs:260-268`, `exporter.rs:108-120` |
| `AllowedValues::UnaryTests` loses CDATA-ness | stored as `String`, so `is_cdata` cannot survive | `engine/src/dmn/model.rs:472`, `parser.rs:2110-2113` |

The parser also has **no** resource limits: no document size cap and no element-nesting cap,
and `<itemComponent>` nesting recurses without bound (`parser.rs:452-458`). The FEEL
lexer/parser caps in §4 are the only guards in the DMN surface.

## 4. FEEL grammar

Hand-written lexer + recursive-descent/Pratt parser, grammar rules 1–65
(`engine/src/dmn/feel/parser.rs:1-34`).

### 4.1 Token kinds

`Token` (`parser.rs:63-129`): `Name(String)`, `Number(Decimal)`, `String(String)`,
`AtLiteral(String)`, `LParen`, `RParen`, `LBracket`, `RBracket`, `LBrace`, `RBrace`,
`Comma`, `Dot`, `Colon`, `Plus`, `Minus`, `Star`, `Slash`, `DoubleStar`, `Eq`, `NotEq`,
`Lt`, `Le`, `Gt`, `Ge`, `Question`, `Eof`, `LexError(String)`.

There is deliberately **no** `Keyword` and no boolean/null token: `true`, `false`, `null`,
`for`, `if`, `some`, `every`, `in`, `between`, `and`, `or`, `return`, `function`,
`instance`, `of`, `not` all lex as `Name` and are disambiguated by position in the parser
(`parser.rs:46-62`, `1401-1430`). Every token carries a `Span { start, len, line, col }`
(`parser.rs:136-142`, `engine/src/dmn/feel/ast.rs:49-59`). The lexer splits on `.` so that
qualified names are built from `Name Dot Name` rather than one token
(`parser.rs:446-475`).

### 4.2 Precedence ladder (lowest → highest)

| Level | Operators | Binding power | Associativity | Cite |
|---|---|---|---|---|
| 1 | `or` | 10 | left | `parser.rs:1208` |
| 2 | `and` | 20 | left | `parser.rs:1209` |
| 3 | `= != < <= > >= between in` | 30 (`in`/`between` included) | left | `parser.rs:1201-1210`, `1127` |
| 4 | `instance of` | 35 | n/a (postfix) | `parser.rs:1153` |
| 5 | `+ -` | 40 | left | `parser.rs:1196-1197` |
| 6 | `* /` | 50 | left | `parser.rs:1198-1199` |
| 7 | `**` | 60 | **right** | `parser.rs:1200`, `1173` |
| 8 | unary `-` | 70 | prefix | `parser.rs:1222-1231` |
| 9 | postfix `.name` / `[pred]` / `(args)` / `a..b` | 80 | left-to-right | `parser.rs:1237-1310` |
| 10 | primary (literal, name, `( )`, list, context, `if/for/some/every/function`) | 90 | — | `parser.rs:1350-1438` |

Right-associativity of `**` is implemented by `next_min = bp` (instead of `bp + 1`) for
right-assoc operators (`parser.rs:1173`), giving `2 ** 3 ** 2 = 512`
(`engine/src/dmn/feel/eval.rs:1046-1065`, `ast.rs:111-113`).

`between` is parsed specially so the greedy `and` is not consumed as a boolean operator: the
bounds are parsed at `min_bp = 31` and the `and` is eaten explicitly (`parser.rs:1127-1149`).
Its desugaring is `BinOp(Between, lhs, List([lower, upper]))` — a two-element `List`, not a
real list (`parser.rs:1146-1148`); the evaluator relies on that AST invariant and returns
`Null` for any other shape (`eval.rs:914-930`).

`in`'s operand is parsed as a normal expression at bp 30; unary-test forms are produced only
by the separate `parse_simple_unary_tests` entry point (§4.4).

### 4.3 Error recovery

The parser never `?`-propagates and never panics on untrusted FEEL text. On failure it emits
a `FeelExpression::Err(Span, String)` node at the error site, calls `sync_to_boundary()` to
advance to the next sync token, and continues (`parser.rs:25-29`, `1069-1088`). Sync tokens
are `,` `)` `]` `}` `Eof` plus the keywords `then`, `else`, `return`, `satisfies`
(`parser.rs:1076-1088`). `Err` nodes are propagated upward without being wrapped in
additional nodes (`parser.rs:1177-1180`, `1241-1243`, `1370-1372`). At evaluation time
`Err` maps to `Value::Null` (`eval.rs:417`), so a malformed cell degrades to null rather than
failing the table load.

### 4.4 Two entry points

| Entry point | Behaviour | Cite |
|---|---|---|
| `parse_expression(src)` | full `Expression` (rule 1) | `parser.rs:1862-1882` |
| `parse_simple_unary_tests(src)` | `SimpleUnaryTests` (rule 15): lone `-` → `Literal(Bool(true))` sentinel; optional `not` → `Neg(...)`; comma list → left-folded `BinOp(Or, …)`; a leading comparison operator → `BinOp(op, Name("?"), rhs)` | `parser.rs:1889-1935`, `1798-1845` |

### 4.5 DoS guards and their limits

| Guard | Default | On trip | Cite |
|---|---|---|---|
| Lexer token cap | `max_tokens = 10_000` | emits one `Token::LexError("max_tokens (N) exceeded")` and stops tokenising | `parser.rs:176-196`, `204-218` |
| Parser recursion depth | `max_depth = 256` | `FeelExpression::Err(span, "max recursion depth exceeded")` | `parser.rs:1011-1012`, `1090-1098`, `1121-1123` |
| Range materialisation (evaluator) | `MAX_RANGE_ITERATIONS = 1_000_000` | iteration source yields no elements | `engine/src/dmn/feel/eval.rs:1105-1160` |
| Singleton range nesting | a bound may not be `= value` | `FeelExpression::Err("a range bound cannot be a singleton range")` | `parser.rs`, `parse_primary` `Token::Eq` branch |

Limits worth noting: the depth cap is a `u32` counter incremented in `parse_expr` only, so
postfix chains (`a.b.c…`) and argument lists do not consume depth; the token cap counts
tokens, not bytes, and a `LexError` token is not a hard failure — `parse_primary` converts it
into an `Err` node and evaluation yields `Null` (`parser.rs:1431-1434`, `eval.rs:417`).

The singleton-range guard is a correctness guard as much as a DoS guard. `= value` builds a
point range by parsing a bound, and the bound parser re-enters the same rule, so before the guard
each additional `=` cloned the subtree into both `start` and `end`: twenty equals signs produced a
2^20-node tree (~170 MB of debug output), and twenty-four exhausted multi-gigabyte memory. A run
of `make fuzz` found it as a libFuzzer out-of-memory abort on `feel_parser`; `parse_expression`
now rejects a nested singleton range and `singleton_range_does_not_nest` holds the line.

### 4.6 Multi-word built-in name rule

The lexer emits one `Name` per word, but the registry is keyed by space-bearing names, so the
parser re-joins them. Rule (`parser.rs:1455-1484`):

1. If the first word is not a prefix of any registered multi-word name, stop — single word.
2. Greedily consume following `Name` tokens while the joined text stays a prefix of some
   registered name.
3. Accept the merge only if the join is **exactly** a registered name; otherwise rewind and
   return the first word alone.

The candidate set is derived from the registry itself — every key containing a space, sorted
(`engine/src/dmn/feel/builtins.rs:2224-2235`). This is why `upper case(x)` reaches the
registry as `"upper case"` while `a and contains(x)` keeps `and` as an operator
(`parser.rs:1440-1454`).

## 5. FEEL evaluation

### 5.1 Scope model

`Scope { bindings: HashMap<String, Value>, deps: HashMap<String, Vec<String>> }` — a **flat**
map plus an explicit dependency map. There is no parent chain; nested resolution is an
explicit dependency lookup (`engine/src/dmn/feel/eval.rs:34-57`). `bind` records the
dependency list alongside the value (`eval.rs:74-77`).

Name resolution:

| Form | Resolution | Cite |
|---|---|---|
| `Name(n)` | `bindings[n]` or `Null` | `eval.rs:393` |
| `QualifiedName([a,b,c])` | longest bound prefix — try `a.b.c`, then `a.b`, then `a` — then path-navigate the remaining components | `eval.rs:88-104` |
| `Path { target, name }` | one step: object key, or map over an array | `eval.rs:1335-1342`, `114-125` |
| `AtLiteral(s)` | `temporal::parse_at_literal(s)` or `Null` | `eval.rs:395` |

`QualifiedName` is **never constructed by the parser**: the lexer splits on `.` and
`parse_postfix` builds `Path(Name(a), "b")` (`parser.rs:446-475`, `1266-1277`). The variant
and its longest-match resolver exist but are unreachable in production (`eval.rs:394` is the
only consumer).

### 5.2 Boxed expressions

All ten boxed forms are implemented in `evaluate` (`eval.rs:389-540`):

| Variant | Semantics | Cite |
|---|---|---|
| `List` | evaluate each element → `Array` | `eval.rs:432-436` |
| `Relation` | per row, zip columns with evaluated cells into an `Object`; collect rows | `eval.rs:437-449` |
| `Context` | **topological** evaluation: build intra-context dep graph from free variables, DFS colour-mark, evaluate in dependency order | `eval.rs:450-467`, `725-838` |
| `FunctionDefinition` | build a `FeelFunction` carrier; body not evaluated; captures the definition-site scope by `Arc` | `eval.rs:526-542` |
| `FunctionInvocation` | see §5.5 | `eval.rs:468-512` |
| `If` | `true` → then, `false` → else, everything else (incl. `Null`, non-bool) → `Null` | `eval.rs:1162-1177` |
| `For` | cartesian product over iteration contexts → `Array` | `eval.rs:1192-1259` |
| `Quantified` | `some` folds with `or_3vl`, `every` with `and_3vl` | `eval.rs:1261-1333` |
| `Filter` | binds the element as `item`; keeps elements where the predicate is exactly `Bool(true)`; `Null` target → `Null`; non-list target is filtered as a singleton | `eval.rs:1345-1370` |
| `InstanceOf` | `Null` conforms only to `Any`; otherwise `types::conforms(type_of(v), ty)` | `eval.rs:1377-1384` |

A cyclic boxed context yields an `Object` containing only the cyclic entry keys mapped to
`Null` (`eval.rs:450-467`, `ContextCycleError` at `eval.rs:898-903`).

### 5.3 Lambda semantics and capture

Capture is **lexical**: `FunctionDefinition` stores `captured = Some(Arc::new(scope.clone()))`
at definition time (`eval.rs:538`). Application starts from the captured scope and then binds
parameters, so parameters shadow captured names of the same key (`eval.rs:589-593`,
`608-615`). Positional application requires `arg_vals.len() == param_names.len()` when any
positional argument was supplied, else `Null` (`eval.rs:584-586`). Named application zips
`param_names` in declaration order and yields `Null` for a missing named argument
(`eval.rs:565-577`). `external: true` is parsed but rejected at both invocation sites —
`lambda_apply`'s caller returns `Null` with an `eprintln!` diagnostic (`eval.rs:508-514`),
and `apply_function_value` returns `Null` immediately (`eval.rs:660-662`).

`apply_function_value(func, &[Value])` is the value-based application entry point used by
built-ins that receive already-evaluated arguments (notably `sort` with a comparator):
`engine/src/dmn/feel/eval.rs:651-675`, called from `engine/src/dmn/feel/builtins.rs:1369-1372`.

### 5.4 Null propagation and three-valued logic

The whole surface uses `Option<_>` as the "unknown" carrier; `None` becomes `Value::Null` only
at the evaluator boundary via `opt_bool` (`eval.rs:971-976`).

| Operation | Semantics | Cite |
|---|---|---|
| `feel_equals` | **any null operand → `None`** — equality is unknown when a side is null, so `null = null` is `Null`, not `true`; cross-kind → `None` (null, not false); Number/Integer promoted; `Date` ≡ `DateTimeTz` at UTC midnight; lists/contexts elementwise with `None` propagation | `eval.rs:163-205`, `277-314` |
| `feel_compare` | any null → `None`; cross-kind or unordered → `None` | `eval.rs:212-233` |
| `and_3vl` | `false` dominant; `true,true` → `true`; else `None` | `eval.rs:240-246` |
| `or_3vl` | `true` dominant; `false,false` → `false`; else `None` | `eval.rs:253-259` |
| `not_3vl` | `null` → `null`; else negate | `eval.rs:265-271` |
| `=` / `!=` | `feel_equals` / negated | `eval.rs:935-936` |
| `<` `<=` `>` `>=` | from `feel_compare` | `eval.rs:937-940` |
| `between` | `a >= b and a <= c` joined by `and_3vl` | `eval.rs:914-930` |
| `+`/`-`/`*`/`/`/`**` | any null → `Null`; div-by-zero and overflow → `Null`; no string↔number coercion | `eval.rs:990-1066` |

`Value::Range` equality is unimplemented: `feel_equals` has no `Range` arm, so two ranges
compare as `None` (`eval.rs:197-200`, `TODO(31-05)`).

### 5.5 `FunctionInvocation` dispatch — exact order

`engine/src/dmn/feel/eval.rs:468-512`:

1. **Registry first.** If the callee AST is a `Name(s)` *and* `s` is a registry key, the
   arguments are evaluated and the built-in is called immediately (`eval.rs:476-495`). The
   registry is consulted before the scope, so a scope binding named `sum` cannot shadow the
   built-in. Positional arguments are evaluated left to right; named arguments are bound to the
   built-in's declared parameter list (`bind_named_arguments`, `eval.rs:607-640`, using
   `builtins::builtin_param_names`, `builtins.rs:2163-2216`). That binding yields `Null` for an
   undeclared built-in, an unknown parameter name, a mix of positional and named arguments, or a
   gap before a supplied parameter; trailing omissions are allowed.
2. **Otherwise evaluate the callee** in the scope (`eval.rs:498-499`).
3. If the value is `Value::Function`:
   - `body.is_none()` → "unregistered built-in function" diagnostic to stderr, `Null`
     (`eval.rs:503-507`);
   - `external` → rejection diagnostic, `Null` (`eval.rs:508-514`);
   - otherwise `lambda_apply` (`eval.rs:515-518`).
4. Any other callee kind → `Null` (`eval.rs:520-523`).

The registry is rebuilt on **every** invocation: `builtins::builtin_registry()` returns a
freshly allocated `HashMap` per call (`builtins.rs:2237-2351`, called at `eval.rs:477`). There
is no memoised registry (contrast the AST cache in `engine/src/dmn/feel/cache.rs:196-320`).

## 6. Built-in function library

`builtin_registry() -> HashMap<&'static str, BuiltinFn>` with
`BuiltinFn = fn(&[Value]) -> Value` (`builtins.rs:37`, `2237`). **85 registered names** in 9
families. Arity is what the implementation accepts; every function returns `Value::Null` for
unsupported arities and wrong types, never an error and never a panic.

| Family | Name | Arity | Semantics | Cite |
|---|---|---|---|---|
| Conversion | `date` | 1 | `temporal::parse_at_literal` on a string; parse failure → null | `builtins.rs:110-122` |
| Conversion | `time` | 1 | same | `builtins.rs:125-136` |
| Conversion | `date and time` | 1 | same | `builtins.rs:139-150` |
| Conversion | `duration` | 1 | same; days-time or years-months | `builtins.rs:153-164` |
| Conversion | `number` | 1 | `Decimal` parse of a trimmed string; identity on `Number` | `builtins.rs:167-182` |
| Conversion | `string` | 1 | FEEL string form of any value; `string(null)` → null | `builtins.rs:185-199` |
| Conversion | `years and months duration` | 1 or 2 | 2-arg: month delta between two dates; 1-arg: year-month ISO duration only (rejects `T`/`D`/`H`/`S`) | `builtins.rs:202-238` |
| Numeric | `decimal` | 2 | `round_dp(scale)`; **`Number` first arg only** | `builtins.rs:262-284` |
| Numeric | `floor` | 1 | `Decimal::floor` | `builtins.rs:287-292` |
| Numeric | `ceiling` | 1 | `Decimal::ceil` | `builtins.rs:295-300` |
| Numeric | `round up` | 1–2 | `AwayFromZero` at optional scale (default 0) | `builtins.rs:335-337`, `306-331` |
| Numeric | `round down` | 1–2 | `ToZero` | `builtins.rs:341-343` |
| Numeric | `round half up` | 1–2 | `MidpointAwayFromZero` | `builtins.rs:347-349` |
| Numeric | `round half down` | 1–2 | `MidpointTowardZero` | `builtins.rs:353-355` |
| Numeric | `abs` | 1 | absolute value of `Number`/`Integer` and of both duration kinds | `builtins.rs:358-371` |
| Numeric | `modulo` | 2 | Decimal `%`; div-by-zero → null; mixed kinds → null | `builtins.rs:374-396` |
| Numeric | `sqrt` | 1 | negative → null | `builtins.rs:399-410` |
| Numeric | `log` | 1 | natural log; `x <= 0` → null | `builtins.rs:413-424` |
| Numeric | `exp` | 1 | `e**x`; overflow → null | `builtins.rs:427-435` |
| Numeric | `odd` | 1 | non-integral `Number` → null | `builtins.rs:438-452` |
| Numeric | `even` | 1 | non-integral `Number` → null | `builtins.rs:455-468` |
| String | `substring` | 2–3 | 1-based start clamped to `[1, len+1]`; optional length | `builtins.rs:478-515` |
| String | `string length` | 1 | character count | `builtins.rs:518-523` |
| String | `upper case` | 1 | `to_uppercase` | `builtins.rs:526-531` |
| String | `lower case` | 1 | `to_lowercase` | `builtins.rs:534-539` |
| String | `substring before` | 2 | empty string when the needle is absent | `builtins.rs:542-561` |
| String | `substring after` | 2 | empty string when the needle is absent | `builtins.rs:564-583` |
| String | `replace` | 3 | **regex** replace; bad pattern → null | `builtins.rs:586-611` |
| String | `contains` | 2 | literal containment | `builtins.rs:614-619` |
| String | `starts with` | 2 | literal prefix | `builtins.rs:622-627` |
| String | `ends with` | 2 | literal suffix | `builtins.rs:630-635` |
| String | `matches` | 2 | **regex** match; no flags argument | `builtins.rs:638-658` |
| String | `split` | 2 | literal delimiter (not regex) | `builtins.rs:661-679` |
| String | `string join` | 2 | non-string elements dropped | `builtins.rs:682-703` |
| List | `list contains` | 2 | membership by `feel_equals == Some(true)` | `builtins.rs:710-722` |
| List | `list replace` | 3 | regex replace on string elements; bad pattern → **empty array** | `builtins.rs:725-761` |
| List | `count` | 1 | element count, `count([null]) == 1` | `builtins.rs:764-772` |
| List | `min` | 1 | numeric elements only; empty → null | `builtins.rs:775-810` |
| List | `max` | 1 | numeric elements only; empty → null | `builtins.rs:813-847` |
| List | `sum` | 1 | numeric elements only; empty → **null** | `builtins.rs:850-870` |
| List | `mean` | 1 | arithmetic mean of numeric elements | `builtins.rs:873-901` |
| List | `all` | 1 | vacuously true for `[]`; a non-bool element → null | `builtins.rs:904-923` |
| List | `any` | 1 | vacuously false for `[]`; a non-bool element → null | `builtins.rs:926-945` |
| List | `sublist` | 2–3 | 1-based start, clamped; optional length | `builtins.rs:948-979` |
| List | `append` | 2 | push one value | `builtins.rs:982-995` |
| List | `concatenate` | 2 | two lists concatenated | `builtins.rs:998-1016` |
| List | `insert before` | 3 | 1-based position, clamped to `len` | `builtins.rs:1019-1040` |
| List | `remove` | 2 | out-of-range position → list unchanged | `builtins.rs:1043-1067` |
| List | `reverse` | 1 | in place copy | `builtins.rs:1070-1082` |
| List | `index of` | 2 | 1-based positions of all matches | `builtins.rs:1085-1102` |
| List | `union` | 2 | order preserving, deduplicated by `feel_equals` | `builtins.rs:1105-1134` |
| List | `distinct values` | 1 | dedupe by `feel_equals` | `builtins.rs:1137-1163` |
| List | `flatten` | 1 | one level | `builtins.rs:1166-1183` |
| List | `product` | 1 or 1…n | a list or separate numbers; a null or non-numeric element makes the call null; empty → null | `builtins.rs` |
| List | `median` | 1 or 1…n | a list or separate numbers; even length → mean of the two middles; a null or non-numeric element makes the call null | `builtins.rs` |
| List | `stddev` | 1 or 1…n | **sample** standard deviation (n − 1); fewer than two values → null | `builtins.rs` |
| List | `mode` | 1 or 1…n | always an ascending list; `mode([])` is an empty list | `builtins.rs` |
| List | `sort` | 1–2 | optional FEEL comparator applied via `apply_function_value`; incomparable pairs stable | `builtins.rs:1354-1405` |
| Date-time | `now` | 0 | `DateTimeValue::Fixed` at the injected clock, UTC | `builtins.rs:1429-1435` |
| Date-time | `today` | 0 | date part of the same clock | `builtins.rs:1443-1450` |
| Date-time | `day of year` | 1 | 1–366; accepts a date or a date-and-time | `builtins.rs` |
| Date-time | `day of week` | 1 | the English day name (`"Tuesday"`); accepts a date or a date-and-time | `builtins.rs` |
| Date-time | `month of year` | 1 | the English month name (`"September"`); accepts a date or a date-and-time | `builtins.rs` |
| Date-time | `week of year` | 1 | ISO week number 1–53; accepts a date or a date-and-time | `builtins.rs` |
| Boolean | `not` | 1 | `not_3vl`; non-bool → null | `builtins.rs:1533-1543` |
| Context | `context` | 0–1 | empty context, or a context rebuilt from `"k:v"` strings with number/bool/string inference | `builtins.rs:1556-1585` |
| Context | `context put` | 3 | key must be a string (no coercion despite the doc) | `builtins.rs:1592-1612` |
| Context | `context merge` | 2 | later keys win | `builtins.rs:1618-1636` |
| Context | `get value` | 2 | string key only; absent key → null | `builtins.rs:1642-1656` |
| Context | `get entries` | 1 | list of `{key, value}` contexts, sorted by key | `builtins.rs:1662-1689` |
| Range | `before` | 2 | `r1.end < r2.start` | `builtins.rs:1722-1743` |
| Range | `after` | 2 | `r1.start > r2.end` | `builtins.rs:1747-1766` |
| Range | `meets` | 2 | strictly before **and** numeric gap exactly 1 | `builtins.rs:1771-1810` |
| Range | `met by` | 2 | `meets(r2, r1)` | `builtins.rs:1813-1820` |
| Range | `overlaps` | 2 | `r1.start < r2.end && r2.start < r1.end` | `builtins.rs:1824-1853` |
| Range | `overlaps before` | 2 | `overlaps(r2, r1)` | `builtins.rs:1856-1863` |
| Range | `overlaps after` | 2 | `overlaps(r1, r2)` | `builtins.rs:1866-1872` |
| Range | `finishes` | 2 | equal ends and `r1.start <= r2.start` | `builtins.rs:1876-1906` |
| Range | `finished by` | 2 | equal starts and mutual overlap | `builtins.rs:1910-1944` |
| Range | `includes` | 2 | scalar with endpoint inclusivity honoured; sub-range by endpoint comparison | `builtins.rs:1949-2020` |
| Range | `during` | 2 | `includes(r, x)` | `builtins.rs:2024-2031` |
| Range | `starts` | 2 | equal starts and `r1.end <= r2.end` | `builtins.rs:2035-2065` |
| Range | `started by` | 2 | `starts(r2, r1)` | `builtins.rs:2068-2075` |
| Range | `coincides` | 2 | equal starts and equal ends | `builtins.rs:2078-2109` |
| Equality | `is` | 2 | **total** two-valued equality: `is(null,null) == true`, never null | `builtins.rs:2126-2149` |

Multi-word names (the set that `merge_multiword_name` can produce) are exactly the registry
keys containing a space (`builtins.rs:2224-2235`): `date and time`, `years and months
duration`, `round up`, `round down`, `round half up`, `round half down`, `string length`,
`upper case`, `lower case`, `substring before`, `substring after`, `starts with`,
`ends with`, `string join`, `list contains`, `list replace`, `insert before`, `index of`,
`distinct values`, `day of year`, `day of week`, `month of year`, `week of year`,
`context put`, `context merge`, `get value`, `get entries`, `met by`, `overlaps before`,
`overlaps after`, `finished by`, `started by`.

Named-argument invocation (`abs(n: -1)`) is supported only for built-ins that declare a
parameter list in `builtin_param_names` (`builtins.rs:2163-2216`): `abs`, `sqrt`, `exp`, `log`,
`even`, `odd`, `floor`, `ceiling`, the four `round *` functions, `modulo`, `decimal`, `number`,
the eight numeric list reducers, all thirteen string functions, `list contains`, `count`, `sort`,
`all`, `any`, `append`, `concatenate`, `union`, `insert before`, `remove`, `index of`, `sublist`,
`distinct values`, the five context functions, and `date`/`time`/`date and time`/`duration`/
`years and months duration`/`string` — **60 of the 85 names**. The remaining 25 (`is`, `not`,
the time-query functions, `flatten`, `reverse`, `list replace`, and all fourteen range
relations) accept positional arguments only; invoking one by name evaluates to `null`
(`eval.rs:607-619`).

### 6.1 Null and error conventions

- Every built-in is total: `fn(&[Value]) -> Value`. There is no error channel; a failure is
  `Value::Null` (`builtins.rs:34-37`).
- Null propagation is centralised in `null_unary` / `null_binary` / `null_ternary`
  (`builtins.rs:62-103`) and applied by most scalar operators.
- Documented null-domain exceptions: `count([null]) == 1` (`builtins.rs:769`),
  `all([]) == true` / `any([]) == false` (`builtins.rs:913`, `935`), empty
  `sum`/`min`/`max`/`mean`/`product`/`median`/`stddev`/`mode` → `Null` (`builtins.rs:858-860`,
  `784`, `822`, `882`, `1195`, `1218`, `1258`, `1307`), `sort([]) == []`
  (`builtins.rs:1359-1361`), and `is(null, null) == true` (`builtins.rs:2135-2137`) — `is` is
  the only total two-valued built-in, which is why it special-cases null before consulting
  `feel_equals` (`builtins.rs:2135-2148`).
- Three-valued exceptions: `not`, `all`, `any` and `is` are the only built-ins that reason
  about `None` explicitly (`builtins.rs:916-918`, `938-940`, `1539`, `2144-2148`).
- Regex-backed built-ins use a shared compiled-pattern cache; compilation failure maps to
  `Null` at the boundary (`builtins.rs:46-58`, `605-608`, `653-656`), except `list replace`,
  which returns an empty array (`builtins.rs:744-747`).
- Diagnostic output: two reject paths write directly to `stderr` via `eprintln!`
  (`eval.rs:506-513`). These are the only log-shaped side effects in the FEEL path.

### 6.2 Numeric domain (Decimal128)

All FEEL numerics are `rust_decimal::Decimal`; the headline property is `0.1 + 0.2 == 0.3`
(`engine/src/dmn/feel/numeric.rs:1-6`, `58-63`). `Value::Float(f64)` exists in
`crate::data_types::Value` but is never constructed on this path.

| Rule | Cite |
|---|---|
| Lexical numbers → `Decimal::from_str_exact`; failure → `LexError` token | `parser.rs:408-431` |
| `Number`/`Integer` are one domain; arithmetic promotes `Integer` via `Decimal::from` | `coercion.rs:39-46`, `numeric.rs:20-51` |
| Equality treats `Number(5) == Integer(5)`; `Integer/Integer` compared directly | `eval.rs:177-183` |
| Division by zero and overflow yield `Null` (no `Result`, no panic) | `eval.rs:951-957`, `972-986` |
| `**` uses `checked_powd`; right-associativity comes from the AST shape | `eval.rs:1046-1065` |
| Rounding scale must be a non-negative integral `Integer`/`Number`; otherwise the whole result is `Null` | `builtins.rs:306-331` |
| 128-bit Decimal limits are not surfaced: an unrepresentable intermediate yields `Null` | `eval.rs:1040-1043` |

## 7. Decision tables

`DecisionTableEvaluator::evaluate(&DecisionTable, &Scope, &FeelAstCache) -> Result<Value, DecisionTableError>`
is a stateless unit-struct entry point (`engine/src/dmn/decision_table.rs:52-69`). Errors:
`HitPolicyViolation{policy,message}`, `AggregatorMismatch`, `InvalidOutputValue` (never
constructed), `Other` (`decision_table.rs:29-45`).

### 7.1 Input-entry matching

For each rule, every input clause must match; `zip` is over clauses and entries
(`decision_table.rs:83-100`).

1. The cell text is parsed as **`SimpleUnaryTests`** via the AST cache, keyed by
   `(entry kind, trimmed text)` (`decision_table.rs:125`, `engine/src/dmn/feel/cache.rs:46-49`).
2. The clause's `inputExpression` is parsed as **`Expression`** and evaluated in the scope
   (`decision_table.rs:128-129`).
3. `eval::eval_in_unary_test(input_value, &ast)`; only `Some(true)` fires — `Some(false)` and
   `None` both mean "does not fire" (`decision_table.rs:132-135`).

Recognised cell forms and their exact outcomes:

| Cell text | Parse to | Fires? | Cite |
|---|---|---|---|
| `-` | `Literal(Bool(true))` sentinel | always | `engine/src/dmn/feel/parser.rs:1906-1915`, `engine/src/dmn/feel/eval.rs:1455` |
| `< x`, `<= x`, `> x`, `>= x`, `= x`, `!= x` | `BinOp(op, Name("?"), rhs)` | yes | `parser.rs:1802-1823`, `eval.rs:1437-1440` |
| `x > ?` (placeholder on the right) | operator inverted | yes | `eval.rs:1442-1454` |
| `a, b, c` | left-folded `BinOp(Or, …)` folded with `or_3vl` | per element | `parser.rs:1827-1843`, `eval.rs:1426-1435` |
| bare literal (`5`, `"High"`, `false`) | `Literal(...)` | **no** — only `Literal(Bool(true))` has an arm | `eval.rs:1455-1456` |
| bare `true` | `Literal(Bool(true))` | yes, unconditionally (input not compared) | `eval.rs:1455` |
| `not(...)` | `Neg(...)` | **no** — no `Neg` arm | `parser.rs:1930-1934`, `eval.rs:1423-1458` |
| interval `[1..10]` | `Range` | **no** — no `Range` arm | `parser.rs:1504-1544`, `eval.rs:1456` |
| parse error | `Err` | **no**, silently | `parser.rs:1871-1873`, `eval.rs:417` |

An unparseable or unrecognised cell never produces an error; the rule simply does not fire
(`decision_table.rs:132-135`).

### 7.2 Three-valued behaviour

`Null` is collapsed into "does not fire": there is no distinct unknown status, so a null input
value excludes the rule exactly like `false` (`decision_table.rs:115-135`). Nulls arise from a
null operand in an ordering (`eval.rs:214`), cross-kind equality (`eval.rs:201-203`), an
unordered kind (`eval.rs:231`), or an unbound name (`eval.rs:393`).

### 7.3 Hit policies

| Policy | Behaviour | Zero matches | Cite |
|---|---|---|---|
| `UNIQUE` | 1 match → packed output; ≥2 → `Err(HitPolicyViolation{"UNIQUE", "{n} rules matched but UNIQUE allows exactly one"})` | defaults | `decision_table.rs:159-171` |
| `ANY` | every later match compared to the first per output column with `feel_equals_public == Some(true)`; anything else (including two null outputs, since FEEL equality on null is unknown) is a disagreement → `Err(HitPolicyViolation{"ANY", "conflicting outputs for column '{col}': …"})` | defaults | `decision_table.rs:174-199`, `eval.rs:3529-3531` |
| `PRIORITY` | position of the **first output** in the first output clause's `outputValues`; highest position wins, strict `>` so ties go to the earlier document rule; non-enumerated matches skipped | defaults | `decision_table.rs:213-246` |
| `FIRST` | `matches.first()`; no conflict detection | defaults | `decision_table.rs:202-210` |
| `RULE ORDER` | all matches in document order | `[]` | `decision_table.rs:287-306` |
| `OUTPUT ORDER` | sorted by enumeration position; non-enumerated → `i64::MAX` (last); stable sort so ties keep document order | `[]` | `decision_table.rs:249-284` |
| `COLLECT` | **first output column only**, then optional aggregator | aggregator: `Null`/`0`; else `[]` | `decision_table.rs:309-336`, `374-383` |

Aggregators (`decision_table.rs:386-459`):

| Aggregator | Accepted inputs | Empty set | Cite |
|---|---|---|---|
| `SUM` | `Value::Number` only; anything else → `AggregatorMismatch("SUM requires numeric outputs")` | `Null` | `decision_table.rs:396-409` |
| `MIN` | `Value::Number` only → `AggregatorMismatch("MIN requires numeric outputs")` | `Null` | `decision_table.rs:412-431` |
| `MAX` | `Value::Number` only → `AggregatorMismatch("MAX requires numeric outputs")` | `Null` | `decision_table.rs:434-453` |
| `COUNT` | not inspected — counts matching rules | `Number(0)` | `decision_table.rs:456-459` |

Load-time validation (`validate_decision_table`, `decision_table.rs:531-564`) enforces:
COLLECT+aggregation needs exactly one output column (`533-538`); PRIORITY and OUTPUT ORDER need
`output_values` on the first output clause (`541-556`); `aggregation` is only valid with
COLLECT (`559-561`). It returns `Result<(), String>` and is **not called** by
`DecisionEvaluator` — only by `engine/tests/hit_policy_advanced.rs:168,183`.

### 7.4 Output packing

- Single output clause and one value → the bare scalar; otherwise `Value::Object`
  (`decision_table.rs:495-510`).
- Context keys are `label` → `name` → `output_{idx}` (`decision_table.rs:513-522`).
- Zero matches: `UNIQUE`/`ANY`/`FIRST`/`PRIORITY` return
  `null_with_defaults` (`decision_table.rs:466-492`), which evaluates each clause's
  `defaultOutputEntry` as an `Expression` in **`Scope::empty()`** and returns a scalar for one
  clause or a context for several. `RULE ORDER`, `OUTPUT ORDER` and `COLLECT` never consult
  the defaults — they return `[]`, `Null`, or `Number(0)` (`decision_table.rs:254`, `292`,
  `313-318`).
- `input_values` / `output_values` / `ItemDefinition.allowed_values` are **not** enforced as
  constraints on input or output values at evaluation time; `output_values` is read only as
  the PRIORITY / OUTPUT ORDER ordering key (`decision_table.rs:339-347`).

## 8. DRD evaluation

### 8.1 Types

| Type | Shape | Cite |
|---|---|---|
| `EvaluateRequest` | `{ inputs: HashMap<String, Value>, trace: bool }` | `engine/src/dmn/evaluator/mod.rs:53-59` |
| `EvaluateResult` | `{ outputs, rules_fired: u64, duration_ms: f64, trace: Option<InferenceTrace>, drd_chain_depth: usize, outputs_evaluated: Vec<String> }` | `evaluator/mod.rs:62-76` |
| `SharedDmnModel` | `{ model, ast_cache, generation, drd_order_cache: RwLock<HashMap<String, Vec<String>>> }` | `evaluator/mod.rs:79-93` |
| `DecisionEvaluator` | `{ pub shared: Arc<RwLock<SharedDmnModel>> }` with `new`, `evaluate(&self, id, request)`, `reload(&self, &str)` | `evaluator/mod.rs:397-406`, `422`, `590-603` |
| `DecisionEvalError` | `NotFound`, `EvaluationFailed` (never constructed), `Internal`, `DrdCycle(Vec<String>)`, `ParseError`, `TableError` | `evaluator/mod.rs:610-630` |

### 8.2 Dependency resolution and cycle detection

- Only `DefinitionsChild::Decision` nodes participate (`evaluator/mod.rs:237-244`).
- The reachable set is computed over **`required_decisions` only** (`evaluator/mod.rs:190-198`,
  `212-231`); `required_inputs` and `required_knowledge` are ignored by the evaluator.
- Order: DFS with white/gray/black colouring over all decisions in sorted id order, then a
  filter to the target's reachable set (`evaluator/mod.rs:158-235`).
- A gray re-entry raises `DrdCycle(collect_cycle(...))` (`evaluator/mod.rs:183-185`);
  `collect_cycle` returns the walked path, capped at 100 nodes (`evaluator/mod.rs:247-269`).
  Because the DFS covers the whole model before filtering, **a cycle anywhere in the document
  — even one unreachable from the target — fails the evaluation** (`evaluator/mod.rs:205-215`).
- The order is memoised per target and invalidated by `bump_generation` on reload
  (`evaluator/mod.rs:108-145`).

### 8.3 Level-based parallelism

`level(d) = 1 + max(level(dep))` over in-reachable `required_decisions`, or 0 with no
dependency (`evaluator/mod.rs:272-326`). Levels are then evaluated in order with a barrier
between levels (`evaluator/mod.rs:467-531`). Within a level, `work.len() > 1` dispatches a
**rayon** `par_iter()`; a single-decision level stays sequential to avoid pool overhead
(`evaluator/mod.rs:11`, `487-503`). There are no configuration knobs — the threshold is the
literal `> 1` test. Each decision's scope is `request.inputs` plus the merged context of all
earlier levels, i.e. a superset of its actual dependencies (`evaluator/mod.rs:446-485`).
Per-decision outputs merge under `variable.name` using the **first entry of the output
HashMap** (arbitrary order), or merge flat when the decision has no variable
(`evaluator/mod.rs:517-528`).

Decision services are **not supported at evaluation time**: `DecisionService` is modelled and
round-tripped, and a call to one is resolved by `resolve_invocations`: the service's
parameters (its input decisions and input data, in document order) bind the arguments, the
output decisions are evaluated with them, and the result is the single output's value or a
context of several (`evaluator/mod.rs`, `call_invocable`). A call that supplies the wrong
number of arguments, names a parameter that does not exist, or passes a value of the wrong
primitive type evaluates to null.

A business knowledge model is evaluated the same way, but bound into the decision's scope as a
function value (`bind_model_functions`) so that a call inside a `for` body sees the iteration
variables. FEEL evaluation has no view of the model, so call resolution rewrites the decision's
AST before evaluation, with a depth cap (`MAX_INVOCATION_DEPTH`) against cyclic models.

Prometheus metrics are emitted per evaluation: `rrules_dmn_output_evaluations_total`
(`evaluator/mod.rs:29-34`) and `rrules_dmn_drd_chain_depth_seconds`, a histogram whose name
says seconds but which observes a decision *count* (`evaluator/mod.rs:37-46`, `442-444`).

### 8.4 Trace contents

`InferenceTrace { decision_id, started_at: Instant, events: Vec<TraceEvent> }`, and
`TraceEvent { elapsed_ms, event_type, message, rule_index: Option<usize>, inputs: Vec<(String,Value)>, output: Option<Value> }`
(`engine/src/dmn/evaluator/trace.rs:12-37`).

Captured only when `request.trace` is true (`evaluator/mod.rs:461-465`, `583`). Emitted events:

| `event_type` | When | Payload | Cite |
|---|---|---|---|
| `drd_resolve` | once per resolved upstream decision | message only | `evaluator/mod.rs:509-516` |
| `evaluate_table` | once for the target, if it has a table | message carries the hit policy; `output` is the first output value | `evaluator/mod.rs:547-557` |
| `merge_scope` | once, if upstream outputs were merged | `output` is the merged context | `evaluator/mod.rs:558-569` |

`rule_index` is always `None` and `inputs` is always empty; the `match_rule` /
`aggregate` / `resolve_reference` event types named in the module doc (`trace.rs:16`) are never
emitted. `tracker::DmnDecisionTracker` separately accumulates per-decision evaluation counts
and a last-evaluated stamp, and offers `find_dead_decisions` / `evaluation_summary` /
`coverage_for` (`engine/src/dmn/tracker.rs:26-63`); only the requested decision id is recorded
(`evaluator/mod.rs:574-577`).

## 9. Conformance

### 9.1 How the harness works

`engine/tests/tck_sweep.rs` is a single `#[test] fn tck_sweep()` that walks a vendored OMG DMN
TCK corpus. **51 vector directories** are vendored, one `.dmn` each
(`engine/tests/fixtures/tck/*/`); discovery requires both a `.dmn` and a test-XML file, and
skips dot/underscore directories (`engine/src/dmn/test_util.rs:32-69`). Fixtures are
git-tracked and located via `env!("CARGO_MANIFEST_DIR")` — there is no env-var override and no
download; a missing corpus directory panics (`tck_sweep.rs:107-112`) while an empty discovery
prints a warning and returns success (`tck_sweep.rs:117-120`).

Three separate harnesses exist: this XML-driven sweep (the only one that reports
totals), the ad-hoc per-vector readers `engine/tests/feel_tck_{numeric,temporal,subset}.rs`
(which read only `0100-arithmetic` and `fixtures/tck/feel/feel_grammar_subset.json`, and whose
`SKIP_LIST` constants are documentation rather than pruning — `feel_tck_numeric.rs:36-56`;
`feel_tck_subset.rs:254-283` compares arrays by **length only** and objects by **key set
only**), and `engine/tests/tck_precision_audit.rs`, a literal scanner over the same files.
`TckVector.category` is the *parent directory* name (`engine/src/dmn/test_util.rs:99-104`),
which is `"tck"` for every vendored vector, so the "category" strings printed beside the
documented divergences are not produced by the harness.

A vector is run through **four** layers (`tck_sweep.rs:134-169`):

| Layer | What it checks | Cite |
|---|---|---|
| ParserOnly | parse → export → re-parse, then `ir1 == ir2`, else `structurally_equal` after `normalize_ns` | `tck_sweep.rs:208-238` |
| Evaluator | every `<testCase>`: evaluate the decision named by the case, compare all expected outputs | `tck_sweep.rs:240-390` |
| RoundTrip | the same round-trip as ParserOnly (duplicated code, second label) | `tck_sweep.rs:409-439` |
| DMNDI | diagram count + IR equality for DMNDI-bearing fixtures | `tck_sweep.rs:441-507` |

Two corpus facts bear on the numbers below, both verifiable with `cmp`/`grep` over
`engine/tests/fixtures/tck/`:

- **14 of the 51 `*-test-*.xml` files are byte-identical copies of the vector's `.dmn`.** They
  contain no `<testCases>`, so `parse_test_cases` yields nothing and the Evaluator layer
  returns `skip_result("No extractable test cases from XML")` (`tck_sweep.rs:256-259`).
- **50 of the 51 vectors contain DMNDI markup** (the exception is
  `fixtures/tck/0068-feel-equality/`), which is the denominator of the documented "21/50
  preserved".
- `categorize_by_level` is a directory-name prefix heuristic (`tck_sweep.rs:532-537`) that
  splits the corpus into **45 level-2** and **6 level-3** vectors (`0100-arithmetic`,
  `1100-feel-decimal-function`, `1141`–`1144`) — not the 110/13 the documentation reports.
- The published totals live in a **generated artifact**, not in the test: `write_results_report`
  rewrites a `TCK_RESULTS.md` under the workspace-level `.planning/` tree — which is not part of
  this repository, so the file is a build artefact of the sweep rather than something a reader of
  the repository can open (`tck_sweep.rs:597-649`). Running the suite writes outside the tree it
  is testing.

Parser and round-trip layers are the **same computation**: `execute_parser_layer`
(`tck_sweep.rs:208-238`) and `execute_round_trip_layer` (`tck_sweep.rs:409-439`) differ only in
the layer tag and message strings, so two of the three counted passes per vector are one check
counted twice.

Typed value parsing (`engine/tests/tck_values.rs:369-420`): `xsd:string`, `xsd:boolean`,
`xsd:decimal|double|float|integer|int|long`, `xsd:date`, `<list><item>`, and
`<component name>` contexts. **Everything else degrades to `Value::String`** — notably
`xsd:time`, `xsd:dateTime` and `xsd:duration`.

Comparison policy (`tck_values.rs:425-449`): numbers by value (`Number`/`Integer`
interchangeable), strings exact, booleans exact, dates exact, lists and contexts elementwise,
and a singleton list unwraps against a scalar. There is **no** epsilon and no tolerance.
Expected values with no `xsd` type are inferred: `"true"`/`"false"` → `Bool`, empty/`"null"`
→ `Null`, a decimal parse → `Number`, else `String` (`tck_values.rs:403-418`).

Case acceptance: `errorResult="true"` passes when every output is `Null` (or evaluation
errors); otherwise every expected `(name, value)` pair must be found — by output name, or by a
structural match against any produced output — and pass `deep_equal`; a case with no
extractable expectation fails (`tck_sweep.rs:297-363`). A vector passes only when all of its
cases pass (`tck_sweep.rs:365-373`).

Decision targeting is by **id**: `collect_decision_ids` collects `Decision.id`
(`tck_sweep.rs:393-407`), then the harness looks for an expectation whose *name* appears in
that id list and otherwise falls back to the model's first decision
(`tck_sweep.rs:276-281`). In the vendored corpus `resultNode name` is the decision **name**
(e.g. `filter01`) while `decision/@id` is a UUID (e.g. `_4a786da5-…`) — see
`engine/tests/fixtures/tck/0001-filter/`. Cross-checking every vendored vector, only **5 of
1510** distinct `resultNode` names occur in their model's decision-id set, so the id lookup
almost always misses and the case is evaluated against the model's first declared decision —
correct only for single-decision models.

### 9.2 Numbers that can be re-derived from the harness

| Claim in `TCK-COVERAGE-GAPS.md` | What the harness actually does |
|---|---|
| "51/51 vectors" | 51 `.dmn` fixture directories exist and the test prints the discovered count (`tck_sweep.rs:114-115`); **no test asserts 51** anywhere in the repository. |
| "123 layer passes (51 × 3 layers)" | The harness defines **four** layers (`tck_sweep.rs:134-169`, `648`), so "51 × 3" equals neither the layer count nor 123. |
| "110/110 + 13/13 by compliance level" | `by_level` increments `vectors` once **per layer pass**, not per vector (`tck_sweep.rs:185-192`), so per-level totals count layer passes; `categorize_by_level` is a directory-prefix heuristic (`tck_sweep.rs:532-537`). |
| "0 failures" | Only ParserOnly (and DMNDI) results reach `record_pass_fail` for failures; the Evaluator and RoundTrip layers record a result **only when it passes** (`tck_sweep.rs:149-159`), and the process exits non-zero only on `results.failed > 0` (`tck_sweep.rs:129-131`, `594`). A wrong evaluation result therefore cannot fail the test. |
| Evaluator layer skipped | 14 of 51 vectors have a test XML identical to the DMN, so their Evaluator layer is a skip (`tck_sweep.rs:256-259`); skips from that layer are also not recorded. |
| Known-divergence deduction | `record_pass_fail` matches a divergence with `dir_name.contains(kd.vector)` (`tck_sweep.rs:194-205`), but the ids are `DMN15-31-collect-distinct-sum` / `sum-empty-list` while the directories are `0114-collect-aggregators` / `0100-arithmetic`, so the deduction never fires. |
| Temporal expectations compared | `xsd:time`/`dateTime`/`duration` have no branch in `scalar_to_value` (`tck_values.rs:384-420`) and no arm in `deep_equal` (`tck_values.rs:425-449`), so a temporal expectation can only match if the actual value is also a string. |

The documented total reproduces **only** as **51 ParserOnly + 51 RoundTrip + 21 DMNDI = 123**,
i.e. with **zero Evaluator passes**, and three independent decompositions of the generated
artifact agree: `45×2 + 20 = 110` for level 2, `6×2 + 1 = 13` for level 3, and `21×3 + 30×2 =
123` over the per-directory table. `dmndi_preserved` is incremented only in the two `Pass` arms
(`tck_sweep.rs:490-500`), so the 21 third-passes are DMNDI passes, not evaluator passes. This is
arithmetic over a committed artifact, not an observed run; what is directly verifiable from the
code is that the suite cannot distinguish "evaluator never passed" from "evaluator passed",
because that layer's failures and skips are discarded.

Two further holes weaken the per-case comparison even where it does run. An
`errorResult="true"` case passes when *every* output is `Null`, and `all()` over an **empty**
output map is vacuously true (`tck_sweep.rs:302-309`), so an evaluation producing no outputs
passes an error case; and because the flag is captured from any one `resultNode`
(`engine/tests/tck_values.rs:249-258`), a case mixing a normal and an error result is judged
all-null. An expectation whose element shape is unrecognised becomes `TckNode::Null`
(`tck_values.rs:331`), which then compares equal to a null result (`tck_values.rs:427`) — a
silent pass rather than the "visible mismatch" the module doc claims (`tck_values.rs:10-12`).
On an output-name miss the harness also accepts **any** produced output that compares equal to
the expectation (`tck_sweep.rs:314-328`), so "every expected output must match" is weaker than
stated.

Derived statement, defensible from the code: the corpus has 51 vectors; the parser and DMNDI
layers are hard-gated; the evaluator layer is strict per case (all cases, all expected
outputs, exact structural equality) but its outcome is invisible to the pass/fail gate. The
"51/51, 123/123, 0 failures" figures are documentation, not assertions.

### 9.3 Documented divergences

| Divergence | Stated reason | Reflected in code |
|---|---|---|
| `DMN15-31` COLLECT with `distinct` + SUM (`0114-collect-aggregators`) | DMN 1.6 / FEEL 1.6 feature, out of Level 3 scope | Yes at the model level — `BuiltinAggregator` has no `distinct` (`engine/src/dmn/model.rs:436-445`) |
| `sum([])` returning `Decimal(0)` (`0100-arithmetic`) | `rust_decimal` empty-iterator default | No longer reproduces — `sum([])` is `Null` (`builtins.rs:858-860`); the divergence entry is stale (`tck_sweep.rs:85-92`, `TCK-COVERAGE-GAPS.md:47-57`) |
| DMNDI round-trip 21/50 | non-standard fixture directory naming | Not a code divergence; the layer counts losses explicitly (`tck_sweep.rs:467-505`) |
| Multi-word names, `sort` comparator, `round *` scale, `get entries` shape | previously-fixed defects | Confirmed fixed at `parser.rs:1455-1484`, `builtins.rs:1365-1402`, `builtins.rs:306-331`, `builtins.rs:1662-1689` |
| An internal phase document reported `Parser 51/51, Round-trip 51/51, **Evaluator 21/51**` and said the sweep failed only on parser/DMNDI layers | conflicts with the root doc's "51/51 evaluator"; its 21 equals the DMNDI pass count. That document lives in the workspace-level `.planning/` tree, which this repository does not publish — the measured state is `TCK-COVERAGE-GAPS.md` | `TCK-COVERAGE-GAPS.md:1-20`, `tck_sweep.rs:467-505` |

## 10. Known gaps

Implementation defects, stubs and spec/code disagreements. Entries closed on 2026-10-25 are struck
through with what replaced them; the rest are current.

| # | Gap | Evidence |
|---|---|---|
| 1 | ~~Unary tests are evaluated in an **empty scope**~~ — **closed 2026-10-25**: `eval_in_unary_test_with_scope` binds `?` to the input and resolves names in the caller's scope, which the decision table passes | `engine/src/dmn/feel/eval.rs` (`eval_in_unary_test_with_scope`), `engine/src/dmn/decision_table.rs:132` |
| 2 | ~~`eval_in_unary_test` supports only `Or`, `?`-comparisons and the `-` sentinel~~ — **closed 2026-10-25**: ranges (`[1..10]`, `(1..10]`), comma lists, bare literals and `not(...)` all evaluate; a null result does not match | `engine/src/dmn/feel/eval.rs` (`range_contains`) |
| 3 | ~~`week of year` always returns 1~~ — **closed 2026-10-25**: it formats the ISO week with `%V` and parses it | `engine/src/dmn/feel/builtins.rs` (`iso_week_num`) |
| 4 | ~~Multi-output decisions broadcast one packed value to every output clause~~ — **closed 2026-10-25**: `project_output` projects each clause out of the context (or out of each element of a rule-order list) | `engine/src/dmn/evaluator/mod.rs` (`project_output`) |
| 5 | ~~`COLLECT` reads only the first output column~~ — **closed 2026-10-25**: a multi-output COLLECT returns one context per matching rule; single-output tables aggregate as before | `engine/src/dmn/decision_table.rs` (`handle_collect`) |
| 6 | ~~TCK sweep swallows evaluator and round-trip failures~~ — **closed 2026-10-25**: every layer outcome is recorded and printed, and the suite gates on a documented 143/31 baseline | `engine/tests/tck_sweep.rs` |
| 7 | ~~TCK typed-value support omits temporal types and `deep_equal` has no temporal arm~~ — **closed 2026-10-25**: a temporal result compares against the TCK's ISO 8601 string (and `xsd:date` parses directly) | `engine/tests/tck_values.rs` (`is_temporal`, `numbers_close`) |
| 8 | ~~`decimal(num, scale)` accepts `Value::Number` only~~ — the scale argument is now taken from either numeric carrier and clamped to `Decimal::MAX_SCALE`; a null/negative/non-integral scale yields null. A `String` first argument is still null, which is the spec's "no implicit conversion" rule | `engine/src/dmn/feel/builtins.rs` (`builtin_decimal`) |
| 9 | `context put` documents key coercion but returns `Null` for a non-string key | `engine/src/dmn/feel/builtins.rs:1601-1607` |
| 10 | `get entries` doc says it returns `"key:value"` strings; the code returns `{key, value}` contexts | `engine/src/dmn/feel/builtins.rs:1662-1689` |
| 11 | `mode` returns a bare scalar when there is exactly one mode, contradicting the "per DMN spec" comment (the spec returns a list of modes) | `engine/src/dmn/feel/builtins.rs:1335-1340` |
| 12 | ~~`min`/`max`/`sum`/`mean`/`product`/`median`/`stddev` skip non-numeric elements instead of returning null~~ **closed 2026-10-25**: `numeric_series` rejects a null or non-numeric element, making the whole call null; `all`/`any` likewise reject non-booleans | `engine/src/dmn/feel/builtins.rs` |
| 13 | `matches` ignores the optional flags argument; `date`/`time`/`date and time` ignore extra arguments and have no component-based overloads | `engine/src/dmn/feel/builtins.rs:638-658`, `110-150` |
| 14 | `list replace` returns an empty array on a bad regex while every other regex built-in returns null | `engine/src/dmn/feel/builtins.rs:744-747` |
| 15 | `FeelExpression::QualifiedName` is never produced by the parser (the lexer splits on `.` and `Path` is built instead), so `Scope::resolve_qn`'s longest-match rule is dead in production | `engine/src/dmn/feel/parser.rs:446-475`, `1266-1277`; only consumer `engine/src/dmn/feel/eval.rs:394` |
| 16 | `parse_type_ref` discards type parameters: `list<number>` becomes `List(Any)`, `context<…>` an empty context, `function<…>` an unconstrained function | `engine/src/dmn/feel/parser.rs:1747-1792` |
| 17 | `Value::Range` equality is a `TODO`; two ranges compare as null | `engine/src/dmn/feel/eval.rs:197-200` |
| 18 | The built-in registry is reallocated on every `FunctionInvocation`; there is no memoised `OnceLock` equivalent to `multi_word_builtin_names` | `engine/src/dmn/feel/builtins.rs:2237-2351` called from `engine/src/dmn/feel/eval.rs:477` |
| 19 | Two reject paths write to `stderr` with `eprintln!` inside the evaluator | `engine/src/dmn/feel/eval.rs:506-513` |
| 20 | The namespace allow-list is mislabelled (`20200524` is DMN 1.3, not 1.5; `20180521` is 1.2) and omits the real DMN 1.5 URI; the check's result is discarded, so `DmnParseError::InvalidNamespace` is unreachable | `engine/src/dmn/parser.rs:79-87`, `266-268`; variant declared at `engine/src/dmn/model.rs:586-587` |
| 21 | `DmnParseError::InvalidNamespace`, `DecisionTableError::InvalidOutputValue`, `DecisionEvalError::EvaluationFailed` are never constructed | `engine/src/dmn/model.rs:586`, `engine/src/dmn/decision_table.rs:40`, `engine/src/dmn/evaluator/mod.rs:617` |
| 22 | Unknown XML children (`extensionElements`, `otherElements`, vendor metadata) are silently dropped, so no extension data survives a round-trip | `engine/src/dmn/parser.rs:613` and the identical `_ => self.skip_element(...)` arm in every element loop |
| 23 | ~~`BusinessKnowledgeModel` has no body field, so BKM invocation is unimplemented~~ **closed 2026-10-25**: the BKM carries its formal parameters and body, and a call to one evaluates the body with the arguments bound. `required_knowledge` is still not used to order evaluation, and a BKM whose body is not a literal expression is skipped | `engine/src/dmn/model.rs`, `evaluator/mod.rs` |
| 24 | `DecisionService` is parsed and exported but cannot be evaluated; a service id yields `NotFound` | `engine/src/dmn/model.rs:251-262`, `engine/src/dmn/evaluator/mod.rs:536-541` |
| 25 | A cycle **anywhere** in the document fails every evaluation of every decision, because the DFS runs over the whole model before filtering to the target | `engine/src/dmn/evaluator/mod.rs:205-215` |
| 26 | `drd_resolve` merges upstream output via `out.outputs.iter().next()`, i.e. an arbitrary HashMap entry | `engine/src/dmn/evaluator/mod.rs:519-524` |
| 27 | `validate_decision_table` exists but is never called in production, so the COLLECT-single-output and PRIORITY-needs-allowedValues invariants are unenforced at run time | `engine/src/dmn/decision_table.rs:531-564`; only callers `engine/tests/hit_policy_advanced.rs:168`, `183` |
| 28 | `defaultOutputEntry` is evaluated in `Scope::empty()` with a freshly constructed `FeelAstCache`, so it can neither see inputs nor reuse the model cache | `engine/src/dmn/decision_table.rs:479-481` |
| 29 | `defaultOutputEntry` is ignored by `RULE ORDER`, `OUTPUT ORDER` and `COLLECT` | `engine/src/dmn/decision_table.rs:254`, `292`, `313-318` |
| 30 | `input_values` is parsed into the IR and then never read by anything | `engine/src/dmn/model.rs:339`; no reader outside the parser |
| 31 | `HitPolicy::Priority`'s doc comment does not state the priority direction; the code treats the **last** enumeration entry as highest priority | `engine/src/dmn/model.rs:395-399` vs `engine/src/dmn/decision_table.rs:225-236` |
| 32 | `DmnDecisionTracker::last_evaluated` is documented as RFC 3339 but stores `"{unix_seconds}s"` | `engine/src/dmn/tracker.rs:16` vs `tracker.rs:65-70` |
| 33 | The trace module promises `GET /v1/decisions/{id}/trace`; no such route exists | `engine/src/dmn/evaluator/trace.rs:5` |
| 34 | `rrules_dmn_drd_chain_depth_seconds` is a histogram of a decision **count**, not a duration | `engine/src/dmn/evaluator/mod.rs:37-46`, `442-444` |
| 35 | The FEEL lexer has no `not` prefix operator, so boolean negation exists only as the `not(...)` built-in; the AST has no `Not` variant either | `engine/src/dmn/feel/parser.rs:1222-1231`, `engine/src/dmn/feel/ast.rs:78-123` |
| 36 | The recursion-depth guard counts only `parse_expr` entries, so deep postfix/argument chains are not depth-limited | `engine/src/dmn/feel/parser.rs:1090-1098`, `1121-1123` |
| 37 | `test_util` and `bench_util` are unconditional `pub mod`s, so TCK and benchmark helpers ship in non-test builds | `engine/src/dmn/mod.rs:27`, `34` |
| 38 | The TCK harness targets decisions by **id** while `resultNode name` carries the decision **name**, so the name lookup always misses and every case is evaluated against the model's first decision | `engine/tests/tck_sweep.rs:276-281`, `393-407`; corpus evidence `engine/tests/fixtures/tck/0001-filter/` |
| 39 | 14 of 51 vendored `*-test-*.xml` fixtures are byte-identical copies of their `.dmn`, so those vectors' Evaluator layer silently skips | `engine/tests/fixtures/tck/*/`, `engine/tests/tck_sweep.rs:256-259` |
| 40 | `DmnExportError::InvalidModel` is declared and never constructed (the exporter performs no IR validation) | `engine/src/dmn/exporter.rs:52` |
| 41 | CDATA-ness is lost for every FEEL-text field modelled as `String` (`inputExpression`, `defaultOutputEntry`, `unaryTests`, DMNDI labels) | `engine/src/dmn/parser.rs:2110-2113`; `engine/src/dmn/model.rs:337`, `354`, `472`, `570` |
| 42 | A namespace-less root document fails IR round-trip: `source_namespace` goes `None` → `Some(uri)` because the exporter always emits `xmlns` | `engine/src/dmn/parser.rs:260-268`, `engine/src/dmn/exporter.rs:108-120` |
| 43 | The XML parser has no size or nesting limits, and `<itemComponent>` recursion is unbounded | `engine/src/dmn/parser.rs:452-458` |
| 44 | `Decision.id`/`InputData.id` are mandatory IR strings, but the DMN schema makes them optional, so a document without ids is rejected rather than represented | `engine/src/dmn/model.rs:204`, `226`; `engine/src/dmn/parser.rs:566-570` (`MissingElement("decision id attribute")`) |
| 45 | `number` declares three parameters (`from`, `grouping separator`, `decimal separator`) but the implementation reads only `args[0]`, so both separators are ignored | `builtins.rs:2163-2216` vs `builtins.rs:167-182` |
| 46 | `sort` declares only `["list"]`, so `sort(list, comparator: f)` by name is rejected as an unknown parameter even though positional `sort(list, f)` works | `builtins.rs:2163-2216` vs `builtins.rs:1354-1405` |
| 47 | `matches` declares two parameters; DMN 1.5 also defines a flags argument, which neither the declaration nor the implementation accepts | `builtins.rs:2163-2216`, `builtins.rs:638-658` |
| 48 | `all`/`any` return `Null` when a non-boolean element is present, even when a later element settles the fold (`all([5, false])` is null, not `false`) | `builtins.rs:904-945` |
| 49 | General entity references are silently dropped in `<itemComponent>` enumeration literals: `read_text_or_text_child` has `Text`/`CData` arms but no `GeneralRef` arm, so `a &amp; b` becomes `a  b` while the same text in a `<description>` is decoded | `engine/src/dmn/parser.rs:1928-1990` vs `parser.rs:2047-2107` |
| 50 | Entity validation is not universal: unknown/custom entity references inside a skipped (unknown) element are discarded rather than rejected, because only `read_text_content` resolves references | `engine/src/dmn/parser.rs:2115-2149`, `2063-2066` |
| 51 | DMNDI reference attributes keep a leading `#` while all `href` references strip it, so a document that uses `#` in `dmnElementRef` re-exports unchanged but resolves inconsistently | `engine/src/dmn/parser.rs:1662-1665`, `1718-1719` vs `2237-2239` |
| 52 | The exporter never emits `extensionElements`/`otherElements`, non-table boxed expressions, `<decisionTable><description>`, entry ids, `<association><sourceRef>` children, or `dmndi:Size` — a decision that used a `literalExpression` exports as a childless `<decision>` | `engine/src/dmn/exporter.rs:171-180`, `308-310`; `engine/src/dmn/parser.rs:616`, `870-873`, `1117-1167`, `1487`, `1631` |
| 53 | An `errorResult="true"` case passes vacuously when the evaluation produced **no** outputs, because `all()` over an empty map is true | `engine/tests/tck_sweep.rs:302-309` |
| 54 | On an output-name miss the harness accepts any produced output that compares equal, so output naming is unverified | `engine/tests/tck_sweep.rs:314-328` |
| 55 | An expected value with an unrecognised element shape becomes `TckNode::Null` and then compares equal to a null result — a silent pass, contrary to the module doc's "visible mismatch" claim | `engine/tests/tck_values.rs:331`, `427`, `10-12` |
| 56 | `errorResult` is captured per `resultNode` but consumed per **case**, so a case mixing normal and error result nodes is judged all-null | `engine/tests/tck_values.rs:249-258` |
| 57 | `fail_result`/`skip_result` hardcode `TestLayer::ParserOnly`, so every reported failure is attributed to the parser layer regardless of origin | `engine/tests/tck_sweep.rs:539-544` |

### Fixture integrity (fixed 2026-10-25)

14 of the 51 vendored vectors shipped a `*-test-*.xml` that was a byte-for-byte copy of the
`.dmn` model, so the sweep found no `<testCases>` and *skipped* the evaluator layer for them —
which is how a suite can report "zero failures" over vectors it never evaluated. The real upstream
test cases were re-vendored from `github.com/dmn-tck/tck` (all 14 resolved, from
`compliance-level-2` or `compliance-level-3`), the skip count is now zero, and the evaluator layer
runs for all 51 vectors.

Two further decision-table defects were found and fixed while doing it: `PRIORITY` compared
enumeration positions backwards (the *lowest*-priority output won), and an enumeration written the
way the TCK writes one (`"Approved", "Declined"` as unary-test text) was rejected outright instead
of being parsed as an ordered list.
