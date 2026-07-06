# Phase 23 Research: Execute wiring core

**Phase:** 23 — Execute wiring core
**Milestone:** v5.0 Live Execution
**Researched:** 2026-07-02
**Confidence:** HIGH (all claims verified by direct codebase reads with file:line evidence; no external docs consulted — this is a brownfield internal-wiring phase)

## Executive Summary

Phase 23's keystone is re-routing `POST /v1/execute` from the stub `engine::environment::Engine` surface through the real v3.0 `EngineRegistry`/`ExecutionEngine` surface so rules actually fire. The research resolved all four open flags against the actual code, and uncovered two blockers the planning docs did not fully flag. **(1) The pluggable-engine surface has never compiled:** all four engine modules (`rete`, `treat`, `leaps`, `phreak`) carry the same import bug — `use engine::monitoring::coverage::CoverageTracking;` (an absolute path that doesn't resolve inside the `engine` crate itself) at `rete/mod.rs:13`, `treat/mod.rs:12`, `leaps/mod.rs:13`, `phreak/mod.rs:12`. `cargo check -p engine --features engine-rete` fails with `E0433`. So Phase 23's first concrete task is fixing that one-line import (`engine::` → `crate::`) in all four files; until then no engine feature can be enabled on the `api` crate. **(2) `ReteEngine::execute_rules` is itself a placeholder, not just unwired:** it iterates `action_nodes × join_nodes × nagrams` and increments a counter, but never evaluates `Rule.condition` against facts and never applies `Rule.actions` to working memory (`rete/mod.rs:321`). There is also no `RuleSet → ReteNetwork` compiler anywhere — `RuleSet::compile()` (`rules.rs:285`) is a stub that sets a `compiled: bool` flag, and `ReteEngine` has no `load_ruleset`/`add_rule` (only `with_network()` at `rete/mod.rs:221`, which is never called). Therefore the ROADMAP success criterion #1 (a `score := score + 10` rule returns `rules_fired >= 1` and `facts_after` shows the incremented score) cannot be met by pure wiring — Phase 23 must also build the `RuleSet → engine` bridge and make `execute_rules` actually evaluate conditions and execute actions.

**Recommended bridging approach (Flag 2): Option (b) — extend the `ExecutionEngine` trait.** Add `fn load_ruleset(&mut self, &RuleSet) -> Result<(), String>` and `fn get_facts(&self) -> Vec<Fact>` (both with default impls returning `Err`/`Vec::new()`), implement them on `ReteEngine`, and replace the placeholder counting loop in `execute_rules` with a direct evaluation loop: for each loaded rule, evaluate its `Condition` against the engine's `working_facts`, and if it matches, apply its `Action`s to working memory and record one coverage event per fired rule (outside any inner loop). This is the minimal honest path; the existing Rete network types (`TestNode`/`JoinNode`/`ActionNode` with "placeholder" predecessor strings) are themselves too placeholder to drive, so a direct interpreter inside `ReteEngine` is lower-risk than building a full network compiler now.

**Per-request compile cost (Flag 4): ship per-request construction, no pool.** `ReteEngine::new()` is O(1) (four empty `HashMap::new()` calls); `load_ruleset` will be O(rules) — sub-millisecond for any realistic ruleset. `ReteEngine` is `!Clone` (`#[derive(Debug)]` only at `rete/mod.rs:200`), and `environment::Engine::clone()` (the stub surface, `environment.rs:78`) is irrelevant because Phase 23 routes through the registry, not `environment::Engine`. No pool is warranted. **Inference trace (Flag 3): leave `None` for Phase 23.** The `inference_trace: Option<InferenceTrace>` field is already plumbed (`execution.rs:44`); populating it would require instrumenting `execute_rules`, which is EXEC-04 (Phase 28) scope. An `InferenceTraceCollector` exists in `chaining/mode_selector.rs:135` but is tied to the stub chaining path, not `ReteEngine` — do not wire it in Phase 23.

## Current State (Stub Diagnosis)

`POST /v1/execute` flows through `execute_rules` (`api/src/v1/execution.rs:355`) → `execute_single` (`execution.rs:312`). Today:

- `execute_single` builds a throwaway `EngineBuilder::new().build()` (`execution.rs:316`) — the stub `environment::Engine` — assigns it to `_engine` (underscore = never driven), and builds a `_working_memory` (`execution.rs:317`) that is also unused.
- `rules_fired` is hardcoded to `0` (`execution.rs:331`).
- `facts_after` is built by cloning the request's input JSON: `facts_after.insert(fact_type.clone(), fact_value.clone())` (`execution.rs:327`) — i.e., it echoes the request input verbatim regardless of any engine mutation. `json_to_engine_value` (`execution.rs:533`) is called only to populate a `payload` HashMap that is then discarded (never inserted into any engine).
- `inference_trace` is always `None` (`execution.rs:341`).
- The handler signature `execute_rules(req: web::Json<ExecuteRequest>)` (`execution.rs:355`) takes no `web::Data` for an engine registry or selector — there is no registry in app state. `main.rs` injects `engine_state` (`web::Data::new(v1::execution::EngineState::new())` at `main.rs:32`) but that `EngineState` (`execution.rs:75`) holds the stub `Arc<RwLock<environment::Engine>>`, not a registry, and `execute_single` doesn't even receive it.

The stub `environment::Engine` surface (`engine/src/environment.rs`) is fully placeholder: `load_rules` returns `Ok(())` doing nothing (`environment.rs:122`), `execute_forward` returns `ExecutionResult::new(true, "Forward chaining not yet implemented")` (`environment.rs:132`), and `Session::execute` returns `"Not implemented"` (`environment.rs:191`). Routing through it would never fire a rule.

## Research Flag 1: Non-Rete EngineFactory Inventory

**All four `EngineFactory` impls exist.** File:line evidence:

| Factory | Struct location | `impl EngineFactory` location | `create_engine` returns |
|---------|-----------------|-------------------------------|-------------------------|
| `ReteEngineFactory` | `engine/src/engine/rete/mod.rs:392` | `rete/mod.rs:407` | `Box::new(ReteEngine::new())` (`rete/mod.rs:413`) |
| `TreatEngineFactory` | `engine/src/engine/treat/mod.rs:459` | `treat/mod.rs:480` | `Box::new(TreatEngine::new())` (`treat/mod.rs:486`) |
| `LeapsEngineFactory` | `engine/src/engine/leaps/mod.rs:473` | `leaps/mod.rs:488` | `Box::new(LeapsEngine::new())` (`leaps/mod.rs:494`) |
| `PhreakEngineFactory` | `engine/src/engine/phreak/mod.rs:674` | `phreak/mod.rs:689` | `Box::new(PhreakEngine::new())` (`phreak/mod.rs:695`) |

**Cargo feature definitions** (`engine/Cargo.toml:29-36`):
```toml
default = []
engine-rete = []
engine-treat = ["engine-rete"]   # each non-rete feature pulls engine-rete
engine-leaps = ["engine-rete"]   # for shared types (ReteNetwork, etc.)
engine-phreak = ["engine-rete"]
```
Module gating (`engine/src/engine/mod.rs:30-39`): `rete` under `cfg(feature="engine-rete")`; `treat` under `cfg(any(feature="engine-treat", feature="engine-leaps"))`; `leaps` under `cfg(feature="engine-leaps")`; `phreak` under `cfg(feature="engine-phreak")`. The re-export at `registry.rs:209-210` only re-exports `ReteEngine, ReteEngineFactory` under `engine-rete`.

**Two gaps Phase 23 must close:**

1. **`register_default_engines` (`registry.rs:158`) only registers Rete**, and only under `#[cfg(feature = "engine-rete")]`. It does NOT register `treat`/`leaps`/`phreak` even when their features are on. So `EngineRegistry::available_engines()` would return only `["rete"]` today. Phase 23 must extend it with three more `#[cfg(feature = "engine-X")]` blocks registering the corresponding factories (and the re-exports at `registry.rs:209` must be extended so the factory types are nameable from `register_default_engines`).

2. **BLOCKER — none of the four engine modules compile.** Each carries `use engine::monitoring::coverage::CoverageTracking;` (an absolute `engine::` path that does not resolve inside the `engine` crate in the 2021 edition):
   - `engine/src/engine/rete/mod.rs:13`
   - `engine/src/engine/treat/mod.rs:12`
   - `engine/src/engine/leaps/mod.rs:13`
   - `engine/src/engine/phreak/mod.rs:12`

   `cargo check -p engine --features engine-rete` → `error[E0433]: failed to resolve: use of unresolved module or unlinked crate 'engine'`. With all features: 4 errors. **The entire pluggable-engine surface is dead code that has never been compiled by CI with features on.** The fix is mechanical: `use engine::monitoring::coverage::CoverageTracking;` → `use crate::monitoring::coverage::CoverageTracking;` in all four files.

**`api/Cargo.toml:11`** currently declares `engine = { path = "../engine" }` with NO features. Phase 23 must change it to `engine = { path = "../engine", features = ["engine-rete", "engine-treat", "engine-leaps", "engine-phreak"] }`.

**Recommendation: enable all four engines now.** Since all four factories are confirmed real (struct + `impl EngineFactory` + `create_engine` returning a real engine), and the only blocker is the identical import bug (one-line fix per file), there is no reason to ship Rete-only. Phase 23 should fix all four imports, extend `register_default_engines` to register all four under their cfg blocks, and enable all four features on the `api` crate. `EngineRegistry::available_engines()` will then return `["rete", "treat", "leaps", "phreak"]` (order may vary by `HashMap` iteration) and the `engine` param accepts all four. (Note: `EngineSelector::with_defaults()` (`selector.rs:77`) already advertises all four names, so the selector needs no change for the name set — but see Flag 2 / Implementation for the `&mut self` sharing problem.)

## Research Flag 2: RuleSet → ExecutionEngine Bridging

**The `ExecutionEngine` trait** (`engine/src/engine/registry.rs:64-90`) exposes: `execute(&mut self) -> EngineResult`, `insert_fact(&mut self, Fact)`, `retract_fact`, `run(&mut self) -> usize`, `get_metrics`, `engine_type`, `reset`, `set_config`. It has **no `load_ruleset` and no way to read facts back out** — `execute()` returns `EngineResult { success, rules_fired, message }` (`registry.rs:93`), no facts. `Box<dyn ExecutionEngine>` is `!Clone`.

**`ReteEngine`** (`rete/mod.rs:200`) holds: `network: ReteNetwork`, `alpha_memory`, `beta_memory`, `working_facts: HashMap<FactId, Fact>` (`rete/mod.rs:204`), and `metrics`. Its only construction is `ReteEngine::new()` (empty network, `rete/mod.rs:210`) and `ReteEngine::with_network(network)` (`rete/mod.rs:221`) — the latter is never called anywhere in the codebase (confirmed via grep across `engine/src` and `api/src`). There is no `add_rule`, no `from_rule_set`, no `load_ruleset`.

**`ReteEngine::execute_rules`** (`rete/mod.rs:321`) is itself a placeholder:
```rust
pub fn execute_rules(&mut self) -> usize {
    let mut rules_fired = 0;
    for (_node_id, action_node) in &self.network.action_nodes {
        for (_join_id, _join_node) in &self.network.join_nodes {
            let nagrams = self.beta_memory.get_nagrams(&_join_node.id);
            for _nagram in &nagrams {
                CoverageTracking::record_rule_fire(&action_node.rule_id);
                rules_fired += 1;
                self.metrics.record_rule_fired();
            }
        }
    }
    self.metrics.set_execution_time_ms(0.0);
    rules_fired
}
```
It (a) iterates `action_nodes × join_nodes × nagrams` — there is no condition evaluation, no fact matching; (b) never reads `working_facts` to decide whether a rule's `Condition` holds; (c) never applies `Rule.actions` to `working_facts` — `facts_after` could never reflect `score := score + 10`; (d) records `CoverageTracking::record_rule_fire` once per nagram (the double-counting bug, Pitfall 6).

**`RuleSet::compile`** (`engine/src/rules.rs:285`) is a stub: it calls `validate_all()` then sets `self.compiled = true` — the comment says "Here we would build the Rete network and compile rules." It does not build any network. There is no `RuleSet → ReteNetwork` compiler anywhere in the codebase.

**`Condition::evaluate`** (`rules.rs:155`) is also simplified: `Compare` only handles `Integer`/`Float`/`String` equality via `to_value()` (which only resolves `True`/`False` to a `Value`), and `Pattern` unconditionally returns `true` (`rules.rs:176-179`). Full pattern matching against working memory is not implemented. `WorkingMemory::match_pattern` (`fact.rs:237`) returns `Vec::new()`.

**Decision: Option (b) — extend the trait, implement a direct evaluation loop.** The three options posed were (a) call `EngineRegistry::create_engine_by_name` then inject a `RuleSet` via some existing method, (b) extend the trait with `load_ruleset`, (c) bridge through `environment::Engine`. Option (a) is impossible — no existing method accepts a `RuleSet`. Option (c) routes through the stub (`load_rules`/`execute_forward` are no-ops) and would require implementing those stubs anyway, inheriting the raw-pointer `Session` design; reject it. Option (b) is the cleanest and is required.

Concretely, Phase 23 adds to the `ExecutionEngine` trait (`registry.rs:64`):
```rust
/// Loads rules into the engine. Default: unsupported.
fn load_ruleset(&mut self, _rules: &crate::rules::RuleSet) -> Result<(), String> {
    Err("load_ruleset not supported by this engine".into())
}
/// Returns the post-execution facts for serialization. Default: empty.
fn get_facts(&self) -> Vec<crate::fact::Fact> {
    Vec::new()
}
```
Then implements on `ReteEngine`:
- `load_ruleset`: store `RuleSet` (or `Vec<Rule>`) inside `ReteEngine` as a new field `rules: Vec<Rule>` (the simplest honest bridge — do NOT attempt to compile a `ReteNetwork` from the ruleset given the network types' placeholder state). Optionally also populate `network.action_nodes` with one `ActionNode` per rule (so existing coverage/`rules_not_fired` plumbing keyed on `rule_id` still resolves), but drive firing from the stored rules, not from network propagation.
- `execute_rules` rewrite: for each rule in `self.rules`, evaluate `rule.condition` against `self.working_facts` (extend `Condition::evaluate` to handle `Pattern` by matching `fact_type` + `field` against facts of that type via `working_facts.values()`), collect fired rule ids into a `HashSet<String>` (once-per-rule — fixes Pitfall 6), apply each fired rule's `Action`s to `working_facts` (`InsertFact`/`UpdateFact`/`RetractFact` mapping to the existing `WorkingMemory`-style ops, but on `ReteEngine::working_facts`), then after the loop record `CoverageTracking::record_rule_fire` once per fired rule id and `record_rule_eval` once per evaluated rule. Return `rules_fired = fired_set.len()`.
- `get_facts`: `self.working_facts.values().cloned().collect()`.

This is a direct interpreter, not Rete network propagation. It is the minimal honest path because the existing Rete network code (`TestNode`/`JoinNode` with `predecessor: "placeholder"`, `execute_rules` counting loop) is itself too incomplete to drive. The planner should treat "make `execute_rules` actually evaluate + act" as **in-scope for Phase 23**, not a follow-up — success criterion #1 requires it. A full Rete network compiler can be a future enhancement that swaps in behind the same `load_ruleset`/`get_facts` trait methods without changing the API contract.

**The exact method the executor will call:** `registry.create_engine_by_name(&name)` (`registry.rs:177`) → `Option<Result<Box<dyn ExecutionEngine>, String>>`; then `engine.load_ruleset(&rule_set)?`; then for each request fact `engine.insert_fact(fact)?` (trait, `registry.rs:69`); then `engine.execute()` (`registry.rs:66`) inside `spawn_blocking`; then `engine.get_facts()` for `facts_after`. `rules_not_fired` = rules in the ruleset whose ids are not in the fired `HashSet`.

## Research Flag 3: inference_trace Population Source

`ExecuteResponse.inference_trace: Option<InferenceTrace>` (`execution.rs:44`) is always `None` today (`execution.rs:341`). The API `InferenceTrace` struct (`execution.rs:49`) is a different type from `engine::environment::InferenceTrace` (`environment.rs:235`) and from the chaining `InferenceTraceCollector` (`engine/src/chaining/mode_selector.rs:135`). `ReteEngine::execute_rules` emits no trace. `EngineBuilder::enable_debug_trace(true)` (`environment.rs:53`) configures the stub `environment::Engine`, not `ReteEngine` — and the stub never produces a trace either.

**Decision: Phase 23 leaves `inference_trace: None` (default off).** The field is already plumbed as `Option`, so no structural work is needed. Populating it would require instrumenting `ReteEngine::execute_rules` to emit `InferenceTraceEntry` per fired rule (timestamp, event_type="rule_fired", rule_name, data). That is real work and belongs with EXEC-04 (Phase 28), which the traceability table assigns `inference_trace` to. An alternative — deriving a minimal trace from the fired-rule `HashSet` + `rules_not_fired` — is possible but adds no value over the `rules_fired`/`rules_not_fired` fields already present; defer until Phase 28 when the frontend test-runner consumes it. Do NOT wire `InferenceTraceCollector` from the chaining code — it is coupled to the stub `environment::Engine` surface and would re-entangle Phase 23 with the surface it is trying to leave.

## Research Flag 4: Per-Request Compile Cost

`ReteEngine::new()` (`rete/mod.rs:210`) is O(1): four `HashMap::new()`/`Default::default()` calls and an empty `ReteNetwork`. `ReteEngine::with_network()` (`rete/mod.rs:221`) takes a pre-built network but is never called. Under the Option (b) design, `load_ruleset` will be O(rules) — a linear pass to clone `Rule`s into `self.rules` and optionally insert `ActionNode`s. For any realistic ruleset (tens to low-hundreds of rules) this is sub-millisecond — dominated by the HTTP I/O the API already accepts.

**Clone analysis (Pitfall 9):** `ReteEngine` is `!Clone` — it has `#[derive(Debug)]` only (`rete/mod.rs:200`); `Box<dyn ExecutionEngine>` is `!Clone` by trait-object rule. `environment::Engine` derives `Clone` (`environment.rs:78`) but is the stub surface; cloning it copies the `RuleSet` (cheap) and empty `WorkingMemory`, but Phase 23 does not route through `environment::Engine` for execution, so its `Clone` is irrelevant. There is no "compiled network" to save — the network is empty until `load_ruleset` populates it, and even then the per-request work is rule evaluation, not network compilation.

**Recommendation: ship per-request construction, no pool.** The cost is `ReteEngine::new()` (O(1)) + `load_ruleset` (O(rules)) per request inside `spawn_blocking`. A pool keyed by `(engine_name, ruleset_version)` would add eviction/invalidation complexity (every reload invalidates all pooled engines) and break the existing batch "no shared locks between requests" contract (`execution.rs:304-311` docstring). Only revisit if a future milestone makes `load_ruleset` parse real rule-file text (currently deferred to Phase 25's shared `RuleSet` snapshot — and even then parsing happens once per reload, not once per request). This matches `ARCHITECTURE.md` §1's recommendation and the `PITFALLS.md` Pitfall 7 conclusion.

## Implementation Approach

Ordered, concrete steps. Each ends with a verifiable check.

1. **Fix the import bug (BLOCKER).** In `engine/src/engine/{rete,treat,leaps,phreak}/mod.rs` change `use engine::monitoring::coverage::CoverageTracking;` → `use crate::monitoring::coverage::CoverageTracking;` (lines 13/12/13/12). Verify: `cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` compiles clean.
2. **Prep rename (Pitfall 10).** Rename `api/src/metrics.rs::EngineState` (`metrics.rs:12`) → `MetricsSnapshot`. Update `MetricsTx` alias (`metrics.rs:22`), `create_metrics_channel` (`metrics.rs:25`), the WS broadcast site (`main.rs:210` comment), and any imports. This is a pure rename — no logic change — done before adding execute logic so the two `EngineState` types (execution.rs:75 vs metrics.rs:12) don't collide in review. Verify: `cargo check -p api`.
3. **Extend `register_default_engines`** (`registry.rs:158`) with three `#[cfg(feature = "engine-X")]` blocks registering `TreatEngineFactory::new()`, `LeapsEngineFactory::new()`, `PhreakEngineFactory::new()`. Extend the re-export at `registry.rs:209` so the factory types are nameable. Verify: a unit test asserting `available_engines()` contains all four names when all features are on.
4. **Enable engine features on the `api` crate.** `api/Cargo.toml:11` → `engine = { path = "../engine", features = ["engine-rete", "engine-treat", "engine-leaps", "engine-phreak"] }`. Verify: `cargo check -p api`.
5. **Extend the `ExecutionEngine` trait** (`registry.rs:64`) with `load_ruleset` and `get_facts` default methods (see Flag 2 sketch). Implement both on `ReteEngine` (store `Vec<Rule>`; `get_facts` returns `working_facts.values().cloned().collect()`). Implement on the other three engines as a follow-on if Phase 23 promises them via the `engine` param — minimally `ReteEngine` is required for success criterion #1; the other three can return `Err` from `load_ruleset` initially (the `engine` param would then 500 for non-rete until their impls land, OR Phase 23 implements all four — recommend at least Rete + stub-the-rest with a clear 400/501 for non-rete until verified). Verify: `cargo test -p engine`.
6. **Rewrite `ReteEngine::execute_rules`** (`rete/mod.rs:321`) to actually fire. For each rule: evaluate `rule.condition` against `working_facts` (extend `Condition::evaluate` for `Pattern` — match `fact_type` against `working_facts.values().filter(|f| f.fact_type_id == fact_type)`, then check `field` value against `value` with the `ComparisonOp`); collect fired rule ids in a `HashSet<String>`; apply each fired rule's `Action`s (`InsertFact`/`UpdateFact`/`RetractFact`) to `working_facts`. After the loop, call `CoverageTracking::record_rule_eval` once per evaluated rule and `CoverageTracking::record_rule_fire` once per fired rule (outside any inner nagram loop — fixes Pitfall 6). Return `fired_set.len()`. Note: the existing `execute_rules` is the `ExecutionEngine::execute` path (`rete/mod.rs:351`), so this one rewrite serves both. Verify: a unit test inserting a fact + a rule with `Condition::Pattern` matching it → `execute()` returns `rules_fired >= 1` and `get_facts()` shows the action's effect.
7. **Add `engine: Option<String>` to `ExecuteRequest`** (`execution.rs:16`, serde default None). Add `engine_name: String` to `ExecuteResponse` (or keep as-is; the ROADMAP adds `execution_id`+`engine_generation` in Phase 24 — Phase 23 only needs the `engine` input).
8. **Build + share `EngineRegistry` and `EngineSelector` as `web::Data` in `main.rs`.** At `main.rs:32` (alongside `engine_state`): `let mut registry = EngineRegistry::new(); registry.register_default_engines(); let registry = web::Data::new(registry);` and `let selector = web::Data::new(EngineSelector::with_defaults());`. Read the config default from `RRULES_DEFAULT_ENGINE` env var (`std::env::var("RRULES_DEFAULT_ENGINE")`), defaulting to `"rete"`; if set, call `selector`'s current-engine setter at startup (note `EngineSelector::select_engine` takes `&mut self` at `selector.rs:98` — so set it once at construction, then share; per-request validation should use a `&self`-style check against `available_engines()` rather than calling `select_engine`, which would require a `Mutex`). Inject both via `.app_data(...)` at `main.rs:114`. Verify: `cargo check -p api`.
9. **Rewrite `execute_single`** (`execution.rs:312`) to accept `web::Data<EngineRegistry>`, `web::Data<EngineSelector>` (and a shared `RuleSet` source — for Phase 23 use an empty `RuleSet::new()` initially, OR load from `RULE_STORAGE_PATH` if Phase 25's shared snapshot is deferred; the success criterion requires a real rule, so Phase 23 needs at least one rule available — recommend building a minimal `RuleSet` from the rules directory at startup, shared via `web::Data<Arc<RwLock<RuleSet>>>`, with reload handled in Phase 25). Resolution: if `req.engine` present, use it; else use `selector.current_engine()`. Validate the name is in `registry.available_engines()`; if not, return `HttpResponse::BadRequest()` (HTTP 400). Build the engine via `registry.create_engine_by_name(&name)`, `engine.load_ruleset(&rule_set)?`, convert each request fact via `json_to_engine_value` (`execution.rs:533`) into a `Fact` and `engine.insert_fact(fact)`, then `web::block(move || engine.execute())` (or `tokio::task::spawn_blocking`) since `execute(&mut self)` is CPU-bound. Serialize `facts_after` from `engine.get_facts()` via a reverse `engine_value_to_json` mapping (mirror of `json_to_engine_value` — `Value::Integer` → json number, `Value::String` → json string, etc.), grouped by `fact_type_id`. Build `rules_not_fired` from the ruleset's rule ids minus the fired set (the engine must expose the fired ids — extend `EngineResult` or have `execute_single` compute it by re-reading coverage; simplest: have `ReteEngine` expose `fired_rule_ids(&self) -> Vec<String>` via a new trait method or via `get_metrics`). Keep `inference_trace: None`. Verify: success criteria #1–#3.
10. **Fix the `facts_after` echo bug (Pitfall 11).** Remove `facts_after.insert(fact_type.clone(), fact_value.clone())` (`execution.rs:327`); build `facts_after` only from `engine.get_facts()` post-execution. Verify: a `score := score + 10` rule shows the incremented `score`.
11. **Update the `execute_rules` handler signature** (`execution.rs:355`) to pass through the new `web::Data` args; `batch_execute_rules` (`execution.rs:374`) must clone the `web::Data` handles into each `JoinSet` task (they're `Arc` under the hood, so `.clone()` is cheap) and pass them to `execute_single` — preserving the "no shared locks between requests" contract (each task still builds its own engine instance from the shared factory). Verify: batch still runs concurrently.

## Validation Architecture

`config.json` has `nyquist_validation: true`, so this section applies.

**Test framework:** Rust built-in `#[test]` via `cargo test`; `actix-test` (`api/Cargo.toml:48`) for HTTP integration tests; `serial_test` (workspace dev-dep, `engine/Cargo.toml:21`) for `CoverageTracking` singleton isolation.

| Req ID | Behavior | Test type | Command | File exists? |
|--------|----------|-----------|---------|--------------|
| EXEC-01 | `/v1/execute` fires real rules, truthful `rules_fired` (not hardcoded 0) | integration | `cargo test -p api --test execute_integration -- execute_fires_real_rule -x` | ❌ Wave 0 |
| EXEC-02 | `facts_after` reflects post-inference working memory (score incremented) | integration | `cargo test -p api --test execute_integration -- facts_after_not_echoed -x` | ❌ Wave 0 |
| ENGINE-07 | `engine: "rete"` succeeds; `engine: "unknown"` → HTTP 400 | integration | `cargo test -p api --test execute_integration -- engine_param_validation -x` | ❌ Wave 0 |
| ENGINE-08 | omitting `engine` resolves to config default | integration | `cargo test -p api --test execute_integration -- default_engine_resolution -x` | ❌ Wave 0 |
| MON-05 | `CoverageTracking` auto-populated; `record_*` once per rule per execution | unit + integration (serial) | `cargo test -p engine --test coverage -- record_once_per_rule -x` and `cargo test -p api --test execute_integration -- coverage_auto_populated -x` | ❌ Wave 0 |
| (bridge) | `ReteEngine::load_ruleset` + `execute` evaluates conditions + applies actions | unit | `cargo test -p engine --test rete_execution -- fires_on_matching_fact -x` | ❌ Wave 0 |
| (bridge) | `ReteEngine::get_facts` returns post-execution facts | unit | `cargo test -p engine --test rete_execution -- get_facts_after_execute -x` | ❌ Wave 0 |
| (compile) | `cargo check` with all four engine features compiles | build | `cargo check -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` | n/a |

**Per-task commit:** `cargo check -p api && cargo test -p engine --lib -x` (fast).
**Per-wave merge:** `cargo test --workspace` (full suite; use `--features engine-rete,engine-treat,engine-leaps,engine-phreak` for the api crate — add to workspace test invocation or rely on api/Cargo.toml features).
**Phase gate:** full suite green before `/gsd-verify-work`; the four ROADMAP success criteria below must pass.

**Wave 0 gaps:**
- `api/tests/execute_integration.rs` — covers EXEC-01/02, ENGINE-07/08, MON-05 integration (POST /v1/execute against a `score := score + 10` rule → `rules_fired >= 1`, `facts_after.score` incremented; GET /v1/metrics/coverage shows `fired == true`).
- `engine/tests/rete_execution.rs` — covers `load_ruleset`/`execute`/`get_facts` unit behavior.
- `engine/tests/coverage.rs` (or extend) — `serial_test`-gated test that `record_rule_fire` is called exactly once per fired rule per execution (Pitfall 6 regression).
- A fixture: a ruleset with one rule `r1` matching `fact_type: "scorecard", field: "score"` and action `UpdateFact` adding 10 — loaded by the integration test (either a temp rules dir or constructed in-test via `RuleSet::add_rule`).

**ROADMAP success criteria → test map:**
1. `score := score + 10` → `rules_fired >= 1` + `facts_after` incremented → `execute_fires_real_rule` + `facts_after_not_echoed`.
2. `engine: "rete"` ok; `engine: "unknown"` → 400 → `engine_param_validation`.
3. omitting `engine` → default executes → `default_engine_resolution`.
4. `GET /v1/metrics/coverage` shows `fired == true` after a run → `coverage_auto_populated`.

## Risks & Open Questions

1. **Scope of `execute_rules` rewrite.** This research concludes `ReteEngine::execute_rules` must be rewritten to actually evaluate conditions and apply actions (not just wiring). This is more work than the ROADMAP's "wiring core" framing implies. The planner must decide: (a) do the full rewrite in Phase 23 (required to meet success criterion #1), or (b) descope success criterion #1 to Phase 25 and ship Phase 23 as pure registry wiring with an empty ruleset (`rules_fired: 0` from a real engine, not hardcoded). Recommendation: (a) — the milestone's promise is live execution, and the rewrite is unavoidable somewhere in v5.0; doing it in Phase 23 keeps the vertical slice honest. Flag for human confirmation at plan review.
2. **Non-Rete engines' `load_ruleset` support.** `TreatEngine`/`LeapsEngine`/`PhreakEngine` also need `load_ruleset` impls if the `engine` param should accept them. Phase 23 can either implement all four or return `Err` (→ HTTP 500) for non-rete. The success criteria only test `"rete"`, so stubbing the other three is acceptable for Phase 23; a follow-on phase implements them. Recommendation: register all four factories (so `available_engines()` is truthful) but only implement `load_ruleset` on `ReteEngine` in Phase 23; have `execute_single` return a clear 501/400 if a non-rete engine is selected before its impl lands. Planner to confirm.
3. **`EngineSelector` sharing vs `&mut self`.** `EngineSelector::select_engine` takes `&mut self` (`selector.rs:98`), so it cannot be called on a shared `web::Data<EngineSelector>` per request. Phase 23 should set the default engine once at startup (mutably, before wrapping in `web::Data`) and use a `&self` validation against `available_engines()` (`selector.rs:87`) for per-request `engine` param checks. Alternatively wrap the selector in `Mutex` — unnecessary if per-request only reads. Planner to pick.
4. **Shared `RuleSet` source.** Success criterion #1 needs a real rule loaded. Phase 23 must build a `RuleSet` at startup (from `RULE_STORAGE_PATH` or a test fixture) and share it (e.g. `web::Data<Arc<RwLock<RuleSet>>>`). The full reload-then-swap is Phase 25; Phase 23 only needs a read-only shared snapshot built once at startup. Planner to decide whether to load from the rules directory now or inject a test fixture for the integration test and defer directory loading to Phase 25.
5. **`rules_not_fired` source.** The `EngineResult` (`registry.rs:93`) carries only `rules_fired` (count), not the fired ids. Phase 23 must either extend `EngineResult` with `fired_rule_ids: Vec<String>` or add a trait method `fired_rule_ids(&self) -> Vec<String>` read after `execute()`. Planner to pick; recommend extending `EngineResult` (single return value, no extra trait method).
6. **`Condition::evaluate` for `Pattern`.** The current impl returns `true` for all `Pattern` conditions (`rules.rs:176`). Phase 23 must implement real pattern matching (fact_type + field + value + op) for success criterion #1 to be meaningful. This is part of the `execute_rules` rewrite (step 6).

## Sources

**Codebase (HIGH confidence, direct reads):**
- `api/src/v1/execution.rs` — stub `execute_single` (312-343), `ExecuteRequest` (16-23), `ExecuteResponse` (35-45), `EngineState` (75-78), `json_to_engine_value` (533-558), `execute_rules` handler (355), `batch_execute_rules` (374-395), `execute_with_session` stub (561).
- `api/src/main.rs` — app state wiring (32, 114-124), `RULE_STORAGE_PATH` env (48).
- `api/src/metrics.rs` — metrics `EngineState` (12-19), `MetricsTx` (22), `dead_rules_endpoint` fresh `RuleExecutionTracker::new()` (214), `coverage_endpoint` reads singleton (233).
- `engine/src/engine/registry.rs` — `ExecutionEngine` trait (64-90), `EngineRegistry` (139-200), `register_default_engines` (158-167, Rete-only), `create_engine_by_name` (177), re-export (209-210).
- `engine/src/engine/selector.rs` — `EngineSelector` (59), `with_defaults` (77), `select_engine(&mut self)` (98), `available_engines(&self)` (87).
- `engine/src/engine/mod.rs` — feature gating (30-39).
- `engine/src/engine/rete/mod.rs` — import bug (13), `ReteEngine` (200-218), `with_network` (221, never called), `assert_fact` (232), `execute_rules` placeholder (321-341, no condition eval / no action exec), `ReteEngineFactory` (392-415).
- `engine/src/engine/treat/mod.rs` — import bug (12), `TreatEngineFactory` (459-488).
- `engine/src/engine/leaps/mod.rs` — import bug (13), `LeapsEngineFactory` (473-496).
- `engine/src/engine/phreak/mod.rs` — import bug (12), `PhreakEngineFactory` (674-697).
- `engine/src/environment.rs` — stub `Engine` (78-93, derives Clone), `load_rules` stub (122), `execute_forward` stub (132), `Session` stub (152-208).
- `engine/src/rules.rs` — `Rule` (16-31), `Condition` (117-140), `Condition::evaluate` simplified (155-181, `Pattern` returns true), `Action` (198-215), `RuleSet` (228-298), `compile` stub (285-292).
- `engine/src/fact.rs` — `Fact` (14-23), `WorkingMemory` (141-149), `insert_fact`/`update_fact`/`retract_fact` (169-216), `get_facts_by_type` (224), `match_pattern` stub (237).
- `engine/src/data_types.rs` — `Value` (16-33).
- `engine/src/monitoring/coverage.rs` — `CoverageTracking` singleton `LazyLock<Mutex>` (86-87), `record_rule_eval` (97), `record_rule_fire` (105), `to_json` (167).
- `engine/src/monitoring/detection.rs` — `RuleExecutionTracker` plain struct (64, NOT singleton), `record_fire` (92), `find_dead_rules` (137).
- `engine/src/chaining/mode_selector.rs` — `InferenceTraceCollector` (135, tied to stub chaining path).
- `engine/Cargo.toml` (29-36), `api/Cargo.toml` (11), workspace.
- `cargo check` runs: `--features engine-rete` → E0433 (rete:13); all four features → 4× E0433.

**Planning docs (HIGH confidence):**
- `.planning/research/PITFALLS.md` — Pitfalls 1, 6, 7, 9, 10, 11 (full context).
- `.planning/research/SUMMARY.md` — Phase 1 roadmap framing.
- `.planning/research/ARCHITECTURE.md` — two-engine-surface finding, §1–§2, build order.
- `.planning/ROADMAP.md` — Phase 23 goal, deliverables, success criteria.
- `.planning/REQUIREMENTS.md` — EXEC-01/02, ENGINE-07/08, MON-05 → Phase 23.

## Metadata

**Confidence breakdown:**
- Standard stack / cargo features: HIGH — read all four `mod.rs` factory impls and both `Cargo.toml`s; verified the import bug by `cargo check`.
- Architecture (bridging): HIGH — read the trait, `ReteEngine`, `RuleSet`, `Condition`/`Action` in full; the absence of `load_ruleset` and the placeholder nature of `execute_rules` are confirmed at file:line.
- Pitfalls: HIGH — all pitfalls re-verified against current code (line numbers match the PITFALLS.md citations modulo the few drifts noted above).

**Research date:** 2026-07-02
**Valid until:** 2026-08-01 (stable — internal codebase; no external library versions in play)