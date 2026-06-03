# Features Research

**Domain:** Business Rules Engine Testing (Forward/Backward Chaining, Rete Algorithm)
**Researched:** 2026-06-03
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Rule-by-Rule Unit Tests** | Developers need to test individual rules in isolation before integrating into rule sets | LOW | Basic assertion framework for single rule behavior |
| **Working Memory Fixture Support** | Tests must be able to load specific facts, then verify which rules fire | MEDIUM | Helpers for creating Fact collections as test fixtures |
| **Forward Chaining Verification** | Must verify that asserted facts trigger expected subsequent firings in correct order | HIGH | Requires tracking activation/firing sequence with salience ordering |
| **Backward Chaining Trace Capture** | For query-based inference, must capture reasoning path for debugging and verification | MEDIUM | InferenceTrace structure showing rule applications and fact matches |
| **Salience/Priority Order Testing** | Rules should fire in priority order when multiple are activated; missing this causes flaky tests | HIGH | Must verify firing sequence respects salience values and conflict resolution strategy |
| **Fact Assertion/Retraction Verification** | Tests need to verify facts were added/removed as side effects of rule execution | LOW | After executing rules, check working memory state |
| **No-Loop / Lock-on-Active Enforcement** | These flags prevent infinite loops; tests should verify they work correctly | MEDIUM | Verify rule doesn't re-fire after no_loop=true or lock_on_active=true |
| **Agenda State Inspection** | Tests should inspect the current agenda (pending activations) to verify conflict resolution | MEDIUM | Peek at Agenda state before execution completes |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Test-Driven Rule Development** | Write tests first, then implement rules; tests serve as executable specification | MEDIUM | TDD workflow with fast feedback loop |
| **Firing Order Visualization** | Visual display of firing sequence helps debugging complex rule flows | HIGH | Timeline or dependency graph of rule executions |
| **Rule Coverage Metrics** | Track which conditions/paths were exercised during test runs | MEDIUM | Coverage data for rule condition branches |
| **Snapshot-based Testing** | Save working memory state as JSON snapshot, use as test baseline | LOW | `#[test_snapshot("testdata/initial_facts.json")]` attribute |
| **Mock Fact Proxies** | Fake facts that record how rules accessed them (for verification) | MEDIUM | `FactMock::track_access()` for asserting fact usage patterns |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Full Rete Network Inspection** | Users want to verify internal network structure | HIGH complexity, unstable API | Test behavior via inputs/outputs instead of internals |
| **Real-Time Firing Observation** | See rules fire as they happen during test execution | Creates tight coupling between test and engine | Capture post-execution trace instead |
| **Concurrent Rule Execution Testing** | Test rules in parallel for performance | Non-deterministic, hard to debug | Test sequential behavior first; add concurrent tests separately |

## Feature Dependencies

```
[Working Memory Fixture Support]
    └──requires──> [Fact Creation Helpers]
                       └──requires──> [Data Type Definitions]

[Forward Chaining Verification]
    ├──requires──> [Agenda State Inspection]
    └──enhances──> [Salience/Priority Order Testing]

[Backward Chaining Trace Capture]
    └──requires──> [InferenceTrace Data Structure]

[Rule Coverage Metrics]
    └──requires──> [Execution Instrumentation Point]
```

### Dependency Notes

- **Working Memory Fixture Support requires Fact Creation Helpers:** Tests need easy ways to create fact collections; this depends on having stable `Fact` struct with `with_id()` for reproducible tests.

- **Forward Chaining Verification requires Agenda State Inspection:** To verify firing order, must be able to inspect the agenda before execution completes.

- **Backward Chaining Trace Capture requires InferenceTrace Data Structure:** Already implemented in `backward_chaining.rs`; must expose via public API for testing.

- **Rule Coverage Metrics requires Execution Instrumentation Point:** Need hooks at rule condition evaluation and action execution; would require modifying engine to track coverage.

## MVP Definition

### Launch With (v1)

Minimum viable product — what's needed to validate the concept.

- [ ] **Rule-by-Rule Unit Tests** — Test individual rules in isolation with known facts; verify expected actions execute
- [ ] **Working Memory Fixture Support** — Create fact collections as test fixtures; insert into working memory before rule execution
- [ ] **Fact Assertion/Retraction Verification** — After rule execution, inspect working memory for expected state
- [ ] **Basic Salience Testing** — Verify higher salience rules fire before lower ones (via agenda inspection)

### Add After Validation (v1.x)

Features to add once core is working.

- [ ] **Forward Chaining Trace Capture** — Record complete sequence of rule firings with timestamps and matched facts
- [ ] **No-Loop Verification Tests** — Verify rules with `no_loop=true` don't re-fire after their actions execute
- [ ] **Test Snapshot System** — Save/load working memory state as JSON for reproducible tests

### Future Consideration (v2+)

Features to defer until product-market fit is established.

- [ ] **Rule Coverage Metrics** — Track which conditions were evaluated during test runs
- [ ] **Firing Order Visualization** — Visual timeline of rule executions
- [ ] **Mock Fact Proxies** — Fake facts that record how rules accessed them

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Rule-by-Rule Unit Tests | HIGH | LOW | P1 |
| Working Memory Fixture Support | HIGH | MEDIUM | P1 |
| Fact Assertion/Retraction Verification | HIGH | LOW | P1 |
| Basic Salience Testing | HIGH | HIGH | P1 |
| No-Loop Enforcement Tests | MEDIUM | MEDIUM | P2 |
| Forward Chaining Trace Capture | MEDIUM | HIGH | P2 |
| Backward Chaining Trace Capture | MEDIUM | MEDIUM | P2 |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | Drools (KIE) | CLIPS | Our Approach |
|---------|--------------|-------|--------------|
| **Rule Unit Tests** | `@RuleUnit` with test runner | Manual testing framework | Built-in Rust `#[test]` macros |
| **Working Memory Fixtures** | Stateful sessions, insert facts | `(assert ...)` function | `WorkingMemory::insert_fact()` |
| **Firing Order Verification** | Agenda inspection via session | Trace logging | Agenda API + execution trace capture |
| **Forward Chaining Tests** | Executable rule units | Run with facts, check results | `execute_forward()` wrapper with assertions |
| **Backward Chaining Tests** | Query execution with traces | `trace-on` mode | `InferenceTrace` with `explain()` method |

## Sources

- Apache KIE/Drools documentation - RuleUnit testing framework
- CLIPS expert system - Forward chaining with working memory facts
- Rust testing book (doc.rust-lang.org) - Unit/integration test patterns
- rRules engine source code - WorkingMemory, Agenda, Activation structures

---
*Feature research for: Business Rules Engine Testing*
*Researched: 2026-06-03*
