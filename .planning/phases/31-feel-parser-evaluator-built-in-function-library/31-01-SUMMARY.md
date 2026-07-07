---
phase: 31-feel-parser-evaluator-built-in-function-library
plan: 01
subsystem: dmn/feel
tags: [feel, parser, lexer, ast, dmn, foundation]
requires: []
provides: [feel-ast-skeleton, feel-lexer, feel-parser-entry-points, feel-module-skeleton]
affects: [engine/src/dmn/feel/parser.rs, engine/src/dmn/feel/ast.rs, engine/src/dmn/feel/types.rs]
tech-stack:
  added: [dashmap@6.2.1, regex@1.12.4]
  patterns: [hand-written-recursive-descent, pratt-precedence, tdd-red-green, closed-enum-free-functions, error-recovery-partial-ast]
key-files:
  created:
    - engine/src/dmn/feel/ast.rs
    - engine/src/dmn/feel/parser.rs
    - engine/src/dmn/feel/eval.rs
    - engine/src/dmn/feel/builtins.rs
    - engine/src/dmn/feel/coercion.rs
    - engine/src/dmn/feel/cache.rs
    - engine/tests/fixtures/tck/feel/.gitkeep
  modified:
    - engine/Cargo.toml
    - engine/src/dmn/feel/mod.rs
    - engine/src/dmn/feel/types.rs
decisions:
  - "Single FeelExpression enum (not #[non_exhaustive]) carrying all grammar forms + 10 boxed expressions — exhaustive match is the safety net (Anti-Pattern 5)"
  - "Flat Scope + explicit dependency tracking (CONTEXT.md user override, not parent/child chain)"
  - "Error recovery producing partial AST with Err nodes carrying spans (user override, NOT fail-fast)"
  - "Closed enum + free functions pattern (Phase 30 Pattern 2) for the FEEL module"
  - "FeelFunction carrier extended with param_names/body/captured fields as a data-model decision (Arc-backed for cheap closure sharing)"
  - "Pratt precedence ladder: or=10, and=20, comparison/between/in=30, instance of=35, additive=40, mult=50, **=60 RIGHT-assoc, unary=70, postfix=80, primary=90"
  - "Between desugaring: BinOp(Between, a, List([b, c])) — and NOT consumed by Pratt loop (parse_expr(31) stops before and at bp 20)"
  - "Err node propagation in binary ops, parens, and postfix — don't wrap errors in more AST nodes"
  - "Keyword-as-name disambiguation by position (Pitfall 3): lexer emits Token::Name for all keyword strings; parser decides keyword vs name by position"
metrics:
  duration: ~9min
  completed: 2026-07-07T06:27:04Z
  tasks: 3
  files: 11
  tests-passing: 76 feel + 33 parser/lexer + 0 Rete regression
status: complete
---

# Phase 31 Plan 01: FEEL Parser + AST Foundation Summary

Hand-written recursive-descent + Pratt-precedence parser for FEEL grammar rules 1-65, with dual entry points (parse_expression / parse_simple_unary_tests), keyword-as-name disambiguation, error-recovery partial AST, and DoS guards — the foundation all 8 subsequent Phase 31 plans build on.

## Tasks Completed

### Task 1: Wave-0 FEEL Infrastructure (auto)

Created the module skeleton (`ast.rs`, `eval.rs`, `builtins.rs`, `coercion.rs`, `cache.rs`), extended `FeelFunction` carrier with `param_names`/`body`/`captured` fields (Arc-backed), added `FeelExpression` enum with all variants + `Span` + `BinOpKind` + `QuantKind`, wired sub-modules in `mod.rs`, added `dashmap`/`regex` dependencies to `Cargo.toml`, created TCK fixture directory.

**Commit:** `328f6db8f` — `feat(31-01): Wave-0 FEEL infrastructure — FeelExpression AST, module skeleton, FeelFunction carrier extension`

### Task 2: Lexer with TDD (auto, tdd=true)

**RED:** 13 failing lexer tests covering numbers (Decimal, never f64), strings with rule-64 escapes, at-literals, keyword-as-name emission, punctuators (**, <=, >=, !=), qualified-name splitting on `.`, max_tokens DoS guard, line/col tracking.

**Commit:** `6c2b63069` — `test(31-01): add failing lexer tests for FEEL grammar lexical forms`

**GREEN:** Full lexer implementation: `Token` enum with all variants, `TokenSpan` struct, `Lexer` with `new()`/`with_max_tokens()`/`tokenize()`. Whitespace skip, numbers via `Decimal::from_str_exact`, strings with `\'`/`\"`/`\\`/`\n`/`\r`/`\t`/`\u{XXXX}` escapes, `@""` at-literals, names (keyword strings emit `Token::Name`), all punctuators including `**`/`<=`/`>=`/`!=`. `max_tokens` DoS guard (default 10_000). All 13 tests pass.

**Commit:** `3e78b8dc1` — `feat(31-01): implement FEEL lexer — Token/Lexer with Decimal numbers, string escapes, DoS guard`

### Task 3: Recursive-Descent + Pratt Parser with TDD (auto, tdd=true)

**RED:** 19 failing parser tests covering precedence (1+2*3), right-assoc ** (Pitfall 2), or<and precedence, path<filter (A2), instance-of precedence (A3), keyword-as-name context keys (Pitfall 3), function with keyword params, dash sentinel, not() form, comma-list Or-chain, error recovery Err node, DoS depth cap, for/some/instance-of, list/context, function definitions (external + non-external), between desugaring, not() as invocation.

**Commit:** `4c48797ef` — `test(31-01): add failing parser tests for grammar 1-65`

**GREEN:** Full recursive-descent + Pratt parser:
- `Parser` struct with `tokens`/`pos`/`depth`/`max_depth`
- Pratt precedence ladder: or=10, and=20, comparison/between/in=30, instance of=35, additive=40, mult=50, **=60 RIGHT-assoc, unary=70, postfix=80, primary=90
- Statement-level dispatch: for/if/some/every/function at primary position
- Postfix: `.path` < `[filter]` < `(invocation)` — left-to-right processing yields A2 naturally
- Between desugaring: `BinOp { op: Between, lhs: a, rhs: List([b, c]) }` — `and` NOT consumed by Pratt (parse_expr(31) stops before `and` at bp 20)
- Keyword-as-name disambiguation by position (Pitfall 3)
- Error recovery: `Err(span, msg)` nodes, sync to boundary tokens
- DoS guards: `max_depth` (default 256) → Err node; `max_tokens` on lexer (10_000)
- Err propagation in binary ops, parens, and postfix (don't wrap errors)
- `parse_simple_unary_tests`: `-` → `Literal(Bool(true))` sentinel, `not(...)` → `Neg(...)`, comma-list → Or-chain
- Type reference parsing for `instance of`: simple types + multi-word (`date and time`, `days and time duration`, `years and months duration`)
- All 33 tests pass (14 lexer + 19 parser), 76/76 FEEL tests, zero Rete regression

**Commit:** `0f6d65cfd` — `feat(31-01): implement recursive-descent + Pratt parser for grammar 1-65`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Lexer appends trailing Eof token, breaking dash sentinel check**
- **Found during:** Task 3 GREEN
- **Issue:** The lexer's `tokenize()` method appends a trailing `Token::Eof`, so `tokens.len()` for `"-"` is 2 (not 1). The initial dash sentinel check `p.tokens.len() <= 1` was always false.
- **Fix:** Changed to check that the first token is `Minus` and the second is `Eof` (or absent): `matches!(p.peek(), Token::Minus) && second_is_eof`
- **Files modified:** `engine/src/dmn/feel/parser.rs`
- **Commit:** `0f6d65cfd`

**2. [Rule 1 - Bug] Err nodes wrapped in BinOp/paren nodes instead of propagating**
- **Found during:** Task 3 GREEN
- **Issue:** `parse_expression("1 + ")` produced `BinOp(Add, 1, Err(...))` instead of `Err(...)` at top level. The depth guard test produced "expected ')'" instead of "max recursion depth exceeded" because Err nodes were being wrapped at each recursion level.
- **Fix:** Added Err propagation in three places: (a) Pratt loop binary ops — if RHS is Err, return it directly; (b) `parse_primary` for `(` — if inner is Err, return it without checking for `)`; (c) `parse_postfix` — if base is Err, skip the postfix loop entirely.
- **Files modified:** `engine/src/dmn/feel/parser.rs`
- **Commit:** `0f6d65cfd`

**3. [Rule 3 - Blocking] Duplicate Span import**
- **Found during:** Task 3 GREEN
- **Issue:** `Span` was already imported at line 37 (`use super::ast::Span;`), and the new parser section re-imported it, causing E0252.
- **Fix:** Removed `Span` from the new import line, keeping only the existing import.
- **Files modified:** `engine/src/dmn/feel/parser.rs`
- **Commit:** `0f6d65cfd`

### Minor Notes

- The `dashmap` acceptance criterion `grep -c 'name = "dashmap"' engine/Cargo.toml` returns 0 because `cargo add` writes `dashmap = "6.2.1"` inline (not the `name = "dashmap"` form which appears in `Cargo.lock`). The dependency IS present (`grep -c '^dashmap' engine/Cargo.toml` returns 1). This is a plan-verifier grep pattern mismatch; the intent is clear and the dependency is correctly added.

## TDD Gate Compliance

- **RED gate:** `test(31-01):` commit exists for both Task 2 (`6c2b63069`) and Task 3 (`4c48797ef`) — failing tests committed before implementation.
- **GREEN gate:** `feat(31-01):` commit exists after each RED commit — `3e78b8dc1` (Task 2 GREEN) and `0f6d65cfd` (Task 3 GREEN).
- All RED tests were verified to fail before GREEN implementation. All GREEN tests pass.

## Four Prohibitions Verification

1. **Never reuse Rust ==/Ord for FEEL equality** — No custom `PartialEq`/`Ord` impls for FEEL types in parser. The `#[derive(PartialEq)]` on `FeelExpression` is for test assertions only (AST structural equality), not FEEL value equality (which is `feel_equals`/`feel_compare` → `Option<_>` in 31-02).
2. **Never use f64** — All numeric literals use `Decimal` (`rust_decimal`). The `f64` matches in `parser.rs` are doc comments saying "never f64", not actual usage.
3. **No implicit number↔string conversion** — No `to_string().parse()` or `as f64` patterns in parser.
4. **FEEL path never routed through EngineRegistry/engine/src/engine/** — No references to `EngineRegistry`, `engine::engine`, or `crate::engine::engine` in any FEEL file.

## Self-Check: PASSED

All 5 task commits verified present in git log. All 11 files verified present on disk. All 76 FEEL tests + 33 parser/lexer tests pass. Zero Rete regression.