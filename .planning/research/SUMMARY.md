# Project Research Summary

**Project:** rRules (BRMS) — Milestone v5.0 "Live Execution"
**Domain:** Wiring live rule execution into an existing Rust actix-web + React BRMS
**Researched:** 2026-07-02
**Confidence:** HIGH

## Executive Summary

rRules v5.0 is a brownfield **wiring** milestone, not a build milestone. The real pluggable engines (Rete/TREAT/LEAPS/PHREAK via `EngineRegistry`), the `CoverageTracking` singleton, the hot-reload plumbing (`EngineState`, `ReloadJob`, `JobStore`), the frontend monitoring dashboards and their contract adapters, and the Prometheus workspace crate are all already in place. The gap is that `POST /v1/execute` (`api/src/v1/execution.rs:307`) routes through a **stub** `engine::environment::Engine` (`load_rules`, `execute_forward`, `Session::execute` all return placeholder strings) instead of the real `EngineRegistry`/`ExecutionEngine` surface. As a result nothing fires, `rules_fired` is hardcoded to `0`, `facts_after` echoes the request input, `CoverageTracking` never receives data, and every monitoring dashboard displays zeros. The keystone act of v5.0 is: **route `/v1/execute` through the real `EngineRegistry`** (share `Arc<EngineRegistry>` as `web::Data`; construct a fresh `Box<dyn ExecutionEngine>` per request via `spawn_blocking` since engines are `&mut self`/`!Clone`), and enable the engine cargo features on the `api` crate so `register_default_engines` actually registers factories.

The recommended approach is **connect-before-build**, ordered so each phase ends with a verifiable vertical slice and downstream features inherit live data for free. Once `execute_single` drives a real engine, `CoverageTracking` auto-populates (ReteEngine already calls `record_rule_eval`/`record_rule_fire` at `rete/mod.rs:293,332`) and the three frontend dashboards (Engine, Coverage, DeadRule) display live data with **zero protocol/UI changes** — their adapters already tolerate the real shapes. A small set of new wiring is required alongside the keystone: an `ExecutionCounters` (`AtomicU64`) struct + `prometheus` (already a workspace dep, promote into `api/Cargo.toml`) drives `/metrics` and the WebSocket broadcast; a shared `Arc<RwLock<RuleSet>>` (or `ArcSwap`) makes `reload_engine` actually swap rules; `RuleExecutionTracker` must be converted to a `LazyLock<Mutex<…>>` singleton (today `dead_rules_endpoint` instantiates a fresh empty tracker at `metrics.rs:214`, so it always returns `[]`); and `facts_after` must be serialized from post-execution `WorkingMemory` rather than echoing the request JSON.

The principal risks are (1) treating `EngineState.engine` (the stub `environment::Engine`) as a shared mutable execution engine — `execute(&mut self)` plus an actix `RwLock` write-lock would serialize the entire API and collapse the batch `Semaphore + JoinSet` concurrency; (2) coverage double-counting — `record_rule_fire` is called **inside the per-nagram inner loop** at `rete/mod.rs:332`, so one logical firing is recorded as many; (3) the `reload_engine` no-op being mistaken for working hot-reload (it acquires and drops a write lock at `execution.rs:163-166`, never recompiles, and the async branch discards its result so `GET /v1/jobs/{id}` 404s); and (4) feature-gating — `register_default_engines` only registers Rete today, gated by `engine-rete`; the API build must enable all four engine cargo features or the `engine` param is a lie. Mitigations: share the **factory**, construct a fresh engine per request; record coverage once per rule per execution outside the inner loop; make reload swap-don't-mutate plus a generation tag and write a real `ReloadJob` to `JobStore`; and enable engine features in the `api` crate build. Stack additions are minimal — only `dashmap` 6.2.1 for the session store and the already-pinned `prometheus` 0.13 promoted into `api/Cargo.toml`. **No engine pooling is needed** — construction is O(1) today because `load_rules` is a stub; revisit only if a future milestone makes `load_rules` parse real rule files.

## Key Findings

### Recommended Stack

See `.planning/research/STACK.md` for full rationale. The milestone reuses the existing actix-web 4.4 / tokio "full" / engine crate / hybrid Git+PostgreSQL stack verbatim. Only two NEW additions are required.

**Core technologies:**
- `dashmap` 6.2.1 — concurrent session store for stateful `/v1/execute/session/{id}` calls. The existing `Arc<RwLock<HashMap>>` pattern (used for `job_store`, `user_store`) serializes every session lookup behind a single write lock; DashMap is sharded, takes `&self`, and fits `web::Data`'s `Send + Sync + 'static` bound. MSRV 1.65 fits the toolchain.
- `prometheus` 0.13 (workspace, **promote into `api/Cargo.toml`**) — register API-layer `IntCounter`/`Gauge`/`Histogram` (`execute_requests_total`, `execute_rules_fired_total`, `execute_duration_seconds`, `active_sessions`) for live `/metrics` output. Already used by `engine/src/metrics/prometheus.rs`; this is **extension, not replacement** — the engine-side per-engine gauges stay as-is.

**Supporting (already-present, no version change):**
- `tokio` 1.35 ("full") — `task::spawn_blocking` for CPU-bound `engine.execute()` (keeps the actix worker free); `time::interval` for session TTL sweep.
- `tracing` 0.1 — `#[tracing::instrument]` spans carrying `execution_id`, `engine`, `session_id`, `rules_fired` to feed the inference-trace UI without a new logging crate.

**Critical version requirements:** `dashmap` must be pinned to `6.2.1` (not `7.0.0-rc2` — no semver guarantees). `prometheus` must match the workspace pin `0.13` on both sides to avoid version skew.

**What NOT to add:** `deadpool`/`r2d2` engine pool (construction is O(1) — premature optimization), `tower-sessions`/`actix-session` (those solve user auth sessions, not rule-engine working-memory sessions; Phase 22 JWT auth already exists), a new abstraction over `EngineSelector`/`EngineRegistry` (would duplicate `EngineFactory`), `sqlx`-backed session persistence (`ExecutionEngine` is not serializable), or a second async runtime.

### Expected Features

See `.planning/research/FEATURES.md` for full landscape and prioritization matrix.

**Must have (table stakes — P1):**
- Real single-execute wiring (`POST /v1/execute` fires rules, returns truthful `rules_fired`/`rules_not_fired`/`facts_after`) — closes the documented stub; the milestone's core promise.
- Per-request engine selection (`engine` field on `ExecuteRequest` + config default via env `RRULES_DEFAULT_ENGINE`) — exercises the v3.0 pluggable engines; without it the selector is dead code.
- Stateless one-shot mode as the default — the dominant pattern for API-driven decisions.
- `CoverageTracker` auto-populated from every execute — the bridge that makes v4.0 monitoring surfaces show live data.
- Prometheus metrics emission per execute (latency histogram, rules fired, engine label) — makes the dashboard real.
- Async reload-then-execute actually swaps the compiled engine — makes hot-reload meaningful for execution, not just file watching.
- Batch-execute drives real execution — inherited once single is real; verify only.
- Frontend Run button + fired-rules/results panel in the rule editor — the user-facing proof that execution works.
- Inference trace surfaced on demand (`trace: bool`, opt-in; off by default for perf) — minimum debugging support; the `InferenceTrace` types already exist.

**Should have (differentiators — P2, v5.x):**
- Side-by-side engine comparison on one input (Rete AND TREAT AND LEAPS AND PHREAK) — no competitor exposes four Rete-family algorithms in one UI.
- Per-execution Prometheus metric emission + live dashboard correlation within seconds.
- Coverage + dead-rule detection fed by real runs (couples execution to the v4.0 monitoring surfaces).
- Stateful KIE-session-style sessions (multi-interaction: insert facts, fire, insert more, fire again, dispose).
- Decision-table row-match highlighting (green/yellow/red) — high UX value, high coupling cost.
- Per-rule / per-node timing in the trace (`duration_us`).
- Run-panel engine selector dropdown wired to the runtime selector.

**Defer (v6+, anti-features):**
- Always-on full inference trace (doubles latency — keep opt-in).
- Cross-request shared working memory in stateless mode (race conditions, non-deterministic).
- Synchronous batch with shared engine/lock (defeats existing concurrency; one panicking item poisons the engine).
- GraphQL / streaming execute response (small audience; REST + JSON suffices; WS already exists for monitoring).
- Auto-reload on every execute (re-compiles per request — kills throughput; reload is event-driven).
- Client-side rule execution in the browser (WASM drift; bypasses governance/audit/coverage; two engines = two truths).
- Live breakpoints / step-through debugger (a milestone of its own — the inference trace covers ~80% of debugging).
- Expected-vs-actual diff + "Save as test" (P3 — a `TestRun` entity + UI; bridge to regression testing).

### Architecture Approach

See `.planning/research/ARCHITECTURE.md` for component map and per-subsystem data flows.

The pivotal finding: **two unrelated "engine" surfaces exist and are NOT connected.** `engine::environment::Engine` (the stub) is built by `/v1/execute` and never fires; `engine::engine::ExecutionEngine` + `EngineRegistry` (the real one, with `ReteEngine::execute_rules` actually propagating the Rete network and calling `CoverageTracking`) is dead code from the API's perspective. v5.0's central act is to route `/v1/execute` through the real surface.

**Major components:**
1. `EngineRegistry` (shared `web::Data`) — holds registered factories; `create_engine_by_name()` produces a fresh owned `Box<dyn ExecutionEngine>` per request. Build once at startup; extend `register_default_engines` to register all four factories (today only Rete, gated by `engine-rete` feature).
2. `EngineSelector` — resolves default engine name from config + validates per-request `engine` param. Keep separate from the registry (config object vs. factory).
3. `execute_single` / `execute_rules` (modified) — resolve engine, load shared RuleSet, insert facts, call `engine.execute()` inside `spawn_blocking`, return real `rules_fired`/`facts_after`. Signature change: accept `web::Data<EngineRegistry>` + `web::Data<EngineSelector>` (+ later `EngineState` for the reloaded RuleSet).
4. `SessionManager` (NEW, `api/src/v1/sessions.rs`) — owns `Box<dyn ExecutionEngine>` per session_id; TTL eviction via background `tokio::time::interval`. Does NOT salvage the abandoned `environment::Session` stub.
5. `ExecutionCounters` (NEW, `engine/src/monitoring/counters.rs` or `api/src/metrics.rs` extension) — `AtomicU64` counters read by `/metrics`, `/v1/monitoring/engine-state`, and broadcast via the existing `metrics_tx` WS channel.
6. `RuleExecutionTracker` (singleton-ized) — mirror the `CoverageTracking` `LazyLock<Mutex<…>>` pattern; add `record_*` calls next to `CoverageTracking` calls in `rete/mod.rs:293,332`.
7. Shared `RuleSet` snapshot (`Arc<RwLock<RuleSet>>` or `ArcSwap<RuleSet>`) — built once at startup from `RULE_STORAGE_PATH`, updated atomically by reload; read per request. Decouples "which rules" (shared, reloadable) from "the engine instance" (per-request, not shared).
8. Frontend `api/execution.ts` + `TestRunner` component (NEW) — thin client over `/v1/execute`; monitoring dashboards need NO frontend changes (adapters already wired).

**Key patterns:**
- **Factory shared, instance per-request** — `web::Data<EngineRegistry>`; fresh `Box<dyn ExecutionEngine>` per call. Avoids shared mutable state, preserves the batch "no shared locks" contract.
- **Process-wide singleton via `LazyLock<Mutex>`** — `CoverageTracking` (existing) and `RuleExecutionTracker` (new). Any handler reads live data without `web::Data` plumbing.
- **ArcSwap/Arc-RwLock for hot-reloadable shared snapshot** — readers never block on reload; reload is atomic.
- **Generation tag on every `ExecuteResponse`** — solves both the hot-reload race and frontend staleness with one field.

**Anti-patterns to refuse:**
- Sharing a mutable engine across requests (serialize + stale-after-reload).
- Routing through the `environment::Engine` stub (`execute_forward` returns "Forward chaining not yet implemented").
- Per-request rule parsing (push parse cost onto the hot path).
- Persisting sessions to PostgreSQL (`ExecutionEngine` is not serializable).

### Critical Pitfalls

See `.planning/research/PITFALLS.md` for full warning signs and prevention strategies.

1. **`&mut self` engines shared across actix workers serialize all execution** — Do NOT run execution against `EngineState.engine` (the stub). Treat `EngineState.engine` as a read-only compiled-rule-base cache; construct a fresh `ReteEngine` per request from the cached `RuleSet`. Keep the batch `Semaphore + JoinSet` intact. `ExecutionEngine: Send + Sync` does NOT imply `execute(&mut self)` is safe to share — `Sync` only makes `&self` shareable.
2. **`RuleExecutionTracker` is not a singleton — `GET /v1/metrics/dead-rules` always returns `[]`** — `dead_rules_endpoint` does `RuleExecutionTracker::new()` at `metrics.rs:214` and `ReteEngine` never calls `record_fire` on it. Convert to `LazyLock<Mutex<…>>` (mirroring `CoverageTracking`), add `record_*` calls next to `CoverageTracking` calls in `rete/mod.rs`. Pin with an integration test: after one `/v1/execute` firing rule `r1`, dead-rules must NOT contain `r1` and coverage must show `r1.fired == true`.
3. **Coverage double-counting at `rete/mod.rs:332`** — `record_rule_fire` is called inside the per-nagram inner loop, so one logical firing is recorded as many. Three independent counters (`CoverageTracking`, `RuleExecutionTracker.total_fires`, per-engine `metrics.record_rule_fired`) drift. Define one canonical "fire" semantics: once per rule per execution, outside the inner loop (collect `HashSet<rule_id>` during the loop, record after). Add a `record_coverage: bool` flag on `BatchExecuteRequest` (default `false`) so batch doesn't pollute production coverage.
4. **`reload_engine` is a no-op** — Acquires + drops write lock at `execution.rs:163-166`, never recompiles, never writes `ReloadJob` to `JobStore`, so `GET /v1/jobs/{id}` 404s. The moment wiring replaces `_engine` with real recompilation, two races appear (read-vs-write on shared Engine; coverage/tracker corruption mid-fire with ghost rule_ids). Make reload **swap-in-place**, not in-place mutation: build the new `RuleSet` fully off to the side, then under the write-lock do a single `*self.engine = new`. Add a generation counter on `Engine` and include `engine_generation` in `ExecuteResponse`.
5. **`facts_after` echoes the input JSON** — `execution.rs:327` clones the request facts into `facts_after` regardless of engine mutation; users will see "facts unchanged" even when rules demonstrably modified facts. After wiring, build `facts_after` from the engine's post-execution `WorkingMemory` via a reverse `engine_value_to_json` mapping. Test: a rule that sets `score := score + 10` must show the incremented `score`.
6. **Two colliding `EngineState` structs** — `execution.rs:75` (shared `Arc<RwLock<environment::Engine>>`) and `metrics.rs:12` (broadcast payload) share a name and are both passed via `web::Data`. Rename the metrics one to `MetricsSnapshot` early as a prep commit, before adding logic.
7. **Feature-gating** — `register_default_engines` only registers `ReteEngineFactory` under `#[cfg(feature = "engine-rete")]`; the `treat`/`leaps`/`phreak` modules are gated by their own cargo features but `register_default_engines` does NOT register them even when those features are on. Enable all four engine cargo features in the `api` crate build, and verify the other three `EngineFactory` impls exist before promising >1 engine in the API.
8. **Per-request engine construction cost** (moderate) — The stub builds `EngineBuilder::new().build()` per call; wiring naively re-compiles the Rete network on every request. Construction is O(1) today because `load_rules` is a stub, but a Phase 1 spike must measure compile cost on a representative ruleset; if it's >1ms, ship an engine pool (keyed by `(engine_name, ruleset_version)`) in Phase 1, not Phase 2. `Engine::clone()` saves the rule parse but not the network compile — confirm by inspection which engines are `Clone`.

## Implications for Roadmap

Based on research, suggested 6-phase structure (mirrors ARCHITECTURE.md §7 build order, ordered connect-before-build so each phase ends with a verifiable vertical slice):

### Phase 1: Execute wiring core (route through EngineRegistry)
**Rationale:** This is the keystone — six table-stakes and four differentiators are downstream of it; until `execute_single` actually fires rules, nothing else has live data. It also fixes the engine-ownership contract before any handler is wired, avoiding the serialization anti-pattern.
**Delivers:** `POST /v1/execute` returns non-zero `rules_fired` from a real `ReteEngine` (start with empty RuleSet — proves wiring). Adds `engine: Option<String>` to `ExecuteRequest`. Builds + shares `Arc<EngineRegistry>` + `EngineSelector` as `web::Data` in `main.rs`. Enables `engine-rete` feature on the `api` crate.
**Addresses features:** Real single-execute wiring, per-request engine selection (scaffolding), stateless one-shot mode, `CoverageTracker` auto-populated (free, since ReteEngine already calls it).
**Avoids pitfalls:** `&mut self` engine sharing (Pitfall 1 — factory-shared/instance-per-request), engine selector misroute (Pitfall 2 — capability-check scaffolding), tracker singleton unification (Pitfall 3 — must land alongside the first real execute), canonical fire semantics (Pitfall 6 — record once-per-rule outside the nagram loop), `facts_after` echo bug (Pitfall 11), `EngineState` name collision (Pitfall 10 — prep rename), per-request compile cost spike (Pitfall 7).
**Research flag:** YES — `register_default_engines` extension (verify the three other `EngineFactory` impls exist), `RuleSet` → `ExecutionEngine` bridging API (the trait does not expose `load_ruleset`), `inference_trace` population source (ReteEngine vs. derived from coverage).

### Phase 2: Execution counters + metrics broadcast (live monitoring)
**Rationale:** Phase 2's counters are incremented by Phase 1's `execute_single`; without Phase 1, this reads zeros forever. Independent of RuleSet loading (Phase 3) — dashboards light up even with an empty rule set ("0 fired, 0 dead" — still live, not hardcoded).
**Delivers:** `ExecutionCounters` (`AtomicU64`) as `web::Data`. `metrics_endpoint` (Prometheus) and `get_engine_state` read counters; broadcast `EngineState` via `metrics_tx` after each execution. The three frontend dashboards (Engine, Coverage, DeadRule) display live data with **zero protocol/UI changes** — their existing adapters already tolerate the real shapes.
**Uses stack:** `prometheus` 0.13 (promote into `api/Cargo.toml`); `tokio` broadcast channel (existing `metrics_tx`).
**Implements components:** `ExecutionCounters`, `metrics_tx` producer.
**Addresses features:** Prometheus metrics emission per execute, coverage/dead-rule fed by real runs (CoverageTracker portion — RuleExecutionTracker comes in Phase 4).
**Avoids pitfalls:** Stale polling race (Pitfall 8 — extend `ExecuteResponse` with fired-rule list + `execution_id` correlation + `engine_generation`).

### Phase 3: Real reload + JobStore (execute against refreshed rules)
**Rationale:** Phase 3 adds RuleSet loading into the `execute_single` that Phase 1 made real. Doing Phase 3 first would load rules into a stub that never fires them. Reload currently is a no-op (acquires + drops write lock at `execution.rs:163-166`) and `JobStore` is never written, so `GET /v1/jobs/{id}` 404s.
**Delivers:** Shared `Arc<RwLock<RuleSet>>` (or `ArcSwap<RuleSet>`) built at startup from `RULE_STORAGE_PATH`; `execute_single` loads it into each request's engine. `reload_engine` actually rebuilds the shared RuleSet and **swaps-in-place** (not in-place mutation). Async reload branch writes `ReloadJob{Pending→Running→Completed/Failed}` to `JobStore` so `GET /v1/jobs/{id}` works. Generation counter on `Engine`, included in `ExecuteResponse`.
**Implements components:** Shared `RuleSet` snapshot, real `reload_engine`, `JobStore` population.
**Addresses features:** Async reload-then-execute (swap engine), batch-execute drives real execution (inherits for free — verify only).
**Avoids pitfalls:** Hot-reload race (Pitfall 4 — swap-don't-mutate + generation tag + in-flight drain/generation coordination), no-op `reload_engine` (Pitfall 12), stub `start_file_watcher` (Pitfall 13 — implement or drop claim), async job not stored (Pitfall 14).
**Research flag:** YES — `RuleSet` → Rete network bridging (read `rete/mod.rs` network construction in full); `ArcSwap` vs `Arc<RwLock<RuleSet>>` perf trade-off (measure, don't guess).

### Phase 4: RuleExecutionTracker singleton + dead-rule hooks
**Rationale:** Phase 4's `record_*` calls are added next to Phase 1's existing `CoverageTracking::record_*` calls in the same `rete/mod.rs:293,332` lines. Must land after Phase 1 (tracker needs real firings to record) but before Phase 6 (frontend test-runner's dead-rules panel depends on it). Pin with the end-to-end integration test (single `/v1/execute` → coverage shows `r1.fired == true` → dead-rules excludes `r1`).
**Delivers:** `RuleExecutionTracker` converted to `LazyLock<Mutex<…>>` singleton with associated `record_evaluation`/`record_fire` functions. `record_*` calls added in `rete/mod.rs` alongside `CoverageTracking` calls. `dead_rules_endpoint` reads from the singleton (optionally upgraded to `DeadRuleInfo[]` via `DeadRuleReport::generate` with adapter update).
**Implements components:** Singleton-ized `RuleExecutionTracker`, dead-rule live data.
**Addresses features:** Coverage/dead-rule detection fed by real runs (the dead-rule portion).
**Avoids pitfalls:** Stale tracker data (Pitfall 3 — single source of truth), three-counter drift (Pitfall 6 — unify counters around `CoverageTracking` as canonical, or delegate).

### Phase 5: Stateful sessions
**Rationale:** Sessions wrap the per-request engine that Phase 1 built. A separate session-lifecycle subsystem (store, TTL, dispose), high-value but separable from P1 — schedule for v5.x once P1 is stable. Must ship the registry + TTL + LRU + sweep **together** with the session endpoint, not as a follow-up (the OOM failure mode is silent and only manifests in production).
**Delivers:** NEW `api/src/v1/sessions.rs` with `Session` + `SessionManager` (`Arc<DashMap<String, StatefulSession>>`, TTL, LRU, max session count). `execute_single` upserts/looks-up session by `session_id`; stateless path unchanged. NEW routes `DELETE /v1/execute/session/{id}`, `POST /v1/execute/session/{id}/reset`. Background `tokio::time::interval` sweep task. `rrules_sessions_active` Prometheus gauge. Server-generated session IDs returned in response (treat client-supplied IDs as idempotency keys, not primary keys).
**Uses stack:** `dashmap` 6.2.1 (NEW).
**Implements components:** `SessionManager`.
**Addresses features:** Stateful KIE-session-style sessions, multi-interaction test-runner scenarios.
**Avoids pitfalls:** Session leaks (Pitfall 5 — TTL + LRU + sweep + max count + Prometheus gauge + RBAC-gated `/v1/sessions` admin endpoint).
**Research flag:** Light — TTL/sweep defaults are config knobs, not architecturally load-bearing.

### Phase 6: Frontend test-runner
**Rationale:** Phase 6 depends on Phases 1–4 being real so the UI shows meaningful data. Could be done in parallel with Phase 5 if the `session_id` path is deferred. The monitoring dashboards need NO frontend changes — Phase 6 is pure new UI over already-working APIs. Consume `ExecuteResponse` fired-rule list; do NOT poll for per-run results (the race is visible to users from day one).
**Delivers:** NEW `frontend/src/api/execution.ts` (`executeRule`, `batchExecute`, `getJobStatus`, `listJobs`) + `ExecuteRequest`/`ExecuteResponse`/`InferenceTrace`/`BatchExecute*` types in `frontend/src/api/types.ts`. NEW `TestRunner` component (facts editor, engine selector dropdown populated from a new `GET /v1/engines` or hardcoded `["rete"]` initially, Run/Test buttons, results panel with `rules_fired`/`rules_not_fired`/`facts_after`/`inference_trace`). Optional "Reload then Run" → `reloadRules({async:true})` + poll `getJobStatus` + `executeRule`.
**Implements components:** Frontend `api/execution.ts`, `TestRunner` panel.
**Addresses features:** Frontend Run button + fired-rules/results panel, inference trace surfaced on demand, run-panel engine selector dropdown.
**Avoids pitfalls:** Stale polling race (Pitfall 8 — drive refresh from the execute response, not from polling; keep polling only for the long-term monitoring dashboard), dead-rules panel empty (Phase 4 fix first).

### Phase Ordering Rationale

- **Phase 1 before 2:** Phase 2's counters are incremented by Phase 1's `execute_single`. Without Phase 1, Phase 2 reads zeros forever.
- **Phase 1 before 3:** Phase 3 adds RuleSet loading into the `execute_single` that Phase 1 made real. Doing Phase 3 first would load rules into a stub that never fires them.
- **Phase 2 independent of 3/4:** Dashboards light up as soon as executions run, even with an empty rule set (they show "0 fired, 0 dead" — still live, not hardcoded).
- **Phase 4 after 1:** Phase 4's `record_*` calls are added next to Phase 1's existing `CoverageTracking::record_*` calls in the same `rete/mod.rs` lines.
- **Phase 5 after 1:** Sessions wrap the per-request engine that Phase 1 built.
- **Phase 6 last:** It depends on Phases 1–4 being real so the UI shows meaningful data. Could be done in parallel with Phase 5 if the `session_id` path is deferred.
- **Connect-before-build grouping:** Every phase ends with a verifiable vertical slice. The cheapest end-to-end path is made real first.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1 (wiring core):** `register_default_engines` extension — verify the three other `EngineFactory` impls exist in `engine/src/engine/{treat,leaps,phreak}/mod.rs` before promising >1 engine in the API. `RuleSet` → `ExecutionEngine` bridging — the trait does not expose `load_ruleset`; rete/mod.rs builds its own network; define how `RuleSet` rules become Rete network nodes. `inference_trace` population source — `ReteEngine::execute_rules` does not emit a trace; decide whether to populate from ReteEngine or derive from coverage data. Per-request compile-cost spike (Pitfall 7) — measure on a representative ruleset; if >1ms, ship an engine pool in Phase 1.
- **Phase 3 (real reload + JobStore):** `ArcSwap` vs `Arc<RwLock<RuleSet>>` perf trade-off — measure under realistic read concurrency; `ArcSwap` adds a dep but is lock-free. In-flight execution coordination (drain vs. generation-accept) — pick one and document.

Phases with standard patterns (skip research-phase):
- **Phase 2 (counters + metrics broadcast):** straightforward `AtomicU64` + `prometheus::TextEncoder` extension; the engine-side exporter at `engine/src/metrics/prometheus.rs:45` is the reference impl.
- **Phase 4 (RuleExecutionTracker singleton):** direct mirror of the existing `CoverageTracking` `LazyLock<Mutex<…>>` pattern; the callsites are already known (`rete/mod.rs:293,332`).
- **Phase 5 (stateful sessions):** `DashMap` + TTL sweep is a well-trodden Rust pattern; `SessionManager` mirrors the existing `job_store`/`user_store` shape.
- **Phase 6 (frontend test-runner):** thin client over already-working REST; the monitoring dashboards are unchanged.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Minimal additions (`dashmap`, `prometheus` promote); both verified against the actual workspace `Cargo.toml` and the existing `engine/src/metrics/prometheus.rs` reference impl. |
| Features | MEDIUM | BRMS execution-API patterns cross-checked against Drools/KIE, DMN/Kogito, and five rule-engine test-runner UIs; grounded in the current rRules code. Competitor patterns are web-sourced (MEDIUM); rRules code grounding is HIGH. |
| Architecture | HIGH | Direct reading of the codebase; every integration point cited to file:line. The two-engine-surface discovery, per-request construction recommendation, engine resolution algorithm, CoverageTracking auto-population, RuleExecutionTracker bug, reload-engine no-op, JobStore gap, and frontend-already-wired finding are all HIGH confidence. |
| Pitfalls | HIGH | Derived from reading the actual codebase at cited file:lines. The `EngineState` name collision, `facts_after` echo bug, nagram-loop double-counting, and feature-gating pitfalls are confirmed by direct inspection. |

**Overall confidence:** HIGH

### Gaps to Address

- **TREAT/LEAPS/PHREAK factory registration:** `register_default_engines` only registers Rete. If v5.0 promises all four engines via the `engine` param, verify the three other `EngineFactory` impls exist and register them under their respective `cfg` blocks. Read `engine/src/engine/{treat,leaps,phreak}/mod.rs` before committing to >1 engine in the API. **Address in Phase 1 planning.**
- **RuleSet → ExecutionEngine bridging API:** The exact API for loading a `RuleSet` into a `ReteEngine` — the `ExecutionEngine` trait does not expose `load_ruleset`; `rete/mod.rs` builds its own network. Phase 3 must define how `RuleSet` rules become Rete network nodes. Read `rete/mod.rs` network construction in full. **Address in Phase 3 planning.**
- **`inference_trace` population:** `ExecuteResponse.inference_trace` is `Option<InferenceTrace>` and currently always `None`. `ReteEngine::execute_rules` does not emit a trace. Phase 6's "see fired rules" relies on either the trace or on `rules_not_fired`/`CoverageTracking` data. Decide whether to populate the trace from ReteEngine or derive the display from coverage data. **Address in Phase 6 planning.**
- **Per-request compile cost:** Construction is O(1) today because `load_rules` is a stub, but a Phase 1 spike must measure compile cost on a representative ruleset. If >1ms, ship an engine pool (keyed by `(engine_name, ruleset_version)`) in Phase 1, not Phase 2. `Engine::clone()` saves the rule parse but not the network compile — confirm which engines are `Clone`. **Address in Phase 1 spike.**
- **`ArcSwap` vs `Arc<RwLock<RuleSet>>`:** Both viable; choice is a perf trade-off not yet measured. Prefer `Arc<RwLock<RuleSet>>` to avoid a new dep unless contention shows up under realistic read concurrency. **Address in Phase 3 planning.**
- **Session TTL/sweep defaults:** Not architecturally load-bearing but needs a config knob. Pick values (default ~15 min idle TTL, ~24 h max lifetime, ~1024 max sessions). **Address in Phase 5 planning.**

## Sources

### Primary (HIGH confidence)
- rRules codebase (direct reads): `api/src/v1/execution.rs` (stub at 307–343; `EngineState` at 75; `reload_engine` no-op at 163–166; `execute_with_session` stub at 561; `start_file_watcher` stub at 529; `json_to_engine_value` at 533; batch semaphore at 379–395), `api/src/main.rs` (app state wiring 32–124; routes 152–217; file watcher spawn 79), `engine/src/engine/{registry,selector,rete/mod}.rs` (`EngineRegistry`, `ExecutionEngine: Send + Sync` at 69, `execute(&mut self)` at 66, `register_default_engines` at 158, `ReteEngine::execute_rules` at 321, `CoverageTracking` calls at 293/332, `ReteEngineFactory` at 392), `engine/src/environment.rs` (`EngineBuilder::build` 59; `create_session` ~88; `Engine` derives `Clone` 81; `load_rules` stub 122; `execute_forward` stub 132; `Session` stub 152), `engine/src/monitoring/{coverage,detection}.rs` (`CoverageTracking` singleton at 86; `RuleExecutionTracker` plain struct at 64), `api/src/metrics.rs` (Prometheus at 188; fresh `RuleExecutionTracker::new()` at 214; `CoverageTracking` read at 233; `metrics_tx` 22), `api/src/v1/routes/monitoring.rs` (hardcoded zeros at 30), `Cargo.toml` (workspace + per-crate), `frontend/src/api/{client,rules}.ts`, `frontend/src/hooks/useEngineMetrics.ts`, `frontend/src/features/monitoring/dashboards/*` (adapters present).

### Secondary (MEDIUM confidence)
- Drools / KIE Server REST execution (BatchExecutionCommand, lookup, stateless vs stateful, DisposeCommand): Red Hat PAM 7.9 Runtime Commands; Red Hat Decision Manager 7.13 KIE Server REST API.
- DMN decision-service REST endpoints (business-domain vs `dmnresult`, no native batch, no rule-level trace): KIE docs GitHub; KIE Community blog.
- Rule-engine test-runner UI patterns (fired-rules, working-memory deltas, inference trace, coverage, timing, diff highlighting): GoRules, IBM ODM, OpenRules, Progress Corticon, DecisionRules docs.
- crates.io: `dashmap` v6.2.1 stable, MSRV 1.65, ~292M downloads.
- docs.rs: `deadpool` v0.13.0 — evaluated and **rejected for v5.0** (premature; engine construction is O(1)).

### Tertiary (LOW confidence)
- None. All findings are grounded in either the codebase (HIGH) or external BRMS/rule-engine docs (MEDIUM).

---
*Research completed: 2026-07-02*
*Ready for roadmap: yes*