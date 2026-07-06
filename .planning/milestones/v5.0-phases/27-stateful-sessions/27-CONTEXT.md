# Phase 27: Stateful sessions - Context

**Gathered:** 2026-07-04
**Status:** Ready for planning
**Mode:** `--auto` (all gray areas auto-selected; recommended option chosen for each; logged inline)

<domain>
## Phase Boundary

Make `/v1/execute` carry **multi-interaction stateful execution** through a real `SessionManager`: a caller supplies a `session_id`, inserts facts, fires, then calls again with the same `session_id` and sees **accumulated working memory** from the prior call — insert more → fire again → dispose — with **bounded resource use** (TTL + LRU + max-count + background sweep + a Prometheus gauge so a leak is visible on the dashboard). The stateless one-shot path (Phase 23) stays unchanged when `session_id` is absent.

After this phase: `POST /v1/execute` with a `session_id` accumulates working memory across calls (SC-1); a session untouched for >TTL is gone while a touched one persists (SC-2); when the session count exceeds `MAX_SESSIONS` the LRU entry is evicted and `rrules_sessions_active` reflects the live count (SC-3); `DELETE /v1/execute/session/{id}` disposes a session and `POST /v1/execute/session/{id}/reset` clears working memory without disposing (SC-4).

This is the **stateful-sessions** layer of the v5.0 Live Execution vertical slice. It builds on Phase 23's real per-request evaluator (the `Box<dyn ExecutionEngine>` surface that a stateful session wraps), Phase 24's `MetricsSnapshot.active_sessions` field (left at `0` for this phase — `execution.rs:655-657`), and Phase 25's `SharedRuleSet{ruleset, generation}` (the snapshot a session captures at creation). The keystone is bounded honesty: stateful sessions work across calls AND cannot leak to OOM.

**In scope (locked by ROADMAP.md §"Phase 27" + research/PITFALLS.md):**
- NEW `api/src/v1/sessions.rs` with `Session` + `SessionManager` (`Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>>`, TTL, LRU, max session count) — DashMap 6.2.1 pinned (NOT 7.0.0-rc2)
- `execute_single` upserts/looks-up by `session_id`; stateless one-shot path unchanged when `session_id` is absent
- NEW routes `DELETE /v1/execute/session/{id}`, `POST /v1/execute/session/{id}/reset`
- Background `tokio::time::interval` sweep task (every ~60s) evicts expired sessions
- `rrules_sessions_active` Prometheus gauge + populate the existing `MetricsSnapshot.active_sessions` field (`execution.rs:657`) — so a leak is visible on the dashboard
- Server-generated session IDs (UUID v4) returned in the response; client-supplied IDs treated as idempotency keys, not primary keys
- Ship registry + TTL + LRU + sweep + max-count + gauge TOGETHER (the OOM failure mode is silent — Pitfall 5)

**Out of scope (explicitly deferred):**
- Frontend test-runner / session UI — Phase 28 (the frontend consumes `session_id` from the response; no frontend code in Phase 27)
- `inference_trace` population — Phase 28 (EXEC-04)
- RBAC-gated `/v1/sessions` admin list/count endpoint — Pitfall 5 step 4 mentions it; NOT in ROADMAP Phase 27 deliverables. Deferred to a future governance/ops phase. Phase 27 ships only the `rrules_sessions_active` gauge + the lifecycle routes (DELETE/reset).
- Rewiring/removing the v1.0 `engine::environment::Session` stub (`environment.rs:152`) — it is unused by the live execute path; document (D-01) but do not delete (other v1.0-era code references it).
- Cross-session shared working memory / "undeclared state" — explicitly an anti-feature (FEATURES.md §"Anti-Features"); stateless stays strictly per-request.

</domain>

<decisions>
## Implementation Decisions

### Carried forward as locked (not re-asked — from ROADMAP.md, PITFALLS.md, Phase 23/24/25/26 CONTEXT)
- **L-01:** The real execution surface is `EngineRegistry` + per-request `Box<dyn ExecutionEngine>` (Phase 23 L-03). A stateful session wraps **this** surface — a per-session `Box<dyn ExecutionEngine>` held across calls — NOT the v1.0 `engine::environment::Session` stub (see D-01 research correction). The `ExecutionEngine` trait (`engine::engine/registry.rs:64`) is the contract: `load_ruleset(&mut self, &RuleSet)`, `insert_fact(&mut self, Fact)`, `execute(&mut self) -> EngineResult`, `get_facts(&self) -> Vec<Fact>`.
- **L-02:** Per-request isolation (Phase 23 L-03 / Phase 25 L-05): the shared `Arc<RwLock<SharedRuleSet{ruleset, generation}>>` is read-locked only to clone the ruleset into an engine, then released before `execute()`. The sessionful path reads the shared ruleset once at **session creation**, `load_ruleset`s it into the session's engine, and releases; subsequent calls reuse the session's engine + accumulated working memory (no shared lock on the hot session-resume path).
- **L-03:** `SharedRuleSet{ruleset, generation}` (Phase 25 D-02) is the shared snapshot. A stateful session captures `engine_generation` at creation and stamps it into every sessionful `ExecuteResponse` so callers can detect "this session is running a stale rule set" after a reload (see D-07).
- **L-04:** `web::Data<T>` shared state across actix workers; CPU-bound engine work runs in `web::block` / `spawn_blocking` (Phase 23/24 pattern). The sessionful `execute` call locks the per-session Mutex, then runs the engine inside `web::block` (the `&mut self` engine is `!Sync` — D-11).
- **L-05:** `MetricsSnapshot.active_sessions: usize` already exists at `api/src/v1/execution.rs:657`, hardcoded `0` in Phase 24 with the inline comment "Phase 27 populates active_sessions." Phase 27 populates it from `SessionManager::len()` in the snapshot built after each execute (D-08).
- **L-06:** `serial_test` (workspace dev-dep, established in Phase 23/26) is the tool for `SessionManager` registry-isolation tests; `DashMap` is `Send + Sync` so it shares cleanly across actix workers via `web::Data`. `chrono` (workspace dep) stamps `last_used`/`created_at`.
- **L-07:** Wave 0 RED → GREEN + Nyquist closure (Phase 23/24/25/26) — write the SC-1/SC-2/SC-3/SC-4 integration tests RED first, then implement to GREEN. Tests extend `api/tests/execute_integration.rs` and mirror the established pattern.

### Discussed gray areas (auto-selected recommended option for each)

#### Session payload — keystone research correction
- **D-01: `StatefulSession` wraps a per-session `Box<dyn ExecutionEngine>` (the real Phase-23 evaluator), NOT the v1.0 `engine::environment::Session`.** The existing `engine::environment::Session` (`environment.rs:152-208`) is a **non-functional v1.0-era stub**: `Session::execute()` returns `Ok(ExecutionResult::new(true, "Not implemented"))` (`environment.rs:191`), `insert_fact` is a no-op (`environment.rs:176-183`), and it holds a raw `*const Engine` pointer (`environment.rs:154` — not `Send`, unsafe across await points). The real execution surface (Phase 23) bypasses `environment::Engine`/`Session` entirely and uses the pluggable `EngineRegistry` → `Box<dyn ExecutionEngine>` per request. A stateful session therefore holds: one `Box<dyn ExecutionEngine>` (engine fixed at creation — D-06), accumulated working memory (the engine's own `working_facts`), `engine_name`, `engine_generation` (captured at creation — L-03), `last_used: Instant`, `created_at: Instant`, and the server-generated `Uuid` + optional client idempotency alias (D-04). This mirrors Phase 23's "fresh engine per request" but lifts the engine to session lifetime. The v1.0 `environment::Session` is documented and left in place (other v1.0 code references it) but is NOT used by the live path.
  - `[auto]` Q: "Should a stateful session wrap the existing `engine::environment::Session` or a per-session `Box<dyn ExecutionEngine>`?" → Selected: **"Per-session `Box<dyn ExecutionEngine>` — the real Phase-23 surface; the v1.0 `Session` is a non-functional stub"** (recommended — L-01; the v1.0 `Session` does not execute and is not `Send`; Phase 23 already established `EngineRegistry` as the real surface)

#### Session registry data structure
- **D-02: `Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>>` (DashMap 6.2.1, pinned per ROADMAP — NOT 7.0.0-rc2).** `DashMap` is `Send + Sync` and sharded, so lookups/upserts across different `session_id`s do not contend on a single lock — critical because the sessionful execute path takes a per-entry `Mutex` (D-11) and must NOT serialize unrelated sessions. `Arc<DashMap>` is shared across actix workers via `web::Data` (mirrors `EngineRegistry`/`ExecutionCounters` wiring). Chosen over `Arc<RwLock<HashMap>>` (a single global write lock on insert/evict would serialize session creation across workers — the exact Pitfall-1-style regression Phase 23 avoided for engines). The per-entry `Arc<Mutex<StatefulSessionEntry>>` (D-11) handles the `&mut self` engine contract.
  - `[auto]` Q: "`DashMap` (sharded, ROADMAP-pinned 6.2.1) or `Arc<RwLock<HashMap>>` for the session registry?" → Selected: **"`Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>>` — sharded, no global write lock"** (recommended — ROADMAP locks DashMap; avoids serializing unrelated sessions; matches the established no-shared-write-lock-on-hot-path discipline)

#### TTL / LRU / max-count / sweep policy
- **D-03: Ship TTL + LRU + max-count + background sweep + Prometheus gauge TOGETHER (Pitfall 5 — OOM is silent).** Defaults (config knobs via env vars, matching the `RRULES_DEFAULT_ENGINE` pattern): idle TTL ~15 min (`RRULES_SESSION_IDLE_TTL_SECS`, default `900`), max lifetime ~24 h (`RRULES_SESSION_MAX_LIFETIME_SECS`, default `86400`), max sessions ~1024 (`RRULES_MAX_SESSIONS`, default `1024`), sweep interval 60 s (`RRULES_SESSION_SWEEP_INTERVAL_SECS`, default `60`). A session is evicted if (a) `now - last_used > idle_ttl`, (b) `now - created_at > max_lifetime`, or (c) the registry size exceeds `MAX_SESSIONS` and this entry is the LRU (oldest `last_used`). The background `tokio::time::interval` sweep task (D-10) runs the eviction pass every sweep interval and updates the gauge. All four controls ship in one phase because a partial fix (e.g. TTL without max-count) still leaks under caller-collision or burst creation.
  - `[auto]` Q: "Which of TTL / LRU / max-count / sweep / gauge ship in Phase 27?" → Selected: **"All of them, together — partial fixes still leak"** (recommended — ROADMAP deliverable + Pitfall 5 both mandate shipping together; the OOM failure mode is silent)

#### Session ID semantics (Pitfall 5 step 3)
- **D-04: Server-generated UUID v4 is the primary key; client-supplied `session_id` is an idempotency alias.** When `req.session_id` is `None` → stateless one-shot (Phase 23 path, unchanged). When present → (a) if an existing session has that client-supplied id as its alias → **resume** it (lock, insert facts, execute, return the session's server UUID + `engine_generation`); (b) if no session has that alias → create a new session with a fresh `Uuid::new_v4()` primary key, record the client alias → server-UUID mapping, and return the server UUID. The response carries a new `ExecuteResponse.session_id: Option<String>` (the server UUID) so the caller learns the canonical id on first creation. This prevents caller-collision leaks (a caller that generates fresh IDs per call still leaks, but TTL+LRU+max-count bound it; a caller that reuses an id resumes safely instead of colliding). `session_id` already exists on `ExecuteRequest` (`execution.rs:23`) — Phase 27 gives it resume semantics and adds the server-UUID response field.
  - `[auto]` Q: "Server-generated UUID primary key with client `session_id` as idempotency alias, or client `session_id` as the primary key?" → Selected: **"Server UUID primary key + client `session_id` as idempotency alias (resume-on-match)"** (recommended — ROADMAP deliverable: "Server-generated session IDs (UUID v4) returned in the response; client-supplied IDs treated as idempotency keys, not primary keys"; Pitfall 5 step 3)

#### Stateful execute path integration
- **D-05: `execute_single` branches on `req.session_id`; the sessionful path goes through `execute_single`, NOT a separate route.** When `session_id` is absent → the existing stateless one-shot path (Phase 23, `execution.rs:504-575`) runs unchanged. When present → `execute_single` looks up / creates the session via `web::Data<Arc<SessionManager>>` (new param), locks the per-session `Mutex`, `insert_fact`s the request facts into the session's engine, runs `execute()` inside `web::block`, updates `last_used`, builds `ExecuteResponse` with the session's `engine_generation` (captured at creation, D-07) + the server `session_id`, and updates the gauge. NEW routes (registered in the `/v1` scope in `main.rs`): `DELETE /v1/execute/session/{id}` (dispose — removes from registry) and `POST /v1/execute/session/{id}/reset` (clear the session's working memory — `engine.reset()` or rebuild — WITHOUT disposing; subsequent calls resume with empty WM). The existing `execute_with_session` stub at `execution.rs:1081-1094` (`POST /v1/execute/session/{id}`) is **removed/repurposed** — it returns `rules_fired: 0` and is not wired to the registry; the ROADMAP deliverable says `execute_single` owns the sessionful path, so the dedicated `execute_with_session` route is dead code after Phase 27 (planner may keep the route as a thin alias to `execute_single` or delete it — discretion).
  - `[auto]` Q: "Does the sessionful path go through `execute_single` (with `session_id` in the body) or the dedicated `execute_with_session` route?" → Selected: **"`execute_single` branches on `session_id`; the `execute_with_session` stub is removed/repurposed"** (recommended — ROADMAP deliverable: "execute_single upserts/looks-up by session_id; stateless one-shot path unchanged when session_id is absent")

#### Engine selection per session
- **D-06: Engine is fixed at session creation; a resume call with a conflicting `engine` param returns HTTP 400.** The first call with a given `session_id` (and optional `engine`) creates the session's `Box<dyn ExecutionEngine>` via `registry.create_engine_by_name(&engine_name)` (the same factory path as Phase 23, `execution.rs:535-549`) and `load_ruleset`s the shared ruleset into it. Subsequent calls on the same session reuse that engine. If a resume call supplies an `engine` that differs from the session's recorded `engine_name` → HTTP 400 `("session engine is fixed: expected {X}, got {Y}")` — changing engines mid-session would discard accumulated working memory and is not supported. A resume call may omit `engine` (the session's engine is used regardless). The default-engine resolution (`RRULES_DEFAULT_ENGINE` > `selector.current_engine()`, `execution.rs:518-521`) applies only at creation.
  - `[auto]` Q: "Can a caller change the engine on a resume call to an existing session?" → Selected: **"No — engine is fixed at session creation; conflicting `engine` on resume → HTTP 400"** (recommended — changing engines mid-session discards working memory; honest 400 beats silent discard)

#### Reload interaction with live sessions
- **D-07: Existing stateful sessions keep their already-loaded `RuleSet` snapshot after a reload; `engine_generation` in the sessionful response reflects the generation captured at session creation.** A `reload_rules` swap (Phase 25 D-04) replaces the shared `SharedRuleSet` and bumps its `generation`; in-flight and existing stateful sessions are NOT silently reloaded — they hold their own `Box<dyn ExecutionEngine>` with the ruleset they were created with (per-request-clone isolation, Phase 25 L-05). The sessionful `ExecuteResponse.engine_generation` therefore stays at the session's creation-generation (NOT the current shared generation), so a caller can compare it to a fresh stateless execute's generation to detect "this session is running a stale rule set." New sessions created after a reload get the new generation. `POST /v1/execute/session/{id}/reset` MAY optionally reload against the current ruleset (planner's discretion — D-05 says reset clears WM; whether it also refreshes the ruleset is a planner call, default = just clear WM). The contract is: "existing sessions are NOT silently reloaded."
  - `[auto]` Q: "When `reload_rules` swaps the shared ruleset, do existing stateful sessions reload, or keep their snapshot?" → Selected: **"Keep their snapshot; `engine_generation` in the response reflects creation-generation so callers detect staleness"** (recommended — matches Phase 25 L-05 isolation; silent mid-session reload would surprise callers; the generation tag makes staleness visible)

#### `rrules_sessions_active` gauge + MetricsSnapshot.active_sessions
- **D-08: Add a dedicated `rrules_sessions_active` IntGauge on the Phase-24 prometheus `Registry` and populate the existing `MetricsSnapshot.active_sessions` field from `SessionManager::len()`.** The gauge is set to `SessionManager::len()` after each execute (stateless and sessionful) and on every sweep eviction. The existing `MetricsSnapshot.active_sessions` field at `execution.rs:657` (hardcoded `0` in Phase 24, with the inline "Phase 27 populates active_sessions" comment) reads from `SessionManager::len()` in the snapshot built at `execution.rs:651-660`. The Prometheus `/metrics` text output (Phase 24 `TextEncoder` over the dedicated registry) exposes `rrules_sessions_active <N>`. The leak becomes visible on the dashboard (Pitfall 5 step 4).
  - `[auto]` Q: "How is active session count surfaced — Prometheus gauge, MetricsSnapshot field, or both?" → Selected: **"Both: dedicated `rrules_sessions_active` gauge + populate the existing `MetricsSnapshot.active_sessions` field"** (recommended — ROADMAP mandates the gauge; the field already exists and was explicitly left for Phase 27)

#### E2E keystone test (SC-1/SC-2/SC-3/SC-4)
- **D-09: Extend `api/tests/execute_integration.rs` with the four SC scenarios, Wave 0 RED → GREEN + Nyquist.** SC-1 (accumulated WM): two calls with the same `session_id`; call 1 inserts `{scorecard:{score:0}}` and fires `score := score + 10` → `facts_after.score == 10`; call 2 (same `session_id`) inserts no new facts but fires again → `facts_after.score == 20` (proves WM accumulated across calls — impossible in the stateless path). SC-2 (TTL): a session untouched for >TTL is gone from the registry; a session touched within TTL persists. Use a tiny TTL via env (`RRULES_SESSION_IDLE_TTL_SECS=1`) + a short `tokio::time::sleep`, OR inject a fake clock — planner picks (D-discretion). SC-3 (LRU + gauge): when the session count exceeds `MAX_SESSIONS` the LRU entry is evicted and `rrules_sessions_active` (read from `/metrics` or the WS snapshot) reflects the live (capped) count. SC-4 (lifecycle): `DELETE /v1/execute/session/{id}` disposes (subsequent call with that id creates a fresh session with empty WM); `POST /v1/execute/session/{id}/reset` clears WM without disposing (subsequent call on the same id resumes with empty WM but the same engine/generation). Use `serial_test` (`#[serial]`) + `SessionManager::clear()` for registry isolation (mirrors Phase 26 `CoverageTracking`/`RuleExecutionTracker` test isolation).
  - `[auto]` Q: "Where to place the SC-1/SC-2/SC-3/SC-4 tests and what should each assert?" → Selected: **"Extend `api/tests/execute_integration.rs`; four RED→GREEN scenarios mirroring Phase 23/24/25/26 Wave 0; `#[serial]` + `clear()` for registry isolation"** (recommended — established v5.0 test pattern; the cross-call `score:20` assertion is what makes SC-1 meaningful vs the stateless path)

#### Background sweep task lifecycle
- **D-10: Spawn the sweep task in `main.rs` alongside the existing `start_file_watcher` (`main.rs:146`) using `tokio::spawn` + `tokio::time::interval`.** The sweep task loops on a `tokio::time::interval(sweep_interval)` and runs `SessionManager::sweep_expired()` (evicts by TTL/max-lifetime/LRU + updates the gauge). Graceful shutdown follows the existing `start_file_watcher` pattern (no explicit `CancellationToken` unless the planner finds one already in use — match the established pattern). The `SessionManager` is constructed once at startup and injected as `web::Data<Arc<SessionManager>>` (mirrors `EngineRegistry`/`ExecutionCounters`); the sweep task takes an `Arc<SessionManager>` clone.
  - `[auto]` Q: "Where does the sweep task live and how is it shut down?" → Selected: **"`tokio::spawn` in `main.rs` alongside `start_file_watcher`; `tokio::time::interval` loop; match existing shutdown pattern"** (recommended — consistent with the existing background-task wiring; no new shutdown machinery)

#### Session concurrency model (per-session Mutex) — implied by the `&mut self` engine contract
- **D-11: `DashMap<String, Arc<Mutex<StatefulSessionEntry>>>` — concurrent calls on the SAME `session_id` serialize; different sessions stay fully concurrent.** The `Box<dyn ExecutionEngine>` is `!Sync` (`execute(&mut self)`, `insert_fact(&mut self)` — Phase 23 Pitfall 1), so a session entry must be wrapped in a `Mutex`. The per-entry `Arc<Mutex<...>>` means two concurrent calls with the same `session_id` serialize on that one Mutex (the second waits for the first's `web::block` to finish), while calls on different `session_id`s take different DashMap shards + different per-entry Mutexes and stay concurrent. This is the session-level analogue of Phase 23's "factory shared, instance per request" — here "registry shared, engine per session, Mutex per session." Lookups use DashMap's sharded locks (no global contention). The per-session Mutex also protects `last_used`/`engine_generation` metadata. Not explicitly named in ROADMAP text but implied by the engine `&mut self` contract.
  - `[auto]` Q: "How are concurrent calls on the same `session_id` handled, given the engine is `&mut self`/`!Sync`?" → Selected: **"Per-session `Mutex` — same-session calls serialize, different sessions stay concurrent"** (recommended — required by the `!Sync` engine contract; mirrors Phase 23's isolation discipline at session granularity)

#### NEW `sessions.rs` module + wiring
- **D-12: NEW `api/src/v1/sessions.rs` with `SessionManager` + `StatefulSessionEntry` + sweep task; inject as `web::Data<Arc<SessionManager>>`.** `SessionManager` is constructed once at startup (after `EngineRegistry`/`EngineSelector`/`SharedRuleSet`) and injected via `.app_data(web::Data::new(session_manager.clone()))` in `main.rs` (mirrors `execution.rs` web::Data wiring at `main.rs:186-198`). `execute_single` gains a `web::Data<Arc<SessionManager>>` param (and the existing `web::Data<Arc<RwLock<SharedRuleSet>>>` is used at session creation to load the ruleset). New routes registered in the `/v1` scope (`main.rs:208-267`): `DELETE /v1/execute/session/{id}` and `POST /v1/execute/session/{id}/reset`. Add `dashmap = "6.2.1"` to `api/Cargo.toml` (NOT 7.0.0-rc2 — ROADMAP pins 6.2.1). The `uuid` crate is already a dep (`execution.rs:11`).
  - `[auto]` Q: "Where does the new session code live and how is it wired?" → Selected: **"NEW `api/src/v1/sessions.rs`; `web::Data<Arc<SessionManager>>` injected in `main.rs`; `execute_single` gains the param; DELETE/reset routes in the `/v1` scope"** (recommended — matches ROADMAP deliverable "NEW `api/src/v1/sessions.rs`" + the established `web::Data` wiring pattern)

### Claude's Discretion (technical impl details left to the planner/researcher)
- **`StatefulSessionEntry` exact field set** — the contract is: a `Box<dyn ExecutionEngine>` + `engine_name: String` + `engine_generation: u64` + `last_used: Instant` + `created_at: Instant` + `server_id: Uuid` + `client_alias: Option<String>`. Whether to factor a `StatefulSession` struct (engine + metadata) separate from the entry, or keep it flat, is planner's choice.
- **TTL test mechanism** — real short TTL + `tokio::time::sleep` vs. an injectable `Clock`/`now()` trait for deterministic tests. The contract is SC-2 must be deterministic; mechanism is planner's pick (injecting a fake clock is cleaner but adds a trait — planner weighs it).
- **`reset` semantics** — D-05 says reset clears working memory without disposing. Whether reset ALSO reloads against the current shared ruleset (refreshing `engine_generation`) or keeps the session's original ruleset is planner's discretion (D-07 leans "keep original"; reset-also-reload is a reasonable alternative the planner may pick).
- **`execute_with_session` route fate** — remove entirely vs. keep as a thin alias to `execute_single`. The contract is "no production code returns `rules_fired: 0` for a sessionful call"; mechanism is planner's choice.
- **Sweep-task shutdown** — `CancellationToken` vs. runtime-teardown-only. Match the existing `start_file_watcher` pattern unless the planner finds a cleaner existing mechanism.
- **Idempotency-alias storage** — a separate `DashMap<String, Uuid>` (alias → server UUID) vs. a field on the entry + a linear scan. The contract is "client `session_id` resumes an existing session"; storage is planner's pick (a second DashMap is O(1) lookup; a field + scan is simpler but O(n)).
- **LRU eviction selection** — `DashMap` iter + `min_by(last_used)` vs. an auxiliary ordering structure. At `MAX_SESSIONS ~1024` a periodic iter is cheap; planner picks.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap & requirements (scope + success criteria)
- `.planning/ROADMAP.md` §"Phase 27: Stateful sessions" (line 217) — goal, depends-on (Phase 23), requirement (SESS-01), 7 key deliverables, Pitfall 5 to avoid, research flags ("Light — TTL/sweep defaults are config knobs (proposed: ~15 min idle TTL, ~24 h max lifetime, ~1024 max sessions); not architecturally load-bearing"), and the 4 success criteria verification must satisfy. **Note the UI hint: "yes (session endpoints surface in the API the frontend will call)" — frontend is Phase 28.**
- `.planning/REQUIREMENTS.md` — traceability: `SESS-01` → Phase 27 (Pending).

### Milestone research (pitfalls + features — HIGH confidence, file:line evidence)
- `.planning/research/PITFALLS.md` §"Pitfall 5" (line 120) — stateful sessions never expire / leak to OOM. Prevention: `Arc<RwLock<HashMap<String, SessionEntry>>>` (D-02 chooses DashMap instead — sharded) with `SessionEntry { session, last_used, created_at }`; max session count (~1024) + LRU + idle TTL (~15 min) + max lifetime (~24 h) + background sweep every 60 s; reject caller-collision / prefer server-generated IDs (D-04); `/v1/sessions` admin endpoint + `rrules_sessions_active` gauge (D-08 ships the gauge; the admin endpoint is deferred — see domain Out of scope).
- `.planning/research/PITFALLS.md` §"Pitfall 1" (line 13) — the `&mut self` engine contract that drives D-11 (per-session Mutex). "For stateful sessions, store per-session `Engine` clones in a session registry, not in `EngineState`" — D-01 generalizes this to `Box<dyn ExecutionEngine>` (the real Phase-23 surface, not the v1.0 `Engine`/`Session` stub).
- `.planning/research/FEATURES.md` §"Stateful KIE-session-style sessions" (line 41) — `Engine::create_session(stateful=true, Some(id))` + `Session::insert_fact/execute/destroy` exist but are stubs (D-01 correction); `session_id` field already on `ExecuteRequest` (execution.rs:23); needs a `SessionStore` (D-02 `SessionManager`). Anti-Features §"Cross-request shared working memory in stateless mode" (line 52) — confirms stateless stays strictly per-request (D-05).

### Prior phase context (the scaffold Phase 27 builds on)
- `.planning/phases/23-execute-evaluator-wiring/23-CONTEXT.md` — **L-03** (per-request `Box<dyn ExecutionEngine>` via `create_engine_by_name`, no pool — the surface a stateful session wraps, D-01), **L-06** (once-per-rule coverage — the sessionful path reuses the same recording), the `web::Data` + `spawn_blocking` pattern, and `serial_test` as a workspace dev-dep. Also the deferred item "Stateful sessions — Phase 27" (line 138, 26-CONTEXT also defers to 27).
- `.planning/phases/25-real-reload-jobstore/25-CONTEXT.md` — **D-02** (`SharedRuleSet{ruleset, generation}` — the snapshot a session captures at creation, D-07) and **L-05** (per-request-clone isolation — sessions keep their snapshot on reload, D-07).
- `.planning/phases/26-ruleexecutiontracker-singleton-dead-rule-live-data/26-CONTEXT.md` — **D-05** (deferred stateful sessions = Phase 27, line 148) and the `serial_test` + `clear()` singleton-isolation test pattern (D-09 mirrors it for `SessionManager`).

### Live code (the actual integration points — verified during this discussion)
- `api/src/v1/execution.rs:21-34` — `ExecuteRequest.session_id: Option<String>` already exists (Phase 23); Phase 27 gives it resume semantics (D-04/D-05).
- `api/src/v1/execution.rs:47-65` — `ExecuteResponse`; Phase 27 adds `session_id: Option<String>` (server UUID, D-04). `engine_generation: u64` (line 63) already exists; the sessionful path stamps the session's creation-generation (D-07).
- `api/src/v1/execution.rs:504-575` — `execute_single`; Phase 27 adds the `session_id` branch (D-05) + `web::Data<Arc<SessionManager>>` param (D-12).
- `api/src/v1/execution.rs:651-660` — `MetricsSnapshot` build; `active_sessions: 0` at line 657 with the inline "Phase 27 populates active_sessions" comment (L-05/D-08).
- `api/src/v1/execution.rs:1081-1094` — `execute_with_session` stub (`POST /v1/execute/session/{id}`, returns `rules_fired: 0`); removed/repurposed (D-05).
- `engine/src/environment.rs:95-208` — `Engine::create_session` + `Session` (the v1.0 STUB to NOT use; D-01): `execute()` returns "Not implemented" (line 191), `insert_fact` is a no-op (line 176), raw `*const Engine` (line 154, not `Send`). Documented and left in place.
- `engine/src/engine/registry.rs` — `EngineRegistry` + `ExecutionEngine` trait (`load_ruleset`/`insert_fact`/`execute`/`get_facts`/`reset`) + `create_engine_by_name` — the real surface a session wraps (L-01/D-01).
- `api/src/main.rs:44-102` — `web::Data` wiring pattern for shared state (registry/selector/ruleset/counters); `:146` `tokio::spawn(v1::execution::start_file_watcher(...))` — the sweep task goes alongside (D-10); `:186-198` `.app_data(...)` chain — add `web::Data::new(session_manager.clone())` (D-12); `:208-267` `/v1` route scope — add DELETE/reset routes (D-05).
- `api/Cargo.toml` — add `dashmap = "6.2.1"` (NOT 7.0.0-rc2 — ROADMAP pins 6.2.1, D-02). `uuid` already a dep.

### No external specs
No ADRs or external standard specs apply — brownfield internal-wiring phase against the project's own Rust codebase. Drools/KIE stateful-session lifecycle (lookup/DisposeCommand) is background context only (FEATURES.md sources), not a spec to implement.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`ExecuteRequest.session_id: Option<String>`** (`execution.rs:23`) — already exists from Phase 23; Phase 27 gives it real resume/idempotency semantics (D-04). No new request field needed for the idempotency alias.
- **`ExecuteResponse.engine_generation: u64`** (`execution.rs:63`, `#[serde(default)]`) — already exists from Phase 24/25; the sessionful path stamps the session's creation-generation (D-07). Add `session_id: Option<String>` (server UUID) with `#[serde(skip_serializing_if = "Option::is_none")]` (D-04).
- **`EngineRegistry::create_engine_by_name`** (`registry.rs:177`) + `available_engines()` — used at session creation to build the per-session `Box<dyn ExecutionEngine>` and validate the `engine` param (D-06), exactly as `execute_single` does at `execution.rs:525-549`.
- **`SharedRuleSet{ruleset, generation}`** (Phase 25 D-02, `execution.rs:67`) — read once at session creation to `load_ruleset` into the session's engine and capture `engine_generation` (D-07); the read-lock-then-clone-then-release pattern (`execution.rs:559-575`) is reused.
- **`MetricsSnapshot.active_sessions` field** (`execution.rs:657`) — left at `0` in Phase 24 with the inline "Phase 27 populates active_sessions" comment; Phase 27 reads `SessionManager::len()` into it (D-08/L-05).
- **Phase-24 prometheus `Registry` + `ExecutionCounters`** (`metrics.rs`, `execution.rs` counters) — the dedicated registry where the `rrules_sessions_active` IntGauge is registered (D-08); mirrors the `ExecutionCounters` pattern (`main.rs:102`, `app_data` at `:188`).
- **`uuid::Uuid::new_v4()`** (`execution.rs:11`, used at `:512`) — server-generated session IDs (D-04).
- **`serial_test` + `clear()`** (Phase 23/26 pattern) — `SessionManager::clear()` + `#[serial]` for registry-isolation tests (D-09).
- **`tokio::spawn` + `tokio::time::interval`** — for the background sweep task (D-10); `start_file_watcher` (`main.rs:146`) is the established spawn-pattern reference.
- **`web::Data<Arc<T>>`** — `SessionManager` is `Arc`-wrapped and shared across actix workers (D-12), mirroring `EngineRegistry`/`ExecutionCounters`.

### Established Patterns
- **Factory shared, instance per request → registry shared, engine per session, Mutex per session** (Phase 23 L-03 generalized) — `EngineRegistry` is `web::Data`-shared; a per-session `Box<dyn ExecutionEngine>` is created at session creation; a per-session `Mutex` serializes same-session calls (D-11). This is the session-level analogue of Phase 23's "no shared locks between requests" discipline.
- **`web::Data<T>` + `spawn_blocking` for CPU-bound engine work** (Phase 23/24) — the sessionful `execute` locks the per-session Mutex, then runs the engine inside `web::block` (the `&mut self` engine is `!Sync`, L-04).
- **Wave 0 RED → GREEN + Nyquist closure** (Phase 23/24/25/26) — SC-1/SC-2/SC-3/SC-4 written RED first, then implemented to GREEN (D-09). The cross-call `score:20` assertion (SC-1) is the keystone that proves WM accumulated across calls — impossible in the stateless path.
- **`#[serde(default)]` / `#[serde(skip_serializing_if = "Option::is_none")]`** for response field additions — keeps existing clients/fixtures parsing (Phase 24 `engine_generation`, Phase 25 batch fields).

### Integration Points
- **NEW `api/src/v1/sessions.rs`** — `SessionManager` (`Arc<DashMap<String, Arc<Mutex<StatefulSessionEntry>>>>`), `StatefulSessionEntry`, `sweep_expired()`, `len()`, `clear()` (D-02/D-03/D-12).
- **`api/src/v1/execution.rs:504` (`execute_single`)** — add `web::Data<Arc<SessionManager>>` param; branch on `req.session_id` (D-05); stamp `engine_generation` from the session's creation-generation (D-07); populate `active_sessions` at `:657` (D-08).
- **`api/src/v1/execution.rs:1081` (`execute_with_session`)** — remove/repurpose the stub (D-05).
- **`api/src/main.rs:44-102`** — construct `SessionManager` once at startup; `:146` spawn the sweep task alongside `start_file_watcher` (D-10); `:186-198` add `web::Data::new(session_manager.clone())` (D-12); `:208-267` add `DELETE /v1/execute/session/{id}` + `POST /v1/execute/session/{id}/reset` routes (D-05).
- **`api/Cargo.toml`** — add `dashmap = "6.2.1"` (D-02/D-12).
- **`api/tests/execute_integration.rs`** — add SC-1/SC-2/SC-3/SC-4 keystone e2e, `#[serial]` + `SessionManager::clear()` for isolation (D-09).

</code_context>

<specifics>
## Specific Ideas

- **Keystone honesty anchor (SC-1):** author one rule `r1` (`score := score + 10`). `POST /v1/execute` with `session_id: "s1"`, `facts: {scorecard:{score:0}}` → `rules_fired >= 1`, `facts_after.scorecard.score == 10`, response carries `session_id: "<server-uuid>"`. Then `POST /v1/execute` with the SAME `session_id: "s1"` and NO new facts → fires `r1` again against the accumulated WM → `facts_after.scorecard.score == 20`. The `score:20` (not `score:10`) on the second call is the proof that working memory accumulated across calls — the stateless one-shot path would return `score:10` both times (it starts from the request facts each call). If `score:20` appears, the keystone is honest: D-01 (real `Box<dyn ExecutionEngine>` per session), D-04 (resume-on-match), D-05 (`execute_single` branch), D-06 (engine fixed), D-11 (per-session Mutex) all work.
- **SC-3 (LRU + gauge) honesty check:** set `RRULES_MAX_SESSIONS=2` in the test, create 3 sessions (s1, s2, s3 in order, each touched in that order), then assert s1 (the LRU) is evicted, s2 and s3 persist, and `rrules_sessions_active` (read from `/metrics` text or the WS `MetricsSnapshot`) reads `2` — NOT `3`. The gauge reflecting the capped count (not the raw count) is what makes the leak visible (Pitfall 5 step 4).
- **SC-4 (lifecycle) honesty check:** `DELETE /v1/execute/session/{id}` → a subsequent `POST /v1/execute` with that `session_id` creates a FRESH session (empty WM, new `engine_generation` if a reload happened in between); `POST /v1/execute/session/{id}/reset` on a session with accumulated WM → a subsequent `POST /v1/execute` with that `session_id` resumes with EMPTY WM (same engine/generation) — proving reset clears WM without disposing.
- The user's consistent posture across v5.0 (Phase 23/25/26 CONTEXT "Specifics") is to take the **honest-for-real-authored-rules** option. The planner should not narrow D-03 (all four controls ship together) or D-06 (engine fixed, 400 on conflict) to "minimal that passes SC-1 only" — the OOM bounding (SC-2/SC-3) and the mid-session-engine-honesty (D-06) must both be real, because the user explicitly prefers the bounded-honest path.

</specifics>

<deferred>
## Deferred Ideas

- **RBAC-gated `/v1/sessions` admin list/count endpoint** — Pitfall 5 step 4 mentions it; NOT in ROADMAP Phase 27 deliverables. Phase 27 ships only the `rrules_sessions_active` gauge + the lifecycle routes (DELETE/reset). A future governance/ops phase can add the admin endpoint (list sessions, force-evict, inspect WM) with RBAC.
- **Frontend test-runner / session UI** — Phase 28 (consumes `session_id` from `ExecuteResponse`; the "Reload then Run" flow + multi-interaction scenarios).
- **`inference_trace` population** — Phase 28 (EXEC-04).
- **Removing the v1.0 `engine::environment::Session` stub** — documented (D-01) but left in place (other v1.0 code references it); a future cleanup phase can remove it once confirmed unused.
- **Cross-session shared working memory / undeclared state** — explicitly an anti-feature (FEATURES.md §"Anti-Features"); stateless stays strictly per-request. Not a deferral — a permanent design constraint.
- **Session-scoped `record_coverage: bool` flag** — Pitfall 6 mentioned a batch `record_coverage` flag; whether stateful sessions write to the global `CoverageTracking` singleton (they do — same `direct_eval.rs` call sites, L-06) is already covered; a separate opt-out flag is not in ROADMAP and is deferred.

None of these were scope-creep suggestions from the user — they are the phase's own downstream milestones / explicit out-of-scope items, listed so the planner knows the boundaries.

</deferred>

---

*Phase: 27-stateful-sessions*
*Context gathered: 2026-07-04 (--auto mode)*