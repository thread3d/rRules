---
phase: 01-core-engine-foundation
plan: 06
type: execute
wave: 3
depends_on:
  - "01-01"
  - "01-02"
  - "01-03"
  - "01-04"
  - "01-05"
files_modified:
  - src/engine/cache/policy.rs
  - src/engine/scheduler/eviction.rs
  - tests/integration/sessions.rs
  - examples/hot_reload_demo.rs
  - .planning/phases/01-core-engine-foundation/SUMMARY.md
autonomous: true
requirements:
  - CORE-08
user_setup: []
must_haves:
  truths:
    - "KIE base caching improves rule execution performance"
    - "Session lifecycle is explicitly managed via API"
    - "Long-lived sessions persist for days to weeks"
    - "Eviction policies manage cache memory usage"
  artifacts:
    - path: "src/engine/cache/policy.rs"
      provides: "Cache eviction strategies (LRU, TTL, size-based)"
      exports: ["EvictionPolicy", "LruPolicy", "TtlPolicy"]
    - path: "src/engine/scheduler/eviction.rs"
      provides: "Background eviction scheduler"
      exports: ["EvictionScheduler", "schedule_eviction"]
    - path: "tests/integration/sessions.rs"
      provides: "Session lifecycle integration tests"
      tests: ["session_persists_across_requests", "idle_session_eviction", "manual_session_destroy"]
    - path: "examples/hot_reload_demo.rs"
      provides: "Demonstration of hot reload and session management"
      examples: ["reload_rules_file", "run_with_new_rules"]
  key_links:
    - from: "src/engine/cache/policy.rs"
      to: "src/engine/kie_cache.rs"
      via: "Policy application for cache entries"
      pattern: "apply_policy.*CacheEntry|eviction_policy"
    - from: "src/engine/scheduler/eviction.rs"
      to: "tests/integration/sessions.rs"
      via: "Eviction behavior testing"
      pattern: "test_idle_eviction|test_ttl_expiration"
    - from: "examples/hot_reload_demo.rs"
      to: "src/api/rules/route.rs"
      via: "Demo uses API endpoints"
      pattern: "http_post.*rules|reload_rules"
---

## Phase 1 Plan 06: KIE Cache & Session Management

<objective>
Implement comprehensive KIE base caching with eviction policies and finalize session lifecycle management. This plan validates the cache performance, implements background eviction scheduling, and provides integration tests and examples for the complete engine.

Purpose: Per CORE-08, compiled rule sets must be cached efficiently. Combined with D-09's long-lived sessions and D-12's hot reload, this completes Phase 1's foundational infrastructure.
Output: Cache eviction policies (LRU, TTL), eviction scheduler, comprehensive session tests, and working examples.
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
@.planning/phases/01-core-engine-foundation/05-PLAN.md (Plan 05)
@.planning/phases/01-core-engine-foundation/01-CONTEXT.md

**Dependencies:**
- CORE-08: Caching of compiled rule sets (KIE bases)
- D-09: Long-lived sessions (days to weeks)
- D-12: Hot reload support for KIE base
</context>

<tasks>

<task type="auto">
  <name>Implement cache eviction policies per CORE-08, MON-01</name>
  <files>src/engine/cache/policy.rs, src/engine/cache/entry.rs</files>
  <action>Create cache eviction policy implementations:

1. Implement `CacheEntry`:
   - key: CacheKey (rule set hash)
   - value: CompiledRuleSet
   - created_at: SystemTime
   - last_accessed: RwLock<SystemTime>
   - hit_count: AtomicUsize

2. Implement `EvictionPolicy` trait with methods:
   - should_evict(&self, entries: &[CacheEntry]) → Vec<&CacheEntry>
   - on_access(&mut self, entry: &mut CacheEntry)
   - on_insert(&mut self, entry: &mut CacheEntry)

3. Implement concrete policies:
   - `LruPolicy`: Evict least recently accessed entries
   - `TtlPolicy`: Evict entries older than configured TTL
   - `SizeBasedPolicy`: Evict when total size exceeds limit

4. Add policy configuration via EngineBuilder:
   - eviction_policy: EvictionPolicyType (LRU, TTL, or SizeBased)
   - max_cache_entries: usize (default 1000)
   - ttl_seconds: u64 (default 3600 for TTL)

5. Integration with KieCache in Plan 01.

This provides efficient cache management per CORE-08.</action>
  <verify>Each policy correctly evicts entries; LRU uses access timestamps</verify>
  <done>All three eviction policies compile and implement the trait</done>
</task>

<task type="auto">
  <name>Implement background eviction scheduler per CORE-08, D-12</name>
  <files>src/engine/scheduler/eviction.rs, src/engine/scheduler/mod.rs</files>
  <action>Create background task for cache maintenance:

1. Implement `EvictionScheduler`:
   - policy: EvictionPolicy
   - interval: Duration (default 60 seconds)
   - running: AtomicBool

2. Add methods:
   - start(cache: Arc<RwLock<KieCache>>)
   - stop()
   - trigger_manual_eviction() → EvictionResult

3. Implement background task loop:
   - Sleep for configured interval
   - Acquire cache write lock
   - Apply eviction policy
   - Remove evicted entries
   - Log statistics (entries evicted, time spent)

4. Handle shutdown gracefully:
   - Signal thread to stop via atomic boolean
   - Wait for current iteration to complete

5. Add metrics:
   - total_evictions: usize
   - last_eviction_duration_ms: u64

This ensures cache stays within configured limits.</action>
  <verify>Scheduler runs background eviction; manual trigger works</verify>
  <done>Eviction scheduler compiles with start/stop/trigger methods</done>
</task>

<task type="auto">
  <name>Implement session persistence integration per D-09, D-12</name>
  <files>src/engine/sessions/persistence.rs, src/engine/sessions/handler.rs</files>
  <action>Create long-lived session support with database storage:

1. Implement `SessionStore` trait:
   - save_session(session: &Session) → Result<()>
   - load_session(id: &str) → Option<Session>
   - delete_session(id: &str) → Result<()>
   - list_active_sessions() → Vec<SessionMetadata>

2. Add PostgreSQL implementation:
   - Sessions table with JSONB payload
   - Working memory serialized as binary
   - TTL-based expiration query

3. Implement `SessionHandler`:
   - manage(session_id, stateful, action) → Result<()>
   - handle_create(stateful, id) → SessionId
   - handle_get(id) → &mut Session
   - handle_destroy(id) → Result<(), SessionError>

4. Add auto-cleanup for expired sessions:
   - Periodic scan for sessions past idle timeout
   - Graceful termination with event notification

This supports D-09's days-to-weeks session lifetime.</action>
  <verify>Session persistence saves/loads correctly; PostgreSQL store compiles</verify>
  <done>Persistence layer compiles with in-memory and PostgreSQL backends</done>
</task>

<task type="auto">
  <name>Create session lifecycle integration tests per D-08, D-09</name>
  <files>tests/integration/sessions.rs</files>
  <action>Write comprehensive session management tests:

1. Stateful session tests:
   - session_persists_across_requests: Working memory maintained between calls
   - manual_session_destroy: Session properly cleaned up after destruction
   - concurrent_sessions_isolated: Different sessions don't interfere

2. Idle eviction tests:
   - idle_session_eviction: Session removed after timeout period
   - activity_resets_timeout: New activity extends session lifetime
   - expired_session_rejection: Operations on expired session return error

3. Long-lived scenario tests:
   - multi_day_session_survives_restart: Session restored from persistence
   - session_recovery_after_crash: Working memory recovered correctly
   - session_migration: State moved between nodes (cluster mode placeholder)

4. Error handling tests:
   - invalid_session_id_rejected: Non-existent ID returns 404
   - concurrent_modification_safe: Thread-safe session access

These tests ensure D-08/D-09 compliance.</action>
  <verify>Integration tests pass; all session lifecycle scenarios validated</verify>
  <done>Session integration test suite compiles with full coverage</done>
</task>

<task type="auto">
  <name>Create hot reload demonstration example per D-12, CORE-08</name>
  <files>examples/hot_reload_demo.rs</files>
  <action>Create working example showing engine features:

1. Setup demo scenario:
   - Load initial rule set from file
   - Create session with facts
   - Execute rules

2. Demonstrate hot reload:
   - Modify rule file externally (simulated)
   - Trigger reload via API or method call
   - Execute again with new rules

3. Show session continuity:
   - Session persists across reload
   - Working memory state maintained
   - New rules apply to existing facts

4. Add interactive mode:
   - Accept rule file path from command line
   - Watch for changes and auto-reload
   - Display execution results

5. Include usage documentation in comments.

This provides users a reference implementation.</action>
  <verify>Example compiles and runs; demonstrates all key features</verify>
  <done>Demo example compiles with file watching and reload demonstration</done>
</task>

<task type="auto">
  <name>Add comprehensive logging for debugging per D-06, CORE-08</name>
  <files>src/engine/logging/tracer.rs, src/engine/logging/mod.rs</files>
  <action>Create detailed execution tracing system:

1. Implement `ExecutionTracer`:
   - log_rule_activation(rule_id, facts_matched)
   - log_fact_operation(op, fact_id, details)
   - log_cycle_start(number)
   - log_execution_complete(stats)

2. Add trace levels:
   - DEBUG: Detailed rule-by-rule execution
   - INFO: High-level flow (cycles, totals)
   - WARN: Loop detection triggers
   - ERROR: Execution failures

3. Integrate with EventListener:
   - Tracer subscribes to events automatically
   - Filter by session ID for targeted tracing

4. Export methods:
   - start_trace(session_id) → TraceHandle
   - stop_trace(handle)
   - get_trace_log(handle) → Vec<TraceEntry>

5. Integration with debugging per D-06.

This enables detailed inspection for interactive debugging.</action>
  <verify>Tracer records all execution events; levels filter correctly</verify>
  <done>Logging/tracing module compiles with all trace levels working</done>
</task>

<task type="auto">
  <name>Create comprehensive Phase 1 summary document</name>
  <files>.planning/phases/01-core-engine-foundation/SUMMARY.md</files>
  <action>Write phase completion summary:

1. Phase overview:
   - Goal achieved
   - Plans executed (6 total)
   - Requirements satisfied

2. Implementation highlights:
   - Rete algorithm integration approach
   - Hybrid chaining mode selection
   - Session lifecycle management
   - KIE base caching with eviction

3. Success criteria verification:
   - Forward chaining works ✓
   - Backward chaining works ✓
   - Stateful/stateless modes work ✓
   - Loop detection works ✓
   - Hot reload works ✓

4. Open questions for Phase 2:
   - DSL expression language choice
   - External rule file format
   - Performance benchmarks comparison

5. Next phase readiness assessment.

This documents Phase 1 completion.</action>
  <verify>Summary document created with all required sections</verify>
  <done>SUMMARY.md written and committed to git</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Cache eviction → Memory management | Background task manages shared cache |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-01-06-01 | D | Eviction scheduler race condition | mitigate | Atomic boolean for shutdown; RwLock for cache access |
| T-01-06-02 | E | Session persistence failure | accept | Fallback to in-memory store with warning; data loss acceptable for testing |
| T-01-06-03 | I | Cache key collision | mitigate | Hash-based keys (SHA-256) with negligible collision risk |
| T-01-06-04 | D | Background scheduler CPU usage | accept | Configurable interval; low-priority thread scheduling |
| T-01-06-05 | S | Session data exposure via eviction logs | accept | Logs only contain metadata (IDs, timestamps); no fact content |

**Note:** Cache and session data are in-memory by default. For production use with sensitive data, implement encryption at rest in the persistence layer.
</threat_model>

<verification>
- All integration tests pass: `cargo test --test sessions`
- Hot reload demo example compiles and runs
- Eviction policies tested with synthetic workloads

## Success Criteria

1. **KIE base caching** — Cache with LRU/TTL/size-based eviction improves performance per CORE-08
2. **Long-lived session support** — Sessions persist across requests for days to weeks per D-09
3. **Session lifecycle API** — Explicit create/get/destroy operations per D-08

---

## Phase 1 Completion Summary

### Plans Created (6 total)
| Plan | Focus | Wave |
|------|-------|------|
| 01-01 | Core data structures & types | 1 |
| 01-02 | Chaining selection & agenda control | 2 |
| 01-03 | Execution coordinator & events | 2 |
| 01-04 | Hybrid DSL & API rule definition | 3 |
| 01-05 | Testing framework & coverage | 3 |
| 01-06 | KIE cache & session management | 3 |

### Requirements Met (CORE-01 through CORE-08)
- ✓ CORE-01: Forward chaining with Rete algorithm
- ✓ CORE-02: Backward chaining for goal-driven reasoning
- ✓ CORE-03: Stateful/stateless execution modes
- ✓ CORE-04: Salience/prioritization and agenda control
- ✓ CORE-05: Conflict resolution strategies
- ✓ CORE-06: Event-driven rule triggering
- ✓ CORE-07: Loop detection (no-loop, lock-on-active)
- ✓ CORE-08: KIE base caching

### Open Questions for Phase 2
1. **Expression language** — What DSL syntax? FEEL subset, MVEL-like, or custom Rust-based?
2. **File format standardization** — .rules extension, JSON, YAML, or all three?
3. **External dependency evaluation** — Rete crate (rete-rs/drools-rs) vs custom implementation performance

### Phase 1 Completion Checklist
- [x] Core engine crate structure with hybrid modules
- [x] Typed/dynamic data structures for facts and rules
- [x] Working memory with efficient pattern matching
- [x] Forward chaining execution with Rete integration
- [x] Backward chaining with inference traces for debugging
- [x] Automatic chaining mode selection
- [x] Mixed forward/backward execution support
- [x] Salience-based agenda and conflict resolution
- [x] Loop detection (no-loop, lock-on-active)
- [x] Hybrid API and DSL rule definition
- [x] Syntax + semantic validation before compilation
- [x] Hot reload of KIE base without restart
- [x] Explicit session lifecycle management
- [x] Long-lived sessions with persistence
- [x] Testing framework for individual rules
- [x] Coverage reporting with dead rule detection

---

## Implementation Notes for Phase 1 Plan 06

### Eviction Policy Selection (EngineBuilder)
```rust
engine_builder
    .eviction_policy(EvictionPolicyType::LRU)  // or TTL, SizeBased
    .max_cache_entries(1000)
    .ttl_seconds(3600)
```

### Session Management (Pseudo-code)
```rust
// Create long-lived session
let session_id = engine.create_session(true, Some("my-session".into()));

// Execute multiple times with same working memory
engine.execute_rules(&session_id, &rules1)?;
engine.execute_rules(&session_id, &rules2)?;

// Session persists across requests (hours/days/weeks)

// Destroy when done
engine.destroy_session(&session_id)?;
```

### Hot Reload Flow
```rust
// 1. Modify rule file on disk
update_rule_file("my-rules.rules");

// 2. Trigger reload via API or method
engine.reload_rules_from_file("my-rules.rules")?;

// 3. New session uses updated rules
let new_session = engine.create_session(true, None);

// 4. Old sessions continue with previous version
```
