---
active: true
iteration: 1
session_id: 8e79bdb1-6230-45ea-8ea9-426fe94a4743
max_iterations: 10
completion_promise: "DONE"
started_at: "2026-06-09T08:12:23Z"
---

Analyse the entire codebase and write fully comprehensive tests, then fix every issue discovered.
Work through this in strict phases:
Phase 1 — Codebase audit
Read every source file. Build a complete picture of: all public APIs and entry points, all data structures and their invariants, all computation/evaluation logic, rendering/display pipeline, error handling paths, and any parsing or input processing.
Phase 2 — Test plan
Before writing a single test, output a structured test plan covering:

Unit tests for every discrete function and transformation
Integration tests for multi-step evaluation pipelines
Edge cases: empty input, malformed expressions, division by zero, deeply nested structures, unicode, very large/small numbers
Boundary and stress tests where applicable
Regression anchors for any logic that looks fragile

Get confirmation before proceeding, or proceed automatically if the codebase is small enough to hold in context.
Phase 3 — Test implementation
Write the full test suite using the most appropriate framework for the language(s) present. Tests must be:

Deterministic and isolated (no shared mutable state between tests)
Self-describing (test names read as specifications)
Comprehensive: aim for 100% coverage of all non-trivial logic paths

Phase 4 — Run the tests
Execute the full suite. Capture all failures, errors, and unexpected output. Do not skip flaky tests — investigate them.
Phase 5 — Fix all issues
For every failure: diagnose the root cause, implement a fix, re-run the relevant tests to confirm green. Do not mask failures with try/catch or by weakening assertions. If a fix is non-trivial, explain the reasoning briefly.
Phase 6 — Final report
Produce a summary: total tests written, pass/fail counts, issues found and fixed, any known limitations or areas needing human review.
