# Phase 8: Advanced Testing Features - Context

**Date:** 2026-06-07  
**Milestone:** v2.0 - Testing Framework Foundation  
**Phase Status:** Planning  

## Overview

This phase extends the core testing infrastructure (Phase 7) with advanced features for comprehensive rule validation.

## Key Capabilities

| Feature | Status | Notes |
|---------|--------|-------|
| No-loop/lock-on-active enforcement | Planned | Enhanced ConstraintVerifier |
| Working memory JSON snapshots | Implemented | Add baseline management |
| Rule coverage instrumentation | Partially implemented | Enhance with reporting |
| Snapshot management system | Planned | Baseline comparison |
| HTML report generation | Deferred | Phase 9 |

## Gray Areas

All gray areas auto-selected:

1. **No-loop/lock-on-active tests**
   - Approach: Use existing ConstraintVerifier
   - Enhancement: Add agenda group tracking for lock_on_active

2. **JSON snapshot testing**
   - Approach: Extend WorkingMemorySnapshot with comparison helpers
   - Enhancement: Add baseline directory management

3. **Rule coverage instrumentation**
   - Approach: Enhance RuleCoverage with reporting methods
   - Enhancement: Add CSV export and threshold assertions

4. **Snapshot management system**
   - Approach: Create `tests/snapshots/` structure
   - Enhancement: Git-integrated update mechanism

5. **HTML report generation**
   - Approach: Defer to Phase 9 (external dependencies)
   - Reason: Requires web frontend stack

## Testing Strategy

- Per-test engine isolation (Phase 7)
- Deterministic mode for reproducible results
- Snapshot testing for regression detection
- Coverage metrics for test completeness validation

## Success Criteria

What must be TRUE after this phase:

1. Tests can verify no_loop constraint prevents rule re-firing
2. Working memory state serializes to JSON for snapshot comparison
3. Coverage reports show which rules were evaluated vs fired
4. Snapshot management supports baseline updates with git integration

## Deliverables

- Enhanced ConstraintVerifier in `engine/src/testing/mod.rs`
- WorkingMemorySnapshot utilities for comparison
- RuleCoverage reporting enhancements
- Snapshot baseline management utilities
- Documentation in testing.md or README
