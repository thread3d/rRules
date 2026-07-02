# Pitfalls Research

**Domain:** Wiring live rule execution into an existing BRMS (rRules: Rust actix-web + pluggable Rete/TREAT/LEAPS/PHREAK engines + React frontend)
**Researched:** 2026-07-02
**Confidence:** HIGH (derived from reading the actual rRules codebase: `api/src/v1/execution.rs`, `engine/src/engine/{rete,treat,leaps,phreak}/mod.rs`, `selector.rs`, `registry.rs`, `environment.rs`, `monitoring/{detection,coverage}.rs`, `api/src/metrics.rs`, `api/src/main.rs`)

## Scope note

These are **integration pitfalls specific to adding live execution to THIS existing system** — mutable engines behind actix-web workers, an existing global `CoverageTracking` singleton, a `RuleExecutionTracker` that is currently instantiated empty per-request, a hot-reload path that takes a write-lock but does not recompile, and a frontend that polls `/v1/metrics/*`. They are not generic Rust advice. Each pitfall lists warning signs, a prevention strategy tied to the code as it exists today, and the phase that should address it.

## Critical Pitfalls

### Pitfall 1: `&mut self` engines shared across actix workers serialize all execution

**What goes wrong:**
The natural "fix" for the stub is to wire `execute_rules` to the shared `EngineState.engine` (`Arc<RwLock<engine::environment::Engine>>` already injected via `web::Data` in `api/src/main.rs:32,114`). But the algorithm engines (`ReteEngine::execute_rules(&mut self)`, `TreatEngine`, `LeapsEngine`, `PhreakEngine` — all `fn execute(&mut self)` in `registry.rs:66`) require **exclusive mutable access**. Taking `engine.write()` for every execution serializes the entire API: the batch endpoint's `Semaphore + JoinSet` concurrency (`execution.rs:379–395`) collapses to one-at-a-time, and every `/v1/execute` request blocks every other across all actix workers. Throughput drops to single-threaded.

**Why it happens:**
`EngineState` already exists and is shared via `web::Data`, so reaching for it is the obvious path. The `ExecutionEngine` trait is declared `Send + Sync` (`registry.rs:69`), which falsely suggests it is safe to share — but `Sync` only makes `&self` shareable; `execute(&mut self)` still needs a write-lock. The current stub sidesteps this by building a throwaway `EngineBuilder::new().build()` per call (`execution.rs:316`), which is mutation-free and so appears "obviously wrong" to fix toward the shared engine.

**How to avoid:**
Do **not** run execution against the shared `EngineState.engine`. Instead:
1. Treat `EngineState.engine` as a **read-only compiled-rule-base cache** (read-lock only, to clone out the `RuleSet` / compiled network).
2. Per request, **clone the engine** (`Engine` derives `Clone` — `environment.rs:81`) or build a fresh algorithm engine from the cached `RuleSet`, then mutate the clone. The current `execute_single` already builds a per-request engine; keep that pattern but populate it from the cached rule set rather than `EngineBuilder::new()` (empty).
3. For stateful sessions, store per-session `Engine` clones in a session registry (see Pitfall 5), not in `EngineState`.
4. Keep the batch `Semaphore + JoinSet` model intact — it only works because each task owns its engine.

**Warning signs:**
- Load tests showing throughput flat regardless of `max_concurrency` / actix worker count.
- `execute_rules` handler containing `engine_state.engine.write().await` (or `.write().unwrap()`).
- Batch endpoint latency scaling linearly with `requests.len()` instead of `requests.len() / max_concurrency`.
- `RwLock` write-lock contention appearing in profiles under concurrent load.

**Phase to address:**
**Phase 1 — Execution wiring core.** Decide the engine-ownership model before any handler is wired. Document the "shared cache, private execution clone" contract in the phase spec and enforce it with a lint or a wrapper type that exposes only `clone_rules()` from the shared state.

---

### Pitfall 2: Engine selector chooses an algorithm that does not fit the ruleset (silent wrong results or panics)

**What goes wrong:**
`EngineSelector::select_engine` (`selector.rs`) only validates the name is one of `rete|treat|leaps|phreak` — it does **not** check that the loaded ruleset is representable by that algorithm. LEAPS and PHREAK have different support for, e.g., accumulators/accumulation nodes (`leaps/mod.rs:342 execute_aggregations`) and truth maintenance (`phreak`). Selecting `leaps` for a ruleset that uses accumulations, or `phreak` for a ruleset relying on TREAT-style caching semantics, can silently drop firings, panic on unsupported node types, or produce different `rules_fired` counts vs Rete. The v3.0 contract says "all engine implementations must pass identical functional test suite" (`PROJECT.md` Constraints) — but per-request selection can violate that contract at runtime if a ruleset is loaded that only Rete handles.

**Why it happens:**
The selector is a pure string validator with no knowledge of the rule set's feature footprint. The `EngineConfig` carries an `options` map that is never consumed for capability negotiation. Per-request `engine` param (a v5.0 target) makes this user-controllable, so a caller can pick any engine for any ruleset.

**How to avoid:**
1. Add a **capability profile** per engine (a `fn supports(&self, rule_set: &RuleSet) -> Result<(), UnsupportedFeature>` on `ExecutionEngine`), and call it during engine construction in the execute path. Reject with `400` and a clear message listing the unsupported feature and the engine that does support it.
2. Default engine selection should be **ruleset-driven**, not config-driven: pick the cheapest engine that supports all features. Keep the explicit `engine` param as an override that **runs the capability check first** and refuses mismatched combos.
3. The functional-equivalence test suite (v3.0) should be extended with a "selector misroute" test: assert that a ruleset using accumulations rejects `leaps`.

**Warning signs:**
- `rules_fired` differing across engines for the same facts+ruleset (this is **the** signal — wire a parity assertion into integration tests).
- `select_engine` accepting any of the four names regardless of loaded rules.
- 500s or `unwrap()` panics deep inside `leaps`/`phreak` when a Rete-authored ruleset is run against them.
- Frontend showing different results for the "same" run when the user toggles the engine dropdown.

**Phase to address:**
**Phase 1 (wiring core)** for the capability-check scaffolding; **Phase 2 (per-request selection)** for the full ruleset-driven default and the override+reject flow. Do not ship the `engine` query param before the capability check exists.

---

### Pitfall 3: Stale / empty tracker data — monitoring reads pre-population state

**What goes wrong:**
Two trackers exist and they are wired inconsistently:
- `CoverageTracking` (`coverage.rs:93`) is a **process-global singleton** (`static INSTANCE: LazyLock<Mutex<TrackingState>>`) and is **already called from inside `ReteEngine::execute_rules`** (`rete/mod.rs:293,332` via `CoverageTracking::record_rule_eval` / `record_rule_fire`). So it will auto-populate the moment execution is wired. Good.
- `RuleExecutionTracker` (`detection.rs:64`) is **not** a singleton — it is a plain struct, and the dead-rules endpoint instantiates a **fresh empty one** each call: `let tracker = RuleExecutionTracker::new();` at `api/src/metrics.rs:214`. So `GET /v1/metrics/dead-rules` **always returns `[]`**, even after live execution is wired, because no one ever records into the tracker the endpoint reads. The dead-rule detection feature (MON-03) is effectively a no-op today and will remain so unless the tracker is made the single source of truth.

The pitfall: after wiring execution, the dashboard's coverage panel lights up (singleton) but the dead-rules panel stays empty (per-call instantiation), and the team concludes "execution isn't populating trackers" when in fact it's populating the **wrong** tracker.

**Why it happens:**
Two trackers with overlapping responsibilities (`CoverageTracking` for per-rule eval/fire counts; `RuleExecutionTracker` for dead-rule detection with categories and timestamps) evolved independently. Neither engine's `execute_rules` calls `RuleExecutionTracker::record_fire` — only `CoverageTracking::record_rule_fire`. The dead-rules endpoint assumes someone else populated a shared instance; no such shared instance exists.

**How to avoid:**
1. Make `RuleExecutionTracker` a singleton (`LazyLock<Mutex<...>>`) like `CoverageTracking`, **or** have the dead-rules endpoint read from the `CoverageTracking` singleton and project into `DeadRuleInfo` (a rule with `evaluation_count > 0 && fire_count == 0` is dead).
2. Inside each engine's `execute_rules`, call **both** trackers (or have one delegate to the other). The `record_fire` calls must carry `rule_id`, `rule_name`, and a timestamp — `rule_name` is currently only available on the action node, so thread it through.
3. Add an integration test: after a single `/v1/execute` that fires rule `r1`, `GET /v1/metrics/dead-rules` must **not** contain `r1`, and `GET /v1/metrics/coverage` must show `r1.fired == true`. This pins the end-to-end population path.
4. Make the `RuleExecutionTracker::new()` call at `metrics.rs:214` a compile error by removing `Default`-based construction from the API surface, or add a `#[deprecated]` to the public `new()` to catch stray local instantiation.

**Warning signs:**
- `GET /v1/metrics/dead-rules` returns `[]` after confirmed rule firings.
- `grep` shows `RuleExecutionTracker::new()` anywhere outside tests and the singleton initializer.
- Coverage dashboard shows live data but dead-rule / "last fired at" panels show nothing.
- A reviewer asks "which tracker is the source of truth?" and there is no single answer.

**Phase to address:**
**Phase 1 (wiring core)** — the singleton unification must land alongside the first real `execute_rules` call, otherwise Phase 3 (frontend) builds on data that looks broken.

---

### Pitfall 4: Hot-reload race conditions (reload while executing)

**What goes wrong:**
The existing `reload_engine` (`execution.rs:131`) takes `self.engine.write()` but **does nothing with it** — the body `let _engine = self.engine.write()...` acquires and immediately drops the lock (lines 163–166). Real rule content is read from disk but never pushed into the shared engine. So today there is no race because reload is a no-op on the engine. The moment wiring replaces `_engine` with actual recompilation, two races appear:

1. **Read-vs-write race on the shared `Engine`:** an in-flight executor that cloned the `Engine` (or read-locked it for the rule base) can see a half-swapped rule set if reload mutates fields in place. `Engine` is `Clone` but the clone is a shallow snapshot at clone time; if reload mutates the shared instance's `RuleSet` while a task is iterating it, the task sees torn state.
2. **Coverage/tracker corruption mid-fire:** reload mid-`execute_rules` can swap the action-node list out from under the firing loop, so `CoverageTracking::record_rule_fire` records a `rule_id` that no longer exists in the rule set, inflating coverage with ghost rules.

**Why it happens:**
The reload path is structured as "acquire write lock, then rebuild," but there is no versioning or epoch guard. The async reload (`async_reload: true`, `execution.rs:450`) spawns the reload on a tokio task with **no coordination** with in-flight executions — the 202 response fires the reload and forgets it.

**How to avoid:**
1. Make reload **swap-in-place**, not in-place mutation: build the new `Engine` fully on a detached copy, then under the write-lock do a single `*self.engine = new_engine`. In-flight executors that already cloned the old `Engine` keep running against the old snapshot safely; new executors get the new one.
2. Add an **epoch / generation counter** on `Engine`. Each `ExecuteResponse` includes `engine_generation` so callers can detect they executed against a stale rule set (the frontend can show "rules updated since this run").
3. For the async reload path, track in-flight execution count (an `AtomicUsize` incremented per `execute_single`, decremented on completion) and either (a) wait for it to drain before swapping (slower but safe), or (b) swap immediately and accept that in-flight runs use the old generation (faster; rely on generation tagging). Pick one and document it. Do not leave the choice implicit.
4. The current `_engine` no-op reload means **hot-reload is currently broken** — call this out in the phase spec so the team doesn't assume it works and only discover the race after wiring.

**Warning signs:**
- `reload_engine` body that mutates `self.engine` fields under the write-lock instead of swapping the whole `Engine`.
- A `202 Accepted` reload whose result is never observable in subsequent `/v1/execute` calls.
- Intermittent `rule_id not found` / `panic` on `action_nodes` iteration during load tests that trigger reloads.
- Coverage showing `rule_id`s that don't match any rule in the current ruleset.

**Phase to address:**
**Phase 1 (wiring core)** for the swap-don't-mutate + generation tag; **Phase 4 (async reload + execute-against-refreshed-rules)** for the in-flight drain/generation coordination. The reload race must be designed before the first real reload, not patched after.

---

### Pitfall 5: Stateful sessions never expire or get cleaned up

**What goes wrong:**
`execute_with_session` (`execution.rs:561`) is a stub returning `rules_fired: 0`. v5.0 adds real stateful sessions (days-to-weeks lifetime per `Engine::create_session(stateful=true, ...)` in `environment.rs`). With no session store, the obvious implementation is a `HashMap<String, Session>` inside `EngineState` — and sessions accumulate forever. Under actix (multiple workers sharing one `EngineState` via `web::Data`), the map grows unbounded; a long-lived client that opens a session per request (or a client that crashes without closing) leaks memory and engine state until OOM.

**Why it happens:**
There is no session registry today, so the team will build one under time pressure. `Session` holds a full `Engine` clone (with `working_memory` and `rule_set`), so each session is heavyweight. Without a TTL or LRU, the HashMap is a leak. The `session_id` is caller-supplied (`ExecuteRequest.session_id: Option<String>`), so a caller that reuses IDs collides with prior state, and a caller that generates fresh IDs per call leaks.

**How to avoid:**
1. Wrap sessions in an **`Arc<RwLock<HashMap<String, SessionEntry>>>`** with `SessionEntry { session: Session, last_used: Instant, created_at: Instant }`.
2. Enforce a **max session count** (config `MAX_SESSIONS`, default ~1024) with LRU eviction, plus an **idle TTL** (default ~15 min) and a **max lifetime** (default ~24 h). Run a background tokio task that sweeps expired sessions every 60s.
3. Reject caller-supplied `session_id` collisions with `409 Conflict` unless the caller presents a resume token; prefer server-generated IDs (`Uuid::new_v4()`) returned in the response, and treat client-supplied IDs as an idempotency key, not a primary key.
4. Add a `/v1/sessions` admin endpoint (RBAC-gated) to list/count sessions for ops debugging, and a Prometheus gauge `rrules_sessions_active` so the leak is visible on the dashboard.
5. Tests: a session that is not touched for `TTL` must be gone from the registry; a session touched within TTL must persist. A session count exceeding `MAX_SESSIONS` must evict the LRU entry.

**Warning signs:**
- `Session` stored in a plain `HashMap` with no `last_used` field.
- No background sweep task in `main.rs`.
- RSS climbing during load tests that open many sessions.
- Caller-supplied `session_id` used directly as the map key with no collision handling.
- No metric for active session count.

**Phase to address:**
**Phase 2 (stateful sessions)** — the registry, TTL, LRU, and sweep task must ship together with the session endpoint, not as a follow-up. The OOM failure mode is silent and only manifests in production.

---

### Pitfall 6: Coverage / fired-rule double-counting across batch and single execute

**What goes wrong:**
`CoverageTracking` is a **process-global singleton** (`coverage.rs:87`). Every `execute_single` call — whether from `/v1/execute` directly or as one item in `/v1/rules/batch-execute` — writes to the same global. So a batch of 100 requests that fires rule `r1` 100 times records `fire_count: 100` in coverage, indistinguishable from 100 separate API calls by 100 users. Worse, `ReteEngine::execute_rules` (`rete/mod.rs:321–345`) calls `CoverageTracking::record_rule_fire` **inside the per-nagram inner loop** — so one logical rule firing against multiple matching facts increments `fire_count` once per partial match, not once per rule. Combined with batch concurrency (8 parallel tasks), the global `Mutex` also becomes a contention point under high throughput.

Additionally, `RuleExecutionTracker.total_fires` (`detection.rs`) and the per-engine `metrics.record_rule_fired()` (`rete/mod.rs:333`) are **three separate counters** for the same event. The `/metrics` Prometheus output, the coverage JSON, and the dead-rules list can all disagree on how many times a rule fired.

**Why it happens:**
Three independent counters evolved at different layers (engine metrics, global coverage singleton, per-call tracker) with no shared definition of "what is one fire." Batch execution reuses `execute_single` verbatim (`execution.rs:393`), so it inherits all single-call side effects, multiplied by concurrency.

**How to avoid:**
1. Define **one canonical "fire" semantics**: a rule fires once per `execute_rules` call if at least one of its action nodes activates, regardless of how many facts matched. Implement this by recording coverage/tracker **once per rule per execution**, outside the nagram loop — collect a `HashSet<rule_id>` during the loop, record after.
2. Decide whether batch aggregates into coverage or not. Recommendation: **batch does NOT write to the global coverage singleton by default** (a batch is a test/bulk operation, not production traffic); add an explicit `record_coverage: bool` flag on `BatchExecuteRequest` (default `false`). Single `/v1/execute` always records.
3. Unify the three counters: have `RuleExecutionTracker` and the engine `metrics` read from `CoverageTracking` as the single source, or have all three record from one call site (a `record_firing(rule_id, rule_name, ts)` helper) so they can't drift.
4. Test: run `r1` once via `/v1/execute`, assert coverage `fire_count == 1`. Run a batch of 10 with `record_coverage=false`, assert coverage still `fire_count == 1`. Run with `record_coverage=true`, assert `fire_count == 11`. Pin this.
5. For the `Mutex` contention: switch `CoverageTracking`'s `INSTANCE` from `Mutex` to `parking_lot::Mutex` or shard by rule-id prefix if profiling shows contention under batch load. Don't optimize prematurely — measure first.

**Warning signs:**
- Coverage `fire_count` >> number of API calls.
- `record_rule_fire` called inside an inner loop over nagrams/facts (it is, in `rete/mod.rs:332`).
- Three different numbers for "rules fired" in `/metrics`, `/v1/metrics/coverage`, and `/v1/metrics/dead-rules`.
- Batch load tests showing `Mutex` wait time in flamegraphs.

**Phase to address:**
**Phase 1 (wiring core)** for the canonical-fire semantics and the once-per-rule recording; **Phase 2 (batch)** for the `record_coverage` flag and the batch-vs-single isolation. The nagram-loop recording bug is a Phase 1 fix because it corrupts the baseline data the frontend depends on.

---

### Pitfall 7: Per-request engine construction cost (rebuilding Rete network every call)

**What goes wrong:**
The stub does `let _engine = EngineBuilder::new().build()` per call (`execution.rs:316`). Wiring real execution naively does the same with rules loaded: build an `Engine`, parse/load the ruleset, compile the Rete (or TREAT/LEAPS/PHREAK) network, then fire. Compiling a Rete network is the expensive part — doing it per `/v1/execute` request means every call pays parse + compile time. For a ruleset of a few hundred rules this is milliseconds to tens of milliseconds of pure overhead per request, dwarfing the actual firing work. The batch endpoint would re-compile the same ruleset N times across its tasks.

**Why it happens:**
The engines are `&mut self` and not `Sync` (Pitfall 1), so the easy "safe" answer is a fresh engine per call. `Engine` derives `Clone` (`environment.rs:81`), which suggests cloning, but cloning an `Engine` clones the `RuleSet` — it's unclear whether the compiled Rete network is also cloned or rebuilt. If the algorithm engine (`ReteEngine`) is a separate struct from `Engine`, then "cloning the Engine" doesn't even give you a cloned `ReteEngine` — you still have to rebuild the network.

**How to avoid:**
1. **Cache the compiled rule base, not the mutable engine.** Keep one compiled `RuleSet` (and, where the algorithm supports it, a pre-built network template) in `EngineState` under a read-lock. Per request, clone the network template and instantiate working memory — not the rules.
2. For engines whose compiled state is cheap to clone (Rete's `beta_memory` is per-execution anyway), separate the **immutable compiled network** (`alpha`/`beta`/`action` node graph) from the **per-execution mutable memory** (`beta_memory` nagrams, `WorkingMemory`). Clone the graph once, populate memory per request.
3. Build a tiny **engine pool** per algorithm: pre-warm `N` instances per worker (where N = expected concurrency), check one out, run, reset (`ExecutionEngine::reset()` at `registry.rs`), return to pool. `reset()` already exists — use it instead of reconstructing.
4. Benchmark compile-vs-clone-vs-pool in the phase spike. If clone is <10% of compile cost, clone is fine; if not, use the pool.
5. The `EngineBuilder` defaults (`max_iterations: 1000`, etc.) are re-applied per call today — a pool also fixes the silent config-reset bug where per-call construction drops any runtime config.

**Warning signs:**
- `/v1/execute` p99 latency dominated by `EngineBuilder::new().build()` + rule loading, not by firing.
- `execute_single` containing `EngineBuilder::new()` after wiring.
- Batch throughput not scaling with `max_concurrency` past ~4 because each task recompiles.
- Flamegraph showing rule parsing/compilation in the hot path of every request.

**Phase to address:**
**Phase 1 (wiring core)** — the compile-cache / pool decision is architectural and must be made before the first real handler. A Phase 1 spike should measure compile cost on a representative ruleset; if it's >1ms, the pool must ship in Phase 1, not Phase 2.

---

### Pitfall 8: Frontend test-runner shows stale results (polling vs execution-completion race)

**What goes wrong:**
The frontend test-runner UI (v5.0 target) will call `/v1/execute` and then read `/v1/metrics/coverage` and `/v1/metrics/dead-rules` to display fired rules and coverage. Two staleness sources:
1. **`/v1/metrics/dead-rules` always returns `[]`** (Pitfall 3) — the UI looks permanently broken regardless of execution.
2. **Polling race:** if the UI polls coverage on an interval (the existing WebSocket at `/v1/metrics/ws` pushes `EngineState` frames, but coverage is a separate REST endpoint), a poll immediately after `/v1/execute` can return pre-write coverage state because the global `CoverageTracking` `Mutex` was released but the read happened before the write committed (or, under batch, before all tasks drained). The user sees "0 rules fired" for a frame, then the correct number pops in.
3. **Engine generation mismatch:** after a hot-reload (Pitfall 4), the UI's last-known rule list doesn't match the engine's rule set, so fired-rule names in the response don't resolve to rules shown in the IDE, and coverage shows orphan `rule_id`s.

**Why it happens:**
The frontend currently has no execution-triggered refresh contract — it polls. The WebSocket broadcasts `EngineState` (engine-level status), not per-execution results. There's no "execution completed" event the UI can subscribe to, so it falls back to polling and races the backend's write.

**How to avoid:**
1. Make `/v1/execute` **return the coverage delta in the response** — include `fired_rules: Vec<{rule_id, rule_name, fired: true}>` and `coverage_after` in `ExecuteResponse` (extend the struct at `execution.rs:36`). The UI shows results from the response itself, not from a follow-up poll. This eliminates the race entirely for the single-execute path.
2. For batch, include per-item fired-rule lists in `BatchItemResult` (already has `response: Option<ExecuteResponse>` — just populate it).
3. Add an `execution_id` (already present — `Uuid::new_v4()` at `execution.rs:313`) to coverage entries so the UI can filter "show me coverage from this run" vs cumulative coverage.
4. Drive UI refresh from the execute response, not from polling. Keep polling only for the long-term monitoring dashboard (cumulative coverage, dead rules over time).
5. For the dead-rules panel, fix Pitfall 3 first — no amount of frontend work makes a backend-empty endpoint show data.
6. After a hot-reload, the UI must re-fetch the rule list before rendering results; tag the response with `engine_generation` (Pitfall 4) so the UI can warn "rules changed since this run."

**Warning signs:**
- UI flickers "0 fired" then corrects on the next poll.
- Frontend making a `GET /v1/metrics/coverage` immediately after `POST /v1/execute`.
- Coverage panels and the execute-result panel showing different numbers for the same run.
- Dead-rules panel permanently empty despite confirmed firings.
- No `execution_id` correlation between the execute response and coverage entries.

**Phase to address:**
**Phase 1 (wiring core)** — extend `ExecuteResponse` with fired-rule list and coverage delta so the data is available; **Phase 3 (frontend test-runner)** — consume the response, don't poll. Do not build the frontend against the polling pattern first and "optimize later" — the race is visible to users from day one.

---

## Moderate Pitfalls

### Pitfall 9: `Engine` derives `Clone` but the algorithm engines may not — cloning the facade doesn't clone the network

**What goes wrong:**
`Engine` (the environment facade) derives `Clone` (`environment.rs:81`), but the pluggable algorithm engines (`ReteEngine`, etc.) are separate structs and may not be `Clone`. A handler that clones `Engine` expecting a ready-to-fire Rete instance still has to rebuild `ReteEngine` from `Engine.rule_set`. The clone saves the rule parse but not the network compile.

**Prevention:** Confirm by inspection which engines are `Clone`. If none, the per-request path is "clone `Engine` (cheap) → construct `ReteEngine::from_rule_set(&rule_set)` (the expensive part)." Pool the `ReteEngine` instances, not the `Engine` instances.

**Warning signs:** Handlers calling `engine.clone()` and assuming it's ready to `execute_rules()`. Tests that pass because the ruleset is tiny.

**Phase to address:** Phase 1 — verify during the compile-cost spike (Pitfall 7).

---

### Pitfall 10: `EngineState` (execution) vs `EngineState` (metrics) name collision

**What goes wrong:**
There are **two different `EngineState` structs** in the codebase: `api/src/v1/execution.rs:75` (the shared `Arc<RwLock<engine::environment::Engine>>`) and `api/src/metrics.rs:12` (a broadcast payload). They share a name and are both passed around via `web::Data` / channels. Wiring execution will have both in scope in some handlers/middleware (`api/src/middleware/auth.rs:51` references the metrics one). Mixing them up compiles (different types, so usually caught) but creates confusion in `use` statements and review.

**Prevention:** Rename one. The execution one is the more central concept; rename the metrics one to `MetricsSnapshot` or `EngineStateSnapshot`. Do this as a Phase 1 prep commit before adding logic.

**Warning signs:** `use` imports of both `EngineState` types in the same module; reviewer confusion in PRs.

**Phase to address:** Phase 0 (prep/rename) — cheap to fix early, painful once both are wired.

---

### Pitfall 11: `json_to_engine_value` loses type fidelity on round-trip

**What goes wrong:**
`json_to_engine_value` (`execution.rs:533`) maps JSON numbers to `Integer` if they fit in `i64`, else `Float`. The execute response rebuilds `facts_after` by cloning the original JSON (`facts_after.insert(fact_type.clone(), fact_value.clone())` at `execution.rs:327`), **not** the converted engine values. So the input and output facts are identical (the stub doesn't mutate them). Once real execution runs, the engine mutates `WorkingMemory`, but the response still echoes the input JSON unless the handler is changed to serialize the post-execution `WorkingMemory` back to JSON. Users will see "facts unchanged" even when rules fired and modified facts.

**Prevention:** After wiring, `facts_after` must be built from the engine's `WorkingMemory` post-execution, via a reverse `engine_value_to_json` mapping. Test: a rule that sets `score := score + 10` must show the incremented `score` in `facts_after`.

**Warning signs:** `facts_after` identical to request `facts` after a rule that demonstrably modifies facts. No `engine_value_to_json` function exists.

**Phase to address:** Phase 1 — belongs with the wiring core; without it the frontend cannot show meaningful results.

---

## Minor Pitfalls

### Pitfall 12: `reload_engine` is currently a no-op and will be assumed to work

The write-lock is acquired and dropped without recompiling (`execution.rs:163–166`). The async path (`async_reload: true`) spawns and forgets. v5.0's "execute against refreshed rules" feature depends on reload actually working. Treat fixing reload as a Phase 1 task, not a Phase 4 task — it's a prerequisite, not a feature.

### Pitfall 13: `start_file_watcher` is a stub (`log::info!` only)

`execution.rs:529` logs and returns. The "hot reload on rule changes" feature implies a file watcher exists; it doesn't. If v5.0 claims hot-reload works, this must be implemented or the claim dropped. Phase 4.

### Pitfall 14: `JobStore` is `Arc<RwLock<HashMap>>` but async reload never writes to it

`reload_rules` with `async_reload: true` spawns a task whose result is discarded (`let _result = ...`, `execution.rs:455`). `GET /v1/jobs/{job_id}` will always 404 for async reloads. Either make the spawned task write `ReloadJob` into the store, or remove the async option until it's real. Phase 4.

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Phase 1 — Execution wiring core | Shared `Engine` write-lock serializing execution (Pitfall 1); per-request compile cost (Pitfall 7); tracker singleton unification (Pitfall 3); canonical fire semantics (Pitfall 6); `facts_after` echo bug (Pitfall 11) | Decide engine-ownership contract first; spike compile cost; unify trackers before wiring; record once-per-rule; serialize post-execution working memory |
| Phase 1 — prep/rename | `EngineState` name collision (Pitfall 10) | Rename metrics `EngineState` → `MetricsSnapshot` before adding logic |
| Phase 2 — Per-request engine selection + stateful sessions | Selector misroute (Pitfall 2); session leaks (Pitfall 5); batch double-counting (Pitfall 6) | Capability-check before selection; TTL+LRU+sweep on session registry; `record_coverage` flag on batch |
| Phase 3 — Frontend test-runner | Stale polling race (Pitfall 8); dead-rules panel empty (Pitfall 3) | Consume `ExecuteResponse` fired-rule list; don't poll for per-run results; fix backend tracker first |
| Phase 4 — Async reload + execute-against-refreshed | Hot-reload race (Pitfall 4); no-op `reload_engine` (Pitfall 12); stub file watcher (Pitfall 13); async job not stored (Pitfall 14) | Swap-don't-mutate + generation tag; implement real reload; implement watcher or drop claim; persist `ReloadJob` |

## Cross-cutting recommendations

- **One source of truth for "rules fired."** Pick `CoverageTracking` as the canonical counter; have `RuleExecutionTracker` and engine `metrics` derive from or delegate to it. Three independent counters will drift.
- **Generation tag on every execution response.** Solves both the hot-reload race (Pitfall 4) and the frontend staleness (Pitfall 8) with one field.
- **Integration test that runs end-to-end: `/v1/execute` → `GET /v1/metrics/coverage` → `GET /v1/metrics/dead-rules`.** This single test catches Pitfalls 3, 6, and 8 simultaneously and should be the Phase 1 acceptance gate.
- **Do not trust `ExecutionEngine: Send + Sync` as evidence that sharing is safe.** `Sync` permits `&self` sharing; `execute(&mut self)` demands exclusion. Document this in the phase spec.

## Sources

- `api/src/v1/execution.rs` (stub at line 307–343; `EngineState` at 75; `reload_engine` no-op at 163–166; `execute_with_session` stub at 561; `start_file_watcher` stub at 529; `json_to_engine_value` at 533; batch semaphore at 379–395) — confidence HIGH
- `engine/src/engine/registry.rs` (`ExecutionEngine: Send + Sync` at 69; `fn execute(&mut self)` at 66) — confidence HIGH
- `engine/src/engine/selector.rs` (`select_engine` validates name only, no capability check) — confidence HIGH
- `engine/src/engine/rete/mod.rs` (`execute_rules(&mut self)` at 321; `CoverageTracking::record_rule_fire` inside nagram loop at 332) — confidence HIGH
- `engine/src/monitoring/coverage.rs` (`static INSTANCE: LazyLock<Mutex<TrackingState>>` global singleton at 87) — confidence HIGH
- `engine/src/monitoring/detection.rs` (`RuleExecutionTracker` plain struct at 64; `record_fire` at 92) — confidence HIGH
- `api/src/metrics.rs` (fresh `RuleExecutionTracker::new()` at 214 → dead-rules always empty; `CoverageTracking` singleton read at 233) — confidence HIGH
- `engine/src/environment.rs` (`Engine` derives `Clone` at 81; `EngineBuilder::build` at 59; `create_session` at ~88) — confidence HIGH
- `api/src/main.rs` (shared `EngineState` at 32, injected at 114; default actix worker count) — confidence HIGH