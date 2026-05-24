---
phase: 01-core-engine-foundation
plan: 03
type: execute
wave: 2
depends_on:
  - "01-01"
  - "01-02"
files_modified:
  - src/engine/execution/executor.rs
  - src/engine/execution/results.rs
  - src/engine/events/event_dispatcher.rs
  - tests/unit/executors.rs
autonomous: true
requirements:
  - CORE-06
user_setup: []
must_haves:
  truths:
    - "Event-driven rule triggering works correctly"
    - "Execution results include fired rules and facts added/removed"
    - "Session state persists across multiple executions"
  artifacts:
    - path: "src/engine/execution/executor.rs"
      provides: "Main execution coordinator for forward/backward/mixed modes"
      exports: ["RuleExecutor", "execute_rules"]
    - path: "src/engine/execution/results.rs"
      provides: "Execution result structures with statistics and traces"
      exports: ["ExecutionResult", "ExecutionStats", "EventLog"]
    - path: "src/engine/events/event_dispatcher.rs"
      provides: "Event dispatch system for rule firing notifications"
      exports: ["EventDispatcher", "EventListener", "RuleFiredEvent"]
  key_links:
    - from: "src/engine/execution/executor.rs"
      to: "src/engine/salience/agenda.rs"
      via: "Agenda consumption for rule activation"
      pattern: "agenda.pop.*Activation|execute_activation"
    - from: "src/engine/events/event_dispatcher.rs"
      to: "src/engine/execution/results.rs"
      via: "EventLog population"
      pattern: "dispatch_event.*RuleFiredEvent"
    - from: "src/engine/execution/results.rs"
      to: "tests/integration/forward_chaining.rs"
      via: "Test assertions on execution results"
      pattern: "assert_eq!.*fired_rules|assert!.*fact_added"
---

## Phase 1 Plan 03: Execution Coordinator & Event System

<objective>
Implement the main execution coordinator that orchestrates forward/backward/mixed chaining and collects execution results. This plan adds event-driven triggering capabilities and result reporting for debugging and monitoring.

Purpose: Per D-05, users need to execute mixed chaining modes seamlessly. The executor integrates all previous components into a unified API while collecting detailed results per CORE-06.
Output: Unified RuleExecutor with event dispatching, comprehensive execution results with statistics and traces.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/01-core-engine-foundation/01-PLAN.md (Plan 01)
@.planning/phases/01-core-engine-foundation/02-PLAN.md (Plan 02)
@.planning/phases/01-core-engine-foundation/01-CONTEXT.md

**Dependencies:**
- D-04, D-05: Chaining mode selection and mixed execution
- CORE-06: Event-driven rule triggering capabilities
- CORE-07: Loop detection (already implemented in Plan 02)
</context>

<tasks>

<task type="auto">
  <name>Implement main RuleExecutor with unified API per D-04, D-05, D-07</name>
  <files>src/engine/execution/executor.rs, src/engine/execution/mod.rs</files>
  <action>Create the primary execution coordinator integrating all chaining modes:

1. Implement `RuleExecutor` struct:
   - engine: &Engine
   - session: &Session (for stateful) or WorkingMemory temporary (stateless)

2. Implement `execute_rules(&mut self, rules: &[Rule]) -> ExecutionResult`:
   - Determine chaining mode via ChainingSelector
   - Initialize Agenda with rule activations
   - Loop until no more activations (fixed point)
   - Apply conflict resolution at each cycle

3. Add method variants:
   - execute_forward_only()
   - execute_backward_only()
   - execute_mixed() — hybrid mode from Plan 02

4. Integration points:
   - Use Agenda from salience module
   - Use WorkingMemory from fact module
   - Use LoopDetection for safety checks

5. Export from lib.rs: `pub use execution::RuleExecutor;`

This provides the unified entry point per D-07's explicit mode choice.</action>
  <verify>RuleExecutor.execute_rules runs rules and returns ExecutionResult</verify>
  <done>Unified executor with forward/backward/mixed variants compiles</done>
</task>

<task type="auto">
  <name>Implement execution results and statistics per CORE-06, D-06</name>
  <files>src/engine/execution/results.rs</files>
  <action>Create detailed result structures for debugging and monitoring:

1. Implement `ExecutionResult`:
   - success: bool
   - fired_rules: Vec<FiredRule>
   - facts_added: Vec<FactReference>
   - facts_retracted: Vec<FactReference>
   - error: Option<String>

2. Implement `FiredRule`:
   - rule_id: String
   - salience: i32
   - timestamp: SystemTime
   - matched_facts: Vec<FactReference>
   - actions_executed: Vec<ActionType>

3. Add `ExecutionStats` for metrics:
   - total_rules_evaluated: usize
   - rules_fired: usize
   - activations_created: usize
   - cycles_completed: usize
   - facts_modified: usize
   - execution_time_ms: u64

4. Add `InferenceTrace` (from backward chaining):
   - goal: Option<FactPattern>
   - inference_steps: Vec<InferenceStep>
   - failed_goals: Vec<FailedGoal>

5. Implement Debug + Serialize for all types.

This provides detailed results per D-06's interactive debugging requirement.</action>
  <verify>ExecutionResult includes all required fields; serialization works for API transport</verify>
  <done>All result types compile with Debug and Serialize implementations</done>
</task>

<task type="auto">
  <name>Implement event dispatcher for rule triggering per CORE-06, D-06</name>
  <files>src/engine/events/event_dispatcher.rs, src/engine/events/events.rs</files>
  <action>Create event system for real-time execution feedback:

1. Implement `Event` enum:
   - RuleFired { rule_id: String, salience: i32 }
   - FactAdded { fact_id: String, fact_type: String }
   - FactRetracted { fact_id: String }
   - CycleStarted { cycle_number: usize }
   - CycleEnded { cycle_number: usize }
   - ExecutionCompleted { stats: ExecutionStats }

2. Implement `EventDispatcher` with:
   - subscribers: Vec<Box<dyn EventListener>>
   - subscribe(listener) → SubscriberId
   - unsubscribe(id)
   - dispatch(event)

3. Implement `EventListener` trait:
   - fn on_event(&mut self, event: &Event)

4. Built-in listeners:
   - TraceCollector (for backward chaining debug per D-06)
   - MetricsRecorder (throughput/latency)
   - LoggingListener (console/file output)

5. Integration with RuleExecutor to dispatch events during execution.

This enables CORE-06 compliance for event-driven triggering.</action>
  <verify>EventDispatcher delivers events to subscribers; TraceCollector records inference steps</verify>
  <done>Event system compiles with all event types and listener trait</done>
</task>

<task type="auto">
  <name>Add rule execution hooks and callbacks per D-10, D-12</name>
  <files>src/engine/hooks.rs</files>
  <action>Create extension points for custom behavior during execution:

1. Implement `RuleHook` trait:
   - fn before_execution(&mut self, rules: &[Rule]) -> HookResult
   - fn after_rule_fired(&mut self, fired: &FiredRule) -> HookResult
   - fn on_fact_modified(&mut self, fact: &Fact, change: ChangeType)

2. Add `HookContext` with:
   - engine: EngineRef
   - session_id: Option<String>
   - execution_mode: ChainingMode

3. Implement built-in hooks:
   - AuditLogHook (record all rule firings per GOV-01)
   - ValidationHook (re-validate facts after modification)
   - NotificationHook (send webhooks for important events)

4. Integration via EngineBuilder:
   - add_hook(Box<dyn RuleHook>)

This supports D-12's hot reload requirement by enabling hooks to detect and apply rule changes.</action>
  <verify>Custom hooks execute during rule execution; AuditLogHook records fired rules</verify>
  <done>Hook system compiles with four hook points and three built-in implementations</done>
</task>

<task type="auto">
  <name>Implement working memory snapshots for session persistence per D-09, CORE-03</name>
  <files>src/engine/snapshot.rs, src/engine/persistence.rs</files>
  <action>Create snapshot mechanism for long-lived sessions:

1. Implement `WorkingMemorySnapshot`:
   - facts: Vec<Fact> (full copy or references)
   - timestamp: SystemTime
   - session_id: String

2. Add persistence layer:
   - Storage trait with save/load/delete operations
   - PostgreSQL implementation (per PROJECT.md hybrid storage)
   - In-memory fallback for testing

3. Implement `SessionManager::save_snapshot(session_id)`:
   - Create working memory snapshot
   - Serialize to storage backend
   - Return snapshot ID for later restoration

4. Add automatic snapshots on:
   - Session inactivity timeout (D-09: long-lived sessions)
   - Rule set changes (hot reload per D-12)
   - Manual save request

This enables D-09 compliance for days-to-weeks session lifetime.</action>
  <verify>Snapshots preserve working memory state; restore from snapshot works correctly</verify>
  <done>Snapshot system compiles with in-memory and PostgreSQL implementations</done>
</task>

<task type="auto">
  <name>Create unit tests for execution coordinator per CORE-06, CORE-07</name>
  <files>tests/unit/executors.rs</files>
  <action>Write comprehensive unit tests:

1. Executor tests:
   - forward_chaining_executes_all_applicable_rules
   - backward_chaining_finds_proof_for_query
   - mixed_mode_handles_hybrid_rules
   - loop_detection_stops_infinite_recursion
   - max_iterations_limit_enforced

2. Results tests:
   - execution_result_records_all_fired_rules
   - statistics_include_correct_counts
   - trace_collects_inference_steps

3. Event tests:
   - events_dispatched_in_order
   - subscribers_receive_matching_events
   - loop_event_logs_cycle_info

4. Add property-based testing for:
   - Salience ordering under various strategies
   - Fact propagation through multiple cycles
   - Concurrent session execution (thread safety)

These tests verify the executor integration works correctly.</action>
  <verify>All unit tests pass; coverage includes error cases</verify>
  <done>Executor test suite compiles with all expected scenarios covered</done>
</task>

<task type="auto">
  <name>Add execution metrics collection per MON-01, MON-02 (early implementation)</name>
  <files>src/engine/metrics.rs, src/engine/stats_collector.rs</files>
  <action>Create minimal metrics infrastructure for future dashboards:

1. Implement `ExecutionStatsCollector`:
   - throughput: rules_per_second
   - latency_ms: average execution time
   - rule_hits: count per rule
   - facts_processed: total insert/retract operations

2. Add `MetricsRecorder` listener that aggregates stats:
   - Per-execution summary
   - Rolling window (configurable size)
   - Histogram for latency distribution

3. Export methods:
   - get_current_throughput() → f64
   - get_latency_histogram() → LatencyBuckets
   - reset_metrics()

4. Integration with EventDispatcher to collect stats.

This provides early infrastructure for MON-01/MON-02 compliance in Phase 5.</action>
  <verify>MetricsCollector aggregates data from events; get methods return valid statistics</verify>
  <done>Metric collection compiles and updates during event processing</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Execution API → Rule hooks | User-provided hooks execute in engine context |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-01-03-01 | E | Hook execution causing infinite loop | mitigate | Hook callbacks have timeout; panic handling with graceful recovery |
| T-01-03-02 | S | Snapshot data exposure | accept | Snapshots contain no secrets (per architecture); encryption optional |
| T-01-03-03 | D | Event listener bypass | mitigate | All dispatch go through EventDispatcher; direct event injection blocked |
| T-01-03-04 | I | Execution stats manipulation | accept | Stats are informational only; no security-critical decisions based on them |
| T-01-03-05 | D | Session snapshot tampering | mitigate | Snapshots use content-addressable storage (hash-based keys) |

**Note:** Hook system allows arbitrary code execution (user-defined hooks). This is acceptable because hooks are explicitly configured by the application developer, not untrusted end users.
</threat_model>

<verification>
- All executor tests pass: `cargo test --test executors`
- Event dispatch verified with subscriber count assertions
- Snapshot/restore cycle preserves working memory state

## Success Criteria

1. **Event-driven rule triggering** — Events dispatched for all rule firings and fact modifications per CORE-06
2. **Comprehensive execution results** — ExecutionResult includes fired rules, stats, and traces per D-06
3. **Unified execution API** — Single RuleExecutor handles forward/backward/mixed modes

---

## Implementation Notes for Phase 1 Plan 03

### Execution Flow (Pseudo-code)
```rust
fn execute_rules(&mut self, rules: &[Rule]) -> ExecutionResult {
    let mode = ChainingSelector::select(rules, query);
    
    loop {
        // 1. Create activations from working memory
        let activations = create_activations(rules);
        
        // 2. Sort by salience/conflict strategy
        let sorted = self.agenda.sort(activations);
        
        // 3. Fire each activation
        for activation in sorted {
            dispatch_event(RuleFired { ... });
            apply_actions(activation)?;
            
            if self.loop_detected() {
                break;
            }
        }
        
        // 4. Check fixed point
        if activations.is_empty() {
            break;
        }
    }
    
    ExecutionResult::success(fired_rules, facts_modified)
}
```

### Event Flow (Pseudo-code)
```rust
fn dispatch_event(&mut self, event: Event) {
    for listener in &self.subscribers {
        listener.on_event(&event);
    }
}

// Hook integration
for hook in &self.hooks {
    hook.before_execution(rules);
}
```

### Open Questions for Implementation
1. **Hook panic handling** — Should failed hooks stop execution or be logged and skipped?
2. **Event ordering guarantees** — Are events guaranteed delivered in order? Need to specify.
3. **Metrics retention policy** — How many past executions to keep for rolling averages?
