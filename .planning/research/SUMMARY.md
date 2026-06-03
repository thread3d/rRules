# Research Summary: rRules Testing Framework (v2.0 Milestone)

**Project:** rRules - Business Rules Management System (BRMS)
**Domain:** Rust-based rule engine with Rete algorithm, forward/backward chaining inference
**Researched:** 2026-06-03
**Confidence:** HIGH

## Executive Summary

This research focuses on building a comprehensive testing framework for rRules v2.0 milestone. The rule engine uses the Rete algorithm for efficient pattern matching with both forward and backward chaining inference modes.

The recommended approach prioritizes:
1. **Core Rust stack first**: criterion (benchmarking) + mockall (mocking) + serial_test (test isolation)
2. **BDD/Gherkin later**: cucumber-rs adds complexity best deferred until engine stability
3. **Strict state isolation**: per-test engine instances with RAII-based cleanup to prevent test pollution

Critical pitfalls to address: infinite loops (max 1000 firings), state pollution between tests, non-deterministic rule firing order, and fact lifecycle management.

## Key Findings

### Recommended Stack

**Core technologies:**
- `criterion` (0.5-0.8.x) — Micro-benchmarking — Industry standard for Rust benchmarks
- `mockall` (0.12-0.13.x) — Mocking/duck-typing — Essential for isolating rule engine components
- `serial_test` (2-3.x) — Parallel test isolation — Critical for integration tests that share state

**BDD/Gherkin:**
- `gherkin` (15.x) — Gherkin parser — Most actively maintained, pure Rust
- `cucumber-rs` (0.2-0.3.x) — BDD test runner — Official Cucumber implementation for Rust
- `spec-rs` (0.6-0.7.x) — Spec-style testing — Good for rule execution scenarios

**Integration:**
- `assert_cmd` (2.x) — CLI integration tests — Perfect for `rulerun` command testing
- `reqwest` + `wiremock` — HTTP API testing — Mock REST endpoints for rule execution
- `tempfile` (3.10-3.12.x) — Test file cleanup — Safe temporary directory creation

### Expected Features

**Must have (table stakes):**
- Rule-by-Rule Unit Tests — Developers need to test individual rules in isolation
- Working Memory Fixture Support — Create fact collections as test fixtures
- Forward Chaining Verification — Verify rule firing sequence respects salience ordering
- Backward Chaining Trace Capture — Capture reasoning path for debugging
- Salience/Priority Order Testing — Rules fire in priority order when multiple activated

**Should have (competitive):**
- Test-Driven Rule Development — TDD workflow with fast feedback loop
- Firing Order Visualization — Visual display of firing sequence helps debugging
- Snapshot-based Testing — Save/load working memory state as JSON baseline

### Critical Pitfalls

1. **Infinite Loop Detection in Rule Firing** — Rules can fire indefinitely when they re-match their own conditions; implement max 1000 firings limit and history tracking
2. **State Isolation Between Tests** — Tests pollute each other's working memory; use per-test engine instances with unique session IDs
3. **Non-Deterministic Rule Firing** — Equal-salience rules may fire in different order; implement deterministic conflict resolution (sort by name as tiebreaker)
4. **Test Data Management and Cleanup** — Facts accumulate without proper retraction; use RAII patterns for fact lifecycle management

## Implications for Roadmap

### Phase 1: Core Engine Implementation (v1.0) - COMPLETED
The v1.0 milestone has stub implementations. This research informs that testing infrastructure must parallel engine development.

### Phase 2: Testing Infrastructure Foundation (NEW - for v2.0)
**Rationale:** Without proper testing, we cannot verify the core engine works correctly; criterion + mockall are essential prerequisites for any meaningful testing
**Delivers:** Test harness setup, working memory fixtures, basic assertion framework
**Addresses:** Table stakes from FEATURES.md (unit tests, fixture support)
**Avoids:** State isolation pitfalls using per-test engine instances

### Phase 3: Rule Engine Testing Framework (NEW - for v2.0)
**Rationale:** With basic infrastructure in place, add comprehensive testing capabilities; this is where we verify forward/backward chaining actually works
**Delivers:** Forward chaining verification, backward chaining traces, salience order tests, no-loop enforcement tests
**Uses:** criterion benchmarks, mockall isolation patterns
**Implements:** Deterministic firing mode to avoid Pitfall #3

### Phase 4: BDD/Gherkin Integration (Deferred from v2.0)
**Rationale:** cucumber-rs adds complexity best deferred until engine stability is established; the async integration patterns need more research
**Delivers:** Gherkin feature files with Given/When/Then syntax for business-readable tests
**Research Flags:** Async integration patterns with cucumber-rs 0.3.x

### Phase Ordering Rationale

1. Core engine must exist before testing (v1.0 foundation)
2. Testing infrastructure (criterion, mockall) enables all other testing
3. Rule engine testing framework verifies actual functionality
4. BDD/Gherkin is a user-facing feature that can wait for stability

### Research Flags

**Phases likely needing deeper research during planning:**
- **Phase 4:** cucumber-rs async integration patterns — needs investigation

**Phases with standard patterns (skip research-phase):**
- **Phase 2:** Core Rust stack is well-documented and mature
- **Phase 3:** Rule engine testing follows established patterns from Drools/CLIPS

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | criterion, mockall are mature with active maintenance |
| Features | HIGH | Rule engine testing patterns well-established in expert systems |
| Architecture | MEDIUM | Missing ARCHITECTURE.md; some gaps in API exposure design |
| Pitfalls | HIGH | Common issues documented across multiple sources |

**Overall confidence:** HIGH

### Gaps to Address

- **InferenceTrace API exposure** — Need to verify the existing InferenceTrace structure is accessible for testing
- **Execution instrumentation points** — May need engine modifications to track coverage data
- **Deterministic mode implementation** — Conflict resolution strategy may require salience-aware sorting

## Sources

### Primary (HIGH confidence)
- [criterion.rs documentation](https://bheisler.github.io/criterion.rs/book/) — Statistics-driven micro-benchmarking
- [Mockall docs.rs](https://docs.rs/mockall/latest/mockall/) — Mocking for Rust with async support
- [Gherkin crate docs](https://docs.rs/gherkin/) — Gherkin v15 parser, pure Rust

### Secondary (MEDIUM confidence)
- [cucumber-rs GitHub](https://github.com/cucumber-rs/cucumber-rs) — Official Cucumber implementation for Rust
- Drools RuleUnit testing framework — Industry pattern for rule unit testing
- CLIPS expert system testing patterns — Forward chaining with working memory facts

---
*Research completed: 2026-06-03*
*Ready for roadmap: yes*
