---
wave: 2
depends_on:
- plan-01-integrate-testing-stack.md
- plan-02-fix-fixture-builder.md
files_modified:
- engine/src/testing/mod.rs
autonomous: true
requirements_addressed:
- TEST-03
---

## Objective

Wire up execute_forward in engine/src/testing/mod.rs with activation tracking. Currently, execute_forward is a placeholder that returns an empty ExecutionResult without actually firing rules or tracking activations.

## Tasks

### Task 01: Expose EngineBuilder from environment module for testing use

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/environment.rs
</read_first>

<action>
The EngineBuilder already exists and is re-exported. Verify:
1. EngineBuilder::build() creates a functional Engine instance
2. The Engine has working_memory_mut() method for adding test facts

If the Engine builder doesn't provide full functionality, add methods to enable engine configuration.
</action>

<acceptance_criteria>
- EngineBuilder can create fully functional Engine instances
- Engine provides working memory access for test setup
</acceptance_criteria>
```

### Task 02: Update TestSession to hold actual Engine instance

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Change TestSession struct:
1. Replace `_builder: EngineBuilder` with `engine: Engine`
2. Initialize by creating engine from EngineBuilder in new()
3. The session should own the Engine to allow fact insertion and execution

Update TestSession methods accordingly.
</action>

<acceptance_criteria>
- TestSession holds actual Engine instance (not just builder)
- Session can insert facts into engine's working memory
- Session ID generation remains functional for isolation tracking
</acceptance_criteria>
```

### Task 03: Implement execute_forward with activation tracking

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
/home/threaded/projects/rRules/engine/src/forward_chaining.rs
/home/threaded/projects/rRules/engine/src/salience/agenda.rs
</read_first>

<action>
Update ForwardChainingTester to:
1. Store Engine instance (from TestSession)
2. Implement execute_forward() that:
   a. Takes a RuleSet as input (or loads from session config)
   b. Calls the actual engine execution
   c. Captures activations from the Agenda during firing

Key: The execution must actually iterate through the agenda, pop activations, and record rule names.
</action>

<acceptance_criteria>
- execute_forward() fires rules using real Engine execution
- Activations are captured with rule names in the trace
- No longer just returns empty result
</acceptance_criteria>
```

### Task 04: Add method to get activation traces

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add methods to ForwardChainingTester:
1. `fn execute_with_rules(&mut self, rules: RuleSet)` - Execute with specified rules
2. `fn activation_trace(&self) -> &[String]` - Get captured rule names in order
3. `fn firing_count(&self) -> usize` - Count of fired rules

The trace should record rule.name values as they fire.
</action>

<acceptance_criteria>
- Methods provide clean API for forward chaining testing
- Trace captures actual execution order, not just stored activations
</acceptance_criteria>
```

### Task 05: Add tests for ForwardChainingTester

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add integration tests:
1. test_forward_chaining_fires_single_rule - Single rule fires with matching fact
2. test_forward_chaining_respects_salience - Higher salience rules fire first
3. test_forward_chaining_activation_trace - Trace captures correct rule order

Use assert_eq_test! macro for assertions (returns TestResult).
</action>

<acceptance_criteria>
- All forward chaining tests pass
- Salience ordering is verified when multiple rules match
</acceptance_criteria>
```

## Verification Criteria

1. **Execution works**: ForwardChainingTester::execute_with_rules() actually fires rules
2. **Activation tracking**: activation_trace() returns rule names in firing order
3. **Tests pass**: `cargo test --lib testing` shows all new tests passing

## Must Haves

- execute_forward() uses real Engine, not placeholder
- Activations are captured with rule names during execution
- ForwardChainingTester provides clean API for forward chaining verification
- Tests verify actual firing behavior, not stubs
