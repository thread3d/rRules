# Rule engine

The `engine` crate exposes **one** boxed-trait execution surface (`ExecutionEngine`,
`engine/src/engine/registry.rs:64-117`), **four** registered algorithm names
(`EngineType::{Rete, Treat, Leaps, Phreak}`, `engine/src/engine/registry.rs:10-19`) and
**one** shared evaluate-and-apply routine (`direct_eval::direct_execute_with_limit`,
`engine/src/engine/direct_eval.rs:81-224`) that **all four** engines delegate to
(`engine/src/engine/rete/mod.rs:374-402`, `engine/src/engine/treat/mod.rs:388-410`,
`engine/src/engine/leaps/mod.rs:394-416`, `engine/src/engine/phreak/mod.rs:578-600`).
Rete used to inline its own copy; the copies had already diverged (no agenda-group
locking, no deterministic fact resolution) and it now shares the evaluator. All four
ship separate network structures (alpha/beta
memory, join nodes, caches, aggregation nodes, truth maintenance) that fact insertion
populates but rule execution never consults. The four names therefore select a metrics
identity plus a set of bookkeeping structures, not four matching strategies.

Module gating (`engine/src/engine/mod.rs:30-44`): `rete` behind `engine-rete`, `treat`
behind `any(engine-treat, engine-leaps)`, `leaps` behind `engine-leaps`, `phreak` behind
`engine-phreak`; `direct_eval`, `metrics`, `registry`, `selector` are always compiled.

## Execution semantics

The evaluator is a **direct interpreter**: every pass re-evaluates every loaded rule's
`Condition` against the whole working-memory set
(`engine/src/engine/direct_eval.rs:100-133`; Rete twin at
`engine/src/engine/rete/mod.rs:406-441`).

1. **Resolve the fixpoint cap** from `RRULES_MAX_CHAIN_ITERATIONS`, unparseable/absent ⇒
   `1000`, read per `execute()` call (`engine/src/engine/direct_eval.rs:73-76`,
   `engine/src/engine/rete/mod.rs:378-381`). The API only logs a warning for a bad value
   (`api/src/app.rs:125-129`).
2. **Loop `for _pass in 0..max_iter`** (`engine/src/engine/direct_eval.rs:96`); `max_iter
   == 0` means nothing is evaluated and `rules_fired == 0`.
3. **Rule order** is `Vec` order, i.e. the order `load_ruleset` cloned
   (`engine/src/engine/rete/mod.rs:630-633`). No salience/timestamp/name sort anywhere.
4. **Skip already-fired rules** by id (`engine/src/engine/direct_eval.rs:102-104`).
5. **Evaluate** against `working_facts.values()`; the id enters `evaluated_set` *before*
   evaluation, so "evaluated" means "condition was computed"
   (`engine/src/engine/direct_eval.rs:106-108`).
6. **Fired test**: `rule.condition.evaluate(&facts)` (`engine/src/rules.rs:168-229`).
   `Pattern { fact_type, field, value, op }` scans the facts of that type and is true when any
   one of them carries a field satisfying `op` against `value` — the operator is part of the
   pattern, so `amount > 100` and `amount = 100` are the same shape (`engine/src/rules.rs:196-212`).
   A pattern whose fact type is absent, or whose field is missing, is `false` (the missing field
   compares as `Null` through `compare_values`), so only `Condition::True` (`:170`) fires on empty
   working memory. Actions that compute values take an `Expression` rather than a `Value`
   (`engine/src/rules.rs:241-260`), which is what makes `score := score + 10` representable.
7. **Trace, if enabled**: push `TraceEvent{event_type:"rule_fired", timestamp: now}` and add
   `facts.len()` to `facts_matched` (`engine/src/engine/direct_eval.rs:115-122`) — the sum of
   working-memory size per firing, not a match count.
8. **Apply actions**: clone `rule.actions`, then `apply_action` against live working memory
   (`engine/src/engine/direct_eval.rs:126-129`).
9. **Mark fired**, set `newly_fired_this_pass`; Rete additionally calls
   `metrics.record_rule_fired()` here (`engine/src/engine/rete/mod.rs:437-439`). "Fired"
   means the condition returned `true` — even when no action changed anything (an
   `UpdateFact` with an absent target is a silent return,
   `engine/src/engine/direct_eval.rs:211-214`).
10. **Intra-pass visibility**: writes are immediate, so a later rule in the same pass sees
    them; there is no activation snapshot and no agenda
    (`engine/src/engine/direct_eval.rs:100-133`).
11. **Terminate the pass** when nothing newly fired (`engine/src/engine/direct_eval.rs:134-136`);
    with step 4 this bounds productive passes at `|rules|`, the env cap being the outer net.
12. **Record coverage once per rule, after the loop**: `CoverageTracking::record_rule_eval`
    / `record_rule_fire` plus `RuleExecutionTracker::record_evaluation` / `record_fire`
    (`engine/src/engine/direct_eval.rs:150-159`; duplicated for Rete at
    `engine/src/engine/rete/mod.rs:457-466`).
13. **Assemble** `EngineResult::success(fired_set.len())`, set
    `fired_rule_ids` from HashSet iteration and attach `trace`
    (`engine/src/engine/treat/mod.rs:404-409`, `engine/src/engine/rete/mod.rs:488-491`).

### `EngineResult`

| Field | Source | Notes |
|---|---|---|
| `success` | always `true` (`engine/src/engine/registry.rs:183-191`) | `EngineResult::failure` (`:194-202`) is unreachable from `execute()`; no evaluator error is surfaced. |
| `rules_fired` | `fired_set.len()` | distinct rules fired this call, `≤ |rules|`. |
| `message` | `"Executed {n} rules"` (`engine/src/engine/registry.rs:187`) | |
| `fired_rule_ids` | HashSet iteration | explicitly unordered (`engine/src/engine/registry.rs:165-172`). |
| `trace` | `Some` only if `collect_trace && !events.is_empty()` (`engine/src/engine/direct_eval.rs:165-176`) | `events` *is* in firing order (`engine/src/engine/registry.rs:141-146`); duration covers the loop only (`:86`, `166-168`). |

`set_collect_trace` (`engine/src/engine/registry.rs:91-98`) defaults to `false` in all four
engines (`engine/src/engine/rete/mod.rs:243`, `treat/mod.rs:197`, `leaps/mod.rs:240`,
`phreak/mod.rs:446`); when false no `Vec`, `String` or `chrono` call happens
(`engine/src/engine/direct_eval.rs:86-88`, `165`).

### `rules_not_fired`

Computed in the API, not the engine: loaded rule ids minus
`EngineResult.fired_rule_ids` (`api/src/v1/execution.rs:790-800`). The stateless path takes
ids from the filtered load (`api/src/v1/execution.rs:654-681`), so `rule_filter`-excluded
rules never appear; the sessionful path takes ids from the unfiltered shared ruleset
(`api/src/v1/execution.rs:1037-1054`). Operationally it is: evaluated rules whose
condition never returned true, plus rules never evaluated because the pass cap ran out.

### Action semantics

`apply_action` exists twice with identical bodies
(`engine/src/engine/direct_eval.rs:193-247`, `engine/src/engine/rete/mod.rs:507-562`):

| Action | Behaviour |
|---|---|
| `UpdateFact { id, fields }` | Target = exact `fact.id` key, else first fact whose `fact_type_id == id`; none ⇒ silent return (`engine/src/engine/direct_eval.rs:203-214`). Field expressions evaluate against a **clone** of working memory taken at action start (`:196-199`) and are written into the payload (`:219-222`). |
| `InsertFact` | `Fact::new` ⇒ new UUID (`engine/src/fact.rs:27-34`); inserted into `working_facts` only — no alpha/beta memory, no metric (`engine/src/engine/direct_eval.rs:224-227`). |
| `RetractFact` | Removes by exact key, else first `fact_type_id` match; no memory cleanup, no metric (`engine/src/engine/direct_eval.rs:228-240`). |
| `CallFunction`, `LogMessage` | No-ops (`engine/src/engine/direct_eval.rs:241-246`). |

## Engine registry

| Trait member | Default | Overridden by |
|---|---|---|
| `execute` | required (`registry.rs:66`) | all four → own `execute_rules` (`rete/mod.rs:571-573`, `treat/mod.rs:420-422`, `leaps/mod.rs:450-452`, `phreak/mod.rs:641-643`) |
| `insert_fact` | required (`:69`) | all four → `assert_fact` |
| `retract_fact -> Result<bool,String>` | required (`:72`) | Rete/TREAT/LEAPS real; PHREAK stub (`engine/src/engine/phreak/mod.rs:650-653`) |
| `run -> usize` | required (`:75`) | all four: `execute_rules().rules_fired` |
| `get_metrics`, `engine_type`, `reset` | required (`:78`, `:81`, `:84`) | all four; `reset` also drops loaded rules (`engine/src/engine/rete/mod.rs:607-615`) |
| `set_config` | `Ok(())` no-op (`:87-89`) | nobody |
| `set_collect_trace` | no-op (`:98`) | all four store a flag |
| `load_ruleset` | `Err("load_ruleset not supported by this engine")` (`:106-108`) | all four clone `ruleset.rules` (`engine/src/engine/rete/mod.rs:630-633` and siblings) |
| `get_facts` | empty `Vec` (`:114-116`) | all four clone `working_facts` |

`ExecutionEngine: Send + Sync + Debug` (`engine/src/engine/registry.rs:64`) but no engine is
`Clone`; the API builds a fresh engine per request (`api/src/v1/execution.rs:618-640`) and per
session (`api/src/v1/sessions.rs:221-231`).

**Naming.** `EngineType::name()` gives the lowercase name (`registry.rs:23-30`);
`all_variants()` lists all four unconditionally (`:33-40`). `create_engine_by_name`
lowercases and maps `"rete"|"treat"|"leaps"|"phreak"`; **anything else returns `None`**
(`:276-285`), and `create_engine` returns `None` for an unregistered factory (`:269-273`).
`available_engines()` is HashMap-ordered (`:288-293`); the API sorts it for `/v1/engines`
(`api/src/v1/execution.rs:1142-1151`). `EngineSelector` keeps `current = "rete"`
(`engine/src/engine/selector.rs:67-74`), `with_defaults()` advertises all four names
**regardless of features** (`:77-79`), `select_engine` checks that list then a hardcoded
match (`:98-116`), and `from_yaml`/`from_json` always return `Err` (`:127-144`).

| Env var | Read at | Default | Effect |
|---|---|---|---|
| `RRULES_DEFAULT_ENGINE` | per request (`api/src/v1/execution.rs:598-602`), per session create (`api/src/v1/sessions.rs:196-208`), startup (`api/src/app.rs:105-116`) | `"rete"` (session path) / `selector.current_engine()` (stateless) | Engine used when the request omits `engine`. |
| `RRULES_MAX_CHAIN_ITERATIONS` | per `execute()` (`engine/src/engine/direct_eval.rs:73`, `rete/mod.rs:378`) | `1000` | Fixpoint pass cap. |
| `RRULES_SESSION_*`, `RRULES_MAX_SESSIONS` | `api/src/v1/sessions.rs:451-465` | — | Session lifecycle only. |

**Unknown engine name**: `available_engines()` membership is checked first and failure
returns `400 "unknown engine: {name}"` (`api/src/v1/execution.rs:606-613`,
`api/src/v1/sessions.rs:210-216`); a registered-but-uncreatable engine is `500`
(`api/src/v1/execution.rs:626-631`); `None` from `create_engine_by_name` maps to the same
400 (`:632-637`). Sessions pin the engine at creation and reject a conflicting `engine` on
resume with 400 (`api/src/v1/sessions.rs:171-190`). Startup *warns and falls back* to `rete`
for an invalid `RRULES_DEFAULT_ENGINE` (`api/src/app.rs:117-123`), but per-request resolution
re-reads the raw env value (`api/src/v1/execution.rs:600-602`), so a bad value still fails
every engine-omitting request with 400.

| Feature | Gates | Declared |
|---|---|---|
| `engine-rete` | `pub mod rete` (`engine/src/engine/mod.rs:30-31`), `ReteEngine`/`ReteEngineFactory` re-export (`registry.rs:308-309`), `metrics` re-export (`mod.rs:47-48`) | `engine/Cargo.toml:64` |
| `engine-treat` | registration of `TreatEngineFactory` (`registry.rs:250-253`) | `engine/Cargo.toml:66` (`= ["engine-rete"]`) |
| `engine-leaps` | `pub mod leaps` (`mod.rs:36-37`); alone it *also* compiles `treat` via `any(engine-treat, engine-leaps)` (`mod.rs:34-35`) | `engine/Cargo.toml:67` (`= ["engine-rete"]`) |
| `engine-phreak` | `pub mod phreak` (`mod.rs:38-39`) | `engine/Cargo.toml:68` (`= ["engine-rete"]`) |
| ~~`history`~~ | deleted in 9.0.30 with `coverage/`; both went in 9.0.58 | — |

The crate default is exactly those four engine features
(`engine/Cargo.toml:58-63`: `default = ["engine-rete","engine-treat","engine-leaps","engine-phreak"]`)
and the API pins the same four (`api/Cargo.toml:17`). Behaviour that **requires all four**:

* `register_default_engines` registering four factories with `available_engines()` returning
  `["rete","treat","leaps","phreak"]` (`registry.rs:245-266`, contract documented `:239-244`).
* `test_register_default_engines_advertises_all_four`, itself gated on all four
  (`registry.rs:351-380`).
* `POST /v1/execute` (stateless and sessionful) accepting `engine: treat|leaps|phreak`,
  because validation is `available_engines().contains(...)`
  (`api/src/v1/execution.rs:606-613`).
* `GET /v1/engines` listing four names, and per-engine Prometheus labels
  (`api/src/v1/execution.rs:1120-1151`, `:732-741`).

With `--no-default-features` no factory registers and every name resolves to `None`
(`registry.rs:262-266`) while `EngineSelector` still advertises four
(`engine/src/engine/selector.rs:77-79`).

## Algorithm implementations

Shared by all four: `HashMap<String, Fact>` working memory keyed by `fact.id.to_string()`,
the cloned rules vector, `SimpleMetricsCollector`, and the semantics above.

### Rete (`engine/src/engine/rete/mod.rs`)

| Structure | Lines | Reality |
|---|---|---|
| `AlphaMemory` | `47-86` | Real insert/remove/get/size; `remove` used by retraction (`:587`). |
| `BetaMemory` | `88-141` | Real; `remove_fact` drops every nagram mentioning the id and prunes empty buckets (`:120-127`). |
| `MemoryReference`/`Nagram`/`NodeId` | `22-38` | `generation` is always written `0` (`:342`, `:351`). |
| `TestNode`, `JoinNode`, `ActionNode` | `144-193` | Node records only: `JoinNode` is built with `NodeId("placeholder")` predecessors (`:173-174`) and no join condition is ever evaluated; `ActionNode`s are stored (`:219-221`) but nothing propagates into them. |
| `assert_fact` | `274-294` | Stores the fact, inserts into alpha memory **only if a matching test node exists** (`:282-291`). |
| `join_propagation` | `315-358` | Appends `alpha × existing-nagram` products and singletons with no dedup and no predicate (`:337-355`); nagram count grows quadratically. |
| `execute_rules` | `374-402` | Delegates to the shared evaluator; the network is never consulted by execution. |
| metrics | `293`, `357`, `439`, `596-601` | `alpha_memory_size`/`beta_memory_size` are overwritten from live memory at read time (`:598-599`). |

There is no token propagation, no join predicate, no agenda, no negative/aggregate node: the
network is a data-collection harness and matching happens in `Condition::evaluate`.

### TREAT (`engine/src/engine/treat/mod.rs`)

Reuses `AlphaMemory`/`BetaMemory`/`NodeId` from `crate::engine::rete` (`:11`).

| Structure | Lines | Reality |
|---|---|---|
| `LRUCache` / `CacheStats` | `31-139` | `HashMap<String, Nagram>` + `max_size`/`current_size`; eviction sorts keys and drops the lexicographically first (`:66-73`) — not LRU; `stats()` hardcodes `hit_count: 0, miss_count: 0` (`:110-117`) so `hit_rate()` is always `0.0`; `CacheEntry` (`:22-27`) and `get_oldest_entry` (`:82-86`) are dead. |
| `TreatNetwork` | `142-170` | `base: ReteNetwork` + cache + `cached_nodes`; `add_cached_join_node` has no production caller. |
| `propagate_to_successors` | `258-291` | Cache key is per node (`format!("join_{}", successor)`, `:267`), so a hit short-circuits propagation for a *different* fact (`:275-279`); exactly one `record_cache_miss` per test node after the loop (`:289-290`). |
| `join_propagation_with_cache` | `294-374` | Hit ⇒ clone cached nagram + append each alpha fact (`:311-324`); miss ⇒ cache one representative nagram (`:351-371`). |
| `execute_rules` | `388-410` | Delegates to `direct_execute`, then re-adds `record_rule_fired()` per fired rule (`:398-400`); the engine also keeps a second hit/miss counter pair (`:191-192`, `:452-453`) beside the metrics collector. |
| `TreatEngineFactory::with_cache_size` | `507-510` | Ignores its argument (`let _ = max_cache_size;`). |

### LEAPS (`engine/src/engine/leaps/mod.rs`)

| Structure | Lines | Reality |
|---|---|---|
| `AggregationOp` (`Sum`/`Count`/`Max`/`Min`) | `21-91` | Real `compute` over `&[f64]` (`:44-55`) and `compute_from_facts` over `Integer`/`Float` fields (`:58-84`); empty ⇒ `None`. |
| `AggregationNode` | `95-186` | ~~Tracks `source_facts` + `current_value`; `recalculate()` always returns `None`, so `current_value` never becomes `Some`.~~ **Closed in 9.0.55**: the node keeps each contributing fact's *value* (`source_values`) and recomputes on every add and removal, so `current_value` is real; `execute_aggregations` rewrites the cached values from the facts the engine holds now. |
| `LeapsNetwork` + `assert_fact` | `188-222`, `271-300` | `base: ReteNetwork` + `agg_nodes` + `aggregated_join_nodes`; insertion feeds the fact id to **every** aggregation node regardless of type/field (`:293-297`). |
| `join_propagation_with_predecessor` | `330-364` | Looks alpha facts up by `predecessor_id.0` as if it were a fact type (`:338`), whereas Rete uses the predecessor test node's `fact_type_id` (`rete/mod.rs:326`). |
| `execute_aggregations` | `367-380` | No-op: computes into a local `_value` and discards it (`:375-378`). |
| `compute_aggregation` | `426-440` | Real, but scans `working_facts` by `fact_type_id`, independent of the network. |
| metrics pollution | `322-324` | `record_rule_fired()` on join traversal when the join is marked aggregated. |
| `execute_rules` | `394-416` | Delegates to `direct_execute`, then `record_rule_fired()` per fired rule (`:404-406`). |

### PHREAK (`engine/src/engine/phreak/mod.rs`)

Redefines its own `AlphaMemory` (`:25-48`), `BetaMemory` (`:51-74`), `FactId`, `Nagram`,
`MemoryReference` (`:18-22`, `:117-122`) instead of importing Rete's; its `AlphaMemory` has no
`remove` and its `BetaMemory` no `remove_fact`, so PHREAK bookkeeping is not interchangeable.

| Structure | Lines | Reality |
|---|---|---|
| `Justification`, `DependencyGraph`, `TruthMaintenance` | `124-327` | Real records and edges; `DependencyGraph::remove_justification` (`:215-224`) retains `producers` keys by premise rather than conclusion, so conclusion-keyed entries are not cleaned. |
| `ExecutionTrace`/`RuleFire` | `363-425` | Real recorder, populated only by network traversal (`:508`), never by `execute_rules`. |
| `assert_fact` | `464-490` | Stores, propagates, and `tms.support_fact` (`:487`). |
| `execute_rules` | `578-600` | Delegates to `direct_execute`; **never touches the TMS** (documented `:574-577`). |
| justifications | — | None created outside tests, so `justification_count() == 0` (asserted `:793`). |
| ~~`retract_fact` is `Ok(false)` unconditionally~~ | **Fixed in 9.0.32**: the trait method retracts for real (working memory, alpha, beta, TMS) and the shared cross-engine suite asserts it for all four engines. The placeholder `retract_fact_with_maintenance` is deleted. |

## Fact lifecycle

**Insert.** `insert_fact` → `assert_fact`: working-memory insert, alpha insert if a matching
test node exists, join/beta propagation, `record_fact_inserted`
(`rete/mod.rs:274-294`, `treat/mod.rs:232-255`, `leaps/mod.rs:271-300`,
`phreak/mod.rs:464-490`). `Action::InsertFact` inside the evaluator bypasses all of that — it
mutates `working_facts` only with a fresh UUID (`direct_eval.rs:224-227`, `fact.rs:27-34`).
Because ids are UUID v4 (`engine/src/fact.rs:16`, `:29`), two `Fact::new` calls never collide;
only caller-supplied `Fact::with_id` (`:37-40`) can, and then insert silently overwrites.

**Retract.**

| Engine | Removes | Returns | Source |
|---|---|---|---|
| Rete | working memory by id, alpha entry for the fact's `fact_type_id`, every beta nagram referencing the id (empty buckets pruned) | `Ok(true)`; `Ok(false)` for unknown id | `rete/mod.rs:580-590`, `:120-127` |
| TREAT | Rete's three + `cache.clear()` (a cached nagram may mention the fact) | `Ok(true)`/`Ok(false)` | `treat/mod.rs:429-442` |
| LEAPS | Rete's three + `reset()` on **every** aggregation node (source set and cached value cleared) | `Ok(true)`/`Ok(false)` | `leaps/mod.rs:459-475`, `:144-147` |
| PHREAK | nothing | always `Ok(false)` | `phreak/mod.rs:650-653` |

Idempotency (Rete/TREAT/LEAPS): retracting an absent id returns `Ok(false)` and mutates
nothing, so a second retract is a no-op
(test: `engine/src/engine/rete/mod.rs:635-636`). No engine calls
`record_fact_retracted` (`engine/src/engine/metrics.rs:77`), so `facts_retracted` stays `0` after
successful retracts. Retraction does not touch the
coverage/tracker singletons; `fired_set` is per-call, so a later `execute()` re-evaluates all
rules against the reduced working memory.

## Conflict resolution and salience

* `Rule.salience: i32` (`engine/src/rules.rs:22`, builder `:77-80`) orders every pass: both the
  shared evaluator (`engine/src/engine/direct_eval.rs:122-124`) and Rete's inlined loop
  (`engine/src/engine/rete/mod.rs:404-406`) build a stable salience-descending firing order, so
  higher salience fires first and ties keep load order. Values outside -1000…=1000 are rejected by
  `Rule::validate`.
* `no_loop` is satisfied by the once-per-rule fixpoint (a rule never fires twice in one execution),
  which is why the field is inert: no execution path reads it (`direct_eval.rs` reads `salience`,
  `agenda_group` and `lock_on_active`, never `no_loop`). The guarantee it names is unconditional, so
  the flag changes nothing — recorded here because the user guide used to advise setting it.
  `lock_on_active` closes the rule's `agenda_group` for the rest of the pass
  (`direct_eval.rs:171-178`), so the group's other rules wait for the next pass.
* `Agenda` (`engine/src/salience/agenda.rs:45-111`) implements salience-desc/timestamp-asc
  insert (`:57-68`), front-pop (`:71-78`), `remove_activation` for no-loop (`:96-100`) and
  `all_activations` (`:103-105`); `Activation` clones the whole `Rule` (`:12-21`).
* `ConflictStrategy = Salience | DepthFirst | BreadthFirst | Recency | Lexicographic`
  (`agenda.rs:113-128`) is applied by `ConflictResolver::sort`/`sort_in_place`
  (`:144-200`). Depth/breadth order by `fact_matches.len()`; `Recency` is newest-first;
  `Lexicographic` sorts on `rule.name`.
* The enum and resolver used to be duplicated verbatim in `engine/src/salience/agenda.rs` and
  `engine/src/salience/conflict_resolution.rs`. **Closed in 9.0.30**: `agenda.rs` no longer defines
  its own copies, so there is one `ConflictStrategy` and one `ConflictResolver`
  (`salience/conflict_resolution.rs:9-116`), which `salience/mod.rs:12` re-exports.
* Nothing in the execution path constructs an `Agenda` or `ConflictResolver`: callers are the
  modules' own tests and the test-only `deterministic_conflict_strategy`
  (`engine/src/testing/mod.rs:653-661`).

The four engines are held to one functional suite in `engine/tests/cross_engine_suite.rs`: the
engine names come from `EngineRegistry::available_engines()`, so every compiled engine is judged by
the same scenario — firing and applying an action, retracting with idempotence, resetting, and being
built freshly per execution. Before that suite existed each engine had its own keystone test with a
copied fixture, so nothing failed when one engine drifted from the others.

Net: salience and agenda-group locking affect execution through the evaluator's own ordering; the
`salience::Agenda`/`ConflictResolver` types remain a tested standalone utility that the live path
does not use, so the two could drift.

## Chaining modes

`ChainingMode = Automatic | ForwardOnly | BackwardOnly`
(`engine/src/chaining/mode_selector.rs:124-131`). `EngineBuilder::chaining_mode`
(`engine/src/environment.rs:34-37`, default `Automatic` `:27`) stores it and
`Engine::get_chaining_mode` returns it (`:107-109`); no execution path reads it.

* **Forward**: ~~`execute_forward(&mut Engine, &Session)` ignores both arguments and returns a
  default `ExecutionResult` — the body is comments only~~ — **closed in 9.0.52**: it runs the shared
  fixpoint evaluator over the engine's rules and working memory
  (`engine/src/forward_chaining.rs`), and both it and `execute_forward_simple` delegate to one `run`,
  so the two entry points cannot answer differently. `Engine::load_rules` parses the same JSON the
  API stores (a rule document, a bare rule, or an array) instead of returning `Ok(())` without
  reading anything, which is what makes a firing path reachable at all.
  `engine/tests/forward_chaining.rs` loads a rule, fires it, and reads the effect back out of
  working memory. `Engine::execute_forward` reports the count through its own `ExecutionResult`
  rather than the fixed string.
* **Backward**: ~~`execute_backward(&Engine, &Session, goal)` always returns
  `InferenceTrace::new(false)` with exactly `Query{goal}` and `Backtrack{"No rules match the query"}`~~
  — **closed in 9.0.54**: the search is real. A goal names a field (`"<fact_type>.<field>"`); the
  search finds the rules whose actions write that field, evaluates their conditions against working
  memory, and for each unmet condition recurses into "which rule could supply it" (depth-limited to
  `MAX_DEPTH = 8`). The trace records `Query`, `RuleApplication`, `SubGoal`, `FactMatch` and
  `Backtrack` steps and `explain()` prints them; `Engine::execute_backward` delegates to the same
  search and renders its trace as a path, so the two cannot disagree.
  `engine/tests/forward_chaining.rs` covers a direct hit, a goal nothing can produce, a goal that
  names no field, and a chain that needs a supplying rule.
  **Limit:** conditions are matched against the facts that exist and against *rules that could insert*
  what is missing; there is no variable unification or fact binding across rules, so a goal whose
  chain needs a fact to be *derived and then matched* is reported as unexplored rather than solved.
* **Mode selector**: `analyze_rule_structure` returns `GoalDriven` **iff** the condition tree
  contains a `Pattern` whose `fact_type` starts with `"query_"`, else `DataDriven`
  (`engine/src/chaining/mode_selector.rs:42-67`); `InferencePattern::Mixed` is never produced
  (`:46-49`) though `select` handles it (`:92-95`). `select` short-circuits to
  `BackwardOnly` when the target starts with `"query_"` or contains `'?'` (`:76-82`), else
  all-`DataDriven` ⇒ `ForwardOnly`, all-`GoalDriven` ⇒ `BackwardOnly`, mixed **or empty rule
  list** ⇒ `Automatic` (`:99-103`). No engine consults it.

Four different "trace" mechanisms: evaluator rule-firing trace
(`TraceEvent`/`TraceData`, `engine/src/engine/registry.rs:128-154`, mapped to the API
`InferenceTrace` at `api/src/v1/execution.rs:812-838`); `InferenceTraceCollector`
(`engine/src/chaining/mode_selector.rs:134-197`), a manual recorder no executor calls
(re-exported at `engine/src/lib.rs:54`); PHREAK's `ExecutionTrace`
(`engine/src/engine/phreak/mod.rs:363-425`, populated by propagation only); and
`BackwardChainingTrace` whose two recorders are empty stubs
(`engine/src/testing/mod.rs:599-636`).

## Observability

**One coverage implementation, one singleton.** `monitoring::coverage::CoverageTracking` is a
process-wide `static INSTANCE: LazyLock<Mutex<TrackingState>>`
(`engine/src/monitoring/coverage.rs:86-90`) with associated-function recording
(`record_rule_eval`/`record_rule_fire`/`record_rule_skipped`, `:97-118`), owned-clone reads
(`:121-131`), `coverage_percent = fired / total_tracked` (`:152-157`), plus JSON (`:167-197`)
and CSV (`:200-226`) exports. It is the only one the engine records into, from the two evaluator
sites (`direct_eval.rs:152-158`, `rete/mod.rs:459-465`).

~~`coverage::CoverageTracker` is a separate `&mut self` type with different `record_fire` and
percentage semantics, and `coverage/history/` is a byte-identical copy.~~ — **deleted in 9.0.58**: the
second `CoverageTracker` had no production callers, and the copy under `history/` went in 9.0.30 with
the feature that no build enabled.

**Execution tracker / dead rules — a second singleton.** `RuleExecutionTracker` is a unit
struct over a third `LazyLock<Mutex<TrackerState>>`
(`engine/src/monitoring/detection.rs:70-97`); `record_evaluation`/`record_fire` insert-or-update
`DeadRuleInfo` and bump global counters (`:117-147`), with `record_fire` stamping
`last_fired_at` internally via `chrono::Utc::now().to_rfc3339()` (`:138`). `new()` is
`#[deprecated]` and stateless (`:109-114`). `find_dead_rules()` = tracked rules with
`fire_count == 0` (`:175-183`); `find_truly_inactive_rules()` = both counts zero (`:190-198`,
which cannot report a rule never recorded); `activation_rate()` = `total_fires/total_evaluations`
(`:219-225`); `reset()` keeps entries (`:230-237`); `clear()` wipes all and is the documented
test-isolation primitive (`:239-249`). `DeadRuleReport::generate()` reads that singleton and
attaches a recommendation per dead rule (`:279-319`). Separately,
`monitoring::dead_rules::HealthRegistry` is a **non-singleton** path
(`engine/src/monitoring/dead_rules.rs:118-273`) whose `detect_dead_rules()` returns *all*
rules with a confidence score (`:195-232`), hardcodes a 24-hour threshold (`:203`) although
it reads `SlidingWindowConfig` into `config` (`:200`) for the confidence window only
(`:219-224`), and whose `find_dead_rules(min_inactive)` also requires
`activation_count == 0` (`:179-185`).

**Per-engine metrics.** Engines hold `crate::engine::metrics::SimpleMetricsCollector`
(`engine/src/engine/metrics.rs:98-166`) behind `crate::engine::metrics::MetricsCollector`
(`:63-94`); `EngineMetrics` (`:13-33`) carries `rules_fired`, `facts_inserted`,
`facts_retracted`, `alpha_memory_size`, `beta_memory_size`, `network_traversals`,
`execution_time_ms`, `cache_hits`, `cache_misses`, with `total_operations` (`:42-44`),
`firing_rate` (`:47-53`), `total_memory_estimate` (`:56-59`) and Prometheus/JSON export
(`:177-282`). A ~~second~~ stack with a different trait and struct of the same names used to live in
`engine/src/metrics/`, with no engine users and a `prometheus` module gated on a feature no build
declared; **it is deleted in 9.0.58**, so one metric stack remains.

**Singleton implications.** Coverage and dead-rule state are shared by every engine, session and
request in the process (`monitoring/coverage.rs:86-90`, `monitoring/detection.rs:91-97`), keyed by
rule id only: two rulesets reusing an id (`"r1"`) collide silently, counters are cumulative rather
than per request, `coverage_percent` mixes rule identities, and rule names are never refreshed
because recording uses `or_insert_with` (`monitoring/detection.rs:121-125`, `:140-144`). Tests must
`clear()` under `#[serial]`, as the module's own tests do (`monitoring/detection.rs:345-461`). The
per-engine `SimpleMetricsCollector` is *not* shared — it lives in each engine value, hence per
request when engines are built per request (`api/src/v1/execution.rs:618-640`).

## Simulation and testing utilities

**What-if contract.** `SimulationRequest{name, base_state, proposed_changes, fact_sets}`
(`engine/src/simulation/types.rs:17-37`); `RuleChange` =
`AddCondition|RemoveCondition|ModifyAction|RemoveRule|AddRule|DisableRule` carrying JSON-encoded
AST (`:41-78`); `SimulationResult` (`:116-129`), `TestCaseResult{fact_set_index, variants,
differences}` (`:133-146`), `VariantResult{variant_label, rules_fired, execution_time_ms, output,
success, error}` (`:150-174`), `ChangeReport` = `RuleFiredBeforeButNotAfter|
RuleFiredAfterButNotBefore|DifferentResultsForRule|ChangeApplicationError` (`:178-206`). The
documented runner contract (`engine/src/simulation/runner.rs:44-51`): run the base per fact set,
clone-and-modify per change, run each variant over the same fact sets, diff. `run()` is that
skeleton only — empty `fact_sets` yields no cases (`:62-68`), an empty individual set is skipped
(`:83-85`), the base runs first (`:91-110`), one variant per change follows (`:113-134`), each
diffed against the base (`:137-147`, `compute_differences` `:258-300`). Execution itself is a
placeholder: each variant clones the `Engine` (`:173`, `:216`), parses each `TestFact` and
**drops it** (`let _fact = ...`, `:188`, `:231`), calls the placeholder
`Engine::execute_forward()` (`:192`, `:235`), and hardcodes `rules_fired: Vec::new()` /
`output: None` (`:193`, `:201`, `:236`, `:245`); `change` only feeds the label (`:241`), and
`base_state` is never read (defined once at `engine/src/simulation/types.rs:25`). Hence
`differences` is empty apart from `ChangeApplicationError`, `success` is always `true`, and the
HTTP endpoint forwards straight into it (`api/src/v1/simulation.rs:150-161`).

| Utility | Lines | Contract |
|---|---|---|
| `TestSession` | `engine/src/testing/mod.rs:286-353` | Fresh `EngineBuilder::new().build()` + UUID per instance (`:308-317`); `insert_fact` writes working memory (`:340-342`); backward execution clones the engine and calls the placeholder (`:345-352`). |
| `FixtureBuilder` | `:371-398` | Accumulates `Fact::new(name, payload)`, builds a `WorkingMemory` (`:391-397`). |
| `ForwardChainingTester` | `engine/src/testing/mod.rs` | Session + `activations: Vec<String>`. ~~`execute_forward` never fills it, so the four assertions only inspect data pushed by hand, and `add_rule` discards the rule.~~ **Fixed in 9.0.69**: `add_rule` calls `Engine::add_rule` (it used to format the DSL text `"rule <id>"` for a JSON loader and discard the parse error), and `execute_forward` maps the report's `fired_rules` ids to rule **names** into the trace. `assert_fired_in_order` now advances past the activation it matched, so a repeated expectation cannot match one firing twice. Two tests run the tester against a real engine — one where a rule fires, one where nothing matches — and the first fails if either defect returns. |
| ~~`BackwardChainingTrace`~~ | — | **Deleted in 9.0.69**: both "recorders" were empty, and nothing called the type. The real trace is `engine::backward_chaining::InferenceTrace` (`explain()` renders it), which is what `Engine::execute_backward` returns. |
| `DeterministicMode` | `:642-650` | Consumed only by `deterministic_conflict_strategy` (`:653-661`), which the execution path never calls. |
| `assert_eq_test!`/`assert_test!` | `:667-708` | Macros returning `TestError::AssertionFailed`. |
| `ValueSnapshot`/`FactSnapshot`/`WorkingMemorySnapshot` | `:41-196` | Snapshot DTOs; FEEL-only `Value` variants degrade to `Debug` strings (`:102-131`); `compare` keys facts by `fact_type` (`:160-163`), collapsing same-type facts; `SnapshotDiff` renders Added/Removed/Changed (`:199-243`). |
| `SnapshotManager` | `engine/src/testing/snapshot.rs:68-269` | Baselines at `{base_dir}/{test}.snapshot.json` (`:105-107`); `SnapshotMode::{Read,Update,Skip}` (`:15-23`) drive `assert_matches_baseline` (`:189-231`); `update_baseline` aliases `save_baseline` (`:176-186`). |
| `serialize_deterministic`, `deserialize_working_memory` | `engine/src/testing/json.rs:22-38` | Recursive key sorting for stable snapshot text. |

## Invariants and limits

* A rule fires **at most once per `execute()` call** (`engine/src/engine/direct_eval.rs:102-104`,
  `engine/src/engine/rete/mod.rs:408-410`).
* A pass ends when nothing newly fires; passes ≤ `RRULES_MAX_CHAIN_ITERATIONS` (default 1000)
  (`engine/src/engine/direct_eval.rs:73-76`, `:134-136`).
* Firing order is rule-vector order and mutations are visible to later rules in the same pass
  (`engine/src/engine/direct_eval.rs:100-133`); salience is inert.
* `fired_rule_ids` is unordered; only `TraceData.events` is in firing order
  (`engine/src/engine/registry.rs:141-146`, `:165-172`).
* `success` is always `true` and `trace` is `None` unless `set_collect_trace(true)` *and* a rule
  fired (`engine/src/engine/registry.rs:183-191`, `engine/src/engine/direct_eval.rs:165-176`).
* `Condition::Pattern` requires an existing fact of the named type (`engine/src/rules.rs:196-211`).
* Working memory is `HashMap<String, Fact>` keyed by `Uuid::to_string()`
  (`engine/src/fact.rs:16`, `engine/src/engine/direct_eval.rs:69`).
* Retraction is idempotent for Rete/TREAT/LEAPS and unimplemented for PHREAK
  (`engine/src/engine/phreak/mod.rs:650-653`).
* `facts_retracted` is always `0` (`engine/src/engine/metrics.rs:77`, `:139-141`).
* Coverage/dead-rule state is process-global; tests need `clear()` + `#[serial]`
  (`engine/src/monitoring/detection.rs:239-249`).
* `available_engines()` order is unspecified; consumers sort
  (`engine/src/engine/registry.rs:288-293`, `api/src/v1/execution.rs:1142-1149`).
* `reset()` drops loaded rules but preserves `collect_trace`
  (`engine/src/engine/rete/mod.rs:612`, `:614`).

## Known gaps

1. **~~Duplicated evaluator bodies.~~** Closed 2026-10-25: Rete now calls
   `direct_execute_with_limit` like the other three engines and its duplicate `apply_action`
   is deleted. The copies had already diverged — the inlined loop honoured neither
   `lock_on_active` nor deterministic fact resolution — which is exactly the drift the
   duplication invited.
2. **LEAPS looks alpha facts up by node id as if it were a fact type**
   (`engine/src/engine/leaps/mod.rs:338`) whereas Rete resolves the predecessor test node
   (`engine/src/engine/rete/mod.rs:326`); LEAPS beta memory therefore normally stays empty.
3. **~~PHREAK retraction is a stub returning `Ok(false)` for every id.~~** **Corrected in 9.0.32**:
   the trait's `retract_fact` removes the fact from working memory, alpha memory and beta memory and
   prunes its truth-maintenance edges, and `every_engine_retracts_a_fact_and_reports_the_second_attempt_as_a_no_op`
   holds all four engines to that. The *placeholder* the gap actually described was
   `retract_fact_with_maintenance`, a separate public method that returned `Ok(false)` for everything
   and had no callers; it is deleted.
4. **PHREAK truth maintenance is dead weight:** facts are supported (`phreak/mod.rs:487`), no
   `Justification` is ever created, `justification_count()` is structurally 0 (asserted `:793`),
   and `execute_rules` ignores the TMS (`:574-577`).
5. ~~**TREAT's cache is neither LRU nor sound:** key-sorted eviction, a per-node key that can skip a
   different fact, a hardcoded `stats()`, duplicate counters, and a `with_cache_size` that ignored its
   argument.~~ — **closed in 9.0.56**: entries carry a `last_used` mark and eviction takes the coldest,
   which `eviction_takes_the_least_recently_used_entry` pins (the alphabetical version evicted `aaa`
   whether it had just been used or never); the propagation key names the **fact** as well as the node,
   so the second fact reaching a cached join node is no longer answered with the first one's entry;
   `stats()` counts the hits and misses the cache itself observes; the duplicated `cache_hits`/
   `cache_misses` fields are gone, leaving the metrics collector as the single source; and
   `TreatEngineFactory::with_cache_size` carries its argument into every engine it builds.
   **Note:** execution goes through the shared evaluator rather than this network, so the cache is
   tested where it lives — `engine/src/engine/treat/mod.rs`'s unit tests — and not through the public
   execution path.
6. ~~**LEAPS aggregation never computes:** `recalculate` returns `None`, `execute_aggregations`
   discards its result, and every fact is fed to every node.~~ — **closed in 9.0.55**: the node holds
   the values it aggregates and recomputes on add and on removal; a fact whose payload lacks the
   aggregated field (or carries it as a non-number) is not made a member, which is also what stops the
   network treating every fact as an input; and `execute_aggregations` recomputes every node from
   working memory rather than discarding the result. `engine/src/engine/leaps/mod.rs`'s tests assert the
   sums and the membership rule.
7. **~~Salience/conflict resolution unwired~~** (partly closed 2026-10-25): both evaluators now
   order passes by `salience` and honour `lock_on_active` within an agenda group, and
   `Rule::validate` rejects an out-of-range salience. Still open: `Agenda`/`ConflictResolver` have
   no production callers — the execute path has its own salience sort (`direct_eval.rs:124`). The
   duplication is closed (9.0.30): `agenda.rs` no longer defines a second `ConflictStrategy` and
   `ConflictResolver`, so the exported API cannot drift from itself.
8. **Chaining does not run:** `execute_forward` returns a default result
   (`engine/src/forward_chaining.rs:45-57`), `execute_backward` returns a fixed failure trace
   (`engine/src/backward_chaining.rs:80-93`), `Engine::execute_forward` returns
   "not yet implemented" (`engine/src/environment.rs:132-138`), and three parallel
   `ExecutionResult`/`InferenceTrace` type pairs coexist
   (`forward_chaining.rs:9-40`, `backward_chaining.rs:9-49`, `environment.rs:211-246`).
9. **What-if simulation is a skeleton:** facts parsed and dropped
   (`engine/src/simulation/runner.rs:177-189`, `:219-232`), `RuleChange` never applied
   (`:208-248`), `rules_fired` always empty (`:193`, `:236`), `base_state` ignored
   (`engine/src/simulation/types.rs:25`).
10. ~~**Two parallel metric stacks and a dead file:** `engine::engine::metrics` is used,
    `engine::metrics` is not, and `engine/src/metrics/prometheus.rs` is gated on a feature no build
    declares.~~ — **closed in 9.0.58**: `engine::metrics` and its `prometheus` module are deleted, leaving
    `engine::engine::metrics` as the one metric stack.
11. ~~**Two coverage implementations plus a duplicated file.**~~ — **closed in 9.0.58**: the singleton
    `CoverageTracking` (`monitoring/coverage.rs:86-233`) is the only one left; `CoverageTracker`
    (`engine/src/coverage/mod.rs`) had no production callers and is deleted, and the byte-identical copy
    under `coverage/history/` went in 9.0.30 with the `history` feature nothing enabled.
12. **~~Code that was never compiled: `testing/report/`~~** — **deleted in 9.0.58.** A 573-line
    HTML report generator (`HtmlReport`, `ReportSummary`, `TestResult`) sat in
    `engine/src/testing/report/`, and **no module declared it**: nothing in the crate reached `mod
    report;`, so it had never been type-checked, let alone run. It also imported
    `crate::testing::RuleCoverage`, a name that does not exist there — which is why it could not have
    compiled had anyone declared it. Found by scanning for public functions nothing references outside
    their own file; the scan is a heuristic, and this entry is what came of checking it.
13. **~~Dead test module:~~** **Deleted in 9.0.30.** `engine/src/tests/mod.rs` was not declared in
    `engine/src/lib.rs` and had never compiled: 992 lines and 70 tests targeting an older API
    (`evaluate`, `Agenda::with_strategy`, `EngineBuilder::conflict_strategy`) — 45 compile errors.
    It contributed no coverage, because a test that never builds never runs, and its subject matter
    is covered by the 38 in-module tests in `engine/src/rules.rs`.
13. **LEAPS metrics count non-events:** `record_rule_fired()` on aggregated-join traversal
    (`leaps/mod.rs:322-324`) inflates `rules_fired` beyond `EngineResult.rules_fired`.
14. **`HealthRegistry::detect_dead_rules` returns live rules too** and hardcodes a 24-hour
    threshold while ignoring `SlidingWindowConfig` for it
    (`monitoring/dead_rules.rs:195-232`, `:203`); its `find_dead_rules` additionally requires
    `activation_count == 0` (`:179-185`), so the two APIs disagree.
15. **`EngineSelector` over-advertises:** `with_defaults()` lists four engines regardless of
    features (`engine/src/engine/selector.rs:77-79`) and `from_yaml`/`from_json` always error
    (`:127-144`); the runtime resolves names through the registry instead
    (`api/src/v1/execution.rs:606-613`).
16. **`facts_matched` is not a match count** — it sums working-memory size once per firing
    (`engine/src/engine/direct_eval.rs:116`), so it scales as `|facts| × firings`.
17. **`RRULES_DEFAULT_ENGINE` fallback is startup-only:** the warning/fallback
    (`api/src/app.rs:117-123`) affects only `selector.current_engine()`, while per-request
    resolution re-reads the raw env var (`api/src/v1/execution.rs:600-602`) and rejects the
    value with 400 (`:606-613`).
18. **`EngineBuilder::max_iterations` (default 1000, `engine/src/environment.rs:29`) is not wired
    to the evaluator**, which reads `RRULES_MAX_CHAIN_ITERATIONS` instead
    (`engine/src/engine/direct_eval.rs:73-76`); the two knobs can disagree.
