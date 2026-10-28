//! The LEAPS algorithm engine implementation.
//!
//! LEAPS (Leaf Aggregation and Propagation System) extends the Rete algorithm
//! with specialized aggregation nodes for efficient computation of Sum, Count,
//! Max, Min operations over fact sets.

use crate::engine::metrics::MetricsCollector;
use crate::engine::registry::{EngineFactory, EngineResult, EngineType, ExecutionEngine};
use crate::engine::rete::{AlphaMemory, BetaMemory, FactId, MemoryReference};
use crate::fact::Fact;
use std::collections::{HashMap, HashSet};

// NOTE: `CoverageTracking` is no longer imported here — the shared direct
// evaluator (`engine::direct_eval`) owns once-per-rule coverage recording
// after the fixpoint loop (L-06 / Pitfall 6), so the placeholder per-nagram
// `record_rule_eval`/`record_rule_fire` calls in the old `execute_rules`
// are gone.

/// Aggregation operations supported by LEAPS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AggregationOp {
    /// Sum of values
    Sum,
    /// Count of facts
    Count,
    /// Maximum value
    Max,
    /// Minimum value
    Min,
}

/// The value of `field` in `fact`, when it is a number that can be aggregated.
fn numeric_field(fact: &Fact, field: &str) -> Option<f64> {
    match fact.payload.get(field)? {
        crate::data_types::Value::Integer(value) => Some(*value as f64),
        crate::data_types::Value::Float(value) => Some(*value),
        crate::data_types::Value::Number(value) => value.to_string().parse::<f64>().ok(),
        _ => None,
    }
}

impl AggregationOp {
    /// Returns the display name for this operation.
    pub fn name(&self) -> &'static str {
        match self {
            AggregationOp::Sum => "sum",
            AggregationOp::Count => "count",
            AggregationOp::Max => "max",
            AggregationOp::Min => "min",
        }
    }

    /// Computes the aggregation result for a set of values.
    pub fn compute(&self, values: &[f64]) -> Option<f64> {
        if values.is_empty() {
            return None;
        }

        match self {
            AggregationOp::Sum => Some(values.iter().sum()),
            AggregationOp::Count => Some(values.len() as f64),
            AggregationOp::Max => Some(values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b))),
            AggregationOp::Min => Some(values.iter().fold(f64::INFINITY, |a, &b| a.min(b))),
        }
    }

    /// Computes aggregation result from a set of facts.
    pub fn compute_from_facts(&self, facts: &[&Fact], field: &str) -> Option<f64> {
        let values: Vec<f64> = facts
            .iter()
            .filter_map(|f| {
                if let Some(val) = f.payload.get(field) {
                    match val {
                        crate::data_types::Value::Integer(i) => Some(*i as f64),
                        crate::data_types::Value::Float(f) => Some(*f),
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .collect();

        if values.is_empty() {
            return None;
        }

        match self {
            AggregationOp::Sum => Some(values.iter().sum()),
            AggregationOp::Count => Some(values.len() as f64),
            AggregationOp::Max => Some(values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b))),
            AggregationOp::Min => Some(values.iter().fold(f64::INFINITY, |a, &b| a.min(b))),
        }
    }
}

impl std::fmt::Display for AggregationOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// An aggregation node computes aggregations over fact sets.
#[derive(Debug)]
pub struct AggregationNode {
    /// Unique identifier for this node
    pub id: String,
    /// Operation to perform
    pub op: AggregationOp,
    /// Field to aggregate
    pub field: String,
    /// Current aggregated value
    pub current_value: Option<f64>,
    /// The value each contributing fact supplies, by fact id.
    ///
    /// The ids alone were useless: `recalculate` had nothing to compute from, so it returned `None`
    /// and the node only ever counted names — which is what "aggregations never compute" looked like.
    source_values: HashMap<String, f64>,
}

impl AggregationNode {
    /// Creates a new aggregation node.
    pub fn new(id: impl Into<String>, op: AggregationOp, field: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            op,
            field: field.into(),
            current_value: None,
            source_values: HashMap::new(),
        }
    }

    /// Adds a fact's contribution and returns the recomputed aggregate.
    ///
    /// A fact whose payload lacks the aggregated field — or carries it as something that is not a
    /// number — contributes nothing and is not tracked. That is also what stops the network feeding
    /// every fact to every node: only facts that can move the number are members.
    pub fn add_fact(&mut self, fact: &Fact) -> Option<f64> {
        let value = numeric_field(fact, &self.field)?;
        self.source_values.insert(fact.id.to_string(), value);
        self.recalculate()
    }

    /// Removes a fact's contribution and returns the recomputed aggregate.
    pub fn remove_fact(&mut self, fact_id: &str) -> Option<f64> {
        self.source_values.remove(fact_id);
        self.recalculate()
    }

    /// Recomputes the aggregate from the contributions held.
    fn recalculate(&mut self) -> Option<f64> {
        let values: Vec<f64> = self.source_values.values().copied().collect();
        self.current_value = self.op.compute(&values);
        self.current_value
    }

    /// Forgets every contributing fact and the cached aggregate.
    ///
    /// Used on retraction: the cached value was derived from a working-memory
    /// snapshot that just changed, and this stub implementation cannot recompute
    /// incrementally, so the node starts over instead of serving a stale number.
    pub fn reset(&mut self) {
        self.source_values.clear();
        self.current_value = None;
    }

    /// Gets the current aggregated value.
    pub fn get_value(&self) -> Option<f64> {
        self.current_value
    }

    /// Gets the number of source facts.
    pub fn source_count(&self) -> usize {
        self.source_values.len()
    }
}

/// Aggregation result with metadata.
#[derive(Debug, Clone)]
pub struct AggregationResult {
    /// The computed value
    pub value: f64,
    /// Number of facts used in computation
    pub count: usize,
    /// Operation performed
    pub op: AggregationOp,
}

impl AggregationResult {
    pub fn new(value: f64, count: usize, op: AggregationOp) -> Self {
        Self { value, count, op }
    }

    /// Creates an empty result (no facts).
    pub fn empty(op: AggregationOp) -> Self {
        match op {
            AggregationOp::Sum | AggregationOp::Max | AggregationOp::Min => Self {
                value: 0.0,
                count: 0,
                op,
            },
            AggregationOp::Count => Self {
                value: 0.0,
                count: 0,
                op,
            },
        }
    }
}

/// LEAPS network extends Rete with aggregation nodes.
#[derive(Debug)]
pub struct LeapsNetwork {
    /// Base Rete network
    base: crate::engine::rete::ReteNetwork,
    /// Aggregation nodes in the network
    pub agg_nodes: HashMap<String, AggregationNode>,
    /// Track which join nodes have aggregations downstream
    aggregated_join_nodes: HashSet<String>,
}

impl LeapsNetwork {
    pub fn new() -> Self {
        Self {
            base: crate::engine::rete::ReteNetwork::new(),
            agg_nodes: HashMap::new(),
            aggregated_join_nodes: HashSet::new(),
        }
    }

    /// Adds an aggregation node to the network.
    pub fn add_agg_node(&mut self, node: AggregationNode) {
        self.agg_nodes.insert(node.id.clone(), node);
    }

    /// Marks a join node as having aggregations downstream.
    pub fn mark_join_with_aggregation(&mut self, node_id: impl Into<String>) {
        self.aggregated_join_nodes.insert(node_id.into());
    }
}

impl Default for LeapsNetwork {
    fn default() -> Self {
        Self::new()
    }
}

/// The LEAPS algorithm engine implementation.
#[derive(Debug)]
pub struct LeapsEngine {
    network: LeapsNetwork,
    alpha_memory: AlphaMemory,
    beta_memory: BetaMemory,
    working_facts: HashMap<FactId, Fact>,
    /// Rules loaded via `load_ruleset` (L-02). The direct evaluator
    /// (`execute_rules`, Plan 23-06) iterates this Vec instead of the
    /// placeholder LEAPS network. Populated by cloning from a shared
    /// `RuleSet` snapshot; per-request engines carry their own copy.
    rules: Vec<crate::rules::Rule>,
    metrics: crate::engine::metrics::SimpleMetricsCollector,
    /// Phase 28 (D-01 / EXEC-04): trace-collection flag. Set via
    /// `set_collect_trace` before `execute()`; threaded into `direct_execute`.
    /// Default `false` (SC-3 latency neutrality — off-by-default).
    collect_trace: bool,
    /// Fixpoint cap for this engine. `None` defers to the
    /// `RRULES_MAX_CHAIN_ITERATIONS` environment variable (default 1000);
    /// `set_config("max_iterations", …)` sets it explicitly.
    max_iterations: Option<usize>,
}

impl LeapsEngine {
    /// Creates a new LEAPS engine with default settings.
    pub fn new() -> Self {
        Self {
            network: LeapsNetwork::new(),
            alpha_memory: AlphaMemory::new(),
            beta_memory: BetaMemory::new(),
            working_facts: HashMap::new(),
            rules: Vec::new(),
            metrics: crate::engine::metrics::SimpleMetricsCollector::new(),
            collect_trace: false,
            max_iterations: None,
        }
    }

    /// Creates a new LEAPS engine with the given network.
    pub fn with_network(network: LeapsNetwork) -> Self {
        Self {
            network,
            alpha_memory: AlphaMemory::new(),
            beta_memory: BetaMemory::new(),
            working_facts: HashMap::new(),
            rules: Vec::new(),
            metrics: crate::engine::metrics::SimpleMetricsCollector::new(),
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

        // Propagate to alpha memory (test nodes) - collect node IDs first
        let node_ids: Vec<_> = {
            self.network
                .base
                .test_nodes
                .values()
                .filter(|n| n.fact_type_id == fact_type)
                .map(|n| n.id.clone())
                .collect()
        };

        for node_id in node_ids {
            self.alpha_memory.insert(&fact_type, fact_id.clone());
            self.propagate_to_successors(&node_id, &fact_id);
        }

        // Feed the aggregation nodes: each takes the fact's contribution and recomputes.
        if let Some(fact) = self.working_facts.get(&fact_id) {
            let contributions: Vec<Option<f64>> = self
                .network
                .agg_nodes
                .values_mut()
                .map(|node| node.add_fact(fact))
                .collect();
            let _ = contributions;
        }

        self.metrics.record_fact_inserted();
    }

    /// Propagates a fact through successor nodes.
    fn propagate_to_successors(&mut self, node_id: &crate::engine::rete::NodeId, fact_id: &str) {
        // Collect all successor data first (before any mutable borrow)
        let successors_data: Vec<_> = {
            let test_node = match self.network.base.test_nodes.get(node_id) {
                Some(n) => n,
                None => return,
            };

            test_node
                .successors
                .iter()
                .filter_map(|s| {
                    self.network.base.join_nodes.get(s).map(|join_node| {
                        (
                            join_node.id.clone(),
                            join_node.predecessor.clone(),
                            s.0.clone(),
                        )
                    })
                })
                .collect()
        };

        for (join_id, predecessor_id, successor_key) in successors_data {
            // Track aggregation for join nodes
            if self.network.aggregated_join_nodes.contains(&successor_key) {
                self.metrics.record_rule_fired();
            }
            self.join_propagation_with_predecessor(&join_id, &predecessor_id, fact_id);
        }
    }

    /// Propagates from join node with predecessor info.
    fn join_propagation_with_predecessor(
        &mut self,
        join_node_id: &crate::engine::rete::NodeId,
        predecessor_id: &crate::engine::rete::NodeId,
        _fact_id: &str,
    ) {
        // Get all data before mutation
        let (test_facts_from_alpha, existing_nagrams) = {
            let facts_from_alpha = self.alpha_memory.get_facts(&predecessor_id.0);
            let existing_nagrams = self.beta_memory.get_nagrams(join_node_id);
            (facts_from_alpha, existing_nagrams)
        };

        // Combine to form new partial matches
        for fact_from_beta in &existing_nagrams {
            for alpha_fact_id in &test_facts_from_alpha {
                let mut new_nagram = fact_from_beta.clone();
                new_nagram.push(MemoryReference {
                    fact_id: alpha_fact_id.clone(),
                    generation: 0,
                });
                self.beta_memory.insert(join_node_id, new_nagram);
            }
        }

        for alpha_fact_id in &test_facts_from_alpha {
            let nagram = vec![MemoryReference {
                fact_id: alpha_fact_id.clone(),
                generation: 0,
            }];
            self.beta_memory.insert(join_node_id, nagram);
        }

        self.metrics
            .record_beta_memory_additions(test_facts_from_alpha.len() + existing_nagrams.len());
    }

    /// Recompute every aggregation node from working memory.
    ///
    /// The nodes already recompute as facts arrive and leave; this recomputes them from the facts the
    /// engine holds *now*, which is what a caller wants after a bulk change and what a stale cache
    /// would get wrong.
    pub fn execute_aggregations(&mut self) {
        let facts: Vec<Fact> = self.working_facts.values().cloned().collect();
        for agg_node in self.network.agg_nodes.values_mut() {
            agg_node.reset();
            for fact in &facts {
                agg_node.add_fact(fact);
            }
        }
    }

    /// Executes the rules loaded via `load_ruleset` against `working_facts`
    /// using the shared direct interpreter (D-04, Plan 23-06).
    ///
    /// This bypasses the placeholder LEAPS network entirely — it delegates
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

    /// Gets an aggregation result.
    pub fn get_aggregation(&self, agg_id: &str) -> Option<AggregationResult> {
        let agg_node = self.network.agg_nodes.get(agg_id)?;
        let value = agg_node.current_value?;
        Some(AggregationResult::new(
            value,
            agg_node.source_count(),
            agg_node.op,
        ))
    }

    /// Computes an aggregation from facts directly.
    pub fn compute_aggregation(
        &self,
        fact_type: &str,
        op: AggregationOp,
        field: &str,
    ) -> Option<AggregationResult> {
        let facts: Vec<&Fact> = self
            .working_facts
            .values()
            .filter(|f| f.fact_type_id == fact_type)
            .collect();

        let value = op.compute_from_facts(&facts, field)?;
        Some(AggregationResult::new(value, facts.len(), op))
    }
}

impl Default for LeapsEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionEngine for LeapsEngine {
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
        // LEAPS aggregation nodes cache values derived from source facts;
        // recompute from scratch after a retraction instead of serving a
        // stale aggregate.
        for agg in self.network.agg_nodes.values_mut() {
            agg.reset();
        }
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
        metrics
    }

    fn engine_type(&self) -> EngineType {
        EngineType::Leaps
    }

    fn reset(&mut self) {
        self.network = LeapsNetwork::new();
        self.alpha_memory = AlphaMemory::new();
        self.beta_memory = BetaMemory::new();
        self.working_facts.clear();
        self.rules.clear();
        self.metrics.reset_metrics();
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

/// Factory for creating LEAPS engine instances.
#[derive(Debug, Clone)]
pub struct LeapsEngineFactory;

impl LeapsEngineFactory {
    /// Creates a new factory.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LeapsEngineFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineFactory for LeapsEngineFactory {
    fn engine_type(&self) -> EngineType {
        EngineType::Leaps
    }

    fn create_engine(&self) -> Result<Box<dyn ExecutionEngine>, String> {
        Ok(Box::new(LeapsEngine::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agg_op_names() {
        assert_eq!(AggregationOp::Sum.name(), "sum");
        assert_eq!(AggregationOp::Count.name(), "count");
        assert_eq!(AggregationOp::Max.name(), "max");
        assert_eq!(AggregationOp::Min.name(), "min");
    }

    #[test]
    fn test_agg_op_compute() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        assert_eq!(AggregationOp::Sum.compute(&values), Some(15.0));
        assert_eq!(AggregationOp::Count.compute(&values), Some(5.0));
        assert_eq!(AggregationOp::Max.compute(&values), Some(5.0));
        assert_eq!(AggregationOp::Min.compute(&values), Some(1.0));
    }

    #[test]
    fn test_agg_op_empty_values() {
        let values: Vec<f64> = vec![];

        assert_eq!(AggregationOp::Sum.compute(&values), None);
        assert_eq!(AggregationOp::Count.compute(&values), None);
    }

    #[test]
    fn test_leaps_engine_creation() {
        let engine = LeapsEngine::new();
        assert_eq!(engine.engine_type(), EngineType::Leaps);
    }

    #[test]
    fn test_aggregation_node_operations() {
        let mut node = AggregationNode::new("agg1", AggregationOp::Sum, "amount");

        let fact = |id: u128, amount: i64| {
            let mut payload = HashMap::new();
            payload.insert(
                "amount".to_string(),
                crate::data_types::Value::Integer(amount),
            );
            Fact::with_id(uuid::Uuid::from_u128(id), "order", payload)
        };

        // The node recomputes as contributions arrive…
        assert_eq!(node.add_fact(&fact(1, 10)), Some(10.0));
        assert_eq!(node.source_count(), 1);
        assert_eq!(node.add_fact(&fact(2, 25)), Some(35.0));
        assert_eq!(node.source_count(), 2);

        // …and on removal.
        assert_eq!(
            node.remove_fact(&uuid::Uuid::from_u128(1).to_string()),
            Some(25.0)
        );
        assert_eq!(node.source_count(), 1);
        assert_eq!(
            node.remove_fact(&uuid::Uuid::from_u128(2).to_string()),
            None
        );
        assert_eq!(node.source_count(), 0);
    }

    #[test]
    fn a_fact_without_the_aggregated_field_is_not_a_member() {
        let mut node = AggregationNode::new("agg1", AggregationOp::Count, "amount");
        let mut payload = HashMap::new();
        payload.insert("other".to_string(), crate::data_types::Value::Integer(1));
        let unrelated = Fact::with_id(uuid::Uuid::from_u128(9), "order", payload);
        assert_eq!(node.add_fact(&unrelated), None);
        assert_eq!(
            node.source_count(),
            0,
            "only contributing facts are members"
        );
    }

    #[test]
    fn the_engine_feeds_its_aggregation_nodes() {
        // The node the network holds must see the facts the engine asserts — feeding ids alone was
        // what left every aggregation empty.
        let mut engine = LeapsEngine::new();
        engine.network.add_agg_node(AggregationNode::new(
            "sum_amounts",
            AggregationOp::Sum,
            "amount",
        ));
        for amount in [10, 20, 30] {
            let mut payload = HashMap::new();
            payload.insert(
                "amount".to_string(),
                crate::data_types::Value::Integer(amount),
            );
            engine.assert_fact(Fact::new("order", payload));
        }

        let node = engine
            .network
            .agg_nodes
            .get("sum_amounts")
            .expect("the node is in the network");
        assert_eq!(node.source_count(), 3);
        assert_eq!(node.get_value(), Some(60.0), "10 + 20 + 30");
    }

    #[test]
    fn test_compute_aggregation() {
        let mut engine = LeapsEngine::new();

        // Add some facts
        for i in 0..5 {
            let mut fact = Fact::new("order", HashMap::new());
            fact.payload.insert(
                "amount".to_string(),
                crate::data_types::Value::Integer((i + 1) * 10),
            );
            engine.assert_fact(fact);
        }

        // Compute aggregation
        if let Some(result) = engine.compute_aggregation("order", AggregationOp::Sum, "amount") {
            assert_eq!(result.value, 150.0); // 10+20+30+40+50
            assert_eq!(result.count, 5);
        }

        if let Some(result) = engine.compute_aggregation("order", AggregationOp::Count, "amount") {
            assert_eq!(result.value, 5.0);
        }
    }
}
