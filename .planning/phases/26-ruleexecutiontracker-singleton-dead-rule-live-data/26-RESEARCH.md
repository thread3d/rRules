# Phase 26: RuleExecutionTracker singleton + dead-rule live data - Research

**Researched:** 2026-07-03
**Domain:** Rust internal wiring — process-global singleton tracker, dead-rule detection, three-surfaces metric agreement
**Confidence:** HIGH (all claims verified against live code with file:line evidence)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Mirror `CoverageTracking` — zero-sized-struct + `LazyLock<Mutex<State>>` + associated functions. `RuleExecutionTracker` becomes a unit-like struct backed by `static INSTANCE: LazyLock<Mutex<TrackerState>>`. Public API: `record_evaluation(rule_id, rule_name)`, `record_fire(rule_id, rule_name)`, `find_dead_rules() -> Vec<DeadRuleInfo>` (owned), `reset()`/`clear()` for tests.
- **D-02:** `CoverageTracking` is the canonical source of truth for fire/eval COUNTS; `RuleExecutionTracker` derives counts and owns dead-rule metadata (`last_fired_at`, `rule_name`, `category`); per-engine `metrics.record_rule_fired` is decoupled from the fire-count contract.
- **D-03:** Record in `engine/src/engine/direct_eval.rs` next to the `CoverageTracking` calls (lines ~106, ~109), NOT at `rete/mod.rs:293,332` (stale ROADMAP refs).
- **D-04:** Deprecate `new()` (`#[deprecated]`); rewrite the one production call site (`metrics.rs:380`); migrate `detection.rs` tests to singleton + `clear()` + `serial_test`.
- **D-05:** The parallel `HealthRegistry` (`engine/src/monitoring/dead_rules.rs`) and `coverage/history/mod.rs` `CoverageTracker` are OUT of scope — document, do not merge.
- **D-06:** Singleton stamps `last_fired_at` internally via `chrono::Utc::now().to_rfc3339()`; call site passes `rule_id`/`rule_name` only.
- **D-07:** Extend `api/tests/execute_integration.rs` with the keystone e2e + three-surfaces-agreement assertion. Fire `r1`, leave `r2` unfired; assert dead-rules excludes `r1` AND includes `r2`, coverage shows `r1.fired`, three surfaces agree on fire count. `#[serial]` isolation.

### Carried-forward locks (L-01..L-05)
- **L-01:** Singleton pattern = mirror `CoverageTracking` exactly (`coverage.rs:83-93`).
- **L-02:** Recording call site is the shared direct evaluator (`direct_eval.rs:106,109`), NOT `rete/mod.rs:293,332`. **⚠ CORRECTION — see §Corrections: Rete keeps its OWN inlined `execute_rules`; recording must be added in BOTH `direct_eval.rs` AND `rete/mod.rs:393-398`.**
- **L-03:** `dead_rules_endpoint` (`metrics.rs:378-386`) MUST read from the singleton instead of `RuleExecutionTracker::new()`.
- **L-04:** Once-per-rule semantics reuses `evaluated_set` + `fired_set` `HashSet<String>`; no new per-nagram recording.
- **L-05:** `serial_test` (workspace dev-dep) is the established isolation tool.

### Claude's Discretion
- Singleton state shape (reuse struct as `Mutex` payload vs. factor a `TrackerState`).
- `CoverageTracking` extension vs. delegation for dead-rule metadata fields.
- Per-engine `metrics.record_rule_fired` fate (verify against Phase 24 wiring — see §Source-of-Truth Unification).
- Deprecation mechanism (`#[deprecated]` vs `#[cfg(test)]`-gated vs `pub(crate)`).
- Test isolation mechanism (mirror `CoverageTracking`'s `clear()` + `serial_test`).

### Deferred Ideas (OUT OF SCOPE)
- Merge `HealthRegistry` (`dead_rules.rs`) into the unified tracker — deferred (D-05).
- Merge `coverage/history/mod.rs` `CoverageTracker` — deferred (D-05).
- Stateful sessions — Phase 27.
- Frontend dead-rules panel / test-runner UI — Phase 28.
- `inference_trace` population — Phase 28 (EXEC-04).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| MON-06 | Dead-rule detection reads live firings (no longer always `[]`); three metric surfaces agree on fire count. | Singleton conversion (§Singleton Design), call-site recording (§Recording Call Site), endpoint rewrite (§Endpoint Rewrite), e2e keystone (§E2E Keystone), three-surfaces agreement (§Source-of-Truth Unification). |
</phase_requirements>

## Summary

Phase 26 converts `RuleExecutionTracker` (`engine/src/monitoring/detection.rs:64`) from a locally-instantiated struct into a process-wide `LazyLock<Mutex<TrackerState>>` singleton, mirroring the existing `CoverageTracking` pattern (`engine/src/monitoring/coverage.rs:86`). The dead-rules endpoint (`api/src/metrics.rs:380`) currently constructs a fresh empty tracker per request (Pitfall 3 — always returns `[]`); it will instead read from the singleton, which is fed live by the direct evaluator at the same call sites that already record `CoverageTracking`.

**Primary recommendation:** Add `RuleExecutionTracker::record_evaluation(id, name)` / `record_fire(id, name)` adjacent to the existing `CoverageTracking` calls in **two** places — `direct_eval.rs:105-110` (Treat/Leaps/Phreak path) AND `rete/mod.rs:393-398` (Rete keeps its own inlined `execute_rules`, confirmed against live code). The singleton stamps `last_fired_at` internally (D-06). Deprecate `new()`, rewrite the one production call site, migrate the 6 in-crate tests to `clear()` + `#[serial]`. Pin with a two-rule keystone e2e (`r1` fires, `r2` unfired) asserting SC-1/SC-2/SC-3.

**Three-counter drift resolution (refined by live-code verification):** The three counters ROADMAP names are (1) `CoverageTracking.fire_count` (`coverage.rs:108`), (2) `RuleExecutionTracker.total_fires` (`detection.rs:112`), (3) per-engine `EngineMetrics.rules_fired` (`engine/src/engine/metrics.rs:12`, incremented at `rete/mod.rs:381`, `treat/mod.rs:387`, `leaps/mod.rs:309,388`, `phreak/mod.rs:574`). **Live-code finding:** the Prometheus `rrules_execute_rules_fired_total` counter (the `/metrics` surface for SC-3) is fed from `api/src/v1/execution.rs:637` `rules_fired_total.inc_by(engine_result.rules_fired)`, where `engine_result.rules_fired == fired_set.len()` — **NOT** from `EngineMetrics.rules_fired`. So `EngineMetrics::record_rule_fired` is **not** a source of truth for any of the three SC-3 surfaces; it is a per-engine in-memory gauge exposed only via `get_metrics()` (no live endpoint reads it). The honest unification: `CoverageTracking` is canonical for counts; `RuleExecutionTracker` derives counts at the same call sites and owns metadata; `EngineMetrics.rules_fired` is left as a per-engine gauge and documented as out-of-contract. SC-3 agreement is between `CoverageTracking.fire_count`, the dead-rules exclusion (fire_count > 0 ⇒ not dead), and `rrules_execute_rules_fired_total` (all three ultimately derived from `fired_set`).

## Corrections to CONTEXT.md / ROADMAP.md

| # | Claim | Correction | Evidence |
|---|-------|------------|----------|
| C1 | CONTEXT L-02 / D-03: "all four engines delegate to `direct_eval.rs`" | **WRONG for Rete.** Only Treat/Leaps/Phreak delegate (`treat/mod.rs:383`, `leaps/mod.rs:384`, `phreak/mod.rs:570` all call `direct_eval::direct_execute`). Rete keeps its OWN inlined `execute_rules` at `rete/mod.rs:340-406` with duplicated coverage recording at 393-398. **`RuleExecutionTracker` recording must be added in BOTH `direct_eval.rs:105-110` AND `rete/mod.rs:393-398`.** | `[VERIFIED: live code]` `engine/src/engine/rete/mod.rs:340-406`; grep confirms no `direct_execute` call in `rete/mod.rs`. |
| C2 | CONTEXT D-05: "parallel `RuleHealthRegistry` (`dead_rules.rs`)" | Actual struct name is **`HealthRegistry`** at `dead_rules.rs:119` (not `RuleHealthRegistry`). No callers anywhere in `engine/src/` or `api/src/` — confirmed unwired. | `[VERIFIED: live code]` `grep -rn "HealthRegistry"` → only the definition; no use sites. |
| C3 | ROADMAP §"Phase 26": "`rete/mod.rs:293,332`" call-site line refs | **Stale.** `rete/mod.rs:296-300` is now a comment explaining recording moved out; the actual Rete recording is at `rete/mod.rs:393-398`. The shared non-Rete recording is at `direct_eval.rs:105-110`. | `[VERIFIED: live code]` `rete/mod.rs:296-300` comment; `rete/mod.rs:393-398` actual calls. |
| C4 | CONTEXT D-02: per-engine `metrics.record_rule_fired` as a "drift counter" for SC-3 | **Refined.** `EngineMetrics.rules_fired` is NOT a source for any SC-3 surface. The `/metrics` Prometheus fire count comes from `ExecutionCounters.rules_fired_total` (`api/src/v1/execution.rs:637`), fed by `engine_result.rules_fired` (`fired_set.len()`). `EngineMetrics.rules_fired` feeds only `get_metrics()` (in-memory, no endpoint). It is a per-engine gauge, NOT a third drift counter for the dead-rule/coverage contract. | `[VERIFIED: live code]` `api/src/v1/execution.rs:637-647`; `engine/src/engine/metrics.rs:12`. |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Rule fire/eval counting (canonical) | Engine (rule evaluator) | — | `CoverageTracking` singleton recorded once-per-rule at the fixpoint loop tail (`direct_eval.rs:105-110`, `rete/mod.rs:393-398`) — single source of truth. |
| Dead-rule metadata (`last_fired_at`, `rule_name`, `category`) | Engine (monitoring) | — | `RuleExecutionTracker` singleton owns metadata not present in `RuleCoverageState`; fed at the same call sites. |
| Dead-rules HTTP surface | API | Engine | `dead_rules_endpoint` reads `RuleExecutionTracker::find_dead_rules()` (singleton) — no local instantiation. |
| Coverage HTTP surface | API | Engine | `coverage_endpoint` already reads `CoverageTracking` singleton — unchanged. |
| Prometheus fire-count surface | API | Engine | `ExecutionCounters.rules_fired_total` incremented from `engine_result.rules_fired` in `execute_single` — already wired (Phase 24); unchanged. |
| Per-engine in-memory gauge | Engine | — | `EngineMetrics.rules_fired` (`engine/metrics.rs`) — out-of-contract for SC-3; left as-is. |

## Standard Stack

No new packages. Phase 26 is internal wiring against already-installed workspace deps.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `std::sync::LazyLock` | std | Process-global singleton interior mutability | Already used by `CoverageTracking` (`coverage.rs:86`). `[VERIFIED: live code]` |
| `std::sync::Mutex` | std | Singleton lock | Already used by `CoverageTracking`. `[VERIFIED: live code]` |
| `chrono` | workspace dep | `Utc::now().to_rfc3339()` for `last_fired_at` | Already used in `detection.rs`/`dead_rules.rs`. `[VERIFIED: live code]` |
| `serial_test` | workspace dev-dep | Singleton-isolation in tests | Already used by `CoverageTracking` tests; established pattern (L-05). `[VERIFIED: live code]` |

**Installation:** none required.

## Package Legitimacy Audit

No external packages installed this phase. All dependencies are existing workspace deps verified in the live `Cargo.toml` and already used by production code (`LazyLock`, `Mutex`, `chrono`) or by tests (`serial_test`).

**Packages removed due to [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

## Architecture Patterns

### System Architecture Diagram

```
POST /v1/execute (api/src/v1/execution.rs::execute_single)
        │
        ▼
EngineRegistry → ExecutionEngine (one of Rete/Treat/Leaps/Phreak)
        │
        ├── Rete:  rete/mod.rs::execute_rules  (inlined, lines 340-406)
        │              │  fixpoint loop collects fired_set + evaluated_set
        │              ▼  after loop (393-398):
        │            CoverageTracking::record_rule_eval(id)   ◄── canonical COUNTS
        │            CoverageTracking::record_rule_fire(id)   ◄── canonical COUNTS
        │            [NEW] RuleExecutionTracker::record_evaluation(id, name)  ◄── metadata
        │            [NEW] RuleExecutionTracker::record_fire(id, name)        ◄── metadata + last_fired_at
        │
        └── Treat/Leaps/Phreak:  direct_eval.rs::direct_execute (lines 53-113)
                                   │  fixpoint loop collects fired_set + evaluated_set
                                   ▼  after loop (105-110):
                                 CoverageTracking::record_rule_eval(id)   ◄── canonical COUNTS
                                 CoverageTracking::record_rule_fire(id)   ◄── canonical COUNTS
                                 [NEW] RuleExecutionTracker::record_evaluation(id, name)
                                 [NEW] RuleExecutionTracker::record_fire(id, name)
        │
        ▼  EngineResult { rules_fired: fired_set.len(), fired_rule_ids }
execute_single (api/src/v1/execution.rs:637)
        │
        ├── ExecutionCounters.rules_fired_total.inc_by(engine_result.rules_fired)  ◄── Prometheus /metrics
        └── snapshot_rules_fired.fetch_add(...)                                     ◄── WebSocket MetricsSnapshot

GET /v1/metrics/dead-rules  (api/src/metrics.rs::dead_rules_endpoint)
        │  [NEW] RuleExecutionTracker::find_dead_rules()  (singleton read, owned Vec<DeadRuleInfo>)
        ▼  filter fire_count == 0  →  Vec<String> of rule_ids

GET /v1/metrics/coverage   (api/src/metrics.rs::coverage_endpoint)
        │  CoverageTracking::to_json()  (singleton read — unchanged)
        ▼

GET /metrics (Prometheus)  (api/src/metrics.rs::metrics_endpoint)
        │  ExecutionCounters.registry.gather() → TextEncoder
        ▼  includes rrules_execute_rules_fired_total{engine=...}
```

SC-3 agreement: `CoverageTracking.fire_count` (coverage endpoint) and `rrules_execute_rules_fired_total` (Prometheus) both derive from `fired_set`; dead-rules endpoint excludes any rule with `fire_count > 0`. All three are consistent by construction because they share the `fired_set` origin.

### Recommended Project Structure
```
engine/src/monitoring/
├── detection.rs       # RuleExecutionTracker → singleton (D-01/D-04)
├── coverage.rs        # CoverageTracking singleton (THE pattern to mirror — unchanged)
└── dead_rules.rs      # HealthRegistry (parallel — OUT of scope, D-05)
engine/src/coverage/
└── history/mod.rs     # CoverageTracker (parallel — OUT of scope, D-05)
engine/src/engine/
├── direct_eval.rs     # [NEW] RuleExecutionTracker calls at 105-110 (Treat/Leaps/Phreak)
└── rete/mod.rs        # [NEW] RuleExecutionTracker calls at 393-398 (Rete inlined)
api/src/
├── metrics.rs         # dead_rules_endpoint rewrite (L-03/D-04)
└── v1/execution.rs    # unchanged (already wires ExecutionCounters)
api/tests/
└── execute_integration.rs  # [NEW] keystone e2e (D-07)
```

### Pattern 1: Singleton tracker via `LazyLock<Mutex<…>>` (mirror `CoverageTracking`)
**What:** Unit-like struct + `static INSTANCE: LazyLock<Mutex<State>>` + associated-function API; owned returns from inside the lock.
**When to use:** Process-global state that must be recorded from multiple call sites and read from multiple endpoints.
**Example:**
```rust
// Source: engine/src/monitoring/coverage.rs:83-110 (THE pattern to mirror)
use std::sync::{LazyLock, Mutex};
use std::collections::HashMap;

struct TrackerState {
    rules: HashMap<String, DeadRuleInfo>,
    total_evaluations: usize,
    total_fires: usize,
}

static INSTANCE: LazyLock<Mutex<TrackerState>> = LazyLock::new(|| {
    Mutex::new(TrackerState {
        rules: HashMap::new(),
        total_evaluations: 0,
        total_fires: 0,
    })
});

pub struct RuleExecutionTracker;

impl RuleExecutionTracker {
    pub fn record_evaluation(rule_id: impl Into<String>, rule_name: impl Into<String>) {
        let mut s = INSTANCE.lock().unwrap();
        let id = rule_id.into();
        s.rules.entry(id.clone()).or_insert_with(|| DeadRuleInfo::new(&id, rule_name)).record_evaluation();
        s.total_evaluations += 1;
    }

    pub fn record_fire(rule_id: impl Into<String>, rule_name: impl Into<String>) {
        let mut s = INSTANCE.lock().unwrap();
        let id = rule_id.into();
        let ts = chrono::Utc::now().to_rfc3339();          // D-06: singleton stamps internally
        let info = s.rules.entry(id.clone()).or_insert_with(|| DeadRuleInfo::new(&id, rule_name));
        info.fire_count += 1;
        info.last_fired_at = Some(ts);
        s.total_fires += 1;
    }

    pub fn find_dead_rules() -> Vec<DeadRuleInfo> {        // owned return (can't return refs from lock)
        let s = INSTANCE.lock().unwrap();
        s.rules.values().filter(|i| i.fire_count == 0).cloned().collect()
    }

    pub fn clear() {                                        // test isolation (L-05)
        let mut s = INSTANCE.lock().unwrap();
        s.rules.clear();
        s.total_evaluations = 0;
        s.total_fires = 0;
    }

    #[deprecated(note = "RuleExecutionTracker is now a process-wide singleton; use associated functions directly.")]
    pub fn new() -> Self { Self }
}
```
`[VERIFIED: live code]` `coverage.rs:86-110` is the canonical shape. `[CITED: coverage.rs:127-131]` for owned return from inside the lock.

### Anti-Patterns to Avoid
- **Returning `Vec<&DeadRuleInfo>` from `find_dead_rules`:** can't return references from inside a `Mutex` guard. Mirror `CoverageTracking::get_all_states` (`coverage.rs:127-131`) — return owned `Vec<DeadRuleInfo>` (`#[derive(Clone)]` already on `DeadRuleInfo`).
- **Recording inside the fixpoint inner loop:** would re-introduce Pitfall 6 (per-nagram double-count). Record once per id in `evaluated_set`/`fired_set` AFTER the loop, exactly where `CoverageTracking` is recorded.
- **Adding `RuleExecutionTracker` calls only in `direct_eval.rs`:** would miss Rete (C1 correction). Add in BOTH `direct_eval.rs:105-110` AND `rete/mod.rs:393-398`.
- **Instantiating `RuleExecutionTracker::new()` in production code:** re-introduces Pitfall 3 (always-empty tracker). The `#[deprecated]` attribute (D-04) makes any stray use a compile warning.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Process-global singleton | Custom global registry / `once_cell` | `std::sync::LazyLock<Mutex<State>>` | Already the established pattern (`coverage.rs:86`); std-only, no new dep. |
| Test isolation for singletons | Per-test `LazyLock` re-init / process spawning | `serial_test` + `clear()` | Established in `CoverageTracking` tests (L-05); `serial_test` already a workspace dev-dep. |
| Timestamp for `last_fired_at` | Custom clock / epoch formatting | `chrono::Utc::now().to_rfc3339()` | `chrono` already a workspace dep; used in `dead_rules.rs`. |
| Three-surfaces agreement | Cross-surface sync logic / drift reconciliation | Single source of truth (`fired_set` → `CoverageTracking` + `ExecutionCounters.rules_fired_total`) | All three surfaces derive from `fired_set`; agreement is by construction (D-02). |

## Common Pitfalls

### Pitfall 1: Stale/empty tracker (Pitfall 3)
**What goes wrong:** `dead_rules_endpoint` constructs `RuleExecutionTracker::new()` per request → always-empty → always returns `[]`.
**Why it happens:** `api/src/metrics.rs:380` instantiates locally instead of reading the singleton.
**How to avoid:** Rewrite to `RuleExecutionTracker::find_dead_rules()` (singleton read). Deprecate `new()` so any future stray local instantiation is a compile warning (D-04).
**Warning signs:** `GET /v1/metrics/dead-rules` returns `[]` even after a known-firing execute. Existing test `metrics_integration.rs:91` already documents this as "honest deferred tech debt."

### Pitfall 2: Three-counter drift (Pitfall 6)
**What goes wrong:** `CoverageTracking.fire_count`, `RuleExecutionTracker.total_fires`, and per-engine `EngineMetrics.rules_fired` diverge.
**Why it happens:** Three independent increment sites; one can fire without the others.
**How to avoid:** Record `CoverageTracking` and `RuleExecutionTracker` at the SAME call sites (`direct_eval.rs:105-110` + `rete/mod.rs:393-398`), both consuming the same `fired_set`/`evaluated_set`. `EngineMetrics.rules_fired` is out-of-contract (C4) — left as a per-engine gauge. SC-3 agreement is enforced by the e2e keystone (D-7).
**Warning signs:** `/metrics` Prometheus `rrules_execute_rules_fired_total` ≠ `/v1/metrics/coverage` `fire_count` ≠ dead-rules exclusion set.

### Pitfall 3: Missing Rete call site (C1 correction)
**What goes wrong:** Adding `RuleExecutionTracker` calls only in `direct_eval.rs` leaves Rete (the default engine) without dead-rule tracking — dead-rules still returns `[]` for Rete executes.
**Why it happens:** CONTEXT L-02 assumed all four engines delegate to `direct_eval.rs`. Rete does not.
**How to avoid:** Add the calls in BOTH `direct_eval.rs:105-110` (Treat/Leaps/Phreak) AND `rete/mod.rs:393-398` (Rete inlined).
**Warning signs:** Keystone e2e using `engine: "rete"` fails — dead-rules returns `[]` for a fired rule.

### Pitfall 4: Borrow conflict at the recording loop
**What goes wrong:** `for id in &evaluated_set { RuleExecutionTracker::record_evaluation(id, ???) }` — `rule_name` is not in `evaluated_set` (a `HashSet<String>` of ids only).
**Why it happens:** `direct_eval.rs` collected only ids into the sets, not `(&id, &name)` pairs.
**How to avoid:** Build a `HashMap<String, String>` (id → name) from `rules` before the loop, OR look up `rules.iter().find(|r| r.id == id).map(|r| r.name.clone())` at the recording site. The `Rule` struct has `pub id: String` and `pub name: String` (`engine/src/rules.rs:18,20`). Cleanest: collect `let id_to_name: HashMap<&String, &String> = rules.iter().map(|r| (&r.id, &r.name)).collect();` once before the recording loops, then pass `id_to_name[id]` (or `.copied()`) into the tracker calls. `[VERIFIED: live code]` `rules.rs:16-20`.

## Code Examples

### Recording call site — `direct_eval.rs` (Treat/Leaps/Phreak path)
```rust
// Source: engine/src/engine/direct_eval.rs:100-113 (existing) + [NEW] Phase 26 lines
use crate::monitoring::detection::RuleExecutionTracker;

// before the recording loops, build id→name lookup
let id_to_name: std::collections::HashMap<&String, &String> =
    rules.iter().map(|r| (&r.id, &r.name)).collect();

for id in &evaluated_set {
    CoverageTracking::record_rule_eval(id);
    RuleExecutionTracker::record_evaluation(id, id_to_name[id]);   // [NEW]
}
for id in &fired_set {
    CoverageTracking::record_rule_fire(id);
    RuleExecutionTracker::record_fire(id, id_to_name[id]);         // [NEW]
}
```

### Recording call site — `rete/mod.rs` (Rete inlined path)
```rust
// Source: engine/src/engine/rete/mod.rs:389-398 (existing) + [NEW] Phase 26 lines
use crate::monitoring::detection::RuleExecutionTracker;

let id_to_name: std::collections::HashMap<&String, &String> =
    self.rules.iter().map(|r| (&r.id, &r.name)).collect();

for id in &evaluated_set {
    CoverageTracking::record_rule_eval(id);
    RuleExecutionTracker::record_evaluation(id, id_to_name[id]);   // [NEW]
}
for id in &fired_set {
    CoverageTracking::record_rule_fire(id);
    RuleExecutionTracker::record_fire(id, id_to_name[id]);         // [NEW]
}
```

### Endpoint rewrite — `api/src/metrics.rs:378-386`
```rust
pub async fn dead_rules_endpoint() -> impl Responder {
    let dead = RuleExecutionTracker::find_dead_rules();   // singleton read
    let rule_ids: Vec<String> = dead.into_iter().map(|i| i.rule_id).collect();
    HttpResponse::Ok().json(rule_ids)
}
```

## Source-of-Truth Unification (D-02, refined by C4)

| Surface | Fire-count origin | Path |
|---------|-------------------|------|
| `/v1/metrics/coverage` `fire_count` | `fired_set` | `direct_eval.rs:108-110` / `rete/mod.rs:396-398` → `CoverageTracking::record_rule_fire` → `coverage.rs:108` |
| `/v1/metrics/dead-rules` exclusion | `fired_set` | same call sites → `RuleExecutionTracker::record_fire` → `detection.rs` `fire_count` field; `find_dead_rules` filters `fire_count == 0` |
| `/metrics` Prometheus `rrules_execute_rules_fired_total` | `fired_set` | `engine_result.rules_fired = fired_set.len()` (`rete/mod.rs:403` / `direct_execute` return) → `execute_single` `execution.rs:637` `.inc_by(engine_result.rules_fired)` |

All three derive from `fired_set`. `CoverageTracking` is the canonical record (D-02); `RuleExecutionTracker` records at the same sites and owns metadata; `ExecutionCounters.rules_fired_total` is fed by `engine_result.rules_fired` (already wired in Phase 24 — unchanged).

**Per-engine `EngineMetrics.rules_fired` fate (C4):** Leave as-is. It is incremented at `rete/mod.rs:381`, `treat/mod.rs:387`, `leaps/mod.rs:309,388`, `phreak/mod.rs:574` and exposed only via `MetricsCollector::get_metrics()` (in-memory). No HTTP endpoint reads it; it is not a source for any SC-3 surface. Document it as out-of-contract. Do NOT remove (would touch all four engines unnecessarily; removal is its own consolidation).

## Recording Call Site Confirmation (L-02 / D-03 — with C1 correction)

| Path | File:line | Engines | Action |
|------|-----------|---------|--------|
| Shared direct evaluator | `engine/src/engine/direct_eval.rs:105-110` | Treat, Leaps, Phreak (`treat/mod.rs:383`, `leaps/mod.rs:384`, `phreak/mod.rs:570`) | Add `RuleExecutionTracker::record_evaluation` + `record_fire` adjacent to `CoverageTracking` calls |
| Rete inlined `execute_rules` | `engine/src/engine/rete/mod.rs:393-398` | Rete only | Same addition |
| ~~`rete/mod.rs:293,332`~~ | `rete/mod.rs:296-300` (comment only) | — | Stale ROADMAP ref — ignore |

`rule_name` sourcing: `Rule` has `pub id: String` (`rules.rs:18`) and `pub name: String` (`rules.rs:20`). The evaluator iterates `rules: &[Rule]` (`direct_eval.rs:54`) / `self.rules: Vec<Rule>` (`rete/mod.rs:362`). Build an `id → name` HashMap once before the recording loops (see §Code Examples).

## `new()` Deprecation + Test-Migration Plan (D-04)

**Call sites of `RuleExecutionTracker::new()` (verified):**
- `api/src/metrics.rs:380` — production (THE bug) — rewrite to singleton read.
- `engine/src/monitoring/detection.rs:276,286,297,311,334,350` — 6 in-crate tests.

**Recommended mechanism:** `#[deprecated(note = "RuleExecutionTracker is a singleton; use associated functions + clear() for test isolation.")]` on `new()`. Rationale: emits a compile warning catching any future stray local instantiation while preserving test ergonomics during migration. `#[cfg(test)]`-gating is more aggressive but would require rewriting all 6 tests in the same commit; `#[deprecated]` allows incremental migration and is the lowest-risk option (D-04 contract: "no production code instantiates a local `RuleExecutionTracker`").

**Test-migration path (the 6 `detection.rs` tests):**
1. Convert each test to `#[serial_test::serial]` (L-05).
2. Start each test with `RuleExecutionTracker::clear();` to isolate from prior tests.
3. Replace `let mut tracker = RuleExecutionTracker::new(); tracker.record_evaluation(...)` with `RuleExecutionTracker::record_evaluation(...)` (associated function).
4. Replace `tracker.find_dead_rules()` with `RuleExecutionTracker::find_dead_rules()` (owned `Vec<DeadRuleInfo>`).
5. Replace `tracker.get_rule_info(id)` reads with a new associated function `RuleExecutionTracker::get_rule_info(id) -> Option<DeadRuleInfo>` (owned, mirrors `CoverageTracking::get_rule_state` at `coverage.rs:121-124`).
6. The `find_truly_inactive_rules` test at `detection.rs:311` directly inserts into `tracker.rules` (private field) — this test must be rewritten to use the public API (e.g., `record_evaluation` only, no `record_fire`) since the singleton's `rules` field is no longer externally accessible. Planner note: this is the one test that cannot be a mechanical migration.

**Planner may instead gate a test-only constructor behind `#[cfg(test)]` if the `find_truly_inactive_rules` test's direct-field manipulation is essential — D-04 explicitly allows this (Claude's discretion).**

## Parallel-Systems Boundary (D-05)

| Parallel system | Location | Struct | Wired to execute path? | Phase 26 action |
|-----------------|----------|--------|------------------------|-----------------|
| Dead-rule health registry | `engine/src/monitoring/dead_rules.rs:119` | `HealthRegistry` (NOT `RuleHealthRegistry` — C2) | **No** — `grep` finds zero callers in `engine/src/` or `api/src/` | Document, do not merge. |
| Coverage history tracker | `engine/src/coverage/history/mod.rs` | `CoverageTracker` (separate from `CoverageTracking` singleton) | **No** — no callers outside its own module/tests | Document, do not merge. |
| Coverage tracker (third copy) | `engine/src/coverage/mod.rs:26` | `CoverageTracker` | **No** — only in its own tests | Document, do not merge. |

**Recommendation:** Leave all three untouched. Phase 26 unifies only the three ROADMAP-named counters (`CoverageTracking` + `RuleExecutionTracker` + per-engine `EngineMetrics` — and the last is out-of-contract per C4). Merging these parallel systems is a separate consolidation phase. The planner must NOT double-wire dead-rule detection by accidentally calling into `HealthRegistry` or `coverage/history`.

## E2E Keystone Test Design (D-07)

**File:** `api/tests/execute_integration.rs` (extend — the Phase 23/24/25 keystone pattern is already here).

**Scenario (SC-1 + SC-2):**
1. Author two rules into the shared `RuleSet`:
   - `r1`: condition `score == 0`, action `score := score + 10` (the existing `score_plus_10_rule` fixture at `execute_integration.rs:54`).
   - `r2`: condition `score == 999` (never matches the inserted fact), no actions or a no-op action — does NOT fire.
2. `RuleExecutionTracker::clear(); CoverageTracking::clear();` before the test (singleton isolation, `#[serial]`).
3. `POST /v1/execute` with a fact `{ scorecard: { score: 0 } }` and `engine: "rete"` (default — also covers the C1 Rete-path correction).
4. Assert `GET /v1/metrics/dead-rules` returns a list that **excludes `r1`** AND **includes `r2`** (the `r2` inclusion is the real proof — `[]`-excludes-`r1` is trivially true today, so without `r2` the test would pass against the broken code).
5. Assert `GET /v1/metrics/coverage` shows `r1.fired == true` + `r1.evaluation_count >= 1` and `r2.fired == false`.

**SC-3 (three-surfaces agreement):**
6. After the run, assert the fire count for `r1` is consistent across:
   - `GET /metrics` (Prometheus): parse `rrules_execute_rules_fired_total{engine="rete"}` value ≥ 1.
   - `GET /v1/metrics/coverage`: `r1.fire_count >= 1`.
   - `GET /v1/metrics/dead-rules`: `r1` excluded ⇒ `r1.fire_count > 0`.
   All three derive from `fired_set`; the single-source-of-truth design (D-02) makes this pass by construction. If drift is reintroduced, the test fails.

**Test layers:**
- **Unit (`engine/tests/detection.rs` or in-crate `detection.rs` tests):** singleton API — `record_evaluation`/`record_fire` increment counts; `find_dead_rules` returns owned `Vec<DeadRuleInfo>`; `clear()` resets; once-per-rule (call twice with same id → `evaluation_count == 2`, not `== number of inner-loop iterations`).
- **Unit:** `find_truly_inactive_rules` rewritten to public API (D-04 migration).
- **Integration (`api/tests/execute_integration.rs`):** the keystone e2e above (SC-1/SC-2/SC-3).
- **Regression:** update `api/tests/metrics_integration.rs:91` `dead_rules_returns_valid_json_array` — the comment "always returns []" is now stale; either upgrade it to assert non-empty after an execute, or leave it (it still passes — valid JSON array).

**Wave 0 RED→GREEN:** Write the keystone RED first. The RED scaffold asserts dead-rules **includes `r2`** — today this FAILS (dead-rules returns `[]`, which excludes `r2`), proving the panel is broken. After implementation, GREEN: dead-rules includes `r2` and excludes `r1`.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `chrono::Utc::now().to_rfc3339()` is the right timestamp format for `last_fired_at` | Singleton Design | Low — format is Claude's discretion (D-06); `dead_rules.rs` already uses RFC3339-style `DateTime<Utc>`. |
| A2 | `#[deprecated]` on `new()` is preferable to `#[cfg(test)]`-gating | new() Deprecation | Low — D-04 explicitly allows either; planner may switch. |
| A3 | `EngineMetrics.rules_fired` has no live endpoint reader | Source-of-Truth (C4) | Low — verified by grep; `get_metrics()` trait exists but no HTTP handler calls it. If a future Phase 24 surface reads it, the out-of-contract designation should be revisited. |

All other claims are `[VERIFIED: live code]` with file:line evidence.

## Open Questions (RESOLVED)

1. **`find_truly_inactive_rules` test migration** — RESOLVED: Plan 26-01 Task 1 rewrites the test to the public API under the singleton model (record_evaluation only, no private-field manipulation). — the existing test (`detection.rs:311`) directly manipulates the private `rules` field, which is impossible against a singleton. Planner must decide: rewrite to public API (`record_evaluation` only, no `record_fire`), or add a `#[cfg(test)]` test-only constructor that exposes field manipulation. Recommendation: rewrite to public API — the test's intent (a rule with 0 evals + 0 fires is "truly inactive") is expressible as "never call `record_evaluation` or `record_fire` for that id, then call `find_truly_inactive_rules`" — but the singleton only knows about rules it has been told about, so this test may need to assert behavior for a rule that was never recorded. Planner should clarify whether `find_truly_inactive_rules` is still meaningful under the singleton model (it can only find rules that were `record_evaluation`-ed but never `record_fire`-d).
2. **`set_rule_category` wiring** — `detection.rs:116` has a `set_rule_category` method but no call site feeds it. RESOLVED: Plan 26-01 Task 2 keeps `set_rule_category` on the singleton API (retained, unused — the keystone does not assert on `category`).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | All code | ✓ | (workspace) | — |
| `chrono` | `last_fired_at` timestamp | ✓ | workspace dep | — |
| `serial_test` (dev-dep) | singleton test isolation | ✓ | workspace dev-dep | — |
| `prometheus` (dev/runtime) | `/metrics` endpoint (unchanged) | ✓ | workspace dep | — |

No missing dependencies. No fallback needed.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `serial_test` (dev-dep) + `actix_web::test` for HTTP integration |
| Config file | `Cargo.toml` (workspace) — per-crate `cargo test -p <crate>` |
| Quick run command | `cargo test -p engine --lib monitoring::detection` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MON-06 (SC-1) | dead-rules excludes fired `r1`, includes unfired `r2` | integration (e2e) | `cargo test -p api --test execute_integration keystone_dead_rules -- --serial` | ❌ Wave 0 (extend existing file) |
| MON-06 (SC-2) | coverage shows `r1.fired == true`, `r2.fired == false` | integration (e2e) | `cargo test -p api --test execute_integration keystone_coverage` | ❌ Wave 0 |
| MON-06 (SC-3) | three surfaces agree on fire count | integration (e2e) | `cargo test -p api --test execute_integration keystone_three_surfaces_agree` | ❌ Wave 0 |
| MON-06 (unit) | singleton `record_evaluation`/`record_fire`/`find_dead_rules`/`clear` | unit | `cargo test -p engine --lib monitoring::detection` | ✅ (existing tests, rewrite to singleton API) |
| MON-06 (unit) | once-per-rule: recording from `fired_set`/`evaluated_set` does not double-count | unit | `cargo test -p engine --lib monitoring::detection::tests::test_once_per_rule` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p engine --lib monitoring::detection && cargo test -p api --test execute_integration`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full workspace suite green before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `api/tests/execute_integration.rs` — add `keystone_dead_rules`, `keystone_coverage`, `keystone_three_surfaces_agree` tests (SC-1/SC-2/SC-3) — RED first.
- [ ] `engine/src/monitoring/detection.rs` tests — rewrite the 6 existing tests to singleton API + `#[serial]` + `clear()`; add `test_once_per_rule`.
- [ ] `api/tests/metrics_integration.rs:91` — update the stale "always returns []" comment (optional: upgrade assertion to non-empty after execute).

*(No framework install needed — `serial_test` already a workspace dev-dep.)*

## Security Domain

This phase is internal wiring with no authentication, authorization, input-validation, or cryptography surface changes. The dead-rules and coverage endpoints are existing unauthenticated read-only dashboard routes (registered in Phase 24); Phase 26 only changes what they read, not their auth posture.

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | unchanged (out of scope) |
| V3 Session Management | no | unchanged |
| V4 Access Control | no | unchanged (read-only dashboard) |
| V5 Input Validation | no | no new inputs (singleton read paths) |
| V6 Cryptography | no | no crypto changes |

**Known threat patterns:** none new. The singleton `Mutex` lock is held briefly (HashMap insert/clone); no panic-on-poison concerns beyond the existing `CoverageTracking` pattern (which uses `.lock().unwrap()` consistently — `coverage.rs:98,106,122,128,135,161,168,210`).

## Sources

### Primary (HIGH confidence — verified against live code)
- `engine/src/monitoring/detection.rs:64-191` — `RuleExecutionTracker` struct + methods to convert.
- `engine/src/monitoring/coverage.rs:83-158` — `CoverageTracking` singleton pattern to mirror.
- `engine/src/engine/direct_eval.rs:53-113` — shared direct evaluator (Treat/Leaps/Phreak path); recording at 105-110.
- `engine/src/engine/rete/mod.rs:340-406` — Rete inlined `execute_rules` (C1 correction); recording at 393-398.
- `engine/src/engine/{treat,leaps,phreak}/mod.rs:383,384,570` — `direct_execute` callers (confirms delegation).
- `engine/src/engine/{rete,treat,leaps,phreak}/mod.rs:381,387,309,388,574` — `self.metrics.record_rule_fired()` sites (out-of-contract per C4).
- `api/src/metrics.rs:378-405` — `dead_rules_endpoint` (the bug) + `coverage_endpoint` (the pattern).
- `api/src/v1/execution.rs:637-647` — `rules_fired_total.inc_by(engine_result.rules_fired)` (SC-3 Prometheus origin).
- `engine/src/engine/metrics.rs:12` — `EngineMetrics.rules_fired` (out-of-contract gauge).
- `engine/src/monitoring/dead_rules.rs:119` — `HealthRegistry` (parallel, unwired — C2).
- `engine/src/coverage/history/mod.rs` + `engine/src/coverage/mod.rs:26` — parallel `CoverageTracker` structs (unwired).
- `engine/src/rules.rs:16-20` — `Rule { pub id: String, pub name: String, ... }` (rule_name sourcing).
- `api/tests/execute_integration.rs:1-60` — existing keystone test pattern (Wave 0 RED→GREEN + `#[serial]`).
- `api/tests/metrics_integration.rs:91` — existing dead-rules structural test (stale comment).

### Secondary (MEDIUM confidence)
- `.planning/research/PITFALLS.md` §Pitfall 3, §Pitfall 6 — drift / stale-tracker descriptions.
- `.planning/ROADMAP.md` §"Phase 26" — success criteria SC-1/SC-2/SC-3.
- `.planning/phases/26-.../26-CONTEXT.md` — locked decisions D-01..D-07 (with C1/C2/C4 corrections flagged).

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new deps; all verified in live `Cargo.toml`/code.
- Architecture (singleton + call sites): HIGH — verified against live code; one major correction (C1: Rete inlined path).
- Source-of-truth unification: HIGH — verified `ExecutionCounters.rules_fired_total` origin (`execution.rs:637`) and `EngineMetrics` out-of-contract status.
- Pitfalls: HIGH — Pitfall 3 (`metrics.rs:380`) and Pitfall 6 (three-counter) verified; C1 (missing Rete site) is a newly identified pitfall.
- Parallel-systems boundary: HIGH — `HealthRegistry` and `CoverageTracker` grep-confirmed unwired.

**Corrections to propagate:** C1 (Rete inlined path — update CONTEXT L-02/D-03 and ROADMAP call-site refs), C2 (`HealthRegistry` not `RuleHealthRegistry`), C3 (ROADMAP line refs stale), C4 (`EngineMetrics` out-of-contract — refine D-02). The planner should reflect C1 in task design (two recording sites, not one).

**Research date:** 2026-07-03
**Valid until:** 2026-08-02 (30 days — stable internal-wiring domain; no external API surface)

## RESEARCH COMPLETE

**Phase:** 26 - RuleExecutionTracker singleton + dead-rule live data
**Confidence:** HIGH

### Key Findings
- **C1 correction (load-bearing):** Rete keeps its own inlined `execute_rules` (`rete/mod.rs:340-406`) — it does NOT delegate to `direct_eval.rs`. `RuleExecutionTracker` recording must be added in BOTH `direct_eval.rs:105-110` AND `rete/mod.rs:393-398`. CONTEXT L-02/D-03's "all four engines delegate" claim is wrong for Rete.
- **C4 refinement (D-02):** `EngineMetrics.rules_fired` is NOT a source for any SC-3 surface. The Prometheus fire count comes from `ExecutionCounters.rules_fired_total` (`execution.rs:637`), fed by `engine_result.rules_fired` (`fired_set.len()`). `EngineMetrics` is out-of-contract — leave as a per-engine gauge.
- **C2 correction:** The parallel dead-rule system is `HealthRegistry` (`dead_rules.rs:119`), not `RuleHealthRegistry`. Grep-confirmed unwired. Leave untouched (D-05).
- Singleton conversion is a direct mirror of `CoverageTracking` (`coverage.rs:86`); `find_dead_rules` returns owned `Vec<DeadRuleInfo>`; `last_fired_at` stamped internally via `chrono::Utc::now().to_rfc3339()` (D-06).
- Keystone e2e uses two rules (`r1` fires, `r2` unfired) — the `r2` inclusion in dead-rules is the RED→GREEN proof that the panel is actually live.

### File Created
`/home/threaded/projects/rRules/.planning/phases/26-ruleexecutiontracker-singleton-dead-rule-live-data/26-RESEARCH.md`

### Confidence Assessment
| Area | Level | Reason |
|------|-------|--------|
| Standard Stack | HIGH | No new deps; all verified in live code. |
| Architecture | HIGH | Singleton pattern + call sites verified; C1 correction flagged. |
| Pitfalls | HIGH | Pitfall 3 + 6 verified; C1 (missing Rete site) newly identified. |
| Source-of-Truth | HIGH | `ExecutionCounters` origin and `EngineMetrics` out-of-contract status grep-verified. |

### Open Questions
1. `find_truly_inactive_rules` test migration (private-field access impossible against singleton) — planner decides rewrite vs `#[cfg(test)]` constructor.
2. `set_rule_category` has no callers — keep on singleton API or mark unused.

### Ready for Planning
Research complete. Planner can now create PLAN.md files — must account for the C1 correction (two recording sites: `direct_eval.rs` AND `rete/mod.rs`).