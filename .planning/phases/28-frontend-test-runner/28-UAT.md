---
status: testing
phase: 28-frontend-test-runner
source: [28-VERIFICATION.md]
started: 2026-07-06T11:20:56Z
updated: 2026-07-06T11:20:56Z
---

# Phase 28 — User Acceptance Testing

> Human verification of the 3 live-server runtime invariants / manual flows that
> automated tests cannot exercise. Source: `28-VERIFICATION.md` (status: human_needed,
> 4/4 must-haves verified at code level, 2 behavior-unverified).

## Current Test

number: 1
name: SC-1 keystone click-through — Run renders real `facts_after`
expected: |
  Start the dev stack. Open the rule editor for the `score := score + 10` rule.
  Enter facts JSON (e.g. `{"scorecard": {"score": 0}}`), click "Run".
  The results panel renders `rules_fired` containing the rule, and
  `facts_after.scorecard.score == 10`.
awaiting: user response

## Tests

### 1. SC-1 keystone click-through — Run renders real `facts_after`
expected: Open the edit page for the `score := score + 10` rule, click Run, confirm the results panel shows `rules_fired` with the rule and `facts_after.scorecard.score == 10` (real execution, not a mocked response).
result: [pending]

### 2. SC-3 trace-off latency neutrality
expected: Benchmark `POST /v1/execute` with `trace` omitted vs `trace: true` against the Phase-24 baseline. Trace-off latency ≈ Phase-24 baseline (no allocation on the off path); the delta is within a loose bound. The mechanism (`collect_trace=false` skips all Vec/String/chrono allocation) is present and `trace_off_is_none` confirms the off path returns `null`, but a latency-neutral benchmark requires a manual measurement with a running server.
result: [pending]

### 3. SC-4 live reload-then-run + engine_generation correlation
expected: Start the dev stack, edit a rule (e.g. `score := score + 10`), save, click "Reload then Run". Confirm the results panel updates to reflect the edited rule and `engine_generation` shown is greater than the pre-reload generation captured by the panel. The component-level polling + correlation is GREEN in Vitest; the full live loop through the backend JobStore needs a running server.
result: [pending]

## Summary

total: 3
passed: 0
issues: 0
pending: 3
skipped: 0
blocked: 0

## Gaps