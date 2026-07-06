# Phase 26: RuleExecutionTracker singleton + dead-rule live data - Context

**Gathered:** 2026-07-03
**Status:** Ready for planning
**Mode:** `--auto` (all gray areas auto-selected; recommended option chosen for each; logged inline)

<domain>
## Phase Boundary

Make `RuleExecutionTracker` a **process-wide singleton** so dead-rule detection reads **live** firings — `GET /v1/metrics/dead-rules` no longer always returns `[]`. Unify the **three-counter drift** by defining one canonical "fire" semantics with `CoverageTracking` as the single source of truth, so `/metrics`, `/v1/metrics/coverage`, and `/v1/metrics/dead-rules` agree. Pin the behavior with an end-to-end integration test.

After this phase: a `POST /v1/execute` that fires rule `r1` causes `GET /v1/metrics/dead-rules` to **exclude** `r1` (previously always `[]`), `GET /v1/metrics/coverage` to show `r1.fired == true` + `r1.evaluation_count >= 1`, and all three metrics surfaces to agree on `r1`'s fire count (no drift).

This is the **tracker-singleton + dead-rule-live-data** layer of the v5.0 Live Execution vertical slice. It builds on Phase 23's real per-request evaluator (which already records `CoverageTracking` once-per-rule) and Phase 24's live counters. The keystone is honesty: the dead-rules panel that v4.0 shipped finally has real data.

**In scope (locked by ROADMAP.md §"Phase 26" + PITFALLS.md):**
- Convert `RuleExecutionTracker` (`engine/src/monitoring/detection.rs:64`) to a `LazyLock<Mutex<…>>` singleton, mirroring the existing `CoverageTracking` pattern (`engine/src/monitoring/coverage.rs:86`)
- Add `record_evaluation`/`record_fire` calls next to the `CoverageTracking` calls in the shared direct evaluator (`engine/src/engine/direct_eval.rs:106,109`) — once per rule per execution, outside the inner loop (Pitfall 6)
- `dead_rules_endpoint` (`api/src/metrics.rs:378-386`) reads from the singleton — remove the `RuleExecutionTracker::new()` at `metrics.rs:380` (Pitfall 3)
- Unify the three-counter drift: one canonical "fire" semantics; `RuleExecutionTracker` and per-engine `metrics.record_rule_fired` derive from / delegate to `CoverageTracking` as the single source of truth
- Pin with an e2e integration test: after one `/v1/execute` firing `r1`, dead-rules excludes `r1` and coverage shows `r1.fired == true`; the three surfaces agree on fire count

**Out of scope (explicitly deferred):**
- Stateful sessions — Phase 27
- Frontend test-runner / dead-rules panel UI changes — Phase 28 (the v4.0 dashboard adapters already tolerate real shapes; Phase 26 only feeds them live data, no frontend code)
- `inference_trace` population — Phase 28 (EXEC-04)
- Merging the parallel `RuleHealthRegistry` (`engine/src/monitoring/dead_rules.rs`) into the unified tracker — see D-05 (separate v1.0-era system, not wired to the live execute path; deferred)
- Merging `coverage/history/mod.rs` tracker — deferred (D-05)

</domain>

<decisions>
## Implementation Decisions

### Carried forward as locked (not re-asked — from ROADMAP.md, PITFALLS.md, Phase 23/24/25 CONTEXT)
- **L-01:** Singleton pattern = mirror `CoverageTracking` exactly (`engine/src/monitoring/coverage.rs:83-93`): `static INSTANCE: LazyLock<Mutex<TrackingState>> = LazyLock::new(|| Mutex::new(...))` with associated-function API (`record_rule_eval(rule_id)`, `record_rule_fire(rule_id)` — no `&self`/`&mut self`). The ROADMAP research flag explicitly says "direct mirror of the existing `CoverageTracking` `LazyLock<Mutex<…>>` pattern; call sites already known."
- **L-02:** The recording call site is the **shared direct evaluator** (`engine/src/engine/direct_eval.rs:106,109`), NOT `rete/mod.rs:293,332` as the ROADMAP text states. **Call-site correction:** Phase 23-06 extracted the once-per-rule coverage recording into `direct_eval.rs` (shared by all four engines — Rete/Treat/Leaps/Phreak all delegate to it). The `rete/mod.rs:296-299` comments confirm this ("CoverageTracking::record_rule_eval used to be called here … moved … coverage recording after the fixpoint loop (L-06 / Pitfall 6)"). The new `RuleExecutionTracker::record_evaluation`/`record_fire` calls go next to the `CoverageTracking::record_rule_eval`/`record_rule_fire` calls in `direct_eval.rs:106,109` — once per rule, after the fixpoint loop, outside the inner nagram loop. This keeps all four engines consistent automatically.
- **L-03:** `dead_rules_endpoint` (`api/src/metrics.rs:378-386`) currently calls `RuleExecutionTracker::new()` → always-empty tracker → returns `[]` (Pitfall 3). It MUST read from the singleton instead.
- **L-04:** Once-per-rule semantics is already established (Phase 23 L-06 / Pitfall 6): `direct_eval.rs` collects `evaluated_set` + `fired_set` (`HashSet<String>`) and records once per id after the fixpoint loop. Phase 26's `RuleExecutionTracker` recording reuses these same sets — no new per-nagram recording.
- **L-05:** `serial_test` (already a workspace dev-dep per 23-CONTEXT) is the established tool for singleton-isolation tests; `CoverageTracking` tests already use this pattern. Phase 26's tests mirror it.

### Discussed gray areas (auto-selected recommended option for each)

#### Singleton API shape
- **D-01: Mirror `CoverageTracking` — zero-sized-struct + `LazyLock<Mutex<State>>` + associated functions.** `RuleExecutionTracker` becomes a unit-like struct (or keeps its name as the associated-function namespace) backed by a `static INSTANCE: LazyLock<Mutex<TrackerState>>`. Public API: `record_evaluation(rule_id, rule_name)`, `record_fire(rule_id, rule_name)`, `find_dead_rules() -> Vec<DeadRuleInfo>` (owned, like `CoverageTracking::get_all_states`), `reset()`/`clear()` for tests. The current `&mut self` methods (`detection.rs:80,92,137`) become associated functions taking `rule_id`/`rule_name` by value (matching `CoverageTracking::record_rule_fire(rule_id: impl Into<String>)`). `find_dead_rules` returns owned `Vec<DeadRuleInfo>` (not `Vec<&DeadRuleInfo>` — can't return refs from inside the lock; mirrors `CoverageTracking::get_all_states`).
  - `[auto]` Q: "Mirror `CoverageTracking` (associated functions + `LazyLock<Mutex<State>>`) or keep `&mut self` methods behind a global `Mutex<RuleExecutionTracker>`?" → Selected: **"Mirror `CoverageTracking` — associated functions + `LazyLock<Mutex<State>>`"** (recommended — L-01; consistency with the established singleton pattern; call sites already pass `rule_id` by value)

#### Source-of-truth unification (Pitfall 6 — three-counter drift)
- **D-02: `CoverageTracking` is the canonical source of truth for fire/eval COUNTS; `RuleExecutionTracker` derives counts from it and only owns the dead-rule-specific metadata; per-engine `metrics.record_rule_fired` stops being an independent counter.** The three drift counters today are: `CoverageTracking` (`coverage.rs:105`), `RuleExecutionTracker.total_fires` (`detection.rs:70,112`), and per-engine `EngineMetrics::record_rule_fired` (`direct_eval.rs`→`self.metrics.record_rule_fired()` at `rete/mod.rs:381`, `treat/mod.rs:387`, `leaps/mod.rs:309,388`, `phreak/mod.rs:574`). Unification: `CoverageTracking` remains the single source for `fired`/`evaluated`/`fire_count`/`evaluation_count` (it already records once-per-rule at `direct_eval.rs:106,109`). `RuleExecutionTracker` either (a) delegates its counts to `CoverageTracking` reads, or (b) is fed at the same `direct_eval.rs` call sites and kept consistent by construction — planner picks. The dead-rule-specific fields `last_fired_at`, `rule_name`, `category` (`detection.rs:28,33,34`) live in the `RuleExecutionTracker` singleton (CoverageTracking's `RuleCoverageState` doesn't carry them — extending it is planner's discretion). The per-engine `metrics.record_rule_fired` is **decoupled from the fire-count contract** — it feeds per-engine gauges (Phase 24 `MetricsSnapshot`), not the dead-rule/coverage surfaces; the planner verifies it doesn't drift from `CoverageTracking` or removes it from the execute path if redundant. SC-3 ("the three surfaces agree on fire count") is satisfied because all three surfaces ultimately read from `CoverageTracking`.
  - `[auto]` Q: "How to unify the three-counter drift — `CoverageTracking` canonical with the other two deriving, or just feed all three from the same call sites?" → Selected: **"`CoverageTracking` canonical; `RuleExecutionTracker` + per-engine metrics derive/delegate"** (recommended — ROADMAP deliverable explicitly says "delegate to `CoverageTracking` as the single source of truth"; SC-3 requires agreement, which only a single source guarantees)

#### Recording call site (ROADMAP line-number correction)
- **D-03: Record in `engine/src/engine/direct_eval.rs` next to the `CoverageTracking` calls (lines ~106, ~109), NOT at `rete/mod.rs:293,332`.** The ROADMAP's "`rete/mod.rs:293,332`" line references are **stale** — Phase 23-06 moved once-per-rule coverage recording out of `rete/mod.rs` into the shared `direct_eval.rs` evaluator (all four engines call it). Recording in `direct_eval.rs` means Rete/Treat/Leaps/Phreak all get dead-rule tracking for free, with no 4× duplication and no risk of one engine forgetting. The call is `RuleExecutionTracker::record_evaluation(id, name)` for each id in `evaluated_set` and `RuleExecutionTracker::record_fire(id, name)` for each id in `fired_set`, immediately adjacent to the existing `CoverageTracking::record_rule_eval`/`record_rule_fire` calls. `rule_name` is read from the `Rule` in the loop (the evaluator already iterates `self.rules`).
  - `[auto]` Q: "Where to add the `RuleExecutionTracker` recording calls — `rete/mod.rs:293,332` (ROADMAP text) or `direct_eval.rs` (where coverage recording actually lives post-Phase-23)?" → Selected: **"`direct_eval.rs` next to the `CoverageTracking` calls"** (recommended — L-02; all four engines covered with one edit; matches the actual post-23-06 code structure)

#### `new()` removal / deprecation (Pitfall 3)
- **D-04: Deprecate the public `new()` (`#[deprecated]`) and rewrite the one production call site (`metrics.rs:380`); migrate `detection.rs` tests to the singleton + `reset()`/`clear()` + `serial_test`.** `new()` is called in production only at `api/src/metrics.rs:380` (the bug) and in 6 test sites within `detection.rs:276,286,297,311,334,350`. Removing `new()` outright would break the api crate's call (which is exactly the call we're removing) and the in-crate tests. Deprecating it emits a compiler warning that catches any *future* stray local instantiation while the migration proceeds. The production `dead_rules_endpoint` reads from the singleton. Tests that need isolation call `RuleExecutionTracker::clear()` under `#[serial]` (mirrors `CoverageTracking` test isolation). Planner may instead gate a test-only constructor behind `#[cfg(any(test, feature = "test-utils"))]` if a deprecation attribute is too noisy — exact mechanism is Claude's discretion; the contract is "no production code instantiates a local `RuleExecutionTracker`."
  - `[auto]` Q: "Remove `new()` entirely, deprecate it, or gate it behind `#[cfg(test)]`?" → Selected: **"Deprecate `new()` + rewrite the one production call + migrate tests to singleton+`reset()`"** (recommended — catches stray usage via compiler warning with lowest risk; preserves test ergonomics)

#### Parallel dead-rule / tracker systems (scope guard)
- **D-05: The parallel `RuleHealthRegistry` (`engine/src/monitoring/dead_rules.rs`) and `coverage/history/mod.rs` tracker are OUT of scope for Phase 26 — document, do not merge.** The codebase has **two other** rule-tracking systems the ROADMAP didn't enumerate: (1) `RuleHealthRegistry` in `engine/src/monitoring/dead_rules.rs` with its own `record_fire`/`detect_dead_rules`/`RuleHealth`/`RuleActivity` (a v1.0-era system, **not wired to the live `direct_eval.rs` execute path** — no calls from the evaluator); (2) `engine/src/coverage/history/mod.rs` with `record_evaluation`/`record_fire` (a coverage-history tracker, also separate). Phase 26 unifies the **`RuleExecutionTracker`↔`CoverageTracking`↔per-engine-metrics** drift only (the three the ROADMAP names). Merging `RuleHealthRegistry` and the history tracker into the unified source-of-truth is a larger consolidation and is **deferred** — the researcher should document these parallel systems in `26-RESEARCH.md` (so the planner knows they exist and doesn't accidentally double-wire dead-rule detection) but Phase 26 does not merge or remove them.
  - `[auto]` Q: "Are the parallel `RuleHealthRegistry` and `coverage/history` trackers in scope for the unification?" → Selected: **"Out of scope — document them, unify only the three ROADMAP-named counters"** (recommended — ROADMAP scopes Phase 26 to `RuleExecutionTracker`+`CoverageTracking`+per-engine-metrics; merging the v1.0-era `RuleHealthRegistry` is its own consolidation phase)

#### Timestamp source for `record_fire`
- **D-06: The singleton stamps `last_fired_at` internally; the call site passes only `rule_id`/`rule_name`.** The current `record_fire` signature takes a `timestamp: impl Into<String>` param (`detection.rs:92-113`) and the `direct_eval.rs` call site has no timestamp handy. Mirroring `CoverageTracking`'s `record_rule_fire(rule_id)` signature, the singleton generates the timestamp internally via `chrono::Utc::now().to_rfc3339()` (chrono is already a workspace dep — `dead_rules.rs` and `detection.rs` use it). This keeps the call site clean and guarantees `last_fired_at` is always truthful. Exact timestamp format (RFC3339 vs unix epoch) is Claude's discretion.
  - `[auto]` Q: "How should `last_fired_at` be populated — pass a timestamp to `record_fire`, or have the singleton stamp it internally?" → Selected: **"Singleton stamps internally via `chrono::Utc::now()`; call site passes `rule_id`/`rule_name` only"** (recommended — matches `CoverageTracking` signature; call site has no timestamp; truthful `last_fired_at`)

#### E2E keystone test (SC-1/SC-2/SC-3)
- **D-07: Extend `api/tests/execute_integration.rs` with the keystone e2e + three-surfaces-agreement assertion, mirroring the Phase 23/24/25 Wave 0 RED→GREEN + Nyquist pattern.** Keystone scenario (SC-1 + SC-2): author rule `r1` (e.g. `score := score + 10`), `POST /v1/execute` with a fact that triggers `r1`, then assert (a) `GET /v1/metrics/dead-rules` does NOT contain `r1` (previously always `[]`), (b) `GET /v1/metrics/coverage` shows `r1.fired == true` + `r1.evaluation_count >= 1`. SC-3 (three-surfaces agreement): after the run, `/metrics` (Prometheus), `/v1/metrics/coverage`, and `/v1/metrics/dead-rules` agree on `r1`'s fire count — assert the coverage `fire_count`, the dead-rules exclusion (fire_count > 0 ⇒ not dead), and the prometheus counter are consistent. Use `serial_test` (`#[serial]`) for singleton isolation since `CoverageTracking` + `RuleExecutionTracker` are process-global. RED first (dead-rules returns `[]` today so the "excludes r1" assertion already passes trivially — the RED scaffold must assert against a SECOND unfired rule `r2` that SHOULD appear in dead-rules but doesn't, proving the panel is actually live), then GREEN.
  - `[auto]` Q: "Where to place the e2e keystone test and what should it assert?" → Selected: **"Extend `api/tests/execute_integration.rs`; keystone fires `r1` + leaves `r2` unfired; assert dead-rules excludes `r1` AND includes `r2`, coverage shows `r1.fired`, three surfaces agree on fire count"** (recommended — matches Phase 23/24/25 test pattern; the `r2` unfired rule is what makes the RED scaffold meaningful, since `[]`-excludes-`r1` is trivially true today)

### Claude's Discretion (technical impl details left to the planner/researcher)
- **Singleton state shape** — whether to reuse the existing `RuleExecutionTracker { rules, total_evaluations, total_fires }` struct as the `Mutex<…>` payload, or factor a `TrackerState` struct like `CoverageTracking`'s `TrackingState`. The contract is "process-global, `LazyLock<Mutex<…>>`, associated-function API, owned return from `find_dead_rules`." Planner picks the cleanest.
- **`CoverageTracking` extension vs. delegation for dead-rule fields** — `last_fired_at`/`rule_name`/`category` aren't in `RuleCoverageState`. Either (a) extend `RuleCoverageState` with these so `RuleExecutionTracker` becomes a pure view over `CoverageTracking`, or (b) keep `RuleExecutionTracker` as a separate singleton fed at the same `direct_eval.rs` call sites. D-02 allows either; planner picks based on which is cleaner. The single-source-of-truth contract is about COUNTS (`fired`/`fire_count`/`evaluated`/`evaluation_count`) — the metadata fields may live in either store.
- **Per-engine `metrics.record_rule_fired` fate** — verify whether the per-engine `EngineMetrics::record_rule_fired` (Rete/Treat/Leaps/Phreak) still feeds any live surface (Phase 24 `MetricsSnapshot` gauges) or is now redundant. If redundant on the dead-rule/coverage contract, leave it for the per-engine gauges but document that it is NOT a source of truth for fire counts. If it drifts, remove from the execute path. Researcher confirms against Phase 24 wiring.
- **Deprecation mechanism** — `#[deprecated]` on `new()` vs. `#[cfg(test)]`-gated test constructor vs. `pub(crate)` + rewrite. D-04's contract is "no production local instantiation"; mechanism is planner's choice.
- **Test isolation** — whether to add a `RuleExecutionTracker::clear()` (mirrors `CoverageTracking`) and use `serial_test`, or reset via `LazyLock` re-init. Mirror `CoverageTracking`'s established approach.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap & requirements (scope + success criteria)
- `.planning/ROADMAP.md` §"Phase 26: RuleExecutionTracker singleton + dead-rule live data" (line 188) — goal, depends-on (Phase 23), requirement (MON-06), 5 key deliverables, 2 pitfalls to avoid (3, 6), research flags ("None — direct mirror of the existing `CoverageTracking` `LazyLock<Mutex<…>>` pattern; call sites already known"), and the 3 success criteria verification must satisfy. **Note the stale call-site line refs (`rete/mod.rs:293,332`) — see L-02/D-03 correction.**
- `.planning/REQUIREMENTS.md` — traceability: `MON-06` → Phase 26 (Pending).

### Milestone research (pitfalls — HIGH confidence, file:line evidence)
- `.planning/research/PITFALLS.md` §"Pitfall 3" — stale/empty tracker: `dead_rules_endpoint` instantiates a fresh empty tracker per call (`metrics.rs:380`); singleton-ize; remove/deprecate public `new()` to catch stray local instantiation (D-04).
- `.planning/research/PITFALLS.md` §"Pitfall 6" — three-counter drift: `CoverageTracking`, `RuleExecutionTracker.total_fires`, per-engine `metrics.record_rule_fired`; unify around `CoverageTracking` as canonical; record once-per-rule outside the inner loop (D-02/L-04).

### Prior phase context (the scaffold Phase 26 builds on)
- `.planning/phases/23-execute-evaluator-wiring/23-CONTEXT.md` — **L-06** (once-per-rule coverage outside the inner loop / Pitfall 6), the `CoverageTracking` singleton pattern reference (`coverage.rs:86`), and the deferred item "`RuleExecutionTracker` singleton-ization — Phase 26" (line 141). Also confirms `serial_test` is a workspace dev-dep.
- `.planning/phases/23-execute-evaluator-wiring/23-06-PLAN.md` (or `23-06-SUMMARY.md`) — origin of the shared `engine/src/engine/direct_eval.rs` evaluator that all four engines delegate to (the actual recording call site — L-02/D-3 correction). Confirms `fired_set`/`evaluated_set` `HashSet` once-per-rule semantics.
- `.planning/phases/25-real-reload-jobstore/25-CONTEXT.md` — confirms the live execute path is real and records into `CoverageTracking`; Phase 26 adds the parallel `RuleExecutionTracker` recording at the same call sites. (No `24-CONTEXT.md` exists — Phase 24 ran without one.)

### Live code (the actual integration points — verified during this discussion)
- `engine/src/monitoring/detection.rs:64-191` — `RuleExecutionTracker` struct + `&mut self` methods (`new`, `record_evaluation`, `record_fire`, `find_dead_rules`, `reset`, `clear`) to convert to singleton.
- `engine/src/monitoring/coverage.rs:83-158` — the `CoverageTracking` `LazyLock<Mutex<TrackingState>>` singleton pattern to mirror (L-01/D-01).
- `engine/src/engine/direct_eval.rs:100-110` — the canonical once-per-rule recording call site (next to which `RuleExecutionTracker` calls go — L-02/D-03).
- `api/src/metrics.rs:369-386` — `dead_rules_endpoint` to rewrite to read from the singleton (L-03/D-04).
- `engine/src/monitoring/dead_rules.rs` + `engine/src/coverage/history/mod.rs` — the parallel tracker systems that are OUT of scope (D-05); researcher documents, planner does not merge.

### No external specs
No ADRs or external standard specs apply — brownfield internal-wiring phase against the project's own Rust codebase.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`CoverageTracking` singleton** (`engine/src/monitoring/coverage.rs:86` — `static INSTANCE: LazyLock<Mutex<TrackingState>>`) — the exact pattern to mirror for `RuleExecutionTracker`: `LazyLock<Mutex<…>>` + associated-function API (`record_rule_eval(rule_id)`, `record_rule_fire(rule_id)`) + owned returns from inside the lock (`get_all_states() -> Vec<(String, RuleCoverageState)>`). Phase 26 copies this shape onto `RuleExecutionTracker`.
- **Shared direct evaluator** (`engine/src/engine/direct_eval.rs`) — the single once-per-rule recording call site for all four engines. Already collects `evaluated_set` + `fired_set` (`HashSet<String>`) and calls `CoverageTracking::record_rule_eval`/`record_rule_fire` after the fixpoint loop (`direct_eval.rs:106,109`). Phase 26 adds `RuleExecutionTracker::record_evaluation`/`record_fire` immediately adjacent. `rule_name` is available from the `Rule` being iterated.
- **`dead_rules_endpoint`** (`api/src/metrics.rs:378-386`) — the one production call site of `RuleExecutionTracker::new()`; rewrite to `RuleExecutionTracker::find_dead_rules()` (singleton) and map to `Vec<String>` as today.
- **`serial_test`** (workspace dev-dep) — the established singleton-isolation tool for `CoverageTracking` tests; Phase 26 tests mirror it (`#[serial]` + `clear()` between tests).
- **`chrono`** (workspace dep) — already used in `detection.rs`/`dead_rules.rs`; D-06 uses `chrono::Utc::now().to_rfc3339()` for the singleton-stamped `last_fired_at`.

### Established Patterns
- **Singleton trackers via `LazyLock<Mutex<…>>`** — `CoverageTracking` (`coverage.rs:86`) is THE established pattern; Phase 26 mirrors it (L-01). Associated functions, no `&self`/`&mut self`, owned returns from inside the lock.
- **Once-per-rule recording outside the inner loop** (Phase 23 L-06 / Pitfall 6) — `direct_eval.rs` collects `HashSet`s then records once per id; Phase 26's `RuleExecutionTracker` recording rides the same sets (L-04).
- **Wave 0 RED → GREEN + Nyquist closure** (Phase 23/24/25) — write the SC-1/SC-2/SC-3 integration tests RED first, then implement to GREEN. The `r2`-unfired-rule trick (D-07) makes the dead-rules RED scaffold meaningful.

### Integration Points
- **`engine/src/monitoring/detection.rs:64-191`** — convert `RuleExecutionTracker` to singleton: add `static INSTANCE: LazyLock<Mutex<…>>`, convert `&mut self` methods to associated functions, `find_dead_rules() -> Vec<DeadRuleInfo>` (owned), deprecate `new()` (D-01/D-04).
- **`engine/src/engine/direct_eval.rs:~106,~109`** — add `RuleExecutionTracker::record_evaluation(id, name)` + `RuleExecutionTracker::record_fire(id, name)` next to the existing `CoverageTracking` calls (L-02/D-03).
- **`api/src/metrics.rs:378-386`** — `dead_rules_endpoint` reads `RuleExecutionTracker::find_dead_rules()` from the singleton; remove `RuleExecutionTracker::new()` (L-03/D-04).
- **`api/tests/execute_integration.rs`** — add the SC-1/SC-2/SC-3 keystone e2e (fire `r1`, leave `r2` unfired, assert dead-rules excludes `r1` AND includes `r2`, coverage shows `r1.fired`, three surfaces agree) (D-07).

</code_context>

<specifics>
## Specific Ideas

- **Keystone honesty anchor (SC-1 + SC-2):** author two rules — `r1` (`score := score + 10`, fires) and `r2` (a rule whose condition never matches the inserted fact, does NOT fire). `POST /v1/execute` with the triggering fact. Assert: `GET /v1/metrics/dead-rules` returns a list that **excludes `r1`** AND **includes `r2`** (the `r2` inclusion is the real proof — `[]`-excludes-`r1` is trivially true today, so without `r2` the test would pass against the broken code). Assert `GET /v1/metrics/coverage` shows `r1.fired == true` + `r1.evaluation_count >= 1` and `r2.fired == false`. This single scenario exercises the singleton (D-01), the live call site (D-03), the dead-rules endpoint rewrite (L-03), and once-per-rule recording (L-04).
- **SC-3 (three-surfaces agreement):** after the keystone run, assert the fire count for `r1` is consistent across `GET /metrics` (Prometheus `rrules_execute_rules_fired_total` or per-engine gauge), `GET /v1/metrics/coverage` (`fire_count`), and `GET /v1/metrics/dead-rules` (`r1` excluded ⇒ fire_count > 0). The single-source-of-truth design (D-02) is what makes this pass; if it drifts, the test fails.
- The user's consistent posture across v5.0 (Phase 23/25 CONTEXT "Specifics") is to take the **honest-for-real-authored-rules** option. The planner should not narrow D-02 (single source of truth) to "minimal that passes the dead-rules-returns-`[]` fix" — the three-counter unification (SC-3) must be honest, not just the dead-rules panel. The `r2` unfired rule in the keystone is what keeps this honest.

</specifics>

<deferred>
## Deferred Ideas

- **Merge `RuleHealthRegistry` (`engine/src/monitoring/dead_rules.rs`) into the unified tracker** — a separate v1.0-era dead-rule system with its own `RuleHealth`/`RuleActivity`/`detect_dead_rules`, not wired to the live `direct_eval.rs` execute path. Phase 26 documents it (D-05) but does not merge or remove it. A future consolidation phase could unify all rule-tracking systems.
- **Merge `coverage/history/mod.rs` tracker** — another parallel `record_evaluation`/`record_fire` tracker (coverage history). Same treatment: document, defer (D-05).
- **Stateful sessions** — Phase 27.
- **Frontend dead-rules panel / test-runner UI** — Phase 28 (the v4.0 dashboard adapters already tolerate real shapes; Phase 26 only feeds live data).
- **`inference_trace` population** — Phase 28 (EXEC-04).

None of these were scope-creep suggestions from the user — they are the phase's own downstream milestones / explicit out-of-scope parallel systems, listed so the planner knows the boundaries.

</deferred>

---

*Phase: 26-ruleexecutiontracker-singleton-dead-rule-live-data*
*Context gathered: 2026-07-03 (--auto mode)*