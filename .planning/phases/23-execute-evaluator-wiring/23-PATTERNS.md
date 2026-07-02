# Phase 23: Execute evaluator + wiring - Pattern Map

**Mapped:** 2026-07-02
**Files analyzed:** 13 new/modified files (8 source + 3 test + 2 config)
**Analogs found:** 13 / 13 (every file has a real codebase analog — this is a brownfield wiring phase, no greenfield)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `engine/src/engine/rete/mod.rs` | engine (service) | transform (direct interpreter) | same file — `ReteEngine::execute_rules` placeholder + `ReteEngineFactory` | exact (self-modification, factory analog for other 3) |
| `engine/src/engine/treat/mod.rs` | engine (service) | transform | `engine/src/engine/rete/mod.rs` | exact (mirror) |
| `engine/src/engine/leaps/mod.rs` | engine (service) | transform | `engine/src/engine/rete/mod.rs` | exact (mirror) |
| `engine/src/engine/phreak/mod.rs` | engine (service) | transform | `engine/src/engine/rete/mod.rs` | exact (mirror) |
| `engine/src/engine/registry.rs` | trait/registry (config) | CRUD | same file — `register_default_engines` + `ExecutionEngine` trait + re-export | exact (self-modification) |
| `engine/src/rules.rs` | model | transform | same file — `Condition::evaluate` + `Action` enum + `RuleSet` | exact (self-modification) |
| `engine/src/fact.rs` | model | CRUD | same file — `WorkingMemory::insert_fact`/`update_fact`/`retract_fact` + `get_facts_by_type` | exact (self-modification) |
| `engine/src/monitoring/coverage.rs` | utility (singleton) | event-driven | same file — `CoverageTracking::record_rule_eval`/`record_rule_fire` | exact (self-modification, read-only use) |
| `api/src/v1/execution.rs` | controller (handler) | request-response | same file — `execute_single` stub + `json_to_engine_value` + `batch_execute_rules` + `reload_rules` | exact (self-modification) |
| `api/src/main.rs` | config (wiring) | request-response | same file — `engine_state` `web::Data` + `RULE_STORAGE_PATH` env + `.app_data(...)` chain | exact (self-modification) |
| `api/src/metrics.rs` | utility (rename) | request-response | same file — `EngineState` struct + `MetricsTx` alias + `create_metrics_channel` | exact (self-rename) |
| `api/Cargo.toml` | config | n/a | `engine/Cargo.toml` `[features]` block | role-match |
| `api/tests/execute_integration.rs` | test (integration) | request-response | `api/tests/batch_execution_integration.rs` | exact (role + data flow) |
| `engine/tests/rete_execution.rs` | test (unit) | transform | `engine/src/engine/rete/mod.rs` `#[cfg(test)] mod tests` + `engine/tests/snapshot_integration.rs` | role-match |
| `engine/tests/coverage.rs` | test (unit, serial) | event-driven | `engine/src/monitoring/coverage.rs` `#[cfg(test)] mod tests` | exact |

## Pattern Assignments

### `engine/src/engine/rete/mod.rs` (engine, transform — direct interpreter)

**Analog:** self — `ReteEngine::execute_rules` placeholder (`rete/mod.rs:321-341`) is the rewrite target; `ReteEngineFactory` (`rete/mod.rs:407-415`) is the analog the other three engines mirror.

**Import-bug fix (L-01)** — line 13, change:
```rust
// BROKEN (absolute path doesn't resolve inside the `engine` crate, 2021 edition)
use engine::monitoring::coverage::CoverageTracking;
// FIX
use crate::monitoring::coverage::CoverageTracking;
```

**Imports pattern** (lines 8-13) — note `crate::` paths, `HashMap`/`HashSet` from std:
```rust
use crate::engine::registry::{EngineFactory, EngineResult, EngineType, ExecutionEngine};
use crate::engine::metrics::MetricsCollector;
use crate::fact::Fact;
use std::collections::{HashMap, HashSet};
```

**Core pattern — the placeholder to rewrite** (`rete/mod.rs:321-341`). The new direct interpreter must: iterate `self.rules` (new field per L-02), evaluate `rule.condition` against `self.working_facts`, collect fired ids in a `HashSet<String>` (once-per-rule, fixes Pitfall 6), apply `rule.actions` to `working_facts`, then call `CoverageTracking::record_rule_*` once per rule AFTER the loop (not inside the nagram inner loop as the placeholder does):
```rust
pub fn execute_rules(&mut self) -> usize {
    let mut rules_fired = 0;
    for (_node_id, action_node) in &self.network.action_nodes {
        for (_join_id, _join_node) in &self.network.join_nodes {
            let nagrams = self.beta_memory.get_nagrams(&_join_node.id);
            for _nagram in &nagrams {
                CoverageTracking::record_rule_fire(&action_node.rule_id); // BUG: per-nagram double-count
                rules_fired += 1;
                self.metrics.record_rule_fired();
            }
        }
    }
    self.metrics.set_execution_time_ms(0.0);
    rules_fired
}
```

**Action-application analog** — apply `Action::InsertFact`/`UpdateFact`/`RetractFact` to `self.working_facts: HashMap<FactId, Fact>` (`rete/mod.rs:204`) by mirroring `WorkingMemory::insert_fact`/`update_fact`/`retract_fact` (`fact.rs:169-216`, excerpted below in the `fact.rs` section). The `assert_fact` method (`rete/mod.rs:232-252`) already shows the insert-into-`working_facts` idiom: `self.working_facts.insert(fact_id.clone(), fact)`.

**Trait impl analog** (`rete/mod.rs:350-388`) — the `impl ExecutionEngine for ReteEngine` block is where `load_ruleset` + `get_facts` (L-02) get added. `execute` (`:351`) delegates to `execute_rules`; `insert_fact` (`:356-359`) delegates to `assert_fact` and returns `Ok(())`. `get_facts` impl: `self.working_facts.values().cloned().collect()`.

**Factory analog** (`rete/mod.rs:407-415`) — the other three factories already mirror this exactly; `register_default_engines` just needs to register them (see `registry.rs` section):
```rust
impl EngineFactory for ReteEngineFactory {
    fn engine_type(&self) -> EngineType { EngineType::Rete }
    fn create_engine(&self) -> Result<Box<dyn ExecutionEngine>, String> {
        Ok(Box::new(ReteEngine::new()))
    }
}
```

**Fixpoint cap (D-07)** — `ReteEngine::new()` (`rete/mod.rs:210-218`) is the O(1) per-request constructor; the fixpoint loop wraps the new `execute_rules` body with a `for _ in 0..max_iter { if no_new_fires { break } }` guard. Read the cap from an env var (see `main.rs` section, `RRULES_STORAGE_PATH` env pattern).

---

### `engine/src/engine/treat/mod.rs` / `leaps/mod.rs` / `phreak/mod.rs` (engine, transform)

**Analog:** `engine/src/engine/rete/mod.rs` (above). All three already mirror Rete's structure exactly — verified by grep:

| Engine | import-bug line | `working_facts` field | `execute_rules` line | `impl ExecutionEngine` | `impl EngineFactory` | `create_engine` |
|--------|-----------------|-----------------------|----------------------|------------------------|----------------------|-----------------|
| rete   | 13              | 204                   | 321                  | 350                    | 407                 | 413 |
| treat  | 12              | 176                   | 357                  | 413                    | 480                 | 486 |
| leaps  | 13              | 216                   | 358                  | 431                    | 488                 | 494 |
| phreak | 12              | 424                   | 540                  | 631                    | 689                 | 695 |

**Import-bug fix** — identical one-line change in each (`use engine::monitoring::coverage::CoverageTracking;` → `use crate::monitoring::coverage::CoverageTracking;`).

**Direct-evaluator (D-01/D-04)** — the planner may either factor a shared helper (engine-agnostic — operates on `self.rules` + `self.working_facts`) or mirror the Rete rewrite per-engine. The treat `execute_rules` (`treat/mod.rs:357-404`) shows the same placeholder counting loop with the same per-nagram double-count bug (line 395) — same fix. The `TreatEngine`/`LeapsEngine`/`PhreakEngine` `impl ExecutionEngine` blocks already implement `execute`/`insert_fact`/`reset` (analog: `rete/mod.rs:350-388`); add `load_ruleset` + `get_facts` to each.

---

### `engine/src/engine/registry.rs` (trait + registry, CRUD)

**Analog:** self — `register_default_engines` (`:158-167`), `ExecutionEngine` trait (`:64-90`), re-export (`:209-210`).

**Trait extension (L-02)** — add `load_ruleset` + `get_facts` as default methods, mirroring the existing `set_config` default-impl idiom (`registry.rs:87-89`):
```rust
// Existing default-impl pattern to mirror:
fn set_config(&mut self, _key: &str, _value: &str) -> Result<(), String> {
    Ok(())
}
```
New methods (per RESEARCH Flag 2 sketch):
```rust
fn load_ruleset(&mut self, _rules: &crate::rules::RuleSet) -> Result<(), String> {
    Err("load_ruleset not supported by this engine".into())
}
fn get_facts(&self) -> Vec<crate::fact::Fact> {
    Vec::new()
}
```

**Extend `EngineResult` (RESEARCH Open Q#5)** — `EngineResult` (`registry.rs:93-101`) currently has `success`/`rules_fired`/`message`. Add `pub fired_rule_ids: Vec<String>` and update both constructors (`success` `:105`, `failure` `:114`) to initialize it (empty in `failure`; the engine populates in `success`). The planner should confirm via `EngineResult::success` callers — `rete/mod.rs:353` is the only one (`EngineResult::success(fired)`).

**`register_default_engines` extension (D-01)** — current Rete-only block (`registry.rs:158-167`):
```rust
pub fn register_default_engines(&mut self) {
    #[cfg(feature = "engine-rete")]
    { self.register_factory(Box::new(ReteEngineFactory::new())); }
    #[cfg(not(feature = "engine-rete"))]
    { /* No engines available without features */ }
}
```
Add three mirror blocks under their cfg features:
```rust
#[cfg(feature = "engine-treat")]
{ self.register_factory(Box::new(TreatEngineFactory::new())); }
#[cfg(feature = "engine-leaps")]
{ self.register_factory(Box::new(LeapsEngineFactory::new())); }
#[cfg(feature = "engine-phreak")]
{ self.register_factory(Box::new(PhreakEngineFactory::new())); }
```

**Re-export extension** — current (`registry.rs:208-210`):
```rust
#[cfg(feature = "engine-rete")]
pub use crate::engine::rete::{ReteEngine, ReteEngineFactory};
```
Add three parallel re-exports so the factory types are nameable from `register_default_engines`:
```rust
#[cfg(feature = "engine-treat")]
pub use crate::engine::treat::TreatEngineFactory;
#[cfg(feature = "engine-leaps")]
pub use crate::engine::leaps::LeapsEngineFactory;
#[cfg(feature = "engine-phreak")]
pub use crate::engine::phreak::PhreakEngineFactory;
```
(Module gating in `engine/src/engine/mod.rs:30-39` already exposes `treat`/`leaps`/`phreak` under the right features — no change needed there.)

**`create_engine_by_name`** (`registry.rs:177-186`) — already maps `"rete"`/`"treat"`/`"leaps"`/`"phreak"` → `EngineType`; **no change needed**. `available_engines()` (`:189-194`) returns registered factory keys — automatically truthful once `register_default_engines` registers all four.

---

### `engine/src/rules.rs` (model, transform)

**Analog:** self — `Condition::evaluate` (`:153-181`), `Action` enum (`:197-215`), `RuleSet` (`:228-298`).

**Condition extension (D-05)** — current impl only handles `Eq` on `Integer`/`Float`/`String`; `Pattern` unconditionally returns `true` (`rules.rs:176-179`):
```rust
Condition::Pattern { .. } => {
    // Full pattern matching implemented in Rete integration
    true
}
```
Rewrite `Condition::evaluate` to: (a) handle full `ComparisonOp` enum (`Eq`/`Ne`/`Lt`/`Le`/`Gt`/`Ge`, defined `:143-151`) across `Integer`/`Float`/`String`/`Boolean` (use `DynamicCast` helpers from `data_types.rs:109-158` — `as_integer`/`as_float`/`as_string`/`as_bool` for cross-type numeric compares); (b) implement real `Pattern` matching: filter `working_facts.values()` by `fact_type_id == fact_type`, then compare `fact.payload.get(field)` to `value` with the (new) `Pattern`-carried `op`. **Note:** the current `Pattern` struct (`rules.rs:135-139`) has no `op` field — D-05 needs to add `op: ComparisonOp` to `Condition::Pattern` (or default to `Eq` and add `op` later; planner picks). The signature `evaluate(&self, _facts: &[&Fact])` (`:155`) is the existing contract — the rewrite passes the engine's `working_facts` slice.

**Expression type (D-06 — research correction)** — `Action::UpdateFact` (`rules.rs:205-208`) currently accepts only literal `Value`s:
```rust
UpdateFact { id: String, fields: HashMap<String, Value> },
```
`score := score + 10` is **not representable**. Introduce an `Expression` enum (arithmetic `+ - * /`, string concat, comparisons, nested, field-read `FieldRef { fact_type, field }`) and extend `UpdateFact` to `fields: HashMap<String, Expression>` (or a new variant `UpdateFactExpr` to keep backward compat). Evaluate against `working_facts` in the engine's action-application step. The `Value` enum (`data_types.rs:16-33`) is the leaf type. `Action::CallFunction` (`rules.rs:212`) already exists and wires in D-06's "wire existing `CallFunction`" requirement.

**RuleSet** (`rules.rs:228-298`) — `add_rule` (`:245`), `validate_all` (`:267`), `compile` stub (`:285`). Phase 23 uses `RuleSet` as a read-only shared snapshot (`Arc<RwLock<RuleSet>>`); no `compile` change needed (compile stays a stub — the direct interpreter bypasses the network compiler).

---

### `engine/src/fact.rs` (model, CRUD)

**Analog:** self — `WorkingMemory` (`:138-149`) and its CRUD methods. `Fact` (`:13-23`) with `id: Uuid`, `fact_type_id: String`, `payload: HashMap<String, Value>`.

**Action-application analog for the engine** — the `ReteEngine.working_facts: HashMap<FactId, Fact>` uses `String` FactId ( rete's `FactId = String` at `rete/mod.rs:16`), while `WorkingMemory.facts` uses `Uuid` (`fact.rs:143`). The methods to mirror on `ReteEngine`:
```rust
// fact.rs:169-183 — insert
pub fn insert_fact(&mut self, fact: Fact) -> Result<(), WorkingMemoryError> {
    if fact.payload.is_empty() && fact.fact_type_id != "generic" {
        return Err(WorkingMemoryError::EmptyPayload);
    }
    if self.facts.contains_key(&fact.id) { return Err(WorkingMemoryError::DuplicateId(fact.id)); }
    self.index.add(&fact);
    self.facts.insert(fact.id, fact);
    Ok(())
}
// fact.rs:186-205 — update
pub fn update_fact(&mut self, id: Uuid, updates: HashMap<String, Value>) -> Result<(), WorkingMemoryError> {
    let fact = self.facts.get_mut(&id).ok_or(WorkingMemoryError::FactNotFound(id))?;
    for (field, value) in updates { fact.payload.insert(field, value); }
    self.index.remove(fact); self.index.add(fact);
    Ok(())
}
// fact.rs:208-216 — retract
pub fn retract_fact(&mut self, id: Uuid) -> Result<Option<Fact>, WorkingMemoryError> {
    match self.facts.remove(&id) { Some(fact) => { self.index.remove(&fact); Ok(Some(fact)) } None => Ok(None) }
}
// fact.rs:224-229 — get by type (the Pattern-match filter idiom)
pub fn get_facts_by_type(&self, type_id: &str) -> Vec<&Fact> {
    let ids = self.index.get_by_type(type_id);
    ids.into_iter().filter_map(|id| self.facts.get(&id)).collect()
}
```
**`match_pattern` stub** (`fact.rs:237-240`) returns `Vec::new()` — D-05 implements real pattern matching either here or directly in the engine's evaluator (RESEARCH Open Q#6 — planner picks; recommended: in the engine, since `WorkingMemory` is the stub surface Phase 23 is leaving).

---

### `engine/src/monitoring/coverage.rs` (utility, event-driven — read-only use)

**Analog:** self — `CoverageTracking` singleton (`:86-110`). Phase 23 records into this existing singleton (no new singleton — CONTEXT.md `Established Patterns`).

**Singleton pattern** (`coverage.rs:83-93`):
```rust
use std::sync::LazyLock;
static INSTANCE: LazyLock<Mutex<TrackingState>> = LazyLock::new(|| {
    Mutex::new(TrackingState { rules: HashMap::new() })
});
pub struct CoverageTracking;
```

**Once-per-rule recording (L-06 / Pitfall 6)** — the methods to call (do NOT call inside the nagram inner loop — call once per rule id outside):
```rust
// coverage.rs:97-102
pub fn record_rule_eval(rule_id: impl Into<String>) {
    let mut state = INSTANCE.lock().unwrap();
    let entry = state.rules.entry(rule_id.into()).or_default();
    entry.evaluated = true;
    entry.evaluation_count += 1;
}
// coverage.rs:105-110
pub fn record_rule_fire(rule_id: impl Into<String>) {
    let mut state = INSTANCE.lock().unwrap();
    let entry = state.rules.entry(rule_id.into()).or_default();
    entry.fired = true;
    entry.fire_count += 1;
}
```
The fix: collect fired ids in a `HashSet<String>` during the evaluate loop, then after the loop call `record_rule_fire(id)` once per id in the set. `record_rule_eval(id)` once per evaluated rule (whether fired or not).

**`clear()`** (`coverage.rs:161-164`) — used by tests for singleton isolation; pair with `serial_test` (see test section).

---

### `api/src/v1/execution.rs` (controller, request-response)

**Analog:** self — `execute_single` stub (`:312-343`), `json_to_engine_value` (`:533-558`), `batch_execute_rules` (`:374-430`), `reload_rules` (`:444-480`).

**Handler signature + `web::Data` analog** — `reload_rules` (`execution.rs:444-447`) is the in-file analog for accepting `web::Data<T>` on an execution handler:
```rust
pub async fn reload_rules(
    req: web::Json<ReloadRequest>,
    engine_state: web::Data<EngineState>,
) -> impl Responder { ... }
```
Apply the same shape to `execute_rules` (`:355`) and `batch_execute_rules` (`:374`): add `registry: web::Data<EngineRegistry>`, `selector: web::Data<EngineSelector>`, `ruleset: web::Data<Arc<RwLock<RuleSet>>>`.

**`execute_single` rewrite target (D-08 / Pitfall 11)** — current stub (`execution.rs:312-343`). The echo bug is line 327:
```rust
facts_after.insert(fact_type.clone(), fact_value.clone()); // BUG: echoes request input
```
and `rules_fired = 0` is hardcoded (`:331`). Rewrite to: resolve engine name (from `req.engine` or selector default), validate via `registry.available_engines()` (400 on unknown — L-07), `create_engine_by_name` → `load_ruleset` → `insert_fact` per request fact (convert via `json_to_engine_value`) → `web::block(move || engine.execute())` → serialize `facts_after` from `engine.get_facts()` via the reverse `engine_value_to_json`.

**`json_to_engine_value` analog for the reverse mapping** (`execution.rs:533-558`) — mirror it inverted:
```rust
fn json_to_engine_value(value: serde_json::Value) -> engine::Value {
    match value {
        serde_json::Value::Null => engine::Value::Null,
        serde_json::Value::Bool(b) => engine::Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { engine::Value::Integer(i) }
            else if let Some(f) = n.as_f64() { engine::Value::Float(f) }
            else { engine::Value::Null }
        }
        serde_json::Value::String(s) => engine::Value::String(s),
        serde_json::Value::Array(arr) => engine::Value::Array(arr.into_iter().map(json_to_engine_value).collect()),
        serde_json::Value::Object(obj) => {
            let map = obj.into_iter().map(|(k, v)| (k, json_to_engine_value(v))).collect();
            engine::Value::Object(map)
        }
    }
}
```
The new `engine_value_to_json(Value) -> serde_json::Value` inverts each arm: `Value::Integer(i)` → `json!(i)`, `Value::Float(f)` → `json!(f)`, `Value::Bool(b)` → `json!(b)`, `Value::String(s)` → `json!(s)`, `Value::Null` → `json!(null)`, `Value::Array`/`Object` recurse, `Value::DateTime(dt)` → `json!(dt.to_string())`. Group `facts_after` by `fact_type_id` into the existing `HashMap<String, serde_json::Value>` shape (`ExecuteResponse.facts_after`, `execution.rs:42`).

**`spawn_blocking` for CPU-bound engine work** — `execute(&mut self)` is sync. Use `web::block(move || engine.execute()).await` (actix-web's ergonomic wrapper around `tokio::task::spawn_blocking`). The batch path analog for concurrent CPU-bound work is `batch_execute_rules`'s `JoinSet` + `Semaphore` (`execution.rs:379-395`):
```rust
let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));
let mut join_set = tokio::task::JoinSet::new();
for request in requests {
    let permit_sem = semaphore.clone();
    join_set.spawn(async move {
        let _permit = permit_sem.acquire_owned().await.expect("batch semaphore closed unexpectedly");
        execute_single(request).await
    });
}
```
The batch extension (CONTEXT integration point `execution.rs:374`) clones the new `web::Data` handles (Arc under the hood, cheap) into each `JoinSet` task — preserves the "no shared locks between requests" contract (`execution.rs:304-311` docstring). Each task still builds its own `Box<dyn ExecutionEngine>` via `registry.create_engine_by_name`.

**`ExecuteRequest` extension** (`execution.rs:15-23`) — add `engine: Option<String>` with serde default None, mirroring the existing `#[serde(skip_serializing_if = "Option::is_none")]` idiom used on `session_id`/`rule_filter`.

**Error handling pattern** — `reload_rules` (`execution.rs:471-477`) shows the 500-on-error idiom: `HttpResponse::InternalServerError().json(serde_json::json!({ ... }))`. For 400 on unknown engine (L-07): `HttpResponse::BadRequest().json(serde_json::json!({ "error": format!("unknown engine: {}", name) }))`.

---

### `api/src/main.rs` (config, request-response — wiring)

**Analog:** self — `engine_state` `web::Data` (`:32`), `RULE_STORAGE_PATH` env (`:48`), `.app_data(...)` chain (`:114-124`).

**App-state wiring pattern** (`main.rs:32-43`) — inject `EngineRegistry` + `EngineSelector` + shared `RuleSet` alongside the existing stores:
```rust
let engine_state = web::Data::new(v1::execution::EngineState::new());
// ... existing stores ...
let job_store = v1::execution::init_job_store();
```
New (D-03):
```rust
let mut registry = engine::engine::EngineRegistry::new();
registry.register_default_engines();
let registry = web::Data::new(registry);

let mut selector = engine::engine::EngineSelector::with_defaults();
// Set default engine ONCE at startup (select_engine takes &mut self — selector.rs:98)
let default_engine = std::env::var("RRULES_DEFAULT_ENGINE").unwrap_or_else(|_| "rete".to_string());
if selector.select_engine(&default_engine).is_err() {
    tracing::warn!(engine = %default_engine, "Invalid RRULES_DEFAULT_ENGINE, falling back to rete");
    let _ = selector.select_engine("rete");
}
let selector = web::Data::new(selector);

// D-03: shared read-only ruleset snapshot (reload-then-swap is Phase 25)
let ruleset = build_ruleset_from_dir(&rules_dir); // empty + warn on missing dir (D-03a)
let ruleset = web::Data::new(std::sync::Arc::new(std::sync::RwLock::new(ruleset)));
```

**Env-var reading analog** (`main.rs:48`):
```rust
let rules_dir = std::env::var("RULE_STORAGE_PATH").unwrap_or_else(|_| "./rules".to_string());
```
Mirror for `RRULES_DEFAULT_ENGINE` (above) and `RRULES_MAX_CHAIN_ITERATIONS` (D-07, parse to `usize`, default ~1000).

**`.app_data(...)` injection** (`main.rs:114-124`) — append the three new handles inside the `HttpServer::new` closure (they clone per worker):
```rust
.app_data(engine_state.clone())
.app_data(web::Data::new(metrics_tx.clone()))
// ... existing ...
.app_data(web::Data::new(job_store.clone()))
.app_data(web::Data::new(user_store.clone()))
.app_data(registry.clone())            // NEW
.app_data(selector.clone())            // NEW
.app_data(ruleset.clone())             // NEW
```

---

### `api/src/metrics.rs` (utility, request-response — pure rename, L-05)

**Analog:** self — `EngineState` (`:12-19`), `MetricsTx` alias (`:22`), `create_metrics_channel` (`:25`).

**Rename `EngineState` → `MetricsSnapshot`** (Pitfall 10 — disambiguate from `execution.rs:75` `EngineState`):
```rust
// metrics.rs:12-19 — current
#[derive(Debug, Clone, Serialize)]
pub struct EngineState { /* fields */ }
// metrics.rs:22
pub type MetricsTx = Arc<tokio::sync::broadcast::Sender<EngineState>>;
// metrics.rs:25
pub fn create_metrics_channel(capacity: usize) -> (MetricsTx, tokio::sync::broadcast::Receiver<EngineState>) { ... }
```
Rename all three sites + the WS broadcast comment at `main.rs:210-214` + any imports (`main.rs:35` `create_metrics_channel` call site is generic, no type-name change needed there). Pure rename — **no logic change**. Verify with `cargo check -p api`.

---

### `api/Cargo.toml` (config)

**Analog:** `engine/Cargo.toml:29-36` `[features]` block (engine-rete/treat/leaps/phreak).

**Change** (`api/Cargo.toml:11`):
```toml
# current
engine = { path = "../engine" }
# Phase 23
engine = { path = "../engine", features = ["engine-rete", "engine-treat", "engine-leaps", "engine-phreak"] }
```
The `engine-treat`/`-leaps`/`-phreak` features each pull `engine-rete` (`engine/Cargo.toml:34-36`), so listing all four is explicit and safe. Verify: `cargo check -p api`.

---

### `api/tests/execute_integration.rs` (test, integration — Wave 0)

**Analog:** `api/tests/batch_execution_integration.rs` — same actix-web `test::init_service` + `TestRequest` + `json_body` helper pattern.

**Test scaffold to copy** (`batch_execution_integration.rs:14-52`):
```rust
use actix_web::{body::BoxBody, test, web, App, HttpResponse, Responder};
use serde_json::{json, Value};
use api::v1::execution::{execute_rules, batch_execute_rules};

async fn json_body(resp: actix_web::dev::ServiceResponse<BoxBody>) -> Value {
    assert!(resp.status().is_success(), "expected success, got {}", resp.status());
    let bytes = test::read_body(resp).await;
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("invalid JSON: {e}; body={}", String::from_utf8_lossy(&bytes)))
}
```
**Difference from analog** — Phase 23 handlers now take `web::Data<EngineRegistry>` + `web::Data<EngineSelector>` + `web::Data<Arc<RwLock<RuleSet>>>`, so `init_service` must `.app_data(...)` a real registry (`register_default_engines()`), a `with_defaults()` selector, and a `RuleSet` containing the `score := score + 10` fixture rule (built in-test via `RuleSet::add_rule`, `rules.rs:245`). The analog's `App::new().configure(configure_test_routes)` becomes `App::new().app_data(registry).app_data(selector).app_data(ruleset).configure(configure_test_routes)`.

**Test cases to cover** (RESEARCH Validation Architecture):
- `execute_fires_real_rule` (EXEC-01): POST `/v1/execute` with `facts: {scorecard: {score: 0}}` → `rules_fired >= 1`.
- `facts_after_not_echoed` (EXEC-02 / Pitfall 11): `facts_after.scorecard.score == 10` (incremented), not `0` (echoed).
- `engine_param_validation` (ENGINE-07): `engine: "rete"` → 200; `engine: "unknown"` → 400.
- `default_engine_resolution` (ENGINE-08): omit `engine` → resolves to `RRULES_DEFAULT_ENGINE` (set to `"rete"` in test env).
- `coverage_auto_populated` (MON-05): after a run, `GET /v1/metrics/coverage` shows the rule `fired == true`.

---

### `engine/tests/rete_execution.rs` (test, unit — Wave 0)

**Analog:** `engine/src/engine/rete/mod.rs` `#[cfg(test)] mod tests` (`:417-485`) for the engine-construction/fact-assert idiom; `engine/tests/snapshot_integration.rs` for the integration-test file shape.

**Engine unit-test idiom** (`rete/mod.rs:451-466`):
```rust
let mut engine = ReteEngine::new();
let person_node = TestNode::new("test-person", "person");
engine.network.add_test_node(person_node);
let mut fact = Fact::new("person", HashMap::new());
fact.payload.insert("name".to_string(), crate::data_types::Value::String("Alice".to_string()));
engine.assert_fact(fact);
```
Phase 23 unit tests use `ReteEngine::new()` → `load_ruleset(&RuleSet)` → `insert_fact(Fact)` → `execute()` → assert `EngineResult.rules_fired >= 1` and `get_facts()` shows the action's effect. Build the fixture rule via `Rule::builder(...).condition(...).action(...).build()` (`rules.rs:60-110`).

---

### `engine/tests/coverage.rs` (test, unit + serial — Wave 0)

**Analog:** `engine/src/monitoring/coverage.rs` `#[cfg(test)] mod tests` (`:235-353`) — shows the `tracker.clear()` + `record_rule_eval`/`record_rule_fire` + `get_all_states`/`get_summary` idiom.

**Singleton-isolation pattern** (`coverage.rs:286-297`):
```rust
#[test]
fn test_coverage_tracking_singleton() {
    let tracker = CoverageTracking;
    tracker.clear();
    CoverageTracking::record_rule_eval("rule-1");
    let states = tracker.get_all_states();
    assert_eq!(states.len(), 2);
}
```
Phase 23 adds a `serial_test`-gated regression for Pitfall 6: after one `execute()` that fires rule `r1`, `CoverageTracking.get_rule_state("r1").fire_count == 1` (not `> 1` from per-nagram double-count). Use `#[serial_test::serial]` (workspace dev-dep, `engine/Cargo.toml:21`) so concurrent tests don't race the singleton. `clear()` before each test.

## Shared Patterns

### `web::Data<T>` shared state + `spawn_blocking` for CPU-bound work
**Source:** `api/src/v1/execution.rs:374-395` (`JoinSet` + `Semaphore`), `api/src/v1/execution.rs:444-447` (`web::Data<EngineState>` handler arg), `api/src/main.rs:32-124` (`.app_data(...)` chain)
**Apply to:** `api/src/v1/execution.rs` (`execute_single`, `execute_rules`, `batch_execute_rules`), `api/src/main.rs`
```rust
// Handler accepts web::Data; web::Data is Arc under the hood so .clone() into batch tasks is cheap
pub async fn execute_rules(
    req: web::Json<ExecuteRequest>,
    registry: web::Data<EngineRegistry>,
    selector: web::Data<EngineSelector>,
    ruleset: web::Data<Arc<RwLock<RuleSet>>>,
) -> impl Responder { ... }
// CPU-bound engine.execute() inside spawn_blocking
let result = web::block(move || engine.execute()).await.map_err(|e| ...)?;
```

### Env-var config with sensible default
**Source:** `api/src/main.rs:48`
**Apply to:** `api/src/main.rs` (`RRULES_DEFAULT_ENGINE`, `RRULES_MAX_CHAIN_ITERATIONS`)
```rust
let value = std::env::var("ENV_VAR_NAME").unwrap_or_else(|_| "default".to_string());
```

### Error handling — HTTP status + JSON body
**Source:** `api/src/v1/execution.rs:471-477`
**Apply to:** `api/src/v1/execution.rs` (`execute_single` 400 on unknown engine, 500 on engine error)
```rust
HttpResponse::BadRequest().json(serde_json::json!({ "error": format!("unknown engine: {}", name) }))
HttpResponse::InternalServerError().json(serde_json::json!({ "error": e.to_string() }))
```

### Singleton tracker via `LazyLock<Mutex<…>>`
**Source:** `engine/src/monitoring/coverage.rs:83-93`
**Apply to:** `engine/src/engine/{rete,treat,leaps,phreak}/mod.rs` (record into `CoverageTracking` — do NOT introduce a new singleton; `RuleExecutionTracker` singleton-ization is Phase 26)
```rust
static INSTANCE: LazyLock<Mutex<TrackingState>> = LazyLock::new(|| Mutex::new(TrackingState { rules: HashMap::new() }));
// call site: CoverageTracking::record_rule_fire(&rule_id); // once per fired rule id, OUTSIDE the nagram loop
```

### Trait default-method idiom (for extending `ExecutionEngine`)
**Source:** `engine/src/engine/registry.rs:87-89` (`set_config` default impl)
**Apply to:** `engine/src/engine/registry.rs` (`load_ruleset` + `get_facts` default impls)
```rust
fn method(&mut self, _args...) -> Result<(), String> { Ok(()) }  // or Err("not supported".into())
```

### `#[cfg(feature = "engine-X")]` factory registration + re-export
**Source:** `engine/src/engine/registry.rs:158-167` + `:208-210`
**Apply to:** `engine/src/engine/registry.rs` (three new blocks for treat/leaps/phreak)
```rust
#[cfg(feature = "engine-X")]
{ self.register_factory(Box::new(XEngineFactory::new())); }
// + re-export:
#[cfg(feature = "engine-X")]
pub use crate::engine::X::XEngineFactory;
```

### Integration test scaffold (actix-web)
**Source:** `api/tests/batch_execution_integration.rs:14-52`
**Apply to:** `api/tests/execute_integration.rs`
```rust
use actix_web::{body::BoxBody, test, web, App, HttpResponse, Responder};
use serde_json::{json, Value};
async fn json_body(resp: actix_web::dev::ServiceResponse<BoxBody>) -> Value { /* assert + parse */ }
#[actix_web::test]
async fn test_name() {
    let app = test::init_service(App::new().app_data(...).configure(configure_test_routes)).await;
    let req = test::TestRequest::post().uri("/v1/execute").set_json(json!({...})).to_request();
    let body = json_body(test::call_service(&app, req).await).await;
    assert_eq!(body["rules_fired"], ...);
}
```

## No Analog Found

None. Every file in this phase is a brownfield modification of an existing file or a new test file with a direct same-role analog. The two genuinely new constructs — the `Expression` type (D-06) and the reverse `engine_value_to_json` mapping — have no codebase precedent for the type itself, but the planner can derive `Expression` from the existing `Condition` AST shape (`rules.rs:117-140`) and the reverse mapping from `json_to_engine_value` (`execution.rs:533-558`) as quoted above. The direct-evaluator loop has no precedent (the existing `execute_rules` is a counting placeholder), but RESEARCH Flag 2 / Implementation Approach step 6 specifies it concretely and the action-application step mirrors `WorkingMemory`'s CRUD methods (`fact.rs:169-216`).

## Metadata

**Analog search scope:** `engine/src/engine/{rete,treat,leaps,phreak}/mod.rs`, `engine/src/engine/{registry,selector,mod}.rs`, `engine/src/{rules,fact,data_types,lib}.rs`, `engine/src/monitoring/coverage.rs`, `api/src/{main,metrics}.rs`, `api/src/v1/execution.rs`, `api/tests/batch_execution_integration.rs`, `engine/tests/snapshot_integration.rs`, `engine/Cargo.toml`, `api/Cargo.toml`.
**Files scanned:** 18
**Pattern extraction date:** 2026-07-02
**Rust edition:** 2021 (verified — `engine/Cargo.toml:4`, `api/Cargo.toml:4`)