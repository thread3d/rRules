# Phase 7: Core Testing Infrastructure Foundation - Research

**Date:** 2026-06-04  
**Status:** Completed

## Phase Overview

Phase 7 establishes a comprehensive testing infrastructure foundation for the rRules rule engine (v2.0 milestone). This research covers how to plan this phase effectively given existing code and requirements.

---

## Current State Analysis

### Project Position
- **Milestone:** v2.0 - Testing Framework
- **Previous phases:** Phase 1-6 completed (v1.0)
- **Phase status:** Ready for planning (context already gathered)

### Existing Code Status

| File | Status | Notes |
|------|--------|-------|
| `engine/src/testing/mod.rs` | Implemented | TestSession, FixtureBuilder, ForwardChainingTester, BackwardChainingTrace, assertion macros |
| `engine/src/environment.rs` | Existent | EngineBuilder and Session management |
| `engine/src/fact.rs` | Existent | WorkingMemory with insert_fact, retract_fact, get_facts_by_type |
| `engine/src/salience/conflict_resolution.rs` | Existent | ConflictResolver with strategy selection |

### Known Limitations (to fix in Phase 7)

1. Testing module exists but **not integrated** with actual execution methods
2. `execute_forward` and `execute_backward` are placeholders with "not yet implemented" messages
3. No working memory JSON serialization (needed for snapshot testing - deferred to Phase 8)
4. InferenceTrace exists but may need visibility adjustments for public API access
5. FixtureBuilder.with_fact() is stubbed - doesn't actually add facts

---

## Requirements Mapping

| ID | Requirement | Priority |
|----|-------------|----------|
| TEST-01 | Working Memory Fixture Support with per-test isolation | P1 |
| TEST-02 | Rule-by-Rule Unit Tests with assertion helpers | P1 |
| TEST-03 | Forward Chaining Verification with activation sequence tracking | P1 |
| TEST-04 | Backward Chaining Trace Capture for debugging | P1 |
| TEST-05 | Deterministic Firing Mode to eliminate flaky tests | P1 |

---

## Critical Integration Points

Phase 7 must integrate with:

1. **`engine/src/forward_chaining.rs`** — Add activation tracking to `execute_forward`
2. **`engine/src/backward_chaining.rs`** — Expose InferenceTrace via public API
3. **`engine/src/salience/agenda.rs`** — Add deterministic sorting option to agenda processing
4. **Cargo.toml workspace dependencies** — Add criterion, mockall, serial_test

---

## Implementation Considerations

### 1. Testing Stack Integration

**Selected:** criterion + mockall + serial_test

| Crate | Purpose | Why |
|-------|---------|-----|
| `criterion` | Benchmarking | Performance testing of rule execution |
| `mockall` | Mocking dependencies | Isolate engine from external systems |
| `serial_test` | Parallel isolation | Ensure tests run independently |

**Cargo.toml additions needed:**
```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["async" ] }
mockall = "0.12"
serial_test = "3.0"
```

### 2. Fixture API Design

**Design:** `FixtureBuilder` returns `TestSession` wrapper with WorkingMemory access

```rust
// Pattern to implement:
pub struct FixtureBuilder {
    facts: Vec<Fact>,
}

impl FixtureBuilder {
    pub fn new() -> Self { ... }
    
    pub fn with_fact(mut self, name: &str, value: impl Into<String>) -> Self {
        // Actually add fact to self.facts
        self.facts.push(Fact::new(name, value));
        self
    }
    
    pub fn build(self) -> WorkingMemory {
        let mut wm = WorkingMemory::new();
        for fact in self.facts {
            wm.insert_fact(fact);
        }
        wm
    }
}
```

### 3. Forward Chaining Verification

**Design:** Automatic activation tracking with rule-name traces only (for performance)

```rust
// In engine/src/testing/mod.rs:
pub struct ForwardChainingTester {
    session: TestSession,
    activations: Vec<String>, // Just rule names, not full state
}

impl ForwardChainingTester {
    pub fn new() -> Self { ... }
    
    pub fn run_with_tracing(&mut self) -> Result<(), TestError> {
        // Run forward chaining and capture activation sequence
        self.activations = execute_forward_with_tracking(...);
        Ok(())
    }
}
```

### 4. Backward Chaining Trace Capture

**Design:** InferenceTrace structure needs public API exposure

```rust
// May need to change from pub(crate) to pub:
pub struct InferenceTrace {
    rule_applications: Vec<String>,
    missing_facts: Vec<String>,
}

impl InferenceTrace {
    // Public methods for test inspection:
    pub fn applications(&self) -> &[String] { &self.rule_applications }
    pub fn missing_facts(&self) -> &[String] { &self.missing_facts }
}
```

### 5. Deterministic Mode Configuration

**Design:** Per-test via TestSession builder; always use rule name tiebreaker in test mode

```rust
// In engine/src/testing/mod.rs:
pub fn deterministic_conflict_strategy(mut activations: Vec<Activation>) -> Vec<Activation> {
    // Sort by (salience DESC, rule_name ASC) for determinism
    activations.sort_by(|a, b| {
        b.salience.cmp(&a.salience).then(a.rule.name.cmp(&b.rule.name))
    });
    activations
}
```

---

## Critical Pitfalls to Address

1. **Infinite Loop Detection** — Implement max 1000 firings limit per test scenario
2. **State Isolation** — Use per-test engine instances with unique session IDs (UUIDs)
3. **Non-Deterministic Firing** — Always sort equal-salience rules by name as tiebreaker
4. **Test Data Cleanup** — Use RAII patterns for fact lifecycle management

---

## Testing Patterns to Establish

### Pattern 1: Per-Test Engine Isolation

```rust
#[test]
fn test_rule_fires_correctly() {
    let mut tester = ForwardChainingTester::new();
    
    // Create isolated fixture
    let fixture = FixtureBuilder::new()
        .with_fact("customer", "high_score")
        .build();
    
    // Run with specific rule set
    tester.session().insert_facts(fixture);
    tester.run_with_rules(&["rule_1", "rule_2"]);
    
    // Verify activation sequence
    assert_eq!(tester.activations(), vec!["rule_1", "rule_2"]);
}
```

### Pattern 2: Deterministic Execution Mode

```rust
#[test]
fn test_deterministic_ordering() {
    let mut tester = ForwardChainingTester::new();
    
    // Enable deterministic mode for this test
    tester.set_deterministic(true);
    
    // Same input always produces same output sequence
    let result1 = tester.run();
    let result2 = tester.run();
    
    assert_eq!(result1.activations, result2.activations);
}
```

### Pattern 3: Backward Chaining Trace Capture

```rust
#[test]
fn test_backward_chaining_trace() {
    let trace = backward_chain_query(&query)?;
    
    // Inspect the reasoning path
    assert!(trace.applications().contains(&"rule_3"));
    assert!(trace.missing_facts().contains(&"missing_fact"));
}
```

---

## Next Steps for Planning

1. **Cargo.toml updates** — Add testing stack dependencies
2. **Testing module integration** — Wire up execute_forward/execute_backward
3. **WorkingMemory serialization** — Implement JSON format (deferred to Phase 8)
4. **Public API exposure** — Adjust InferenceTrace visibility if needed
5. **Test organization** — Structure tests as mod.rs per feature area

---

## References

- [criterion docs](https://bheisler.github.io/criterion.rs/book/)
- [mockall docs](https://docs.rs/mockall/latest/mockall/)
- [serial_test docs](https://docs.rs/serial_test/latest/serial_test/)
