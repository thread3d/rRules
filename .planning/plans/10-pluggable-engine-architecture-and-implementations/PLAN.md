# Phase 10: Pluggable Engine Architecture and Rete-Family Algorithm Implementations

**Phase Number:** 10  
**Milestone:** v3.0 - Multiple Rule Engine Implementations  
**Date:** 2026-06-09  
**Status:** Draft Plan

---

## Executive Summary

Implement a pluggable engine architecture supporting multiple Rete-family algorithms (Rete, TREAT, LEAPS, PHREAK) with runtime selection via configuration or API endpoint.

**Success Criteria:**
1. Engine trait provides unified interface for all Rete-family algorithms
2. Rete implementation correctly handles party tokens, alpha/beta memory, network caching
3. TREAT, LEAPS, and PHREAK implementations each demonstrate their key algorithmic features
4. Users can select engine via configuration file or API endpoint
5. Engine-specific metrics track execution characteristics per algorithm

---

## Phase Overview

### Goals

| Goal | Description |
|------|-------------|
| **G10-01** | Design pluggable engine architecture with trait-based interface |
| **G10-02** | Implement native Rete algorithm with party tokens, alpha/beta memory |
| **G10-03** | Implement TREAT algorithm with optimized left-memory management |
| **G10-04** | Implement LEAPS algorithm with aggregation node support |
| **G10-05** | Implement PHREAK algorithm with incremental truth maintenance |
| **G10-06** | Build runtime engine selection via configuration and API endpoint |
| **G10-07** | Add per-engine metrics collection (firing rate, memory usage, network traversal) |
| **G10-08** | Create comparison dashboard for algorithm performance analysis |

### Deliverables

| File/Module | Description |
|-------------|-------------|
| `engine/src/engine/mod.rs` | Main exports and type definitions |
| `engine/src/engine/registry.rs` | Engine registry with dynamic registration |
| `engine/src/engine/selector.rs` | Runtime engine selection (config/API) |
| `engine/src/engine/metrics.rs` | Unified metrics collection trait |
| `engine/src/engine/rete/` | Rete algorithm implementation module |
| `engine/src/engine/treat/` | TREAT algorithm implementation module |
| `engine/src/engine/leaps/` | LEAPS algorithm implementation module |
| `engine/src/engine/phreak/` | PHREAK algorithm implementation module |

---

## Plan Breakdown

### Plan 10-01: Design Pluggable Engine Architecture

**Objective:** Create the foundation for pluggable engines with trait-based interface.

**Tasks:**

1. **Create `engine/` directory structure**
   ```
   engine/src/engine/
   ├── mod.rs
   ├── registry.rs
   ├── selector.rs
   └── metrics.rs
   ```

2. **Define Engine trait**
   ```rust
   pub trait ExecutionEngine: Send + Sync {
       fn execute(&mut self) -> ExecutionResult;
       fn insert_fact(&mut self, fact: Fact);
       fn remove_fact(&mut self, fact_id: &str);
       fn run(&mut self) -> usize;  // Returns rules fired count
       fn get_metrics(&self) -> EngineMetrics;
   }
   ```

3. **Implement EngineRegistry**
   - Factory pattern for engine creation
   - Dynamic registration by algorithm name

4. **Implement EngineSelector**
   - Parse configuration file (YAML)
   - Provide API endpoint for runtime selection

**Output:**
- `engine/src/engine/registry.rs`
- `engine/src/engine/selector.rs`
- `engine/src/engine/metrics.rs`

**Verification:**
- Can create different engine types via registry
- Configuration parsing works correctly

---

### Plan 10-02: Implement Native Rete Algorithm

**Objective:** Create a full Rete algorithm implementation with party tokens, alpha/beta memory.

**Tasks:**

1. **Create `rete/` module structure**
   ```
   rete/
   ├── mod.rs
   ├── network.rs
   ├── node.rs
   ├── party_tokens.rs
   └──alpha_memory.rs
       beta_memory.rs
   ```

2. **Implement party token structures**
   ```rust
   pub struct MemoryReference {
       pub fact_id: FactId,
       pub generation: usize,
   }
   
   pub type Nagram = Vec<MemoryReference>;
   ```

3. **Implement alpha memory network**
   - Test nodes with fact-type indexing
   - Propagation to join nodes

4. **Implement beta memory**
   - Join node partial match storage
   - Nagram management

5. **Implement network propagation**
   - Assert fact → propagate through network
   - Retract fact → clear dependent activations

**Output:**
- `engine/src/engine/rete/` module with all components
- Unit tests for each network element

**Verification:**
- Rete algorithm fires rules in correct order
- Network caching reduces redundant evaluations

---

### Plan 10-03: Implement TREAT Algorithm

**Objective:** Create TREAT implementation with optimized left-memory management.

**Tasks:**

1. **Extend Rete network structures**
   - Add LRU cache for join node partial matches

2. **Implement caching strategy**
   ```rust
   pub struct TreatNetwork {
       base: ReteNetwork,
       cache: LruCache<NodeId, Vec<Nagram>>,
   }
   ```

3. **Implement cache hit/miss logic**
   - Try to use cached partial match before full evaluation
   - Update cache on network changes

4. **Optimize partial matching**
   - Avoid creating full Nagrams when only need existence check

**Output:**
- `engine/src/engine/treat/` module

**Verification:**
- TREAT produces same results as Rete for identical inputs
- Cache statistics show hits for repeated patterns

---

### Plan 10-04: Implement LEAPS Algorithm

**Objective:** Create LEAPS implementation with aggregation node support.

**Tasks:**

1. **Create aggregation node types**
   ```rust
   pub enum AggregationOp {
       Sum,
       Count,
       Max,
       Min,
   }
   
   pub struct AggregationNode {
       op: AggregationOp,
       source_field: String,
       current_value: f64,
   }
   ```

2. **Implement aggregation operations**
   - Incremental updates when base facts change
   - Recalculate on node retraction

3. **Integrate with network structure**
   - Add aggregation nodes to join node chains
   - Propagate aggregated values downstream

**Output:**
- `engine/src/engine/leaps/` module

**Verification:**
- Aggregation results match equivalent Rete query
- Incremental updates maintain correctness

---

### Plan 10-05: Implement PHREAK Algorithm

**Objective:** Create PHREAK implementation with incremental truth maintenance.

**Tasks:**

1. **Implement justification structure**
   ```rust
   pub struct Justification {
       rule_id: String,
       matched_facts: Vec<FactId>,
       derived_facts: Vec<FactId>,
   }
   ```

2. **Build dependency graph**
   - Track which rules produce which facts
   - Maintain reverse dependencies for invalidation

3. **Implement incremental update logic**
   - When base fact changes, find dependent justifications
   - Re-evaluate only affected parts of network

**Output:**
- `engine/src/engine/phreak/` module

**Verification:**
- PHREAK produces same results as Rete
- Justification traces are complete and useful for debugging

---

### Plan 10-06: Build Runtime Engine Selection

**Objective:** Enable users to select engine via configuration or API.

**Tasks:**

1. **Implement configuration parsing**
   ```yaml
   engine:
     algorithm: rete  # treat, leaps, phreak
   ```

2. **Create runtime selection endpoint**
   ```
   POST /api/v1/engine/select
   Body: { "algorithm": "treat" }
   ```

3. **Implement session migration**
   - Detect incompatible state between engine switches
   - Provide clear error messages

4. **Add engine-specific logging**
   - Log algorithm name with each execution

**Output:**
- Engine selector integration with API layer
- Configuration documentation

**Verification:**
- Configuration file selection works
- Runtime API endpoint switches engines correctly

---

### Plan 10-07: Add Per-Engine Metrics Collection

**Objective:** Track and expose engine-specific performance metrics.

**Tasks:**

1. **Extend EngineMetrics trait**
   ```rust
   pub trait EngineMetrics {
       fn firing_rate(&self) -> f64;
       fn memory_usage(&self) -> usize;
       fn alpha_memory_size(&self) -> usize;
       fn beta_memory_size(&self) -> usize;
       fn network_traversals(&self) -> u64;
   }
   ```

2. **Implement metrics collection in each engine**
   - Track firing rate (rules per second)
   - Track memory usage by alpha/beta memories
   - Count network traversals

3. **Create Prometheus exporter**
   ```rust
   pub fn to_prometheus(metrics: &dyn EngineMetrics) -> String;
   ```

**Output:**
- `engine/src/engine/metrics.rs` with implementation for each engine
- Prometheus endpoint `/metrics`

**Verification:**
- Metrics are collected and exported correctly
- Prometheus format is valid

---

### Plan 10-08: Create Algorithm Comparison Dashboard

**Objective:** Build web dashboard for comparing algorithm performance.

**Tasks:**

1. **Create metrics storage structure**
   - Store historical metrics per engine
   - Allow time-based queries

2. **Build React dashboard components**
   ```tsx
   EngineComparisonChart {
       // Line chart for firing rate over time
   }
   
   MemoryUsageMetric {
       // Bar chart comparing alpha/beta memory usage
   }
   ```

3. **Add historical comparison**
   - Compare current run to baseline
   - Show percentage differences

**Output:**
- React dashboard components in `frontend/src/metrics/`
- Metrics API endpoints for frontend consumption

**Verification:**
- Dashboard displays real-time metrics
- Historical comparisons work correctly

---

## Dependencies

| Phase | Dependency | Reason |
|-------|------------|--------|
| Phase 10-01 | No dependencies | Foundational architecture |
| Phase 10-02 | 10-01 | Needs trait definition |
| Phase 10-03 | 10-02 | Builds on Rete network |
| Phase 10-04 | 10-01 | Uses engine interface |
| Phase 10-05 | 10-01 | Uses engine interface |
| Phase 10-06 | 10-01 | Needs engine registry |
| Phase 10-07 | 10-02,10-03,10-04,10-05 | Requires all engine metrics |
| Phase 10-08 | 10-07 | Needs metrics data |

---

## Risks & Dependencies

### Technical Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Trait object overhead significant | Performance | Profile early; consider enum-based registry for hot path |
| Network propagation complex | Correctness | Extensive unit tests per node type |
| Cross-engine equivalence hard to verify | Quality | Shared test suite with engine-agnostic assertions |

### Schedule Risks

| Milestone | Risk |
|-----------|------|
| All engines pass functional tests | Test coverage must be comprehensive before verification |
| Runtime switching without state corruption | Session state must be engine-agnostic or migratable |

---

## Verification Plan

### Unit Tests Per Engine

```rust
// Each engine module has its own tests
mod rete_tests {
    #[test]
    fn test_party_token_propagation() { ... }
    
    #[test]
    fn test_alpha_memory_insert() { ... }
}

mod treat_tests {
    #[test]
    fn test_cache_hit_on_repeated_pattern() { ... }
}
```

### Cross-Engine Verification

```rust
// Shared tests that all engines must pass
#[cfg(test)]
mod cross_engine_tests {
    use super::super::*;
    
    fn run_test_with<E: ExecutionEngine>(engine_factory: impl Fn() -> E) {
        let mut engine = engine_factory();
        // Load rules, insert facts, execute, verify
    }
    
    #[test]
    fn all_engines_produce_same_results() {
        run_test_with(ReteEngine::new);
        run_test_with(TreatEngine::new);
        run_test_with(LeapsEngine::new);
        run_test_with(PhreakEngine::new);
    }
}
```

---

## Acceptance Criteria

| Requirement | Verification |
|-------------|--------------|
| ENGINE-01: Pluggable architecture | Registry allows dynamic engine creation by name |
| ENGINE-02: Rete implementation | Party tokens, alpha/beta memory work correctly |
| ENGINE-03: TREAT implementation | Join node caching improves performance |
| ENGINE-04: LEAPS implementation | Aggregation nodes compute correct sums/counts |
| ENGINE-05: PHREAK implementation | Justification tracking works for debugging |
| ENGINE-06: Runtime selection | Config file and API both work to switch engines |
| METRIC-01: Metrics collection | Prometheus endpoint returns valid metrics |
| METRIC-02: Comparison dashboard | Dashboard displays real-time engine metrics |

---

## Success Criteria (What Must Be TRUE)

At completion, the following must be verifiable:

1. **All engines implemented** - `ReteEngine`, `TreatEngine`, `LeapsEngine`, `PhreakEngine` all exist
2. **Pluggable architecture works** - Can create and execute with any engine via registry
3. **Configuration selection works** - YAML config file selects algorithm correctly
4. **API selection works** - `/engine/select` endpoint switches engines without crash
5. **Functional equivalence verified** - Same rules/facts produce same results across all algorithms
6. **Metrics are collected** - Performance metrics export to Prometheus format

---

## Notes

- Start with Rete as it's the most straightforward algorithm
- TREAT can build on Rete network structures (add caching layer)
- LEAPS extends Rete network with special node types
- PHREAK adds metadata to Rete execution but doesn't change core structure

---

*Phase plan created: 2026-06-09*
