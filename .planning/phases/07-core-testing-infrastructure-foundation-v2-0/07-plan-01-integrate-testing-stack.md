---
wave: 1
depends_on: []
files_modified:
- engine/Cargo.toml
- Cargo.toml
autonomous: true
requirements_addressed:
- TEST-01
---

## Objective

Integrate the Rust testing stack (criterion, mockall, serial_test) into the workspace by updating Cargo.toml files. This establishes the foundation for performance benchmarks, mocking, and ordered test execution.

## Tasks

### Task 01: Update engine/Cargo.toml dev-dependencies

```xml
<read_first>
/home/threaded/projects/rRules/engine/Cargo.toml
</read_first>

<action>
Add/update dev-dependencies section with:
- criterion = { version = "0.5", features = ["async_tokio"] } (already present, verify)
- mockall = "0.12" (already present, verify)
- serial_test = "3" (already present, verify)

Verify all versions are correct using `cargo tree -d` to check installed versions.
</action>

<acceptance_criteria>
- All three dev-dependencies (criterion, mockall, serial_test) are declared in engine/Cargo.toml
- No version conflicts with existing dependencies
</acceptance_criteria>
```

### Task 02: Add workspace-level testing library versions to root Cargo.toml

```xml
<read_first>
/home/threaded/projects/rRules/Cargo.toml
</read_first>

<action>
Add a [workspace.dependencies] section if not present, and include:
- criterion = { version = "0.5", features = ["async_tokio"] }
- mockall = "0.12"
- serial_test = "3"

Reference these in engine/Cargo.toml using workspace = true format.
</action>

<acceptance_criteria>
- Workspace dependencies section exists with all three testing libraries
- Engine references use { workspace = true } format
- `cargo tree` shows consistent versions across workspace
</acceptance_criteria>
```

### Task 03: Verify integration

```xml
<read_first>
None
</read_first>

<action>
Run:
1. cargo update -p criterion --precise <latest>
2. cargo update -p mockall --precise <latest>
3. cargo update -p serial_test --precise <latest>
4. cargo build --workspace --tests

Verify no compilation errors and all testing crates resolve correctly.
</action>

<acceptance_criteria>
- All dev-dependencies compile without warnings
- `cargo test --workspace --no-run` succeeds
</acceptance_criteria>
```

## Verification Criteria

1. **Dependencies Resolved**: `cargo tree -d` shows criterion, mockall, and serial_test with no conflicts
2. **Build Success**: `cargo build --workspace --tests` completes without errors
3. **Test Compilation**: `cargo test --workspace --no-run` compiles all test code

## Must Haves

- criterion v0.5 with async_tokio feature for async benchmarking support
- mockall v0.12 for mocking Engine and other components in tests
- serial_test v3 for ordered test execution to ensure deterministic test runs
- All versions referenced via workspace.dependencies in root Cargo.toml
