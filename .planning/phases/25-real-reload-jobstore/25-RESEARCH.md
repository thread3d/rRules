# Phase 25: Real reload + JobStore - Research

**Researched:** 2026-07-03
**Domain:** Rust actix-web hot-reload + async job tracking + batch truthful aggregation (brownfield internal-wiring)
**Confidence:** HIGH (every claim verified by reading the actual rRules codebase with `file:line` citations)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **L-01:** Keep existing `Arc<RwLock<RuleSet>>` (`api/src/main.rs:120`), swap-in-place under the write lock. **No `ArcSwap` dependency.**
- **L-02:** `EngineState::reload_engine` no-op (`api/src/v1/execution.rs:151-195`, acquire-and-drop write lock at `:183-186`) → real build-then-swap.
- **L-03:** Async reload writes `ReloadJob{Pending → Running → Completed/Failed}` to the existing `JobStore` (`execution.rs:259-264`) so `GET /v1/jobs/{id}` works (no more 404).
- **L-04:** `ExecuteResponse.engine_generation: u64` (`execution.rs:64`, hardcoded `0u64` at `:532` and `:626`) → stamped from the shared ruleset's generation on every execute, incremented on each successful reload.
- **L-05:** Per-request isolation preserved — `execute_single` clones the ruleset under a short read lock and releases before `web::block` (`execution.rs:387-401`).
- **L-06:** Batch per-item isolation already holds; panic → `JoinError` → `failed` already correct (`execution.rs:648-655`).

### Claude's Discretion (auto-selected — research the HOW)
- **D-01:** Reload in-flight = **generation-accept** (swap immediately, tag generation). Pitfall 4 option (b). No drain, no in-flight counter on the hot execute path.
- **D-02:** Generation source-of-truth = **inside the shared ruleset payload**, read atomically under the existing read lock. NOT a separate `AtomicU64` (races). NOT the per-engine `generation` field (per-instance, always 0).
- **D-03:** Batch truthful aggregation = propagate `execute_single` `Err` as per-item FAILURE (`failed += 1`, `success:false`, `error:Some`, `response:None`). JoinSet task returns `Result<ExecuteResponse, String>`. NOT whole-batch 400.
- **D-04:** BOTH sync + async reload swap via ONE shared `reload_and_swap(&shared, &rules_dir) -> Result<ReloadResult, String>` helper.
- **D-05:** Full-directory reload reusing `build_ruleset_from_dir(&RULE_STORAGE_PATH)`; `rule_file` deferred (field stays, not honored).

### Deferred Ideas (OUT OF SCOPE)
- File watcher (`start_file_watcher` stub, Pitfall 13) — request-triggered reload only.
- Single-file / partial reload (`ReloadRequest.rule_file`) — `rule_file` field stays but ignored.
- `RuleExecutionTracker` singleton + dead-rule live data — Phase 26.
- Stateful sessions — Phase 27.
- Frontend "Reload then Run" UI + `getJobStatus` polling — Phase 28.
- `inference_trace` population (EXEC-04) — Phase 28.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DEPLOY-05 | Hot reload of rules without restart | `reload_and_swap` helper (D-04) + real `build_ruleset_from_dir` parser + JobStore transitions (L-03). Async reload 202 → poll `GET /v1/jobs/{id}` Pending→Running→Completed → next execute reflects edited rules. |
| EXEC-03 | Batch execution with truthful per-item aggregation | D-03: JoinSet task returns `Result<ExecuteResponse, String>`; `Err` → `failed += 1`/`success:false` instead of the current zero-fired-success mask (`execution.rs:618-628`). |
</phase_requirements>

## Summary

Phase 25 makes three honest: (1) **reload** actually rebuilds a `RuleSet` from disk and swaps it into the shared `Arc<RwLock<…>>` under the write lock, bumping a generation counter; (2) **`GET /v1/jobs/{id}`** returns real `Pending → Running → Completed/Failed` transitions because the spawned reload task writes to the already-existing `JobStore`; (3) **batch-execute** aggregates `succeeded`/`failed`/`total_fired` truthfully by propagating `execute_single`'s `Err` as a per-item failure instead of masking it as a zero-fired success. The shared ruleset is wrapped in a new `SharedRuleSet { ruleset: RuleSet, generation: u64 }` inside the existing `Arc<RwLock<…>>` so generation is read atomically with the rules under one lock. A critical prerequisite discovered during research: `build_ruleset_from_dir` (`api/src/main.rs:44-71`) currently returns an **empty** `RuleSet` (the live parser is explicitly deferred to Phase 25), so Phase 25 must also implement the real JSON rule-file parser inside that helper — otherwise reload swaps one empty ruleset for another and the keystone scenario cannot pass.

**Primary recommendation:** Define `SharedRuleSet` in the API layer, implement `build_ruleset_from_dir` as a JSON `.rules`-file deserializer (all rule AST types already derive `Serialize, Deserialize`), extract one `reload_and_swap` helper used by both sync and async paths, change the JoinSet task to return `Result<ExecuteResponse, String>`, and stamp `engine_generation` from the shared snapshot.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Rule-file parsing (disk → RuleSet) | API server (startup + reload) | — | `build_ruleset_from_dir` runs at startup and on reload; the engine crate's `RuleSet` is a pure data model with no I/O. |
| Compiled ruleset storage (shared snapshot) | API server (`web::Data`) | — | `Arc<RwLock<SharedRuleSet>>` built once at `main.rs`, injected into execute/reload/batch handlers. |
| Ruleset swap (reload) | API server (reload handler) | — | `reload_and_swap` builds off-to-side, swaps under write lock. NOT the engine crate; the shared `EngineState.engine` is legacy and unused by the execute path. |
| Generation tagging | API server (execute_single) | — | Read atomically with the ruleset under the read lock; stamped into `ExecuteResponse.engine_generation`. |
| Per-request execution | API server (execute_single) | engine crate (ReteEngine) | `execute_single` clones the snapshot into a fresh `Box<dyn ExecutionEngine>` and runs in `web::block`. |
| Async job tracking | API server (JobStore) | — | `JobStore = Arc<RwLock<HashMap>>` already wired; spawned task writes transitions. |
| Batch aggregation | API server (batch_execute_rules) | — | JoinSet + Semaphore; collector branches `Ok`/`Err` per item. |

## Standard Stack

No new dependencies. Phase 25 is pure internal wiring against the existing stack.

### Core (already in the project — verified)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `actix-web` | existing | HTTP handlers, `web::Data`, `web::block` | Already used by all v1 handlers `[VERIFIED: codebase api/src/main.rs]` |
| `tokio` | existing | `JoinSet`, `Semaphore`, `spawn` for async reload | Already used by `batch_execute_rules` (`execution.rs:591-592`) `[VERIFIED: codebase]` |
| `serde_json` | existing | Deserialize `.rules` JSON files into `Rule` AST | `Rule`/`Condition`/`Action`/`Expression`/`ComparisonOp` all derive `Serialize, Deserialize` (`engine/src/rules.rs:15,116,148,230,417`) `[VERIFIED: codebase engine/src/rules.rs]` |
| `std::sync::{Arc, RwLock}` | std | Shared ruleset + JobStore | L-01 locks `Arc<RwLock<…>>`; no `ArcSwap` `[VERIFIED: codebase api/src/main.rs:120]` |
| `uuid` | existing | Job IDs, execution IDs | Already used (`execution.rs:12`) `[VERIFIED: codebase]` |
| `chrono` | existing | `ReloadResult` timestamps | Already used (`execution.rs:4`) `[VERIFIED: codebase]` |

**Installation:** `cargo build` — no `cargo add` needed.

## Package Legitimacy Audit

Not applicable — Phase 25 installs **zero** external packages. All work is against the existing workspace deps (`actix-web`, `tokio`, `serde_json`, `uuid`, `chrono`, `std`). No `npm view` / `pip index` / `cargo search` verification required.

## Architecture Patterns

### System Architecture Diagram

```
                          POST /v1/rules/reload {async:true}
                                     │
                                     ▼
                        reload_rules handler
                        │                │
              (sync) inline            (async) insert Pending → JobStore
                        │                │ spawn task
                        ▼                ▼
              ┌─ reload_and_swap(&shared, &rules_dir) ─────────┐
              │  1. build_ruleset_from_dir(RULE_STORAGE_PATH)   │
              │     → read each .rules JSON file                │
              │     → serde_json::from_str::<Rule>              │
              │     → RuleSet::add_rule                         │
              │  2. shared.write() →                            │
              │       *shared = SharedRuleSet {                 │
              │         ruleset: new,                           │
              │         generation: old.generation + 1,         │
              │       }                                         │
              │  3. return ReloadResult                          │
              └─────────────────────────────────────────────────┘
                        │ (async: Running → Completed/Failed → JobStore)
                        ▼
                GET /v1/jobs/{id} → ReloadJob from JobStore
                                     (no more 404)

  POST /v1/execute         POST /v1/rules/batch-execute
        │                          │
        ▼                          ▼
  execute_single              JoinSet task per item
        │                          │
        ▼                          ▼
  shared.read() ──► (ruleset.clone(), generation)   ← atomic under ONE read lock
        │                          │
        release guard              release guard
        ▼                          ▼
  load_ruleset(clone)         load_ruleset(clone)
  web::block(execute)         web::block(execute)
        │                          │
        ▼                          ▼
  ExecuteResponse {           Result<ExecuteResponse, String>
    engine_generation: gen      Ok → succeeded += 1, success:true
  }                            Err → failed += 1, success:false, error:Some
                               JoinError → failed += 1 (already correct)
```

### Recommended Project Structure
```
api/src/
├── main.rs                    # build_ruleset_from_dir → real parser; SharedRuleSet wiring
├── v1/
│   └── execution.rs           # SharedRuleSet struct, reload_and_swap, reload_rules rewrite,
│                              # execute_single generation read, batch ExecuteResult propagation
└── tests/
    └── execute_integration.rs # SC-1/SC-2/SC-3 + reload + jobs tests
```

### Pattern 1: SharedRuleSet — atomic ruleset+generation under one lock (D-02)
**What:** Wrap the shared state so the ruleset and generation are read/written atomically under the existing `RwLock`. No separate `AtomicU64` (it would race — the counter could bump between reading the ruleset and reading the counter).
**When to use:** Any system where a snapshot is swapped and consumers must know which generation they saw.
**Recommendation over alternatives:**
- **vs. `RwLock<(RuleSet, u64)>` tuple:** a named struct is clearer at the call site (`g.ruleset`, `g.generation`) than `.0`/`.1` and self-documents the invariant.
- **vs. adding `generation` to `RuleSet` (engine crate):** `RuleSet` is a pure domain model in the `engine` crate; reload-generation is an API-layer concurrency concern. Adding it would conflate two different "generation" concepts (the per-engine `generation: usize` at `engine/src/engine/rete/mod.rs:23` is per-instance and always 0 — a DIFFERENT field). Keeping the wrapper in the API layer preserves the layer boundary.

```rust
// Source: derived from execution.rs:95-98 (EngineState), main.rs:120 (shared ruleset)
/// Shared reloadable ruleset + generation counter (D-02). The generation is
/// read atomically with the ruleset under the existing RwLock — a separate
/// AtomicU64 would race (counter could bump between ruleset read and counter
/// read). Reload swaps the whole payload under the write lock and bumps
/// generation; execute reads both under the read lock, clones the ruleset,
/// releases, and stamps generation into ExecuteResponse.
#[derive(Debug, Clone)]
pub struct SharedRuleSet {
    pub ruleset: RuleSet,
    pub generation: u64,
}

impl SharedRuleSet {
    pub fn new(ruleset: RuleSet) -> Self {
        Self { ruleset, generation: 0 }
    }
}

// main.rs wiring (replaces main.rs:119-120):
let shared = web::Data::new(std::sync::Arc::new(std::sync::RwLock::new(
    SharedRuleSet::new(build_ruleset_from_dir(&rules_dir)),
)));
```

### Pattern 2: reload_and_swap — one helper for sync + async (D-04)
**What:** A single function builds a new `RuleSet` off-to-side, then swaps under the write lock with generation bump. Both reload paths call it so they cannot diverge.
**When to use:** Whenever two code paths must perform the same mutation.

```rust
// Source: derived from execution.rs:151-195 (reload_engine no-op to replace)
// and main.rs:44 (build_ruleset_from_dir)
pub async fn reload_and_swap(
    shared: &std::sync::Arc<std::sync::RwLock<SharedRuleSet>>,
    rules_dir: &str,
) -> Result<ReloadResult, String> {
    let start_time = chrono::Utc::now();
    // 1. Build fully off to the side (no lock held during I/O + parse).
    let new_ruleset = build_ruleset_from_dir(rules_dir); // real parser (see below)
    let rule_count = new_ruleset.rules.len();
    // 2. Swap under the write lock — single assignment, bump generation.
    let reload_id = {
        let mut guard = shared
            .write()
            .map_err(|e| format!("shared ruleset lock poisoned: {}", e))?;
        let next_gen = guard.generation.wrapping_add(1);
        let id = uuid::Uuid::new_v4().to_string();
        *guard = SharedRuleSet {
            ruleset: new_ruleset,
            generation: next_gen,
        };
        id
    };
    Ok(ReloadResult {
        reload_id,
        rule_count,
        start_time,
        end_time: chrono::Utc::now(),
        success: true,
    })
}
```
**Note:** `reload_and_swap` is `async` only for parity with the existing `reload_engine`/`reload_from_directory` signatures and the tokio spawn; the body is sync I/O + a lock. It can be called from both the sync handler path (`.await`) and the spawned async task (`.await`).

### Pattern 3: build_ruleset_from_dir — real JSON parser (CRITICAL prerequisite)
**What:** The startup helper (`main.rs:44-71`) currently returns `RuleSet::new()` even when rule files exist (it logs "live parsing is Phase 25 scope" at `:66-69`). Phase 25 MUST make it actually parse, otherwise reload swaps one empty ruleset for another and the keystone scenario (edit rule → reload → execute sees the edit) is impossible.
**Why this is safe now:** `Rule`, `Condition`, `Action`, `Expression`, `ComparisonOp` all derive `Serialize, Deserialize` (`engine/src/rules.rs:15,116,148,230,417`). Each `.rules` file is one JSON `Rule` object (or a JSON array of `Rule`s). `serde_json::from_str` + `RuleSet::add_rule` is the entire parser.

```rust
// Source: rewrites main.rs:44-71
fn build_ruleset_from_dir(rules_dir: &str) -> RuleSet {
    let path = std::path::Path::new(rules_dir);
    if !path.exists() {
        tracing::warn!(rules_dir = %rules_dir, "rules directory not found, starting with no rules");
        return RuleSet::new(); // D-03a preserved
    }
    let mut ruleset = RuleSet::new();
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(rules_dir = %rules_dir, error = %e, "failed to read rules dir, starting with no rules");
            return RuleSet::new(); // D-03a: warn + continue, do not crash
        }
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !(name.ends_with(".rules") || name.ends_with(".drl")) { continue; }
        let content = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(file = %name, error = %e, "skipping unreadable rule file");
                continue; // D-03a: one bad file does not crash startup/reload
            }
        };
        // Try single Rule first, then Vec<Rule> (supports both shapes).
        let rules: Vec<Rule> = match serde_json::from_str::<Rule>(&content) {
            Ok(r) => vec![r],
            Err(_) => match serde_json::from_str::<Vec<Rule>>(&content) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(file = %name, error = %e, "skipping unparseable rule file");
                    continue;
                }
            },
        };
        for rule in rules {
            if let Err(e) = ruleset.add_rule(rule) {
                tracing::warn!(error = %e, "skipping rule that failed validation");
            }
        }
    }
    if ruleset.rules.is_empty() {
        tracing::warn!(rules_dir = %rules_dir, "no parseable rule files found, starting with no rules");
    }
    ruleset
}
```

### Pattern 4: execute_single generation read (L-04/L-05)
```rust
// Source: rewrites execution.rs:387-401 (read block) and :526-533 (response)
// Read generation atomically with the ruleset under the existing read lock.
let (all_rule_ids, generation): (Vec<String>, u64) = {
    let guard = ruleset.read().map_err(|e| {
        HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("ruleset lock poisoned: {}", e)
        }))
    })?;
    engine.load_ruleset(&guard.ruleset).map_err(|e| {
        HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("load_ruleset failed: {}", e)
        }))
    })?;
    let ids = guard.ruleset.rules.iter().map(|r| r.id.clone()).collect();
    (ids, guard.generation)
};
// ... (execute in web::block unchanged) ...
Ok(ExecuteResponse {
    execution_id,
    rules_fired: engine_result.rules_fired,
    rules_not_fired,
    facts_after,
    inference_trace: None,
    engine_generation: generation, // was 0u64 at :532
})
```
**Handler signature change:** `execute_single`, `execute_rules`, and `batch_execute_rules` change `ruleset: web::Data<Arc<RwLock<RuleSet>>>` → `ruleset: web::Data<Arc<RwLock<SharedRuleSet>>>` (the field access becomes `guard.ruleset` everywhere).

### Pattern 5: Batch truthful aggregation (D-03)
```rust
// Source: rewrites execution.rs:606-629 (JoinSet task) and :637-657 (collector)
// JoinSet task returns Result<ExecuteResponse, String> instead of ExecuteResponse.
join_set.spawn(async move {
    let _permit = permit_sem.acquire_owned().await
        .expect("batch semaphore closed unexpectedly");
    // D-03: propagate Err as a per-item failure, NOT a zero-fired placeholder.
    execute_single(request, registry, selector, ruleset, counters, metrics_tx)
        .await
        .map_err(|http_response| {
            // Extract the error message from the HttpResponse body.
            // execute_single returns Err(HttpResponse::BadRequest().json({"error": ...}))
            // for unknown engine (ENGINE-07) and Err(InternalServerError) for engine
            // creation / lock failures. Surface the message so the caller sees WHY.
            "execution failed".to_string() // (planner: extract body string; see Risks)
        })
});

// Collector (replaces :637-657):
while let Some(join_result) = join_set.join_next().await {
    match join_result {
        Ok(Ok(resp)) => {                       // task returned Ok(ExecuteResponse)
            total_fired += resp.rules_fired;
            succeeded += 1;
            results.push(BatchItemResult { success: true, response: Some(resp), error: None });
        }
        Ok(Err(msg)) => {                       // task returned Err(String) — D-03 fix
            failed += 1;
            results.push(BatchItemResult { success: false, response: None, error: Some(msg) });
        }
        Err(join_err) => {                      // task panicked — already correct (:648-655)
            failed += 1;
            results.push(BatchItemResult { success: false, response: None, error: Some(join_err.to_string()) });
        }
    }
}
```
**Exact lines to change in `batch_execute_rules`:** the task closure `:606-629` (return `Result<ExecuteResponse, String>`) and the collector `:637-657` (three-arm match). The `Ok(resp)` arm at `:639-646` becomes `Ok(Ok(resp))`; the `Err(join_err)` arm at `:648-655` stays `Err(join_err)`; a NEW `Ok(Err(msg))` arm handles the D-03 failure path.

### Pattern 6: Async reload JobStore transitions (L-03/D-01)
```rust
// Source: rewrites execution.rs:685-696 (async branch of reload_rules)
// reload_rules signature adds: shared: web::Data<Arc<RwLock<SharedRuleSet>>>,
//                                job_store: web::Data<JobStore>
if req.async_reload {
    let job_id = uuid::Uuid::new_v4().to_string();
    // 1. Insert Pending BEFORE spawn so a rapid poll sees it (SC-1).
    {
        let mut jobs = job_store.write().unwrap();
        jobs.insert(job_id.clone(), ReloadJob {
            job_id: job_id.clone(),
            result: None,
            status: ReloadStatus::Pending,
            error: None,
        });
    }
    let shared_clone = shared.clone();   // Arc clone
    let store_clone = job_store.clone(); // Arc clone
    let dir = rules_dir.clone();
    tokio::spawn(async move {
        // 2. Running.
        {
            let mut jobs = store_clone.write().unwrap();
            if let Some(job) = jobs.get_mut(&job_id) {
                job.status = ReloadStatus::Running;
            }
        }
        // 3. reload_and_swap → Completed or Failed.
        match reload_and_swap(&shared_clone, &dir).await {
            Ok(result) => {
                let mut jobs = store_clone.write().unwrap();
                if let Some(job) = jobs.get_mut(&job_id) {
                    job.status = ReloadStatus::Completed;
                    job.result = Some(result);
                }
            }
            Err(e) => {
                let mut jobs = store_clone.write().unwrap();
                if let Some(job) = jobs.get_mut(&job_id) {
                    job.status = ReloadStatus::Failed;
                    job.error = Some(e);
                }
            }
        }
    });
    HttpResponse::Accepted().json(JobResponse {
        job_id,
        status: "pending".to_string(), // D-01: matches the inserted Pending state
    })
} else {
    // Sync: swap inline, return ReloadResponse (D-04).
    match reload_and_swap(shared.get_ref(), &rules_dir).await {
        Ok(result) => HttpResponse::Ok().json(ReloadResponse {
            reload_id: result.reload_id,
            rules_reloaded: result.rule_count,
            status: format!("Successfully reloaded {} rules", result.rule_count),
        }),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "reload_id": uuid::Uuid::new_v4().to_string(),
            "rules_reloaded": 0,
            "status": format!("Reload failed: {}", e)
        })),
    }
}
```

### Anti-Patterns to Avoid
- **In-place mutation of the shared ruleset's `rules` Vec** — Pitfall 4. Build off-to-side and swap the whole `SharedRuleSet` value under the write lock (`*guard = SharedRuleSet { … }`), never `guard.ruleset.rules.push(...)`.
- **Separate `AtomicU64` for generation** — D-02 explicitly rejects this: the counter could bump between reading the ruleset and reading the counter, so a response could be stamped with a generation that doesn't match the rules it ran against.
- **Two divergent reload implementations** (sync inline build, async spawned build) — D-04: one `reload_and_swap` helper.
- **Masking `execute_single` `Err` as a zero-fired `ExecuteResponse`** — the current `execution.rs:618-628` bug; D-03 fixes it.
- **Leaving `build_ruleset_from_dir` returning empty** — reload would swap empty-for-empty; the keystone scenario fails. This is the most easily missed prerequisite.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Rule-file parsing | A custom DSL/text parser for `.rules`/`.drl` | `serde_json::from_str::<Rule>` (all AST types derive `Serialize, Deserialize`) | The AST already round-trips through serde; a text parser is hundreds of lines and a new bug surface. `[VERIFIED: engine/src/rules.rs:15]` |
| Atomic generation tracking | A separate `AtomicU64` + careful ordering | `SharedRuleSet { ruleset, generation }` under the existing `RwLock` | Coherence — the generation is read with the ruleset under one lock, no race window. `[VERIFIED: D-02]` |
| Job status enum / JobStore / handlers | New job-tracking infra | The existing `ReloadJob`/`ReloadStatus`/`JobStore`/`init_job_store`/`get_job_status`/`list_jobs` (`execution.rs:212-264,730-755`) | Already built and wired in `main.rs:272-273`; Phase 25 only writes to it. `[VERIFIED: codebase]` |
| Snapshot swap | A lock-free atomic pointer (`ArcSwap`) | `Arc<RwLock<SharedRuleSet>>` write-lock swap (L-01) | Read lock is held only microseconds (clone-then-release); no contention. No new dep. `[VERIFIED: L-01 + concurrency analysis below]` |

**Key insight:** The reload machinery is 80% already built (JobStore, ReloadJob, handlers, shared ruleset, ExecuteResponse.engine_generation field). Phase 25's job is to **connect** them — make `reload_engine` real, make the spawned task write JobStore, make `build_ruleset_from_dir` parse, and stamp the generation — not to build new infrastructure.

## Runtime State Inventory

Not applicable — this is a greenfield-wiring phase (new code paths + behavior changes), not a rename/refactor/migration. No stored data, OS-registered state, or secrets embed the phase's strings. The `JobStore` is an in-memory `HashMap` that resets on restart (no persistence to migrate).

## Common Pitfalls

### Pitfall A: build_ruleset_from_dir returns empty (MOST EASILY MISSED)
**What goes wrong:** D-05 says "reuse `build_ruleset_from_dir`" but that helper (`main.rs:44-71`) currently returns `RuleSet::new()` even when `.rules` files exist (logs "live parsing is Phase 25 scope" at `:66-69`). If Phase 25 wires `reload_and_swap` to call it without upgrading it, reload swaps one empty ruleset for another; `rules_fired` stays 0 after reload; the keystone SC-1 scenario cannot pass.
**Why it happens:** Phase 23 explicitly deferred the live parser to Phase 25 (`23-CONTEXT.md` D-03 note, `main.rs:50,63-69`). The CONTEXT.md D-05 wording ("reusing `build_ruleset_from_dir`") assumes the helper already parses; it does not.
**How to avoid:** Implement the real JSON parser in `build_ruleset_from_dir` (Pattern 3) as the FIRST task of Phase 25. Both startup AND reload then load real rules. The keystone test writes a `.rules` JSON file to a temp `RULE_STORAGE_PATH`.
**Warning signs:** `cargo test` SC-1 fails with `rules_fired == 0` after reload; `build_ruleset_from_dir` still contains the string "live parsing is Phase 25 scope".

### Pitfall B: Separate AtomicU64 generation races with the ruleset read
**What goes wrong:** A separate `AtomicU64` for generation can be bumped by a reload BETWEEN the moment `execute_single` reads the ruleset and the moment it reads the counter — the response is stamped with a generation that doesn't correspond to the rules it actually ran against.
**How to avoid:** D-02 — `SharedRuleSet { ruleset, generation }` under ONE `RwLock`; read both under the single read guard, clone the ruleset, copy the generation, release. Atomic and coherent.

### Pitfall C: In-place mutation causes torn reads (Pitfall 4)
**What goes wrong:** Mutating `guard.ruleset.rules` in place while an in-flight executor iterates the cloned snapshot is safe ONLY because the snapshot is a clone — but mutating in place during a read-lock window would tear. The contract is swap-don't-mutate.
**How to avoid:** `*guard = SharedRuleSet { ruleset: new, generation: next }` — a single whole-value assignment under the write lock.

### Pitfall D: JobStore insert races with the first poll
**What goes wrong:** If the spawned task is created before the `Pending` entry is inserted, a client polling `GET /v1/jobs/{id}` immediately after the 202 could get a 404 (the task hasn't run yet, but neither has the insert).
**How to avoid:** Insert `Pending` synchronously in the handler BEFORE `tokio::spawn` (Pattern 6, step 1).

### Pitfall E: Batch Err masking inflates succeeded (the current bug)
**What goes wrong:** `execution.rs:618-628` converts `execute_single` `Err` into a zero-fired `ExecuteResponse` and the collector (`:639-641`) counts it as `succeeded += 1` with `success: true`. A batch with one unknown-engine item reports `succeeded == N` instead of `succeeded == N-1, failed == 1`.
**How to avoid:** D-03 — task returns `Result<ExecuteResponse, String>`; collector has three arms (`Ok(Ok)`, `Ok(Err)`, `Err(JoinError)`).

## Concurrency Analysis

### Why generation-accept is safe (D-01 + L-05)
`execute_single` (`execution.rs:387-401`) takes the read lock, calls `engine.load_ruleset(&guard)` which clones the `rules` Vec (`rete/mod.rs:527-529`), collects `all_rule_ids`, and **drops the guard** before entering `web::block`. The per-request engine then executes against its private cloned snapshot. A concurrent reload's write lock only blocks the brief read-then-clone window (microseconds — a `Vec<Rule>::clone`), never the CPU-bound execute. An in-flight run that started before a reload finishes against the OLD snapshot; a run that starts after reads the NEW snapshot and generation. The generation tag lets callers detect which they saw (SC-3). No drain, no in-flight counter, no hot-path blocking.

### Why `Arc<RwLock<SharedRuleSet>>` over `ArcSwap` (validates L-01)
`ArcSwap` would let readers grab an `Arc` snapshot without a lock — useful when the read guard is held a long time. Here the read guard is held only for the `load_ruleset` clone (a `Vec<Rule>` shallow-clone of the rules — O(n_rules) but no compilation, no I/O, no CPU-bound work). Under actix's multi-worker model, reload is rare (operator-triggered) and execute is hot, but the read-lock hold time is dominated by the `Vec::clone`, which is sub-microsecond for typical ruleset sizes. `RwLock` write contention with reads is therefore negligible, and `ArcSwap` would add a dependency and an unfamiliar primitive for no measurable benefit. L-01 (keep `RwLock`) is correct.

### Write-lock swap atomicity
`*guard = SharedRuleSet { ruleset: new, generation: next_gen }` is a single `RwLock` write-guard assignment. `SharedRuleSet` is `Clone` but the assignment replaces the whole value; no reader can observe a half-swapped state because `RwLock` excludes readers while the write guard is held. The new `RuleSet` was built fully off-to-side before the lock was acquired, so no I/O or parsing happens under the write lock — it's held only for the pointer-sized assignment.

## Pitfall Closure

| Pitfall (PITFALLS.md) | Root cause (file:line) | Closure (Phase 25 change) |
|---|---|---|
| **Pitfall 4** (hot-reload race) | `reload_engine` acquire-and-drop write lock (`execution.rs:183-186`); no generation tag | `reload_and_swap` builds off-to-side + swaps whole `SharedRuleSet` + bumps generation (Pattern 2); generation-accept documented (D-01); `engine_generation` stamped on every response (Pattern 4). |
| **Pitfall 12** (`reload_engine` no-op) | `:183-186` acquires `self.engine.write()` and drops it without recompiling | `reload_engine` is replaced/deprecated by `reload_and_swap` operating on the shared `SharedRuleSet` (not the legacy `EngineState.engine`); `reload_rules` handler rewritten to call it (Pattern 6). |
| **Pitfall 14** (JobStore never written for async reload) | `:689-691` spawns `reload_engine` and discards the result (`let _result = ...`); never writes JobStore | Spawned task inserts `Pending` before spawn, transitions `Running` on start, `Completed`/`Failed` on `reload_and_swap` outcome (Pattern 6). `GET /v1/jobs/{id}` returns real transitions. |

**Cross-cutting (PITFALLS.md line 302):** "generation tag on every execution response" — closed by Pattern 4 (L-04).

## Validation Architecture

**Nyquist validation enabled** (`.planning/config.json` `workflow.nyquist_validation` absent → enabled).

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust built-in) + `actix_web::test::TestRequest` |
| Config file | `api/Cargo.toml` `[package]` + `[features]` (engine-rete enabled Phase 23) |
| Quick run command | `cargo test -p api --test execute_integration -x` |
| Full suite command | `cargo test -p api && cargo test -p engine` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DEPLOY-05 / SC-1 | edit rule → async reload → poll job Pending→Running→Completed → execute reflects edit + bumped generation | integration | `cargo test -p api --test execute_integration -- reload_then_execute_against_refreshed_rules -x` | ❌ Wave 0 (new) |
| DEPLOY-05 / SC-1 | `GET /v1/jobs/{id}` transitions through Pending→Running→Completed for async reload | integration | `cargo test -p api --test execute_integration -- async_reload_job_transitions -x` | ❌ Wave 0 (new) |
| SC-3 / L-04 | `ExecuteResponse.engine_generation` increments after a reload | integration | `cargo test -p api --test execute_integration -- engine_generation_increments_after_reload -x` | ❌ Wave 0 (new) |
| EXEC-03 / SC-2 | batch with one failing item → `succeeded == N-1`, `failed == 1`, that item `success:false`/`error:Some`, others unaffected | integration | `cargo test -p api --test execute_integration -- batch_truthful_aggregation_one_failing_item -x` | ❌ Wave 0 (new) |
| L-05 / D-01 | a run started before reload completes against the OLD snapshot (generation-accept) | integration | `cargo test -p api --test execute_integration -- in_flight_run_uses_old_snapshot -x` | ❌ Wave 0 (new) |
| SC-2 (existing) | one panicking item does not poison the batch (JoinError → failed) | integration | `cargo test -p api --test execute_integration -- batch_panicking_item_isolated -x` | ❌ Wave 0 (new) |
| D-03a | empty/missing rules dir at reload → warn + empty ruleset, no crash | unit | `cargo test -p api --test execute_integration -- reload_empty_dir_warns_not_crashes -x` | ❌ Wave 0 (new) |
| DEPLOY-05 | sync reload also swaps + bumps generation (D-04 honesty) | integration | `cargo test -p api --test execute_integration -- sync_reload_swaps_and_bumps_generation -x` | ❌ Wave 0 (new) |

### Sampling Rate
- **Per task commit:** `cargo test -p api --test execute_integration -x`
- **Per wave merge:** `cargo test -p api && cargo test -p engine`
- **Phase gate:** Full suite green before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `api/tests/execute_integration.rs` — add 8 RED tests above (SC-1/SC-2/SC-3 + reload + jobs + sync-swap + empty-dir). The existing Phase 23/24 tests (`score_plus_10_rule` fixture, `shared_ruleset()` helper at `:82`) are reused but the helper must build a `SharedRuleSet` (generation 0) instead of a bare `RuleSet`. The keystone reload test writes a `.rules` JSON file to a temp dir and points `RULE_STORAGE_PATH` at it (or injects a `build_ruleset_from_dir`-equivalent via `app_data`).
- [ ] Test fixture helper: `write_rule_file(dir, rule)` — serializes a `Rule` to JSON `.rules` file, so the keystone can edit it (`+10` → `+20`) and reload.
- [ ] No framework install needed — `cargo test` + `actix_web::test` already in use (`api/tests/execute_integration.rs` exists from Phase 23).

### Nyquist Dimension Coverage
- **Functional correctness:** SC-1 (reload→execute sees edit), SC-2 (truthful batch), SC-3 (generation bump) — the three ROADMAP success criteria.
- **Boundary/edge:** empty-dir reload (D-03a), one-failing-item batch, panicking-item isolation, in-flight-during-reload (generation-accept).
- **State transition:** job Pending→Running→Completed (and Failed on a bad dir).
- **Concurrency:** in-flight run uses old snapshot; reload write lock does not block execute `web::block`.
- **Honesty contract:** sync reload ALSO swaps (D-04) — tested explicitly so the planner cannot narrow to "async-only passes SC-1."

### Keystone scenario (SC-1, the honesty anchor)
1. Write `r1` (`score := score + 10`) as JSON to a temp `RULE_STORAGE_PATH/r1.rules`.
2. Start the test app with `app_data(SharedRuleSet::new(build_ruleset_from_dir(tmp)))` → generation 0, rule loaded.
3. `POST /v1/execute` `{facts: {scorecard: {score: 0}}}` → `facts_after.scorecard.score == 10`, `engine_generation == 0`.
4. Overwrite `r1.rules` with `score := score + 20`.
5. `POST /v1/rules/reload` `{async: true}` → 202 with `job_id`, `status: "pending"`.
6. Poll `GET /v1/jobs/{job_id}` → observe `pending` → `running` → `completed`.
7. `POST /v1/execute` `{facts: {scorecard: {score: 0}}}` → `facts_after.scorecard.score == 20` (the edited rule), `engine_generation == 1` (bumped).

## Risks & Unknowns

| # | Risk / Unknown | Impact | Mitigation |
|---|---|---|---|
| R1 | **`build_ruleset_from_dir` returns empty today** — D-05 reuses it but it doesn't parse. | HIGH — without the parser, reload swaps empty-for-empty and SC-1 is impossible. | Phase 25 Task 1 implements the JSON parser (Pattern 3). Flagged prominently above. `[VERIFIED: main.rs:44-71]` |
| R2 | **Exact `RuleSet` clone cost** under the read lock — `Vec<Rule>::clone` is O(n_rules) and deep-copies each `Rule` (which owns `Condition`/`Action` trees). | LOW for typical rulesets (tens of rules); the read lock is held only for the clone. | L-05 already releases before `web::block`. If profiling shows contention, the swap to `ArcSwap` is a future option (deferred per L-01). `[ASSUMED]` — no profiling done this session. |
| R3 | **`generation` placement on `RuleSet` vs `SharedRuleSet`** — D-02 leaves the exact shape to discretion. | LOW — both work; `SharedRuleSet` is cleaner (no engine-crate pollution, no conflation with the per-engine `generation: usize` field at `rete/mod.rs:23`). | Recommended `SharedRuleSet` struct (Pattern 1). `[VERIFIED: D-02 discretion]` |
| R4 | **Extracting the error string from `execute_single`'s `Err(HttpResponse)`** in the batch task — `HttpResponse` is not easily `into_string()` without consuming the body. | MEDIUM — the D-03 collector needs `error: Some(String)`. The current `execute_single` returns `Err(HttpResponse::BadRequest().json(...))`. | Option: change `execute_single` to return `Result<ExecuteResponse, (StatusCode, String)>` (status + message) so the batch task maps `Err((code, msg))` cleanly and `execute_rules` reconstructs the `HttpResponse`. OR: have the batch task call a shared inner function returning `Result<ExecuteResponse, String>` and have `execute_rules`/`execute_single` wrap it into `HttpResponse`. The planner picks the cleanest refactor. `[ASSUMED]` — needs a small signature refactor. |
| R5 | **`RuleSet.compiled` is private** (`engine/src/rules.rs:455`) — `build_ruleset_from_dir` builds via `add_rule` which sets `compiled: false`. The direct evaluator (`rete/mod.rs:execute_rules`) iterates `self.rules` and does not check `compiled`, so this is fine. | NONE — `compiled` is a no-op placeholder (`compile()` at `:508-515` just sets the flag). | No action. `[VERIFIED: rules.rs:508-515, rete/mod.rs:340-406]` |
| R6 | **Test isolation for reload** — tests that write to a real temp dir and call `reload_and_swap` must not race with other tests sharing the same `RULE_STORAGE_PATH`. | LOW — each test builds its own `TestServer` with its own `app_data(SharedRuleSet)`. | Use a per-test temp dir (`std::env::temp_dir()` + UUID) and inject the path via `app_data` or a closure rather than the global `RULE_STORAGE_PATH` env var. `[ASSUMED]` |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `Vec<Rule>::clone` under the read lock is sub-microsecond for typical rulesets → no `ArcSwap` needed. | Concurrency Analysis / R2 | If ruleset is huge (thousands of rules) and reload is frequent, read contention could appear. L-01 still holds (swap is rare); revisit only if profiling shows contention. |
| A2 | The batch task can extract a `String` error from `execute_single`'s `Err(HttpResponse)` via a small signature refactor (R4). | Pattern 5 / R4 | If `HttpResponse` body extraction is awkward, the refactor is slightly larger but mechanical. |
| A3 | Per-test temp dirs isolate reload tests from each other and from the global `RULE_STORAGE_PATH`. | R6 / Validation | If isolation is insufficient, tests could flake. Mitigation: inject the path via `app_data`, not env. |

**All other claims are `[VERIFIED: codebase]`** — read directly from the rRules source with `file:line` citations. No external docs, no Context7, no web search needed (CONTEXT: "No external specs — brownfield internal-wiring phase").

## Environment Availability

No external dependencies. The Rust toolchain (`cargo`, `rustc`) is already used by Phases 23/24 (the workspace compiles and `cargo test` runs). No new CLIs, services, runtimes, or databases are introduced. SKIPPED (no external dependencies identified).

## Security Domain

`security_enforcement` is not set in `.planning/config.json` (absent → enabled), but Phase 25 adds **no new untrusted-input boundary**: the reload endpoint already exists (`POST /v1/rules/reload`, Phase 21) and is JWT-gated by the existing `requires_auth` middleware (`main.rs:245-247`). The `.rules` JSON files are read from `RULE_STORAGE_PATH` (server-controlled directory, operator-written files — not user-uploaded at runtime in this phase; rule CRUD is the existing Phase 20 surface). The batch `Err` message (D-03) surfaces `execute_single`'s error string (e.g. `"unknown engine: foo"`) — already the existing 400 response body, no new info disclosure. No ASVS categories newly apply beyond what v4.0 already enforced.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `reload_engine` no-op (acquire-and-drop write lock) | `reload_and_swap` real build-then-swap | Phase 25 | Reload actually changes the executing ruleset |
| `build_ruleset_from_dir` returns empty | JSON `.rules` parser via serde | Phase 25 | Startup AND reload load real rules |
| `engine_generation: 0u64` hardcoded | Stamped from `SharedRuleSet.generation` | Phase 25 | Callers can detect stale-rule-set runs (SC-3) |
| Batch `Err` → zero-fired success mask | `Err` → per-item failure (D-03) | Phase 25 | Truthful `succeeded`/`failed` aggregates (SC-2) |
| Async reload `let _result = ...` (no JobStore write) | Pending→Running→Completed→JobStore | Phase 25 | `GET /v1/jobs/{id}` works (no 404) |

**Deprecated/outdated (Phase 25 removes/deprecates):**
- `EngineState::reload_engine` (`execution.rs:151-195`) — the no-op; replaced by `reload_and_swap`. The legacy `EngineState.engine` (`:96`) is unused by the execute path (Phase 23 moved to `EngineRegistry` + shared ruleset); `reload_rules` no longer takes `web::Data<EngineState>`. The planner may keep `EngineState` for `rule_files` or remove it (discretion).
- `EngineState::reload_from_directory` (`:116-148`) — the sync-path no-op (updates `rule_files` map but never swaps the ruleset); replaced by `reload_and_swap` on both paths (D-04).

## Open Questions

1. **Should `EngineState` be removed entirely?** Its `engine` field is legacy (execute path uses `EngineRegistry`), its `rule_files` map is updated by the old `reload_from_directory`/`reload_engine` but nothing reads it. Phase 25 could delete `EngineState` and its `app_data` wiring (`main.rs:79,210`). Discretion — the planner may keep it to minimize blast radius or remove it for clarity. Recommendation: remove if no handler outside `reload_rules` reads it; verify with `grep EngineState api/src/`.
2. **`execute_single` return type refactor (R4)** — change to `Result<ExecuteResponse, (StatusCode, String)>` for clean batch error extraction, or keep `Result<ExecuteResponse, HttpResponse>` and extract the body string in the batch task? Recommendation: refactor to `(StatusCode, String)` — it's a small, mechanical change that makes the batch path clean and is reversible.

## Sources

### Primary (HIGH confidence — read directly from the codebase)
- `api/src/v1/execution.rs` — `EngineState`/`reload_engine` no-op (`:95-195`), `ExecuteResponse.engine_generation` (`:56-64`, hardcoded `0u64` at `:532`/`:626`), `execute_single` read-lock-then-clone (`:340-534`), `batch_execute_rules` JoinSet + Err-mask bug (`:579-665`, bug at `:618-628`/collector `:639-641`), `reload_rules` async no-JobStore-write (`:679-715`, bug at `:685-696`), `ReloadJob`/`ReloadStatus`/`JobStore`/`init_job_store`/`get_job_status`/`list_jobs` (`:210-264,717-761`).
- `api/src/main.rs` — `build_ruleset_from_dir` returns empty (`:44-71`), shared `Arc<RwLock<RuleSet>>` (`:119-120`), `job_store = init_job_store()` (`:139`), routes `/rules/reload`/`/jobs/{id}`/`/jobs` (`:264,272-273`).
- `engine/src/rules.rs` — `Rule`/`Condition`/`Action`/`Expression`/`ComparisonOp` derive `Serialize, Deserialize` (`:15,116,148,230,417`); `RuleSet { rules: Vec<Rule>, compiled: bool }` (`:451-456`); `add_rule` (`:468-479`); `compile` is a placeholder (`:508-515`).
- `engine/src/engine/rete/mod.rs` — per-engine `generation: usize` (`:23`, per-instance, always 0 — NOT the source of truth); `load_ruleset` clones `ruleset.rules.clone()` (`:527-529`); direct evaluator `execute_rules` iterates `self.rules` (`:340-406`).
- `.planning/phases/25-real-reload-jobstore/25-CONTEXT.md` — L-01..L-06, D-01..D-05 (locked/auto-selected decisions).
- `.planning/research/PITFALLS.md` — Pitfalls 4 (`:92`), 12 (`:275`), 14 (`:283`); cross-cutting generation tag (`:302`).
- `.planning/ROADMAP.md` — Phase 25 section (`:160-186`): SC-1/SC-2/SC-3, research flags.
- `api/tests/execute_integration.rs` — existing fixture pattern (`shared_ruleset()` `:82`, `score_plus_10_rule()` `:53`, `app_data` stack `:166-178`).

### Secondary / Tertiary
- None. This is a brownfield internal-wiring phase with no external specs (CONTEXT: "No external specs"). No Context7, web search, or package registry lookups were performed — all findings are `[VERIFIED: codebase]` or marked `[ASSUMED]` in the Assumptions Log.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all verified in-repo; no new deps.
- Architecture (SharedRuleSet, reload_and_swap, batch propagation): HIGH — derived directly from the existing code shapes + locked decisions.
- Pitfalls: HIGH — every pitfall root cause cited to `file:line`.
- Parser prerequisite (R1): HIGH — `build_ruleset_from_dir` emptiness verified; JSON parser viability verified via serde derives.
- Batch error extraction (R4): MEDIUM — the refactor is needed but the exact shape is planner discretion.

**Research date:** 2026-07-03
**Valid until:** 2026-08-03 (30 days — stable brownfield codebase; no fast-moving external deps)