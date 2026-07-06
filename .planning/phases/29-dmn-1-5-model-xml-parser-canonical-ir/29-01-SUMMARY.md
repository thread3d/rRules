---
phase: 29-dmn-1-5-model-xml-parser-canonical-ir
plan: 01
subsystem: engine/dmn (canonical IR)
tags: [dmn, ir, quick-xml, rust_decimal, parser-stub, model]
requires: []
provides:
  - "engine/src/dmn/{mod,model,parser,exporter}.rs — canonical DMN 1.5 IR + module wiring"
  - "HitPolicy (7 variants) + BuiltinAggregator (4 variants) canonical enums"
  - "DmnParseError — single error-type home for parser.rs (Plan 29-02)"
  - "engine/Cargo.toml deps: quick-xml 0.40 (serialize), rust_decimal 1.42 (serde/macros/maths)"
affects:
  - "29-02-PLAN.md (parser) — parses .dmn XML into this IR"
  - "29-03-PLAN.md (exporter) — emits .dmn XML from this IR"
  - "Phases 30–35 — all parse into or evaluate against this IR"
tech-stack:
  added:
    - "quick-xml 0.40 (serialize feature) — DMN XML NsReader/Writer"
    - "rust_decimal 1.42 (serde/macros/maths features) — FEEL Decimal128 domain (Phase 30+)"
  patterns:
    - "Parallel DMN evaluation surface (engine/src/dmn/) sibling to engine/src/engine/, NOT routed through EngineRegistry"
    - "Canonical IR carries all FEEL text as String/TextContent; Phase 31 parses into a distinct FeelExpression AST"
    - "Round-trip fidelity is structural: TextContent carries is_cdata, Definitions.children is Vec<DefinitionsChild> preserving cross-type document order"
key-files:
  created:
    - engine/src/dmn/mod.rs
    - engine/src/dmn/model.rs
    - engine/src/dmn/parser.rs (stub)
    - engine/src/dmn/exporter.rs (stub)
  modified:
    - engine/Cargo.toml
    - engine/src/lib.rs
decisions:
  - "DMN IR is the sole store; frontend is a projection (milestone research Pitfall 11 — round-trip fidelity structural from day one)"
  - "IR does NOT reuse crate::data_types::Value or the rule-engine Expression AST (milestone research Anti-Pattern avoided)"
  - "No f64 in the FEEL-numeric path; DMNDI geometry (DcBounds/DcPoint) legitimately uses f64 — coordinate data, not FEEL numerics"
  - "HitPolicy::default() = Unique so DecisionTable::default() yields UNIQUE (Phase 29 success criterion 3 at IR level; parser enforces at parse time in 29-02)"
  - "DmnParseError declared in model.rs (single error-type home); parser.rs (29-02) returns Result<_, DmnParseError>"
metrics:
  duration: 4min
  tasks: 3
  files: 7
status: complete
completed_date: 2026-07-06
---

# Phase 29 Plan 01: DMN 1.5 Model + XML Parser + Canonical IR Summary

Established the canonical DMN 1.5 in-memory IR (`engine/src/dmn/model.rs`) and the `dmn` module wiring that every downstream Phase 29 plan and every v6.0 phase parses into or evaluates against; added the locked stack dependencies (`quick-xml 0.40`, `rust_decimal 1.42`) to the engine crate.

## Tasks Completed

| Task | Name | Commit | Key Files |
| ---- | ---- | ------ | --------- |
| 1 | Verify package legitimacy for quick-xml 0.40 and rust_decimal 1.42 | (no code — checkpoint) | none |
| 2 | Add dmn module wiring and locked stack dependencies | 783ce7c4d | engine/Cargo.toml, Cargo.lock, engine/src/lib.rs, engine/src/dmn/{mod,parser,exporter,model}.rs |
| 3 | Define the canonical DMN 1.5 in-memory IR in model.rs | 81a646a69 | engine/src/dmn/model.rs |

## What Was Built

### Task 1 — Supply-chain legitimacy gate (approved out-of-band)

Task 1 is a `checkpoint:human-verify` gate for adding `quick-xml 0.40` and `rust_decimal 1.42`. The human approved both out-of-band before executor spawn, with crates.io evidence:

- **quick-xml** — repo `tafia/quick-xml`, 322M downloads, `0.40.0` published, updated 2026-06-29. Legitimate, actively maintained, not a typosquat.
- **rust_decimal** — repo `paupino/rust-decimal`, 115M downloads, `1.42.0` published, updated 2026-06-12. Legitimate, actively maintained, not a typosquat.

No code written for Task 1 (verification-only gate). Recorded as completed via orchestrator-mediated human approval.

### Task 2 — Module wiring + locked stack deps (783ce7c4d)

- `engine/Cargo.toml`: added `quick-xml = { version = "0.40", features = ["serialize"] }` and `rust_decimal = { version = "1.42", features = ["serde", "macros", "maths"] }`. No `dsntk` dependency (explicitly rejected per research SUMMARY).
- `engine/src/lib.rs`: declared `pub mod dmn;` alongside existing module declarations (parallel DMN evaluation surface, NOT routed through `engine::engine` / `EngineRegistry`).
- `engine/src/dmn/mod.rs`: module doc-comment + `pub mod exporter; pub mod model; pub mod parser;`.
- `engine/src/dmn/parser.rs`, `engine/src/dmn/exporter.rs`: compiling stubs (overwritten by Plans 29-02/29-03).
- `engine/src/dmn/model.rs`: minimal stub created in Task 2 so `cargo build -p engine` passes (Rule 3 — see Deviations); Task 3 overwrote it with the full IR.

### Task 3 — Canonical DMN 1.5 IR (81a646a69)

`engine/src/dmn/model.rs` defines all canonical IR types (names load-bearing — Plans 29-02/29-03 and Phases 30–35 reference them):

- **Root + ordering:** `DmnModel`, `Definitions` (with `children: Vec<DefinitionsChild>` preserving cross-type document order + `diagrams: Vec<DmnDiagram>` for DMNDI), `DefinitionsChild` (8 variants: ItemDefinition / Decision / InputData / BusinessKnowledgeModel / DecisionService / KnowledgeSource / TextAnnotation / Association), `TextContent` (`content` + `is_cdata` for CDATA round-trip).
- **Clause 7 (DMN-05):** `ItemDefinition` (nested `item_components` ordered, `allowed_values`), `InformationItem`.
- **Decisions + DRD:** `Decision` (with `required_decisions` / `required_inputs` / `required_knowledge` edge-ID vectors), `InputData`, `BusinessKnowledgeModel`, `DecisionService`, `KnowledgeSource`, `TextAnnotation`, `Association`.
- **Decision table (Clause 8 shape):** `DecisionTable` (`hit_policy` defaults to `Unique` via `Default` impl — Phase 29 success criterion 3 at IR level), `InputClause`, `OutputClause`, `DmnRule` (ordered input/output/annotation entries), `RuleEntry` (`TextContent` carrier).
- **Hit policy + aggregator enums:** `HitPolicy` (exactly 7 variants: Unique / Any / Priority / First / RuleOrder / OutputOrder / Collect) with `as_str()` returning the DMN attribute strings (`"UNIQUE"`, `"ANY"`, `"PRIORITY"`, `"FIRST"`, `"RULE ORDER"`, `"OUTPUT ORDER"`, `"COLLECT"`); `BuiltinAggregator` (exactly 4 variants: Sum / Min / Max / Count) with `as_str()`. Both derive `PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize` for Phase 32 use.
- **Allowed values + annotations:** `AllowedValues` (`UnaryTests(String)` / `Enumeration(Vec<TextContent>)`) with `as_str_or_items()` debug helper; `Annotation`.
- **DMNDI passthrough (DMN-07 prep):** `DmnDiagram`, `DmnShape`, `DmnEdge`, `DcBounds`, `DcPoint`, `DmnLabel` — opaque structural carriers. `f64` appears ONLY here (DMNDI geometry is coordinate data, not FEEL numerics).
- **Error:** `DmnParseError` (`XmlError` / `InvalidNamespace` / `MissingElement` / `UnexpectedElement` / `XxeRejected` / `Io`) via `thiserror::Error` — single error-type home for `parser.rs` (Plan 29-02).

All structs derive `Debug, Clone, PartialEq, Serialize, Deserialize` (consistent with `rules.rs`); `HitPolicy`/`BuiltinAggregator` additionally derive `Eq, Hash, Copy`. `Default` impls added for `DmnModel`, `Definitions`, `DecisionTable`, `HitPolicy` to reduce downstream boilerplate. Six smoke tests cover default hit policy = UNIQUE, `as_str` round-trip for all HitPolicy/BuiltinAggregator variants, TextContent constructors, AllowedValues debug rendering, and model defaults.

## Verification Results

| Check | Result |
| ----- | ------ |
| `cargo build -p engine` | exits 0 (2 pre-existing warnings: `ComparisonOp` unused import in validation.rs / mode_selector.rs — out of scope) |
| `cargo test -p engine --lib` | 200 passed, 0 failed (6 new `dmn::model::tests::*` + 194 pre-existing) |
| `grep -c 'quick-xml' engine/Cargo.toml` | 1 (≥1) |
| `grep -c 'rust_decimal' engine/Cargo.toml` | 1 (≥1) |
| `grep -c 'dsntk' engine/Cargo.toml` | 0 (NOT added) |
| `grep -c 'pub mod dmn;' engine/src/lib.rs` | 1 |
| `mod.rs` declares `pub mod model; pub mod parser; pub mod exporter;` | yes |
| All 26 required struct/enum names present in model.rs | yes |
| `HitPolicy` variants | exactly 7 (Unique/Any/Priority/First/RuleOrder/OutputOrder/Collect) |
| `BuiltinAggregator` variants | exactly 4 (Sum/Min/Max/Count) |
| `f64` field declarations only in DcBounds/DcPoint | yes (6 f64 field decls, all DMNDI geometry) |
| `use crate::data_types` in model.rs | 0 (IR distinct from rule-engine Value) |
| `use crate::rules` in model.rs | 0 (IR distinct from rule-engine Expression) |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking build issue] Created a minimal `model.rs` stub in Task 2 so `cargo build -p engine` passes**

- **Found during:** Task 2
- **Issue:** Task 2's action specifies `engine/src/dmn/mod.rs` declares `pub mod model; pub mod parser; pub mod exporter;` (three declarations), but Task 2's `<files>` lists only `parser.rs` and `exporter.rs` as stubs (not `model.rs`). With `pub mod model;` declared and `model.rs` absent, `cargo build -p engine` fails with "file not found for module `model`", violating Task 2 acceptance criterion "`cargo build -p engine` exits 0". The plan's `must_haves` truth #1 also requires all three `pub mod` declarations in `mod.rs`, so omitting `pub mod model;` from Task 2 was not an option.
- **Fix:** Created a minimal `model.rs` stub in Task 2 (file-level doc-comment `//! DMN 1.5 canonical in-memory IR — implemented in Plan 29-01 Task 3.`) so the build passes. Task 3 then overwrote it with the full canonical IR. This mirrors the established stub pattern already used for `parser.rs` and `exporter.rs` in Task 2.
- **Files modified:** `engine/src/dmn/model.rs` (stub in Task 2, full IR in Task 3)
- **Commit:** 783ce7c4d (Task 2 stub), 81a646a69 (Task 3 full IR)

No other deviations. The plan executed exactly as written otherwise.

## Authentication Gates

None.

## Known Stubs

| File | Line | Reason | Resolved By |
| ---- | ---- | ------ | ----------- |
| `engine/src/dmn/parser.rs` | whole file | `//! DMN XML parser — implemented in Plan 29-02.` (compiling stub) | Plan 29-02 (Wave 2) |
| `engine/src/dmn/exporter.rs` | whole file | `//! DMN XML exporter — implemented in Plan 29-03.` (compiling stub) | Plan 29-03 (Wave 3) |

These stubs are intentional per the plan: they exist so `mod.rs` declarations compile in Wave 1; Plans 29-02 and 29-03 overwrite them with full implementations. No stubs block this plan's goal (DMN-05 "represented in the canonical IR" half — the IR itself is fully implemented in `model.rs`).

## Threat Flags

None. The supply-chain threat (T-29-SC) was mitigated by the Task 1 checkpoint (approved out-of-band with crates.io evidence). The XXE/entity threats (T-29-01, T-29-02) are mitigated in `parser.rs` (Plan 29-02), not in this plan's `model.rs`; the IR carries no entity references and `DmnParseError::XxeRejected` is declared here for the parser to use. No new security-relevant surface beyond what the plan's `<threat_model>` documents was introduced by this plan's files.

## TDD Gate Compliance

N/A — this plan is `type: execute` (not `type: tdd`); no RED/GREEN/REFACTOR gate applies. Six smoke tests were added in Task 3 (`dmn::model::tests::*`) covering default hit policy, `as_str` round-trip, TextContent constructors, AllowedValues debug rendering, and model defaults. All pass.

## Self-Check: PASSED

- `engine/src/dmn/mod.rs` — FOUND
- `engine/src/dmn/model.rs` — FOUND
- `engine/src/dmn/parser.rs` — FOUND (stub)
- `engine/src/dmn/exporter.rs` — FOUND (stub)
- Commit 783ce7c4d (Task 2) — FOUND in `git log --oneline --all`
- Commit 81a646a69 (Task 3) — FOUND in `git log --oneline --all`