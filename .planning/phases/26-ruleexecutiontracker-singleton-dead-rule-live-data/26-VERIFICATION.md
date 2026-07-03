---
phase: 26-ruleexecutiontracker-singleton-dead-rule-live-data
verified: 2026-07-03T00:00:00Z
status: passed
score: 10/10 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification:
  previous_status: none
  previous_score: n/a
  gaps_closed: []
  gaps_remaining: []
  regressions: []
---

# Phase 26: RuleExecutionTracker Singleton + Dead-Rule Live Data Verification Report

**Phase Goal:** `RuleExecutionTracker` becomes a process-wide singleton so dead-rule detection reads live firings — `GET /v1/metrics/dead-rules` no longer always returns `[]`.
**Verified:** 2026-07-03
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

Roadmap success criteria (SC-1/SC-2/SC-3) plus PLAN must_haves merged. The 3 SCs are the load-bearing contract; PLAN truths add wiring/API detail.

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SC-1: After `POST /v1/execute` fires `r1`, `GET /v1/metrics/dead-rules` does NOT contain `r1` (previously always returned `[]`) | ✓ VERIFIED | `keystone_dead_rules` e2e (api/tests/execute_integration.rs:1270) asserts `!rule_ids.contains("r1")` AND `rule_ids.contains("r2")` — the r2-INCLUSION assertion is the RED→GREEN proof. Test passes GREEN: `cargo test -p api --test execute_integration keystone` → 3 passed. |
| 2 | SC-2: `GET /v1/metrics/coverage` shows `r1.fired == true` and `r1.evaluation_count >= 1` after the run | ✓ VERIFIED | `keystone_coverage` e2e (api/tests/execute_integration.rs:1310) asserts `r1["fired"] == true`, `r1["evaluation_count"] >= 1`, `r2["fired"] == false`. Passes GREEN. |
| 3 | SC-3: `/metrics`, `/v1/metrics/coverage`, and `/v1/metrics/dead-rules` agree on the fire count for a rule (no three-counter drift) | ✓ VERIFIED | `keystone_three_surfaces_agree` e2e (api/tests/execute_integration.rs:1361) parses Prometheus `rrules_execute_rules_fired_total{engine="rete"} >= 1`, asserts coverage `r1.fire_count >= 1`, asserts dead-rules excludes `r1`. All three derive from shared `fired_set` (D-02). Passes GREEN. |
| 4 | RuleExecutionTracker is a process-wide singleton backed by `static INSTANCE: LazyLock<Mutex<TrackerState>>` mirroring CoverageTracking | ✓ VERIFIED | engine/src/monitoring/detection.rs:88-97 — `pub struct RuleExecutionTracker;` (unit-like) + `static INSTANCE: LazyLock<Mutex<TrackerState>> = LazyLock::new(\|\| Mutex::new(TrackerState { rules: HashMap::new(), total_evaluations: 0, total_fires: 0 }));`. Mirrors coverage.rs pattern. |
| 5 | `record_evaluation(rule_id, rule_name)` and `record_fire(rule_id, rule_name)` are associated functions (no `&mut self`); singleton stamps `last_fired_at` internally via `chrono::Utc::now().to_rfc3339()` | ✓ VERIFIED | detection.rs:117-147 — both are `pub fn` associated functions acquiring `INSTANCE.lock().unwrap()`; `record_fire` stamps `let timestamp = chrono::Utc::now().to_rfc3339();` internally (D-06). `test_tracker_records_fires` asserts `last_fired_at.is_some()` — passes GREEN. |
| 6 | `find_dead_rules() -> Vec<DeadRuleInfo>` returns owned data; `clear()` resets singleton for test isolation; `new()` is `#[deprecated]` | ✓ VERIFIED | detection.rs:175-183 (owned `.cloned().collect()`), :244-249 (clear wipes rules + counters), :109-114 (`#[deprecated]` on `new()`). `grep "RuleExecutionTracker::new()" api/src/metrics.rs` returns 0 matches — no production call site. |
| 7 | The direct evaluator at direct_eval.rs records `RuleExecutionTracker::record_evaluation` + `record_fire` adjacent to CoverageTracking (covers Treat/Leaps/Phreak) | ✓ VERIFIED | engine/src/engine/direct_eval.rs:113-122 — `let id_to_name: HashMap<&String, &String> = rules.iter().map(\|r\| (&r.id, &r.name)).collect();` built once before loops; `RuleExecutionTracker::record_evaluation(id, id_to_name[id])` inside `for id in &evaluated_set`; `RuleExecutionTracker::record_fire(id, id_to_name[id])` inside `for id in &fired_set`. |
| 8 | The Rete inlined `execute_rules` at rete/mod.rs records `RuleExecutionTracker::record_evaluation` + `record_fire` (C1 — Rete does NOT delegate to direct_eval.rs) | ✓ VERIFIED | engine/src/engine/rete/mod.rs:400-409 — same id→name pattern from `self.rules`, same recording calls adjacent to CoverageTracking. C1 honored. |
| 9 | `dead_rules_endpoint` reads `RuleExecutionTracker::find_dead_rules()` from the singleton — no local `RuleExecutionTracker::new()` | ✓ VERIFIED | api/src/metrics.rs:386 — `let dead_rules = RuleExecutionTracker::find_dead_rules();`; :389 — `let rule_ids: Vec<String> = dead_rules.into_iter().map(\|info\| info.rule_id).collect();` (move, no clone). `grep "RuleExecutionTracker::new()" api/src/metrics.rs` = 0 matches. |
| 10 | The 7 detection.rs tests (6 migrated + `test_once_per_rule`) pass under `#[serial_test::serial]` + `clear()` | ✓ VERIFIED | `cargo test -p engine --lib monitoring::detection` → 7 passed, 0 failed. `test_once_per_rule` (detection.rs:450) asserts `evaluation_count == 2` after two `record_evaluation("r1", "R1")` calls — Pitfall 6 / L-04 regression guard. |

**Score:** 10/10 truths verified (0 present, behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| engine/src/monitoring/detection.rs | Singleton + associated-function API + deprecated new() + 7 serial tests | ✓ VERIFIED | 462 lines; TrackerState, static INSTANCE, unit struct, all associated fns, deprecated new(), DeadRuleReport::generate() as associated fn, 7 #[serial] tests. Substantive + wired (imported by direct_eval.rs, rete/mod.rs, metrics.rs, execute_integration.rs). |
| engine/src/engine/direct_eval.rs | RuleExecutionTracker recording at post-fixpoint block | ✓ VERIFIED | Lines 113-122: id_to_name HashMap + record_evaluation + record_fire adjacent to CoverageTracking. |
| engine/src/engine/rete/mod.rs | RuleExecutionTracker recording at post-fixpoint block (C1) | ✓ VERIFIED | Lines 400-409: same pattern from self.rules. C1 honored. |
| api/src/metrics.rs | dead_rules_endpoint reads singleton | ✓ VERIFIED | Lines 378-391: RuleExecutionTracker::find_dead_rules() associated fn; no new(). |
| api/tests/execute_integration.rs | 3 keystone e2e tests (SC-1/SC-2/SC-3) | ✓ VERIFIED | keystone_dead_rules:1270, keystone_coverage:1310, keystone_three_surfaces_agree:1361. All pass GREEN. Plus never_fires_rule r2 fixture:145, two_rule_shared_ruleset:164, init_two_rule_app! macro:175, /v1/metrics/dead-rules route registered:274, reset_test_env clears RuleExecutionTracker:331. |
| api/tests/metrics_integration.rs | Stale "always returns []" comment corrected | ✓ VERIFIED | Line 91-97: comment now reflects live singleton read; references keystone_dead_rules for non-emptiness proof. dead_rules_returns_valid_json_array passes GREEN. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| POST /v1/execute | evaluator (direct_eval.rs OR rete/mod.rs) | execute_rules handler → engine.execute() → direct_execute/inline execute_rules → RuleExecutionTracker::record_fire | ✓ WIRED | Recording calls present at both sites; keystone_dead_rules proves r1 fires and r2 is recorded as evaluated-but-not-fired. |
| Evaluator recording block | RuleExecutionTracker singleton | `RuleExecutionTracker::record_evaluation` / `record_fire` associated fns acquiring INSTANCE lock | ✓ WIRED | direct_eval.rs:117,121 + rete/mod.rs:404,408 call the singleton API; detection unit tests prove the API mutates the singleton state. |
| Singleton | dead_rules_endpoint | `RuleExecutionTracker::find_dead_rules()` returns owned Vec<DeadRuleInfo> → mapped to Vec<String> | ✓ WIRED | metrics.rs:386-389; keystone_dead_rules asserts the endpoint returns the live list (r2 included). |
| fired_set | three metric surfaces (Prometheus, coverage, dead-rules) | shared fired_set origin (D-02) | ✓ WIRED | keystone_three_surfaces_agree parses all three surfaces and asserts agreement (Prometheus >= 1, coverage fire_count >= 1, dead-rules excludes r1). |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| dead_rules_endpoint (metrics.rs:378) | `dead_rules` | `RuleExecutionTracker::find_dead_rules()` singleton read | Yes — singleton populated by live evaluator recording (direct_eval.rs + rete/mod.rs) | ✓ FLOWING |
| keystone_dead_rules test | `rule_ids` | HTTP GET /v1/metrics/dead-rules → dead_rules_endpoint | Yes — r2 included after r1 fires (assertion passes) | ✓ FLOWING |
| keystone_coverage test | `r1["fired"]`, `r1["evaluation_count"]` | HTTP GET /v1/metrics/coverage → CoverageTracking::to_json() | Yes — r1.fired == true, evaluation_count >= 1 | ✓ FLOWING |
| keystone_three_surfaces_agree | `prom_fired_total`, `cov_fire_count`, `dead_ids` | 3 HTTP endpoints | Yes — all three >= 1 / excludes r1 | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| SC-1: dead-rules excludes r1, includes r2 | `cargo test -p api --test execute_integration keystone` | 3 passed (keystone_dead_rules, keystone_coverage, keystone_three_surfaces_agree) | ✓ PASS |
| SC-2: coverage shows r1.fired + r2 not fired | (same command — keystone_coverage) | passed | ✓ PASS |
| SC-3: three surfaces agree on fire count | (same command — keystone_three_surfaces_agree) | passed | ✓ PASS |
| Singleton API: 7 detection unit tests | `cargo test -p engine --lib monitoring::detection` | 7 passed, 0 failed | ✓ PASS |
| metrics_integration: dead-rules valid JSON array | `cargo test -p api --test metrics_integration` | 4 passed, 0 failed | ✓ PASS |
| test_once_per_rule (Pitfall 6 guard): evaluation_count == 2 after two record_evaluation calls | (included in detection unit tests above) | passed | ✓ PASS |

### Probe Execution

Not applicable — this phase does not declare or imply probe-based verification. The phase verification relies on `cargo test` (Rust test harness), which was run directly in Behavioral Spot-Checks above.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| MON-06 | 26-01-PLAN, 26-02-PLAN | Phase 26 traceability — dead-rule live data via singleton | ✓ SATISFIED | Singleton implemented (detection.rs); recording wired at both evaluator sites (direct_eval.rs + rete/mod.rs per C1); dead_rules_endpoint reads singleton; SC-1/SC-2/SC-3 keystone e2e GREEN. REQUIREMENTS.md:16 maps MON-06 → Phase 26 (status "Pending" — that table is updated at milestone close, not phase verification; the codebase evidence satisfies the requirement). |

No orphaned requirements — REQUIREMENTS.md maps only MON-06 to Phase 26, and both plans declare MON-06 in their `requirements:` frontmatter.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | — | — | — | No TBD/FIXME/XXX markers; no placeholder/stub patterns; no hardcoded empty data in production paths. | 

Additional regression checks:
- C4 honored: `engine/src/engine/metrics.rs` NOT modified in any Phase 26 commit (git log b23efc315^..HEAD -- engine/src/engine/metrics.rs returns empty). EngineMetrics.rules_fired left as per-engine gauge, out-of-contract for SC-3.
- D-05 honored: `engine/src/monitoring/dead_rules.rs`, `engine/src/coverage/history/mod.rs`, `engine/src/coverage/mod.rs` NOT modified in any Phase 26 commit. Parallel systems untouched.
- `#[deprecated]` on `new()` is the only debt marker adjacent to the phase scope — it is intentional (D-04 contract), referenced to the plan, and the production call site was removed in Plan 26-02.

### Human Verification Required

None. All truths verified with behavioral evidence (passing keystone e2e tests for SC-1/SC-2/SC-3 + unit tests for the singleton API). No visual/real-time/external-service items remain.

### Gaps Summary

No gaps. The phase goal is achieved:
- `RuleExecutionTracker` is a process-wide `LazyLock<Mutex<TrackerState>>` singleton (mirrors CoverageTracking).
- Live recording is wired at BOTH evaluator call sites (direct_eval.rs for Treat/Leaps/Phreak AND rete/mod.rs for Rete — C1 correction honored).
- `dead_rules_endpoint` reads `RuleExecutionTracker::find_dead_rules()` from the singleton — no `RuleExecutionTracker::new()` in metrics.rs.
- The 3 ROADMAP success criteria are pinned by 3 keystone e2e tests, all passing GREEN. The r2-unfired-rule trick (D-07) makes `keystone_dead_rules` a meaningful RED→GREEN proof (r2-INCLUSION is the load-bearing assertion that proves the panel is live, not just trivially empty).
- Three metric surfaces (Prometheus, coverage, dead-rules) agree on fire count by construction (D-02 — shared `fired_set`).
- Parallel systems (dead_rules.rs HealthRegistry, coverage/history, coverage/mod.rs CoverageTracker) and EngineMetrics.rules_fired (C4) untouched.

---

_Verified: 2026-07-03_
_Verifier: Claude (gsd-verifier)_