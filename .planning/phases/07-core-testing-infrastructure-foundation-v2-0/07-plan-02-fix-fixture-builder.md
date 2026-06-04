---
wave: 1
depends_on:
- plan-01-integrate-testing-stack.md
files_modified:
- engine/src/testing/mod.rs
autonomous: true
requirements_addressed:
- TEST-01
---

## Objective

Fix the FixtureBuilder to properly add facts and build WorkingMemory. Currently, `with_fact()` is a no-op and `build()` returns empty working memory. This plan makes it functional.

## Tasks

### Task 01: Fix FixtureBuilder::with_fact to accumulate facts

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Update the with_fact method in FixtureBuilder to:
1. Accept name and value parameters
2. Create a Fact struct with unique ID, fact_type_id derived from name, and payload containing the value
3. Store the fact in a real `facts: Vec<Fact>` field (not `_facts` underscore prefix)

Current stub implementation just discards inputs.
</action>

<acceptance_criteria>
- FixtureBuilder has `facts: Vec<Fact>` field (not `_facts`)
- with_fact() creates and stores actual Fact instances
- Facts have unique IDs, fact_type_id from name parameter, payload with value
</acceptance_criteria>
```

### Task 02: Fix FixtureBuilder::build to create WorkingMemory with facts

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
/home/threaded/projects/rRules/engine/src/fact.rs
</read_first>

<action>
Update the build() method in FixtureBuilder to:
1. Create a new WorkingMemory instance
2. Insert all accumulated facts into the working memory using insert_fact()
3. Return the populated WorkingMemory

Refer to WorkingMemory::insert_fact() signature for correct call pattern.
</action>

<acceptance_criteria>
- build() returns WorkingMemory with all facts inserted
- No facts are lost or skipped during insertion
- Returns Result if insert_fact can fail, or panics on error (consistent with testing use case)
</acceptance_criteria>
```

### Task 03: Add TestSession method to access working memory

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
/home/threaded/projects/rRules/engine/src/environment.rs
</read_first>

<action>
Add methods to TestSession:
1. `with_working_memory(&mut self, f: impl FnOnce(&mut WorkingMemory))` - Execute closure with mutable working memory reference
2. Optionally: `working_memory_mut(&mut self) -> &mut WorkingMemory` for direct access

The TestSession needs to hold an Engine instance (not just EngineBuilder) to provide working memory access.
</action>

<acceptance_criteria>
- TestSession can execute closures with mutable working memory
- API allows test code to add facts before engine execution
</acceptance_criteria>
```

### Task 04: Add tests for FixtureBuilder

```xml
<read_first>
/home/threaded/projects/rRules/engine/src/testing/mod.rs
</read_first>

<action>
Add unit tests in the tests module:
1. test_fixture_builder_adds_facts - Verify facts are stored correctly
2. test_fixture_builder_builds_nonempty_memory - Verify WorkingMemory has facts after build()
3. test_fixture_builder_fact_values - Verify payload values are correct

Use assert_eq! from std::assert (not assert_eq_test!) since these are unit tests.
</action>

<acceptance_criteria>
- All new tests pass with `cargo test --lib testing`
- Tests verify both fact accumulation and WorkingMemory population
</acceptance_criteria>
```

## Verification Criteria

1. **FixtureBuilder works end-to-end**:
   ```rust
   let fixture = FixtureBuilder::new()
       .with_fact("customer", ("high_score", "true"))
       .build();
   assert!(!fixture.facts.is_empty());  // After adding len() method or similar access
   ```

2. **Tests pass**: `cargo test --lib testing` shows all tests passing
3. **No compilation warnings** for the modified code

## Must Haves

- FixtureBuilder::with_fact() actually creates and stores Fact instances
- FixtureBuilder::build() returns WorkingMemory populated with those facts
- TestSession provides working memory access via closure or getter method
- All changes are backward compatible (no breaking API changes)
