---
phase: 31-feel-parser-evaluator-built-in-function-library
plan: 05
subsystem: dmn
tags: [dmn, feel, evaluator, boxed-expressions, topological-sort, cycle-detection]

# Dependency graph
requires:
  - phase: 31-feel-parser-evaluator-built-in-function-library
    provides: scope (Scope with deps map), parser (parse_expression), types (FeelFunction carrier), AST (FeelExpression variants)
provides:
  - All 10 boxed-expression evaluators: List, Relation, Context (topo eval), FunctionInvocation (stub), FunctionDefinition (closure carrier), Conditional (=If delegation), Filter/For/Quantified (delegation to 31-04 arms)
  - Boxed-context topological evaluation with DFS color-marking cycle detection
  - free_vars AST walker for dependency graph construction
  - ContextCycleError type carrying cyclic entry names + source spans
  - external field on FeelFunction carrier (31-06 uses it at invocation rejection)
affects:
  - feel-parser-evaluator-built-in-function-library

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Boxed-context topological evaluation driven by flat-Scope deps map"
    - "DFS color-marking (White/Gray/Black) for cycle detection in dependency graphs"
    - "Free-variable AST walker for intra-context dependency graph construction"
    - "FeelFunction carrier extended with external flag + body=Some(Arc)"

key-files:
  created: []
  modified:
    - engine/src/dmn/feel/eval.rs — boxed-expression arms, free_vars, eval_context_topo, 22 tests
    - engine/src/dmn/feel/types.rs — added external field to FeelFunction

key-decisions:
  - "Boxed-context topo order uses DFS post-order (no reversal) because edge direction is entry→deps"
  - "Cycle detection tracks full cycle path via DFS stack, not just the Gray→Gray hit node"
  - "Free-vars AST walker ignores Quantified iter vars and FunctionDefinition params (bound names)"
  - "FeelFunction.external field added for 31-06 invocation-time rejection of external functions"

requirements-completed: [DMN-15]

coverage:
  - id: D1
    description: List boxed expression evaluates elements recursively into Value::Array
    requirement: DMN-15
    verification:
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#boxed_list_literal
        status: pass
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#boxed_list_nested_context
        status: pass
    human_judgment: false
  - id: D2
    description: Context boxed expression evaluates entries in dependency order via topo sort
    requirement: DMN-15
    verification:
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#boxed_context_simple
        status: pass
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#boxed_context_with_dependency
        status: pass
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#context_topo_out_of_order_dag
        status: pass
    human_judgment: false
  - id: D3
    description: Relation boxed expression evaluates rows into list of context Objects
    requirement: DMN-15
    verification:
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#boxed_relation
        status: pass
    human_judgment: false
  - id: D4
    description: FunctionDefinition boxed expression constructs FeelFunction carrier without evaluating body
    requirement: DMN-15
    verification:
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#boxed_function_definition_carrier
        status: pass
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#boxed_function_definition_body_not_evaluated
        status: pass
    human_judgment: false
  - id: D5
    description: FunctionInvocation boxed expression stubbed to Null pending 31-06/31-07 dispatch
    requirement: DMN-15
    verification:
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#boxed_function_invocation_stub
        status: pass
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#boxed_function_invocation_bound_callee_stub
        status: pass
    human_judgment: false
  - id: D6
    description: DFS color-marking detects cycles (2-cycle, 3-cycle, self-reference) and reports all cyclic entries as Null
    requirement: DMN-15
    verification:
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#context_topo_2_cycle
        status: pass
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#context_topo_3_cycle
        status: pass
      - kind: unit
        ref: engine/src/dmn/feel/eval.rs#context_topo_self_reference
        status: pass
    human_judgment: false
  - id: D7
    description: DAG-order invariance property verified via proptest (shuffled entry order → same result)
    requirement: DMN-15
    verification:
      - kind: property
        ref: engine/src/dmn/feel/eval.rs#context_topo_dag_order_invariance
        status: pass
    human_judgment: false

# Metrics
duration: 20min
completed: 2026-07-07
status: complete
---

# Phase 31 Plan 05: FEEL Boxed Expressions + Topological Context Evaluation Summary

**All 10 FEEL boxed-expression evaluators with recursive dispatch, and the boxed-context topological (dependency-order) evaluation with DFS color-marking cycle detection — the DMN-15 surface.**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-07-07T11:00:00Z
- **Completed:** 2026-07-07T11:20:00Z
- **Tasks:** 2 of 2
- **Files modified:** 2

## Accomplishments

- Implemented all 10 boxed-expression evaluators with recursive dispatch (List, Relation, Context via topo eval, FunctionInvocation stubbed to Null, FunctionDefinition → FeelFunction carrier, Conditional/Filter/For/Quantified delegate to existing arms)
- Implemented `eval_context_topo` with DFS color-marking cycle detection — cyclic entries evaluate to `Value::Null` with source-located error recorded
- Added `free_vars` AST walker for dependency graph construction (ignores bound names from quantifiers and function params)
- Extended `FeelFunction` carrier with `external` field (31-06 uses it at invocation rejection time)
- 22 tests pass: List, Relation, Context (simple + dependency), FunctionDefinition (carrier + external flag + body-not-evaluated), Conditional delegation, Filter/For/Quantified delegation, FunctionInvocation stub, cycle detection (2-cycle, 3-cycle, self-reference, mixed), DAG-order invariance proptest

## Task Commits

Each task was committed atomically:

1. **Task 1: Boxed-expression evaluators + topo eval** - `3c70ac96d` (feat)
2. **Task 2: Cycle-detection tests + DAG-order-invariance proptest** - `9281d6b3f` (test)

## Files Created/Modified

- `engine/src/dmn/feel/eval.rs` — Filled in Relation, Context (with topo eval), FunctionInvocation (stub), FunctionDefinition arms; added `free_vars` helper and `eval_context_topo` function with DFS cycle detection; 22 boxed-expression tests
- `engine/src/dmn/feel/types.rs` — Added `external` field to `FeelFunction` carrier

## Decisions Made

- **Topological order direction:** With edge direction `entry → deps` (meaning "this entry depends on those names"), DFS post-order already gives the correct evaluation order (dependencies finish before dependents). No reversal needed.
- **Full cycle path tracking:** The DFS tracks a stack of visited nodes; when Gray→Gray is detected, all nodes from the cycle start to the current node are marked as cyclic (not just the hit node). This ensures complete error reporting for multi-node cycles.
- **Free-vars AST walker:** Ignores Quantified iter vars and FunctionDefinition params since they are bound names — only references to unbound names contribute to the dependency graph.
- **FeelFunction.external field:** Added as a data-model decision so 31-06 can reject external functions at invocation time (only non-external user lambdas supported per DMN-16).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical Functionality] Added `external` field to FeelFunction carrier**
- **Found during:** Task 1 RED phase (test `boxed_function_definition_external_flag` failed to compile)
- **Issue:** The plan references storing the `external` flag on the `FeelFunction` carrier, but the struct declared in types.rs only had `param_names`, `body`, and `captured` fields. Without this field, 31-06 cannot reject external functions at invocation time (a stated requirement).
- **Fix:** Added `pub external: bool` to `FeelFunction` with safe default (`false`). Updated `placeholder()` constructor and existing test in types.rs to include the new field.
- **Files modified:** engine/src/dmn/feel/types.rs
- **Verification:** `boxed_function_definition_external_flag` test passes

**2. [Rule 1 - Bug] Fixed topological order reversal direction**
- **Found during:** Task 1 GREEN phase (test `boxed_context_with_dependency` failed — `{a:1, b:a+1}` returned `{b:Null, a:1}` instead of `{a:1, b:2}`)
- **Issue:** The code reversed the DFS post-order (`topo_order.reverse()`), but with edge direction `entry → deps`, post-order already gives correct evaluation order (dependencies finish first). Reversing put dependents before dependencies.
- **Fix:** Removed the `reverse()` call and added a comment explaining why no reversal is needed for this edge direction.
- **Files modified:** engine/src/dmn/feel/eval.rs
- **Verification:** `boxed_context_with_dependency` test passes

**3. [Rule 1 - Bug] Fixed free_vars QualifiedName iteration**
- **Found during:** Task 2 RED phase (compilation error)
- **Issue:** `collect_free_vars` tried to call `.extend(qn)` on a `&Vec<String>` which yields `&String`, not `String`.
- **Fix:** Changed to `.extend(qn.iter().cloned())` to properly iterate over owned strings.
- **Files modified:** engine/src/dmn/feel/eval.rs
- **Verification:** Compilation succeeds

**4. [Rule 3 - Blocking] Added Arc and Span imports at module level**
- **Found during:** Task 1 GREEN phase (compilation errors)
- **Issue:** `eval_context_topo` uses `Arc<FeelExpression>` for the closure body and `Span` in `ContextCycleError`, but these weren't imported at the eval.rs module level.
- **Fix:** Added `use std::sync::Arc;` and added `Span` to the `super::ast::{...}` import list.
- **Files modified:** engine/src/dmn/feel/eval.rs
- **Verification:** Compilation succeeds

**5. [Rule 1 - Bug] Fixed Context arm Err case return type**
- **Found during:** Task 1 GREEN phase (type mismatch: HashMap vs Value)
- **Issue:** The `Err` branch of the Context arm returned a raw `HashMap<String, Value>` instead of wrapping it in `Value::Object`.
- **Fix:** Wrapped with `Value::Object(obj)`.
- **Files modified:** engine/src/dmn/feel/eval.rs
- **Verification:** Compilation succeeds

---

**Total deviations:** 5 auto-fixed (1 missing critical functionality, 3 bug fixes, 1 blocking import issue)
**Impact on plan:** All deviations necessary for correctness. The `external` field addition was required by the plan's stated requirement that "31-06 rejects it at invocation time."

## Issues Encountered

- **Cycle detection granularity:** Initial implementation only marked the Gray→Gray hit node as cyclic, missing other nodes in the cycle path. Fixed by tracking the DFS stack and extracting all nodes from the cycle start to current position.
- **Topological order direction confusion:** The standard textbook algorithm reverses post-order for topo sort, but this assumes edges point "from prerequisite to dependent" (opposite of our `entry → deps` direction). Resolved by tracing through a small example.

## Self-Check: PASSED

### Verification Results

| Check | Result |
|-------|--------|
| `cargo test -p engine --lib dmn::feel::eval::boxed` | 22 passed, 0 failed |
| `cargo test -p engine --lib dmn::feel::eval` | 119 passed, 0 failed, 2 ignored |
| `cargo test -p engine --features engine-rete,...` | 437+ passed across all subcrates (zero Rete regression) |
| `grep -c 'FeelExpression::List\|Relation\|Context\|FunctionDefinition' eval.rs` | ≥4 boxed arms present |
| `grep -c 'enum Color\|White\|Gray\|Black' eval.rs` | DFS color-marking present |
| `grep -c 'fn eval_context_topo\|fn free_vars' eval.rs` | 2 helper functions present |

## Next Phase Readiness

- DMN-15 surface complete — all 10 boxed expressions evaluate recursively; boxed contexts use topological order with cycle detection
- Ready for Phase 31-06 (DMN-16: user-defined lambdas, lambda application, closure capture) which extends `FeelFunction.captured` and wires up the `FunctionInvocation` dispatch

---
*Phase: 31-feel-parser-evaluator-built-in-function-library*
*Completed: 2026-07-07*
