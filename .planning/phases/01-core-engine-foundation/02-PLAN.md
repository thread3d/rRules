---
phase: 01-core-engine-foundation
plan: 02
type: execute
wave: 2
depends_on:
  - "01-01"
files_modified:
  - src/engine/chaining/mode_selector.rs
  - src/engine/salience/agenda.rs
  - src/engine/salience/conflict_resolution.rs
  - tests/integration/forward_chaining.rs
  - tests/integration/backward_chaining.rs
autonomous: true
requirements:
  - CORE-03
  - CORE-04
  - CORE-05
user_setup: []
must_haves:
  truths:
    - "Users can switch between stateful (session-based) and stateless rule execution modes"
    - "Rules fire in correct order based on salience, priority, and conflict resolution strategies"
    - "Session lifecycle is explicitly managed via API calls"
  artifacts:
    - path: "src/engine/chaining/mode_selector.rs"
      provides: "Automatic chaining mode detection per D-04"
      exports: ["ChainingSelector", "detect_chaining_mode"]
    - path: "src/engine/salience/agenda.rs"
      provides: "Rule agenda with priority queue for firing order"
      exports: ["Agenda", "Activation"]
    - path: "src/engine/salience/conflict_resolution.rs"
      provides: "Conflict resolution strategies (salience, depth, recency)"
      exports: ["ConflictResolver", "Strategy"]
    - path: "tests/integration/forward_channing.rs"
      provides: "Integration test for forward chaining execution modes"
      tests: ["stateful_session_keeps_memory", "stateless_execution_isolated"]
    - path: "tests/integration/backward_chaining.rs"
      provides: "Integration test for backward chaining with debugging traces"
      tests: ["backward_trace_shows_reasoning_path", "debug_mode_enables_trace"]
  key_links:
    - from: "src/engine/chaining/mode_selector.rs"
      to: "src/engine/rules.rs"
      via: "Condition pattern analysis"
      pattern: "analyze_condition.*PatternMatch|is_query_driven"
    - from: "src/engine/salience/agenda.rs"
      to: "src/engine/fact.rs"
      via: "Activation with fact matches"
      pattern: "Activation::new.*Rule.*\[&Fact\]"
    - from: "src/engine/salience/conflict_resolution.rs"
      to: "src/engine/salience/agenda.rs"
      via: "Agenda sorting strategy"
      pattern: "sort_activations.*Strategy"
---

## Phase 1 Plan 02: Chaining Selection & Agenda Control

<objective>
Implement automatic chaining mode selection and agenda control infrastructure. This plan builds on Plan 01's data structures to enable dynamic inference mode detection, salience-based rule ordering, and conflict resolution strategies.

Purpose: Per D-04, the engine must automatically detect whether forward or backward chaining is optimal based on rule structure—not manual API choice. The agenda system ensures rules fire in correct order per CORE-04/CORE-05.
Output: Automatic chaining selector, salience-based agenda with priority queue, and conflict resolution strategy implementations.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/01-core-engine-foundation/01-PLAN.md (Plan 01)
@.planning/phases/01-core-engine-foundation/01-CONTEXT.md

**Dependencies:**
- D-04: Automatic selection — Engine detects best inference mode based on rule structure and query type
- D-05: Fully mixed execution — Forward/backward chaining can be mixed dynamically in single execution
- CORE-04: Rule salience/prioritization and agenda control
- CORE-05: Conflict resolution strategies for rule firing order
- CORE-07: Loop detection (no-loop, lock-on-active)
</context>

<tasks>

<task type="auto">
  <name>Implement automatic chaining mode selector per D-04, D-06</name>
  <files>src/engine/chaining/mode_selector.rs</files>
  <action>Create `ChainingSelector` that automatically determines optimal inference mode:

1. Implement `analyze_rule_structure(rule: &Rule) -> InferencePattern`:
   - Parse rule condition AST for pattern matches vs query patterns
   - Classify as: "data_driven" (matches existing facts), "goal_driven" (needs to prove goal)
   - Handle mixed rules that could use both modes

2. Implement `ChainingSelector::select(rules: &[Rule], target_fact: Option<&Fact>) -> ChainingMode`:
   - If all rules are data-driven → Return ForwardOnly
   - If target provided and no matching facts exist → Return BackwardOnly
   - If mixed or complex query → Return Automatic (mixed mode)
   - Debug context detected (interactive use case per D-06) → Enable backward tracing

3. Add `InferenceTraceCollector` for backward chaining debug output:
   - Record rule activations with justification
   - Track which facts triggered each activation
   - Format for interactive debugger display

4. Integrate with EngineBuilder to apply selected mode.

This implements the automatic selection requirement — no manual API choice needed per D-04.</action>
  <verify>ChainingSelector::select returns correct mode based on rule analysis; debug context enables trace collection</verify>
  <done>Automatic chaining detection implemented and tested with sample rulesets</done>
</task>

<task type="auto">
  <name>Implement salience-based agenda with priority queue per CORE-04, CORE-05</name>
  <files>src/engine/salience/agenda.rs, src/engine/salience/activation.rs</files>
  <action>Create Agenda management for rule execution ordering:

1. Implement `Activation` struct:
   - rule: RuleId
   - matched_facts: Vec<FactReference>
   - salience: i32 (per CORE-04)
   - timestamp: SystemTime (for tie-breaking per CORE-05)

2. Implement `Agenda` with operations:
   - push(Activation) — insert based on priority queue
   - pop() → Option<Activation> — get highest priority activation
   - peek() → Option<&Activation> — view without removal
   - remove_activation(rule_id) — for no-loop enforcement

3. Implement `PriorityQueue` using binary heap with custom comparator:
   - Primary: salience descending (higher fires first)
   - Secondary: timestamp ascending (earlier wins tie)
   - Tertiary: rule name alphabetically for determinism

4. Add `Agenda::apply_strategy(&mut self, strategy: ConflictStrategy)` to re-sort by alternative criteria.

This ensures CORE-04 compliance: rules fire in correct order based on salience and priority.</action>
  <verify>Agenda returns activations in salience order; tie-breaking uses timestamp correctly</verify>
  <done>Priority queue implementation with configurable conflict resolution strategies</done>
</task>

<task type="auto">
  <name>Implement conflict resolution strategies per CORE-05</name>
  <files>src/engine/salience/conflict_resolution.rs</files>
  <action>Create ConflictResolver for multiple ordering strategies:

1. Implement `ConflictStrategy` enum:
   - Salience (default: higher salience first)
   - DepthFirst (fewer matched facts优先)
   - BreadthFirst (more matched facts first)
   - Recency (most recent activation wins)
   - Lexicographic (rule name alphabetical)

2. Implement `ConflictResolver::sort(activations: &[Activation], strategy: ConflictStrategy) -> Vec<&Activation>`:
   - Each strategy implemented with clear ordering logic
   - Stable sort for deterministic behavior

3. Add configuration via EngineBuilder:
   - default_conflict_strategy: ConflictStrategy
   - per-rule override in Rule definition

4. Integrate with Agenda to apply strategy on push/pop operations.

This satisfies CORE-05: "Conflict resolution strategies for rule firing order."</action>
  <verify>Each conflict strategy produces correct ordering; EngineBuilder accepts strategy configuration</verify>
  <done>All four conflict resolution strategies implemented and tested</done>
</task>

<task type="auto">
  <name>Implement stateful/session-based execution per D-07, D-08, D-09</name>
  <files>src/engine/sessions.rs, src/engine/stateful_executor.rs</files>
  <action>Create explicit session lifecycle management for long-lived sessions:

1. Implement `Session` struct:
   - id: String (explicit ID per D-08)
   - stateful: bool
   - created_at: SystemTime
   - last_active: RwLock<SystemTime>
   - working_memory_snapshot: Option<WorkingMemory> (for session persistence)

2. Implement `SessionManager` with operations:
   - create_session(stateful, id) → SessionId
   - get_session(id) → Option<&mut Session>
   - destroy_session(id) → Result<(), SessionError>
   - list_sessions() → Vec<SessionMetadata>

3. Add `StatefulExecutor::run_in_session(engine: &Engine, session_id: String, rules: &[Rule]) -> ExecutionResult`:
   - Load working memory from persistent storage
   - Execute rules against shared state
   - Commit changes back to storage

4. Implement cleanup policies:
   - Session timeout (configurable per D-09)
   - Idle session eviction
   - Graceful shutdown handling

This provides explicit lifecycle management per D-08 and supports long-lived sessions per D-09.</action>
  <verify>Session creation with explicit ID; stateful execution preserves working memory across runs</verify>
  <done>SessionManager handles create/read/destroy; StatefulExecutor maintains working memory persistence</done>
</task>

<task type="auto">
  <name>Implement stateless execution mode per D-07, CORE-03</name>
  <files>src/engine/stateless_executor.rs</files>
  <action>Create stateless rule execution without session overhead:

1. Implement `StatelessExecutor::run(engine: &Engine, rules: &[Rule], facts: Vec<Facts>) -> ExecutionResult`:
   - Create ephemeral working memory
   - No fact persistence between calls
   - Each execution starts fresh

2. Add request-scoped configuration:
   - Session timeout immediately after completion
   - No session ID required (per D-07 explicit choice)
   - Return results without side effects

3. Implement batch execution mode for high-throughput:
   - Process multiple fact sets sequentially
   - Share compiled rule set (KIE base per CORE-08)
   - Reset working memory between batches

4. Compare with stateful approach in documentation.

This ensures D-07 compliance: "No single default — users must explicitly choose stateful vs stateless."</action>
  <verify>Stateless execution produces same results as isolated session; no persistent state left</verify>
  <done>StatelessExecutor compiles and executes rules without side effects or persistence</done>
</task>

<task type="auto">
  <name>Add no-loop and lock-on-active enforcement per CORE-07</name>
  <files>src/engine/loop_detection.rs, src/engine/firing_tracker.rs</files>
  <action>Implement loop detection mechanisms per D-10/CORE-07:

1. Implement `FiringTracker` with:
   - fired_rules_per_cycle: HashMap<RuleId, usize>
   - cycle_number: usize
   - record_activation(rule_id)

2. Add `no_loop` enforcement (per CORE-07):
   - Track rule activation count per cycle
   - Skip re-firing if already activated this cycle

3. Add `lock_on_active` enforcement:
   - Lock all rules in same agenda group when one fires
   - Release lock on cycle boundary

4. Implement max iteration limit (configurable via EngineBuilder):
   - Prevent infinite loops from circular rules
   - Return error with loop details

5. Integration: Check loop controls before adding activation to Agenda.

This satisfies CORE-07: "Loop detection and control (no-loop, lock-on-active)."</action>
  <verify>Rules with no_loop=true don't fire twice in same cycle; lock_on_active prevents related rules from firing</verify>
  <done>Loop detection module compiles; no_loop and lock_on_active enforced during agenda operations</done>
</task>

<task type="auto">
  <name>Create integration tests for chaining and session modes</name>
  <files>tests/integration/forward_chaining.rs, tests/integration/backward_chaining.rs</files>
  <action>Write comprehensive integration tests:

1. Forward chaining tests (`forward_chaining.rs`):
   - stateful_session_keeps_memory: Facts persist across multiple execute() calls
   - stateless_execution_isolated: Each call starts fresh with no shared state
   - salience_order_respected: Higher salience rules fire before lower ones
   - conflict_resolution_applied: Default strategy produces expected order

2. Backward chaining tests (`backward_chaining.rs`):
   - backward_trace_shows_reasoning_path: InferenceTrace includes all matching rules and facts
   - debug_mode_enables_trace: enable_debug=true produces trace data
   - query_from_nonexistent_fact_fails: Proper error when goal cannot be proven

3. Add test fixtures:
   - Sample rule sets for forward/backward scenarios
   - Fact templates for testing working memory operations
   - Assertion macros for common test patterns

These tests verify the implementation matches D-04, CORE-04, and CORE-05 requirements.</action>
  <verify>All integration tests pass; coverage includes edge cases</verify>
  <done>Integration test suite compiles and passes with all expected behaviors verified</done>
</task>

<task type="auto">
  <name>Add mixed chaining execution support per D-05</name>
  <files>src/engine/mixed_executor.rs, src/engine/chaining/hybrid_mode.rs</files>
  <action>Implement fully mixed forward/backward chaining per D-05:

1. Implement `HybridMode` executor:
   - Start with user-provided query/fact
   - Try backward chaining first if goal specified
   - Fall back to forward chaining for remaining rules
   - Iterate until fixed point (no more activations)

2. Add `ChainingContext` tracking current mode:
   - InBackwardChain: Currently proving a goal
   - InForwardChain: Currently propagating facts
   - Mixed: Both modes active in same cycle

3. Implement cross-mode fact propagation:
   - Backward chain produces new facts → feed to forward chain
   - Forward chain matches conditions → may enable backward goals
   - Shared working memory with transaction semantics

4. Add trace output showing mixed mode transitions.

This implements D-05: "Fully mixed execution — users can mix forward and backward chaining in a single execution."</action>
  <verify>Mixed execution runs both modes; facts flow between chaining strategies</verify>
  <done>MixedExecutor compiles and handles hybrid chaining with shared working memory</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Engine API → Agenda operations | User rules determine activation priorities |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-01-02-01 | D | Agenda priority manipulation via negative salience | accept | Negative values allowed; engine enforces bounds check only on overflow |
| T-01-02-02 | I | Conflict resolution bypass | mitigate | All sort operations use ConflictResolver; direct heap access blocked |
| T-01-02-03 | E | Session exhaustion (memory) | mitigate | SessionManager limits active sessions; idle eviction configurable |
| T-01-02-04 | S | Session ID collision | accept | UUID-based IDs with negligible collision risk; explicit user IDs allowed |
| T-01-02-05 | D | Infinite loop via recursive rules | mitigate | Max iteration limit (EngineBuilder setting); no-loop enforcement per CORE-07 |

**Note:** Session management uses Rust's ownership system to prevent use-after-free. The RwLock provides thread-safe access with deadlock detection via timeout.
</threat_model>

<verification>
- All integration tests pass: `cargo test --test forward_chaining --test backward_chaining`
- Salience ordering verified with rules of varying salience values
- Session isolation confirmed via memory snapshot comparison
- Loop detection triggers max iteration error

## Success Criteria

1. **Automatic chaining mode selection** — ChainingSelector analyzes rule structure and chooses optimal mode
2. **Salience-based firing order** — Agenda returns activations sorted by priority per CORE-04
3. **Multiple conflict resolution strategies** — ConflictResolver implements salience, depth, breadth, recency strategies per CORE-05
4. **Explicit session lifecycle** — SessionManager provides create/get/destroy API per D-08/D-09

---

## Implementation Notes for Phase 1 Plan 02

### Chaining Mode Selection Flow
```
Input: Rules + Query/Fact
    ↓
analyze_rule_structure() → Classify each rule
    ↓
Check query existence and facts
    ↓
Return optimal mode:
- ForwardOnly: All data-driven rules, no query
- BackwardOnly: Query provided with no matching facts
- Automatic: Mixed rules or complex conditions
```

### Agenda Operations (Pseudo-code)
```rust
// Push with priority queue ordering
agenda.push(Activation { salience: 10 }) // Insert at proper position
agenda.push(Activation { salience: 5 })

// Pop returns highest priority
let next = agenda.pop(); // Returns salience=10 activation

// Priority comparator: (-salience, timestamp, rule_name)
```

### Open Questions for Implementation
1. **Exact hybrid mode transition** — When does execution switch from backward to forward? Need clear rules.
2. **Agenda group semantics** — How are rules grouped for lock_on_active? By name prefix or explicit annotation?
3. **Mixed mode tracing** — Should InferenceTrace show both chaining strategies when mixed?
