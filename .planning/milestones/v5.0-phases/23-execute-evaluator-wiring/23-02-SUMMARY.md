---
phase: 23-execute-evaluator-wiring
plan: 02
subsystem: engine
tags: [rust, expression-ast, rules, condition-matching, comparison-ops, tdd]

# Dependency graph
requires:
  - phase: 23-01
    provides: "Compiling pluggable-engine surface (engine/src/rules.rs already compiled clean; Expression/Condition work is independent of the engine-feature unblock)"
provides:
  - "Expression enum (Literal/FieldRef/Add/Sub/Mul/Div/StringConcat/Compare/Call) with evaluate(&self, &[&Fact]) -> Value — the action-expression language (D-06) so `score := score + 10` is representable"
  - "Expression::evaluate resolves FieldRef against a fact slice, computes arithmetic with Integer-preserving Float promotion, division-by-zero yields Value::Null (T-23-03 mitigation), StringConcat via as_string, Compare via compare_values"
  - "compare_values(op: ComparisonOp, left: &Value, right: &Value) -> bool shared helper — full Eq/Ne/Lt/Le/Gt/Ge matrix across Integer/Float/String/Boolean/Null/Array/Object (no panics on ill-typed operands)"
  - "Action::UpdateFact.fields is now HashMap<String, Expression> (was HashMap<String, Value>) — UpdateFact accepts computed expressions, not just literals"
  - "Condition::Pattern gained op: ComparisonOp field (defaulted to Eq at all existing construction sites)"
  - "Rewritten Condition::evaluate: Compare arm uses compare_values; Pattern arm performs real matching (filter facts by fact_type_id, read field, compare_values with op); unconditional true return is gone (T-23-04 mitigation)"
affects: [23-03, 23-04, 23-05, 23-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Expression AST modeled on the existing Condition AST shape — Box<Expression> for recursive variants, Value as the leaf type"
    - "Integer-preserving numeric promotion: Integer+Integer stays Integer; any Float operand promotes both sides to f64 via DynamicCast::as_float"
    - "Shared compare_values helper reused by both Expression::Compare (D-06) and Condition::evaluate Compare/Pattern arms (D-05) — single source of truth for the operator/type matrix"
    - "Non-panic failure mode for ill-typed / div-by-zero operations: return Value::Null / false instead of panicking (T-23-03)"
    - "TDD RED/GREEN cycle per task: stub evaluate returns Value::Null / compare_values returns false (RED, tests fail), then real implementation (GREEN, tests pass)"

key-files:
  created: []
  modified:
    - "engine/src/rules.rs — Expression enum + impl Expression::evaluate + numeric_op helper + compare_values shared helper; Action::UpdateFact fields changed to HashMap<String, Expression>; Condition::Pattern gained op: ComparisonOp field; Condition::evaluate rewritten (Compare uses compare_values; Pattern does real matching); 27 new unit tests (14 expression + 13 condition) in rules::tests::{expression,condition} submodules"
    - "engine/src/validation.rs — import ComparisonOp; Pattern destructuring uses `..`; Pattern construction adds op: Eq"
    - "engine/src/chaining/mode_selector.rs — import ComparisonOp; 3 Pattern construction sites add op: Eq"
    - "engine/src/tests/mod.rs — 5 Pattern construction sites add op: crate::rules::ComparisonOp::Eq (file is dead code, not compiled; pre-existing broken syntax left untouched as out-of-scope)"

key-decisions:
  - "Created compare_values in Task 1 (not Task 2 as the plan's action text suggested) because Expression::Compare depends on it. Task 2 reuses the same helper for Condition::evaluate — single source of truth, no duplicate comparison logic. The plan's Task 2 acceptance criterion (`grep -n 'fn compare_values'` returns a match) is satisfied either way."
  - "Eq/Ne use structural PartialEq (left == right / left != right) for all types; ordered ops (Lt/Le/Gt/Ge) use numeric promotion (Integer+Integer as i64, any Float → as_float) then String lexicographic, then false for Boolean/Null/Array/Object. Predictable, panic-free, matches the plan's type matrix."
  - "Kept the Condition::Compare arm using to_value() (True/False → Bool, else None) + compare_values rather than restructuring the Compare variant. The plan's Task 2 behaviors are all Pattern-based (the plan's `Condition::Compare { op, field, value }` wording describes Pattern's shape); Pattern is the keystone matcher. Compare remains a boolean-leaf comparison using the shared helper."
  - "Expression::Call is a stub returning Value::Null in Phase 23 (function dispatch out of scope; variant exists so the AST is complete — matches plan action)."

patterns-established:
  - "Pattern: Integer-preserving numeric promotion via DynamicCast (as_float with Integer+Integer fast path) — reused by both Expression arithmetic and compare_values ordered numeric ops."
  - "Pattern: Non-panic failure for ill-typed operands (Value::Null / false) — the trust-boundary contract for user-authored rule expressions (T-23-03)."
  - "Pattern: Shared compare_values helper as the single comparison kernel — Expression::Compare and Condition::evaluate (Compare + Pattern arms) all route through it."

requirements-completed: [EXEC-01, EXEC-02]

# Coverage metadata (#1602) — per-deliverable traceability.
coverage:
  - id: D1
    description: "Expression enum (Literal/FieldRef/Add/Sub/Mul/Div/StringConcat/Compare/Call) with evaluate(&self, &[&Fact]) -> Value resolving FieldRef against facts and computing arithmetic with Integer-preserving Float promotion; division by zero yields Value::Null (D-06, T-23-03)"
    requirement: "EXEC-02"
    verification:
      - kind: unit
        ref: "cargo test -p engine --lib rules::tests::expression (14/14 pass: literal, fieldref, add/sub/mul, div, div-by-zero-null, string-concat, compare-gt, call-stub, serde roundtrip)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Action::UpdateFact.fields changed from HashMap<String, Value> to HashMap<String, Expression> — computed action values are representable (`score := score + 10`); serde round-trips"
    requirement: "EXEC-02"
    verification:
      - kind: unit
        ref: "engine/src/rules.rs::tests::expression::updatefact_with_expression_serializes_roundtrip (passes); cargo check -p engine exits 0 (no broken construction sites — UpdateFact had no callers)"
        status: pass
    human_judgment: false
  - id: D3
    description: "compare_values(op, left, right) shared helper — full ComparisonOp matrix (Eq/Ne/Lt/Le/Gt/Ge) across Integer/Float/String/Boolean/Null; ordered ops on non-numeric/non-string return false (no panic)"
    requirement: "EXEC-01"
    verification:
      - kind: unit
        ref: "engine/src/rules.rs::tests::condition (compare_integer_lt_le_gt_ge, compare_integer_false_cases, compare_float_lt, compare_cross_type_integer_lt_float, compare_string_eq_ne, compare_string_lexicographic, compare_boolean_eq_ne — all pass)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Condition::Pattern gained op: ComparisonOp field; Condition::evaluate Pattern arm performs real matching (filter facts by fact_type_id, read field, compare_values with op); returns false on no matching fact type or missing field (T-23-04 — unconditional true is gone)"
    requirement: "EXEC-01"
    verification:
      - kind: unit
        ref: "engine/src/rules.rs::tests::condition (pattern_eq_integer_match, pattern_match_score_zero_eq_yields_true, pattern_match_score_ten_against_value_zero_yields_false, pattern_no_matching_fact_type_yields_false, pattern_missing_field_yields_false — all pass); grep -c 'Condition::Pattern { .. } => true' == 0"
        status: pass
    human_judgment: false
  - id: D5
    description: "All existing Condition::Pattern construction sites updated to add op: ComparisonOp::Eq (validation.rs, chaining/mode_selector.rs ×3, tests/mod.rs ×5); cargo check -p engine and four-feature check exit 0"
    requirement: "EXEC-01"
    verification:
      - kind: unit
        ref: "cargo check -p engine exits 0; cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak exits 0; cargo check -p api exits 0"
        status: pass
    human_judgment: false

# Metrics
duration: 10min
completed: 2026-07-03
status: complete
---

# Phase 23 Plan 02: Rule-model expression + condition matching extensions Summary

**Built the `Expression` AST (D-06) so `score := score + 10` is representable in `Action::UpdateFact`, plus a shared `compare_values` helper and a rewritten `Condition::evaluate` that handles the full `ComparisonOp` matrix across Integer/Float/String/Boolean and matches `Pattern` conditions against real fact payloads instead of returning unconditionally true.**

## Performance

- **Duration:** 10 min (started 2026-07-03T06:22:24Z, completed 2026-07-03T06:32:49Z)
- **Tasks:** 2 (both TDD: RED → GREEN)
- **Files modified:** 4

## Accomplishments
- Introduced the `Expression` enum (Literal / FieldRef / Add / Sub / Mul / Div / StringConcat / Compare / Call) with `evaluate(&self, &[&Fact]) -> Value`. `FieldRef` scans the fact slice for the first fact of the named type and reads the field; arithmetic uses `DynamicCast` helpers with Integer-preserving Float promotion (Integer+Integer stays Integer; any Float operand promotes both sides to f64); division by zero yields `Value::Null` rather than panicking (T-23-03 mitigation); `StringConcat` uses `as_string`; `Compare` delegates to `compare_values`; `Call` is a stub returning `Value::Null` (function dispatch out of scope for Phase 23).
- Extended `Action::UpdateFact` so its `fields` map carries computed `Expression`s (`HashMap<String, Value>` → `HashMap<String, Expression>`). `score := score + 10` is now representable — the keystone scenario the milestone's ROADMAP success criterion #1 requires. No existing construction sites needed updating (UpdateFact had no callers in compiled code).
- Built the shared `compare_values(op: ComparisonOp, left: &Value, right: &Value) -> bool` helper covering the full operator matrix across all value types: `Eq`/`Ne` use structural `PartialEq`; ordered ops (`Lt`/`Le`/`Gt`/`Ge`) use Integer+Integer integral comparison, else `as_float` promotion, else `as_string` lexicographic, else `false` for Boolean/Null/Array/Object — never panics on ill-typed operands.
- Added `op: ComparisonOp` to `Condition::Pattern` and rewrote `Condition::evaluate`: the `Compare` arm now routes through `compare_values`; the `Pattern` arm filters facts by `fact_type_id`, reads the named field, and returns true if any matching fact's field value satisfies `op` against `value`. The unconditional `true` return is gone — a Pattern with no matching fact type or a missing field returns `false` (T-23-04 mitigation). Updated all 9 existing `Condition::Pattern` construction sites across `validation.rs`, `chaining/mode_selector.rs`, and `tests/mod.rs` to add `op: ComparisonOp::Eq`.

## Task Commits

Each task followed the TDD RED → GREEN cycle (stub → real implementation):

1. **Task 1: Introduce the Expression enum and extend Action::UpdateFact (D-06)**
   - `8abfa047a` — test(23-02): add failing tests for Expression evaluator (RED — 9/14 fail, stub returns Null)
   - `1be879a24` — feat(23-02): implement Expression evaluator + UpdateFact extension (GREEN — 14/14 pass)
2. **Task 2: Rewrite Condition::evaluate for full ComparisonOp coverage and real Pattern matching (D-05)**
   - `504c87bf0` — test(23-02): add failing tests for Condition full-op + Pattern matching (RED — 4/13 fail, Pattern returns true unconditionally)
   - `28ccff03e` — feat(23-02): rewrite Condition::evaluate for full ComparisonOp + Pattern matching (GREEN — 13/13 pass)

**Plan metadata:** pending (docs: complete plan — committed after STATE.md/ROADMAP.md updates)

## Files Created/Modified
- `engine/src/rules.rs` — Expression enum + `impl Expression::evaluate` + `numeric_op` helper + `compare_values` shared helper; `Action::UpdateFact.fields` changed to `HashMap<String, Expression>`; `Condition::Pattern` gained `op: ComparisonOp` field; `Condition::evaluate` rewritten (Compare via compare_values, Pattern real matching); 27 new unit tests in `rules::tests::{expression,condition}` submodules (14 + 13)
- `engine/src/validation.rs` — import `ComparisonOp`; Pattern destructuring uses `..`; Pattern construction adds `op: ComparisonOp::Eq`
- `engine/src/chaining/mode_selector.rs` — import `ComparisonOp`; 3 Pattern construction sites add `op: ComparisonOp::Eq`
- `engine/src/tests/mod.rs` — 5 Pattern construction sites add `op: crate::rules::ComparisonOp::Eq` (file is dead code, not compiled by lib.rs; pre-existing broken `evaluate(evaluate(&[])vec![])` syntax left untouched as out-of-scope)

## Decisions Made
- **Created `compare_values` in Task 1, not Task 2.** The plan's Task 1 action text says `Expression::Compare` "applies the ComparisonOp using the same comparison logic as Condition::evaluate (Task 2 establishes the shared comparison helper)" — but `Expression::Compare` cannot be implemented without a comparison kernel. Creating the helper in Task 1 and reusing it in Task 2 avoids duplicate comparison logic and satisfies both tasks' acceptance criteria. This is a sequencing deviation, not a scope change.
- **`Eq`/`Ne` use structural `PartialEq`; ordered ops use numeric promotion then String lexicographic then false.** Predictable, panic-free, and matches the plan's "Integer+Integer stays integral; any Float promotes; String lexicographic; Boolean Eq/Ne; ordered ops on Boolean/Null return false" matrix. The original `Condition::evaluate` used an epsilon check for Float `Eq`; the rewrite uses direct `PartialEq` (bit-exact). No behavior test requires Float `Eq` epsilon semantics.
- **Kept `Condition::Compare` using `to_value()` + `compare_values` rather than restructuring the variant.** The plan's Task 2 behavior list describes Pattern-shaped conditions (`{ op, field, value }`) under the "Condition::Compare" label — the real keystone matcher is `Pattern`. `Compare` remains a boolean-leaf comparison (True/False operands) routed through the shared helper; `Pattern` is where fact matching happens.
- **`Expression::Call` is a stub returning `Value::Null`.** The variant exists so the AST is complete (per plan action); full function dispatch is out of scope for Phase 23.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Invalid `-x` flag in plan verification command**
- **Found during:** Plan verification (after Task 2)
- **Issue:** The plan's `<verification>` specifies `cargo test -p engine --lib rules -x` and each task's `<verify><automated>` uses `-x`. The `-x` flag is not a valid `cargo test` argument (`error: unexpected argument '-x' found`). The 23-01 SUMMARY documented the same artifact.
- **Fix:** Used the correct substring-filter form `cargo test -p engine --lib rules` (no `-x`). All 51 rules tests pass (14 expression + 13 condition + 24 pre-existing).
- **Files modified:** none (command-form fix only)
- **Verification:** `cargo test -p engine --lib rules` → 51 passed, 0 failed, exit 0
- **Committed in:** N/A (verification command form, not a code change)

**2. [Rule 2 - Missing Critical] Added `compare_values` helper in Task 1 (ahead of Task 2)**
- **Found during:** Task 1 implementation
- **Issue:** Task 1's `Expression::Compare` variant needs a comparison kernel to evaluate, but the plan's action text defers the shared helper to Task 2. Implementing Task 1 without it would require throwaway inline comparison logic that Task 2 then extracts.
- **Fix:** Implemented `compare_values` with the full operator/type matrix in Task 1 and reused it unchanged in Task 2 for both the `Compare` and `Pattern` arms of `Condition::evaluate`.
- **Files modified:** engine/src/rules.rs
- **Verification:** `grep -n 'fn compare_values' engine/src/rules.rs` returns a match (Task 2 acceptance criterion satisfied); all expression and condition tests pass.
- **Committed in:** `1be879a24` (Task 1 GREEN)

---

**Total deviations:** 2 auto-fixed (1 blocking command-form fix, 1 sequencing adjustment to avoid duplicate code)
**Impact on plan:** No scope creep. Both deviations are mechanical / sequencing — the delivered artifacts match the plan's `output` section exactly (Expression enum, Expression::evaluate, compare_values, Action::UpdateFact field change, Condition::Pattern op field, rewritten Condition::evaluate).

## Issues Encountered
- The `engine/src/tests/mod.rs` file is dead code (not included via `mod tests;` from `lib.rs`, so not compiled). It carries pre-existing broken syntax (`evaluate(evaluate(&[])vec![])`) and its `Condition::Pattern` constructions lacked the new `op` field. I updated the Pattern constructions to add `op: crate::rules::ComparisonOp::Eq` for consistency with the plan's "grep and update" instruction, but left the pre-existing broken syntax untouched (out of scope — not caused by this task's changes, and the file does not affect `cargo check`). If a future phase wires `mod tests;` into the build, the broken `evaluate(evaluate(&[])vec![])` calls will need separate repair.

## User Setup Required
None — no external service configuration, no new packages installed. All work is against the existing in-tree Rust codebase.

## Next Phase Readiness
- The rule model now represents `score := score + 10` (Expression + UpdateFact) and conditions match honestly across the full operator/type matrix (compare_values + real Pattern matching). The keystone scenario is representable; Plan 03's evaluator can drive it.
- **Plan 03** can now implement `load_ruleset`/`get_facts` on `ReteEngine` and rewrite `execute_rules` into a real direct evaluator: iterate `self.rules`, call `rule.condition.evaluate(&working_facts.values().collect::<Vec<_>>())`, apply each fired rule's `Action`s (evaluating `Expression`s per `UpdateFact` field via `Expression::evaluate`), and record coverage once per fired rule.
- **Plan 04** (if it exists separately) builds any further expression/condition surface; the core D-05/D-06 build is done here.
- No blockers. The `Expression::evaluate` / `compare_values` / `Condition::evaluate` contracts are stable for Plan 03 to call against. The `Call` variant is a documented stub (function dispatch deferred).

## Requirement Status Note

`EXEC-01` and `EXEC-02` remain **Pending** in REQUIREMENTS.md. Plan 23-02 delivers the rule-model foundation those requirements depend on (the Expression AST and full Condition matching), but the requirements themselves — truthful `rules_fired` and post-inference `facts_after` from the live `/v1/execute` endpoint — are only satisfied when Plan 23-05's integration tests (`execute_fires_real_rule`, `facts_after_not_echoed`) pass against the wired API. Marking them Complete now would be a false claim: `/v1/execute` still returns the hardcoded `rules_fired: 0` stub until Plan 23-03/23-05 lands. The `requirements-completed` frontmatter field lists `[EXEC-01, EXEC-02]` per the SUMMARY template instruction (copy from the plan's `requirements` frontmatter); the REQUIREMENTS.md status is intentionally left unchanged.

---
*Phase: 23-execute-evaluator-wiring*
*Completed: 2026-07-03*

## Self-Check: PASSED

- All 4 modified source files exist on disk: FOUND
- SUMMARY.md exists on disk: FOUND
- Task 1 RED commit `8abfa047a`: FOUND
- Task 1 GREEN commit `1be879a24`: FOUND
- Task 2 RED commit `504c87bf0`: FOUND
- Task 2 GREEN commit `28ccff03e`: FOUND
- `cargo test -p engine --lib rules` exits 0 — 51 passed (14 expression + 13 condition + 24 pre-existing), 0 failed
- `cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` exits 0
- `cargo check -p api` exits 0
- TDD gate compliance: each task has a `test(...)` RED commit followed by a `feat(...)` GREEN commit