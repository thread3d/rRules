# Phase 28: Frontend test-runner - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-06
**Phase:** 28-frontend-test-runner
**Mode:** `--auto` (no interactive user turns — recommended option auto-selected for each gray area; logged inline)
**Areas discussed:** inference_trace population source, trace opt-in flag, GET /v1/engines endpoint, TestRunner placement + rule source, rule save/edit dependency, facts editor UX, Reload then Run flow, engine-selector dropdown, results panel rendering, execution.ts API client + types

---

## inference_trace population source (EXEC-04)

| Option | Description | Selected |
|--------|-------------|----------|
| Instrument the live direct evaluator | Emit one real `InferenceTraceEntry` per fired rule when `trace == true`; `None` otherwise | ✓ |
| Derive from `rules_not_fired`/`CoverageTracking` | Synthesize trace entries from existing fields (23-RESEARCH: "adds no value over rules_fired/rules_not_fired") | |
| Hybrid | Real per-fired-rule entries + coverage-derived summary | |

**User's choice:** [auto] Instrument the live direct evaluator (recommended — 23-RESEARCH Flag 3 assigns population to Phase 28; user's v5.0 posture is the honest path; a derived trace duplicates existing fields).
**Notes:** Do NOT wire `InferenceTraceCollector` from `chaining/mode_selector.rs` (coupled to the stub `environment::Engine` surface — L-02).

---

## `trace` opt-in flag + performance guard

| Option | Description | Selected |
|--------|-------------|----------|
| `trace: Option<bool>` off by default; evaluator skips trace work unless `Some(true)` | Opt-in per run; SC-3 latency unaffected when off | ✓ |
| `trace: bool` always collected, conditionally serialized | Always pays the collection cost | |
| Query-param / header opt-in | Non-body flag | |

**User's choice:** [auto] `trace: Option<bool>` on `ExecuteRequest`, off by default; evaluator skips trace collection unless `Some(true)` (recommended — ROADMAP: "off by default for performance").

---

## `GET /v1/engines` backend endpoint

| Option | Description | Selected |
|--------|-------------|----------|
| NEW `GET /v1/engines` route from `available_engines()` + config default | Honest, feature-gated-aware; ROADMAP names this endpoint | ✓ |
| Hardcode the four engine names in the frontend | Lies if engines are feature-gated/renamed | |
| Derive from `/metrics` labels | Fragile, over-fetched | |

**User's choice:** [auto] NEW `GET /v1/engines` route backed by `EngineRegistry::available_engines()` + the resolved default (recommended — ROADMAP deliverable; minimal handler reusing shared `web::Data<EngineRegistry>`).

---

## TestRunner placement + rule source

| Option | Description | Selected |
|--------|-------------|----------|
| New section in the edit page; runs the on-disk rule via stateless `/v1/execute` | Matches ROADMAP "in the rule editor"; `/v1/execute` runs the live shared ruleset | ✓ |
| Standalone "Test Runner" page/route with a rule picker | Separate route | |
| Per-row "Run" action on the rules list | Inline list action | |

**User's choice:** [auto] New section in `frontend/src/routes/rules/$id.edit.tsx`; runs the on-disk rule via stateless `/v1/execute` (optionally scoped by `rule_filter`) (recommended).

---

## Rule save/edit dependency (edit-page scaffold gap)

| Option | Description | Selected |
|--------|-------------|----------|
| Run already-saved rules; "Reload then Run" bridges edits; minimal Save→`PUT /rules/{id}` enabler only | Scopes Phase 28 to the test-runner; reuses Phase 20 management API | ✓ |
| Wire full rule fetch/save/authoring UX in the edit page | Out of scope (authoring is a separate phase) | |
| Run unsaved edits by POSTing the rule body to a new endpoint | New backend surface; not in ROADMAP | |

**User's choice:** [auto] Run already-saved rules; "Reload then Run" bridges edits; planner may add a minimal Save→`PUT /rules/{id}` enabler but no full authoring UX (recommended — keeps Phase 28 scoped; reuses Phase 20 API).

---

## Facts editor UX

| Option | Description | Selected |
|--------|-------------|----------|
| Raw JSON editor (CodeMirror JSON) matching `facts`/`facts_after` shape | Consistent with `facts_after` render; reuses CodeMirror; lowest build cost | ✓ |
| Structured key-value form (fact_type → fields) | Friendlier but constrains JSON value types + more build cost | |
| JSON editor with schema hint/template | Middle ground | |

**User's choice:** [auto] Raw JSON editor (CodeMirror JSON) matching the `facts`/`facts_after` shape (recommended).

---

## "Reload then Run" flow

| Option | Description | Selected |
|--------|-------------|----------|
| Ship in Phase 28 — poll `getJobStatus` (Pitfall 8 carve-out) then execute | SC-4 requires it; Phase 25 made `getJobStatus` real | ✓ |
| Defer — plain "Run" only | SC-4 would fail verification | |

**User's choice:** [auto] Ship "Reload then Run" in Phase 28 (recommended — SC-4 makes it required despite ROADMAP calling it "Optional").

---

## Engine-selector dropdown

| Option | Description | Selected |
|--------|-------------|----------|
| `useQuery` over `GET /v1/engines`; default from backend; selection in local state sent as `ExecuteRequest.engine` | Honest default; omit-on-none falls back to config | ✓ |
| Hardcoded list + hardcoded default | Lies about feature-gated engines | |
| Persist selection in Zustand across route changes | Only if needed | |

**User's choice:** [auto] `useQuery` over `GET /v1/engines`; default from the backend response; selection in local component state, sent as `ExecuteRequest.engine` (recommended).

---

## Results panel rendering

| Option | Description | Selected |
|--------|-------------|----------|
| Stacked sections (Fired / Not-fired / Facts after / Trace); trace hidden when `None` | Matches edit-page section pattern; conditional trace UX | ✓ |
| Tabbed panel (Results | Facts | Trace) | More complexity, little gain at this size | |

**User's choice:** [auto] Stacked sections; trace section hidden when `inference_trace` is `None` (recommended).

---

## execution.ts API client + types

| Option | Description | Selected |
|--------|-------------|----------|
| NEW `execution.ts` (5 functions) + types in `types.ts` mirroring backend snake_case serde; `useMutation`/`useQuery` | Matches existing api/ pattern + snake_case types precedent | ✓ |
| camelCase TS types with a transform layer | Adds a transform; existing types.ts uses snake_case | |
| Single `useExecute` hook instead of a typed api module | Breaks the one-function-per-endpoint pattern | |

**User's choice:** [auto] NEW `execution.ts` (executeRule/batchExecute/getJobStatus/listJobs/listEngines) + types in `types.ts` mirroring backend snake_case serde shapes; `useMutation` for execute, `useQuery` for engines/job-status (recommended).

---

## Claude's Discretion

- Trace `data` payload per entry (light vs. heavy); `TraceSummary.facts_matched` semantics; timestamp source (`chrono::Utc::now()` vs. monotonic offset).
- TestRunner state management (local `useState` vs. small Zustand slice); whether the engine selection persists across route changes.
- Minimal Save wiring (D-05) — wire the edit page's Save button to `PUT /v1/rules/{id}` vs. document "save via management UI first".
- `rule_filter` scoping — scope Run to the edited rule via `name_pattern` vs. run the whole ruleset.
- Component test depth (Vitest + Testing Library); `GET /v1/engines` response exact shape (`{ engines, default }` ± per-engine description); error rendering in the panel (inline banner).
- Whether trace emission lives in the shared `evaluate_ruleset_direct` helper or is mirrored into Rete's inlined `execute_rules` (Phase 26 C1 — Rete keeps its own loop).

## Deferred Ideas

- Batch-execute UI / results matrix (`batchExecute` is API-only in Phase 28).
- Stateful-session test-runner UI (multi-interaction session resume in the IDE).
- Full rule-authoring/save UX in the edit page (only a minimal Save enabler may land).
- "Load from `facts_after`" convenience for iterative testing.
- Per-engine description/metadata in `GET /v1/engines`.
- Network-node-level Rete trace (alpha/beta propagation).
- Rich trace `data` payload (fact/action snapshot) — deferred to keep SC-3 latency honest.