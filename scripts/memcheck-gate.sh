#!/usr/bin/env bash
# memcheck-gate.sh — DMN memory-regression gate (PERF-04).
#
# Compares a CURRENT memory snapshot against a frozen BASELINE snapshot and
# exits non-zero if RSS or total allocation counts regress beyond a threshold.
#
# This is the single source of truth for the regression gate: both `make
# memcheck` (local) and `.github/workflows/bench.yml` (CI) call this script,
# so the two paths can never drift apart (the original PERF-04 bug was that
# the CI workflow's inline jq compared a CURRENT file that was never
# produced against a baseline it overwrote every run — the gate always
# passed).
#
# Usage:
#   scripts/memcheck-gate.sh [BASELINE] [CURRENT]
#       BASELINE  default engine/benches/snapshots/baseline.json
#       CURRENT   default engine/benches/snapshots/current.json
#
# Env:
#   BENCH_RSS_THRESHOLD_PCT    percent regression allowed for RSS   (default 10)
#   BENCH_ALLOC_THRESHOLD_PCT  percent regression allowed for alloc (default 10)
#
# Exit codes:
#   0  no regression (or jq unavailable — gate is informational locally)
#   1  regression detected (RSS or allocations beyond threshold)
#   2  baseline or current snapshot missing

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BASELINE="${1:-$REPO_ROOT/engine/benches/snapshots/baseline.json}"
CURRENT="${2:-$REPO_ROOT/engine/benches/snapshots/current.json}"

RSS_THRESHOLD="${BENCH_RSS_THRESHOLD_PCT:-10}"
ALLOC_THRESHOLD="${BENCH_ALLOC_THRESHOLD_PCT:-10}"

if ! command -v jq &> /dev/null; then
    echo "memcheck-gate: jq not installed — skipping regression comparison."
    echo "  (CI installs jq; locally run 'sudo apt-get install -jq' to enable the gate.)"
    exit 0
fi

if [ ! -f "$BASELINE" ]; then
    echo "::error::Baseline snapshot not found: $BASELINE"
    echo "  Run 'make baseline-update' to capture a frozen baseline, then commit it."
    exit 2
fi

if [ ! -f "$CURRENT" ]; then
    echo "::error::Current snapshot not found: $CURRENT"
    echo "  Run 'make memcheck' first to generate the current snapshot."
    exit 2
fi

echo "memcheck-gate: comparing CURRENT vs BASELINE"
echo "  baseline: $BASELINE"
echo "  current : $CURRENT"
echo "  thresholds: RSS +${RSS_THRESHOLD}%  alloc +${ALLOC_THRESHOLD}%"

# --- RSS regression -------------------------------------------------------
# Both files are single MemoryBaseline objects with top-level peak_rss_bytes.
BASELINE_RSS=$(jq '.peak_rss_bytes' "$BASELINE")
CURRENT_RSS=$(jq '.peak_rss_bytes' "$CURRENT")

if [ "$BASELINE_RSS" = "null" ] || [ "$CURRENT_RSS" = "null" ]; then
    echo "::error::peak_rss_bytes missing from one of the snapshots."
    exit 2
fi

RSS_DELTA_PCT=0
if [ "$BASELINE_RSS" -gt 0 ]; then
    RSS_DELTA_PCT=$(( (CURRENT_RSS - BASELINE_RSS) * 100 / BASELINE_RSS ))
fi
# Treat negative deltas (improvements) as 0% for threshold comparison.
RSS_REGRESSION_PCT=$(( RSS_DELTA_PCT > 0 ? RSS_DELTA_PCT : 0 ))

echo "  peak_rss_bytes:  baseline=$BASELINE_RSS  current=$CURRENT_RSS  delta=${RSS_DELTA_PCT}%"

# --- Allocation regression ------------------------------------------------
# MemoryBaseline has no top-level total_allocations; sum per-phase
# allocation_count as the allocation proxy.
BASELINE_ALLOC=$(jq '[.phases[].allocation_count] | add // 0' "$BASELINE")
CURRENT_ALLOC=$(jq '[.phases[].allocation_count] | add // 0' "$CURRENT")

ALLOC_DELTA_PCT=0
if [ "$BASELINE_ALLOC" -gt 0 ]; then
    ALLOC_DELTA_PCT=$(( (CURRENT_ALLOC - BASELINE_ALLOC) * 100 / BASELINE_ALLOC ))
fi
ALLOC_REGRESSION_PCT=$(( ALLOC_DELTA_PCT > 0 ? ALLOC_DELTA_PCT : 0 ))

echo "  total_allocs:    baseline=$BASELINE_ALLOC  current=$CURRENT_ALLOC  delta=${ALLOC_DELTA_PCT}%"

# --- Verdict --------------------------------------------------------------
FAIL=0
if [ "$RSS_REGRESSION_PCT" -gt "$RSS_THRESHOLD" ]; then
    echo "::error::RSS regression ${RSS_REGRESSION_PCT}% exceeds ${RSS_THRESHOLD}% threshold."
    FAIL=1
fi
if [ "$ALLOC_REGRESSION_PCT" -gt "$ALLOC_THRESHOLD" ]; then
    echo "::error::Allocation regression ${ALLOC_REGRESSION_PCT}% exceeds ${ALLOC_THRESHOLD}% threshold."
    FAIL=1
fi

if [ "$FAIL" -ne 0 ]; then
    exit 1
fi

echo "memcheck-gate: PASS — no regressions beyond thresholds."
exit 0