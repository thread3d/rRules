# Phase 25: Real reload + JobStore - Context

**Gathered:** 2026-07-03
**Status:** Ready for planning
**Mode:** `--auto` (all gray areas auto-selected; recommended option chosen for each; logged inline)

<domain>
## Phase Boundary

Make async reload-then-execute **actually swap the compiled rule set** so subsequent executes run refreshed rules, make `GET /v1/jobs/{id}` return **real** reload job status, make batch-execute aggregates **truthful**, and make `ExecuteResponse.engine_generation` **increment on reload**. After this phase: editing a rule + `reload_rules({async:true})` (202) → poll `GET /v1/jobs/{id}` through `Pending → Running → Completed` → the next `/v1/execute` reflects the edited rule; `engine_generation` bumps so callers can detect they ran against a prior rule set; `POST /v1/rules/batch-execute` aggregates `total_fired`/`succeeded`/`failed` honestly with one failing item not poisoning the others.

This is the **reload + JobStore + batch-honesty** layer of the v5.0 Live Execution vertical slice. It builds on the Phase 23 shared read-only `Arc<RwLock<RuleSet>>` scaffold (D-03) and the Phase 24 `ExecuteResponse.engine_generation` field (always `0` today).

**In scope (locked by ROADMAP.md §"Phase 25" + research/PITFALLS.md):**
- Real `reload_engine`: rebuild a new `RuleSet` fully off to the side, then under the shared ruleset's write-lock do a single swap (`*shared = new`) + bump generation — **swap-in-place, not in-place mutation** (Pitfall 12)
- Shared `Arc<RwLock<RuleSet>>` (already built once at startup from `RULE_STORAGE_PATH`, `main.rs:117`); `execute_single` loads it into each request's engine (unchanged from Phase 23) — **keep `Arc<RwLock<RuleSet>>`, no `ArcSwap` dep** (research flag: prefer `RwLock` unless contention shows up)
- Generation counter on the shared ruleset; `engine_generation` stamped into every `ExecuteResponse` and incremented on each successful reload (SC-3, Pitfall 4/8)
- Async reload branch writes `ReloadJob{Pending → Running → Completed/Failed}` to the existing `JobStore` so `GET /v1/jobs/{id}` works (no more 404) (Pitfall 14)
- Batch-execute drives real per-item execution with per-item isolation (already true post-Phase-23) AND `total_fired`/`succeeded`/`failed` aggregate **truthfully** — fix the current `execute_single`-Err-masks-as-success bug (SC-2)

**Out of scope (explicitly deferred):**
- File watcher (`start_file_watcher` stub, Pitfall 13) — not in Phase 25 ROADMAP deliverables; reload is triggered via `POST /v1/rules/reload`, not auto-on-file-change. Deferred.
- Single-file / partial reload (`ReloadRequest.rule_file`) — deferred (see D-05); full-directory reload only.
- `RuleExecutionTracker` singleton + dead-rule live data — Phase 26.
- Stateful sessions — Phase 27.
- Frontend test-runner / "Reload then Run" UI — Phase 28 (will consume `engine_generation` + `getJobStatus`).
- `inference_trace` population — Phase 28 (EXEC-04).

</domain>

<decisions>
## Implementation Decisions

### Carried forward as locked (not re-asked — from ROADMAP.md, PITFALLS.md, Phase 23/24 CONTEXT)
- **L-01:** Swap mechanism = keep the existing `Arc<RwLock<RuleSet>>` built once at startup (`api/src/main.rs:117`, Phase 23 D-03). Reload swaps-in-place under the write lock. **No `ArcSwap` dependency** — the ROADMAP research flag explicitly prefers `Arc<RwLock<RuleSet>>` unless read contention shows up, and the existing code already uses it.
- **L-02:** `reload_engine` is a no-op today (`api/src/v1/execution.rs:183-186` — acquires-and-drops the engine write lock, reads files but never recompiles/swaps). It MUST become a real build-then-swap (Pitfall 12).
- **L-03:** The async reload branch MUST persist `ReloadJob{Pending → Running → Completed/Failed}` to the existing `JobStore` so `GET /v1/jobs/{id}` returns real transitions (Pitfall 14). The `JobStore` type, `init_job_store()`, `get_job_status`, `list_jobs`, the `ReloadJob` struct, and the `ReloadStatus` enum (`Pending`/`Running`/`Completed`/`Failed`) **already exist** (`execution.rs:212-264`, `:717-758`) and are wired in `main.rs:272-273` — Phase 25 wires the spawned reload task to write to them.
- **L-04:** `ExecuteResponse.engine_generation: u64` already exists (Phase 24, `execution.rs:64`, `#[serde(default)]`, hardcoded `0u64` at `:532` and `:626`). Phase 25 makes it real: stamped from the shared ruleset's generation on every execute, incremented on every successful reload (SC-3, Pitfall 4/8).
- **L-05:** Per-request isolation is preserved (Phase 23 L-03/D-03): `execute_single` takes the shared ruleset's read lock, clones the rules into the per-request engine via `load_ruleset`, and releases before `execute()`. Reload's write lock only blocks the brief read-then-clone window, never the CPU-bound execute. In-flight runs finish against their already-cloned snapshot.
- **L-06:** Batch per-item isolation already holds post-Phase-23: each `JoinSet` task calls `execute_single` → builds its own `Box<dyn ExecutionEngine>` (`execution.rs:606-629`). A panicking item becomes a `JoinError` → `failed += 1` (already correct, `:648-655`). SC-2's "one panicking item does not poison the others" is satisfied; the remaining gap is the `Err`-masks-as-success miscount (D-03).

### Discussed gray areas (auto-selected recommended option for each)

#### Reload in-flight handling (Pitfall 4 — "pick drain-vs-generation-accept and document")
- **D-01: Generation-accept (swap immediately, tag with generation) — NOT drain.** `execute_single` already clones the ruleset into the per-request engine under a short read lock and releases before executing (L-05), so in-flight runs naturally finish against their cloned snapshot regardless of a concurrent swap. Reload swaps under the write lock between requests. No `AtomicUsize` in-flight counter, no drain/block on the hot execute path. The generation tag (D-02) lets callers detect they ran against a prior rule set (SC-3, frontend "rules changed since this run" in Phase 28). This is Pitfall 4 step 3 option (b), explicitly chosen and documented.
  - `[auto]` Q: "Drain in-flight executes before swapping, or swap immediately and tag responses with generation?" → Selected: **"Generation-accept (swap immediately, tag with generation)"** (recommended — matches the existing per-request-clone design; no hot-path blocking; reload is rare, execute is hot)

#### Generation counter placement (source of truth)
- **D-02: Generation lives inside the shared ruleset payload, read atomically under the existing read lock.** Concretely: wrap the shared state as `SharedRuleSet { ruleset: RuleSet, generation: u64 }` (or `RwLock<(RuleSet, u64)>`, or add `generation` to `RuleSet` — exact shape is Claude's discretion) inside the existing `Arc<RwLock<…>>`. Reload increments `generation` under the write lock during the swap. `execute_single` reads `(ruleset, generation)` together under the single read lock, clones the ruleset into the engine, and stamps `generation` into `ExecuteResponse.engine_generation`. This is **coherent** — a separate `AtomicU64` would race (the counter could bump between reading the ruleset and reading the counter) — and adds **no new lock on the hot read path**. The per-engine `generation: usize` fields (`engine/src/engine/rete/mod.rs:23`, `phreak/mod.rs:121`, etc.) are per-instance, reset each request (always `0`), and are **not** the source of truth for reload tracking.
  - `[auto]` Q: "Where does the generation source-of-truth live — separate `AtomicU64`, inside the shared `RuleSet`, or the per-engine `generation` field?" → Selected: **"Inside the shared ruleset payload, read atomically under the existing read lock"** (recommended — coherent with the snapshot; no extra lock/dep; matches L-01)

#### Batch-execute truthful aggregation (SC-2 gap)
- **D-03: Propagate `execute_single` `Err` as a per-item FAILURE, not a zero-fired success.** Today `batch_execute_rules` converts `execute_single` `Err` into a placeholder `ExecuteResponse { rules_fired: 0, … }` (`execution.rs:618-628`) and the collector (`:639-641`) counts it as `succeeded += 1` with `success: true` — inflating `succeeded` and masking per-item failures (e.g., unknown `engine` → HTTP 400 is counted as a successful zero-fire). Fix: the `JoinSet` task returns `Result<ExecuteResponse, String>`; `Ok(resp)` → `succeeded += 1`, `success: true`, `total_fired += resp.rules_fired`, `response: Some(resp)`; `Err(msg)` → `failed += 1`, `success: false`, `error: Some(msg)`, `response: None`. Panic → `JoinError` → `failed` (already correct). Whole-batch 400 is **not** used — the handler doc explicitly states "one failing request does not fail the batch" (`execution.rs:563-565`). This makes `total_fired`/`succeeded`/`failed` aggregate truthfully (SC-2).
  - `[auto]` Q: "How should batch-execute count an item whose `execute_single` returned `Err` (e.g. unknown engine)?" → Selected: **"Per-item failure: `success:false` + `failed += 1` (don't mask as a zero-fired success)"** (recommended — SC-2 requires truthful aggregates; preserves the no-whole-batch-fail contract)

#### Sync reload path
- **D-04: BOTH sync and async reload swap the shared ruleset + bump generation via ONE shared swap helper.** Sync (`async_reload: false`) runs the swap inline and returns `ReloadResponse`; async runs the same swap in the spawned task with `JobStore` transitions. A sync reload that didn't swap would be a lie (on-disk rules would differ from the executing ruleset). SC-1 only tests async, but honesty requires both paths to swap. The shared helper (`reload_and_swap(&shared, &rules_dir) -> Result<ReloadResult, String>`) keeps them consistent and avoids two divergent reload implementations.
  - `[auto]` Q: "Should sync reload also swap the shared ruleset + bump generation, or is Phase 25 async-only?" → Selected: **"Both swap (shared helper); sync returns `ReloadResponse`, async uses `JobStore`"** (recommended — consistency/honesty; one swap implementation)

#### Reload scope (directory vs single file)
- **D-05: Full-directory reload reusing `build_ruleset_from_dir(&RULE_STORAGE_PATH)` (the startup helper, `api/src/main.rs:44`).** The swapped `RuleSet` then matches what a restart would load — consistency between startup and reload. `ReloadRequest.rule_file: Option<String>` (`execution.rs:270`) is currently ignored by the handler; partial single-file reload adds recompilation/merge complexity and is **deferred** (see Deferred). The `rule_file` field stays in the struct (no API break) but is not honored in Phase 25.
  - `[auto]` Q: "Reload the whole `RULE_STORAGE_PATH` directory (matching startup) or honor `ReloadRequest.rule_file` for partial reload?" → Selected: **"Full-directory reload reusing `build_ruleset_from_dir`; `rule_file` deferred"** (recommended — consistency with startup, less complexity, matches the shared-swap design)

### Claude's Discretion (technical impl details left to the planner/researcher)
- **Reload function placement** — whether to extend the existing `EngineState::reload_engine` (`execution.rs:151`) or write a new `reload_and_swap` operating on `web::Data<Arc<RwLock<SharedRuleSet>>>`. The reload MUST operate on the **shared** ruleset (the `web::Data` injected at `main.rs:117`), not the legacy `EngineState.engine` (which the execute path no longer uses — Phase 23 moved execute to `EngineRegistry` + the shared ruleset). The planner may refactor `reload_rules` to take `web::Data<SharedRuleSet>` + `web::Data<JobStore>` and deprecate/remove the `EngineState.reload_engine` no-op. Architectural choice; not user-facing.
- **`SharedRuleSet` exact shape** — new struct vs. `RwLock<(RuleSet, u64)>` tuple vs. adding a `generation` field to `RuleSet` itself. The contract is: "ruleset + generation readable atomically under one read lock; generation bumpable under the write lock." Planner picks the cleanest.
- **`JobResponse.status` initial value** — the spawned task inserts `Pending` then quickly moves to `Running`; the 202 response returns `JobResponse { status: "pending" }` (or `"running"`). SC-1 polls `Pending → Running → Completed`, so insert `Pending` first. Minor.
- **Test placement** — extend `api/tests/execute_integration.rs` (reload-then-execute-against-refreshed, `engine_generation` increment, batch truthful aggregation incl. one-failing-item) and the jobs/metrics tests; mirror the Phase 23/24 Wave 0 RED→GREEN + Nyquist pattern. The keystone scenario for SC-1: edit a rule (e.g., `score := score + 10` → `+ 20`), `reload_rules({async:true})` → poll `GET /v1/jobs/{id}` to `Completed` → `POST /v1/execute` returns `facts_after` with the **new** delta and a bumped `engine_generation`.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap & requirements (scope + success criteria)
- `.planning/ROADMAP.md` §"Phase 25: Real reload + JobStore" — goal, depends-on (Phase 23), requirements (DEPLOY-05, EXEC-03), 5 key deliverables, 3 pitfalls to avoid (4/12/14), research flags (`rete/mod.rs` network construction; `ArcSwap` vs `Arc<RwLock<RuleSet>>` — prefer `RwLock`), and the 3 success criteria verification must satisfy.
- `.planning/REQUIREMENTS.md` — traceability: `EXEC-03` (line 9) and `DEPLOY-05` (line 14) → Phase 25 (both Pending).

### Milestone research (pitfalls — HIGH confidence, file:line evidence)
- `.planning/research/PITFALLS.md` §"Pitfall 4" (line 92) — hot-reload race: swap-don't-mutate + generation tag + the explicit "pick drain-vs-generation-accept and document" choice (D-01 resolves this as generation-accept).
- `.planning/research/PITFALLS.md` §"Pitfall 12" (line 275) — `reload_engine` no-op (acquire-and-drop write lock, `execution.rs:183-186`); must become real recompilation.
- `.planning/research/PITFALLS.md` §"Pitfall 14" (line 283) — `JobStore` is `Arc<RwLock<HashMap>>` but async reload never writes to it (`let _result = ...`); `GET /v1/jobs/{id}` always 404. Phase 25 makes the spawned task persist `ReloadJob`.
- `.planning/research/PITFALLS.md` §"Pitfall 13" (line 279) — `start_file_watcher` stub; **out of scope for Phase 25** (deferred — reload is request-triggered, not file-watcher-triggered).
- `.planning/research/PITFALLS.md` "Cross-cutting recommendations" (line 299) — "generation tag on every execution response" solves Pitfall 4 + 8 with one field (D-02/L-04).

### Prior phase context (the scaffold Phase 25 swaps)
- `.planning/phases/23-execute-evaluator-wiring/23-CONTEXT.md` — **D-03** (shared read-only `Arc<RwLock<RuleSet>>` built once at startup from `RULE_STORAGE_PATH`; the snapshot Phase 25 swaps), **D-03a** (empty/missing rules dir → warn + continue with empty `RuleSet`), **L-03** (per-request engine, no pool — the isolation reload must preserve, L-05), and the explicit deferred item "RuleSet reload-then-swap — Phase 25" (line 139).
- `.planning/phases/24-execution-counters-metrics-broadcast/24-RESEARCH.md` + `24-02-PLAN.md` + `24-VERIFICATION.md` — origin of `ExecuteResponse.engine_generation: u64` (Pitfall 8 / Open Q6), documented as "always `0` in Phase 24 — Phase 25 wires the real per-engine generation counter" (`execution.rs:56-63`). **No `24-CONTEXT.md` exists** (Phase 24 ran without one); read the 24-02 PLAN + VERIFICATION for the field contract.

### No external specs
No ADRs or external standard specs apply — brownfield internal-wiring phase against the project's own Rust codebase.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`build_ruleset_from_dir(&rules_dir) -> RuleSet`** (`api/src/main.rs:44`, used at `:119`) — the startup RuleSet builder. D-05: reload reuses this exact helper so the swapped `RuleSet` matches a restart.
- **Shared `web::Data<Arc<std::sync::RwLock<RuleSet>>>`** (`api/src/main.rs:117`) — the swap target. Phase 25 wraps it with a generation (D-02) and reload swaps under its write lock.
- **`JobStore` + `ReloadJob` + `ReloadStatus` + handlers — already fully built** (`api/src/v1/execution.rs`): `ReloadJob` struct (`:212`), custom `Serialize` (`:219`), `ReloadStatus{Pending,Running,Completed,Failed}` (`:240`), `JobStore = Arc<RwLock<HashMap<String, ReloadJob>>>` (`:259`), `init_job_store()` (`:262`), `get_job_status` (`:730`, `GET /v1/jobs/{job_id}`), `list_jobs` (`:755`); wired in `main.rs:272-273`. **Phase 25 only needs to make the spawned reload task write transitions into this existing store** — the read side already works.
- **`execute_single`** (`api/src/v1/execution.rs:312`) — the real per-request executor (Phase 23 rewrite). Takes `web::Data<Arc<RwLock<RuleSet>>>`; reads the snapshot under the read lock, `load_ruleset`s into a fresh `Box<dyn ExecutionEngine>`, runs in `web::block`, serializes `facts_after` from `get_facts()`. Phase 25 adds: read `generation` alongside the ruleset (D-02) and stamp it into `ExecuteResponse.engine_generation` (replace the `0u64` at `:532`).
- **`batch_execute_rules`** (`api/src/v1/execution.rs:579`) — `JoinSet` + `Semaphore` per-item execution (already real). D-03: change the task to return `Result<ExecuteResponse, String>` and branch the collector so `Err` → `failed`/`success:false`, not `succeeded`/`success:true`.
- **`ExecuteResponse.engine_generation: u64`** (`execution.rs:64`, `#[serde(default)]`) — the field exists; populate it (D-02/L-04).

### Established Patterns
- **`web::Data<T>` shared state + `spawn_blocking`/`JoinSet` for CPU-bound engine work** — the established execute-path pattern (Phase 23/24); reload's swap is cheap (build off-to-side, one write-lock assignment) so it can run on the async runtime without `spawn_blocking`, but the recompilation read is I/O (`fs::read_dir`) — mirror `reload_engine`'s existing `fs::read_dir` usage.
- **Factory shared, engine instance per request** (Phase 23 L-03) — reload must NOT take `EngineState.engine.write()` in any path that serializes with execute; it swaps the shared **`RuleSet`**, which execute reads under a separate lock with clone-then-release (L-05).
- **Wave 0 RED → GREEN + Nyquist closure** (Phase 23/24) — write the SC-1/SC-2/SC-3 integration tests RED first, then implement to GREEN.

### Integration Points
- **`api/src/v1/execution.rs:679` (`reload_rules` handler)** — the rewrite target: add `web::Data<SharedRuleSet>` + `web::Data<JobStore>` params; on `async_reload`, insert `ReloadJob{Pending}` → return `JobResponse{job_id, status:"pending"}` (202) → spawn task that transitions `Running` → calls `reload_and_swap` → `Completed`/`Failed`. Sync branch calls `reload_and_swap` inline → `ReloadResponse`.
- **`api/src/v1/execution.rs:151` (`EngineState::reload_engine`)** — the no-op to replace/deprecate (Pitfall 12); the real swap operates on the shared ruleset, not `EngineState.engine`.
- **`api/src/main.rs:117`** — change the shared state construction from `Arc<RwLock<RuleSet>>` to `Arc<RwLock<SharedRuleSet{ruleset, generation:0}>>` (or equivalent per D-02); thread it into `execute_rules`, `batch_execute_rules`, and `reload_rules` `web::Data` params.
- **`api/src/v1/execution.rs:532` + `:626`** — replace `engine_generation: 0u64` with the generation read from the shared snapshot.
- **`api/tests/execute_integration.rs`** — add the SC-1 (reload-then-execute-against-refreshed + job transitions), SC-2 (batch truthful aggregation incl. one failing item), SC-3 (generation increment) integration tests.

</code_context>

<specifics>
## Specific Ideas

- **Keystone scenario for SC-1** (the honesty anchor): author/edit a rule so its action changes (e.g., `score := score + 10` → `score := score + 20`), call `reload_rules({async:true})` → 202 with `job_id` → poll `GET /v1/jobs/{job_id}` and observe `pending → running → completed` → call `POST /v1/execute` with a fact that triggers it → assert `facts_after.score` reflects the **+20** delta (the edited rule) AND `engine_generation` is **greater** than a pre-reload execute's generation. This single scenario exercises real recompilation (L-02), swap-in-place (L-01), JobStore transitions (L-03), generation bump (D-02/L-04), and generation-accept in-flight semantics (D-01).
- **SC-2 honesty check**: a batch of N items where one item sends `engine: "unknown"` (→ `execute_single` `Err` / 400) must return `succeeded == N-1`, `failed == 1`, that item's `BatchItemResult{success:false, error:Some(...)}`, and `total_fired` reflecting only the successful items — NOT `succeeded == N` with a masked zero-fire (the current bug).
- The user's consistent posture across v5.0 (Phase 23 CONTEXT "Specifics") is to take the **honest-for-real-authored-rules** option. The planner should not narrow D-04 (sync-also-swaps) or D-03 (truthful batch aggregates) to "minimal that passes the async-only SC-1" — both reload paths must honestly swap, and batch aggregates must be truthful, because the user explicitly prefers the honest path.

</specifics>

<deferred>
## Deferred Ideas

- **File watcher (`start_file_watcher`, Pitfall 13)** — auto-reload on rule-file change. Not in Phase 25 ROADMAP deliverables; reload is request-triggered (`POST /v1/rules/reload`). Future enhancement.
- **Single-file / partial reload (`ReloadRequest.rule_file`)** — `rule_file` is in the request struct but ignored; partial reload adds recompilation/merge complexity. Deferred (D-05). The field stays (no API break) but is not honored in Phase 25.
- **`RuleExecutionTracker` singleton + dead-rule live data** — Phase 26.
- **Stateful sessions** — Phase 27.
- **Frontend "Reload then Run" flow + `getJobStatus` polling UI** — Phase 28 (consumes `engine_generation` + the `JobStore` status this phase makes real).
- **`inference_trace` population** — Phase 28 (EXEC-04).

None of these were scope-creep suggestions from the user — they are the phase's own downstream milestones / explicit out-of-scope items, listed so the planner knows the boundaries.

</deferred>

---

*Phase: 25-real-reload-jobstore*
*Context gathered: 2026-07-03 (--auto mode)*