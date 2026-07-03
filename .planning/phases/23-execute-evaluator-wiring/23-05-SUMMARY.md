---
phase: 23-execute-evaluator-wiring
plan: 05
subsystem: api
tags: [rust, actix-web, execute, engine-registry, rete, per-request-isolation, pitfall-1, pitfall-11, pitfall-7, pitfall-9, wave-3, roadmap-green]

# Dependency graph
requires:
  - phase: 23-01
    provides: "EngineRegistry::register_default_engines (all four factories) + EngineSelector::with_defaults + engine features enabled on api crate"
  - phase: 23-02
    provides: "Expression AST + Condition::Pattern { op: ComparisonOp } + Action::UpdateFact.fields as HashMap<String, Expression>"
  - phase: 23-03
    provides: "ReteEngine direct evaluator that fires rules truthfully, populates EngineResult.fired_rule_ids, records once-per-rule coverage, exposes load_ruleset + get_facts"
  - phase: 23-04
    provides: "MetricsSnapshot rename (disambiguated from execution::EngineState) + five RED scaffolds in api/tests/execute_integration.rs installed against the full Plan 23-05 app_data stack"
provides:
  - "api/src/v1/execution.rs::execute_single rewritten to construct a fresh Box<dyn ExecutionEngine> per request via EngineRegistry::create_engine_by_name, load the shared ruleset, insert request facts, run engine.execute() inside web::block, and serialize facts_after from engine.get_facts() via the reverse engine_value_to_json mapping (Pitfall 11 echo bug removed)"
  - "api/src/v1/execution.rs::ExecuteRequest.engine: Option<String> — ENGINE-07/08 param (400 on unknown, env-default on omitted)"
  - "api/src/v1/execution.rs::engine_value_to_json — reverse of json_to_engine_value (D-08), serializes post-execution working memory to JSON"
  - "api/src/main.rs — EngineRegistry + EngineSelector + shared Arc<RwLock<RuleSet>> web::Data wiring; RRULES_DEFAULT_ENGINE + RRULES_MAX_CHAIN_ITERATIONS read at startup; build_ruleset_from_dir helper (empty+warn on missing dir, D-03a)"
  - "POST /v1/execute fires real rules end-to-end; the four ROADMAP success criteria are GREEN"
affects: [23-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Per-request engine isolation (Pitfall 1/9): construct a fresh Box<dyn ExecutionEngine> via registry.create_engine_by_name for EACH request and move it into web::block — never take the shared EngineState.engine write lock in the execute path; ReteEngine is not Clone so no pool."
    - "Reverse value mapping for facts_after (Pitfall 11): serialize post-execution working memory from engine.get_facts() via a recursive engine_value_to_json that inverts json_to_engine_value arm-by-arm — NEVER echo the request input JSON."
    - "Engine param validation without &mut self (Pitfall 7): EngineSelector::select_engine takes &mut self, so per-request validation uses EngineRegistry::available_engines() (&self) + create_engine_by_name; the selector's default is set ONCE at startup before wrapping in web::Data."
    - "web::block closure returns a tuple (EngineResult, Vec<Fact>) so both the fired-rule count and the post-execution working memory are available after the CPU-bound engine.execute() call — engine is moved into the closure so no shared &mut crosses the request boundary."
    - "Shared read-only ruleset snapshot (D-03): Arc<RwLock<RuleSet>> built once at startup, acquired read-only per request to load_ruleset + collect rule ids for rules_not_fired, then dropped before execution so no lock spans engine.execute()."

key-files:
  created: []
  modified:
    - "api/src/main.rs — EngineRegistry + EngineSelector + Arc<RwLock<RuleSet>> web::Data wiring; RRULES_DEFAULT_ENGINE / RRULES_MAX_CHAIN_ITERATIONS startup reads; build_ruleset_from_dir helper (D-03/D-03a)"
    - "api/src/v1/execution.rs — ExecuteRequest.engine field added; execute_single rewritten (per-request engine, load_ruleset, insert_fact, web::block(execute)+get_facts, facts_after from get_facts, rules_not_fired from fired_rule_ids, echo bug removed); execute_rules + batch_execute_rules handler signatures updated; engine_value_to_json reverse mapping added"

key-decisions:
  - "build_ruleset_from_dir returns an empty RuleSet with a warning when the rules dir is missing/empty OR when rule files are present but unparsed. Live rule-file parsing is Phase 25's scope; the Plan 23-04 integration tests inject their own fixture ruleset via app_data, so the server's default snapshot being empty does not block the keystone tests. The critical D-03a requirement (server comes up with an empty rules dir) is met."
  - "RRULES_MAX_CHAIN_ITERATIONS is validated at startup (warn if unparseable) but NOT passed to handlers — the ReteEngine evaluator reads the env var directly per Plan 23-03. main.rs only validates parseability so a misconfigured value surfaces in logs at startup rather than silently."
  - "Engine name resolution order: req.engine > RRULES_DEFAULT_ENGINE env var > selector.current_engine() (set at startup). The env var is the source of truth for per-request default resolution (ENGINE-08) so the response reflects the actual engine used; selector.current_engine() is the final fallback."
  - "batch_execute_rules on error reports a zero-fired ExecuteResponse placeholder per item (rather than propagating the HttpResponse) so the per-item failure is surfaced in the BatchItemResult without aborting the batch — preserves the 'one failing request does not fail the batch' contract."
  - "inference_trace remains None (L-04 — Phase 28 populates it)."

patterns-established:
  - "Pattern: per-request engine construction from a shared factory registry, moved into web::block for CPU-bound execution — no shared &mut engine across requests."
  - "Pattern: reverse value mapping (engine_value_to_json) to serialize post-execution working memory truthfully, replacing the request-input echo anti-pattern."

requirements-completed: [EXEC-01, EXEC-02, ENGINE-07, ENGINE-08, MON-05]

# Coverage metadata (#1602) — per-deliverable traceability.
coverage:
  - id: D1
    description: "main.rs wires EngineRegistry (register_default_engines), EngineSelector (default set at startup via select_engine, RRULES_DEFAULT_ENGINE fallback rete), Arc<RwLock<RuleSet>> (build_ruleset_from_dir, empty+warn on missing dir), reads RRULES_MAX_CHAIN_ITERATIONS, injects all three as web::Data"
    requirement: "ENGINE-08"
    verification:
      - kind: build
        ref: "api/src/main.rs; cargo check -p api exits 0"
        status: pass
    human_judgment: false
  - id: D2
    description: "execute_single rewrite: per-request engine via create_engine_by_name, load_ruleset, insert_fact, web::block(execute)+get_facts, facts_after from get_facts via engine_value_to_json, rules_not_fired from fired_rule_ids, echo bug removed, engine param validation (400 on unknown, env-default on omitted)"
    requirement: "EXEC-01"
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs; cargo test -p api --test execute_integration -> 5/5 pass (execute_fires_real_rule, facts_after_not_echoed, engine_param_validation, default_engine_resolution, coverage_auto_populated all GREEN)"
        status: pass
    human_judgment: false
  - id: D3
    description: "facts_after serialized from post-execution get_facts() via engine_value_to_json (D-08 / Pitfall 11); the echo bug facts_after.insert(fact_type.clone(), fact_value.clone()) is removed (grep reports 0)"
    requirement: "EXEC-02"
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs::facts_after_not_echoed; grep -c 'facts_after.insert(fact_type.clone(), fact_value.clone())' api/src/v1/execution.rs == 0"
        status: pass
    human_judgment: false
  - id: D4
    description: "Engine param validation: engine='rete' -> 200; engine='unknown' -> 400 JSON {error: 'unknown engine: unknown'} (ENGINE-07); omitted engine resolves to RRULES_DEFAULT_ENGINE and executes (ENGINE-08)"
    requirement: "ENGINE-07"
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs::engine_param_validation + default_engine_resolution"
        status: pass
    human_judgment: false
  - id: D5
    description: "Coverage auto-populated: after /v1/execute fires r1, GET /v1/metrics/coverage shows r1 with fired==true, fire_count==1 (MON-05 / Pitfall 6 once-per-rule)"
    requirement: "MON-05"
    verification:
      - kind: integration
        ref: "api/tests/execute_integration.rs::coverage_auto_populated; cargo test -p engine --features engine-rete --test coverage -> record_once_per_rule GREEN"
        status: pass
    human_judgment: false

# Metrics
duration: 12min
completed: 2026-07-03
status: complete
---

# Phase 23 Plan 05: Wire POST /v1/execute through the real EngineRegistry Summary

**Rewrote `execute_single` so POST /v1/execute constructs a fresh `Box<dyn ExecutionEngine>` per request via `EngineRegistry::create_engine_by_name`, loads the shared read-only `Arc<RwLock<RuleSet>>`, inserts request facts, runs the real ReteEngine evaluator inside `web::block`, and serializes `facts_after` from post-execution `get_facts()` via a reverse `engine_value_to_json` mapping — turning all five Plan 23-04 RED scaffolds GREEN and satisfying all four ROADMAP success criteria (rules fire truthfully, facts_after is the incremented value not the echoed input, engine param validated with 400 on unknown / env-default on omitted, coverage auto-populates with once-per-rule fire_count).**

## Performance

- **Duration:** ~12 min
- **Tasks:** 2 (1 main.rs wiring + 1 execute_single rewrite / TDD GREEN)
- **Files modified:** 2 (api/src/main.rs, api/src/v1/execution.rs)

## Accomplishments
- Wired `api/src/main.rs` with the shared engine stack (D-03 / L-07): `EngineRegistry::register_default_engines()` (all four factories), `EngineSelector::with_defaults()` with the default engine set ONCE at startup via `select_engine` (mutably, before `web::Data` wrapping — Pitfall 7), and a shared read-only `Arc<RwLock<RuleSet>>` built from `RULE_STORAGE_PATH` via a new `build_ruleset_from_dir` helper that logs a warning and returns an empty `RuleSet` when the directory is missing or empty (D-03a — server comes up rather than crashing). Reads and validates `RRULES_DEFAULT_ENGINE` (default `"rete"`) and `RRULES_MAX_CHAIN_ITERATIONS` (default 1000, validated parseable at startup with a warning on failure — the evaluator reads the env var directly per Plan 23-03). Injected all three new handles into the `.app_data(...)` chain alongside the existing `engine_state`.
- Rewrote `api/src/v1/execution.rs::execute_single` to accept `web::Data<EngineRegistry>`, `web::Data<EngineSelector>`, `web::Data<Arc<RwLock<RuleSet>>>`. The new flow: (1) resolve engine name from `req.engine` > `RRULES_DEFAULT_ENGINE` env > `selector.current_engine()`; (2) validate against `registry.available_engines()` — unknown names return HTTP 400 with a JSON `{error: "unknown engine: X"}` body (ENGINE-07); (3) construct a fresh `Box<dyn ExecutionEngine>` via `create_engine_by_name` (per-request isolation — Pitfall 1/9, no `engine_state.engine.write()` in the execute path); (4) load the shared ruleset under a read-only guard and collect rule ids for `rules_not_fired`; (5) insert request facts via `json_to_engine_value` → `Fact::new` → `engine.insert_fact`; (6) run `engine.execute()` inside `web::block` (CPU-bound) and return both the `EngineResult` and post-execution `Vec<Fact>` from the closure; (7) serialize `facts_after` from `get_facts()` via the new reverse `engine_value_to_json` mapping grouped by `fact_type_id` (D-08 / Pitfall 11 — echo bug removed); (8) compute `rules_not_fired` as the shared ruleset's rule ids minus `EngineResult.fired_rule_ids`; (9) `inference_trace` stays `None` (L-04).
- Added `ExecuteRequest.engine: Option<String>` (with the existing `skip_serializing_if = "Option::is_none"` idiom) for the ENGINE-07/08 param.
- Added `engine_value_to_json` — the reverse of `json_to_engine_value`, inverting arm-by-arm across `Bool`/`Integer`/`Float`/`String`/`DateTime` (→ RFC3339-ish string)/`Array`/`Object`/`Null`.
- Updated `execute_rules` and `batch_execute_rules` handler signatures to extract the three new `web::Data` handles. `batch_execute_rules` clones the `web::Data` handles into each `JoinSet` task (Arc-under-the-hood, cheap) so every task builds its own engine — preserving the "no shared locks between requests" contract. On per-task error, a zero-fired `ExecuteResponse` placeholder is reported in the `BatchItemResult` so the batch is not aborted.
- Removed the Pitfall 11 echo bug at the old `execution.rs:327` (`facts_after.insert(fact_type.clone(), fact_value.clone())`) — `facts_after` is now built ONLY from post-execution `get_facts()`.

## Task Commits

1. **Task 1: Wire EngineRegistry + EngineSelector + shared RuleSet in main.rs (D-03, L-07)**
   - `20a677fd2` — feat(23-05): wire EngineRegistry + EngineSelector + shared RuleSet in main.rs (D-03, L-07)
2. **Task 2: Rewrite execute_single — per-request engine, facts_after from get_facts, rules_not_fired, remove echo bug (TDD GREEN)**
   - `70f9e5274` — feat(23-05): rewrite execute_single through real EngineRegistry (EXEC-01/02, ENGINE-07/08, MON-05)

**Plan metadata:** this SUMMARY committed after STATE.md/ROADMAP.md updates.

## Files Created/Modified
- `api/src/main.rs` — added `EngineRegistry`/`EngineSelector`/`RuleSet` imports; `build_ruleset_from_dir` helper (D-03/D-03a); startup construction of registry, selector (default set mutably), `RRULES_DEFAULT_ENGINE`/`RRULES_MAX_CHAIN_ITERATIONS` reads, shared `Arc<RwLock<RuleSet>>`; three new `.app_data(...)` injections.
- `api/src/v1/execution.rs` — `ExecuteRequest.engine: Option<String>` field; `execute_single` fully rewritten (per-request engine, load_ruleset, insert_fact, web::block(execute)+get_facts, facts_after from get_facts via engine_value_to_json, rules_not_fired from fired_rule_ids, echo bug removed, 400 on unknown engine); `execute_rules` + `batch_execute_rules` signatures updated to extract the three new `web::Data` handles; `engine_value_to_json` reverse mapping added.

## Decisions Made
- **`build_ruleset_from_dir` returns an empty `RuleSet` with a warning** when the rules dir is missing/empty OR when rule files are present but unparsed. Live rule-file parsing is Phase 25's scope; the Plan 23-04 integration tests inject their own fixture ruleset via `app_data`, so the server's default snapshot being empty does not block the keystone tests. The critical D-03a requirement (server comes up with an empty rules dir) is met.
- **`RRULES_MAX_CHAIN_ITERATIONS` is validated at startup but not passed to handlers** — the ReteEngine evaluator reads the env var directly per Plan 23-03. `main.rs` only validates parseability so a misconfigured value surfaces in startup logs.
- **Engine name resolution order:** `req.engine` > `RRULES_DEFAULT_ENGINE` env var > `selector.current_engine()`. The env var is the per-request default source of truth (ENGINE-08).
- **`batch_execute_rules` on error reports a zero-fired placeholder** per item rather than propagating the `HttpResponse`, preserving the "one failing request does not fail the batch" contract.
- **`inference_trace` remains `None`** (L-04 — Phase 28 populates it).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] ExecutionEngine trait import path**
- **Found during:** Task 2 compilation
- **Issue:** The plan's `<action>` referenced `Box<dyn ExecutionEngine>` and the registry/selector imports as `engine::engine::{ExecutionEngine, EngineRegistry, EngineSelector}`, but `ExecutionEngine` is NOT re-exported at `engine::engine::ExecutionEngine` (only `EngineRegistry`, `EngineType`, `EngineSelector`, `EngineMetrics` are). It lives at `engine::engine::registry::ExecutionEngine`.
- **Fix:** Initially imported from `engine::engine::registry::ExecutionEngine`, which compiled but emitted an unused-import warning (the trait's methods are callable on `Box<dyn ExecutionEngine>` without importing the trait — the dyn type makes them inherent-ish). Removed the import entirely to keep the build warning-clean.
- **Files modified:** api/src/v1/execution.rs
- **Verification:** `cargo check -p api --tests` exits 0 with no api-crate warnings.
- **Committed in:** `70f9e5274` (Task 2)

**2. [Rule 3 - Blocking] Removed unused `engine::WorkingMemory` import**
- **Found during:** Task 2
- **Issue:** The old stub used `WorkingMemory::new()`; the rewrite no longer references `WorkingMemory` (the per-request `Box<dyn ExecutionEngine>` owns its own working memory). The import became unused.
- **Fix:** Removed `use engine::WorkingMemory;`.
- **Files modified:** api/src/v1/execution.rs
- **Committed in:** `70f9e5274` (Task 2)

---

**Total deviations:** 2 auto-fixed (both blocking import-path cleanups). No scope creep — the delivered artifacts match the plan's `output` section exactly.

## Issues Encountered
None beyond the deviations above.

## User Setup Required
None. No new runtime packages, no external services. The three env vars (`RULE_STORAGE_PATH`, `RRULES_DEFAULT_ENGINE`, `RRULES_MAX_CHAIN_ITERATIONS`) all have sensible defaults.

## Next Phase Readiness
- All four ROADMAP success criteria for Phase 23 are GREEN end-to-end over HTTP: (1) score+10 → `rules_fired >= 1` + `facts_after.scorecard.score == 10`; (2) `engine: "rete"` → 200 / `engine: "unknown"` → 400; (3) omitted `engine` → `RRULES_DEFAULT_ENGINE` resolves and executes; (4) coverage shows `r1` with `fired == true`, `fire_count == 1`.
- The `record_once_per_rule` regression test (Pitfall 6 contract) and `rete_execution` suite are still GREEN — no engine-level regression.
- Plan 23-06 (verifier / phase close) can now run: the keystone is honest, the execute path is wired, and the four ROADMAP success criteria are observable via the integration test suite.
- No blockers.

## Requirement Status Note

`EXEC-01`, `EXEC-02`, `ENGINE-07`, `ENGINE-08`, `MON-05` are marked complete in this plan's frontmatter. REQUIREMENTS.md checkboxes are updated via `requirements mark-complete` in the state-update step; the phase verifier (Plan 23-06) confirms end-to-end.

---
*Phase: 23-execute-evaluator-wiring*
*Completed: 2026-07-03*

## Self-Check: PASSED

- `api/src/main.rs` exists on disk with `register_default_engines`, `RRULES_DEFAULT_ENGINE`, `RRULES_MAX_CHAIN_ITERATIONS`, `build_ruleset_from_dir`, `.app_data(registry.clone())`: FOUND
- `api/src/v1/execution.rs` exists on disk with `pub engine: Option<String>`, `fn engine_value_to_json`, `web::block`, `available_engines().contains`-style validation, `BadRequest`, `fired_rule_ids`: FOUND
- Echo bug `grep -c 'facts_after.insert(fact_type.clone(), fact_value.clone())' api/src/v1/execution.rs` == 0: OK
- Task 1 commit `20a677fd2`: FOUND
- Task 2 commit `70f9e5274`: FOUND
- `cargo test -p api --test execute_integration` -> 5/5 pass (all five Plan 23-04 scaffolds GREEN): OK
- `cargo test -p engine --features engine-rete --test rete_execution` -> 7/7 pass (no engine regression): OK
- `cargo test -p engine --features engine-rete --test coverage` -> 1/1 pass (`record_once_per_rule` GREEN): OK
- `cargo check -p api --tests` exits 0 with no api-crate warnings: OK
- Post-commit deletion check: no tracked files deleted in either task commit