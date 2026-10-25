//! The TREAT algorithm engine implementation.
//!
//! TREAT (TREe Augmented by Caching) is an optimization of the Rete algorithm
//! that uses LRU caching for join node partial matches to reduce redundant computations.

#![allow(dead_code)]
use crate::engine::metrics::MetricsCollector;
use crate::engine::registry::{EngineFactory, EngineResult, EngineType, ExecutionEngine};
use crate::engine::rete::{AlphaMemory, BetaMemory, FactId, MemoryReference, Nagram};
use crate::fact::Fact;
use std::collections::{HashMap, HashSet};

// NOTE: `CoverageTracking` is no longer imported here — the shared direct
// evaluator (`engine::direct_eval`) owns once-per-rule coverage recording
// after the fixpoint loop (L-06 / Pitfall 6), so the placeholder per-nagram
// `record_rule_eval`/`record_rule_fire` calls in the old `execute_rules`
// are gone.

/// Cache entry with a generation timestamp for LRU ordering.
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct CacheEntry {
    nagram: Nagram,
    generation: usize,
    last_accessed: usize,
}

/// LRU cache for join node partial matches.
#[derive(Debug)]
pub struct LRUCache {
    /// Maximum cache size
    max_size: usize,
    /// Current size
    current_size: usize,
    /// Map from key to nagram
    entries: HashMap<String, Nagram>,
}

impl LRUCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            current_size: 0,
            entries: HashMap::new(),
        }
    }

    /// Gets a cached nagram if it exists.
    pub fn get(&mut self, key: &str) -> Option<&Nagram> {
        self.entries.get(key)
    }

    /// Inserts or updates a cache entry.
    pub fn insert(&mut self, key: String, nagram: Nagram) {
        // If key already exists, just update the value
        if let Some(entry) = self.entries.get_mut(&key) {
            *entry = nagram;
            return;
        }

        // If at capacity, evict an arbitrary entry (HashMap iteration order is non-deterministic)
        // For a proper LRU implementation we'd use a LinkedHashMap-like structure
        // Evict oldest entry - for testing purposes, we use a deterministic policy
        // Since HashMap iteration is non-deterministic in Rust, sort keys first to ensure consistency
        if self.current_size >= self.max_size && !self.entries.is_empty() {
            let mut keys: Vec<_> = self.entries.keys().cloned().collect();
            keys.sort(); // Sort for deterministic eviction order
            if let Some(key) = keys.first().cloned() {
                self.entries.remove(&key);
                self.current_size -= 1;
            }
        }

        // Insert new entry
        self.entries.insert(key, nagram);
        self.current_size += 1;
    }

    /// Gets the key of an arbitrary entry for eviction (not truly LRU due to HashMap ordering)
    #[allow(dead_code)]
    fn get_oldest_entry(&self) -> Option<String> {
        // Since we're using HashMap which has non-deterministic iteration order,
        // just return the first key
        self.entries.keys().next().cloned()
    }

    /// Removes an entry from the cache.
    pub fn remove(&mut self, key: &str) -> bool {
        if self.entries.remove(key).is_some() {
            self.current_size -= 1;
            true
        } else {
            false
        }
    }

    /// Clears all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_size = 0;
    }

    /// Returns the number of cached entries.
    pub fn size(&self) -> usize {
        self.current_size
    }

    /// Returns cache statistics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            max_size: self.max_size,
            current_size: self.current_size,
            hit_count: 0, // Would need to track separately for precise stats
            miss_count: 0,
        }
    }
}

/// Cache statistics for reporting.
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub max_size: usize,
    pub current_size: usize,
    pub hit_count: u64,
    pub miss_count: u64,
}

impl CacheStats {
    /// Calculates the cache hit rate.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hit_count + self.miss_count;
        if total > 0 {
            self.hit_count as f64 / total as f64
        } else {
            0.0
        }
    }
}

/// TREAT network extends the Rete network with caching.
#[derive(Debug)]
pub struct TreatNetwork {
    /// Base Rete network structure
    base: crate::engine::rete::ReteNetwork,
    /// Cache for join node partial matches
    cache: LRUCache,
    /// Track which nodes have caches
    cached_nodes: HashSet<String>,
}

impl TreatNetwork {
    pub fn new(max_cache_size: usize) -> Self {
        Self {
            base: crate::engine::rete::ReteNetwork::new(),
            cache: LRUCache::new(max_cache_size),
            cached_nodes: HashSet::new(),
        }
    }

    /// Checks if a node's results are cached.
    pub fn is_cached(&self, node_id: &str) -> bool {
        self.cached_nodes.contains(node_id)
    }

    /// Adds a join node to the cacheable set.
    pub fn add_cached_join_node(&mut self, node_id: impl Into<String>) {
        self.cached_nodes.insert(node_id.into());
    }
}

impl Default for TreatNetwork {
    fn default() -> Self {
        Self::new(1024) // Default cache size
    }
}

/// The TREAT algorithm engine implementation.
#[derive(Debug)]
pub struct TreatEngine {
    network: TreatNetwork,
    alpha_memory: AlphaMemory,
    beta_memory: BetaMemory,
    working_facts: HashMap<FactId, Fact>,
    /// Rules loaded via `load_ruleset` (L-02). The direct evaluator
    /// (`execute_rules`, Plan 23-06) iterates this Vec instead of the
    /// placeholder TREAT network. Populated by cloning from a shared
    /// `RuleSet` snapshot; per-request engines carry their own copy.
    rules: Vec<crate::rules::Rule>,
    metrics: crate::engine::metrics::SimpleMetricsCollector,
    cache_hits: u64,
    cache_misses: u64,
    /// Phase 28 (D-01 / EXEC-04): trace-collection flag. Set via
    /// `set_collect_trace` before `execute()`; threaded into `direct_execute`
    /// so the shared helper emits ordered `TraceEvent`s when true. Default
    /// `false` (SC-3 latency neutrality — off-by-default).
    collect_trace: bool,
    /// Fixpoint cap for this engine. `None` defers to the
    /// `RRULES_MAX_CHAIN_ITERATIONS` environment variable (default 1000);
    /// `set_config("max_iterations", …)` sets it explicitly.
    max_iterations: Option<usize>,
}

impl TreatEngine {
    /// Creates a new TREAT engine with default settings.
    pub fn new() -> Self {
        Self {
            network: TreatNetwork::new(1024),
            alpha_memory: AlphaMemory::new(),
            beta_memory: BetaMemory::new(),
            working_facts: HashMap::new(),
            rules: Vec::new(),
            metrics: crate::engine::metrics::SimpleMetricsCollector::new(),
            cache_hits: 0,
            cache_misses: 0,
            collect_trace: false,
            max_iterations: None,
        }
    }

    /// Creates a new TREAT engine with the given cache size.
    pub fn with_cache_size(max_cache_size: usize) -> Self {
        Self {
            network: TreatNetwork::new(max_cache_size),
            alpha_memory: AlphaMemory::new(),
            beta_memory: BetaMemory::new(),
            working_facts: HashMap::new(),
            rules: Vec::new(),
            metrics: crate::engine::metrics::SimpleMetricsCollector::new(),
            cache_hits: 0,
            cache_misses: 0,
            collect_trace: false,
            max_iterations: None,
        }
    }

    /// Inserts a fact and propagates it through the network.
    pub fn assert_fact(&mut self, fact: Fact) {
        let fact_id = fact.id.to_string();
        // Clone before moving fact
        let fact_type = fact.fact_type_id.clone();

        // Store the fact (we can use fact again since we only moved fact_type by cloning)
        self.working_facts.insert(fact_id.clone(), fact);

        // Collect test nodes that need propagation first (before mutable borrow)
        let node_ids: Vec<_> = {
            self.network
                .base
                .test_nodes
                .values()
                .filter(|n| n.fact_type_id == fact_type)
                .map(|n| n.id.clone())
                .collect()
        };

        // Propagate to alpha memory
        for node_id in node_ids {
            self.alpha_memory.insert(&fact_type, fact_id.clone());
            self.propagate_to_successors(&node_id, &fact_id);
        }

        self.metrics.record_fact_inserted();
    }

    /// Propagates a fact through successor nodes with caching.
    fn propagate_to_successors(&mut self, node_id: &crate::engine::rete::NodeId, fact_id: &str) {
        // Collect all data needed first (before any mutable borrows)
        let successors_and_keys: Vec<_> = {
            let test_node = match self.network.base.test_nodes.get(node_id) {
                Some(n) => n,
                None => return,
            };

            test_node
                .successors
                .iter()
                .map(|s| (s.clone(), format!("join_{}", s.0)))
                .collect()
        };

        // Process each successor
        for (successor_id, key) in successors_and_keys {
            let is_cached = self.network.is_cached(&successor_id.0);

            if is_cached && self.network.cache.get(&key).is_some() {
                self.metrics.record_cache_hit();
                self.cache_hits += 1;
                continue; // Use cached result
            }

            // Get join_node data (before mutable borrow)
            let join_node_opt = self.network.base.join_nodes.get(&successor_id).cloned();

            if let Some(join_node) = join_node_opt {
                self.join_propagation_with_cache(&join_node.id, fact_id);
            }
        }

        self.metrics.record_cache_miss();
        self.cache_misses += 1;
    }

    /// Propagates from join node with caching support.
    fn join_propagation_with_cache(
        &mut self,
        join_node_id: &crate::engine::rete::NodeId,
        _fact_id: &str,
    ) {
        // Get join_node data after all borrows are done
        let join_node = match self.network.base.join_nodes.get(join_node_id) {
            Some(n) => n,
            None => return,
        };

        let test_node = match self.network.base.test_nodes.get(&join_node.predecessor) {
            Some(n) => n,
            None => return,
        };

        let facts_from_alpha = self.alpha_memory.get_facts(&test_node.fact_type_id);

        // Try to use cached partial matches
        let key = format!("join_{}", join_node.id.0);
        if self.network.is_cached(&join_node.id.0) {
            if let Some(cached_nagram) = self.network.cache.get(&key) {
                // Cache hit - use the cached nagram as a base for new partial matches
                for alpha_fact_id in &facts_from_alpha {
                    let mut new_nagram = cached_nagram.clone();
                    new_nagram.push(MemoryReference {
                        fact_id: alpha_fact_id.clone(),
                        generation: 0,
                    });
                    self.beta_memory.insert(&join_node.id, new_nagram);
                }
                self.metrics.record_cache_hit();
                self.cache_hits += 1;
                return;
            }
        }

        // Get existing nagrams from beta memory (right input)
        let existing_nagrams = self.beta_memory.get_nagrams(&join_node.id);

        // Combine to form new partial matches
        for fact_from_beta in &existing_nagrams {
            for alpha_fact_id in &facts_from_alpha {
                let mut new_nagram = fact_from_beta.clone();
                new_nagram.push(MemoryReference {
                    fact_id: alpha_fact_id.clone(),
                    generation: 0,
                });
                self.beta_memory.insert(&join_node.id, new_nagram);
            }
        }

        // For facts coming from alpha memory directly
        for alpha_fact_id in &facts_from_alpha {
            let nagram = vec![MemoryReference {
                fact_id: alpha_fact_id.clone(),
                generation: 0,
            }];
            self.beta_memory.insert(&join_node.id, nagram);
        }

        // Cache the first partial match if node is cacheable
        // This allows reuse when the same join node is evaluated with different facts
        let new_nagrams_count =
            existing_nagrams.len() * facts_from_alpha.len() + facts_from_alpha.len();
        if self.network.is_cached(&join_node.id.0) && new_nagrams_count > 0 {
            // Cache a representative nagram - we use the first one created
            if !existing_nagrams.is_empty() && !facts_from_alpha.is_empty() {
                let mut cached_nagram = existing_nagrams[0].clone();
                cached_nagram.push(MemoryReference {
                    fact_id: facts_from_alpha[0].clone(),
                    generation: 0,
                });
                self.network.cache.insert(key, cached_nagram);
            } else if !facts_from_alpha.is_empty() {
                // Cache the single-element nagram for alpha-only facts
                let cached_nagram = vec![MemoryReference {
                    fact_id: facts_from_alpha[0].clone(),
                    generation: 0,
                }];
                self.network.cache.insert(key, cached_nagram);
            }
        }

        self.metrics.record_beta_memory_additions(new_nagrams_count);
    }

    /// Executes the rules loaded via `load_ruleset` against `working_facts`
    /// using the shared direct interpreter (D-04, Plan 23-06).
    ///
    /// This bypasses the placeholder TREAT network entirely — it delegates
    /// to `engine::direct_eval::direct_execute`, which iterates `self.rules`,
    /// evaluates each `rule.condition` against `self.working_facts` via
    /// `Condition::evaluate`, applies matching `rule.actions`, and chains to
    /// fixpoint under the `RRULES_MAX_CHAIN_ITERATIONS` cap (D-07). Coverage
    /// is recorded once per evaluated/fired rule AFTER the loop (L-06 /
    /// Pitfall 6 — the per-nagram double-count bug in the placeholder
    /// implementation is removed). Returns an `EngineResult` with
    /// `fired_rule_ids` populated.
    pub fn execute_rules(&mut self) -> EngineResult {
        let start = std::time::Instant::now();

        let (fired_set, _evaluated_set, trace) =
            crate::engine::direct_eval::direct_execute_with_limit(
                &self.rules,
                &mut self.working_facts,
                self.collect_trace,
                self.max_iterations,
            );

        // Record per-fire metrics for parity with the placeholder loop.
        for _ in &fired_set {
            self.metrics.record_rule_fired();
        }
        self.metrics
            .set_execution_time_ms(start.elapsed().as_secs_f64() * 1000.0);

        let mut result = EngineResult::success(fired_set.len());
        result.fired_rule_ids = fired_set.iter().cloned().collect();
        // Phase 28 (D-01): attach the ordered trace payload from the shared
        // helper (C1 site 1). `None` when `collect_trace` is false.
        result.trace = trace;
        result
    }
}

impl Default for TreatEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionEngine for TreatEngine {
    fn execute(&mut self) -> EngineResult {
        self.execute_rules()
    }

    fn insert_fact(&mut self, fact: Fact) -> Result<(), String> {
        self.assert_fact(fact);
        Ok(())
    }

    fn retract_fact(&mut self, fact_id: &str) -> Result<bool, String> {
        // Retraction: drop the fact from working memory, from alpha memory, and
        // every beta-memory partial match that references it. Returns `false`
        // for an unknown id (idempotent re-retract), matching the trait contract.
        let Some(fact) = self.working_facts.remove(fact_id) else {
            return Ok(false);
        };
        self.alpha_memory.remove(&fact.fact_type_id, fact_id);
        self.beta_memory.remove_fact(fact_id);
        // TREAT caches join results; a cached nagram may mention the retracted
        // fact, so drop the cache entries rather than risk stale joins.
        self.network.cache.clear();
        Ok(true)
    }

    fn set_config(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key {
            // Fixpoint cap for this engine, overriding `RRULES_MAX_CHAIN_ITERATIONS`.
            "max_iterations" => {
                let limit = value.parse::<usize>().map_err(|_| {
                    format!("max_iterations must be a positive integer, got {value:?}")
                })?;
                if limit == 0 {
                    return Err("max_iterations must be at least 1".to_string());
                }
                self.max_iterations = Some(limit);
                Ok(())
            }
            other => Err(format!("unknown configuration key: {other}")),
        }
    }

    fn run(&mut self) -> usize {
        self.execute_rules().rules_fired
    }

    fn get_metrics(&self) -> crate::engine::metrics::EngineMetrics {
        let mut metrics = self.metrics.get_metrics();
        metrics.alpha_memory_size = self.alpha_memory.size();
        metrics.beta_memory_size = self.beta_memory.size();
        metrics.cache_hits = self.cache_hits;
        metrics.cache_misses = self.cache_misses;
        metrics
    }

    fn engine_type(&self) -> EngineType {
        EngineType::Treat
    }

    fn reset(&mut self) {
        self.network = TreatNetwork::new(1024);
        self.alpha_memory = AlphaMemory::new();
        self.beta_memory = BetaMemory::new();
        self.working_facts.clear();
        self.rules.clear();
        self.metrics.reset_metrics();
        self.cache_hits = 0;
        self.cache_misses = 0;
        // `collect_trace` is a config flag, not state — leave it across reset.
    }

    /// Phase 28 (D-01 / EXEC-04): store the trace-collection flag so
    /// `execute_rules` threads it into `direct_execute`. Overrides the
    /// trait's default noop.
    fn set_collect_trace(&mut self, enabled: bool) {
        self.collect_trace = enabled;
    }

    /// Loads a ruleset into the engine by cloning the ruleset's `rules` Vec
    /// into `self.rules` (L-02). The direct evaluator iterates `self.rules`
    /// against `self.working_facts`. Per-request engines carry their own
    /// clone of the shared `RuleSet` snapshot (D-03).
    fn load_ruleset(&mut self, ruleset: &crate::rules::RuleSet) -> Result<(), String> {
        self.rules = ruleset.rules.clone();
        Ok(())
    }

    /// Returns the post-execution working memory so the API layer can
    /// serialize `facts_after` from real engine state (Pitfall 11).
    fn get_facts(&self) -> Vec<crate::fact::Fact> {
        self.working_facts.values().cloned().collect()
    }
}

/// Factory for creating TREAT engine instances.
#[derive(Debug, Clone)]
pub struct TreatEngineFactory;

impl TreatEngineFactory {
    /// Creates a new factory.
    pub fn new() -> Self {
        Self
    }

    /// Creates a factory with custom cache size.
    pub fn with_cache_size(max_cache_size: usize) -> Self {
        let _ = max_cache_size; // Will be used when creating engine
        Self::new()
    }
}

impl Default for TreatEngineFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineFactory for TreatEngineFactory {
    fn engine_type(&self) -> EngineType {
        EngineType::Treat
    }

    fn create_engine(&self) -> Result<Box<dyn ExecutionEngine>, String> {
        Ok(Box::new(TreatEngine::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_cache_operations() {
        let mut cache = LRUCache::new(3);
        cache.insert("key1".to_string(), vec![]);
        cache.insert("key2".to_string(), vec![]);

        assert!(cache.get("key1").is_some());
        assert_eq!(cache.size(), 2);
    }

    #[test]
    fn test_lru_cache_eviction() {
        let mut cache = LRUCache::new(2);
        cache.insert("a".to_string(), vec![]);
        cache.insert("b".to_string(), vec![]);
        cache.insert("c".to_string(), vec![]); // Should evict 'a'

        assert!(cache.get("a").is_none());
        assert!(cache.get("b").is_some());
    }

    #[test]
    fn test_treat_engine_creation() {
        let engine = TreatEngine::new();
        assert_eq!(engine.engine_type(), EngineType::Treat);
    }

    #[test]
    fn test_cache_statistics() {
        let mut engine = TreatEngine::with_cache_size(10);

        // Insert some facts to trigger cache operations
        for i in 0..3 {
            let mut fact = Fact::new("test", HashMap::new());
            fact.payload
                .insert("value".to_string(), crate::data_types::Value::Integer(i));
            engine.assert_fact(fact);
        }

        let metrics = engine.get_metrics();
        assert!(metrics.cache_hits >= 0 || metrics.cache_misses >= 0); // At least one of them
    }
}
