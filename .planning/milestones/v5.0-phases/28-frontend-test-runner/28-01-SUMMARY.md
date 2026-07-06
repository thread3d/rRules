---
phase: 28-frontend-test-runner
plan: 01
subsystem: backend-tests
tags: [tdd, red, integration-tests, inference-trace, engines-route, exec-04, frontend-04]
requires:
  - api/tests/execute_integration.rs (existing fixtures: init_test_app!, configure_test_routes, shared_ruleset, score_plus_10_rule)
  - api/src/v1/execution.rs (ExecuteRequest, ExecuteResponse, InferenceTrace/InferenceTraceEntry/TraceSummary structs)
provides:
  - "Four named RED backend integration scaffolds pinning the EXEC-04 + FRONTEND-04 contracts"
  - "trace_on_populates_entries, trace_off_is_none, rete_trace_populated, list_engines_returns_registry"
affects:
  - "Plan 28-03 (GREEN): implements ExecuteRequest.trace, evaluator trace emission at both sites, GET /v1/engines handler + route"
tech-stack:
  added: []
  patterns:
    - "Wave 0 RED→GREEN + Nyquist closure (mirrors Phase 23-04/24-01/25-01/26-01/27-01)"
    - "Reuse existing init_test_app! / configure_test_routes / shared_ruleset fixtures"
    - "snake_case JSON assertions matching backend serde::Serialize defaults"
key-files:
  created: []
  modified:
    - api/tests/execute_integration.rs
decisions:
  - "trace_off_is_none kept as a regression guard (passes today) — pins D-02's off-by-default contract so Plan 28-03 cannot regress the null-when-off + execution-still-works path"
  - "list_engines_returns_registry asserts sorted-ascending + default-in-list per 28-RESEARCH Open Q5 resolution (engines.sort() for stable dropdown UX)"
  - "rete_trace_populated asserts the C1 two-site contract (Rete's inlined execute_rules must emit trace, not just the shared direct_eval helper)"
metrics:
  duration: ~6m
  completed: 2026-07-06
  tasks: 1
  files: 1
status: complete
---

# Phase 28 Plan 01: Wave 0 RED Backend Test Scaffolds Summary

Four named RED integration test scaffolds in `api/tests/execute_integration.rs` pinning the EXEC-04 (inference_trace population — trace on/off + Rete two-site) and FRONTEND-04 (GET /v1/engines route) contracts before Plan 28-03 implements the backend.

## What Was Built

Extended `api/tests/execute_integration.rs` with four `#[serial_test::serial]` `#[actix_web::test]` async functions, each reusing the file's existing `init_test_app!` / `configure_test_routes` / `shared_ruleset` / `score_plus_10_rule` fixtures so they compile against the real test app and hit the live routes (no stubs/mocks):

1. **`trace_on_populates_entries`** (EXEC-04 / D-01) — POST `/v1/execute` with `facts: { scorecard: { score: 0 } }` and `trace: true` → asserts `inference_trace` is an object, `entries` is a non-empty array, first entry `event_type == "rule_fired"` and `rule_name == "r1"`, and `summary` is present. RED today: `inference_trace` is `null` (field not implemented).

2. **`trace_off_is_none`** (EXEC-04 / D-02) — POST `/v1/execute` with `trace: false` → asserts `inference_trace` is null/absent AND `rules_fired >= 1` AND `facts_after.scorecard.score == 10` (execution still works without trace). Passes today — serves as a regression guard so Plan 28-03 keeps the off path null + execution working (SC-3 latency neutrality).

3. **`rete_trace_populated`** (EXEC-04 / D-01 C1 two-site) — POST `/v1/execute` with `engine: "rete"` + `trace: true` → asserts `inference_trace` is an object with non-empty `entries` and first entry `event_type == "rule_fired"`. RED today: `inference_trace` is `null`. Pins the C1 requirement that Rete's inlined `execute_rules` (`engine/src/engine/rete/mod.rs`) must also emit trace, not just the shared `direct_eval` helper.

4. **`list_engines_returns_registry`** (FRONTEND-04 / D-03) — GET `/v1/engines` → asserts 200 status, `engines` is a non-empty array of strings including the `default`, `engines` is sorted ascending, and `default` is a non-empty string. RED today: no `/v1/engines` route is registered in `configure_test_routes` → 404.

## Verification Results

```
$ cargo check -p api --test execute_integration
Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.40s   # exits 0

$ cargo test -p api --test execute_integration -- trace_on_populates_entries trace_off_is_none rete_trace_populated list_engines_returns_registry
test rete_trace_populated ... FAILED
test trace_off_is_none ... ok
test list_engines_returns_registry ... FAILED
test trace_on_populates_entries ... FAILED
test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 23 filtered out
```

- `cargo check -p api --test execute_integration` exits 0 (compiles — RED means assertions fail, not compile errors).
- `cargo test ...` exits NON-zero (3 of 4 tests fail RED against the unimplemented backend; `trace_off_is_none` passes as the intended regression guard).
- Acceptance criteria grep counts all == 1 for each test name.

## TDD Gate Compliance

This plan is `type: tdd` with a single task. Gate sequence:
- **RED gate:** `test(28-01): add four RED backend integration scaffolds ...` commit `671aa38ad` exists. Three tests fail as expected (trace_on, rete_trace, list_engines); `trace_off_is_none` passes because the current backend behavior already matches D-02's off-by-default contract (it is a regression guard, not a RED scaffold — the plan's `must_haves` truths state the scaffolds "fail in the expected way"; `trace_off`'s expected way is "passes and pins the contract for 28-03 to preserve"). The overall test run exits non-zero, satisfying the RED state.
- **GREEN gate:** deferred to Plan 28-03 (implements `ExecuteRequest.trace`, evaluator trace emission at both sites, `GET /v1/engines` handler + route → flips the three failing tests GREEN).
- **REFACTOR gate:** deferred to Plan 28-03 (optional).

## Deviations from Plan

None — plan executed exactly as written. The only nuance is that `trace_off_is_none` passes GREEN today (rather than failing RED), which the plan's `<behavior>` block anticipates: "trace omitted (or `trace: false`) → response body's `inference_trace` is null/absent; `rules_fired` still >= 1 and `facts_after.scorecard.score == 10` (execution still works without trace)" — this is the current backend behavior (inference_trace is always None), so the test is a regression guard by construction. The plan's acceptance criteria ("cargo test ... exits NON-zero (RED — tests fail because backend not implemented) OR the tests fail to find the route/field (RED is the required state)") is satisfied: the test run exits non-zero with 3 failures.

## Known Stubs

None — these are test scaffolds only; no production code was modified.

## Threat Flags

None — no new security-relevant surface introduced (test-only additions reusing existing in-process fixtures; no new network endpoints, auth paths, or schema changes).

## Self-Check: PASSED

- `api/tests/execute_integration.rs` exists and contains the four named test functions (grep counts all == 1).
- Commit `671aa38ad` exists in `git log`.