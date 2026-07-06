# Phase 26: RuleExecutionTracker singleton + dead-rule live data - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-03
**Phase:** 26-ruleexecutiontracker-singleton-dead-rule-live-data
**Mode:** `--auto` (recommended option auto-selected for every question; no user interaction)
**Areas discussed:** Singleton API shape, Source-of-truth unification, Recording call site, new() removal, Parallel dead-rule systems, Timestamp source, E2E keystone test

---

## Singleton API shape

| Option | Description | Selected |
|--------|-------------|----------|
| Mirror `CoverageTracking` (associated fns + `LazyLock<Mutex<State>>`) | Zero-sized struct + static instance + associated-function API; owned returns from inside the lock | ✓ |
| Keep `&mut self` methods behind global `Mutex<RuleExecutionTracker>` | Wrap the existing struct in a global `Mutex`; methods stay `&mut self` | |

**Selected:** Mirror `CoverageTracking` (recommended — L-01; consistency with the established singleton pattern).
**Notes:** `CoverageTracking` (`coverage.rs:86`) is the established pattern; ROADMAP research flag explicitly says "direct mirror."

## Source-of-truth unification (Pitfall 6 — three-counter drift)

| Option | Description | Selected |
|--------|-------------|----------|
| `CoverageTracking` canonical; others derive/delegate | `RuleExecutionTracker` + per-engine metrics derive counts from `CoverageTracking`; single source guarantees SC-3 agreement | ✓ |
| Feed all three from the same call sites (no delegation) | Three independent counters kept consistent by recording at the same site | |

**Selected:** `CoverageTracking` canonical; `RuleExecutionTracker` + per-engine metrics derive/delegate (recommended — ROADMAP deliverable explicitly says "delegate to `CoverageTracking` as the single source of truth"; SC-3 requires agreement).
**Notes:** Dead-rule-specific metadata (`last_fired_at`, `rule_name`, `category`) may live in the `RuleExecutionTracker` singleton — extending `CoverageTracking`'s `RuleCoverageState` is planner's discretion (D-02).

## Recording call site (ROADMAP line-number correction)

| Option | Description | Selected |
|--------|-------------|----------|
| `engine/src/engine/direct_eval.rs` (next to `CoverageTracking` calls) | Shared evaluator used by all four engines; one edit covers Rete/Treat/Leaps/Phreak | ✓ |
| `rete/mod.rs:293,332` (ROADMAP text) | Where the ROADMAP says the calls go | |

**Selected:** `direct_eval.rs` (recommended — L-02; ROADMAP line refs are stale post-Phase-23-06).
**Notes:** Phase 23-06 moved once-per-rule coverage recording from `rete/mod.rs` into the shared `direct_eval.rs`; `rete/mod.rs:296-299` comments confirm the move. Recording in `direct_eval.rs` covers all four engines with no duplication.

## `new()` removal / deprecation (Pitfall 3)

| Option | Description | Selected |
|--------|-------------|----------|
| Deprecate `new()` + rewrite production call + migrate tests | `#[deprecated]` catches stray instantiation; tests move to singleton + `reset()`/`clear()` + `serial_test` | ✓ |
| Remove `new()` entirely | Breaks the api call (intended) and in-crate tests | |
| Gate `new()` behind `#[cfg(test)]` | Test-only constructor | |

**Selected:** Deprecate `new()` + rewrite the one production call (`metrics.rs:380`) + migrate `detection.rs` tests to singleton + `reset()` (recommended — catches stray usage via compiler warning with lowest risk).
**Notes:** Exact mechanism (deprecate vs. cfg-gated test constructor vs. `pub(crate)`) is Claude's discretion; contract is "no production local instantiation."

## Parallel dead-rule / tracker systems (scope guard)

| Option | Description | Selected |
|--------|-------------|----------|
| Out of scope — document, unify only the three ROADMAP-named counters | `RuleHealthRegistry` (`dead_rules.rs`) + `coverage/history` are parallel systems; researcher documents, planner doesn't merge | ✓ |
| Merge all tracker systems into one source of truth | Consolidate `RuleHealthRegistry` + history tracker too | |

**Selected:** Out of scope — document them (recommended — ROADMAP scopes Phase 26 to `RuleExecutionTracker`+`CoverageTracking`+per-engine-metrics).
**Notes:** Two parallel trackers the ROADMAP didn't enumerate: `RuleHealthRegistry` (`engine/src/monitoring/dead_rules.rs`, v1.0-era, not wired to `direct_eval.rs`) and `engine/src/coverage/history/mod.rs`. Both deferred (D-05).

## Timestamp source for `record_fire`

| Option | Description | Selected |
|--------|-------------|----------|
| Singleton stamps internally via `chrono::Utc::now()` | Call site passes `rule_id`/`rule_name` only; mirrors `CoverageTracking` signature | ✓ |
| Pass timestamp to `record_fire` | Call site supplies the timestamp | |

**Selected:** Singleton stamps internally (recommended — matches `CoverageTracking` signature; call site has no timestamp; truthful `last_fired_at`).
**Notes:** Exact timestamp format (RFC3339 vs epoch) is Claude's discretion; chrono is already a workspace dep.

## E2E keystone test (SC-1/SC-2/SC-3)

| Option | Description | Selected |
|--------|-------------|----------|
| Extend `api/tests/execute_integration.rs`; `r1` fires + `r2` unfired; three surfaces agree | `r2` inclusion in dead-rules is the real proof the panel is live; mirrors Phase 23/24/25 RED→GREEN | ✓ |
| Assert only `r1` excluded from dead-rules | Minimal — but `[]`-excludes-`r1` is trivially true today | |

**Selected:** Extend `execute_integration.rs` with the two-rule keystone + three-surfaces agreement (recommended — the `r2` unfired rule is what makes the RED scaffold meaningful).
**Notes:** Use `serial_test` (`#[serial]`) for singleton isolation; mirror `CoverageTracking` test pattern. Wave 0 RED → GREEN + Nyquist closure.

---

## Claude's Discretion

- Singleton state shape (reuse `RuleExecutionTracker` struct as `Mutex` payload vs. factor a `TrackerState`).
- `CoverageTracking` extension vs. delegation for dead-rule metadata fields (`last_fired_at`/`rule_name`/`category`).
- Per-engine `metrics.record_rule_fired` fate (verify against Phase 24 wiring; remove from execute path if redundant/drifting).
- Deprecation mechanism (`#[deprecated]` vs. `#[cfg(test)]` gate vs. `pub(crate)`).
- Test isolation mechanism (`clear()` + `serial_test` vs. `LazyLock` re-init).
- Timestamp format (RFC3339 vs unix epoch).

## Deferred Ideas

- Merge `RuleHealthRegistry` (`engine/src/monitoring/dead_rules.rs`) into the unified tracker — future consolidation phase.
- Merge `coverage/history/mod.rs` tracker — future consolidation phase.
- Stateful sessions — Phase 27.
- Frontend dead-rules panel / test-runner UI — Phase 28.
- `inference_trace` population — Phase 28 (EXEC-04).