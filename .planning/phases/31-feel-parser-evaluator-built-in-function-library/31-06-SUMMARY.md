---
phase: 31-feel-parser-evaluator-built-in-function-library
plan: 06
subsystem: dmn
tags: [dmn, feel, lambda, closure, lexical-capture, grammar-rule-55]

# Dependency graph
requires:
  - phase: 31-feel-parser-evaluator-built-in-function-library
    provides: scope (Scope with deps map), parser (parse_expression), types (FeelFunction carrier with body/captured/param_names/external fields), AST (FeelExpression::FunctionDefinition, FeelExpression::FunctionInvocation)
provides:
  - User-defined FEEL lambda invocation with lexical capture (DMN-16)
  - Closure-capture model: definition-site Scope snapshot via Arc<Scope>
  - Lambda application logic: merge captured scope + param bindings, evaluate body
  - External function rejection at invocation time (Null + warning)
  - Built-in registry dispatch structure for 31-07 (placeholder with warning)
affects:
  - feel-parser-evaluator-built-in-function-library

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Lexical scope capture via Arc<Scope> snapshot at definition time"
    - "Lambda application: merge captured + params, evaluate body (params override)"
    - "External function rejection: external=true -> Null + warning (non-external constraint)"
    - "Built-in registry dispatch structured for 31-07 (body=None arm)"

key-files:
  created: []
  modified:
    - engine/src/dmn/feel/eval.rs — FunctionDefinition captures scope, FunctionInvocation dispatches lambda application + external rejection, lambda_apply() helper, 13 new tests

key-decisions:
  - "Lexical capture via Arc<Scope> snapshot at definition time (not dynamic capture)"
  - "External functions rejected at invocation (Null + warning), not parse-time"
  - "Built-in registry dispatch structured but empty for 31-07 (body=None arm)"
  - "Arity mismatch returns Null (documented behavior, per plan recommendation)"

patterns-established:
  - "Lambda application pattern: captured scope -> merge params -> evaluate body"
  - "External function rejection pattern: external=true -> Null + warning at invocation"
  - "Built-in registry dispatch structure: body=None arm for future 31-07 population"

requirements-completed: [DMN-16]

# Coverage metadata (#1602) — one entry per shipped deliverable. Drives DETERMINISTIC UAT routing in verify-work.
coverage:
  - id: D1
    description: Closure capture at definition time populates FeelFunction.captured with definition-site Scope snapshot
    requirement: DMN-16
    verification:
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#lambda_captured_scope_populated_at_definition
        status: pass
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#lambda_captures_definition_site_bindings
        status: pass
    human_judgment: false
  - id: D2
    description: Lambda invocation merges captured scope + param bindings and evaluates body (positional args)
    requirement: DMN-16
    verification:
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#lambda_application_merges_scope_and_evaluates
        status: pass
    human_judgment: false
  - id: D3
    description: Named arguments work (param_names zipped with named values in declaration order)
    requirement: DMN-16
    verification:
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#lambda_named_args_work
        status: pass
    human_judgment: false
  - id: D4
    description: Lexical capture verified at invocation (definition-site x=10 used, not call-site x=99)
    requirement: DMN-16
    verification:
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#lambda_lexical_capture_verified_at_invocation
        status: pass
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#lambda_lexical_capture_not_dynamic
        status: pass
    human_judgment: false
  - id: D5
    description: External functions rejected at invocation (Null + warning logged)
    requirement: DMN-16
    verification:
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#lambda_external_invocation_returns_null
        status: pass
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#lambda_external_definition_still_has_captured
        status: pass
    human_judgment: false
  - id: D6
    description: Arity mismatch returns Null (documented behavior)
    requirement: DMN-16
    verification:
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#lambda_arity_mismatch_invocation_returns_null
        status: pass
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#lambda_arity_mismatch_returns_null
        status: pass
    human_judgment: false
  - id: D7
    description: FeelFunction serde round-trips with captured scope (body + captured + param_names serialize/deserialize)
    requirement: DMN-16
    verification:
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#feel_function_serde_round_trip_with_captured
        status: pass
    human_judgment: false

# Metrics
duration: 25min
completed: 2026-07-07
status: complete
---

# Phase 31 Plan 06: FEEL User-Defined Lambda Invocation with Lexical Capture Summary

**User-defined FEEL lambdas (grammar rule 55, non-external) with lexical capture — closure body evaluated against merged captured scope + param bindings; external functions rejected at invocation.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-07-07T14:30:00Z
- **Completed:** 2026-07-07T14:55:00Z
- **Tasks:** 2 of 2
- **Files modified:** 1

## Accomplishments

- Implemented closure capture at definition time — `FeelFunction.captured` populated with the definition-site `Scope` snapshot via `Arc<Scope>` (lexical, not dynamic)
- Implemented lambda invocation logic in `FunctionInvocation` arm — merge captured scope + param bindings, evaluate body
- Added external function rejection (Null + warning logged) per non-external constraint (CONTEXT.md)
- Structured built-in registry dispatch for 31-07 (body=None arm with placeholder warning)
- 13 tests pass: closure capture, lambda application, named args, lexical capture verification, external rejection, arity mismatch, serde round-trip

## Task Commits

Each task was committed atomically:

1. **Task 1: Closure capture at definition time** - `50325575e` (feat)
2. **Task 2: Lambda invocation + external rejection** - `3373eb2da` (feat)

## Files Created/Modified

- `engine/src/dmn/feel/eval.rs` — Updated FunctionDefinition arm to snapshot captured scope; updated FunctionInvocation arm for lambda dispatch; added `lambda_apply()` helper; 13 new tests in `lambdas` and `external_rejected` modules

## Decisions Made

- **Lexical capture via Arc<Scope>:** The definition-site Scope is snapshotted via `Arc<Scope>` (cheap clone). At invocation, the working scope starts from captured, then params override by name. This ensures lexical scoping (closure sees definition site, not call site).
- **External rejection at invocation:** External functions are parsed with `external=true` but rejected at invocation time (Null + warning). This forward-compatible shape lets future phases add external integration without AST migration.
- **Built-in registry structure for 31-07:** The `body=None` arm is structured to accept a registry populated by 31-07. For now, it logs a warning and returns Null.
- **Arity mismatch → Null:** Documented behavior per plan recommendation — arity mismatch yields Value::Null (not an error).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed `tracing` crate unavailability**
- **Found during:** Task 2 GREEN phase (compilation errors)
- **Issue:** The plan suggested using `tracing::warn!()` for external function rejection, but the `tracing` crate is not available in this build context.
- **Fix:** Switched to `eprintln!()` consistent with the codebase's logging convention (as also suggested in the plan).
- **Files modified:** engine/src/dmn/feel/eval.rs
- **Verification:** Compilation succeeds, tests pass

**2. [Rule 1 - Bug] Fixed type mismatch in lambda_apply Scope construction**
- **Found during:** Task 2 GREEN phase (compilation errors)
- **Issue:** `if let Some(captured)` returned `Arc<Scope>` in one branch and `Scope` in the other, causing a type mismatch.
- **Fix:** Used `.clone().map(|arc| (*arc).clone()).unwrap_or_else(Scope::empty)` to properly dereference the Arc.
- **Files modified:** engine/src/dmn/feel/eval.rs
- **Verification:** Compilation succeeds

**3. [Rule 1 - Bug] Fixed duplicate test code after initial module addition**
- **Found during:** Task 1 RED phase (compilation errors)
- **Issue:** The original edit added tests outside the `#[cfg(test)] mod lambdas` block, creating duplicates and undefined function references.
- **Fix:** Removed duplicate content after line 3129 (the closing brace of the lambdas module).
- **Files modified:** engine/src/dmn/feel/eval.rs
- **Verification:** Compilation succeeds

---

**Total deviations:** 3 auto-fixed (3 bug fixes)
**Impact on plan:** All deviations necessary for correctness. No scope creep.

## Issues Encountered

- **tracing crate unavailability:** The `tracing` crate is not available in the engine build context. Used `eprintln!()` instead, which is consistent with the codebase's existing logging pattern (as also suggested in the plan).
- **Pre-existing retract_integration test failures:** The `retract_integration` tests fail due to feature-gating issues unrelated to this phase (pre-existing issue).

## Self-Check: PASSED

### Verification Results

| Check | Result |
|-------|--------|
| `cargo test -p engine --lib dmn::feel::eval::lambdas` | 7 passed, 0 failed |
| `cargo test -p engine --lib dmn::feel::eval::external_rejected` | 6 passed, 0 failed |
| `cargo test -p engine --lib dmn::feel::eval` | 132 passed, 0 failed, 2 ignored |
| `grep -c 'captured: Some(Arc' eval.rs` | ≥1 (captured scope populated) |
| `grep -c 'lambda_apply\|external function invocation rejected' eval.rs` | ≥2 (lambda application + external rejection present) |
| `grep -c 'mod lambdas\|mod external_rejected' eval.rs` | 2 (two test modules) |

## Next Phase Readiness

- DMN-16 surface complete — user-defined FEEL lambdas with lexical capture and invocation implemented
- Ready for Phase 31-07 (DMN-17: built-in function library, ~70+ functions) which will populate the registry dispatch structure left in place by this plan

---
*Phase: 31-feel-parser-evaluator-built-in-function-library*
*Completed: 2026-07-07*
