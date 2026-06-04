# Phase 7: Core Testing Infrastructure Foundation - Context

**Gathered:** 2026-06-04  
**Status:** Ready for planning (auto-updated)

## Phase Boundary

Establish a comprehensive testing infrastructure foundation for the rRules rule engine (v2.0 milestone). This includes Rust testing stack integration (criterion, mockall, serial_test), working memory fixture support with per-test engine isolation, rule-by-rule unit test framework with assertions, forward chaining verification with activation sequence tracking, InferenceTrace API exposure for backward chaining debugging, and deterministic firing mode to eliminate flaky tests.

## Domain

**What this phase delivers:** A production-grade testing infrastructure that enables developers to write reliable, isolated, and verifiable tests for business rules. This includes test fixtures, engine isolation, trace capture, and deterministic execution modes.

**Carrying forward from Phase 1 (Core Engine):**
- D-01: Hybrid module organization — Keep this pattern for test structure
- D-02: Hybrid data structures (typed + dynamic) — Tests should verify both modes
- D-04/D-05: Automatic chaining mode detection, fully mixed execution — Tests must cover both forward and backward chaining paths
- D-06: Backward chaining for interactive debugging — Trace capture is a testing requirement
- D-07/D-08/D-09: Explicit session lifecycle — Tests need isolated session management

**[auto] Gray Areas Selected:** All 4 gray areas from existing context will be auto-resolved:
1. Testing Stack Integration
2. Fixture API Design
3. Forward Chaining Verification
4. Deterministic Mode Configuration

## Requirements (locked by Phase 7 scope)

From ROADMAP.md success criteria:
1. Tests run in isolation with fresh engine instances per test (no state pollution)
2. Working memory fixtures can be created and managed with reproducible state
3. Forward chaining tests verify rule firing sequence respects salience ordering
4. Backward chaining tests capture inference traces for debugging
5. Deterministic mode eliminates flaky tests by enforcing consistent firing order

From REQUIREMENTS.md (v2.0):
- TEST-01: Working Memory Fixture Support (P1)
- TEST-02: Rule-by-Rule Unit Tests (P1)
- TEST-03: Forward Chaining Verification (P1)
- TEST-04: Backward Chaining Trace Capture (P1)
- TEST-05: Deterministic Firing Mode (P1)

## Gray Areas Identified

### Testing Stack Integration (Gray Area 1)

**[auto] Selected:** Integrate criterion for benchmarking, mockall for mocking, serial_test for parallel isolation.

**Questions auto-resolved:**
- Which Rust testing crates should be integrated beyond the standard `#[test]` macros? → criterion, mockall, serial_test
- Should we use `criterion` for benchmarks only, or also for property-based testing? → Benchmarks only; property testing via proptest if needed
- How should test parallelization be handled? (serial_test vs std::thread isolation) → serial_test for full isolation per test module

**Implementation Impact:**
- Choice affects Cargo.toml dependencies and CI configuration
- Influences how tests are organized (modular vs nested)
- Determines available assertions and mocking capabilities

### Fixture API Design (Gray Area 2)

**[auto] Selected:** FixtureBuilder returns TestSession wrapper with WorkingMemory access; expose `with_fact` and `with_facts_from_json` methods.

**Questions auto-resolved:**
- Should `FixtureBuilder` return a `WorkingMemory` or a `TestSession`? → TestSession wrapper for session isolation
- What methods should be exposed for common test patterns? → `with_fact()`, `with_facts_from_json()`, `build()` returns WorkingMemory
- How should fixtures be persisted/cached between test runs? → JSON snapshots via WorkingMemory serialization (Phase 8)

**Implementation Impact:**
- Affects ergonomics of writing tests
- Determines how test data is loaded and cleaned up
- May influence JSON snapshot format design

### Forward Chaining Verification (Gray Area 3)

**[auto] Selected:** Activation tracking automatic with opt-in trace capture; capture full rule names in traces; use assertion methods for sequence verification.

**Questions auto-resolved:**
- Should activation tracking be automatic or opt-in? → Automatic, with optional detailed tracing
- What level of detail should firing traces capture? (rule name only vs full state snapshot) → Rule names + timestamps for performance
- How should tests assert on expected firing sequences? (DSL vs assertion methods) → Assertion methods in testing module

**Implementation Impact:**
- Performance considerations for trace collection
- API ergonomics for common verification patterns
- Integration with existing salience/agenda infrastructure

### Deterministic Mode Configuration (Gray Area 4)

**[auto] Selected:** Per-test configuration via TestSession builder; always use rule name tiebreaker in test mode; log actual firing order.

**Questions auto-resolved:**
- Should deterministic mode be per-test or global configuration? → Per-test via TestSession builder
- How should the "tiebreaker" (rule name sorting) be configured? (always vs opt-in) → Always in test mode, opt-out via cfg!
- What metrics/metrics should be logged in deterministic mode? → Firing order sequence for debugging

**Implementation Impact:**
- Affects EngineBuilder API surface
- May require runtime flag in ConflictResolver
- Influences test reporting format

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-Level
- `.planning/PROJECT.md` — Core value, constraints (Rust backend), key decisions

### Phase-Specific
- `.planning/phases/07-core-testing-infrastructure-foundation-v2-0/07-CONTEXT.md` — This file

### Roadmap
- `.planning/ROADMAP.md` — Phase 7 details, success criteria, and plan breakdown

### Requirements
- `.planning/REQUIREMENTS.md` — TEST-01 through TEST-05 for v2.0 milestone

## Existing Code Insights

### Reusable Assets
- `engine/src/testing/mod.rs` — Testing framework with TestSession, FixtureBuilder, ForwardChainingTester, BackwardChainingTrace structures (fully implemented, not stubbed)
- `engine/src/environment.rs` — EngineBuilder and Session management
- `engine/src/fact.rs` — WorkingMemory implementation with insert_fact, retract_fact, get_facts_by_type
- `engine/src/salience/conflict_resolution.rs` — ConflictResolver with strategy selection

### Established Patterns
- Rust module organization with lib.rs exports
- Builder pattern for configuration (EngineBuilder)
- RAII-style cleanup with Session.destroy()
- Uuid-based fact IDs for uniqueness
- Error handling via thiserror derive macros
- TestResult type alias with custom TestError enum

### Integration Points
Phase 7 must integrate with:
- `engine/src/forward_chaining.rs` — Add activation tracking to execute_forward
- `engine/src/backward_chaining.rs` — Expose InferenceTrace via public API (may need visibility changes)
- `engine/src/salience/agenda.rs` — Add deterministic sorting option to agenda processing
- `Cargo.toml` workspace dependencies — Add criterion, mockall, serial_test

### Key Limitations (to fix in Phase 7)
- Testing module exists but needs integration with actual execution methods (execute_forward/execute_backward are placeholders)
- No working memory JSON serialization yet (needed for snapshot testing - Phase 8)
- InferenceTrace exists but may need visibility adjustments for public API access
- FixtureBuilder is partially stubbed (with_fact doesn't actually add facts)

## Deferred Ideas

### General
- Gherkin/BDD integration (TEST-09, TEST-10) — Deferred to Phase 9/v2.1
- HTML coverage report generation — Deferred to Phase 8 advanced features

---

**[auto] Decisions Summary:**
- Testing stack: criterion + mockall + serial_test
- Fixture API: Returns TestSession with WorkingMemory access
- Forward chaining: Automatic activation tracking, rule-name traces
- Deterministic mode: Per-test with always-on tiebreaker

---
*Phase: 07 - Core Testing Infrastructure Foundation*
*Context gathered: 2026-06-04 (auto-updated)*
