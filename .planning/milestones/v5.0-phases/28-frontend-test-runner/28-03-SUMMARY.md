---
phase: 28-frontend-test-runner
plan: 03
subsystem: api
tags: [tdd, inference-trace, engines-route, exec-04, frontend-04, rete, direct-eval, c1-two-site, chrono]
requires:
  - phase: 28-01
    provides: "Four named RED backend integration scaffolds (trace_on_populates_entries, trace_off_is_none, rete_trace_populated, list_engines_returns_registry) pinning the EXEC-04 + FRONTEND-04 contracts"
  - phase: 23
    provides: "Real POST /v1/execute path (execute_single, direct_eval.rs, rete/mod.rs inlined loop, EngineRegistry, facts_after from working memory)"
  - phase: 26
    provides: "C1 two-site coverage recording pattern (direct_eval.rs + rete/mod.rs inlined execute_rules) that this plan mirrors for trace emission"
provides:
  - "Engine-side TraceEvent + TraceData types + EngineResult.trace: Option<TraceData> (engine/src/engine/registry.rs)"
  - "ExecutionEngine::set_collect_trace trait method (default noop) — the collect_trace threading path"
  - "Two-site ordered TraceEvent emission (direct_eval.rs C1 site 1 + rete/mod.rs C1 site 2) — firing order, not HashSet order"
  - "ExecuteRequest.trace: Option<bool> (#[serde(default)] off-by-default) — SC-3 latency-neutral opt-in"
  - "execute_single (stateless + sessionful) maps engine TraceData → api InferenceTrace (entries + summary), data: None lean payload"
  - "GET /v1/engines handler + EnginesResponse struct + route registration (main.rs + test fixture) — sorted engines + resolved default"
  - "Four Plan 28-01 RED scaffolds flipped GREEN"
affects:
  - "Plan 28-05 (frontend TestRunner): consumes GET /v1/engines for the dropdown + sends trace: true to populate the trace section"
  - "Plan 28-04 (e2e keystone): re-exercises the score:=score+10 rule through the trace-on path"
  - "Future v5.0 trace-richness work: data payload enrichment (deferred — D-01 discretion kept data: None for SC-3)"

tech-stack:
  added: []
  patterns:
    - "Engine-side trace type → API mapping (Critical Finding 1 — engine cannot import api types; engine emits TraceEvent, execute_single maps to InferenceTraceEntry)"
    - "C1 two-site emission mirrored from Phase-26 coverage recording (direct_eval.rs shared helper + rete/mod.rs inlined loop — Rete does NOT call direct_execute)"
    - "set_collect_trace(bool) trait method called BEFORE engine.execute() inside web::block — the threading path (mirrors set_config pattern)"
    - "Trace-off zero-allocation guard: every trace push/chrono call guarded by `if collect_trace` (SC-3 latency neutrality)"
    - "EnginesResponse sorted ascending for stable dropdown (28-RESEARCH Open Q5 — available_engines() iterates a HashMap)"

key-files:
  created: []
  modified:
    - engine/src/engine/registry.rs
    - engine/src/engine/direct_eval.rs
    - engine/src/engine/rete/mod.rs
    - engine/src/engine/treat/mod.rs
    - engine/src/engine/leaps/mod.rs
    - engine/src/engine/phreak/mod.rs
    - engine/src/engine/mod.rs
    - api/src/v1/execution.rs
    - api/src/main.rs
    - api/src/openapi.rs
    - api/tests/execute_integration.rs

key-decisions:
  - "Engine-side TraceEvent/TraceData types in registry.rs (not api types) — Critical Finding 1 dependency direction (api → engine, never reverse)"
  - "set_collect_trace default noop trait method + per-engine override storing a collect_trace: bool field — mirrors the set_config pattern (28-RESEARCH Open Q1 resolution)"
  - "direct_execute signature gains collect_trace: bool as a third param (free function — cleaner than a field); returns (HashSet, HashSet, Option<TraceData>)"
  - "ReteEngine mirrors trace emission inline in its inlined execute_rules (C1 site 2 — Rete does NOT call direct_execute, so emission is independent)"
  - "data: None per D-01 discretion (lean payload — T-28-03-trace-leak mitigation); facts_matched counts facts in working memory at fire time (honest count)"
  - "TraceData built only when collect_trace && !events.is_empty() — None otherwise (SC-3: trace-off path never allocates the Vec)"
  - "EnginesResponse struct (cleaner than inline json! — 28-RESEARCH Pattern 4 recommendation); engines.sort() for stable dropdown"
  - "Sessionful path also populates inference_trace (do NOT leave :950 as None — sessionful path benefits from trace too)"

patterns-established:
  - "Pattern: engine-side trace type → API wire-schema mapping at the execute_single boundary (reusable for future trace-richness)"
  - "Pattern: set_collect_trace(bool) before web::block(engine.execute()) — the canonical trace threading path"
  - "Pattern: C1 two-site emission (direct_eval shared helper + rete inlined loop) — mirrors Phase-26 coverage recording"

requirements-completed: [EXEC-04, FRONTEND-04]

coverage:
  - id: D1
    description: "Engine-side TraceEvent/TraceData types + EngineResult.trace field + set_collect_trace trait method + two-site ordered emission (direct_eval.rs + rete/mod.rs)"
    requirement: EXEC-04
    verification:
      - kind: unit
        ref: "cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak -- --test-threads=1 (219 passed)"
        status: pass
  - id: D2
    description: "POST /v1/execute with trace: true returns inference_trace populated with ordered entries (event_type 'rule_fired', rule_name, timestamp) + summary"
    requirement: EXEC-04
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs#trace_on_populates_entries"
        status: pass
  - id: D3
    description: "POST /v1/execute with trace omitted/false returns inference_trace: null AND rules_fired/facts_after unchanged (SC-3 latency neutrality — regression guard)"
    requirement: EXEC-04
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs#trace_off_is_none"
        status: pass
  - id: D4
    description: "Rete engine (inlined execute_rules) populates inference_trace when trace: true — C1 two-site requirement"
    requirement: EXEC-04
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs#rete_trace_populated"
        status: pass
  - id: D5
    description: "GET /v1/engines returns 200 with sorted engines array + resolved default (FRONTEND-04 backend half)"
    requirement: FRONTEND-04
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs#list_engines_returns_registry"
        status: pass

duration: 16m
completed: 2026-07-06
status: complete
---

# Phase 28 Plan 03: Backend Trace Population + GET /v1/engines Summary

Engine-side `TraceEvent`/`TraceData` types with two-site ordered emission (direct_eval.rs C1 site 1 + rete/mod.rs C1 site 2), mapped to the api `InferenceTrace` wire schema in `execute_single` (stateless + sessionful), opt-in via `trace: Option<bool>` off-by-default for SC-3 latency neutrality; new `GET /v1/engines` route returns the sorted registry + resolved default.

## Performance

- **Duration:** 16 min
- **Started:** 2026-07-06T09:55:32Z
- **Completed:** 2026-07-06T10:11:34Z
- **Tasks:** 2
- **Files modified:** 11

## Accomplishments

- Added `TraceEvent { rule_id, timestamp, event_type: "rule_fired" }` and `TraceData { events, total_duration_ms, facts_matched }` to `engine/src/engine/registry.rs`; added `EngineResult.trace: Option<TraceData>` (None when collect_trace false); added `ExecutionEngine::set_collect_trace` default noop trait method.
- Threaded `collect_trace: bool` into `direct_execute` (C1 site 1 — Treat/Leaps/Phreak); emits one `TraceEvent` per fired rule IN FIRING ORDER when true, zero allocation when false (SC-3). Mirrored the emission inline in `ReteEngine::execute_rules` (C1 site 2 — Rete does NOT call `direct_execute`, so emission is independent).
- Added `ExecuteRequest.trace: Option<bool>` (`#[serde(default)]` off-by-default); `execute_single` resolves `collect_trace = req.trace.unwrap_or(false)`, calls `engine.set_collect_trace(collect_trace)` before `web::block`, maps `engine_result.trace` → `InferenceTrace` (entries + summary, `data: None`), and replaces both `inference_trace: None` sites (stateless `:730` + sessionful `:950`).
- Added `list_engines` handler + `EnginesResponse` struct; `engines.sort()` for stable dropdown; `default` mirrors `execute_single` resolution (`RRULES_DEFAULT_ENGINE` env > `selector.current_engine()`). Registered `GET /v1/engines` in `main.rs` /v1 scope + `configure_test_routes` fixture + OpenAPI paths/schemas.
- Flipped the four Plan 28-01 RED scaffolds GREEN: `trace_on_populates_entries`, `trace_off_is_none` (regression guard preserved), `rete_trace_populated` (C1 rete site), `list_engines_returns_registry`.

## Verification Results

```
$ cargo test -p api --test execute_integration -- trace_on_populates_entries trace_off_is_none rete_trace_populated list_engines_returns_registry
test trace_off_is_none ... ok
test list_engines_returns_registry ... ok
test rete_trace_populated ... ok
test trace_on_populates_entries ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 23 filtered out

$ cargo test -p api --test execute_integration
test result: ok. 26 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out

$ cargo test -p api  # all api suites
test result: ok (all suites green — no regressions to Phase-23/24/25/26/27 tests)

$ cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak -- --test-threads=1
test result: ok. 219 passed; 0 failed; 0 ignored (no engine regressions)
```

- `cargo check -p engine` exits 0; `cargo check -p api` exits 0.
- SC-3 latency neutrality: trace-off path performs no trace allocation (every `TraceEvent` push / `chrono::Utc::now()` call guarded by `if collect_trace`; `trace_events` Vec is empty and `TraceData` is never constructed when false).
- L-02 honored: `engine/src/engine/chaining/mode_selector.rs` NOT touched (grep-verified: `git diff HEAD~1 -- engine/src/engine/chaining/mode_selector.rs` is empty).
- `trace_off_is_none` regression guard still passes (trace omitted/false → `inference_trace: null`, `rules_fired >= 1`, `facts_after.scorecard.score == 10`).

## TDD Gate Compliance

This plan is `type: tdd` with two tasks. Gate sequence:
- **RED gate:** Plan 28-01 (commit `671aa38ad`) established the four RED scaffolds — three failing (`trace_on`, `rete_trace`, `list_engines`), one passing as a regression guard (`trace_off`). The RED state was verified in the 28-01-SUMMARY.
- **GREEN gate:** This plan (commits `04b1c884e` + `e49f89867`) flips the three failing tests GREEN while preserving the `trace_off_is_none` regression guard. All four target tests now pass.
- **REFACTOR gate:** No separate refactor commit needed — the implementation is clean (engine-side type + mapping, no duplication beyond the required C1 two-site mirror).

## Deviations from Plan

None — plan executed exactly as written. Both tasks followed the plan's `<action>` blocks precisely:
- Task 1: engine-side `TraceEvent`/`TraceData` in registry.rs; `set_collect_trace` default noop; `direct_execute` threads `collect_trace` + emits ordered events; `ReteEngine::execute_rules` mirrors inline (C1 site 2); Treat/Leaps/Phreak thread the flag + attach the returned `TraceData`.
- Task 2: `ExecuteRequest.trace` `#[serde(default)] Option<bool>`; `execute_single` + `execute_sessionful` resolve `collect_trace`, call `set_collect_trace`, map `TraceData` → `InferenceTrace`, replace both `None` sites; `list_engines` handler + `EnginesResponse` struct + `engines.sort()` + route registration in `main.rs` + test fixture.

One pre-existing test isolation issue was observed (out of scope — not a deviation):
- `cargo test -p engine --features all` (parallel) has one flaky failure: `monitoring::coverage::tests::test_coverage_tracking_fire` — the `CoverageTracking` singleton is not isolated across parallel tests, so another test recording `"rule-1"` before `clear()` can cause `evaluation_count == 2`. This passes in isolation (`--test-threads=1`: 219 passed, 0 failed). My changes do NOT touch `CoverageTracking` recording (trace emission is additive and guarded by `collect_trace`); this is a pre-existing flake in the engine test suite.

## Known Stubs

None — the trace `data` payload is `None` per D-01 discretion (lean payload for SC-3 latency neutrality — explicitly deferred per 28-CONTEXT "Claude's Discretion"). This is an intentional design decision, not a stub: the contract is one `InferenceTraceEntry` per fired rule with `event_type` + `rule_name` + `timestamp`, and `data: None` is the chosen lean payload. A rich fact/action snapshot is a future phase (28-CONTEXT "Deferred Ideas": "Trace `data` payload richness").

## Threat Flags

None beyond the plan's `<threat_model>`. The mitigations are implemented as specified:
- **T-28-03-trace-leak** (Information Disclosure): `data: None` per D-01 discretion — no fact/action snapshot leaks via trace. ✓
- **T-28-03-trace-dos** (Denial of Service): trace is opt-in (`trace: Option<bool>`, off by default); one entry per fired rule bounded by `|rules|`; `RRULES_MAX_CHAIN_ITERATIONS` cap bounds the fixpoint loop; zero allocation when off. ✓
- **T-28-03-engine-enum** (Information Disclosure): `GET /v1/engines` is behind the existing `requires_auth` middleware on the `/v1` scope; response contains only engine names + default (no sensitive metadata). ✓
- **T-28-03-stale-result** (Tampering): `engine_generation` already populated (Phase 25); trace entries are append-only per execution. ✓

## Self-Check: PASSED

- `engine/src/engine/registry.rs` contains `pub struct TraceEvent` (grep count 1), `pub struct TraceData` (1), `pub trace: Option<TraceData>` (1), `fn set_collect_trace` (1).
- `engine/src/engine/direct_eval.rs` threads `collect_trace` (grep 11) and calls `chrono::Utc::now` (1).
- `engine/src/engine/rete/mod.rs` threads `collect_trace` (14) and calls `chrono::Utc::now` (1) — C1 site 2.
- `api/src/v1/execution.rs` has `pub trace: Option<bool>` (1), `collect_trace = req.trace.unwrap_or(false)` (2 — stateless + sessionful), `set_collect_trace` calls (2), `pub async fn list_engines` (1), `struct EnginesResponse` (1), `engines.sort()` (1), and zero remaining `inference_trace: None` sites.
- `api/src/main.rs` has `"/engines"` route (1).
- Commit `04b1c884e` (Task 1) exists in `git log`.
- Commit `e49f89867` (Task 2) exists in `git log`.
- The four target tests pass (`cargo test -p api --test execute_integration -- trace_on_populates_entries trace_off_is_none rete_trace_populated list_engines_returns_registry` exits 0).