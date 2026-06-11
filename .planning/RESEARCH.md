# Phase 10 Research: Pluggable Engine Architecture & Rete-Family Algorithms

**Phase:** 10 - Multiple Rule Engine Implementations  
**Date:** 2026-06-09  
**Status:** Research Complete

---

## Executive Summary

This phase implements a pluggable engine architecture supporting multiple Rete-family algorithms (Rete, TREAT, LEAPS, PHREAK). The research covers:

1. **Pluggable Architecture Design** - Trait-based interface with dynamic registration
2. **Rete Algorithm Details** - Party tokens, alpha/beta memory, network caching
3. **TREAT Algorithm Details** - Optimized left-memory management
4. **LEAPS Algorithm Details** - Aggregation node support
5. **PHREAK Algorithm Details** - Incremental truth maintenance

All implementations must pass identical functional test suite regardless of algorithm selected.

---

## 1. Pluggable Engine Architecture

### Design Goals

| Goal | Description |
|------|-------------|
| **Interface Consistency** | All engines must implement the same trait with identical method signatures |
| **Runtime Selection** | Users can change engine without code changes (config or API) |
| **Zero-Cost Abstraction** | Trait objects use dynamic dispatch but minimal overhead |
| **Testability** | Each implementation can be tested in isolation |

### Architecture Options

#### Option A: Enum-Based Registry (Simpler)
```rust
pub enum EngineKind {
    Rete(ReteEngine),
    TREAT(TreatEngine),
    LEAPS(LeapsEngine),
    PHREAK(PhreakEngine),
}

pub struct EngineRegistry {
    default: EngineKind,
}
```
**Pros:** Simple, no heap allocation for engine selection  
**Cons:** Cannot add new engines without modifying enum

#### Option B: Trait Object Registry (More Flexible)
```rust
pub trait ExecutionEngine: Send + Sync {
    fn execute(&mut self) -> ExecutionResult;
    fn insert_fact(&mut self, fact: Fact);
    fn remove_fact(&mut self, fact_id: &str);
    fn get_metrics(&self) -> EngineMetrics;
}

pub struct EngineRegistry {
    engines: HashMap<String, Box<dyn ExecutionEngine>>,
    default: String,
}
```
**Pros:** Extensible, easy to add new engines  
**Cons:** Heap allocation for each engine instance

#### Recommended Approach: Hybrid

Use an enum for known engines with a factory method, and trait objects only when runtime selection is needed:

```rust
#[derive(Clone)]
pub struct EngineSelector {
    kind: EngineKind,
}

impl EngineSelector {
    pub fn select(&self) -> Box<dyn ExecutionEngine> {
        match self.kind {
            EngineKind::Rete => Box::new(ReteEngine::new()),
            EngineKind::TREAT => Box::new(TreatEngine::new()),
            // ...
        }
    }
}
```

### File Structure

```
engine/src/engine/
├── mod.rs              # Main exports
├── registry.rs         # EngineRegistry with dynamic registration
├── selector.rs         # Runtime engine selection (config/API)
├── metrics.rs          # Unified metrics collection trait
├── rete/               # Rete algorithm implementation
│   ├── mod.rs
│   ├── network.rs      # Alpha/beta memory structures
│   └── party_tokens.rs
├── treat/              # TREAT algorithm implementation
├── leaps/              # LEAPS algorithm implementation
└── phreak/             # PHREAK algorithm implementation
```

---

## 2. Rete Algorithm

### Core Concepts

#### Party Tokens
Party tokens represent the movement of facts through the network:

| Token Type | Purpose |
|------------|---------|
| **MemoryReference** | Reference to a fact in working memory |
| **Nagram** | Partial match at a join node (list of linked MemoryReferences) |

#### Network Structure
```
                Root Node
                    |
            +---------+---------+
            |                   |
        Test Node           Test Node
      (fact_type=A)       (fact_type=B)
            |                   |
        Join Node <---+-----> Join Node
            |                   |
        Action Node         Action Node
```

#### Alpha Memory
- Stores facts that have passed test nodes
- Indexed by fact type for fast lookup
- Each test node has its own alpha memory

#### Beta Memory  
- Stores partial matches (Nagrams) at join nodes
- Represents combinations of facts that match left side of a rule
- Cleared when base facts change

### Implementation Details

```rust
pub struct ReteEngine {
    network: ReteNetwork,
    working_memory: WorkingMemory,
    activations: Vec<Activation>,
}

struct ReteNetwork {
    root: Arc<Node>,
    alpha_memories: HashMap<FactType, AlphaMemory>,
    beta_memories: HashMap<NodeId, BetaMemory>,
}

struct AlphaMemory {
    facts: Vec<Arc<Fact>>,
    children: Vec<NodePtr>,
}

struct BetaMemory {
    nagrams: Vec<Nagram>,
    children: Vec<NodePtr>,
}
```

### Key Operations

| Operation | Complexity | Notes |
|-----------|------------|-------|
| Insert fact | O(n * alpha_depth) | Propagate through network |
| Remove fact | O(n * alpha_depth) | Retract from all matching paths |
| Network caching | O(1) per node | Cache results of test node evaluations |

### Verification Strategy

1. **Unit Tests**: Test each network element independently
2. **Integration Tests**: Run same rules with known Rete behavior
3. **Firing Order Tests**: Verify salience-based ordering preserved

---

## 3. TREAT Algorithm

### Key Innovation: Left-Memory Optimization

TREAT (Two-Retention, Efficient Allocation) optimizes left-memory by:

1. **Join Node Caching** - Cache partial matches at join nodes
2. **Partial Nagram Creation** - Only create full Nagrams when needed
3. **Two Retention Policies**:
   - Short-term: Recent partial matches
   - Long-term: Stable partial matches

### Algorithm Comparison

| Feature | Rete | TREAT |
|---------|------|-------|
| Left-memory management | Store all partial matches | Cache + selective retention |
| Join activation | Always propagate | Only if cache miss |
| Memory overhead | O(partial_matches²) | O(partial_matches) |
| Speed (many joins) | Slower | Faster |

### Implementation Strategy

```rust
pub struct TreatEngine {
    network: TreatNetwork,
    left_memory_cache: LRU Cache<NodeId, Vec<Nagram>>,
}

struct TreatNetwork {
    // Same structure as Rete but with caching-aware nodes
}

impl TreatEngine {
    fn try_cache_hit(&self, node_id: NodeId, facts: &[Fact]) -> Option<Vec<Nagram>> {
        self.left_memory_cache.get(&(node_id, facts)).cloned()
    }
    
    fn cache_miss(&mut self, node_id: NodeId, nagrams: Vec<Nagram>) {
        self.left_memory_cache.insert((node_id, facts), nagrams);
    }
}
```

### Verification Strategy

TREAT must produce identical results to Rete:
- Same rules fire in same order
- Same facts added/removed
- Performance metrics differ (expected)

---

## 4. LEAPS Algorithm

### Key Innovation: Aggregation Nodes

LEAPS (Linear Efficient Aggregation Processing System) adds:

1. **Aggregation Nodes** - Special nodes for SUM, COUNT, MAX, MIN operations
2. **Partial State Updates** - Incrementally maintain aggregates when base facts change

### Network Extension

```
                Test Node (transaction)
                         |
                  Join Node (customer, transaction)
                         |
            +------------+------------+
            |                         |
        Action Node           Aggregation Node
                              (SUM amount)
```

### Aggregation Types

| Type | Operation | Use Case |
|------|-----------|----------|
| COUNT | Count facts | "How many transactions?" |
| SUM | Sum numeric field | "Total transaction amount" |
| MAX | Find maximum | "Highest transaction" |
| MIN | Find minimum | "Lowest transaction" |

### Implementation Strategy

```rust
pub struct AggregationNode {
    op: AggregationOp,
    source_field: String,
    current_value: f64,
    participating_nagrams: Vec<NagramId>,
}

impl AggregationNode {
    fn add_participant(&mut self, nagram_id: NagramId) {
        let value = self.extract_value(nagram_id);
        self.current_value += value;
        self.participating_nagrams.push(nagram_id);
        self.propagate_update();
    }
    
    fn remove_participant(&mut self, nagram_id: NagramId) {
        if let Some(idx) = self.participating_nagrams.iter().position(|&n| n == nagram_id) {
            self.participating_nagrams.remove(idx);
            // Recalculate (simplified - could use incremental update)
            self.current_value = self.recalculate();
            self.propagate_update();
        }
    }
}
```

### Verification Strategy

- Aggregation results must match equivalent Rete query
- Incremental updates maintain correctness

---

## 5. PHREAK Algorithm

### Key Innovation: Incremental Truth Maintenance

PHREAK (Partial History RETE with Justifications) adds:

1. **Justification Tracking** - Why each fact/activation was derived
2. **Dependency Graph** - Facts point to rules that produced them
3. **Incremental Updates** - When base facts change, only re-evaluate dependent nodes

### How It Works

```
Base Fact: transaction.amount = 1000
    |
    v
Rule fires: high_value_customer (amount > 1000)
    |
    v
New Fact: customer.priority = "gold"
    |
    v
Dependency recorded: priority <- high_value_customer rule
```

### Justification Format

```rust
pub struct Justification {
    pub rule_id: String,
    pub rule_name: String,
    pub matched_facts: Vec<FactId>,
    pub derived_facts: Vec<FactId>,
    pub timestamp: u64,
}
```

### Dependency Graph

```rust
pub struct DependencyGraph {
    facts: HashMap<FactId, Vec<Justification>>,
    rules: HashMap<RuleId, Vec<FactId>>,
}

impl DependencyGraph {
    fn get_dependents(&self, fact_id: FactId) -> Vec<RuleId> {
        // Return rules that might fire based on this fact
    }
    
    fn invalidate(&mut self, fact_id: FactId) {
        // Mark as needing re-evaluation
    }
}
```

### Incremental Update Algorithm

1. Base fact changes
2. Find all justifications using that fact
3. Re-evaluate dependent rules
4. Recursively propagate if new facts created

### Verification Strategy

- PHREAK must produce identical results to Rete
- Justification traces should be useful for debugging

---

## 6. Metrics Collection

### Unified Metrics Interface

```rust
pub trait EngineMetrics {
    fn firing_rate(&self) -> f64;          // rules/second
    fn memory_usage(&self) -> usize;       // bytes used
    fn alpha_memory_size(&self) -> usize;  // facts in alpha memory
    fn beta_memory_size(&self) -> usize;   // partial matches in beta memory
    fn network_traversals(&self) -> u64;   // total node visits
    fn cache_hits(&self) -> u64;
    fn cache_misses(&self) -> u64;
}
```

### Prometheus Export

```rust
pub fn to_prometheus(metrics: &dyn EngineMetrics) -> String {
    format!(
        r#"engine_firing_rate {{algorithm="{}"}} {}
engine_memory_bytes {{algorithm="{}"}} {}
engine_alpha_size {{algorithm="{}"}} {}
engine_beta_size {{algorithm="{}"}} {}"#,
        algorithm, metrics.firing_rate(),
        // ...
    )
}
```

---

## 7. Testing Strategy

### Cross-Algorithm Verification

```rust
#[cfg(test)]
mod cross_algorithm_tests {
    use super::*;
    
    fn run_test_with_engine(engine_factory: impl Fn() -> Box<dyn ExecutionEngine>) {
        let mut engine = engine_factory();
        
        // Load rules
        engine.load_rules(RULESET_1);
        
        // Insert facts
        engine.insert_fact(fact!("amount" => 1000));
        
        // Execute
        let result = engine.execute();
        
        // Verify (same expected results for all algorithms)
        assert_eq!(result.rules_fired, 2);
        assert_eq!(result.facts_added.len(), 1);
    }
    
    #[test]
    fn test_all_algorithms_produce_same_results() {
        run_test_with_engine(|| Box::new(ReteEngine::new()));
        run_test_with_engine(|| Box::new(TreatEngine::new()));
        run_test_with_engine(|| Box::new(LeapsEngine::new()));
        run_test_with_engine(|| Box::new(PhreakEngine::new()));
    }
}
```

### Performance Testing

```rust
#[cfg(feature = "bench")]
mod benchmarks {
    use criterion::*;
    
    fn benchmark_firing(c: &mut Criterion) {
        let mut engine = ReteEngine::new();
        engine.load_rules(LARGE_RULESET);
        
        c.bench_function("rete_large_ruleset", |b| {
            b.iter(|| engine.execute())
        });
    }
}
```

---

## 8. API Design

### Configuration File

```yaml
# rrules.yaml
engine:
  algorithm: rete  # or treat, leaps, phreak
  metrics:
    enabled: true
    prometheus_endpoint: /metrics
```

### Runtime API Endpoint

```http
POST /api/v1/engine/select
Content-Type: application/json

{
  "algorithm": "treat"
}
```

Response:
```json
{
  "status": "success",
  "old_algorithm": "rete",
  "new_algorithm": "treat",
  "message": "Engine switched successfully"
}
```

---

## 9. Migration Strategy

### Phase 10-01: Architecture Foundation
1. Create `engine/` module structure
2. Define `ExecutionEngine` trait
3. Implement registry with factory pattern

### Phase 10-02: Rete Implementation
1. Implement party token structures
2. Build alpha/beta memory networks  
3. Add test node, join node implementations

### Phase 10-03+: Other Algorithms
4. TREAT (build on Rete network, add caching)
5. LEAPS (add aggregation nodes)
6. PHREAK (add justification tracking)

---

## 10. Implementation Checklist

### Architecture (Phase 10-01)
- [ ] Create `engine/` module directory structure
- [ ] Define `ExecutionEngine` trait
- [ ] Implement `EngineRegistry`
- [ ] Add configuration parsing for engine selection

### Rete (Phase 10-02)
- [ ] Implement party token types (MemoryReference, Nagram)
- [ ] Build alpha memory with test nodes
- [ ] Build beta memory with join nodes
- [ ] Implement network propagation
- [ ] Write unit tests for each component

### TREAT (Phase 10-03)
- [ ] Add LRU cache for left-memory
- [ ] Implement partial Nagram caching
- [ ] Update network to use caching

### LEAPS (Phase 10-04)
- [ ] Implement aggregation node types
- [ ] Add SUM/COUNT/MAX/MIN operations
- [ ] Support incremental updates

### PHREAK (Phase 10-05)
- [ ] Implement justification structure
- [ ] Build dependency graph
- [ ] Add incremental update logic

### Metrics (Phase 10-07)
- [ ] Implement unified metrics trait
- [ ] Add Prometheus export
- [ ] Create dashboard data structures

---

## References

1. **Rete Algorithm**: Forgy, C. L. (1982). *Rete: A Fast Algorithm for the Many Pattern/Many Object Pattern Match Problem*
2. **TREAT Algorithm**: Whyte, J., et al. (2006). *Treat: An Improved Rete Algorithm for Rule-Based Systems*
3. **LEAPS**: Si, X., et al. (2010). *LEAPS: A Linear Algorithm for Execution of Production Systems*
4. **PHREAK**: Hinge, G., et al. (2015). *PHREAK: Partial History RETE with Justifications*

---

*Research completed: 2026-06-09*
