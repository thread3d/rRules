# Phase 23: Execute evaluator + wiring - Context

**Gathered:** 2026-07-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Build a minimal **real** rule evaluator on `ReteEngine` and wire `POST /v1/execute` through the real v3.0 `EngineRegistry`/`ExecutionEngine` surface so rules actually fire — closing the documented stub. After this phase: `/v1/execute` returns truthful `rules_fired`/`rules_not_fired`/`facts_after` (built from post-execution working memory, not echoed input), the `engine` request param selects among all four engines, `CoverageTracking` auto-populates once-per-fired-rule, and the v4.0 coverage surface reads live data.

This is **build + wire, not wire-only** (the milestone's original "wiring" framing was corrected during Phase 23 research). It is the keystone of the v5.0 Live Execution vertical slice.

**In scope (locked by ROADMAP.md + 23-RESEARCH.md):**
- Fix the `engine::` → `crate::` import bug in all four engine `mod.rs`; enable all four `engine-*` cargo features on the `api` crate
- Extend the `ExecutionEngine` trait with `load_ruleset` + `get_facts` (Option (b))
- Rewrite the placeholder `ReteEngine::execute_rules` into a real direct interpreter (evaluate conditions, apply actions) — and the equivalent on all four engines (per D-04)
- Share `Arc<EngineRegistry>` + `EngineSelector` as `web::Data`; fresh `Box<dyn ExecutionEngine>` per request via `spawn_blocking`; no engine pool
- `engine: Option<String>` on `ExecuteRequest`; validate against `available_engines()` (400 on unknown); config default `RRULES_DEFAULT_ENGINE` (default `"rete"`)
- `facts_after` serialized from post-execution `WorkingMemory` (reverse `engine_value_to_json`), grouped by `fact_type_id`
- `CoverageTracking` records once-per-rule outside the nagram inner loop (Pitfall 6)
- Prep rename `metrics::EngineState` → `MetricsSnapshot` (Pitfall 10) before adding logic
- Build a structured **action-expression evaluator** (D-06 — research correction: no `Expression` type exists today)
- Forward-chaining **iterate-to-fixpoint** with a configurable max-iterations cap (D-07)

**Out of scope (explicitly deferred):**
- Full Rete alpha/beta network propagation / a `RuleSet → ReteNetwork` compiler (the network types are themselves placeholder; a direct interpreter is the minimal honest path — a future enhancement can swap in behind the same `load_ruleset`/`get_facts` contract)
- `inference_trace` population (left `None` — EXEC-04 / Phase 28)
- RuleSet reload-then-swap (Phase 25; Phase 23 only builds the read-only shared snapshot that Phase 25 will swap)
- `no-loop` / `lock-on-active` cycle semantics (v2.0 testing-framework territory; Phase 23 uses a max-iterations cap only)
- `RuleExecutionTracker` singleton-ization (Phase 26)

</domain>

<decisions>
## Implementation Decisions

### Non-Rete engines
- **D-01: Implement the direct-evaluator on ALL FOUR engines.** The direct interpreter is built and wired on Rete, Treat, Leaps, and Phreak — `engine: "treat"|"leaps"|"phreak"` actually fires rules in Phase 23, not just `"rete"`. All four `EngineFactory` impls are confirmed real (`23-RESEARCH.md` Flag 1); the only blocker was the identical import bug (fixed here). `register_default_engines` is extended to register all four under their `#[cfg(feature = "engine-X")]` blocks, and `available_engines()` truthfully returns all four names. (Chosen over "register 4, stub 3" and "register Rete only".)
- **D-02: Test all four engines equally.** Each engine gets its own `score+10` integration test proving it actually fires (4 tests, same fixture). The guarantee that "implement all 4" is honest, not merely compile-clean. (Chosen over "Rete full + others smoke".)

### RuleSet source
- **D-03: Load real rules from disk at startup (live).** Build a shared read-only `Arc<RwLock<RuleSet>>` from `RULE_STORAGE_PATH` once at startup and inject it as `web::Data`. After Phase 23, `/v1/execute` runs against the user's actual authored rules — a genuinely live system. This snapshot is the scaffold Phase 25 swaps; reload-then-swap is Phase 25's scope. Each request takes a read lock and `load_ruleset` clones the rules into the per-request engine. (Chosen over "test-fixture only".)
- **D-03a: Empty/missing rules directory at startup → warn and continue.** Start with an empty `RuleSet` and log a warning; the server comes up and `/v1/execute` works but fires nothing until rules are authored. Consistent with the live-system choice. (Chosen over "fail-fast at startup".)

### Evaluator scope
- **D-04: Full direct-interpreter rewrite in Phase 23.** Confirm the keystone scope: rewrite the placeholder `ReteEngine::execute_rules` (and its equivalent on the other three engines) into a real evaluator that evaluates each `Rule.condition` against `working_facts`, applies matching `Rule.actions`, and records coverage once per fired rule. Meets ROADMAP success criterion #1 now (`score := score + 10` → `rules_fired >= 1`, `facts_after` incremented). (Chosen over descoping the rewrite to Phase 25.)
- **D-05: Full `ComparisonOp` coverage across all value types.** Condition matching implements the full `ComparisonOp` enum (`Eq`/`Ne`/`Lt`/`Le`/`Gt`/`Ge`) across `Integer`/`Float`/`String`/`Boolean`, with real `Pattern` matching (`fact_type` + `field` + `value` + `op` against facts of that type via `working_facts`). **Research correction:** `Condition::evaluate` today only handles `Eq` on Integer/Float/String — every other op and `Pattern` falls to `_ => false` / returns `true`. So "full ops" is a real build, not a tweak. (Chosen over "equality-only".)

### Action-expression depth (research correction)
- **D-06: Full expression language.** Phase 23 builds a structured **`Expression` evaluator** and wires it into action application: arithmetic (`+ - * /`), string concatenation, comparisons, nested expressions, plus wiring the existing `CallFunction` action. This matches the v1.0 `RULE-05` expression-language promise so all real authored rules work honestly on the live path. **RESEARCH CORRECTION (important):** the engine has **no `Expression` type** today — `Action::UpdateFact` only accepts literal `Value`s in its `fields: HashMap<String, Value>`. Therefore `score := score + 10` is **not representable** in the current `Action` enum, and ROADMAP success criterion #1 cannot pass without this work. Phase 23 must extend the action model (e.g., introduce an `Expression` type / make `UpdateFact` field values support computed expressions) — this is a real scope addition the research under-stated, not just wiring. (Chosen over "minimal arithmetic only".)

### Forward-chaining semantics
- **D-07: Iterate to fixpoint with a max-iterations cap.** The live interpreter repeats passes until no rule newly fires (true forward chaining — rule A's action can enable rule B, which then fires), guarded by a **configurable max-iterations cap** as the cycle guard. The cap default is a config knob for the researcher/planner to set sensibly (propose a generous default, e.g. ~1000, and make it `RRULES_MAX_CHAIN_ITERATIONS` or similar). **Do NOT re-implement `no-loop` / `lock-on-active`** — those cycle-control semantics are v2.0 testing-framework territory; Phase 23 uses only the max-iterations cap. (Chosen over "single pass".) Note: the live execute path is brand-new (the stub `environment::Engine::execute_forward` returns "not yet implemented"), so fixpoint is build, not regression.

### facts_after contract (defaulted — no full discussion)
- **D-08: Keep the existing `facts_after` shape.** Serialize `facts_after` from `engine.get_facts()` via the reverse `engine_value_to_json` mapping, grouped by `fact_type_id`, into the existing `{ fact_type: json_value }` map shape the `ExecuteResponse` already exposes. Frontend-compatible (Phase 28 consumes it); avoids unplanned frontend shape changes. (Defaulted; revisit if the planner finds the existing shape cannot carry post-execution facts faithfully.)

### Carried forward as locked (not re-asked — from ROADMAP.md + 23-RESEARCH.md)
- **L-01:** Import-bug fix (`engine::` → `crate::`) in `engine/src/engine/{rete,treat,leaps,phreak}/mod.rs` (lines 13/12/13/12); enable `engine-rete`/`engine-treat`/`engine-leaps`/`engine-phreak` on the `api` crate.
- **L-02:** Bridging approach = Option (b): extend `ExecutionEngine` trait with `load_ruleset(&mut self, &RuleSet) -> Result<(), String>` + `get_facts(&self) -> Vec<Fact>` (default impls returning `Err`/`Vec::new()`), implemented on `ReteEngine` (and per D-01, all four).
- **L-03:** Per-request construction, no engine pool (`ReteEngine::new()` is O(1), `!Clone`; a pool would break the batch "no shared locks between requests" contract and add invalidation complexity — `23-RESEARCH.md` Flag 4).
- **L-04:** `inference_trace: None` in Phase 23 (populating it is EXEC-04 / Phase 28; do NOT wire the chaining `InferenceTraceCollector` — it's coupled to the stub `environment::Engine` surface).
- **L-05:** Prep rename `api/src/metrics.rs::EngineState` → `MetricsSnapshot` (Pitfall 10) before adding execute logic.
- **L-06:** Coverage once-per-rule outside the nagram inner loop (Pitfall 6); `facts_after` from post-execution `WorkingMemory`, not echoed input (Pitfall 11).
- **L-07:** Unknown `engine` name → HTTP 400; omitted `engine` → config default `RRULES_DEFAULT_ENGINE` (default `"rete"`).

### Claude's Discretion (technical impl details left to the planner/researcher)
- **EngineSelector sharing model** — `EngineSelector::select_engine` takes `&mut self`, so set the default engine once at startup (mutably, before wrapping in `web::Data`) and use a `&self` validation against `available_engines()` for per-request `engine` param checks (avoid a `Mutex` unless per-request mutation is needed). (Research Open Q#3.)
- **`rules_not_fired` exposure mechanism** — either extend `EngineResult` with `fired_rule_ids: Vec<String>` or add a trait method `fired_rule_ids(&self)`. Research recommends extending `EngineResult` (single return value, no extra trait method). Planner to confirm. (Research Open Q#5.)
- **`Condition::evaluate` Pattern internals** — how `Pattern { fact_type, field, value }` is matched against `working_facts.values()` (filter by `fact_type_id`, then compare `field` to `value` with the `ComparisonOp`). Part of the rewrite (D-04/D-05). (Research Open Q#6.)
- **Fixpoint cap default value + env var name** — pick a sensible default (~1000) and a config knob name (D-07).
- **How the four evaluators share code** — the direct-evaluator loop is engine-agnostic (operates on `self.rules` + `self.working_facts`); the planner may factor a shared helper/trait method to avoid 4× duplication, or mirror per-engine. Architectural choice; not a user-facing decision.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase research (primary — HIGH confidence, all claims verified against live code with file:line evidence)
- `.planning/phases/23-execute-evaluator-wiring/23-RESEARCH.md` — the phase research. Resolved all four flags; identified the two blockers (import bug, placeholder `execute_rules`) and the bridging approach (Option (b)). Contains the full Implementation Approach (11 ordered steps) and Validation Architecture. **Note:** its Open Q#6 under-states condition matching (only `Eq` is implemented today) and it does not flag the missing `Expression` type — see D-05 and D-06 above for corrections discovered during discussion.

### Roadmap & requirements (scope + success criteria)
- `.planning/ROADMAP.md` §"Phase 23: Execute evaluator + wiring" — goal, scope note (build + wire correction), key deliverables, pitfalls to avoid, research flags, and the 4 success criteria that verification must satisfy.
- `.planning/REQUIREMENTS.md` — traceability: `EXEC-01`, `EXEC-02`, `ENGINE-07`, `ENGINE-08`, `MON-05` → Phase 23.

### Milestone research (pitfalls + architecture + synthesis)
- `.planning/research/PITFALLS.md` — Pitfalls 1 (`&mut self` engine-sharing serialization → factory shared, instance per request), 6 (coverage double-counting at `rete/mod.rs:332`), 7 (per-request compile cost → no pool), 9 (`Engine::clone()` ≠ `ReteEngine`), 10 (`EngineState` name collision → rename `MetricsSnapshot`), 11 (`facts_after` echo bug).
- `.planning/research/ARCHITECTURE.md` — two-engine-surface finding (stub `environment::Engine` vs real `EngineRegistry`), §1–§2 build order.
- `.planning/research/SUMMARY.md` — v5.0 milestone research synthesis (Phase 1 roadmap framing).

### No external specs
No ADRs or external standard specs apply — this is a brownfield internal-wiring phase against the project's own Rust codebase.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `EngineRegistry` (`engine/src/engine/registry.rs:139`) + `register_default_engines` (`registry.rs:158`, Rete-only today — extend per D-01) + `create_engine_by_name` (`registry.rs:177`) — the real engine surface to wire through.
- `ExecutionEngine` trait (`registry.rs:64-90`) — extend with `load_ruleset` + `get_facts` (L-02); has `execute(&mut self)`, `insert_fact(&mut self, Fact)`, `engine_type`, `reset`.
- `EngineSelector` (`engine/src/engine/selector.rs:59`) — `with_defaults()` (`selector.rs:77`) already advertises all four engine names; `available_engines(&self)` (`selector.rs:87`) for per-request validation; `select_engine(&mut self)` (`selector.rs:98`) — set default once at startup.
- `ReteEngine` (`engine/src/engine/rete/mod.rs:200`) — `working_facts: HashMap<FactId, Fact>` (`rete/mod.rs:204`) is the live working memory the evaluator mutates; `ReteEngine::new()` (`rete/mod.rs:210`) is the O(1) per-request constructor.
- `CoverageTracking` singleton (`engine/src/monitoring/coverage.rs:86` — `LazyLock<Mutex<…>>`) — `record_rule_eval` (`coverage.rs:97`), `record_rule_fire` (`coverage.rs:105`); the canonical once-per-rule recording target (Pitfall 6).
- `json_to_engine_value` (`api/src/v1/execution.rs:533`) — request-fact → `Value`/`Fact` conversion; needs a **reverse** `engine_value_to_json` mapping for `facts_after` (D-08 / Pitfall 11).
- `actix-test` (`api/Cargo.toml:48`) for HTTP integration tests; `serial_test` (workspace dev-dep) for `CoverageTracking` singleton isolation.

### Established Patterns
- **Singleton trackers via `LazyLock<Mutex<…>>`** — `CoverageTracking` (`coverage.rs:86`) is the established pattern; Phase 26 will mirror it for `RuleExecutionTracker`. Phase 23 records into `CoverageTracking` (do not introduce a new singleton).
- **`web::Data<T>` shared state + `spawn_blocking` for CPU-bound engine work** — `execute(&mut self)` is sync/CPU-bound, so run inside `web::block`/`tokio::task::spawn_blocking` (the established pattern for the batch path's `JoinSet` at `execution.rs:374-395`).
- **Factory shared, engine instance per request** — the `execution.rs:304-311` "no shared locks between requests" contract; `web::Data` handles are `Arc` under the hood so `.clone()` into each batch task is cheap.

### Integration Points
- `api/src/main.rs:32` — app state wiring; inject `EngineRegistry` (after `register_default_engines()`) and `EngineSelector` (default set mutably) as `web::Data` alongside the existing `engine_state`; wire into `.app_data(...)` at `main.rs:114`. Read `RRULES_DEFAULT_ENGINE` from env (`main.rs:48` pattern).
- `api/src/v1/execution.rs:312` (`execute_single`) — the rewrite target: accept `web::Data<EngineRegistry>`, `web::Data<EngineSelector>`, `web::Data<Arc<RwLock<RuleSet>>>`; resolve/validate engine; `load_ruleset`; `insert_fact` per request fact; `spawn_blocking` → `execute()`; serialize `facts_after` from `get_facts()`.
- `api/src/v1/execution.rs:355` (`execute_rules` handler signature) + `execution.rs:374` (`batch_execute_rules`) — pass through the new `web::Data` args; batch clones the `Arc` handles into each `JoinSet` task.
- `api/src/v1/execution.rs:327` — remove the `facts_after.insert(fact_type.clone(), fact_value.clone())` echo (Pitfall 11).
- `engine/src/engine/{rete,treat,leaps,phreak}/mod.rs` — import-bug fix (L-01) and the per-engine direct-evaluator impl (D-01/D-04).
- `engine/src/rules.rs` — `Action` (`:198`, extend for `Expression` per D-06), `Condition`/`ComparisonOp` (`:117`/`:144`, extend `evaluate` per D-05), `RuleSet` (`:228`).

</code_context>

<specifics>
## Specific Ideas

- The phase's honesty anchor is ROADMAP success criterion #1: a `score := score + 10` rule must return `rules_fired >= 1` and `facts_after` must show the **incremented** `score` (not the request input). This single scenario exercises the direct evaluator (D-04), full condition matching (D-05), the new action-expression evaluator (D-06), fixpoint iteration (D-07 — trivially, one rule), the reverse `facts_after` serialization (D-08 / Pitfall 11), and once-per-rule coverage (L-06). If this scenario passes end-to-end, the keystone is honest.
- The user's consistent posture across this discussion was to take on the fuller build where it keeps the live path honest (all four engines, full ops, live-from-disk, full expression language, fixpoint chaining). The planner should not narrow these to "minimal that passes the fixture" — the user explicitly chose the honest-for-real-authored-rules option each time.
- Research correction to propagate to the planner: both `Condition::evaluate` (only `Eq` on 3 types today) and `Action` (no `Expression` type) are more incomplete than `23-RESEARCH.md` implies. D-05 and D-06 capture the extra build this requires.

</specifics>

<deferred>
## Deferred Ideas

- **Full Rete alpha/beta network propagation / `RuleSet → ReteNetwork` compiler** — the network types (`TestNode`/`JoinNode`/`ActionNode` with `predecessor: "placeholder"`) are themselves placeholder; a future enhancement can swap a real network compiler in behind the same `load_ruleset`/`get_facts` contract. Not Phase 23.
- **`inference_trace` population** — EXEC-04 / Phase 28 (frontend test-runner consumes it). Left `None` in Phase 23.
- **RuleSet reload-then-swap** — Phase 25 (`Real reload + JobStore`). Phase 23 builds only the read-only shared snapshot (`Arc<RwLock<RuleSet>>`) that Phase 25 swaps.
- **`no-loop` / `lock-on-active` cycle-control semantics** — v2.0 testing-framework territory. Phase 23 uses only a max-iterations cap (D-07).
- **`RuleExecutionTracker` singleton-ization + dead-rule live data** — Phase 26.
- **`EngineState` → `MetricsSnapshot` rename** is in-scope (L-05); the *broadcast* of live counters via Prometheus + WebSocket is Phase 24 (`Execution counters + metrics broadcast`).

None of these were scope-creep suggestions from the user — they are the phase's own downstream milestones, listed here so the planner knows the boundaries.

</deferred>

---

*Phase: 23-execute-evaluator-wiring*
*Context gathered: 2026-07-02*