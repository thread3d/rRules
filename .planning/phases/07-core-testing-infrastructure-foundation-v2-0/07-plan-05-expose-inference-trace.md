---
wave: 3
depends_on:
- plan-01-integrate-testing-stack.md
files_modified:
- engine/src/testing/mod.rs
- engine/src/backward_chaining.rs
autonomous: true
requirements_addressed:
- TEST-04
---

## Objective

Expose InferenceTrace via public API for backward chaining trace capture. Currently, the tracing types exist but are not properly integrated into the testing framework or fully exposed.

## Tasks

### Task 01: Review and update backward_chaining module exports

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/backward_chaining.rs
/home/threaded/projects/rRules/engine/src/lib.rs
</read_first>

<action>
1. Check what is currently exported from backward_chaining module
2. Ensure InferenceTrace and related types (InferenceStep, StepType) are pub
3. Verify execute_backward() is properly re-exported in lib.rs

Add pub use statements if needed.
</action>

<acceptance_criteria>
- All tracing types (InferenceTrace, InferenceStep, StepType) are public
- execute_backward() is exported from lib::testing module for testing use
</acceptance_criteria>
```

### Task 02: Add backward chaining trace capture to TestSession

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add method to TestSession:
- `fn execute_backward_chaining(&mut self, goal: &str) -> InferenceTrace`

This should:
1. Call the engine's execute_backward() with the goal
2. Return the trace for inspection in tests

The Engine needs to be accessible from TestSession.
</action>

<acceptance_criteria>
- Method accepts goal string and returns InferenceTrace
- Uses actual engine backward chaining execution
</acceptance_criteria>
```

### Task 03: Add trace verification helpers

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add helper methods to TestSession or standalone functions:
1. `fn assert_trace_contains_step(trace: &InferenceTrace, expected_desc: &str)`
   - Verifies a step with given description exists in trace
2. `fn assert_trace_success(trace: &InferenceTrace)` 
   - Verifies inference succeeded (trace.success == true)
3. `fn assert_trace_path_length(trace: &InferenceTrace, expected_len: usize)`
   - Verifies the reasoning path has expected number of steps

All return TestResult<(), TestError>.
</action>

<acceptance_criteria>
- Trace verification helpers are available for tests
- Clear error messages when assertions fail
</acceptance_criteria>
```

### Task 04: Add trace step type checking

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add methods to check specific step types:
1. `fn find_rule_application(trace: &InferenceTrace, rule_id: &str) -> Option<&InferenceStep>`
   - Find a rule application step by rule ID
2. `fn has_fact_match_step(trace: &InferenceTrace) -> bool`
   - Check if trace contains a fact match step
3. `fn assert_no_backtrack_steps(trace: &InferenceTrace)`
   - Verify no backtracking occurred (useful for successful paths)

Use StepType enum values (Query, RuleApplication, SubGoal, FactMatch, Backtrack).
</action>

<acceptance_criteria>
- Methods allow detailed inspection of inference trace
- Support checking for specific step types in the path
</acceptance_criteria>
```

### Task 05: Add tests for backward chaining integration

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add tests:
1. test_backward_chaining_trace_structure - Basic trace has expected fields
2. test_backward_chaining_success_trace - Successful inference has correct path
3. test_trace_contains_rule_application - Rule application steps are recorded

Use the tracing types directly for testing.
</action>

<acceptance_criteria>
- All backward chaining tests pass
- Traces capture actual reasoning paths
</acceptance_criteria>
```

## Verification Criteria

1. **API is accessible**:
   ```rust
   let mut session = TestSession::new();
   let trace = session.execute_backward_chaining("is_eligible");
   assert!(trace.success);
   ```

2. **Tests pass**: `cargo test --lib testing` shows all tests passing
3. **No warnings**: Code compiles cleanly

## Must Haves

- InferenceTrace is publicly accessible from lib::testing module
- TestSession.execute_backward_chaining() returns real traces
- Helper methods for trace verification exist
- Tests verify backward chaining actually works
