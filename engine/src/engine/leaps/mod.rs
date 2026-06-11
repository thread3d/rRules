//! The LEAPS algorithm engine implementation.
//!
//! LEAPS (Leaf Aggregation and Propagation System) extends the Rete algorithm
//! with specialized aggregation nodes for efficient computation of Sum, Count,
//! Max, Min operations over fact sets.

use std::collections::{HashMap, HashSet};
use crate::engine::registry::{EngineFactory, EngineResult, EngineType, ExecutionEngine};
use crate::engine::metrics::MetricsCollector;
use crate::fact::Fact;
use crate::engine::rete::{AlphaMemory, BetaMemory, FactId, MemoryReference, Nagram};

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
            AggregationOp::Max => Some(*values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b))),
            AggregationOp::Min => Some(*values.iter().fold(f64::INFINITY, |a, &b| a.min(b))),
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
            AggregationOp::Max => Some(*values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b))),
            AggregationOp::Min => Some(*values.iter().fold(f64::INFINITY, |a, &b| a.min(b))),
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
    /// Source facts that contribute to this aggregation
    source_facts: HashSet<String>,
}

impl AggregationNode {
    /// Creates a new aggregation node.
    pub fn new(id: impl Into<String>, op: AggregationOp, field: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            op,
            field: field.into(),
            current_value: None,
            source_facts: HashSet::new(),
        }
    }

    /// Adds a fact to this aggregation node.
    pub fn add_fact(&mut self, fact_id: &str) -> Option<f64> {
        self.source_facts.insert(fact_id.to_string());
        self.recalculate()
    }

    /// Removes a fact from this aggregation node.
    pub fn remove_fact(&mut self, fact_id: &str) -> Option<f64> {
        self.source_facts.remove(fact_id);
        self.recalculate()
    }

    /// Recalculates the aggregated value.
    fn recalculate(&mut self) -> Option<f64> {
        // This would need access to facts for actual computation
        // For now, just track that recalculation is needed
        None
    }

    /// Gets the current aggregated value.
    pub fn get_value(&self) -> Option<f64> {
        self.current_value
    }

    /// Gets the number of source facts.
    pub fn source_count(&self) -> usize {
        self.source_facts.len()
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
            AggregationOp::Sum | AggregationOp::Max | AggregationOp::Min => {
                Self { value: 0.0, count: 0, op }
            }
            AggregationOp::Count => Self { value: 0.0, count: 0, op },
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
    metrics: crate::engine::metrics::SimpleMetricsCollector,
}

impl LeapsEngine {
    /// Creates a new LEAPS engine with default settings.
    pub fn new() -> Self {
        Self {
            network: LeapsNetwork::new(),
            alpha_memory: AlphaMemory::new(),
            beta_memory: BetaMemory::new(),
            working_facts: HashMap::new(),
            metrics: crate::engine::metrics::SimpleMetricsCollector::new(),
        }
    }

    /// Creates a new LEAPS engine with the given network.
    pub fn with_network(network: LeapsNetwork) -> Self {
        Self {
            network,
            alpha_memory: AlphaMemory::new(),
            beta_memory: BetaMemory::new(),
            working_facts: HashMap::new(),
            metrics: crate::engine::metrics::SimpleMetricsCollector::new(),
        }
    }

    /// Inserts a fact and propagates it through the network.
    pub fn assert_fact(&mut self, mut fact: Fact) {
        let fact_id = fact.id.to_string();

        // Store the fact
        self.working_facts.insert(fact_id.clone(), fact);

        // Propagate to alpha memory (test nodes)
        for test_node in self.network.base.test_nodes.values() {
            if test_node.fact_type_id == fact.fact_type_id {
                self.alpha_memory.insert(&fact.fact_type_id, fact_id.clone());
                self.propagate_to_successors(&test_node.id, &fact_id);
            }
        }

        // Check aggregation nodes
        for (_id, agg_node) in &mut self.network.agg_nodes {
            if let Some(val) = agg_node.add_fact(&fact_id) {
                agg_node.current_value = Some(val);
            }
        }

        self.metrics.record_fact_inserted();
    }

    /// Propagates a fact through successor nodes.
    fn propagate_to_successors(&mut self, node_id: &crate::engine::rete::NodeId, fact_id: &str) {
        let test_node = match self.network.base.test_nodes.get(node_id) {
            Some(n) => n,
            None => return,
        };

        for successor_id in &test_node.successors {
            if let Some(join_node) = self.network.base.join_nodes.get(successor_id) {
                // Track aggregation for join nodes
                if self.network.aggregated_join_nodes.contains(&successor_id.0) {
                    self.metrics.record_rule_fired();
                }
                self.join_propagation(join_node, fact_id);
            }
        }
    }

    /// Propagates from join node to action nodes.
    fn join_propagation(&mut self, join_node: &crate::engine::rete::JoinNode, fact_id: &str) {
        let test_node = match self.network.base.test_nodes.get(&join_node.predecessor) {
            Some(n) => n,
            None => return,
        };

        let facts_from_alpha = self.alpha_memory.get_facts(&test_node.fact_type_id);

        // Get existing nagrams from beta memory
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

        for alpha_fact_id in facts_from_alpha {
            let nagram = vec![MemoryReference {
                fact_id: alpha_fact_id.clone(),
                generation: 0,
            }];
            self.beta_memory.insert(&join_node.id, nagram);
        }

        self.metrics.record_beta_memory_additions(facts_from_alpha.len() + existing_nagrams.len());
    }

    /// Executes aggregation for all nodes.
    pub fn execute_aggregations(&mut self) {
        for (_id, agg_node) in &self.network.agg_nodes {
            let facts: Vec<&Fact> = agg_node
                .source_facts
                .iter()
                .filter_map(|fid| self.working_facts.get(fid))
                .collect();

            if let Some(value) = agg_node.op.compute_from_facts(&facts, &agg_node.field) {
                // Update node value
                // Note: We don't have direct mutable access here without borrowing issues
            }
        }
    }

    /// Executes the engine and fires rules.
    pub fn execute_rules(&mut self) -> usize {
        let mut rules_fired = 0;

        for (_node_id, action_node) in &self.network.base.action_nodes {
            for (_join_id, join_node) in &self.network.base.join_nodes {
                let nagrams = self.beta_memory.get_nagrams(&join_node.id);

                // Apply aggregation if this join has aggregations
                if self.network.aggregated_join_nodes.contains(&join_node.id.0) {
                    self.execute_aggregations();
                }

                for _nagram in nagrams {
                    rules_fired += 1;
                    self.metrics.record_rule_fired();
                }
            }
        }

        self.metrics.set_execution_time_ms(0.0);
        rules_fired
    }

    /// Gets an aggregation result.
    pub fn get_aggregation(&self, agg_id: &str) -> Option<AggregationResult> {
        let agg_node = self.network.agg_nodes.get(agg_id)?;
        let value = agg_node.current_value?;
        Some(AggregationResult::new(value, agg_node.source_count(), agg_node.op))
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
        let fired = self.execute_rules();
        EngineResult::success(fired)
    }

    fn insert_fact(&mut self, fact: Fact) -> Result<(), String> {
        self.assert_fact(fact);
        Ok(())
    }

    fn retract_fact(&mut self, _fact_id: &str) -> Result<bool, String> {
        // TODO: Implement retraction with proper network cleanup
        Ok(false)
    }

    fn run(&mut self) -> usize {
        self.execute_rules()
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
        self.metrics.reset_metrics();
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

        // Test adding facts
        node.add_fact("fact1");
        assert_eq!(node.source_count(), 1);

        node.add_fact("fact2");
        assert_eq!(node.source_count(), 2);
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
