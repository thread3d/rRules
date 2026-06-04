---
wave: 2
depends_on:
- plan-03-wire-forward-chaining.md
files_modified:
- engine/src/testing/mod.rs
autonomous: true
requirements_addressed:
- TEST-03
---

## Objective

Add forward chaining verification helpers with assertion methods for sequence checks. These provide a fluent API for testing rule firing order, counts, and expected patterns.

## Tasks

### Task 01: Add assert_rule_sequence method to ForwardChainingTester

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add assert_rule_sequence() method that:
1. Takes expected rule names as input (Vec<&str> or similar)
2. Compares with actual activation_trace()
3. Returns TestResult<(), TestError::AssertionFailed>

Use the existing assert_eq_test! macro for comparisons.
</action>

<acceptance_criteria>
- Method accepts expected rule sequence
- Compares against actual firing trace
- Provides clear error message on mismatch
</acceptance_criteria>
```

### Task 02: Add assert_fired_count method

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add assert_fired_count() method that:
1. Takes expected count as usize
2. Compares with firing_count()
3. Returns TestResult<(), TestError::AssertionFailed>
</action>

<acceptance_criteria>
- Method verifies exact rule count fired
- Provides clear error on mismatch
</acceptance_criteria>
```

### Task 03: Add assert_fired_in_order method for partial sequence

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add assert_fired_in_order() method that:
1. Takes expected rule names in order
2. Verifies they fired in that relative order (not necessarily consecutive)
3. Returns TestResult<(), TestError::AssertionFailed>

This tests ordering without requiring all rules to fire consecutively.
</action>

<acceptance_criteria>
- Method verifies relative ordering of specified rules
- Handles cases where other rules may fire between expected ones
</acceptance_criteria>
```

### Task 04: Add assert_rule_not_fired method

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add assert_rule_not_fired() method that:
1. Takes a rule name as input
2. Verifies the rule was NOT in the activation trace
3. Returns TestResult<(), TestError::AssertionFailed>
</action>

<acceptance_criteria>
- Method verifies exclusion of specific rules
- Provides clear error when rule incorrectly fires
</acceptance_criteria>
```

### Task 05: Add with_test_rules helper for test setup

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
/home/threaded/projects/rRules/engine/src/rules.rs
</read_first>

<action>
Add with_test_rules() method to TestSession that:
1. Accepts Rule instances or RuleBuilder configurations
2. Loads them into the engine's rule_set before execution
3. Returns self for chaining

This provides a fluent API: session.with_test_rules(rules).execute_forward()
</action>

<acceptance_criteria>
- Method loads rules into engine
- Supports both individual and multiple rules
- Enables method chaining for fluent API
</acceptance_criteria>
```

### Task 06: Add tests for verification helpers

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add tests in test module:
1. test_assert_rule_sequence_matches - Sequence matches exactly
2. test_assert_fired_count_correct - Count is accurate
3. test_assert_rule_not_fired_exclusion - Excluded rules don't fire
4. test_with_test_rules_chaining - Fluent API works

Use assert_eq_test! for TestResult-returning assertions.
</action>

<acceptance_criteria>
- All helper methods work correctly
- Tests verify both positive and negative cases
</acceptance_criteria>
```

## Verification Criteria

1. **API is usable**:
   ```rust
   let mut tester = ForwardChainingTester::new();
   tester.execute_with_rules(rules);
   tester.assert_rule_sequence(&["rule_a", "rule_b"]);
   tester.assert_fired_count(2);
   ```

2. **Tests pass**: `cargo test --lib testing` shows all tests passing
3. **No warnings**: `cargo clippy` shows no new warnings

## Must Haves

- assert_rule_sequence() for exact sequence verification
- assert_fired_count() for count verification  
- assert_fired_in_order() for partial ordering
- assert_rule_not_fired() for exclusion checks
- Fluent API via with_test_rules() for setup chaining
