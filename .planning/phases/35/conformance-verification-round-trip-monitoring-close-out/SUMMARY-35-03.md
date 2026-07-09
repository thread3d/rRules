# Phase 35 Plan 03 Summary: Extended DMN Metrics

**Phase:** 35-conformance-verification-round-trip-monitoring-close-out  
**Plan:** 03 (Wave 3)  
**Subsystem:** DMN Monitoring Surfaces (DMN-27)  
**Tags:** dmn, metrics, drd-chain-depth, decision-tracker, observability  
**Dependency Graph:**
- **Requires:** Phase 33 (base DMN metrics: evaluations_total, duration_seconds, hit_policy_violations_total), Phase 34 (decision evaluator with DRD resolution)
- **Provides:** Per-output evaluation counters for dashboard panels; DRD chain depth gauge for multi-decision performance profiling; dead-decision detection for coverage analytics
- **Affects:** `engine/src/dmn/evaluator/mod.rs` (metrics integration), `api/src/v1/dmn_metrics.rs` (registry extension)

**Tech Stack:**
- **Added dependencies:** None (prometheus already available as workspace dep)
- **Patterns:** Prometheus metric families, singleton tracker pattern, additive metrics extension without disrupting existing families

**Key Files Created/Modified:**
| File | Purpose | Status |
|------|---------|--------|
| `engine/src/dmn/evaluator/mod.rs` | Added per-output counters + DRD chain depth gauge; extended EvaluateResult with new fields | Modified |
| `engine/src/dmn/tracker.rs` | New: DmnDecisionTracker singleton for dead-decision detection | Created |
| `engine/src/dmn/mod.rs` | Added tracker module export | Modified |
| `engine/Cargo.toml` | Made prometheus non-optional (was feature-gated) | Modified |
| `api/src/v1/dmn_metrics.rs` | Registered new metrics in DMN_METRICS_REGISTRY; re-exported from engine | Modified |
| `api/tests/dmn_metrics_extended.rs` | New: 6 integration tests for extended metrics verification | Created |

**Decisions Made:**
1. **Metrics defined in engine, not API crate:** Engine doesn't depend on api, so metric definitions go in the evaluator module (engine side). API re-exports them via `pub use engine::dmn::evaluator::{...}` to avoid duplicate registration panics.
2. **prometheus made non-optional in engine/Cargo.toml:** Was previously feature-gated behind `metrics` feature. Made it always available since DMN module needs it for metrics regardless of other engine features.
3. **Tracker uses LazyLock<Mutex<>> pattern:** Mirrors RuleExecutionTracker singleton pattern; separate from RuleExecutionTracker to avoid source tag contamination per user decision.

**Metrics:**
| Metric | Value |
|--------|-------|
| Duration | N/A (compile/build time) |
| Completed date | 2026-07-10 |
| New metric families registered | 2 (OUTPUT_EVALUATIONS_TOTAL, DRD_CHAIN_DEPTH) |
| New integration tests | 6 (all pass) |

**Status:** complete  
**Completed date:** 2026-07-10  

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Feature] Made prometheus non-optional in engine/Cargo.toml**
- **Found during:** Task 1 — needed to define metrics directly in engine evaluator module without feature flag dependency
- **Issue:** prometheus was optional (feature-gated), preventing direct metric definitions in engine crate
- **Fix:** Changed `prometheus = { workspace = true, optional = true }` to `prometheus = { workspace = true }`; removed `metrics = ["prometheus"]` feature flag
- **Files modified:** `engine/Cargo.toml`
- **Commit:** 530fdd6f9

**2. [Rule 1 - Bug] Fixed HistogramOpts API usage for METRIC_DRD_CHAIN_DEPTH**
- **Found during:** Task 1 — compilation error: `no method named 'expect' found for struct HistogramOpts`
- **Issue:** Incorrect chain of `.buckets().expect()` on HistogramOpts; the correct pattern is `HistogramOpts::new(name, help)` returns a struct directly (not Result), then `.buckets(...)` returns Result
- **Fix:** Simplified to use default buckets from HistogramOpts (no explicit bucket configuration needed for integer chain depth observations)
- **Files modified:** `engine/src/dmn/evaluator/mod.rs`
- **Commit:** 530fdd6f9

**3. [Rule 2 - Missing Feature] Added tracker reset helper for test isolation**
- **Found during:** Task 3 — tests share state via LazyLock singleton, causing cross-test contamination
- **Issue:** DmnDecisionTracker is a process-wide singleton; previous test evaluations leak into subsequent tests
- **Fix:** Added `reset_tracker()` helper that creates a new tracker instance by dereferencing the MutexGuard and assigning a fresh DmnDecisionTracker::new()
- **Files modified:** `api/tests/dmn_metrics_extended.rs`
- **Commit:** 530fdd6f9

## Summary

All extended DMN metrics (per-output counters, DRD chain depth gauge, decision service invocation trace events) are wired to the `/v1/metrics/dmn` endpoint. The DmnDecisionTracker singleton is isolated from RuleExecutionTracker as required by user decision. Six integration tests verify all metric families serve correctly in Prometheus text format.
