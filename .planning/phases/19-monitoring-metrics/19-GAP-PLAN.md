---
phase: 19-monitoring-metrics
plan: 05
type: execute
wave: 1
depends_on: []
files_modified:
  - api/src/metrics.rs
autonomous: true
requirements:
  - MON-03
  - MON-04
user_setup: []
must_haves:
  truths:
    - "/metrics/dead-rules endpoint returns actual dead rules from RuleExecutionTracker"
    - "/metrics/coverage endpoint returns actual coverage data from CoverageTracking"
    - CoverageTracking records rule evaluations during engine execution (record_eval)"
    - CoverageTracking records rule fires during engine execution (record_fire)"
  artifacts:
    - path: "api/src/metrics.rs"
      provides: "Fixed dead-rules and coverage endpoints"
      must_contain:
        - "dead_rules_endpoint calls RuleExecutionTracker.find_dead_rules()"
        - "coverage_endpoint returns CoverageTracking data via to_json()"
    - path: "engine/src/engine/leaps/mod.rs"
      provides: "Integrates CoverageTracking into LEAPS engine execution"
    - path: "engine/src/engine/phreak/mod.rs"
      provides: "Integrates CoverageTracking into PHREAK engine execution"
    - path: "engine/src/engine/rete/mod.rs"
      provides: "Integrates CoverageTracking into Rete engine execution"
    - path: "engine/src/engine/treat/mod.rs"
      provides: "Integrates CoverageTracking into TREAT engine execution"
  key_links:
    - from: "api/src/metrics.rs::dead_rules_endpoint()"
      to: "engine/src/monitoring/detection.rs::RuleExecutionTracker.find_dead_rules()"
      via: "Engine state retrieval and tracker lookup"
      pattern: "tracker\\.find_dead_rules\\(\\)"
    - from: "api/src/metrics.rs::coverage_endpoint()"
      to: "engine/src/monitoring/coverage.rs::CoverageTracking.to_json()"
      via: "Coverage tracking data serialization"
      pattern: "CoverageTracking\\.to_json\\(\\)"
---

## Phase 19 Gap-Closure Plan

**Gap Source:** VERIFICATION.md - MON-03 and MON-04 partially implemented but stubbed

### Objective

Fix the `/metrics/dead-rules` and `/metrics/coverage` API endpoints to return actual data from the engine instead of hardcoded empty arrays.

**Purpose:** These endpoints are critical for monitoring rule effectiveness. Returning empty data makes them non-functional despite all underlying infrastructure existing.

**Output:**
1. `api/src/metrics.rs` - Fixed endpoints with real data
2. Engine implementations (Rete, TREAT, LEAPS, PHREAK) - Integrated CoverageTracking calls

---

### Context

@.planning/phases/19-monitoring-metrics/VERIFICATION.md
@engine/src/monitoring/detection.rs
@engine/src/monitoring/coverage.rs
@api/src/metrics.rs (lines 197-206)

**Current State:**
- `dead_rules_endpoint()` returns empty array
- `coverage_endpoint()` returns empty array
- `RuleExecutionTracker.find_dead_rules()` exists and works
- `CoverageTracking` singleton fully implemented with `record_rule_eval()`, `record_rule_fire()`

**Missing Integration:**
1. API endpoints don't call the engine's tracker/coverage system
2. Engine implementations don't record coverage during rule execution

---

### Tasks

<task type="auto">
  <name>Task 1: Fix dead-rules endpoint to return actual data</name>
  <files>api/src/metrics.rs</files>
  <action>
Update the `dead_rules_endpoint()` function (lines 196-200) to:
1. Create or retrieve a RuleExecutionTracker instance
2. Call find_dead_rules() on the tracker
3. Map DeadRuleInfo to Vec<String> for JSON response

The endpoint should return actual dead rule IDs, not an empty array.
Use the RuleExecutionTracker type from engine::monitoring::detection.
</action>
  <verify>
cargo build --bin rRules-server && cargo test --lib dead_rules_endpoint
  </verify>
  <done>
dead_rules_endpoint returns populated array when dead rules exist, empty only when no dead rules
</done>
</task>

<task type="auto">
  <name>Task 2: Fix coverage endpoint to return actual data</name>
  <files>api/src/metrics.rs</files>
  <action>
Update the `coverage_endpoint()` function (lines 203-206) to:
1. Call CoverageTracking::to_json() to get serialized coverage data
2. Return that JSON directly as HttpResponse::Ok().json()

The endpoint should return actual coverage report data from the singleton.
Use the CoverageTracking type from engine::monitoring::coverage.
</action>
  <verify>
cargo build --bin rRules-server && cargo test --lib coverage_endpoint
  </verify>
  <done>
coverage_endpoint returns populated JSON with rules array and summary when data exists, valid structure otherwise
</done>
</task>

<task type="auto">
  <name>Task 3: Integrate CoverageTracking into all engine implementations</name>
  <files>engine/src/engine/rete/mod.rs, engine/src/engine/treat/mod.rs, engine/src/engine/leaps/mod.rs, engine/src/engine/phreak/mod.rs</files>
  <action>
For each engine implementation file, modify the execute() and run() methods to:
1. Import CoverageTracking from engine::monitoring::coverage
2. After rule evaluation but before execution, call CoverageTracking::record_rule_eval(rule_id)
3. When a rule fires (actions execute), call CoverageTracking::record_rule_fire(rule_id)

This ensures every rule execution is tracked for coverage reporting.
</action>
  <verify>
cargo test --lib engine::{rete,treat,leaps,phreak} -- coverage
  </verify>
  <done>
CoverageTracking records all rule evaluations and fires during engine execution
</done>
</task>

---

### Threat Model

## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| /metrics endpoints → Engine state | Untrusted metrics collection access (read-only) |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-19-GAP-01 | Information Disclosure | Coverage reports could reveal rule logic patterns | Accept | No PII or sensitive data; rules are configuration, not secrets |

---

### Verification

```bash
# Build verification
cargo check --bin rRules-server

# Run tests for dead rule detection
cargo test --lib monitoring::detection

# Run tests for coverage tracking
cargo test --lib monitoring::coverage
```

---

### Success Criteria

- [ ] `/metrics/dead-rules` returns actual data when engine has rules that were never fired
- [ ] `/metrics/coverage` returns actual JSON with rules array and summary from CoverageTracking
- [ ] All engine implementations (Rete, TREAT, LEAPS, PHREAK) record coverage during execution
- [ ] No regression in existing functionality

---

<output>
Create `.planning/phases/19-monitoring-metrics/19-GAP-SUMMARY.md` when done
</output>
