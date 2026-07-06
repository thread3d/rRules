# Phase 25: Real reload + JobStore - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-03
**Phase:** 25-real-reload-jobstore
**Mode:** `--auto` (recommended option auto-selected for every gray area; no interactive AskUserQuestion)
**Areas discussed:** Reload in-flight handling, Generation counter placement, Batch-execute truthful aggregation, Sync reload path, Reload scope

---

## Reload in-flight handling (Pitfall 4 — drain vs generation-accept)

| Option | Description | Selected |
|--------|-------------|----------|
| Drain in-flight executes before swapping | Track in-flight count (`AtomicUsize`); block reload until executes finish; safer but adds latency on the hot path | |
| Generation-accept (swap immediately, tag with generation) | Let in-flight runs finish against their cloned snapshot; new runs get the new ruleset; tag responses with `engine_generation` | ✓ |

**Auto-selected choice:** Generation-accept (Pitfall 4 step 3 option b)
**Notes:** Matches the existing Phase 23 per-request-clone design (`execute_single` clones the ruleset into the per-request engine under a short read lock, releases before execute). Reload is rare; execute is hot — don't block the hot path for the rare event. The generation tag (next area) lets callers detect staleness (SC-3, frontend "rules changed since this run" in Phase 28).

---

## Generation counter placement

| Option | Description | Selected |
|--------|-------------|----------|
| Separate `AtomicU64` `web::Data` | Lock-free read; bumped on reload | |
| Inside the shared ruleset payload (read under the existing read lock) | `SharedRuleSet { ruleset, generation }` under the single `RwLock`; coherent read | ✓ |
| Per-engine `generation: usize` field (`rete/mod.rs:23` etc.) | Per-instance, reset each request (always 0) — cannot track reloads | |

**Auto-selected choice:** Inside the shared ruleset payload, read atomically under the existing read lock
**Notes:** A separate `AtomicU64` would race — the counter could bump between reading the ruleset and reading the counter. Bundling generation with the ruleset under the one `RwLock` gives a coherent snapshot+generation read with no new lock on the hot path. The per-engine `generation` fields are per-instance and unused for reload tracking.

---

## Batch-execute truthful aggregation (SC-2 gap)

| Option | Description | Selected |
|--------|-------------|----------|
| Mask `execute_single` `Err` as a zero-fired success (current behavior) | `succeeded += 1`, `success:true`, `rules_fired:0` — inflates `succeeded`, hides failures | |
| Propagate `Err` as a per-item failure | `failed += 1`, `success:false`, `error:Some`; panic → `JoinError` → `failed` (already correct) | ✓ |
| Whole-batch 400 on any bad item | Reject the whole batch — contradicts the "one failing request does not fail the batch" contract | |

**Auto-selected choice:** Per-item failure (`success:false` + `failed += 1`)
**Notes:** SC-2 requires `total_fired`/`succeeded`/`failed` to aggregate truthfully. Today an unknown-engine item (400) is miscounted as a successful zero-fire (`execution.rs:618-628` + `:639-641`). The handler doc explicitly states "one failing request does not fail the batch," so whole-batch 400 is wrong.

---

## Sync reload path

| Option | Description | Selected |
|--------|-------------|----------|
| Async-only swap (SC-1 tests async only) | Sync `reload_from_directory` stays as-is (doesn't swap the shared ruleset) | |
| Both sync + async swap via one shared helper | Shared `reload_and_swap`; sync returns `ReloadResponse`, async uses `JobStore`; consistent + honest | ✓ |

**Auto-selected choice:** Both swap (shared helper)
**Notes:** A sync reload that didn't swap would be a lie — on-disk rules would differ from the executing ruleset. SC-1 only tests async, but honesty requires both. One shared helper avoids two divergent reload implementations.

---

## Reload scope (directory vs single file)

| Option | Description | Selected |
|--------|-------------|----------|
| Full-directory reload reusing `build_ruleset_from_dir` | Matches startup; swapped `RuleSet` == what a restart loads | ✓ |
| Honor `ReloadRequest.rule_file` for partial reload | Single-file recompilation/merge — more complexity; `rule_file` currently ignored | |

**Auto-selected choice:** Full-directory reload reusing `build_ruleset_from_dir`; `rule_file` deferred
**Notes:** Consistency with startup; less complexity. `rule_file` stays in the struct (no API break) but is not honored in Phase 25.

---

## Claude's Discretion

- Reload function placement (extend `EngineState::reload_engine` vs new `reload_and_swap` on `web::Data<SharedRuleSet>`); reload must operate on the shared ruleset, not legacy `EngineState.engine`.
- Exact `SharedRuleSet` shape (new struct vs `RwLock<(RuleSet, u64)>` vs `generation` field on `RuleSet`) — contract is "ruleset+generation atomic under one read lock, generation bumpable under write lock."
- `JobResponse.status` initial value (`"pending"` vs `"running"`); SC-1 polls Pending→Running→Completed so insert `Pending` first.
- Test placement (extend `api/tests/execute_integration.rs` + jobs/metrics tests; Wave 0 RED→GREEN + Nyquist pattern from Phase 23/24).

## Deferred Ideas

- File watcher (`start_file_watcher` stub, Pitfall 13) — future enhancement; reload is request-triggered in Phase 25.
- Single-file / partial reload (`ReloadRequest.rule_file`) — deferred (D-05).
- `RuleExecutionTracker` singleton + dead-rule live data — Phase 26.
- Stateful sessions — Phase 27.
- Frontend "Reload then Run" + `getJobStatus` polling UI — Phase 28.
- `inference_trace` population — Phase 28 (EXEC-04).