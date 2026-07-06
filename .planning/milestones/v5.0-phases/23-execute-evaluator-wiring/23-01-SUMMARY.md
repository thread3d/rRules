---
phase: 23-execute-evaluator-wiring
plan: 01
subsystem: engine
tags: [rust, cargo-features, rete, treat, leaps, phreak, trait-extensions, registry]

# Dependency graph
requires: []
provides:
  - "ExecutionEngine::load_ruleset(&mut self, &RuleSet) -> Result<(), String> default-impl trait method (the RuleSet -> engine bridge, L-02 Option (b))"
  - "ExecutionEngine::get_facts(&self) -> Vec<Fact> default-impl trait method (post-execution working memory exposure for facts_after, Pitfall 11 fix)"
  - "EngineResult.fired_rule_ids: Vec<String> field (rules_not_fired derivation source, RESEARCH Open Q#5)"
  - "EngineRegistry::register_default_engines registers all four factories (Rete/Treat/Leaps/Phreak) under their #[cfg(feature = engine-X)] gates (D-01 foundation)"
  - "registry.rs re-exports TreatEngineFactory, LeapsEngineFactory, PhreakEngineFactory under their cfg feature gates"
  - "api crate links all four engine factories via api/Cargo.toml features = [engine-rete, engine-treat, engine-leaps, engine-phreak] (L-01)"
  - "Compiling pluggable-engine surface (E0433 import bug fixed in rete/treat/leaps/phreak mod.rs)"
affects: [23-03, 23-04, 23-05, 23-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "crate-relative import path (crate::monitoring::coverage::CoverageTracking) inside the engine crate — 2021 edition resolves crate::, not the absolute engine:: path"
    - "Trait default-method idiom for extending ExecutionEngine without breaking existing impls (load_ruleset returns Err, get_facts returns Vec::new by default)"
    - "Static #[cfg(feature = engine-X)] factory registration + re-export — no runtime/user input selects which factories register (T-23-01 mitigation)"

key-files:
  created: []
  modified:
    - "engine/src/engine/registry.rs — ExecutionEngine trait (load_ruleset/get_facts default methods), EngineResult (fired_rule_ids field + constructor init), register_default_engines (4 cfg-gated factory registrations), 3 new cfg-gated re-exports, new all-four-features unit test"
    - "engine/src/engine/rete/mod.rs — line 13 import fix (engine:: -> crate::)"
    - "engine/src/engine/treat/mod.rs — line 12 import fix (engine:: -> crate::)"
    - "engine/src/engine/leaps/mod.rs — line 13 import fix (engine:: -> crate::)"
    - "engine/src/engine/phreak/mod.rs — line 12 import fix (engine:: -> crate::)"
    - "api/Cargo.toml — engine dependency enables engine-rete, engine-treat, engine-leaps, engine-phreak"

key-decisions:
  - "Extended EngineResult with a pub fired_rule_ids: Vec<String> field (RESEARCH Open Q#5 resolution) rather than adding a trait method — single return value, no extra trait method, engines populate via field mutation before returning."
  - "Default impls return Err/empty for load_ruleset/get_facts so the trait extension is non-breaking; engines override in later plans (Rete in Plan 03, others in Plan 06)."
  - "Registered all four factories under static cfg gates rather than Rete-only — available_engines() is truthful for the engine param validation in Plan 05 (D-01 foundation)."

patterns-established:
  - "Pattern: crate-relative imports inside the engine crate (never absolute engine:: paths) — fixes the E0433 class of bug across all four engine modules."
  - "Pattern: cfg-gated factory registration + parallel cfg-gated re-exports so register_default_engines can name the factory types."
  - "Pattern: trait default methods returning Err/empty as the 'not yet implemented' contract for new bridge methods — engines opt in by overriding."

requirements-completed: [ENGINE-07, ENGINE-08]

# Coverage metadata (#1602) — per-deliverable traceability.
# Note: ENGINE-07/ENGINE-08 full behavioral tests (engine: "unknown" -> 400,
# omitting engine -> config default) land in Plan 05. This plan delivers the
# registry foundation (available_engines() truthfully returns all four names)
# that Plan 05's validation checks against. The deliverables below are the
# foundational pieces fully proven by automation in this plan.
coverage:
  - id: D1
    description: "E0433 import bug fixed in all four engine modules (rete/treat/leaps/phreak use crate::monitoring::coverage::CoverageTracking)"
    requirement: "ENGINE-07"
    verification:
      - kind: unit
        ref: "cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak (exits 0; was 4x E0433 before fix)"
        status: pass
    human_judgment: false
  - id: D2
    description: "ExecutionEngine trait extended with load_ruleset and get_facts default methods (L-02 bridge contract)"
    requirement: "ENGINE-07"
    verification:
      - kind: unit
        ref: "engine/src/engine/registry.rs:97 (load_ruleset), :105 (get_facts); cargo check -p engine --features all four exits 0"
        status: pass
    human_judgment: false
  - id: D3
    description: "EngineResult extended with pub fired_rule_ids: Vec<String> field, initialized empty in both success and failure constructors"
    requirement: "ENGINE-07"
    verification:
      - kind: unit
        ref: "grep -c fired_rule_ids engine/src/engine/registry.rs == 3 (field decl + 2 constructor inits); cargo check exits 0"
        status: pass
    human_judgment: false
  - id: D4
    description: "register_default_engines registers all four factories (Rete/Treat/Leaps/Phreak) under their #[cfg(feature = engine-X)] gates; available_engines() returns all four names"
    requirement: "ENGINE-07"
    verification:
      - kind: unit
        ref: "engine/src/engine/registry.rs::tests::test_register_default_engines_advertises_all_four (passes with all four features)"
        status: pass
    human_judgment: false
  - id: D5
    description: "registry.rs re-exports TreatEngineFactory, LeapsEngineFactory, PhreakEngineFactory under their cfg feature gates"
    requirement: "ENGINE-07"
    verification:
      - kind: unit
        ref: "engine/src/engine/registry.rs:262-266 (3 pub use re-exports); cargo check -p engine --features all four exits 0"
        status: pass
    human_judgment: false
  - id: D6
    description: "api/Cargo.toml engine dependency enables engine-rete, engine-treat, engine-leaps, engine-phreak; api crate links all four factories"
    requirement: "ENGINE-08"
    verification:
      - kind: unit
        ref: "cargo check -p api exits 0 (api crate compiles with all four engine features)"
        status: pass
    human_judgment: false

# Metrics
duration: 4min
completed: 2026-07-03
status: complete
---

# Phase 23 Plan 01: Pluggable-engine surface compilation foundation Summary

**Fixed the E0433 import bug across all four engine modules, extended ExecutionEngine with load_ruleset/get_facts default methods + EngineResult.fired_rule_ids, and registered all four factories so available_engines() truthfully returns rete/treat/leaps/phreak — the pluggable-engine surface now compiles for the first time and the api crate links all four factories.**

## Performance

- **Duration:** 4 min (3m37s)
- **Started:** 2026-07-03T06:13:29Z
- **Completed:** 2026-07-03T06:17:06Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Fixed the E0433 `use of unresolved module or unlinked crate 'engine'` compilation blocker in all four engine `mod.rs` files by replacing the absolute `engine::monitoring::coverage::CoverageTracking` path with the crate-relative `crate::monitoring::coverage::CoverageTracking` path. The entire pluggable-engine surface was dead code that had never compiled with features on; `cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` now exits 0.
- Extended the `ExecutionEngine` trait with two default methods — `load_ruleset(&mut self, &RuleSet) -> Result<(), String>` (default `Err`) and `get_facts(&self) -> Vec<Fact>` (default `Vec::new()`) — establishing the `RuleSet -> engine` bridge contract (L-02 Option (b)) that downstream plans implement on each engine.
- Added `pub fired_rule_ids: Vec<String>` to `EngineResult` (initialized empty in both constructors) so the API layer can derive `rules_not_fired` as the ruleset ids minus the fired set (RESEARCH Open Q#5 resolution).
- Extended `register_default_engines` to register `TreatEngineFactory`, `LeapsEngineFactory`, and `PhreakEngineFactory` under their `#[cfg(feature = "engine-X")]` gates alongside the existing `ReteEngineFactory`, and added the three parallel cfg-gated re-exports so the factory types are nameable. A new unit test proves `available_engines()` returns all four names when all four features are enabled (D-01 foundation, T-23-01 mitigation).
- Enabled `engine-rete`/`engine-treat`/`engine-leaps`/`engine-phreak` on the api crate's engine dependency so the registry is fully populated at startup; `cargo check -p api` exits 0.

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix the E0433 import bug in all four engine modules and extend the ExecutionEngine trait + EngineResult** - `1e072624a` (fix)
2. **Task 2: Register all four factories in register_default_engines + extend re-exports + enable features on the api crate** - `e883a7383` (feat)

**Plan metadata:** pending (docs: complete plan — committed after STATE.md/ROADMAP.md updates)

## Files Created/Modified
- `engine/src/engine/registry.rs` - ExecutionEngine trait extended with load_ruleset/get_facts default methods; EngineResult extended with fired_rule_ids field + constructor initialization; register_default_engines extended with 3 cfg-gated factory registrations; 3 new cfg-gated re-exports; new all-four-features unit test for available_engines()
- `engine/src/engine/rete/mod.rs` - line 13 import fix: `engine::` -> `crate::` for CoverageTracking
- `engine/src/engine/treat/mod.rs` - line 12 import fix: `engine::` -> `crate::` for CoverageTracking
- `engine/src/engine/leaps/mod.rs` - line 13 import fix: `engine::` -> `crate::` for CoverageTracking
- `engine/src/engine/phreak/mod.rs` - line 12 import fix: `engine::` -> `crate::` for CoverageTracking
- `api/Cargo.toml` - engine dependency enables engine-rete, engine-treat, engine-leaps, engine-phreak

## Decisions Made
- **Extended `EngineResult` with a `pub fired_rule_ids: Vec<String>` field** (RESEARCH Open Q#5) rather than adding a `fired_rule_ids(&self)` trait method. Rationale: a single return value carries the fired ids alongside the count, no extra trait method needed, and engines populate the field via mutation before returning the `EngineResult` by value.
- **Default impls return `Err`/empty for `load_ruleset`/`get_facts`** so the trait extension is non-breaking for existing impls. Engines override in later plans: `ReteEngine` in Plan 03, the other three in Plan 06.
- **Registered all four factories under static cfg gates** (D-01 foundation) rather than Rete-only. `available_engines()` is now truthful, which Plan 05's per-request `engine` param validation depends on. No runtime/user input selects which factories register (T-23-01 mitigation).

## Deviations from Plan

None - plan executed exactly as written. Both tasks followed the plan's actions verbatim; all acceptance criteria passed on the first attempt.

## Issues Encountered
None. The `cargo test` substring filter for the new unit test required the module-path form `registry::tests` rather than the bare test name (the `--exact` flag matches against the full path `engine::registry::tests::test_register_default_engines_advertises_all_four`); resolved by using the module-substring filter `registry::tests` which runs all four tests in the module including the new one.

## User Setup Required
None - no external service configuration required. No new packages were installed (T-23-SC: only existing in-tree cargo features were enabled).

## Next Phase Readiness
- The pluggable-engine surface now compiles with all four features enabled. Downstream plans (23-03, 23-04, 23-05, 23-06) can build on this foundation:
  - **Plan 03** implements `load_ruleset`/`get_facts` on `ReteEngine` and rewrites `execute_rules` into a real direct evaluator.
  - **Plan 04** builds the structured `Expression` evaluator (D-06) and full `ComparisonOp`/`Pattern` matching (D-05) on `rules.rs`.
  - **Plan 05** wires `POST /v1/execute` through the registry, validates the `engine` param against `available_engines()` (ENGINE-07/08 behavioral tests), and serializes `facts_after` from `get_facts()`.
  - **Plan 06** implements `load_ruleset`/`get_facts` + the direct evaluator on `TreatEngine`/`LeapsEngine`/`PhreakEngine` (D-01/D-02).
- No blockers or concerns. The trait contract (`load_ruleset`/`get_facts` default impls) and the `fired_rule_ids` field are stable for downstream plans to implement against.

---
*Phase: 23-execute-evaluator-wiring*
*Completed: 2026-07-03*

## Self-Check: PASSED

- All 6 modified files exist on disk: FOUND
- Task 1 commit `1e072624a`: FOUND
- Task 2 commit `e883a7383`: FOUND
- `cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` exits 0
- `cargo check -p api` exits 0
- `cargo test -p engine --lib --features engine-rete,engine-treat,engine-leaps,engine-phreak registry::tests` exits 0 (4/4 pass, including the new `test_register_default_engines_advertises_all_four`)