# Requirements: rRules - Business Rules Management System

**Defined:** 2026-07-02
**Core Value:** Users can define complex business decisions as reusable rules, deploy them instantly, and monitor their impact — all without modifying application code.
**Milestone:** v5.0 Live Execution — make rule execution real end-to-end so the v4.0 monitoring surfaces read live data.

## v5.0 Requirements

Requirements for the v5.0 "Live Execution" milestone. Each maps to a roadmap phase (Phases 23+; traceability populated during roadmap creation). Closes the `execute_rules` stub (`api/src/v1/execution.rs:307`) and wires execution → trackers → metrics → UI.

### Live Execution Core

- [ ] **EXEC-01**: `POST /v1/execute` fires real rules via the `EngineRegistry`/`ExecutionEngine` surface (stateless one-shot as the default mode) and returns a truthful `rules_fired` count and `rules_not_fired` names — not the current hardcoded `0`.
- [ ] **EXEC-02**: `facts_after` reflects post-inference working memory (inferred/modified facts serialized from the engine's `WorkingMemory`), not an echo of the request input.
- [ ] **EXEC-03**: `POST /v1/rules/batch-execute` drives real execution per item with per-item isolation (each item builds its own engine), inheriting the real `execute_single` path; `total_fired`/`succeeded`/`failed` aggregate truthfully.
- [ ] **EXEC-04**: Inference trace is surfaced on demand via an opt-in `trace` flag (off by default for performance); `ExecuteResponse.inference_trace` is populated when requested rather than always `None`.

### Engine Selection

- [ ] **ENGINE-07**: Per-request engine selection via an `engine` field on `ExecuteRequest`, validated against `EngineRegistry.available_engines()` (HTTP 400 on unknown engine name). Exercises the v3.0 Rete/TREAT/LEAPS/PHREAK selector.
- [ ] **ENGINE-08**: Config-defined default engine (env/config) used when the `engine` param is absent, so ops can set a default without code changes.

### Execution Sessions

- [ ] **SESS-01**: Stateful KIE-session-style sessions — multi-interaction execution (insert facts → fire → insert more → fire again → dispose) via `session_id`, backed by a shared `SessionStore` with TTL eviction, LRU/max-session bounds, and explicit dispose/reset endpoints. Stateless one-shot remains the default when `session_id` is absent.

### Hot Reload

- [ ] **DEPLOY-05**: Async reload-then-execute actually swaps the compiled engine (shared `RuleSet` snapshot, swap-in-place not in-place mutation) so subsequent executes run refreshed rules; `GET /v1/jobs/{id}` returns real reload job status (`Pending → Running → Completed/Failed`) instead of 404.

### Live Monitoring Data

- [ ] **MON-05**: `CoverageTracker` is auto-populated from every execute (single + batch) so coverage reporting reads live data. `record_evaluation`/`record_fire`/`record_skipped` are called exactly once per rule per execution (no double-counting).
- [ ] **MON-06**: `RuleExecutionTracker` is a process-wide singleton (`LazyLock<Mutex<…>>`, mirroring `CoverageTracking`) so dead-rule detection reads live firings — `GET /v1/metrics/dead-rules` no longer always returns `[]`.
- [ ] **MON-07**: Per-execution Prometheus metrics are emitted (latency histogram, `rules_fired` counter, engine label) so `/metrics` and the monitoring dashboard display real activity; `ExecutionCounters` broadcast an `EngineState` snapshot via the existing `metrics_tx` WebSocket channel after each execute.

### Frontend Test Runner

- [ ] **FRONTEND-03**: Run/Test button + results panel in the rule editor (`frontend/src/api/execution.ts` + a `TestRunner` component) that calls `/v1/execute` and renders `rules_fired`/`rules_not_fired`/`facts_after`, with the optional inference trace shown when requested. Monitoring dashboards need no protocol/UI changes (adapters already tolerate real shapes).
- [ ] **FRONTEND-04**: Run-panel engine selector dropdown (populated from available engines via a new `GET /v1/engines` endpoint), defaulting from config — making the pluggable engine a user-facing feature.

## v2 Requirements (v5.x / deferred)

Deferred per research prioritization — high value but separable subsystems; add once v5.0 P1 is stable.

### Engine Comparison

- **CMP-01**: Side-by-side engine comparison on one input (`POST /v1/execute/compare` runs the same facts through Rete AND TREAT AND LEAPS AND PHREAK, returning a per-engine diff of results/timing).

### Execution UX Enhancements

- **UX-01**: Decision-table row-match highlighting in the run panel (green matched / yellow evaluated-not-matched / red violated) — requires the engine to report per-row match outcomes.
- **UX-02**: Per-rule / per-node timing in the inference trace (`duration_us`), rendered as a timeline/flamebar.
- **UX-03**: Expected-vs-actual diff + "Save as test" — bridges execution to the v2.0 regression-testing framework via a `TestRun` entity + UI.

## Out of Scope

Explicitly excluded for v5.0 to prevent scope creep (anti-features from research).

| Feature | Reason |
|---------|--------|
| Always-on full inference trace | Doubles execution latency and bloats payloads; 90% of production executes don't need it. Opt-in via `trace` flag instead. |
| Cross-request shared working memory in stateless mode | Creates undeclared state, race conditions, and non-deterministic results. Use explicit stateful sessions instead. |
| Synchronous batch with shared engine/lock | Serializes items and defeats the v4.0 `Semaphore + JoinSet` concurrency; one panicking item poisons the shared engine. Keep per-item throwaway engines. |
| GraphQL / streaming execute response | Adds a transport/schema/auth layer for one feature; the audience is small. REST + JSON suffices; WebSocket already streams monitoring. |
| Auto-reload on every execute | Re-compiles the rule set per request (O(rules) per call), killing throughput. Reload stays event-driven. |
| Client-side rule execution in browser (WASM) | Duplicates the engine, drifts from the authoritative Rust implementation, and bypasses governance/audit/coverage. Two engines = two truths. |
| Live breakpoints / step-through debugger | Requires engine-level pause/resume, a debug protocol, and a UI state machine — a milestone of its own. The inference trace covers ~80% of debugging need. |

## Traceability

Populated during roadmap creation (Step 10). Each requirement maps to exactly one phase.

| Requirement | Phase | Status |
|-------------|-------|--------|
| EXEC-01 | Phase 23 | Pending |
| EXEC-02 | Phase 23 | Pending |
| EXEC-03 | Phase 25 | Pending |
| EXEC-04 | Phase 28 | Pending |
| ENGINE-07 | Phase 23 | Pending |
| ENGINE-08 | Phase 23 | Pending |
| SESS-01 | Phase 27 | Pending |
| DEPLOY-05 | Phase 25 | Pending |
| MON-05 | Phase 23 | Pending |
| MON-06 | Phase 26 | Pending |
| MON-07 | Phase 24 | Pending |
| FRONTEND-03 | Phase 28 | Pending |
| FRONTEND-04 | Phase 28 | Pending |

> Phase numbers above are provisional placeholders pending the roadmapper's phase-derivation pass (Step 10). The roadmapper owns final phase assignment and will overwrite this table.

**Coverage:**
- v5.0 requirements: 13 total
- Mapped to phases: 13 (provisional)
- Unmapped: 0 ✓

---
*Requirements defined: 2026-07-02*
*Last updated: 2026-07-02 after initial definition for v5.0 Live Execution*