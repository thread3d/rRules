# Phase 28: Frontend test-runner - Context

**Gathered:** 2026-07-06
**Status:** Ready for planning
**Mode:** `--auto` (all gray areas auto-selected; recommended option chosen for each; logged inline)

<domain>
## Phase Boundary

Make live execution a **user-facing feature** of the rule IDE: from the rule editor, a user clicks **Run**/**Test** and sees a results panel rendering `rules_fired`, `rules_not_fired`, `facts_after`, and the **inference trace** — picking the engine from a dropdown populated from a real backend endpoint, with an optional **Reload then Run** flow that re-executes against an edited rule set and correlates the result via `engine_generation`. This closes the v5.0 vertical slice by surfacing the Phase-23–27 real execution path in the UI.

After this phase: a user runs a rule from the editor and sees real `rules_fired`/`rules_not_fired`/`facts_after` (SC-1); the engine-selector dropdown lists the available engines from `GET /v1/engines` and the selection is sent on the execute request (SC-2); with `trace: true` the results panel shows the inference trace, and with `trace` absent/false the trace is omitted and latency is unaffected (SC-3); after "Reload then Run" the results panel reflects the edited rule correlated via `engine_generation` (SC-4).

This is the **frontend + trace-population** layer of the v5.0 Live Execution vertical slice. It builds on Phase 23's real `/v1/execute` (truthful `rules_fired`/`rules_not_fired`/`facts_after`, `engine` param, `CoverageTracking` once-per-rule), Phase 24's `execution_id`/`engine_generation` response fields + WebSocket `MetricsSnapshot` broadcast, Phase 25's `SharedRuleSet{ruleset, generation}` + async `reload_rules` writing `ReloadJob` to `JobStore` (so `getJobStatus` works), and Phase 27's `session_id` response field. It **also crosses into a small amount of backend work** because (a) `inference_trace` is always `None` today (Phase 23 L-04 deferred population to Phase 28 / EXEC-04) and (b) no HTTP route exposes `available_engines()` today — both are named explicitly in the ROADMAP Phase 28 deliverables.

**In scope (locked by ROADMAP.md §"Phase 28" + 23-RESEARCH.md Flag 3 + PITFALLS.md Pitfall 8):**
- NEW `frontend/src/api/execution.ts` (`executeRule`, `batchExecute`, `getJobStatus`, `listJobs`, `listEngines`) + `ExecuteRequest`/`ExecuteResponse`/`InferenceTrace`/`InferenceTraceEntry`/`TraceSummary`/`BatchExecute*` types in `frontend/src/api/types.ts`
- NEW `TestRunner` component mounted in the rule editor page (`frontend/src/routes/rules/$id.edit.tsx`): facts editor, engine-selector dropdown, Run/Test buttons, results panel rendering `rules_fired`/`rules_not_fired`/`facts_after`/`inference_trace`
- **(backend) Populate `inference_trace`** when requested: add `trace: Option<bool>` (off by default) to `ExecuteRequest`; instrument the direct evaluator (`engine/src/engine/direct_eval.rs` + `rete/mod.rs` inlined path) to emit one `InferenceTraceEntry` per fired rule when `trace == true`; leave `inference_trace: None` and latency untouched when `trace` is absent/false (EXEC-04)
- **(backend) NEW `GET /v1/engines` route** returning the available engine names + the config default, backed by `EngineRegistry::available_engines()` (the dropdown's data source — ROADMAP names this endpoint explicitly)
- Optional **"Reload then Run"** flow → `reloadRules({async:true})` + poll `getJobStatus` until `Completed` + `executeRule`, correlated via `engine_generation` (SC-4 makes this required for verification)
- Drive UI refresh from the `ExecuteResponse` itself (`execution_id`/`engine_generation`), NOT from a follow-up poll (Pitfall 8) — polling is used ONLY for the async reload job status
- Monitoring dashboards need NO frontend changes (Phase 24 confirmed the adapters already tolerate real shapes)

**Out of scope (explicitly deferred):**
- A separate "batch-execute UI" / batch results matrix — `batchExecute` is exposed in `execution.ts` for completeness and the ROADMAP deliverable, but no batch UI screen is built in Phase 28 (the `TestRunner` runs single rules; batch is API-only)
- Stateful-session UI (multi-interaction session resume in the IDE) — Phase 27 shipped the backend; a session-aware test-runner panel is a future frontend phase. The `session_id` field is carried in the types but the TestRunner uses the stateless one-shot path
- Full Rete alpha/beta network trace (node-level propagation) — the inference trace is per-fired-rule (rule name + event_type + timestamp + light data), not a network-node trace
- Rule fetch/save wiring in the edit page — the edit route currently has a `TODO(22-05/22-06)` for load/persist. Phase 28 runs against rules already on disk via the shared `RuleSet`; the planner decides whether a minimal save-to-disk path is needed for "edit → run" to be meaningful (see D-05)
- Rewriting the monitoring dashboards — they already render live data (Phase 24 SC-3)

</domain>

<decisions>
## Implementation Decisions

### Carried forward as locked (not re-asked — from ROADMAP.md, PITFALLS.md, Phase 23/24/25/27 CONTEXT)
- **L-01:** `inference_trace: Option<InferenceTrace>` is already plumbed on `ExecuteResponse` (`api/src/v1/execution.rs:56`) and is always `None` today (`execution.rs:730`, `:950`). Phase 23 L-04 explicitly deferred population to Phase 28 / EXEC-04. The API `InferenceTrace`/`InferenceTraceEntry`/`TraceSummary` structs (`execution.rs:189-205`) already exist and serialize. Phase 28 populates them — no new response struct needed.
- **L-02:** Do NOT wire the chaining `InferenceTraceCollector` (`engine/src/chaining/mode_selector.rs:135`) — it is coupled to the stub `environment::Engine` surface that Phase 23 deliberately left behind (23-RESEARCH.md Flag 3). The trace is emitted by the **live direct evaluator** (`direct_eval.rs` + `rete/mod.rs`), not the collector.
- **L-03:** `facts_after` shape is `{ fact_type_id: serde_json::Value }` (`HashMap<String, serde_json::Value>`, Phase 23 D-08). The frontend results panel renders it as JSON. No shape change.
- **L-04:** `ExecuteResponse` already carries `execution_id: String` + `engine_generation: u64` + `session_id: Option<String>` (Phases 24/25/27). The frontend uses `execution_id`/`engine_generation` to correlate a run with its result and to detect staleness after a reload (Pitfall 8). No new response fields.
- **L-05:** Single `VITE_API_URL` (default `/v1`) baseURL via the shared `apiClient` (`frontend/src/api/client.ts:11`). `execution.ts` imports the same `apiClient` — no second axios instance, no `/api/v1` mismatch (Phase 22 D-22-05).
- **L-06:** Stack: React 19 + TanStack Query (`@tanstack/react-query` ^5.50) for the execute mutation + engine-list query, Zustand for cross-component state if needed, Tailwind + `lucide-react` for icons, `@uiw/react-codemirror` for any JSON facts editor. Vitest + `@testing-library/react` for component tests. All already in `frontend/package.json`.
- **L-07:** `ExecuteRequest` already has `session_id`, `facts`, `rule_filter`, `engine` (`execution.rs:21-34`). Phase 28 adds only `trace: Option<bool>`. The stateless one-shot path (Phase 23) is what the TestRunner uses; `session_id` is left absent.
- **L-08:** Pitfall 8 (stale-polling race) — drive the results panel from the `ExecuteResponse` returned by `executeRule`; do NOT poll `/v1/metrics/coverage` or the WS channel for per-run results. Polling is used ONLY for `getJobStatus` during the async reload branch of "Reload then Run" (Pitfall 8 carve-out — the reload job is genuinely async).
- **L-09:** Pitfall 3 (dead-rules panel empty) — already resolved by Phase 26; the monitoring dashboards read live data. No Phase 26 dependency blocker.

### Discussed gray areas (auto-selected recommended option for each)

#### inference_trace population source (EXEC-04 — the crux)
- **D-01: Instrument the live direct evaluator to emit a real per-fired-rule `InferenceTrace` when `trace == true` (do NOT synthesize from coverage).** `evaluate_ruleset_direct` (`engine/src/engine/direct_eval.rs`) already iterates each fired rule and returns `(fired_set, evaluated_set)`; Rete's inlined `execute_rules` (`engine/src/engine/rete/mod.rs`) does the same (Phase 26 C1 wired recording at BOTH sites). Phase 28 threads a `collect_trace: bool` into the evaluator and, when true, pushes one `InferenceTraceEntry { timestamp, event_type: "rule_fired", rule_name: Some(id), data: Option<serde_json::Value> }` per fired rule (in firing order), plus a `TraceSummary { total_duration_ms, facts_matched }`. When false/absent, the evaluator skips the trace work entirely — `inference_trace` stays `None` and per-request latency is unaffected (SC-3). This is the honest path: the trace reflects what the engine actually did, not a reconstruction. Phase 23 research floated "derive from `rules_not_fired`/`CoverageTracking`" but noted it "adds no value over the `rules_fired`/`rules_not_fired` fields already present" — the user's consistent v5.0 posture (Phase 23/25/26/27 CONTEXT "Specifics": take the honest-for-real-authored-rules option) means a real trace, not a derived one.
  - `[auto]` Q: "Populate `inference_trace` by instrumenting the evaluator, or derive it from `rules_not_fired`/`CoverageTracking`?" → Selected: **"Instrument the live direct evaluator — emit one real `InferenceTraceEntry` per fired rule when `trace == true`; `None` otherwise"** (recommended — 23-RESEARCH Flag 3 assigns this to Phase 28; the user's v5.0 posture is the honest path; a derived trace duplicates existing fields)

#### `trace` opt-in flag + performance guard
- **D-02: `trace: Option<bool>` on `ExecuteRequest`, off by default; the evaluator skips trace collection unless `trace == Some(true)`.** Add `#[serde(default)] pub trace: Option<bool>` to `ExecuteRequest` (`execution.rs:21`). `execute_single` resolves `let collect_trace = req.trace.unwrap_or(false)` and passes it into the evaluator; `ExecuteResponse.inference_trace` is `Some(trace)` only when `collect_trace && entries non-empty`, else `None`. The TestRunner UI has a "Trace: on/off" toggle (off by default) so the user opts in per run. SC-3's "trace absent/false → omitted and latency unaffected" is enforced by the evaluator skipping the work (no allocation, no timestamp calls).
  - `[auto]` Q: "How is the trace opt-in surfaced and gated for performance?" → Selected: **"`trace: Option<bool>` on the request, off by default; evaluator skips trace work unless `Some(true)`"** (recommended — ROADMAP deliverable: "`trace: bool` opt-in flag on `ExecuteRequest` (off by default for performance)")

#### `GET /v1/engines` backend endpoint
- **D-03: Add a NEW `GET /v1/engines` route returning `{ engines: [...], default: "rete" }` from `EngineRegistry::available_engines()` + the resolved default.** No HTTP route exposes the engine list today (only `registry.available_engines()` used internally at `execution.rs:548`/`sessions.rs:190`). The ROADMAP Phase 28 deliverable explicitly says the dropdown is "populated from `GET /v1/engines`", so this small backend addition is in-scope. The handler reads `web::Data<EngineRegistry>` (already shared) + the `RRULES_DEFAULT_ENGINE` env (or `EngineSelector::current_engine()`) and returns the list + default. Registered in the `/v1` scope in `main.rs` alongside the execute routes. Chosen over hardcoding the four names in the frontend (which would lie if engines are feature-gated or renamed) and over deriving from `/metrics` labels (fragile, over-fetched).
  - `[auto]` Q: "How does the frontend learn the available engines — new `GET /v1/engines` route, hardcoded list, or derive from `/metrics`?" → Selected: **"NEW `GET /v1/engines` route backed by `available_engines()` + config default"** (recommended — ROADMAP names this endpoint; honest about feature-gated engines; minimal handler reusing shared `web::Data`)

#### TestRunner placement + rule source
- **D-04: Mount `TestRunner` as a new section in the rule edit page (`frontend/src/routes/rules/$id.edit.tsx`); run the rule via the stateless `/v1/execute` path against the shared on-disk `RuleSet`.** The TestRunner renders: (1) a facts editor (D-06), (2) the engine-selector dropdown (D-08), (3) Run + "Reload then Run" buttons (D-07), (4) the results panel (D-09). It calls `executeRule({ facts, engine, trace })` — `session_id` left absent (stateless one-shot, L-07). The rule being "run" is the rule on disk identified by the route's `id` (the shared `RuleSet` is loaded from `RULE_STORAGE_PATH` at startup and swapped on reload — Phase 25); `rule_filter: { name_pattern: <id> }` can scope execution to the edited rule. This matches ROADMAP "TestRunner component in the rule editor." A standalone test-runner page is NOT built.
  - `[auto]` Q: "Where does `TestRunner` mount and what rule does it run?" → Selected: **"New section in the edit page; runs the on-disk rule via stateless `/v1/execute` (optionally scoped by `rule_filter`)"** (recommended — ROADMAP says "in the rule editor"; `/v1/execute` already runs the live shared ruleset)

#### Rule save/edit dependency (the edit-page scaffold gap)
- **D-05: Phase 28 runs against rules already on disk; "Reload then Run" is the bridge for edited rules. If the planner finds the edit page cannot save, it adds a **minimal** save-to-disk path (or reuses the existing management API `PUT /rules/{id}`) sufficient for "edit → save → reload → run" — but a full authoring/save UX is NOT in scope.** The edit route currently has `TODO(22-05/22-06): load rule id via the rule fetch API and persist edits` — the editor is a scaffold without fetch/save wiring. Because `/v1/execute` runs the shared on-disk `RuleSet`, an unsaved edit cannot be run directly. The honest path: the user saves the rule (existing `PUT /v1/rules/{id}` management endpoint from Phase 20) then clicks "Reload then Run", which calls `reloadRules({async:true})` + polls `getJobStatus` → `executeRule`. The planner decides whether to wire the edit page's Save button to `PUT /rules/{id}` (minimal, in-scope as an enabler) or to document "save via the management UI first." A full rule-authoring/save UX (validation, governance approval flow integration) remains out of scope — Phase 28's bar is "run a saved rule and see real results."
  - `[auto]` Q: "The edit page can't save yet (TODO 22-05/22-06). Does Phase 28 wire rule save, or run only already-saved rules?" → Selected: **"Run already-saved rules; 'Reload then Run' bridges edits; planner may add a minimal Save→`PUT /rules/{id}` enabler but no full authoring UX"** (recommended — keeps Phase 28 scoped to the test-runner; reuses the Phase 20 management API rather than rebuilding authoring)

#### Facts editor UX
- **D-06: Raw JSON editor for input facts (CodeMirror JSON), matching the `facts: HashMap<String, serde_json::Value>` shape and the `facts_after` render.** The user types facts as a JSON object keyed by `fact_type_id` (e.g. `{ "scorecard": { "score": 0 } }`) — exactly the shape `/v1/execute` accepts and `facts_after` returns. Reuse `@uiw/react-codemirror` with a JSON language extension (the project already uses CodeMirror via `RuleEditor`). A structured key-value form is friendlier but adds build cost and constrains the JSON value types; the raw JSON editor is consistent with `facts_after` display and lowest-risk. A "load from `facts_after`" convenience (copy the last result's facts into the editor) is deferred.
  - `[auto]` Q: "How does the user enter input facts — raw JSON editor or structured key-value form?" → Selected: **"Raw JSON editor (CodeMirror JSON) matching the `facts`/`facts_after` shape"** (recommended — matches the request/response shape; reuses the CodeMirror stack; lowest build cost)

#### "Reload then Run" flow
- **D-07: Ship "Reload then Run" in Phase 28 (SC-4 requires it).** A second button next to "Run" calls `reloadRules({ async: true })` → polls `getJobStatus(jobId)` (the ONE legitimate polling path — Pitfall 8 carve-out) until `ReloadJob` reaches `Completed` (or `Failed`) → then `executeRule(...)` and renders the result. The result's `engine_generation` is compared to the pre-reload generation so the panel can show "ran against generation N (refreshed)". SC-4 ("After Reload then Run, the results panel reflects the edited rule correlated via `engine_generation`") makes this non-optional for verification, so despite ROADMAP calling it "Optional", it ships. Plain "Run" (no reload) stays the default for the common case where rules haven't changed.
  - `[auto]` Q: "Is the optional 'Reload then Run' flow in Phase 28 or deferred?" → Selected: **"In Phase 28 — SC-4 requires it; poll `getJobStatus` (the Pitfall 8 polling carve-out) then execute"** (recommended — SC-4 verification depends on it; Phase 25 already made `getJobStatus` real)

#### Engine-selector dropdown
- **D-08: Dropdown populated from `GET /v1/engines` (TanStack Query); default = the backend-reported default; selection held in local component state and sent as `engine` on the execute request.** `listEngines()` is fetched once via `useQuery(['engines'])` (cacheable, rarely changes); the dropdown's initial value is the `default` field from the response; the user's selection persists in `useState` for the session. The selected engine is sent as `ExecuteRequest.engine` on every Run. Unknown/missing selection → omit `engine` → backend resolves to the config default (Phase 23 L-07). No Zustand slice unless the planner finds the selection needs to persist across route changes (discretion).
  - `[auto]` Q: "How is the engine-selector dropdown populated and defaulted?" → Selected: **"`useQuery` over `GET /v1/engines`; default from the backend response; selection in local state, sent as `ExecuteRequest.engine`"** (recommended — ROADMAP: "dropdown populated from `GET /v1/engines`"; honest default; omit-on-none falls back to config)

#### Results panel rendering
- **D-09: Stacked sections in a scrollable panel — Fired rules (count + list), Not-fired rules (list), Facts after (JSON tree), Inference trace (entry list, only when `trace:true` and present).** Matches the edit page's existing `<section>` pattern. The trace section renders each `InferenceTraceEntry` (timestamp, event_type, rule_name, data) as a row and shows the `TraceSummary` (total_duration_ms, facts_matched) when present; the section is hidden entirely when `inference_trace` is `None` (so the off-by-default path shows no trace UI). `facts_after` is rendered as a syntax-highlighted JSON block (CodeMirror read-only or a `<pre>`). A tabbed panel is an alternative but adds complexity for little gain at this panel size.
  - `[auto]` Q: "How are results rendered — tabbed panel or stacked sections?" → Selected: **"Stacked sections (Fired / Not-fired / Facts after / Trace); trace section hidden when `None`"** (recommended — matches the edit-page section pattern; trace UX is conditional on the opt-in)

#### execution.ts API client + types
- **D-10: NEW `frontend/src/api/execution.ts` with `executeRule`, `batchExecute`, `getJobStatus`, `listJobs`, `listEngines`; types in `frontend/src/api/types.ts` mirroring the backend serde shapes (snake_case JSON keys).** The backend serializes with `serde::Serialize` default (snake_case): `rules_fired`, `rules_not_fired`, `facts_after`, `inference_trace`, `engine_generation`, `execution_id`, `session_id`, `entries`, `summary`, `total_duration_ms`, `facts_matched`, `event_type`, `rule_name`, `timestamp`. The TS interfaces use those exact keys (the frontend does not transform case — consistent with the existing `ApprovalState`/`ApprovalsResponse` types in `types.ts` which already use snake_case backend keys). `executeRule` is wired as a TanStack `useMutation`; `listEngines` + `getJobStatus` as `useQuery`. `batchExecute` + `listJobs` are exported for completeness (ROADMAP deliverable) but have no UI consumer in Phase 28.
  - `[auto]` Q: "What is the shape of `execution.ts` and the TS types?" → Selected: **"NEW `execution.ts` (5 functions) + types in `types.ts` mirroring backend snake_case serde shapes; `useMutation` for execute, `useQuery` for engines/job-status"** (recommended — matches the existing api/ module pattern and the snake_case types already in `types.ts`)

### Claude's Discretion (technical impl details left to the planner/researcher)
- **Trace `data` payload per entry** — the contract is one `InferenceTraceEntry` per fired rule with `event_type: "rule_fired"` + `rule_name` + `timestamp`. Whether `data` carries the matching fact snapshot, the action applied, or is left `None` is the planner's call (a light payload is useful for debugging; a heavy one risks the latency SC-3 protects). Default lean: `data: None` or the action summary only.
- **`TraceSummary.facts_matched` semantics** — whether it counts evaluated facts, matched facts, or fired rules is the planner's pick; the struct field exists (`execution.rs:205`) and the value just needs to be honest.
- **Timestamp source in the evaluator** — `chrono::Utc::now().to_rfc3339()` (already used in Phase 26 `RuleExecutionTracker`) vs. a relative monotonic offset. The contract is "a timestamp per entry"; mechanism is planner's choice (chrono is the established pattern).
- **TestRunner state management** — `useState` in the component vs. a small Zustand slice. Local state is the default; Zustand only if the selection/result needs to survive route changes.
- **Minimal Save wiring (D-05)** — whether to wire the edit page's Save button to `PUT /v1/rules/{id}` as a Phase 28 enabler or to document "save via management UI first." The contract is "the user can run an edited rule via Reload then Run"; the mechanism is the planner's call.
- **`rule_filter` scoping** — whether Run scopes to the edited rule via `rule_filter: { name_pattern: id }` or runs the whole ruleset. Scoping to the one rule is cleaner for a "test this rule" UX; the planner decides.
- **Component test coverage** — Vitest + Testing Library assertions for "Run renders fired rules" / "trace toggle shows/hides the trace section" / "engine dropdown lists `GET /v1/engines`". The keystone is the live e2e (D-11); component tests are at the planner's depth.
- **`GET /v1/engines` response exact shape** — `{ engines: string[], default: string }` is the proposed shape; the planner may add `description` per engine if useful, but the contract is "dropdown list + default."
- **Error rendering** — how a 400 (unknown engine) or execute failure is surfaced in the panel (inline error banner). Standard pattern; planner's pick.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap & requirements (scope + success criteria)
- `.planning/ROADMAP.md` §"Phase 28: Frontend test-runner" (line 252) — goal, depends-on (Phases 23/24/26; parallel with 27 if session path deferred), requirements (`FRONTEND-03`, `FRONTEND-04`, `EXEC-04`), 6 key deliverables, Pitfalls 8 + 3 to avoid, research flag (`inference_trace` population source decided in Phase 23; if ReteEngine cannot emit a trace, derive from `rules_not_fired`/`CoverageTracking`), and the 4 success criteria verification must satisfy. UI hint: "yes".
- `.planning/REQUIREMENTS.md` — traceability: `FRONTEND-03`, `FRONTEND-04`, `EXEC-04` → Phase 28 (Pending).

### Milestone research (pitfalls + trace decision — HIGH confidence, file:line evidence)
- `.planning/research/PITFALLS.md` §"Pitfall 8" (line 201) — frontend test-runner shows stale results (polling vs execution-completion race). Prevention: consume the `ExecuteResponse` fired-rule list; do NOT poll for per-run results; tag responses with `execution_id`/`engine_generation` so the UI can warn "rules changed since this run"; re-fetch the rule list after a hot-reload. Polling is reserved for the async reload job status only (D-07).
- `.planning/research/PITFALLS.md` §"Pitfall 3" (line ~296) — dead-rules panel empty; resolved by Phase 26 (the panel now has live data). Phase 28 depends on Phase 26 having landed.
- `.planning/research/PITFALLS.md` §"Pitfall 4" (line 105) — generation counter on `Engine`; `engine_generation` in every `ExecuteResponse` solves both the hot-reload race and the frontend staleness with one field (D-07 correlates "Reload then Run" via it).

### Prior phase context (the scaffold Phase 28 builds on)
- `.planning/phases/23-execute-evaluator-wiring/23-CONTEXT.md` — **L-04** (`inference_trace: None` in Phase 23; populating is EXEC-04/Phase 28 — D-01), **D-08** (`facts_after` shape = `{fact_type: json_value}` map — D-09 renders it), **L-07** (unknown `engine` → 400; omitted → `RRULES_DEFAULT_ENGINE` — D-08 fallback), and the `web::Data` + `spawn_blocking` pattern the `GET /v1/engines` handler reuses (D-03).
- `.planning/phases/23-execute-evaluator-wiring/23-RESEARCH.md` §"Research Flag 3: inference_trace Population Source" (line 117) — the decision to leave `inference_trace: None` in Phase 23 and the explicit hand-off to Phase 28; warns NOT to wire `InferenceTraceCollector` from `chaining/mode_selector.rs` (L-02).
- `.planning/phases/24-execution-counters-metrics-broadcast/` — `ExecuteResponse.execution_id` + `engine_generation` (Pitfall 8 correlation fields, L-04) + the WebSocket `MetricsSnapshot` broadcast (the monitoring dashboards already read live data — Phase 28 adds NO dashboard changes).
- `.planning/phases/25-real-reload-jobstore/25-CONTEXT.md` — `SharedRuleSet{ruleset, generation}` (the on-disk ruleset `/v1/execute` runs against, D-04) + async `reload_rules` writing `ReloadJob{Pending→Running→Completed/Failed}` to `JobStore` so `getJobStatus` works (D-07 "Reload then Run" polls it) + `engine_generation` increment on swap (D-07 correlation).
- `.planning/phases/27-stateful-sessions/27-CONTEXT.md` — `ExecuteResponse.session_id: Option<String>` (carried in the TS types, L-07; the TestRunner uses the stateless path so it is absent) + D-08 `rrules_sessions_active` gauge (live on the dashboard already).

### Live code — backend integration points (verified during this discussion)
- `api/src/v1/execution.rs:21-34` — `ExecuteRequest` (`session_id`, `facts`, `rule_filter`, `engine`); Phase 28 adds `trace: Option<bool>` (D-02).
- `api/src/v1/execution.rs:47-66` — `ExecuteResponse` (`execution_id`, `rules_fired`, `rules_not_fired`, `facts_after`, `inference_trace: Option<InferenceTrace>`, `engine_generation`, `session_id`); all fields already exist (L-01/L-04) — no new response fields.
- `api/src/v1/execution.rs:189-205` — `InferenceTrace` / `InferenceTraceEntry` / `TraceSummary` structs (already `serde::Serialize` + `utoipa::ToSchema`); Phase 28 populates them (D-01).
- `api/src/v1/execution.rs:504-575` — `execute_single` (stateless one-shot path the TestRunner calls); Phase 28 threads `collect_trace` from `req.trace` into the evaluator and builds `inference_trace: Some(...)` when collected (D-02). `:730` and `:950` are the `inference_trace: None` sites to replace.
- `api/src/v1/execution.rs:957` — `execute_rules` handler (`POST /v1/execute`, `path = "/v1/execute"`); the TestRunner's `executeRule` hits this.
- `engine/src/engine/direct_eval.rs:43-69` — `evaluate_ruleset_direct` returns `(fired_set, evaluated_set)`; Phase 28 threads `collect_trace: bool` and emits `InferenceTraceEntry` per fired rule when true (D-01). Records `CoverageTracking` once-per-rule already (L-01) — trace emission is additive, do not double-record.
- `engine/src/engine/rete/mod.rs` — Rete's inlined `execute_rules` (Phase 26 C1 second recording site); mirror the trace emission here so all four engines can trace (the evaluator is shared but Rete keeps its own loop — planner confirms whether trace emission lives in the shared helper or is mirrored).
- `api/src/main.rs:208-267` — `/v1` route scope; add `GET /v1/engines` (D-03). `:44-102` `web::Data` wiring (registry already shared — the engines handler reuses `web::Data<EngineRegistry>`).
- `api/Cargo.toml` — no new backend deps expected (`chrono` already a workspace dep for trace timestamps; `utoipa` already schemas the trace structs).

### Live code — frontend integration points (verified during this discussion)
- `frontend/src/api/client.ts:11` — shared `apiClient` (`baseURL: VITE_API_URL || '/v1'`); `execution.ts` imports this (L-05, D-10).
- `frontend/src/api/types.ts` — add `ExecuteRequest`/`ExecuteResponse`/`InferenceTrace`/`InferenceTraceEntry`/`TraceSummary`/`BatchExecuteRequest`/`BatchExecuteResponse`/`EngineInfo`/`EnginesResponse`/`ReloadJob` types here (D-10). The file already uses snake_case backend keys (`ApprovalState`, `ApprovalsResponse`) — match that.
- `frontend/src/api/rules.ts` — the established api-module pattern (one function per endpoint, typed response); `execution.ts` mirrors it (D-10).
- `frontend/src/components/RuleEditor.tsx` — the CodeMirror wrapper to reuse for the facts JSON editor (D-06); same `@uiw/react-codemirror` + theme.
- `frontend/src/routes/rules/$id.edit.tsx` — the edit page where `TestRunner` mounts as a new `<section>` (D-04). NOTE the `TODO(22-05/22-06)`: load/persist not wired (D-05 gap).
- `frontend/src/features/monitoring/dashboards/{EngineMetrics,RuleCoverage,DeadRuleReport}Dashboard.tsx` — the existing dashboards; Phase 28 makes NO changes (they already render live data, Phase 24 SC-3). Referenced only to confirm the adapters tolerate real shapes.
- `frontend/src/types/common.ts` — shared domain types (`Rule`, `Condition`, `Action`, `HitPolicyId`); the execute types may reference `Rule` for `rule_filter`.
- `frontend/package.json` — React 19, TanStack Query/Router/Table, Zustand, axios, CodeMirror, lucide-react, recharts, Tailwind, Vitest + Testing Library — all present (L-06).

### No external specs
No ADRs or external standard specs apply — brownfield frontend + small backend-route phase against the project's own React/Rust codebase. DMN/Drools test-runner UX is background only, not a spec to implement.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`apiClient` (`frontend/src/api/client.ts:11`)** — the single axios instance with auth interceptor + 401 redirect; `execution.ts` reuses it (no second instance, no baseURL drift).
- **`@uiw/react-codemirror` + `@codemirror/lang-javascript`** (already used by `RuleEditor.tsx`) — reuse for the facts JSON editor; add `@codemirror/lang-json` (or use `javascript({ json: true })`) if not already installed — planner verifies the dep.
- **TanStack Query (`@tanstack/react-query` ^5.50)** — `useMutation` for `executeRule` (and "Reload then Run"), `useQuery` for `listEngines` and `getJobStatus` polling. Already used by `useRules.ts` (the rules feature) — mirror its query-key + error handling pattern.
- **`ExecuteResponse` fields** (`execution_id`, `engine_generation`, `session_id`, `rules_fired`, `rules_not_fired`, `facts_after`, `inference_trace`) — all already serialized by the backend (Phases 23/24/25/27); the TS types mirror them with no transformation (D-10).
- **`InferenceTrace`/`InferenceTraceEntry`/`TraceSummary`** (`execution.rs:189-205`) — already `serde::Serialize` + `ToSchema`; Phase 28 populates them, no struct changes.
- **`EngineRegistry::available_engines()`** (`execution.rs:548`/`sessions.rs:190` call sites) — the data source for the new `GET /v1/engines` handler (D-03); `web::Data<EngineRegistry>` is already shared.
- **`evaluate_ruleset_direct` (`direct_eval.rs:43`)** — the live evaluator that already iterates fired rules and returns `(fired_set, evaluated_set)`; trace emission is additive to this loop (D-01).
- **`reload_rules` async + `JobStore` + `getJobStatus`** (Phase 25) — the "Reload then Run" flow reuses these directly (D-07); `ReloadJob{Pending→Running→Completed/Failed}` is already wired.
- **`PUT /v1/rules/{id}` management endpoint** (Phase 20) — the existing save path D-05 may wire the edit page's Save button to (minimal enabler, not a full authoring UX).
- **`lucide-react` + Tailwind** — icons + styling for the Run/Test/Reload buttons, dropdown, results panel sections (D-09); consistent with the existing UI.

### Established Patterns
- **One apiClient, one `VITE_API_URL`** (Phase 22 D-22-05) — `execution.ts` follows `rules.ts`'s one-function-per-endpoint pattern with typed responses; snake_case TS types matching backend serde (the `ApprovalState`/`ApprovalsResponse` precedent in `types.ts`).
- **TanStack Query for server state, Zustand for UI state** (v4.0 frontend architecture) — `executeRule` as `useMutation`, `listEngines`/`getJobStatus` as `useQuery`; component-local `useState` for the engine selection + trace toggle (D-08/D-02).
- **Drive UI from the response, not polling** (Pitfall 8, Phase 24) — the results panel renders the `ExecuteResponse` directly; polling is reserved for `getJobStatus` during async reload (D-07).
- **Wave 0 RED → GREEN + Nyquist closure** (Phase 23/24/25/26/27) — the SC-1..SC-4 e2e tests are written RED first, then implemented to GREEN (D-11). Backend trace + engines-route tests extend `api/tests/execute_integration.rs`; frontend component tests use Vitest + Testing Library.
- **`#[serde(default)]` / `#[serde(skip_serializing_if = ...)]` for additive request/response fields** (Phase 24/25/27) — `trace: Option<bool>` with `#[serde(default)]` keeps existing clients/fixtures parsing (D-02).

### Integration Points
- **NEW `frontend/src/api/execution.ts`** — `executeRule`, `batchExecute`, `getJobStatus`, `listJobs`, `listEngines` (D-10).
- **`frontend/src/api/types.ts`** — add the execute/trace/engine/job TS interfaces (D-10).
- **NEW `frontend/src/components/TestRunner.tsx`** (or `frontend/src/features/rules/TestRunner.tsx`) — the panel component (D-04/D-06/D-08/D-09).
- **`frontend/src/routes/rules/$id.edit.tsx`** — import + mount `<TestRunner ruleId={id} />` as a new `<section>` (D-04).
- **`api/src/v1/execution.rs:21` (`ExecuteRequest`)** — add `trace: Option<bool>` (D-02); `:504` (`execute_single`) thread `collect_trace` + build `inference_trace: Some(...)` (D-01); `:730`/`:950` replace `inference_trace: None`.
- **NEW `GET /v1/engines` handler** in `api/src/v1/execution.rs` (or a new `api/src/v1/engines.rs`) + register in `api/src/main.rs:208-267` `/v1` scope (D-03).
- **`engine/src/engine/direct_eval.rs:43`** — thread `collect_trace: bool` + emit `InferenceTraceEntry` per fired rule (D-01); mirror in `engine/src/engine/rete/mod.rs` inlined path if Rete does not route through the shared helper.
- **`api/tests/execute_integration.rs`** — add SC-1..SC-4 e2e + trace-on/trace-off + `GET /v1/engines` tests (D-11).
- **`frontend/` Vitest suite** — add TestRunner component tests (D-11, planner depth).

</code_context>

<specifics>
## Specific Ideas

- **Keystone honesty anchor (SC-1):** author/save a rule `r1` (`score := score + 10`) on disk. From the edit page, click **Run** with facts `{ "scorecard": { "score": 0 } }`. The results panel shows `rules_fired: 1`, `rules_not_fired: [...]` (the other rules), and `facts_after: { "scorecard": { "score": 10 } }` — the **incremented** score, not the request input (this is the Phase-23 keystone re-exercised through the UI). If the panel shows `score: 10` (not `0`), the frontend is honestly surfacing the real execution path: D-04 (runs on-disk rule via `/v1/execute`), D-09 (renders `facts_after`), D-10 (`executeRule` wiring).
- **SC-2 (engine dropdown) honesty check:** the dropdown lists exactly the engines `GET /v1/engines` returns (≥ rete/treat/leaps/phreak — whichever are feature-enabled); selecting `treat` and clicking Run sends `engine: "treat"` and the response reflects a real treat execution; selecting an engine and reloading the page re-fetches the list (not a hardcoded stale list). If the dropdown lies (hardcoded names that don't match the backend), the keystone is dishonest — D-03 exists precisely to prevent that.
- **SC-3 (trace opt-in) honesty check:** with the **Trace** toggle OFF, Run returns `inference_trace: null` and the trace section is hidden; latency is the same as Phase 23/24 (the evaluator skipped trace work — D-02). With **Trace ON**, the panel shows one `InferenceTraceEntry` per fired rule (`event_type: "rule_fired"`, `rule_name: "r1"`, timestamp) + the `TraceSummary`. The on/off latency delta is the proof that the trace is genuinely opt-in and not silently collected.
- **SC-4 (Reload then Run) honesty check:** edit `r1` to `score := score + 20`, save, click **Reload then Run** → the panel polls `getJobStatus` until `Completed`, runs, and the result shows `facts_after.score == 20` with an `engine_generation` greater than the pre-reload run's — proving the run executed against the refreshed rule set (correlated via `engine_generation`, Pitfall 8). If the score is still `10` or the generation didn't bump, the reload race was not handled.
- **The user's consistent posture across v5.0** (Phase 23/25/26/27 CONTEXT "Specifics") is the **honest-for-real-authored-rules** option. The planner should not narrow D-01 (real evaluator trace, not derived), D-03 (real `GET /v1/engines`, not hardcoded), or D-07 (real reload+correlation, not a fake refresh) to "minimal that renders something" — the panel must reflect what the engine actually did. The trace-off latency guarantee (SC-3) is part of the honesty contract, not an optimization afterthought.

</specifics>

<deferred>
## Deferred Ideas

- **Batch-execute UI / results matrix** — `batchExecute` is exposed in `execution.ts` (ROADMAP deliverable) but no batch screen is built in Phase 28; a future frontend phase can add a batch test-runner panel.
- **Stateful-session test-runner UI** (multi-interaction session resume in the IDE) — Phase 27 shipped the backend; a session-aware panel (insert → fire → insert → fire → dispose) is a future frontend phase. The `session_id` field is carried in the TS types but the TestRunner uses the stateless path (L-07).
- **Full rule-authoring/save UX in the edit page** — the `TODO(22-05/22-06)` fetch/save wiring is only partially addressed (D-05 may add a minimal Save enabler). Validation, governance approval integration, and a polished authoring flow remain a future authoring phase.
- **"Load from `facts_after`" convenience** — copying the last result's facts into the input editor for iterative testing is a nice-to-have; deferred.
- **Per-engine description / metadata in `GET /v1/engines`** — the contract is "list + default"; richer per-engine metadata (description, algorithm notes) is deferred.
- **Network-node-level Rete trace** — the inference trace is per-fired-rule; a node-level alpha/beta propagation trace is a future debugging feature (the network types are themselves placeholder — Phase 23 OUT of scope).
- **Trace `data` payload richness** — D-01 leaves the per-entry `data` payload to the planner; a rich fact/action snapshot in `data` is deferred to keep SC-3 latency honest.

None of these were scope-creep suggestions from the user — they are the phase's own downstream milestones / explicit out-of-scope items, listed so the planner knows the boundaries.

</deferred>

---

*Phase: 28-frontend-test-runner*
*Context gathered: 2026-07-06 (--auto mode)*