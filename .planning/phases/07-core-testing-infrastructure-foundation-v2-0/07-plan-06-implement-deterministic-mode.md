---
wave: 3
depends_on:
- plan-01-integrate-testing-stack.md
files_modified:
- engine/src/testing/mod.rs
autonomous: true
requirements_addressed:
- TEST-05
---

## Objective

Implement deterministic firing mode with conflict resolution sorting by rule name. This eliminates flaky tests where equal-salience rules fire in non-deterministic order across test runs.

## Tasks

### Task 01: Review existing deterministic_conflict_strategy function

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
/home/threaded/projects/rRules/engine/src/salience/agenda.rs
</read_first>

<action>
Review current implementation:
- The deterministic_conflict_strategy() exists but may not be used anywhere
- Verify it sorts by (salience DESC, rule_name ASC)

Update if needed to match specification: always sort by name as tiebreaker.
</action>

<acceptance_criteria>
- Function correctly sorts activations by salience DESC, then rule.name ASC
- Used in conflict resolution when deterministic mode is enabled
</acceptance_criteria>
```

### Task 02: Add DeterministicMode enum for configuration

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add:
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeterministicMode {
    /// Use default conflict resolution (non-deterministic for equal salience)
    Disabled,
    /// Sort by rule name when salience is equal
    EnabledByName,
}
```

Add a new variant if needed to support different deterministic strategies.
</action>

<acceptance_criteria>
- DeterministicMode enum exists with at least Disabled and EnabledByName variants
- Clear semantic meaning for each variant
</acceptance_criteria>
```

### Task 03: Update TestSession to accept DeterministicMode configuration

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Modify TestSession::new() to accept optional deterministic mode:
- `fn new_with_deterministic(mode: DeterministicMode) -> Self`

Store the mode and apply it during conflict resolution. Update EngineBuilder or create a testing-specific builder that applies deterministic mode.
</action>

<acceptance_criteria>
- TestSession can be created with deterministic mode
- Mode is stored and affects conflict resolution
- Default (new()) uses non-deterministic for backward compatibility
</acceptance_criteria>
```

### Task 04: Integrate deterministic sorting into Agenda::push or ConflictResolver

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/salience/agenda.rs
</read_first>

<action>
Option A - Modify Agenda to accept strategy:
- Add `fn push_with_strategy(&mut self, activation: Activation, strategy: &ConflictStrategy)`
  that uses the provided conflict resolution when inserting

Option B - Modify ConflictResolver directly:
- Update sort() methods to always include rule name as tiebreaker
- This makes lexicographic sorting truly deterministic

Prefer Option A for explicit per-call control.
</action>

<acceptance_criteria>
- Agenda can apply deterministic sorting on push
- Deterministic mode is used when TestSession executes rules
</acceptance_criteria>
```

### Task 05: Add assert_deterministic helper method

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add assert_deterministic() helper:
1. Takes a closure that sets up and executes rules
2. Runs the execution multiple times (e.g., 3 iterations)
3. Verifies all runs produce identical activation traces

Useful for verifying a test setup is truly deterministic.
</action>

<acceptance_criteria>
- Helper verifies determinism across multiple runs
- Fails if any run produces different trace
</acceptance_criteria>
```

### Task 06: Add tests for deterministic mode

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add tests:
1. test_deterministic_mode_sorts_by_name - Rules with equal salience sorted by name
2. test_non_deterministic_default_behavior - Default uses timestamp/other factors
3. test_deterministic_multiple_runs_same_result - assert_deterministic verifies consistency

Use assert_eq_test! for TestResult assertions.
</action>

<acceptance_criteria>
- All deterministic mode tests pass
- Tests verify actual sorting behavior
</acceptance_criteria>
```

## Verification Criteria

1. **Deterministic ordering works**:
   ```rust
   let session = TestSession::new_with_deterministic(DeterministicMode::EnabledByName);
   // Rules with salience 0 fire in alphabetical name order
   ```

2. **Tests pass**: `cargo test --lib testing` shows all tests passing
3. **No flakiness**: Multiple runs produce identical activation traces

## Must Haves

- DeterministicMode enum for configuration
- TestSession accepts deterministic mode at creation
- Conflict resolution always sorts by rule name as tiebreaker when determinism enabled
- Helper methods verify determinism across multiple runs
- Tests confirm deterministic behavior
