---
phase: 29-dmn-1-5-model-xml-parser-canonical-ir
plan: 03
subsystem: testing
tags: [dmn, quick-xml, xml-serialization, round-trip, cdata, dmn-1-5]

# Dependency graph
requires:
  - phase: 29-01
    provides: "canonical DMN 1.5 IR types (DmnModel, Definitions, TextContent.is_cdata, HitPolicy/BuiltinAggregator as_str) — the types the exporter serializes"
  - phase: 29-02
    provides: "namespace-aware NsReader parser (parse_dmn) + import-corpus fixtures at engine/tests/fixtures/dmn/ — round-trip tests parse(export(parse(x)))"
provides:
  - "DMN XML exporter (engine/src/dmn/exporter.rs) — export_dmn / export_dmn_to_bytes serializing DmnModel to .dmn XML via quick_xml::writer::Writer"
  - "Round-trip DOM-compare gate (engine/tests/dmn_round_trip.rs) — parse(export(parse(x))) == parse(x) for the import corpus (DMN-06)"
  - "CDATA round-trip fidelity — TextContent.is_cdata drives Event::CData vs escaped BytesText emission"
  - "Element-order preservation — Vec<DefinitionsChild> iterated in order (Pitfall 11)"
  - "DMNDI passthrough emission (dmndi/dc/di namespaces) for DMN-07 prep"
affects: [30-dmn-feel-expression-language, 32-dmn-hit-policy-evaluation, 34-dmn-frontend, 35-dmndi-enrichment]

# Tech tracking
tech-stack:
  added: []
  patterns: ["event-based XML emission via quick_xml::writer::Writer (no serde-XML — preserves attribute order + CDATA fidelity)", "round-trip DOM-compare gate (PartialEq IR equality, not byte-exact)", "canonical IR as sole store — exporter consumes only &DmnModel"]

key-files:
  created:
    - engine/tests/dmn_round_trip.rs
  modified:
    - engine/src/dmn/exporter.rs

key-decisions:
  - "Use quick_xml::writer::Writer with Event::CData/Event::Text directly (not quick_xml::se serde serializer) — serde-XML loses attribute order and CDATA fidelity, same rationale as the parser (Pitfall 11)"
  - "Always emit hitPolicy (default Unique -> 'UNIQUE') so the default is structural on export; the parser defaults absent->Unique, and emitting UNIQUE round-trips because parse('UNIQUE')==Unique"
  - "Emit CDATA when TextContent.is_cdata is true, escaped BytesText otherwise — preserves the CDATA-vs-plain distinction through round-trip (DMN-06)"
  - "Round-trip is structural IR-equality (PartialEq), not byte-exact — the exporter may re-serialize in a different whitespace/attribute-order form as long as parse(export(parse(x)))==parse(x); byte-exact round-trip is Phase 35 (DMN-07)"
  - "textAnnotation.text is always emitted as a <text> child (not the text attribute) — the parser reads both forms into the same TextContent, so the child form round-trips cleanly regardless of source form"
  - "RuleEntry ids on <inputEntry>/<outputEntry>/<annotationEntry> are NOT emitted — RuleEntry has no id field in the IR, so they don't affect round-trip equality"

patterns-established:
  - "Event-based XML emission: BytesStart/BytesEnd/BytesText/BytesCData via Writer::write_event; push_attribute escapes values automatically"
  - "Round-trip DOM-compare pattern: parse -> export -> parse -> assert_eq!(ir1, ir2) on derived PartialEq"
  - "TDD RED/GREEN for exporter: Err-returning stubs + behavior tests (RED), then real implementation (GREEN)"

requirements-completed: [DMN-06]

# Coverage metadata (#1602)
coverage:
  - id: D1
    description: "DMN XML exporter serializing the canonical IR (export_dmn / export_dmn_to_bytes) preserving annotations, descriptions, IDs, CDATA, element order, and DMNDI passthrough"
    requirement: "DMN-06"
    verification:
      - kind: unit
        ref: "engine/src/dmn/exporter.rs#export_dmn_returns_ok_with_definitions"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/exporter.rs#export_dmn_emits_cdata_for_cdata_text"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/exporter.rs#export_dmn_emits_collect_and_aggregation"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/exporter.rs#export_dmn_preserves_element_order"
        status: pass
    human_judgment: false
  - id: D2
    description: "Round-trip DOM-compare gate: parse(export(parse(x))) == parse(x) for every import-corpus fixture (DMN-06), with CDATA + element-order + hitPolicy-default preservation"
    requirement: "DMN-06"
    verification:
      - kind: integration
        ref: "engine/tests/dmn_round_trip.rs#test_round_trip_decision_1_5"
        status: pass
      - kind: integration
        ref: "engine/tests/dmn_round_trip.rs#test_round_trip_decision_1_3"
        status: pass
      - kind: integration
        ref: "engine/tests/dmn_round_trip.rs#test_round_trip_decision_1_1"
        status: pass
      - kind: integration
        ref: "engine/tests/dmn_round_trip.rs#test_round_trip_no_hitpolicy"
        status: pass
      - kind: integration
        ref: "engine/tests/dmn_round_trip.rs#test_round_trip_cdata_description"
        status: pass
      - kind: integration
        ref: "engine/tests/dmn_round_trip.rs#test_round_trip_element_order"
        status: pass
      - kind: integration
        ref: "engine/tests/dmn_round_trip.rs#test_export_produces_valid_xml"
        status: pass
      - kind: integration
        ref: "engine/tests/dmn_round_trip.rs#test_exporter_uses_canonical_ir_only"
        status: pass
    human_judgment: false

# Metrics
duration: 15min
completed: 2026-07-06
status: complete
---

# Phase 29 Plan 03: DMN XML Exporter + Round-Trip Gate Summary

**quick_xml::writer::Writer-based DMN exporter serializing the canonical IR back to .dmn XML with CDATA/element-order/ID preservation, plus an 8-test DOM-compare round-trip gate proving parse(export(parse(x)))==parse(x) for the import corpus (DMN-06)**

## Performance

- **Duration:** 15 min
- **Started:** 2026-07-06T21:08:36Z
- **Completed:** 2026-07-06T21:23:50Z
- **Tasks:** 2 (Task 1 TDD: RED + GREEN; Task 2: round-trip tests)
- **Files modified:** 2 (engine/src/dmn/exporter.rs, engine/tests/dmn_round_trip.rs)

## Accomplishments
- DMN XML exporter (`engine/src/dmn/exporter.rs`) serializing `DmnModel` to `.dmn` XML via `quick_xml::writer::Writer`, preserving annotations, descriptions, IDs, CDATA flags, element order, and DMNDI passthrough
- `export_dmn` / `export_dmn_to_bytes` public API + `DmnExportError` enum (XmlWrite/InvalidModel/Io) with thiserror
- CDATA emission via `Event::CData(BytesCData::new(...))` when `TextContent.is_cdata`, escaped `Event::Text(BytesText::new(...))` otherwise (T-29-05 XML-injection mitigation)
- 8 round-trip DOM-compare tests (`engine/tests/dmn_round_trip.rs`) proving `parse(export(parse(x))) == parse(x)` for all five import-corpus fixtures plus element-order, CDATA, hitPolicy-default, and canonical-IR-only gates (DMN-06)

## Task Commits

Each task was committed atomically (TDD: RED -> GREEN):

1. **Task 1 (RED): Failing exporter behavior tests** - `ddb68ac0e` (test)
2. **Task 1 (GREEN): Implement DMN XML exporter** - `b5f353ae0` (feat)
3. **Task 2: Round-trip DOM-compare tests** - `b11e509de` (test)

_Note: Task 1 was TDD (`tdd="true"`): RED added 4 failing behavior unit tests against Err-returning stubs; GREEN replaced the stubs with the real quick_xml::writer::Writer emitter so all 4 pass._

## Files Created/Modified
- `engine/src/dmn/exporter.rs` - Full DMN XML exporter: export_dmn/export_dmn_to_bytes via quick_xml::writer::Writer; emits definitions/itemDefinition/decision/decisionTable/input/output/rule/inputData/BKM/decisionService/knowledgeSource/textAnnotation/association/annotation + DMNDI passthrough; CDATA-vs-escaped branch on TextContent.is_cdata; HitPolicy::as_str always emitted; element-order-preserving iteration of Definitions::children
- `engine/tests/dmn_round_trip.rs` - 8 round-trip DOM-compare integration tests against the Plan 29-02 import corpus (decision_1_1/1_3/1_5, no_hitpolicy, cdata_description) + element-order/cdata/hitpolicy-default/canonical-IR-only gates

## Decisions Made
- **No serde-XML (`quick_xml::se`):** Used `quick_xml::writer::Writer` with `Event::CData`/`Event::Text`/`BytesStart`/`BytesEnd` directly. Serde-XML loses attribute order and CDATA fidelity (same rationale as the parser in 29-02). Acceptance grep confirms `quick_xml::se` count == 0.
- **Always emit `hitPolicy`:** The default `Unique` is emitted as `hitPolicy="UNIQUE"` so the default is structural on export. This round-trips because `parse("UNIQUE") == Unique`, and the `no_hitpolicy` fixture (which parses to `HitPolicy::Unique` via the absent-attribute default) re-parses to `Unique` after export.
- **CDATA branch on `is_cdata`:** `Event::CData` when `TextContent.is_cdata == true`, `Event::Text` (escaped) otherwise. `BytesText::new` XML-escapes `&`/`<`/`>`/`"` (T-29-05 mitigation). The `cdata_description` fixture round-trips with `is_cdata == true` preserved.
- **Structural (not byte-exact) round-trip:** The gate is `assert_eq!(ir1, ir2)` on derived `PartialEq` of `DmnModel`, not byte equality. The exporter may re-serialize in a different whitespace/attribute-order form; byte-exact round-trip is Phase 35 (DMN-07).
- **`textAnnotation.text` emitted as `<text>` child:** The parser reads both the `text` attribute and a `<text>` child into the same `TextContent`. Always emitting the child form round-trips cleanly regardless of the source form (the `text` attribute is redundant).
- **`RuleEntry` ids not emitted:** `RuleEntry` has no `id` field in the IR, so `<inputEntry id="...">` ids are not re-emitted. They don't affect round-trip equality (the IR doesn't carry them).
- **`isCollection` emitted only when `true`:** The parser defaults `is_collection` to `false` when the attribute is absent, so omitting `isCollection="false"` round-trips cleanly.

## Deviations from Plan

None - plan executed exactly as written. The exporter follows the plan's emission rules (namespace resolution, element order, CDATA branch, hitPolicy/aggregation emission, DMNDI passthrough) and the 8 round-trip tests match the plan's test list one-for-one.

## Issues Encountered
- Two pre-existing engine lib warnings (`unused import: ComparisonOp` in `validation.rs` and `mode_selector.rs`) are out of scope (not caused by this plan's changes) and left untouched per the scope-boundary rule.
- Pre-existing integration test files (`rete_execution.rs`, `coverage.rs`, `engine_execution_tests.rs`) fail to compile because they reference feature-gated modules (`engine::engine::rete`, `engine::engine::treat`) without the corresponding cargo features. These are pre-existing failures unrelated to the DMN module and out of scope; the DMN-specific test targets (`--test dmn_parser`, `--test dmn_round_trip`, `--lib dmn`) all pass cleanly.
- One initial clippy warning (`unnecessary_lazy_evaluations` on the `xmlns` Option chain) was resolved by simplifying to a plain `if/else if/else` expression before the final commit.

## TDD Gate Compliance

Task 1 followed the RED/GREEN cycle:
- **RED** (`ddb68ac0e`): 4 behavior unit tests added against `Err`-returning stubs. `cargo test -p engine --lib dmn::exporter` reported `FAILED. 0 passed; 4 failed` — all 4 failed with the expected stub error (`InvalidModel("exporter not yet implemented (RED stub)")`). No test passed unexpectedly before implementation (fail-fast rule satisfied).
- **GREEN** (`b5f353ae0`): Real `quick_xml::writer::Writer` implementation. `cargo test -p engine --lib dmn::exporter` reported `ok. 4 passed; 0 failed`.
- No REFACTOR commit needed (the implementation was clean; the one clippy nit was folded into the GREEN commit before push).

## User Setup Required

None - no external service configuration required. The exporter is a pure in-process serialization step with no network, DB, or auth dependencies.

## Next Phase Readiness
- The DMN module now has a complete round-trip: parser (29-02) -> IR (29-01) -> exporter (29-03). `parse(export(parse(x))) == parse(x)` is proven for the import corpus.
- Phase 30 (FEEL expression language) can parse the FEEL cell text carried as `String`/`TextContent` on the IR into a `FeelExpression` AST.
- Phase 32 (hit-policy evaluation) can evaluate the `DecisionTable` IR against the `HitPolicy`/`BuiltinAggregator` enums.
- Phase 35 (DMNDI enrichment) can enrich the opaque `DmnDiagram`/`DmnShape`/`DmnEdge` carriers; the exporter already emits them as a `<dmndi:DMNDI>` block (untested by the corpus, structurally complete).
- No blockers.

---
*Phase: 29-dmn-1-5-model-xml-parser-canonical-ir*
*Completed: 2026-07-06*
## Self-Check: PASSED

- `engine/src/dmn/exporter.rs` — FOUND
- `engine/tests/dmn_round_trip.rs` — FOUND
- `.planning/phases/29-dmn-1-5-model-xml-parser-canonical-ir/29-03-SUMMARY.md` — FOUND
- Commit `ddb68ac0e` (test, RED) — FOUND
- Commit `b5f353ae0` (feat, GREEN) — FOUND
- Commit `b11e509de` (test, round-trip) — FOUND
