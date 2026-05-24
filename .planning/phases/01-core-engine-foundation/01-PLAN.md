---
phase: 01-core-engine-foundation
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/engine/Cargo.toml
  - src/engine/lib.rs
  - src/engine/data_types.rs
  - src/engine/fact.rs
  - src/engine/rules.rs
  - src/engine/environment.rs
autonomous: true
requirements:
  - CORE-01
  - CORE-02
user_setup: []
must_haves:
  truths:
    - "Users can execute rules using forward chaining with Rete algorithm"
    - "Users can perform backward chaining for goal-driven reasoning"
    - "Engine uses hybrid typed/dynamic data structures per D-02"
    - "Module organization follows domain layers pattern per D-01"
  artifacts:
    - path: "src/engine/Cargo.toml"
      provides: "Engine crate configuration with Rete dependency"
      exports: ["rete", "serde"]
    - path: "src/engine/lib.rs"
      provides: "Root engine module re-exports"
      exports: ["Engine", "Session", "RuleSet", "execute_forward", "execute_backward"]
    - path: "src/engine/data_types.rs"
      provides: "Typed and dynamic data structures for rules"
      exports: ["Value", "Fact", "RuleExpression"]
    - path: "src/engine/fact.rs"
      provides: "Fact representation and working memory operations"
      exports: ["WorkingMemory", "FactType"]
    - path: "src/engine/rules.rs"
      provides: "Rule definition, parsing, and compilation"
      exports: ["Rule", "Condition", "Action"]
    - path: "src/engine/environment.rs"
      provides: "Engine initialization and configuration"
      exports: ["EngineBuilder", "ChainingMode"]
  key_links:
    - from: "src/engine/lib.rs"
      to: "src/engine/data_types.rs"
      via: "Value type exports"
      pattern: "pub use.*data_types::(Value|Fact)"
    - from: "src/engine/lib.rs"
      to: "src/engine/fact.rs"
      via: "WorkingMemory usage"
      pattern: "WorkingMemory"
    - from: "src/engine/lib.rs"
      to: "src/engine/rules.rs"
      via: "Rule and RuleSet exports"
      pattern: "pub use.*rules::(Rule|RuleSet)"
---

## Phase 1 Plan 01: Core Engine Foundation - Data Models & Types

<objective>
Create the foundational data structures and type system for the rRules engine. This plan implements the hybrid typed/dynamic data model, core fact representation, rule definition structures, and working memory operations.

Purpose: Establishes the bedrock types that all subsequent engine components will use. Per D-02, we need both strongly-typed Rust structs for internal processing and dynamic key-value capabilities for business users.
Output: Core engine library crate with typed data model, fact system, rule definitions, and environment initialization.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/01-core-engine-foundation/01-CONTEXT.md

**Architectural Constraints:**
- D-01: Hybrid module organization — domain layers at top level with feature subdirectories (e.g., rete/arena/, rete/matcher/)
- D-02: Typed internal data structures supporting dynamic casting and reflection
- D-03: Start with Rete crate (evaluate `rete-rs`, `drools-rs`), replace if needed
</context>

<tasks>

<task type="auto">
  <name>Create engine crate structure with hybrid module organization</name>
  <files>src/engine/Cargo.toml, src/engine/lib.rs, src/engine/mod.rs</files>
  <action>Setup Rust crate structure following D-01 domain-layer pattern:

1. Create `src/engine/` directory with Cargo.toml
2. Configure dependencies: serde for serialization, rete-rs (evaluate as starting point per D-03)
3. Implement lib.rs with module declarations exposing:
   - data_types.rs (Value, Fact types)
   - fact.rs (WorkingMemory)
   - rules.rs (Rule, Condition, Action)
   - environment.rs (EngineBuilder)
   - chaining/ directory for forward/backward chaining implementations
4. Create mod.rs with re-exports of public API

Follow domain-layer pattern: data_types as foundation layer, then fact/rules/environment as feature modules.</action>
  <verify>cargo build --package engine succeeds</verify>
  <done>Crate builds without errors; all module declarations present and exported</done>
</task>

<task type="auto">
  <name>Implement hybrid typed/dynamic data structures per D-02</name>
  <files>src/engine/data_types.rs, src/engine/value.rs, src/engine/fact_type.rs</files>
  <action>Create the hybrid data structure system supporting both compile-time types and runtime dynamic capabilities:

1. Implement `Value` enum with variants:
   - Typed: Bool, Integer, Float, String, DateTime
   - Structured: Object(Map<String, Value>), Array(Vec<Value>)
   - Null variant for optional values

2. Implement `FactType` struct representing a fact schema with:
   - name field (String)
   - typed_fields: HashMap<String, ValueType>
   - dynamic fields support via serde_json::Value fallback

3. Implement `DynamicCast` trait for runtime type conversion with methods:
   - as_bool(), as_integer(), as_string() etc.
   - try_into::<T>() for generic casting
   - is_compatible_with(TypeInfo) for validation

4. Add serde serialization/deserialization support with proper tags for polymorphic types.

The implementation must support both typed access (for engine internals) and dynamic key-value lookup (for rule expressions).</action>
  <verify>cargo test --package engine data_types passes; Value::from_json() roundtrip works</verify>
  <done>Data structures compile with serde; dynamic casting methods implemented for all Value variants</done>
</task>

<task type="auto">
  <name>Implement Fact and WorkingMemory for pattern matching</name>
  <files>src/engine/fact.rs, src/engine/working_memory.rs</files>
  <action>Create fact representation and working memory management:

1. Implement `Fact` struct with:
   - id: UUID
   - fact_type_id: String (reference to FactType)
   - payload: HashMap<String, Value>
   - created_at: SystemTime

2. Implement `WorkingMemory` with operations:
   - insert_fact(Fact) -> Result<()>
   - update_fact(id, updates) -> Result<()>
   - retract_fact(id) -> Result<Option<Fact>>
   - get_facts_by_type(type_id) -> Vec<&Fact>
   - match_pattern(pattern: &RuleExpression) -> Vec<&Fact>

3. Implement `FactRetrievalIndex` for efficient pattern matching:
   - Primary index on fact_type
   - Secondary indexes on frequently-queried fields
   - Automatic index rebuild on schema changes

4. Add clone-on-write semantics for session isolation.

This is the working memory that feeds the Rete algorithm's pattern matcher.</action>
  <verify>WorkingMemory::insert_fact, get_facts_by_type work correctly; retrieval index built</verify>
  <done>Fact operations work; WorkingMemory supports insert/update/retract with efficient retrieval</done>
</task>

<task type="auto">
  <name>Implement Rule definition, parsing, and compilation structures</name>
  <files>src/engine/rules.rs, src/engine/condition.rs, src/engine/action.rs</files>
  <action>Create rule representation supporting both API and DSL entry points:

1. Implement `Rule` struct with:
   - id: String (unique identifier)
   - name: String
   - salience: i32 (priority, per CORE-04)
   - no_loop: bool (per CORE-07)
   - lock_on_active: bool (per CORE-07)
   - condition: Condition
   - actions: Vec<Action>

2. Implement `Condition` as AST for rule expressions:
   - And/Or/Not boolean combinations
   - Comparison operators (==, !=, <, >, <=, >=)
   - Pattern matching against facts
   - Function calls with argument validation

3. Implement `Action` for consequence execution:
   - InsertFact, UpdateFact, RetractFact operations
   - CallFunction for custom logic
   - LogMessage for debugging

4. Add `RuleSet` container with:
   - Compilation to KIE base format (per CORE-08)
   - Dependency resolution between rules
   - Validation before compilation

This structure must support syntax validation per D-11 before compilation.</action>
  <verify>Rule::validate() catches syntax errors; RuleSet compilation produces KIE base representation</verify>
  <done>Rules can be parsed, validated, and compiled; Condition AST handles boolean/comparison/pattern operations</done>
</task>

<task type="auto">
  <name>Implement Engine environment and chaining mode selection per D-04, D-07</name>
  <files>src/engine/environment.rs, src/engine/chain_selector.rs</files>
  <action>Create engine initialization with automatic chaining mode selection (per D-04) and explicit session modes (per D-07):

1. Implement `EngineBuilder` for configuration:
   - chaining_mode: Option<ChainingMode> (Automatic, ForwardOnly, BackwardOnly)
   - default_salience: i32
   - max_iterations: usize (for loop detection per CORE-07)
   - enable_debug_trace: bool (per D-06 backward chaining use case)

2. Implement `Engine` struct with:
   - working_memory: Arc<RwLock<WorkingMemory>>
   - rule_set: RuleSet (compiled KIE base)
   - chaining_selector: ChainingSelector

3. Implement `ChainingMode` enum:
   - Automatic (per D-04: auto-detect based on rule structure and query type)
   - ForwardOnly
   - BackwardOnly

4. Add `Engine::new_session(stateful: bool, session_id: Option<String>) -> Session`:
   - stateful = true for long-lived sessions (per D-09: days to weeks)
   - stateful = false for stateless execution
   - session_id optional for explicit lifecycle management (per D-08)

This implementation provides the API entry point per D-10 hybrid approach.</action>
  <verify>EngineBuilder builds engine with correct mode; Session creation works for both stateful and stateless modes</verify>
  <done>Automatic chaining detection implemented; session lifecycle properly separates stateful/stateless modes per user decision</done>
</task>

<task type="auto">
  <name>Create forward chaining execution entry point per CORE-01</name>
  <files>src/engine/forward_chaining.rs, src/engine/lib.rs (export)</files>
  <action>Implement Rete-based forward chaining algorithm:

1. Integrate with rete-rs crate or implement core Rete components:
   - Alpha memory and network for pattern matching
   - Beta memory for join operations
   - Production memory for rules

2. Implement `execute_forward(engine: &mut Engine, session: &Session) -> ExecutionResult`:
   - Initialize Rete network from compiled KIE base
   - Propagate facts through alpha/beta memories
   - Trigger rule actions when conditions match
   - Handle salience-based agenda ordering (per CORE-04)

3. Implement conflict resolution strategies:
   - Salience priority (higher fires first)
   - Depth-first vs breadth-first
   - Random order for equal salience

4. Add loop detection:
   - Track fired rules per activation cycle
   - no_loop and lock_on_active enforcement (per CORE-07)
   - Maximum iteration limit to prevent infinite loops

5. Export `execute_forward` from lib.rs.

The execution should work with the hybrid data structures created in earlier tasks.</action>
  <verify>Forward chaining executes rules in correct salience order; loop detection prevents infinite loops</verify>
  <done>Rete pattern matching fires rules; forward_chaining module compiles and exports execute_forward function</done>
</task>

<task type="auto">
  <name>Create backward chaining execution entry point per CORE-02, D-06</name>
  <files>src/engine/backward_chaining.rs, src/engine/lib.rs (export)</files>
  <action>Implement goal-driven backward chaining for interactive debugging (per D-06):

1. Implement `execute_backward(engine: &Engine, session: &Session, goal: &Goal) -> InferenceTrace`:
   - Start from query/goal fact pattern
   - Recursively match conditions of applicable rules
   - Build inference tree showing why goal succeeded/failed
   - Support interactive trace exploration

2. Implement `InferenceTrace` with:
   - Root goal
   - Rule nodes that contributed to resolution
   - Failed branches with reasons
   - Fact matches at each level

3. Add debugging-specific features:
   - Trace depth limit for performance
   - Step-by-step execution for interactive debugger
   - Explanation generation ("Rule X fired because facts A, B matched")

4. Export `execute_backward` from lib.rs.

This enables the primary use case of D-06: "Interactive debugging as primary backward chaining use case."</action>
  <verify>Backward chaining starts from goal; InferenceTrace shows full reasoning path for debugging</verify>
  <done>Backward_chaining module compiles and exports execute_backward function with InferenceTrace return type</done>
</task>

<task type="auto">
  <name>Add syntax + semantic validation before compilation per D-11</name>
  <files>src/engine/validation.rs, src/engine/validator.rs</files>
  <action>Implement comprehensive rule validation per D-11 (syntax + semantic checks):

1. Implement `SyntaxValidator`:
   - Parse tree validation
   - Balanced parentheses/brackets in expressions
   - Valid operator combinations
   - Known function names and argument counts

2. Implement `SemanticValidator`:
   - Type checking: condition values match fact field types
   - Dependency resolution: referenced facts exist in type registry
   - Circular references detection between rules
   - Unused variable warnings

3. Add validation pipeline to RuleSet::compile():
   - Run syntax validation first (fail fast)
   - Run semantic validation on valid syntax
   - Collect and report all errors with locations

4. Add `ValidationResult` with:
   - is_valid: bool
   - errors: Vec<ValidationError>
   - warnings: Vec<ValidationWarning>

This ensures D-11: "Syntax + semantic validation before compilation; errors caught early with clear messages."</action>
  <verify>Invalid rules fail syntax validation; rules with type mismatches fail semantic validation</verify>
  <done>Validation module compiles; SyntaxValidator and SemanticValidator produce ValidationResult with errors</done>
</task>

<task type="auto">
  <name>Implement KIE base caching layer per CORE-08</name>
  <files>src/engine/kie_cache.rs, src/engine/cache_key.rs</files>
  <action>Create compiled rule cache supporting hot reload (per D-12):

1. Implement `KieCache` with:
   - CompiledRuleSet storage (LRU or TTL-based eviction)
   - CacheKey generation from rule content hash
   - Thread-safe access via RwLock

2. Implement `CacheKey` calculation:
   - Hash of rule definitions
   - Hash of FactType schemas
   - Engine configuration (salience defaults, etc.)

3. Add KIE base loading/saving:
   - Serialize compiled rules to binary format
   - Deserialize on cache hit
   - Atomic replacement for hot reload

4. Implement `Engine::load_rules(rules: &str) -> RuleSet` that checks cache first.

This supports D-12: "Hot reload support — KIE base updates dynamically when rules change without requiring service restart."</action>
  <verify>Cached rule set loads faster than fresh compilation; cache key changes on rule update</verify>
  <done>KieCache compiles and stores/retrieves compiled RuleSets using content-based CacheKey</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Client → Engine API | User-provided rules and fact data crosses boundary |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-01-01 | S | Rule parsing via `Rule::from_str` | mitigate | Syntax validation rejects malformed rules; resource limits on expression depth |
| T-01-02 | D | WorkingMemory operations | mitigate | Transaction isolation with RwLock; fact IDs prevent injection via identifiers |
| T-01-03 | I | Rule compilation (KIE base) | mitigate | Validation phase catches semantic errors; hash-based cache keys prevent tampering |
| T-01-04 | R | Dynamic casting (`Value::try_into`) | accept | Limited to engine internals; no privileged operations from cast result |
| T-01-05 | E | Infinite loop prevention | mitigate | Max iteration limit in EngineBuilder; no_loop/lock_on_active enforcement per CORE-07 |

**Security Note:** This is Phase 1 core engine foundation. Full authentication/authorization (AUTH-01, AUTH-02) is Phase 4 responsibility. Current phase focuses on rule execution safety within a trusted execution environment.
</threat_model>

<verification>
- Engine builds: `cargo build --package engine`
- Forward chaining test: Rules fire in salience order; no-loop prevents re-firing
- Backward chaining test: Inference trace shows path from goal to supporting facts
- Validation tests: Syntax errors caught before compilation; type mismatches detected
- Cache test: Recompilation skipped when rule content unchanged

## Success Criteria

1. **Users can execute rules using forward chaining with Rete algorithm** — `execute_forward` fires rules in salience order with conflict resolution
2. **Users can perform backward chaining for goal-driven reasoning** — `execute_backward` starts from query and traces inference path
3. **Stateful/stateless mode selection via API** — EngineBuilder with explicit session creation supports both modes per D-07
4. **Automatic chaining mode selection** — ChainingSelector chooses forward/backward based on rule structure per D-04
5. **Loop detection controls (no-loop, lock-on-active)** — Enforcement in forward_chaining module per CORE-07

---

## Implementation Notes for Phase 1 Plan 01

### Hybrid Module Organization (D-01)
```
src/engine/
├── Cargo.toml           # Rete dependency (evaluate rete-rs first)
├── lib.rs               # Root exports
├── mod.rs               # Module declarations
├── data_types.rs        # Value, Fact, dynamic casting
├── fact.rs              # WorkingMemory operations
├── rules.rs             # Rule AST and compilation
├── environment.rs       # EngineBuilder, session creation
├── forward_chaining.rs  # Rete algorithm integration
├── backward_chaining.rs # Goal-driven inference with traces
├── validation.rs        # Syntax + semantic validators
└── kie_cache.rs         # Compiled rule caching
```

### Chaining Mode Selection (D-04)
Automatic selection logic:
- Forward chaining: When rules have conditions on existing facts and actions that add new facts
- Backward chaining: When query starts from goal fact pattern needing to be satisfied

### Session Management (D-07, D-08, D-09)
```rust
// Stateless execution (short-lived session)
let result = engine.execute_stateless(rules, facts);

// Stateful long-lived session (days to weeks)
let mut session = engine.create_session(true, Some("my-session".to_string()));
session.insert_fact(fact1)?;
session.execute(rules)?;
```

### Open Questions for Implementation
1. **Rete crate evaluation** — Need to test `rete-rs` vs `drools-rs` for performance and API fit; replacement path needed if inadequate
2. **DSL syntax** — What expression language? FEEL (DMN), MVEL-like, or custom Rust-based?
3. **Binary serialization format** for KIE cache — bincode, serde_cbor, or custom?
