---
phase: 01-core-engine-foundation
plan: 05
type: execute
wave: 3
depends_on:
  - "01-01"
  - "01-02"
  - "01-03"
  - "01-04"
files_modified:
  - src/engine/testing/tester.rs
  - src/engine/testing/scenario_runner.rs
  - src/engine/metrics/collector.rs
  - tests/integration/rule_coverage.rs
  - tests/unit/kie_cache.rs
autonomous: true
requirements:
  - CORE-07
user_setup: []
must_haves:
  truths:
    - "Loop detection (no-loop, lock-on-active) works correctly"
    - "Rule coverage reporting shows which rules fired during execution"
    - "KIE base caching reduces compilation overhead"
  artifacts:
    - path: "src/engine/testing/tester.rs"
      provides: "Unit test framework for individual rules"
      exports: ["RuleTester", "expect_fired", "expect_not_fired"]
    - path: "src/engine/testing/scenario_runner.rs"
      provides: "Scenario-based simulation engine for what-if analysis"
      exports: ["Scenario", "run_scenario", "compare_results"]
    - path: "src/engine/metrics/collector.rs"
      provides: "Rule execution metrics collection"
      exports: ["MetricsCollector", "get_throughput", "get_latency"]
    - path: "tests/integration/rule_coverage.rs"
      provides: "Coverage reporting integration tests"
      tests: ["coverage_shows_fired_rules", "dead_rule_detection_works"]
    - path: "tests/unit/kie_cache.rs"
      provides: "KIE cache performance tests"
      tests: ["cache_hit_reduces_compile_time", "eviction_removes_stale_entries"]
  key_links:
    - from: "src/engine/testing/tester.rs"
      to: "src/engine/execution/results.rs"
      via: "Test assertions on execution results"
      pattern: "assert!.*fired_rules.contains|expect_not_fired.*RuleId"
    - from: "src/engine/metrics/collector.rs"
      to: "src/engine/events/event_dispatcher.rs"
      via: "MetricsCollection via EventListener"
      pattern: "EventListener.*on_event.*RuleFiredEvent"
    - from: "tests/integration/rule_coverage.rs"
      to: "src/engine/rules.rs"
      via: "Coverage analysis per rule"
      pattern: "analyze_rule_coverage.*RuleSet|dead_rules_in"
---

## Phase 1 Plan 05: Testing Framework & Coverage Reporting

<objective>
Implement testing framework for individual rules, scenario-based simulation for what-if analysis, and coverage reporting to show which rules fired. This plan also validates the KIE cache performance and loop detection mechanisms.

Purpose: Per TEST-01 (Phase 5 requirement), developers need unit test capability for rules. The scenario runner enables what-if analysis. Coverage data is essential for MON-03's dead rule detection.
Output: RuleTester framework, ScenarioRunner simulation engine, coverage reporting with dead rule detection.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/01-core-engine-foundation/01-PLAN.md (Plan 01)
@.planning/phases/01-core-engine-foundation/02-PLAN.md (Plan 02)
@.planning/phases/01-core-engine-foundation/03-PLAN.md (Plan 03)
@.planning/phases/01-core-engine-foundation/04-PLAN.md (Plan 04)
@.planning/phases/01-core-engine-foundation/01-CONTEXT.md

**Dependencies:**
- CORE-07: Loop detection and control (no-loop, lock-on-active) - validate these are working
- TEST-01: Unit testing framework for individual rules
- TEST-02: Scenario-based simulation (what-if analysis)
</context>

<tasks>

<task type="auto">
  <name>Implement RuleTester framework per TEST-01, CORE-07</name>
  <files>src/engine/testing/tester.rs</files>
  <action>Create unit test framework for rule validation:

1. Implement `RuleTester` struct:
   - engine: Engine
   - rule_under_test: RuleId

2. Add assertion methods:
   - expect_fired(fact_set) → assert rule fires with given facts
   - expect_not_fired(fact_set) → assert rule doesn't fire
   - expect_salience_order(rules, expected_order) → verify firing sequence
   - expect_no_loop(rule_with_no_loop, fact_pattern) → verify no-loop works

3. Add test utilities:
   - new_tester() → RuleTester for quick setup
   - with_fact(fact) → add fact to working memory
   - execute(ruleset) → run tests against rule set

4. Example usage:
```rust
let tester = RuleTester::new("high-value-customer");
tester.with_fact(fact!("type" => "premium", "spend" => 1500))
     .expect_fired()
     .execute();
```

This provides the foundation for TEST-01 compliance.</action>
  <verify>RuleTester correctly identifies when rules should/shouldn't fire</verify>
  <done>Test framework compiles with all assertion methods working</done>
</task>

<task type="auto">
  <name>Implement ScenarioRunner for what-if analysis per TEST-02, D-06</name>
  <files>src/engine/testing/scenario_runner.rs</files>
  <action>Create scenario-based simulation engine:

1. Implement `Scenario` struct:
   - name: String
   - initial_facts: Vec<Fact>
   - rules_to_test: Vec<RuleId>
   - expected_outcome: ScenarioOutcome

2. Implement `ScenarioOutcome` enum:
   - FactsAdded(Vec<FactType>)
   - FactsRetracted(Vec<FactReference>)
   - SpecificRuleFired(String)

3. Add `run_scenario(scenario: &Scenario) -> SimulationResult`:
   - Execute scenario rules with initial facts
   - Compare actual vs expected outcome
   - Generate detailed difference report

4. Implement diff reporting:
   - Expected vs actual facts added/removed
   - Missing rule firings
   - Unexpected rule firings

5. Add test mode:
   - Dry run (no side effects)
   - Step-by-step execution for debugging

This enables what-if analysis per TEST-02 and interactive debugging per D-06.</action>
  <verify>ScenarioRunner executes scenarios; differences reported accurately</verify>
  <done>Scenario runner compiles with comparison reporting</done>
</task>

<task type="auto">
  <name>Implement rule coverage reporting per TEST-03, MON-02</name>
  <files>src/engine/coverage/reporter.rs, src/engine/coverage/analyzer.rs</files>
  <action>Create system to track and report which rules fire during execution:

1. Implement `CoverageTracker`:
   - rules_evaluated: HashSet<RuleId>
   - rules_fired: HashSet<RuleId>
   - fact_matches_per_rule: HashMap<RuleId, usize>

2. Add analysis methods:
   - analyze_coverage() → CoverageReport
   - identify_dead_rules(threshold: f64) → Vec<DeadRule>
   - calculate_hit_rates() → HashMap<RuleId, f64>

3. Implement `CoverageReport`:
   - total_rules: usize
   - fired_rules: Vec<FiredRuleInfo>
   - never_fired: Vec<NeverFiredRule>
   - hit_rate_histogram: Vec<HistogramBucket>

4. Integration with execution:
   - Update tracker via EventListener (RuleFiredEvent)
   - Snapshot coverage at any time

5. Export methods:
   - get_coverage_for_session(session_id) → CoverageReport
   - reset_coverage()

This enables TEST-03 and MON-02 compliance.</action>
  <verify>CoverageTracker records rule firings; analyzer identifies dead rules</verify>
  <done>Coverage system compiles with reporting and analysis capabilities</done>
</task>

<task type="auto">
  <name>Implement KIE cache performance validation per CORE-08, MON-01</name>
  <files>src/engine/kie_cache/performance.rs</files>
  <action>Create cache performance measurement and optimization:

1. Implement `CacheMetrics`:
   - hit_count: usize
   - miss_count: usize
   - compile_time_saved_ms: Duration
   - avg_hit_latency_ms: f64

2. Add benchmarking methods:
   - warmup_cache(rule_set) → measure initial compilation
   - measure_reuse(rule_set) → time cached retrieval
   - calculate_savings() → compile time reduction percentage

3. Implement eviction policy testing:
   - LRU behavior verified
   - TTL expiration works correctly
   - Memory pressure handling

4. Export methods:
   - get_cache_stats() → CacheMetrics
   - clear_cache()
   - profile_compile_vs_cache()

This validates CORE-08's caching implementation and provides MON-01 data.</action>
  <verify>Cache metrics track hits/misses; savings calculated correctly</verify>
  <done>KIE cache performance module compiles with all measurement methods</done>
</task>

<task type="auto">
  <name>Create loop detection validation tests per CORE-07, CORE-05</name>
  <files>tests/integration/loop_detection.rs</files>
  <action>Write comprehensive tests for loop control mechanisms:

1. Tests for `no_loop`:
   - no_loop_prevents_re_firing: Same rule doesn't fire twice in cycle
   - no_loop_allows_next_cycle: Rule fires again after cycle boundary
   - no_loop_with_fact_update: Fact update triggers new activation

2. Tests for `lock_on_active`:
   - lock_prevents_group_rules: All rules in group blocked when one fires
   - lock_releases_after_cycle: Group unblocked at cycle end
   - lock_doesnt_affect_other_groups: Other groups continue normally

3. Test max iteration limit:
   - infinite_loop_detected: Max iterations stops runaway execution
   - error_message_includes_iterations: Debug info returned

4. Edge cases:
   - Self-referencing rules with no_loop
   - Circular dependencies between rules in same group

These tests ensure CORE-07 loop detection works correctly.</action>
  <verify>All loop detection tests pass; edge cases handled</verify>
  <done>Loop detection test suite compiles and validates all mechanisms</done>
</task>

<task type="auto">
  <name>Write coverage integration tests per TEST-03, MON-02</name>
  <files>tests/integration/rule_coverage.rs</files>
  <action>Create integration tests for coverage reporting:

1. Coverage accuracy tests:
   - coverage_shows_fired_rules: All fired rules recorded
   - coverage_ignores_compilation_errors: Invalid rules not counted

2. Dead rule detection tests:
   - dead_rule_detection_works: Rules never executed identified
   - threshold_configurable: User sets sensitivity (e.g., 10% usage = dead)
   - false_positives_minimal: Active rules not flagged as dead

3. Performance tests:
   - coverage_overhead_acceptable: Tracking doesn't significantly slow execution
   - concurrent_coverage_safe: Thread-safe for multi-session testing

4. Integration with API:
   - GET /coverage returns report
   - POST /coverage/reset clears tracker

These tests verify coverage system accuracy and performance.</action>
  <verify>Integration tests pass; dead rule detection works correctly</verify>
  <done>Coverage integration test suite compiles and validates reporting</done>
</task>

<task type="auto">
  <name>Add metrics collection for throughput and latency per MON-01, MON-02</name>
  <files>src/engine/metrics/collector.rs</files>
  <action>Create comprehensive metrics collection system:

1. Implement `MetricsCollector` as EventListener:
   - Track rule firing rate (rules/second)
   - Record execution latency (ms per execution)
   - Count facts processed per execution

2. Add histogram-based latency tracking:
   - Buckets: <10ms, <50ms, <100ms, <500ms, >500ms
   - P50, P95, P99 percentiles

3. Export query methods:
   - get_throughput() → f64 (rules/second)
   - get_latency_p50() → f64 (ms)
   - get_latency_p95() → f64 (ms)
   - get_rule_hit_counts() → HashMap<String, u64>

4. Integration with metrics endpoint:
   - GET /metrics returns JSON
   - Format compatible with Prometheus (optional)

This provides MON-01/MON-02 compliance data.</action>
  <verify>MetricsCollector aggregates data correctly; percentiles calculated</verify>
  <done>Metric collection compiles and updates during execution</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Test API → RuleTester | User-provided test assertions |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-01-05-01 | D | Test rules causing infinite loops | mitigate | Test harness has max iteration limit; test timeout enforced |
| T-01-05-02 | E | Coverage tracker memory leak | accept | Coverage tracked per-session with explicit reset; LRU eviction configurable |
| T-01-05-03 | I | Metrics manipulation | accept | Metrics are informational only; no security-critical use |
| T-01-05-04 | D | Scenario runner side effects | mitigate | Dry-run mode available; scenarios run in isolation |
| T-01-05-05 | S | Coverage data exposure | accept | No sensitive data in coverage reports (fact IDs only) |

**Note:** Testing framework runs with same privileges as engine. Users must ensure test code is from trusted sources.
</threat_model>

<verification>
- All testing tests pass: `cargo test --test loop_detection --test rule_coverage`
- Coverage accuracy verified with controlled execution traces
- KIE cache performance shows >90% compilation time reduction on reuse

## Success Criteria

1. **Loop detection control** — no-loop and lock-on-active prevent infinite loops per CORE-07
2. **Rule coverage reporting** — CoverageTracker records which rules fire, dead rule detection works
3. **Unit test framework** — RuleTester enables isolated rule testing per TEST-01

---

## Implementation Notes for Phase 1 Plan 05

### Test Framework Usage (Example)
```rust
#[test]
fn high_value_customer_discount() {
    let mut tester = RuleTester::new("high-value-customer");
    
    // Setup: Premium customer with high spend
    tester.with_fact(fact!("type" => "premium", "spend" => 1500))
         .with_rule_set(&my_ruleset)
         .expect_fired()
         .execute();
}

#[test]
fn low_value_customer_no_discount() {
    let mut tester = RuleTester::new("high-value-customer");
    
    // Setup: Standard customer with low spend
    tester.with_fact(fact!("type" => "standard", "spend" => 100))
         .expect_not_fired()
         .execute();
}
```

### Scenario Runner Usage (Example)
```rust
let scenario = Scenario {
    name: "Premium Customer Processing",
    initial_facts: vec![fact!("type" => "premium", "spend" => 2000)],
    rules_to_test: vec!["high-value-customer", "loyalty-bonus"],
    expected_outcome: FactsAdded(vec!["discount_applied".into()]),
};

let result = run_scenario(&scenario);
assert!(result.success());
```

### Open Questions for Implementation
1. **Coverage aggregation** — How to combine coverage across multiple sessions? Need distributed tracking?
2. **Test isolation** — Should each test get fresh Engine/WorkingMemory? Confirm current design.
3. **Scenario versioning** — Do scenarios need version control like rules?
