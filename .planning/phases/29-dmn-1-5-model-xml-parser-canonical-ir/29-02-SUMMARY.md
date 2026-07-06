---
phase: 29-dmn-1-5-model-xml-parser-canonical-ir
plan: 02
subsystem: engine
tags: [dmn, quick-xml, xml-parser, xxe-hardening, cdata, streaming-parser]

# Dependency graph
requires:
  - phase: 29-dmn-1-5-model-xml-parser-canonical-ir-01
    provides: "Canonical DMN 1.5 IR types (DmnModel, Definitions, ItemDefinition, DecisionTable, HitPolicy, TextContent, DmnParseError) in engine/src/dmn/model.rs"
provides:
  - "Streaming namespace-aware DMN 1.1/1.3/1.5 XML parser (parse_dmn / parse_dmn_bytes / parse_dmn_file) in engine/src/dmn/parser.rs"
  - "XXE-hardened event loop rejecting DOCTYPE/PI with DmnParseError::XxeRejected"
  - "CDATA-aware TextContent accumulation (is_cdata flag for round-trip)"
  - "General entity reference resolution (&gt; &amp; &lt; &quot; &apos; + numeric char refs) via resolve_general_ref"
  - "8-fixture integration test corpus under engine/tests/fixtures/dmn/"
affects: [29-03, 30, 31, 32]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "quick-xml 0.40 NsReader streaming pull parser with read_event_into(&mut buf) + into_owned() borrow-break pattern"
    - "Event::GeneralRef resolution for the 5 predefined XML entities + numeric char refs (quick-xml emits these as separate events, NOT inlined into Event::Text)"
    - "Namespace-agnostic local-name matching (match by local name, not qualified name) for prefix-independent DMN import"
    - "XXE hardening via Event::DocType / Event::PI rejection (no entity resolver, no DTD subset)"
    - "expand_empty_elements = true (field, not method) to split <x/> into <x></x> for uniform child parsing"

key-files:
  created:
    - "engine/src/dmn/parser.rs (full parser implementation)"
    - "engine/tests/dmn_parser.rs (8 integration tests)"
    - "engine/tests/fixtures/dmn/decision_1_5.dmn"
    - "engine/tests/fixtures/dmn/decision_1_3.dmn"
    - "engine/tests/fixtures/dmn/decision_1_1.dmn"
    - "engine/tests/fixtures/dmn/no_hitpolicy.dmn"
    - "engine/tests/fixtures/dmn/cdata_description.dmn"
    - "engine/tests/fixtures/dmn/xxe_payload.dmn"
  modified:
    - "engine/src/dmn/parser.rs (GeneralRef handling added during Task 2 verification)"

key-decisions:
  - "Use quick-xml NsReader streaming pull parser (never quick_xml::de serde-XML — serde-XML loses attribute order and CDATA fidelity, breaking round-trip per Pitfall 11)"
  - "Match elements by local name (prefix-agnostic) so any namespace prefix imports; recognize DMN MODEL namespace URIs for 1.1/1.3/1.5"
  - "Reject DOCTYPE/PI as DmnParseError::XxeRejected (XXE hardening T-29-01); never enable entity resolver or DTD subset"
  - "Resolve Event::GeneralRef explicitly — quick-xml 0.40 emits entity references as separate events, NOT inlined into Event::Text; the 5 predefined XML entities + numeric char refs are resolved, unknown named entities error (no custom entities can exist since DOCTYPE is rejected)"
  - "hitPolicy absent -> HitPolicy::Unique default (DMN-04); explicit guard sets the default when attribute is missing"
  - "FEEL expressions stored as String/TextContent (NOT parsed) — Phase 31 parses FEEL"
  - "Element order preserved via Vec insertion order (Pitfall 11) — never sort children"
  - "DMN 1.3 fixture uses <itemComponent><text>...</text></itemComponent> children inside <outputValues> for the Enumeration form (standard DMN), not the plan's prose <outputValues><text>...</text></outputValues> form"

patterns-established:
  - "Borrow-break pattern: every Event::Start(e) arm does let e = e.into_owned() before calling &mut self methods, because e borrows self.buf which self.reader.read_event_into(&mut self.buf) needs"
  - "read_text_content is the central text accumulator: handles Event::Text, Event::CData, Event::GeneralRef; unknown events fall through to error rather than silent drop"
  - "read_text_or_text_child helper handles both raw text and <text>-child forms (DMN enumeration standard form)"
  - "collect_attrs uses #[allow(deprecated)] Attribute::unescape_value() (Attribute::normalized_value requires XmlVersion arg in quick-xml 0.40)"

requirements-completed: [DMN-04, DMN-05]

# Coverage metadata (#1602)
coverage:
  - id: D1
    description: "Streaming namespace-aware DMN 1.1/1.3/1.5 XML parser that imports .dmn files into the canonical IR preserving item definitions, decisions, descriptions, annotations, IDs, and element order"
    requirement: DMN-04
    verification:
      - kind: integration
        ref: "engine/tests/dmn_parser.rs#test_parse_dmn_1_5_imports"
        status: pass
      - kind: integration
        ref: "engine/tests/dmn_parser.rs#test_parse_dmn_1_3_namespace"
        status: pass
      - kind: integration
        ref: "engine/tests/dmn_parser.rs#test_parse_dmn_1_1_namespace"
        status: pass
      - kind: integration
        ref: "engine/tests/dmn_parser.rs#test_element_order_preserved"
        status: pass
    human_judgment: false
  - id: D2
    description: "XXE-hardened parser rejecting DOCTYPE/ENTITY payloads with DmnParseError::XxeRejected (no file read)"
    requirement: DMN-04
    verification:
      - kind: integration
        ref: "engine/tests/dmn_parser.rs#test_xxe_rejected"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/parser.rs#parser::tests::parser_rejects_doctype_inline"
        status: pass
    human_judgment: false
  - id: D3
    description: "CDATA section preservation: TextContent.is_cdata flag set for CDATA-wrapped text and content preserved (including literal &)"
    requirement: DMN-04
    verification:
      - kind: integration
        ref: "engine/tests/dmn_parser.rs#test_cdata_description_preserved"
        status: pass
    human_judgment: false
  - id: D4
    description: "hitPolicy absent defaults to HitPolicy::Unique (DMN-04)"
    requirement: DMN-04
    verification:
      - kind: integration
        ref: "engine/tests/dmn_parser.rs#test_no_hitpolicy_defaults_to_unique"
        status: pass
    human_judgment: false
  - id: D5
    description: "Clause 7 item definitions and information items parsed into ItemDefinition/InformationItem structs (DMN-05)"
    requirement: DMN-05
    verification:
      - kind: integration
        ref: "engine/tests/dmn_parser.rs#test_item_definition_clause_7"
        status: pass
    human_judgment: false

# Metrics
duration: 45min
completed: 2026-07-06
status: complete
---

# Phase 29 Plan 02: DMN 1.5 XML Parser Summary

**Streaming namespace-aware quick-xml NsReader parser importing DMN 1.1/1.3/1.5 into the canonical IR with XXE rejection, CDATA preservation, entity reference resolution, and Clause 7 item definitions**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-07-06T20:14:09Z
- **Completed:** 2026-07-06T20:58:23Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- Implemented the streaming namespace-aware DMN parser (`parse_dmn` / `parse_dmn_bytes` / `parse_dmn_file`) using quick-xml 0.40's `NsReader` with the `read_event_into(&mut buf)` + `into_owned()` borrow-break pattern
- XXE hardening: rejects `Event::DocType` / `Event::PI` with `DmnParseError::XxeRejected`; no entity resolver, no DTD subset (T-29-01 mitigation)
- CDATA-aware text accumulation: `Event::Text` + `Event::CData` merged into `TextContent` with `is_cdata` flag for round-trip fidelity
- General entity reference resolution: handles `&gt;` `&amp;` `&lt;` `&quot;` `&apos;` + numeric char refs via `resolve_general_ref` (quick-xml emits these as separate `Event::GeneralRef` events, not inlined into `Event::Text`)
- hitPolicy absent → `HitPolicy::Unique` default (DMN-04)
- Clause 7 item definitions + information items parsed (`ItemDefinition`, `InformationItem`, recursive `itemComponent`)
- 6-fixture integration test corpus covering DMN 1.1/1.3/1.5, no-hitpolicy, CDATA, and XXE payload
- All 8 integration tests + 8 unit tests pass; `cargo build` clean (only 2 pre-existing out-of-scope `ComparisonOp` warnings)

## Task Commits

Each task was committed atomically:

1. **Task 1 (TDD RED): add failing test for DMN parser entry** - `aac43afba` (test)
2. **Task 1 (TDD GREEN): implement namespace-aware XXE-hardened DMN parser** - `e0ed0d1c9` (feat)
3. **Task 2: integration test corpus + fix GeneralRef entity handling** - `6604faec8` (test)

_Note: Task 1 is `tdd="true"` — RED commit adds the failing test, GREEN commit adds the implementation. The GeneralRef bug fix was a Rule 1 auto-fix discovered during Task 2 verification and folded into the Task 2 commit._

## Files Created/Modified
- `engine/src/dmn/parser.rs` - Full DMN 1.5 parser: `Parser<'a>` struct with `NsReader`, XXE rejection, CDATA handling, GeneralRef resolution, recursive item-definition parsing, hit-policy/aggregator parsing, DMNDI geometry parsing
- `engine/tests/dmn_parser.rs` - 8 integration tests: `test_parse_dmn_1_5_imports`, `test_parse_dmn_1_3_namespace`, `test_parse_dmn_1_1_namespace`, `test_no_hitpolicy_defaults_to_unique`, `test_cdata_description_preserved`, `test_xxe_rejected`, `test_item_definition_clause_7`, `test_element_order_preserved`
- `engine/tests/fixtures/dmn/decision_1_5.dmn` - DMN 1.5 fixture (4 children: itemDefinition, decision, textAnnotation, association)
- `engine/tests/fixtures/dmn/decision_1_3.dmn` - DMN 1.3 fixture (PRIORITY hit policy + Enumeration outputValues)
- `engine/tests/fixtures/dmn/decision_1_1.dmn` - DMN 1.1 fixture (COLLECT + SUM aggregator)
- `engine/tests/fixtures/dmn/no_hitpolicy.dmn` - Decision table with no hitPolicy attribute (UNIQUE default)
- `engine/tests/fixtures/dmn/cdata_description.dmn` - CDATA sections in decision + rule descriptions
- `engine/tests/fixtures/dmn/xxe_payload.dmn` - DOCTYPE + ENTITY xxe payload (must be rejected)

## Decisions Made
- **quick-xml NsReader streaming pull parser** (never `quick_xml::de` serde-XML) — serde-XML loses attribute order and CDATA fidelity, breaking round-trip (Pitfall 11). Acceptance criterion: `grep -c "quick_xml::de::from_reader|from_str" parser.rs` returns 0.
- **Namespace-agnostic local-name matching** — elements matched by local name (after `:`) so any prefix imports; DMN MODEL namespace URIs for 1.1/1.3/1.5 recognized on `<definitions>`.
- **Event::GeneralRef explicit resolution** — quick-xml 0.40 emits `&gt;`, `&amp;`, `&#62;` etc. as separate `GeneralRef` events, NOT inlined into `Event::Text`. A naive `Ok(_) => {}` catch-all silently dropped them (`&gt;= 18` became `= 18`). Fixed via `resolve_general_ref` handling the 5 predefined XML entities + numeric char refs; unknown named entities error (DOCTYPE rejection means no custom entities can exist).
- **DMN 1.3 fixture Enumeration form** — used `<itemComponent><text>"Yes"</text></itemComponent>` children inside `<outputValues>` (standard DMN enumeration form), not the plan's prose `<outputValues><text>"Yes","No"</text></outputValues>` form. The test asserts `AllowedValues::Enumeration(_)` which requires the itemComponent form. Added `read_text_or_text_child` helper to handle both raw-text and `<text>`-child forms (Rule 2 auto-fix).
- **`expand_empty_elements = true` as a field** (not a method) — `reader.config_mut().expand_empty_elements = true;` splits `<x/>` into `<x></x>` for uniform child parsing; it does NOT expand entities.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Event::GeneralRef entity references silently dropped**
- **Found during:** Task 2 (integration test `test_item_definition_clause_7` failed: `&gt;= 18` became `= 18`)
- **Issue:** quick-xml 0.40 emits entity references (`&gt;`, `&amp;`, `&#62;`) as separate `Event::GeneralRef` events, NOT inlined into `Event::Text`. The `Ok(_) => {}` catch-all in `read_text_content` silently dropped them, stripping all entity-decoded characters from text content.
- **Fix:** Added `resolve_general_ref` helper resolving the 5 predefined XML entities (`gt`→`>`, `lt`→`<`, `amp`→`&`, `quot`→`"`, `apos`→`'`) + numeric character references via `BytesRef::resolve_char_ref()`. Added an `Event::GeneralRef(r)` arm to `read_text_content` that resolves and pushes the decoded text. Unknown named entities are an error (not a silent drop) — since DOCTYPE/ENTITY are rejected by XXE hardening, no custom entities can be in scope.
- **Files modified:** `engine/src/dmn/parser.rs`
- **Verification:** All 8 integration tests pass; `test_item_definition_clause_7` now asserts `>= 18` correctly.
- **Committed in:** `6604faec8` (Task 2 commit)

**2. [Rule 2 - Missing Critical] read_text_or_text_child helper for DMN enumeration form**
- **Found during:** Task 1 (GREEN phase — DMN 1.3 fixture parsing)
- **Issue:** `parse_allowed_values` used `read_text_content("itemComponent")` which fails when itemComponent has a `<text>` child (the standard DMN enumeration form `<itemComponent><text>"Yes"</text></itemComponent>`), because `read_text_content` errors on unexpected child `<text>`.
- **Fix:** Added `read_text_or_text_child` helper handling both raw text inside an element AND a `<text>`-child form. Used for itemComponent enumeration literals.
- **Files modified:** `engine/src/dmn/parser.rs`
- **Verification:** `test_parse_dmn_1_3_namespace` passes asserting `AllowedValues::Enumeration(_)`.
- **Committed in:** `e0ed0d1c9` (Task 1 GREEN, amended)

---

**Total deviations:** 2 auto-fixed (1 bug, 1 missing critical)
**Impact on plan:** Both auto-fixes necessary for correctness — the GeneralRef fix restores entity-decoded text (a real data-loss bug), the read_text_or_text_child fix handles the standard DMN enumeration form. No scope creep.

## Issues Encountered
- **quick-xml borrow checker conflict (E0499/E0502):** `self.read_href_attribute(&e)` conflicted with `self.reader.read_event_into(&mut self.buf)` because event `e` borrows `self.buf`. Solved by the `let e = e.into_owned();` borrow-break pattern on every `Event::Start` arm, plus making `read_href_attribute`/`collect_attrs` free functions.
- **`expand_empty_elements` is a field not a method (E0599):** `reader.config_mut().expand_empty_elements(true)` failed; fixed to `reader.config_mut().expand_empty_elements = true;`.
- **`Attribute::normalized_value` requires `XmlVersion` arg (E0061):** Reverted to deprecated `attr.unescape_value()` with `#[allow(deprecated)]`.
- **Doc comment tripped streaming-parser grep acceptance:** A doc comment contained the literal `quick_xml::de::from_reader` string; reworded to `We do NOT use the quick_xml::de serde-XML deserializer` so `grep -c` returns 0.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Parser complete and tested; ready for Plan 29-03 (DMN exporter/serializer) which will consume the canonical IR and re-emit `.dmn` XML with namespace declarations, CDATA sections, and element order preserved.
- FEEL expression parsing deferred to Phase 31 — FEEL stored as `String`/`TextContent` in the IR.
- No blockers.

## Self-Check: PASSED

All 9 created/modified files exist on disk. All 3 task commits (aac43afba, e0ed0d1c9, 6604faec8) present in git log. All 8 integration tests + 8 unit tests pass. `cargo build -p engine` succeeds with only 2 pre-existing out-of-scope `ComparisonOp` warnings.

---
*Phase: 29-dmn-1-5-model-xml-parser-canonical-ir*
*Completed: 2026-07-06*