# Feature Research

**Domain:** BRMS live execution surface + frontend test-runner (rRules v5.0)
**Researched:** 2026-07-02
**Confidence:** MEDIUM (BRMS execution-API patterns cross-checked against Drools/KIE, DMN/Kogito, and five rule-engine test-runner UIs; grounded in current rRules code at `api/src/v1/execution.rs`, `engine/src/environment.rs`, `engine/src/engine/registry.rs`, `engine/src/coverage/mod.rs`, and `frontend/src/routes/rules/$id.edit.tsx`)

## Scope Note

This file covers ONLY the new v5.0 "Live Execution" surface — wiring the existing engine to the API, populating trackers, and the frontend test-runner. It does NOT re-research already-shipped features (authoring IDE, decision tables, RBAC/approvals/audit, Prometheus metrics, dead-rule detection, coverage reporting *structures*, pluggable engines, Docker/Helm, hot-reload job tracking).

## Feature Landscape

### Table Stakes (Users Expect These)

A BRMS whose `/v1/execute` returns `rules_fired: 0` is functionally a no-op. These features are what makes the platform *actually execute rules*. Missing any of them = the execution surface is broken or misleading.

| Feature | Why Expected | Complexity | Notes & rRules Dependencies |
|---------|--------------|------------|------------------------------|
| **Real single-execute wiring** (`POST /v1/execute` fires rules) | The whole point of a BRMS. The stub at `execution.rs:307` (`let _engine = EngineBuilder::new().build()`, `rules_fired = 0`) must drive the engine. | MEDIUM | Reuse `EngineBuilder` (`environment.rs:23`) + `Engine::create_session(stateful, …)` + `execute_forward()` → `ExecutionResult`. Convert `req.facts` (already mapped via `json_to_engine_value`) into `Fact`s via `engine::fact::Fact`. Need to load rules from the rule store (Git-backed) into the engine before firing — `Engine::load_rules` (`environment.rs:122`) is currently a stub taking `&str`; wiring rule-source loading is the bulk of the work. |
| **`rules_fired` + `rules_not_fired` populated truthfully** | `ExecuteResponse` already declares both; today both are `0`/empty. Users will diff response vs. expectation. | LOW | `ExecutionResult` exposes `is_success`/`message` but NOT a fire-list — the engine must surface which rules fired. Either thread `CoverageTracker::record_fire` results or extend `ExecutionResult` to carry a fired-rule-name vector. `CoverageTracker` (`coverage/mod.rs`) already has `record_fire(rule_id)` + `record_skipped(rule_id)`. |
| **`facts_after` reflects post-inference working memory** | `ExecuteResponse.facts_after` today just echoes the input (`facts_after.insert(fact_type, fact_value.clone())`). Users expect to see inferred/modified facts. | MEDIUM | `Engine::working_memory()` / `working_memory_mut()` (`environment.rs:112-117`) expose `&WorkingMemory`. Need a `WorkingMemory → HashMap<String, serde_json::Value>` serializer (inverse of `json_to_engine_value`). Consequence facts inserted by rule actions must be captured here. |
| **Stateless one-shot execution mode** | Drools/KIE stateless session: created per request, disposed after, fire-and-forget. The default execution mode for API-driven decisions (risk scoring, eligibility). | LOW | `Engine::create_session(stateful=false, None)` (`environment.rs:102`) already exists. `execute_single` should default to this when `session_id` is absent. No cross-request state. |
| **Per-request engine selection** (`engine` param) | v3.0 shipped Rete/TREAT/LEAPS/PHREAK with a runtime selector; v5.0 must exercise it. A `?engine=treat` (or body field) that's ignored makes the pluggable architecture a lie. | MEDIUM | `EngineRegistry::create_engine_by_name` (`registry.rs:177`) + `EngineSelector::with_defaults()` (`selector.rs:77`) exist. `ExecuteRequest` does NOT currently carry an `engine` field — add `pub engine: Option<String>` (serde-skip-if-none) with a config-defined default (`EngineConfig::algorithm`, `selector.rs:23`). Validate against `registry.available_engines()`; 400 on unknown. |
| **Config-defined default engine** | Ops must set a default without code changes; per-request override is optional. | LOW | `EngineSelector::current_engine()` + `EngineConfig::algorithm` already model this. Wire `RULES_ENGINE_DEFAULT` env (or YAML) into `EngineState` so `execute_single` falls back to it. |
| **Batch-execute drives real execution** (`POST /v1/rules/batch-execute`) | The batch endpoint already has real concurrency (Semaphore + JoinSet, `execution.rs:374`) layered on the stub. Once `execute_single` is real, batch inherits it for free. | LOW (once single is real) | No new code beyond making `execute_single` real — the `BatchExecuteResponse` (`results`, `total_fired`, `succeeded`, `failed`) already aggregates correctly. Verify per-item isolation: each task builds its own engine (already true). |
| **Async reload-then-execute** (`POST /v1/rules/reload` + execute against refreshed rules) | v4.0 shipped hot-reload with `EngineState` + `ReloadJob`/`ReloadStatus` + `JobStore` + `GET /v1/jobs/{id}`. v5.0 must make the reloaded engine actually serve subsequent executes. | MEDIUM | `EngineState::reload_engine` (`execution.rs:131`) currently rebuilds `rule_files` but does NOT swap a compiled engine into `self.engine` (the write-lock block at 163-166 is a no-op hold). Wire: compile rules → replace `self.engine` contents → subsequent `execute_single` reads from `EngineState` instead of building a throwaway. Requires `execute_single` to accept `web::Data<EngineState>` (signature change). |
| **Auto-populate `CoverageTracker` on every execute** | v4.0 shipped coverage reporting *structures* and dead-rule detection that read empty data because executes never recorded. The milestone's stated goal: "monitoring surfaces read live data." | MEDIUM | `CoverageTracker::record_evaluation`/`record_fire`/`record_skipped` (`coverage/mod.rs`) must be called from inside the engine's firing loop OR from `execute_single` post-run. Cleanest: instrument `Engine::execute_forward` to call into a tracker ref. Tracker state must be shared (Arc) and surfaced to the monitoring module + Prometheus `/metrics`. |
| **Frontend "Run rule" affordance in the rule editor** | A rule IDE where you can author but not run a rule is half-finished. The edit page (`routes/rules/$id.edit.tsx`) currently has Expression + HitPolicy + DecisionTable but no Run button. | MEDIUM | Add a `RunPanel` component below the editor. Needs: a new `frontend/src/api/execute.ts` (none exists — `api/rules.ts` is CRUD-only), a `useExecute` hook, and a route-level execute call. `apiClient` (`api/client.ts`) already targets `/v1` with JWT bearer — reuse. |
| **Fired-rules + results display in the run panel** | The minimum useful run output: which rules fired and the final decision/result. | MEDIUM | `ExecuteResponse.rules_fired` (count) + `rules_not_fired` (names) + `facts_after` (decision). Render as a list with pass/fail highlighting. Ground in DecisionRules' "received output" panel + GoRules results tab. |
| **Inference trace surfaced (when debug enabled)** | `ExecuteResponse.inference_trace` (`InferenceTraceEntry`: timestamp, event_type, rule_name, data) is already declared but always `None`. Power users need to see *why* a rule fired. | MEDIUM | `EngineBuilder::enable_debug_trace(true)` (`environment.rs:53`) + `InferenceTraceCollector` (`chaining::mode_selector`) exist. `execute_single` should enable trace when requested (e.g., `?trace=true` or body `trace: bool`) and serialize the collector's output into `inference_trace`. Off by default (perf). |

### Differentiators (Competitive Advantage)

These distinguish rRules' execution surface from a minimal BRMS and from the Drools/DMN baselines. They lean on existing rRules assets (pluggable engines, Prometheus, governance) that competitors often lack.

| Feature | Value Proposition | Complexity | Notes & rRules Dependencies |
|---------|-------------------|------------|------------------------------|
| **Side-by-side engine comparison on one input** | Run the same facts through Rete AND TREAT AND LEAPS AND PHREAK and diff results/timing. No competitor exposes four Rete-family algorithms in one UI. Directly leverages v3.0. | HIGH | New `POST /v1/execute/compare` endpoint iterating `EngineType::all_variants()` (`registry.rs:33`), calling `execute_single` per engine, returning a `CompareResponse { by_engine: HashMap<String, ExecuteResponse> }`. Frontend: a "Compare engines" toggle in the Run panel rendering a table (engine × rules_fired × duration × result). Reuses `EngineMetrics` (`engine/metrics.rs`) for timing. |
| **Per-execution Prometheus metric emission + live dashboard correlation** | Every execute updates `/metrics` counters (rules fired, latency histogram, engine breakdown) so the monitoring dashboard (`routes/monitoring/index.tsx` → `EngineMetricsDashboard`) shows real activity within seconds. Most BRMS dashboards are batch/lagged. | MEDIUM | `metrics::MetricsCollector` (`engine/metrics.rs`) + existing Prometheus endpoint. Add execution-latency histogram + engine-label dimension. The v4.0 `useEngineMetrics` hook already consumes this; once populated it Just Works. |
| **Coverage + dead-rule detection fed by real runs** | The test-runner and live executions both feed `CoverageTracker`, so "this rule never fires" is backed by actual execution evidence, not synthetic tests. Couples execution to the v4.0 monitoring surfaces. | MEDIUM | `CoverageTracker` is the single shared sink for both API executes and UI test-runs. `monitoring::dead_rules` reads from it. Requires the tracker to be a process-wide `Arc<RwLock<CoverageTracker>>` injected via `web::Data`, not per-request. |
| **Stateful KIE-session-style sessions** (`POST /v1/execute/session/{id}`) | Multi-interaction execution: insert facts, fire, insert more, fire again, dispose. Drools charges a license for this; rRules can expose it natively. The `execute_with_session` handler (`execution.rs:561`) is a stub returning `rules_fired: 0`. | HIGH | `Engine::create_session(stateful=true, Some(id))` + `Session::insert_fact/execute/destroy` (`environment.rs:166-206`) exist. Need a `SessionStore` (Arc<RwLock<HashMap<SessionId, Session>>), handlers: create-session, execute-on-session, dispose-session, TTL eviction. `session_id` field already on `ExecuteRequest`. RBAC: sessions are per-user. |
| **Decision-table row-match highlighting in the run panel** | When a decision-table rule runs, highlight which rows matched (green), were evaluated-but-not-matched (yellow), and violated columns (red). GoRules/OpenRules have this; it turns a decision table from a spec into a live artifact. | HIGH | Requires the engine to report per-row match outcomes (extend `ExecutionResult` or emit via `CoverageTracker`). Frontend: `DecisionTable.tsx` (`features/rules/DecisionTable.tsx`) accepts `rows`/`columns` — add an optional `rowResults?: RowMatchStatus[]` prop. Coupling between engine output shape and UI prop is the hard part. |
| **Expected-vs-actual diff + "Save as test"** | Run a rule, assert the expected output, see a diff, and one-click save it as a regression test (feeds v2.0 testing framework). DecisionRules/Corticon pattern. | HIGH | New `TestRun` entity (name, rule_id, input, expected, actual, status, execution_time). Backend: `POST /v1/rules/{id}/tests` + `POST /v1/rules/{id}/tests/{test_id}/run`. Frontend: "Save as test" button in the Run panel posting the last `ExecuteResponse` + user-provided expected. Reuses `engine::testing` module. |
| **Per-rule / per-node timing in the trace** | Microsecond-level timing per fired rule so users find the slow rule. GoRules ships this; Drools exposes it only via `kie-scanner` tooling. | MEDIUM | `InferenceTraceEntry` already has `timestamp: String` — derive per-entry delta. Add `duration_us: Option<u64>`. `MetricsCollector` already tracks engine-level metrics; per-rule timing needs instrumentation in the firing loop. Frontend: render trace as a timeline/flamebar. |
| **Run-panel engine selector wired to the runtime selector** | A dropdown in the Run panel picking rete/treat/leaps/phreak per run, defaulting from config — making the pluggable engine a user-facing feature, not just an internal one. | LOW | Frontend `<select>` bound to `engine` field on the execute request. List from a new `GET /v1/engines` endpoint exposing `registry.available_engines()`. Pairs with per-request engine selection table-stake. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Always-on full inference trace** | "I want to see everything that happened." | Trace collection doubles execution latency and bloats response payloads; 90% of production executes don't need it. Defaulting it on poisons perf benchmarks. | Opt-in via `trace: bool` (body/query) per request. UI defaults on in the test-runner, off in the API path. |
| **Cross-request shared working memory in stateless mode** | "Let me accumulate facts without managing sessions." | Blurs stateless/stateful distinction; creates undeclared state, race conditions under concurrency, and non-deterministic results that are hell to debug. | Explicit stateful sessions via `session_id` with a real `SessionStore` and TTL. Stateless = strictly per-request. |
| **Synchronous batch with shared engine/lock** | "Batch is just a loop — share the engine for speed." | A single `Arc<RwLock<Engine>>` across batch items serializes them and defeats the concurrency v4.0 already built (Semaphore + JoinSet). One panicking item poisons the shared engine. | Keep the current per-task throwaway engine (`execute_single` builds its own). Sharing is for the *reloaded* engine, not for execute paths. |
| **GraphQL / streaming execute response** | "One query for execute + coverage + trace + metrics." | Adds a transport layer, schema, and auth model for a single feature. The audience (rule authors) is small; REST + a JSON body already carries everything. | REST `POST /v1/execute` with optional fields. WebSocket (already present in v4.0 for monitoring) can stream live-run updates if needed. |
| **Auto-reload on every execute** | "Always run the freshest rules." | Re-compiling the rule set on every request makes execute O(rules) per call instead of amortized; kills throughput. The hot-reload watcher (`start_file_watcher`, `execution.rs:529`) already handles freshness. | `EngineState` holds the compiled engine; reload is event-driven (file watcher / explicit `POST /v1/rules/reload`). Execute reads the current `self.engine`. |
| **Client-side rule execution in the browser** | "Run rules in the frontend for instant feedback." | Duplicates the engine in WASM, drifts from the authoritative Rust implementation, and bypasses governance/audit/coverage. Two engines = two truths. | Frontend calls the backend execute API; the test-runner is a thin client over `/v1/execute`. |
| **Breakpoints / step-through debugger in v5.0** | "I want to step into a rule." | A real step-debugger requires engine-level pause/resume, a debug protocol, and a UI state machine — a milestone of its own. Stuffing it into v5.0 explodes scope. | v5.0 ships the inference trace (post-hoc step list). Defer live breakpoints to a later milestone; the trace covers 80% of debugging need. |

## Feature Dependencies

```
[Real single-execute wiring]
    ├──requires──> [Engine.load_rules actually loading rule store → engine]
    ├──requires──> [WorkingMemory → facts_after serializer]
    └──enables──> [Batch-execute drives real execution] (inherits for free)
                  [CoverageTracker auto-populated]
                  [Prometheus emission + live dashboard]
                  [Frontend Run button]
                  [Side-by-side engine comparison]

[Per-request engine selection]
    ├──requires──> [EngineRegistry wired into execute_single]
    └──requires──> [Config-defined default engine]

[Async reload-then-execute]
    ├──requires──> [reload_engine swaps compiled engine into EngineState]
    └──requires──> [execute_single reads from EngineState, not throwaway]

[Stateful sessions]
    ├──requires──> [SessionStore (Arc<RwLock<HashMap>>)]
    ├──requires──> [execute_single signature change to accept shared state]
    └──enables──> [Multi-interaction test-runner scenarios]

[Frontend Run panel]
    ├──requires──> [new frontend/src/api/execute.ts]
    ├──requires──> [ExecuteResponse types in frontend/src/api/types.ts]
    └──enhances──> [Decision-table row-match highlighting] (needs per-row match data)

[Inference trace surfaced]
    ├──requires──> [enable_debug_trace(true) path + InferenceTraceCollector → InferenceTrace]
    └──enhances──> [Per-rule timing] (timestamp deltas)

[CoverageTracker auto-populated]
    └──enables──> [Dead-rule detection fed by real runs]
                  [Coverage reporting fed by real runs]
                  [Per-rule coverage in test-runner]

[Side-by-side engine comparison]
    └──requires──> [Per-request engine selection] + [Real single-execute]
```

### Dependency Notes

- **Real single-execute is the keystone.** Six table-stakes and four differentiators are downstream of it; until `execute_single` actually fires rules, nothing else has live data. Phase ordering must put it first.
- **CoverageTracker population requires a shared, process-wide tracker.** Per-request trackers can't feed monitoring. This is an architectural decision (inject `Arc<RwLock<CoverageTracker>>` via `web::Data`), not just a call-site addition.
- **Async reload-then-execute conflicts with throwaway engines in `execute_single`.** Today `execute_single` builds `EngineBuilder::new().build()` and discards it. To execute against reloaded rules, `execute_single` must instead read the compiled engine from `EngineState` — a signature change from `execute_single(req)` to `execute_single(req, engine_state, tracker)`. This ripples into `batch_execute_rules`.
- **Decision-table row-match highlighting couples engine output to UI props.** The engine currently reports `rules_fired: usize`, not per-row match status. Enabling row highlighting requires extending the execution result shape — design this before implementing the UI prop, or the UI will be built against a shape the engine can't fill.
- **Stateful sessions depend on shared-state injection** (the same `web::Data` change as reload-then-execute). Build the shared-state plumbing once and both features ride it.
- **Frontend Run panel is gated on a new `api/execute.ts`.** The existing `api/rules.ts` is CRUD-only with no execute function; the test-runner cannot be built until that client exists.

## MVP Definition

### Launch With (v5.0 — "Live Execution")

The minimum that makes the platform *execute* and the monitoring surfaces *read live data*.

- [ ] **Real single-execute** (`POST /v1/execute` fires rules, returns truthful `rules_fired`/`rules_not_fired`/`facts_after`) — closes the documented stub; the milestone's core promise.
- [ ] **Per-request engine selection** (`engine` field + config default) — exercises the v3.0 pluggable engines; without it the selector is dead code.
- [ ] **Stateless one-shot mode** as the default — the dominant execution pattern for API-driven decisions.
- [ ] **CoverageTracker auto-populated** from every execute — the bridge that makes v4.0 monitoring surfaces show live data.
- [ ] **Prometheus metrics emission** per execute (latency, rules fired, engine label) — makes the dashboard real.
- [ ] **Async reload-then-execute** actually swaps the compiled engine — makes hot-reload meaningful for execution, not just file watching.
- [ ] **Batch-execute drives real execution** — inherited once single is real; verify only.
- [ ] **Frontend Run button + fired-rules/results panel** in the rule editor — the user-facing proof that execution works.
- [ ] **Inference trace surfaced on demand** (`trace: true`) — minimum debugging support; the `InferenceTrace` types already exist.

### Add After Validation (v5.x)

- [ ] **Stateful KIE-session-style sessions** — multi-interaction execution; high value but a separate session-lifecycle subsystem (store, TTL, dispose). Trigger: users wanting conversational/multi-step decisions.
- [ ] **Side-by-side engine comparison** — the marquee differentiator; build once single-execute + per-request selection are stable. Trigger: users asking "which engine should I pick?"
- [ ] **Decision-table row-match highlighting** — high UX value, high coupling cost. Trigger: decision-table authors wanting visual match feedback.
- [ ] **Per-rule timing in trace** — cheap once trace is shipped; add the `duration_us` field and a timeline render. Trigger: performance tuning requests.

### Future Consideration (v6+)

- [ ] **Expected-vs-actual diff + "Save as test"** — bridges execution to regression testing; a milestone of its own given the `TestRun` entity + UI.
- [ ] **Live breakpoints / step-through debugger** — deferred per anti-feature rationale; the inference trace covers most debugging.
- [ ] **WebSocket streaming of live-run trace** — incremental trace events as they fire, instead of post-run payload.

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Real single-execute wiring | HIGH | MEDIUM | P1 |
| CoverageTracker auto-populated | HIGH | MEDIUM | P1 |
| Per-request engine selection | HIGH | MEDIUM | P1 |
| Stateless one-shot mode | HIGH | LOW | P1 |
| Prometheus metrics emission per execute | HIGH | MEDIUM | P1 |
| Async reload-then-execute (swap engine) | HIGH | MEDIUM | P1 |
| Batch-execute drives real execution | MEDIUM | LOW | P1 |
| Frontend Run button + results panel | HIGH | MEDIUM | P1 |
| Inference trace surfaced on demand | MEDIUM | MEDIUM | P1 |
| Run-panel engine selector dropdown | MEDIUM | LOW | P2 |
| Per-rule timing in trace | MEDIUM | MEDIUM | P2 |
| Side-by-side engine comparison | HIGH | HIGH | P2 |
| Stateful sessions | HIGH | HIGH | P2 |
| Coverage/dead-rule fed by real runs | HIGH | LOW | P2 |
| Decision-table row-match highlighting | HIGH | HIGH | P2 |
| Expected-vs-actual + Save as test | HIGH | HIGH | P3 |
| Live breakpoints / step debugger | MEDIUM | HIGH | P3 |

**Priority key:**
- P1: Must have for the v5.0 milestone to satisfy "live execution" + "monitoring reads live data."
- P2: Should have; high value but separable subsystems — add in v5.x once P1 is stable.
- P3: Future; deferred per anti-feature rationale or scope size.

## Competitor Feature Analysis

| Feature | Drools / KIE Server | DMN (Kogito / RHPAM) | rRules (v5.0 plan) |
|---------|---------------------|----------------------|--------------------|
| Single execute | `POST /containers/instances/{id}` with `BatchExecutionCommand` | `POST /dmn/models/{model}` (business-domain) or `/dmnresult` (full) | `POST /v1/execute` with `ExecuteRequest` (facts + optional rule_filter + engine + session_id) |
| Batch execute | Not native — one `BatchExecutionCommand` wraps multiple *commands*, not multiple evaluations | No native batch — one model instance per call | `POST /v1/rules/batch-execute` with real per-item concurrency (already built) |
| Stateless vs stateful | `lookup` required (stateless) vs optional (stateful); `DisposeCommand` for stateful | N/A (DMN is stateless by spec) | `session_id` absent = stateless one-shot; present = stateful session via `SessionStore` |
| Per-request engine selection | Not exposed (algorithm fixed in `kmodule.xml`) | N/A (single DMN evaluator) | **Differentiator**: `engine` param selects Rete/TREAT/LEAPS/PHREAK per request |
| Engine comparison | Not offered | Not applicable | **Differentiator**: side-by-side diff across 4 algorithms |
| Fired-rules trace | `out-identifier` on `fire-all-rules` returns count, not named trace; full trace needs audit/JBoss tooling | `decisionResults[]` shows decisions, not rule-level trace | `InferenceTrace` with per-entry timestamp/event_type/rule_name (opt-in) |
| Coverage / dead-rule | Requires external tooling (KIE scanner) | Not native | **Differentiator**: `CoverageTracker` fed by every execute → dead-rule detection + coverage dashboard |
| Test-runner UI | Drools workbench test runner (separate) | DMN modeler test (basic) | Run panel in-rule-editor (P1) + Save-as-test (P3) |
| Working-memory inspector | IBM ODM Working Memory view; Drools via IDE debug | N/A | Inference trace + `facts_after` (post-inference diff); live WM inspector deferred |
| Per-rule timing | Via external profiler | Not native | Per-entry `duration_us` in trace (P2) |

## Sources

- Drools / KIE Server REST execution (BatchExecutionCommand, lookup, stateless vs stateful, DisposeCommand): Red Hat PAM 7.9 Runtime Commands — https://docs.redhat.com/en/documentation/red_hat_process_automation_manager/7.9/html/deploying_and_managing_red_hat_process_automation_manager_services/runtime-commands-con_kie-apis ; Red Hat Decision Manager 7.13 KIE Server REST API — https://docs.redhat.com/en/documentation/red_hat_decision_manager/7.13/html/deploying_and_managing_red_hat_decision_manager_services/kie-server-rest-api-con_kie-apis (confidence: MEDIUM, web)
- DMN decision-service REST endpoints (business-domain vs `dmnresult`, singleton coercion, no native batch, no rule-level trace): KIE docs GitHub — https://github.com/kiegroup/kie-docs/blob/ef14e5d2d3fe5ac85299a4e7d1f0709ce9f18bd7/doc-content/shared-kie-docs/src/main/asciidoc/KieServer/ref-rest-api-dmn-endpoints.adoc ; KIE Community — https://blog.kie.org/2020/08/kogito-2840-about-dmn-endpoints-on-kogito.html (confidence: MEDIUM, web)
- Rule-engine test-runner UI patterns (fired-rules, working-memory deltas, inference trace, coverage, timing, diff highlighting): GoRules testing — https://gorules.mintlify.app/learn/authoring/testing ; IBM ODM ruleset debugging — https://www.ibm.com/docs/en/dbaoc?topic=rulesets-ruleset-debugging-tools ; OpenRules rule debugger — https://openrulesdecisionmanager.com/graphical-decision-modeling/rule-debugger/ ; Progress Corticon trace — https://docs.progress.com/bundle/corticon-js-rule-modeling/page/Trace-rule-execution.html ; DecisionRules rule testing — https://docs.decisionrules.io/doc/rules/common-rule-features/rule-testing (confidence: MEDIUM, web)
- rRules codebase (grounding): `api/src/v1/execution.rs` (stub at line 307, request/response types, batch + reload already built), `engine/src/environment.rs` (`EngineBuilder`, `Engine::create_session`, `Session`, `ExecutionResult`), `engine/src/engine/registry.rs` (`EngineRegistry`, `ExecutionEngine` trait, `EngineType::all_variants`), `engine/src/engine/selector.rs` (`EngineSelector`, `EngineConfig`), `engine/src/coverage/mod.rs` (`CoverageTracker`, `record_evaluation`/`record_fire`/`record_skipped`), `frontend/src/routes/rules/$id.edit.tsx` (editor with no Run button), `frontend/src/api/rules.ts` (CRUD-only, no execute), `frontend/src/api/client.ts` (`/v1` baseURL + JWT bearer) (confidence: HIGH, local code)

---
*Feature research for: BRMS live execution surface + frontend test-runner (rRules v5.0)*
*Researched: 2026-07-02*