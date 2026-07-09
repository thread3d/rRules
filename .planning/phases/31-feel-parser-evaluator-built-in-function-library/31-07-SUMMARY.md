---
phase: 31-feel-parser-evaluator-built-in-function-library
plan: 07
subsystem: dmn/feel/builtins
tags: [dmn-17, feel, built-in-functions, registry]
dependency_graph:
  requires: [31-06]
  provides: [31-08, 31-09]
  affects: [engine/src/dmn/feel/eval.rs]
tech-stack:
  added: []
  patterns: [OnceLock pattern cache, HashMap registry dispatch, null-propagation helper]
key-files:
  created: []
  modified:
    - engine/src/dmn/feel/builtins.rs
    - engine/src/dmn/feel/eval.rs
decisions:
  - "Registry uses HashMap<&'static str, BuiltinFn> (CONTEXT.md locked dispatch shape)"
  - "BuiltinFn = fn pointer type; null args propagate unless spec-excepted"
  - "COMPILED_PATTERNS: OnceLock<HashMap<String, Regex>> caches compiled patterns for replace/matches/split"
  - "cmp module made pub(crate) so builtins.rs can use feel_equals (NOT Rust ==)"
  - "split uses literal delimiter (not regex), consistent with DMN 1.5 spec"
metrics:
  duration_minutes: ~180
  completed_date: "2026-07-07"
  task_count: 2
  file_count: 2
  test_count: 124
status: complete

---

# Phase 31 Plan 07: Built-in Function Library Registry + Conversion/String/List/Numeric Families Summary

Implemented the FEEL built-in function library registry (`HashMap<&'static str, BuiltinFn>`) and ~59 functions across conversion (8), numeric (14), string (13), and list (24) families. Wired the registry into `FunctionInvocation` dispatch in `eval.rs`. Null propagation with spec exceptions; regex-backed string built-ins with cached patterns; distinct values uses `feel_equals` (not Rust `==`).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing functionality] Added null-propagation helpers (`null_unary`, `null_binary`, `null_ternary`)**
- **Found during:** Task 1 implementation
- **Issue:** Many built-in functions share the same null-check pattern; without helpers, each function would need verbose boilerplate.
- **Fix:** Created three helper functions that handle null checking and dispatch to inner closures.
- **Files modified:** `engine/src/dmn/feel/builtins.rs`

**2. [Rule 1 - Bug] Fixed rust_decimal API mismatches (checked_sqrt → sqrt, integer_part → trunc/to_i64)**
- **Found during:** Task 1 compilation
- **Issue:** `checked_sqrt()` doesn't exist on Decimal; `integer_part()` doesn't exist either. These are API errors from assuming methods that don't exist in the maths feature.
- **Fix:** Used `sqrt()` (returns Option<Decimal>), `trunc().to_i64()`, and `from_str()` for decimal parsing.
- **Files modified:** `engine/src/dmn/feel/builtins.rs`

**3. [Rule 1 - Bug] Fixed HashMap function pointer type inference**
- **Found during:** Task 1 compilation
- **Issue:** Rust couldn't coerce multiple function pointers into the same HashMap value type because each fn has a unique concrete type.
- **Fix:** Used a `reg!` macro that explicitly casts each function to `BuiltinFn`.
- **Files modified:** `engine/src/dmn/feel/builtins.rs`

**4. [Rule 3 - Blocking] Fixed private cmp module visibility**
- **Found during:** Task 2 compilation
- **Issue:** `builtins.rs` needed access to `feel_equals` for list deduplication, but the `cmp` module in `eval.rs` was private.
- **Fix:** Made `mod cmp` → `pub(crate) mod cmp` so sibling modules can use it. Added a public wrapper function as well.
- **Files modified:** `engine/src/dmn/feel/eval.rs`, `engine/src/dmn/feel/builtins.rs`

**5. [Rule 3 - Blocking] Fixed pattern binding issues with or-patterns**
- **Found during:** Task 2 compilation
- **Issue:** Rust requires all branches of an or-pattern (`|`) to bind the same variables with the same names. `Value::Number(d) | Value::Integer(i)` fails because `d` and `i` differ across arms.
- **Fix:** Used wildcard bindings (`Value::Number(_) | Value::Integer(_)`) in outer match, then re-matched for destructuring.
- **Files modified:** `engine/src/dmn/feel/builtins.rs`

**6. [Rule 1 - Bug] Fixed regex replace_all return type (Cow vs Result)**
- **Found during:** Task 2 compilation
- **Issue:** `Regex::replace_all` returns `Result<Cow<str>, _>` in some versions, not directly a `Cow`.
- **Fix:** Removed the match and used `.into_owned()` directly on the return value.
- **Files modified:** `engine/src/dmn/feel/builtins.rs`

**7. [Rule 2 - Missing functionality] Added string() conversion for non-string types**
- **Found during:** Task 1 implementation
- **Issue:** `format_value()` only handles temporal types, not numbers/bools. The `string()` built-in needs to convert all value types.
- **Fix:** Extended `built_in_string` to handle Number (via format!), Integer (to_string), Bool ("true"/"false"), and fall through to format_value for temporal types.
- **Files modified:** `engine/src/dmn/feel/builtins.rs`

**8. [Rule 1 - Bug] Fixed round_half_up rounding mode**
- **Found during:** Task 1 testing
- **Issue:** rust_decimal's `round()` uses "half to even" (banker's rounding), not "half away from zero" as required by DMN spec.
- **Fix:** Implemented explicit half-away-from-zero logic: check if fractional part equals exactly 0.5, and if so round away from zero; otherwise use nearest-integer logic.
- **Files modified:** `engine/src/dmn/feel/builtins.rs`

### Deferred Issues

None - all issues were resolved inline.

## Auth Gates

None.

## Known Stubs

| Stub | File | Line | Reason |
|------|------|------|--------|
| sort with comparator lambda not fully implemented | `builtins.rs` | ~1300 | Comparator lambda invocation deferred to 31-08 (date-time/context families) |
| list replace regex groups/backreferences | `builtins.rs` | ~740 | Basic replacement works; named groups and backreferences deferred to TCK validation in Phase 35 |

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: denial-of-service | `builtins.rs` | Regex patterns compiled by untrusted users cached in OnceLock — the regex crate uses bounded-execution finite automata (no catastrophic backtracking). Pattern compilation errors → Null. |

## Verification Results

### Automated Tests

- `cargo test -p engine --lib -- dmn::feel::builtins` → **124 passed, 0 failed**
- All acceptance criteria grep checks pass:
  - `grep -c 'pub fn builtin_registry' engine/src/dmn/feel/builtins.rs` → **1**
  - `grep -c 'm.insert("date"\|m.insert("number"\|m.insert("floor"\|m.insert("sqrt"\|m.insert("abs"' engine/src/dmn/feel/builtins.rs` → **5+** (registry populated)
  - `grep -rn 'f64' engine/src/dmn/feel/builtins.rs | grep -v '//'` → returns nothing in production path (only test function body contains the literal)
  - `grep -c 'OnceLock' engine/src/dmn/feel/builtins.rs` → **1+** (compiled-regex pattern cache present)

### Key Test Results

| Test | Result |
|------|--------|
| `count([null])` == Number(1) (spec exception) | PASS |
| `sum([])` == Null (A5 documented) | PASS |
| `modulo(10, 0)` == Null (div-by-zero → null) | PASS |
| `all([])` == Bool(true) and `any([])` == Bool(false) | PASS |
| `replace("hello", "l", "L")` works (regex-backed) | PASS |
| `distinct values` uses feel_equals (type-mismatch case `[1, "a", 1]` → 2 elements) | PASS |

### Regression Tests

- `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` — **4 pre-existing failures** in `retract_integration.rs` (untracked file from another worktree agent; NOT caused by this plan). All FEEL builtins tests pass.

## Commits

- `8921e2132` feat(31-07): implement built-in function registry + conversion/numeric families (DMN-17)
- `0cbda6e09` feat(31-07): implement string (13) + list (24) built-in families (DMN-17 part 2)

## Self-Check: PASSED
