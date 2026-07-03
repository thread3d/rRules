//! The Rete algorithm engine implementation.
//!
//! This module implements the classic Rete algorithm with:
//! - Party tokens (alpha nodes) for fact type matching
//! - Beta memory for join node partial matches
//! - Network propagation for efficient pattern matching

use crate::engine::registry::{EngineFactory, EngineResult, EngineType, ExecutionEngine};
use crate::engine::metrics::MetricsCollector;
use crate::fact::Fact;
use std::collections::{HashMap, HashSet};

use crate::monitoring::coverage::CoverageTracking;

/// A unique identifier for a fact in working memory.
pub type FactId = String;

/// Reference to a fact stored in beta memory (nagram).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemoryReference {
    pub fact_id: FactId,
    pub generation: usize,
}

/// A nagram is a sequence of memory references forming a partial match.
pub type Nagram = Vec<MemoryReference>;

/// Node identifier for the Rete network.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(pub String);

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        NodeId(s.to_string())
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        NodeId(s)
    }
}

/// Alpha memory stores fact type activations.
#[derive(Debug, Clone, Default)]
pub struct AlphaMemory {
    /// Fact type ID -> list of facts
    entries: HashMap<String, HashSet<FactId>>,
}

impl AlphaMemory {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, fact_type_id: &str, fact_id: FactId) {
        self.entries
            .entry(fact_type_id.to_string())
            .or_default()
            .insert(fact_id);
    }

    pub fn remove(&mut self, fact_type_id: &str, fact_id: &str) {
        if let Some(set) = self.entries.get_mut(fact_type_id) {
            set.remove(fact_id);
            if set.is_empty() {
                self.entries.remove(fact_type_id);
            }
        }
    }

    pub fn get_facts(&self, fact_type_id: &str) -> Vec<String> {
        self.entries
            .get(fact_type_id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn size(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }
}

/// Beta memory stores partial matches (nagrams).
#[derive(Debug, Clone, Default)]
pub struct BetaMemory {
    /// Join node ID -> list of nagrams
    entries: HashMap<NodeId, Vec<Nagram>>,
}

impl BetaMemory {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, join_node_id: &NodeId, nagram: Nagram) {
        self.entries.entry(join_node_id.clone()).or_default().push(nagram);
    }

    pub fn get_nagrams(&self, join_node_id: &NodeId) -> Vec<Nagram> {
        self.entries.get(join_node_id).cloned().unwrap_or_default()
    }

    pub fn remove_for_node(&mut self, join_node_id: &NodeId) {
        self.entries.remove(join_node_id);
    }

    pub fn size(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }
}

/// Test node (alpha node) that matches fact types.
#[derive(Debug, Clone)]
pub struct TestNode {
    pub id: NodeId,
    pub fact_type_id: String,
    pub successors: Vec<NodeId>,
}

impl TestNode {
    pub fn new(id: impl Into<String>, fact_type_id: impl Into<String>) -> Self {
        Self {
            id: NodeId(id.into()),
            fact_type_id: fact_type_id.into(),
            successors: Vec::new(),
        }
    }
}

/// Join node (beta node) that matches multiple conditions.
#[derive(Debug, Clone)]
pub struct JoinNode {
    pub id: NodeId,
    pub predecessor: NodeId, // Left input from alpha memory
    pub condition_node: NodeId, // Right input for join condition
}

impl JoinNode {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: NodeId(id.into()),
            predecessor: NodeId("placeholder".to_string()),
            condition_node: NodeId("placeholder".to_string()),
        }
    }
}

/// Action node (producction) that fires rules.
#[derive(Debug)]
pub struct ActionNode {
    pub id: NodeId,
    pub rule_id: String,
}

impl ActionNode {
    pub fn new(id: impl Into<String>, rule_id: impl Into<String>) -> Self {
        Self {
            id: NodeId(id.into()),
            rule_id: rule_id.into(),
        }
    }
}

/// The Rete network structure.
#[derive(Debug, Default)]
pub struct ReteNetwork {
    /// Test nodes (alpha memory)
    pub test_nodes: HashMap<NodeId, TestNode>,
    /// Join nodes (beta memory)
    pub join_nodes: HashMap<NodeId, JoinNode>,
    /// Action nodes
    pub action_nodes: HashMap<NodeId, ActionNode>,
}

impl ReteNetwork {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_test_node(&mut self, node: TestNode) {
        self.test_nodes.insert(node.id.clone(), node);
    }

    pub fn add_join_node(&mut self, node: JoinNode) {
        self.join_nodes.insert(node.id.clone(), node);
    }

    pub fn add_action_node(&mut self, node: ActionNode) {
        self.action_nodes.insert(node.id.clone(), node);
    }
}

/// The Rete algorithm engine implementation.
#[derive(Debug)]
pub struct ReteEngine {
    network: ReteNetwork,
    alpha_memory: AlphaMemory,
    beta_memory: BetaMemory,
    working_facts: HashMap<FactId, Fact>,
    metrics: crate::engine::metrics::SimpleMetricsCollector,
}

impl ReteEngine {
    /// Creates a new Rete engine with empty network.
    pub fn new() -> Self {
        Self {
            network: ReteNetwork::new(),
            alpha_memory: AlphaMemory::new(),
            beta_memory: BetaMemory::new(),
            working_facts: HashMap::new(),
            metrics: crate::engine::metrics::SimpleMetricsCollector::new(),
        }
    }

    /// Creates a new engine with the given network.
    pub fn with_network(network: ReteNetwork) -> Self {
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
        let fact_type = fact.fact_type_id.clone();

        // Store the fact
        self.working_facts.insert(fact_id.clone(), fact);

        // Collect test nodes that need propagation first (before mutable borrow)
        let node_ids_to_propagate: Vec<_> = self.network.test_nodes.values()
            .filter(|n| n.fact_type_id == fact_type)
            .map(|n| n.id.clone())
            .collect();

        // Propagate to alpha memory
        for node_id in node_ids_to_propagate {
            self.alpha_memory.insert(&fact_type, fact_id.clone());
            self.propagate_to_successors(&node_id, &fact_id);
        }

        self.metrics.record_fact_inserted();
    }

    /// Propagates a fact through successor nodes.
    fn propagate_to_successors(&mut self, node_id: &NodeId, fact_id: &str) {
        // Get successors first
        let join_nodes: Vec<_> = {
            let test_node = match self.network.test_nodes.get(node_id) {
                Some(n) => n,
                None => return,
            };
            test_node.successors.iter()
                .filter_map(|s| self.network.join_nodes.get(s).cloned())
                .collect()
        };

        for join_node in join_nodes {
            self.join_propagation(&join_node, fact_id);
        }
    }

    /// Propagates from join node to action nodes.
    fn join_propagation(&mut self, join_node: &JoinNode, _fact_id: &str) {
        // Get partial matches from left input (alpha memory)
        let test_node = self
            .network
            .test_nodes
            .get(&join_node.predecessor);

        if test_node.is_none() {
            return;
        }

        let facts_from_alpha = self.alpha_memory.get_facts(&test_node.unwrap().fact_type_id);
        let existing_nagrams = self.beta_memory.get_nagrams(&join_node.id);
        let alpha_count = facts_from_alpha.len();

        // Record evaluation for the rule (joined from this join node)
        if let Some(action_node) = self.network.action_nodes.values().find(|n| {
            // Find action nodes that connect to this join node
            true // Simplified - in real implementation would track rule_id -> join_node mapping
        }) {
            CoverageTracking::record_rule_eval(&action_node.rule_id);
        }

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

        self.metrics.record_beta_memory_additions(alpha_count + existing_nagrams.len());
    }

    /// Executes the engine and fires rules.
    pub fn execute_rules(&mut self) -> usize {
        let mut rules_fired = 0;

        // For each action node, check if there are complete matches
        for (_node_id, action_node) in &self.network.action_nodes {
            // Check if there are partial matches for this rule's join nodes
            for (_join_id, _join_node) in &self.network.join_nodes {
                let nagrams = self.beta_memory.get_nagrams(&_join_node.id);

                for _nagram in &nagrams {
                    // Record fire for this rule when it fires
                    CoverageTracking::record_rule_fire(&action_node.rule_id);
                    rules_fired += 1;
                    self.metrics.record_rule_fired();
                }
            }
        }

        self.metrics.set_execution_time_ms(0.0);
        rules_fired
    }
}

impl Default for ReteEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionEngine for ReteEngine {
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
        EngineType::Rete
    }

    fn reset(&mut self) {
        self.network = ReteNetwork::new();
        self.alpha_memory = AlphaMemory::new();
        self.beta_memory = BetaMemory::new();
        self.working_facts.clear();
        self.metrics.reset_metrics();
    }
}

/// Factory for creating Rete engine instances.
#[derive(Debug, Clone)]
pub struct ReteEngineFactory;

impl ReteEngineFactory {
    /// Creates a new factory.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReteEngineFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineFactory for ReteEngineFactory {
    fn engine_type(&self) -> EngineType {
        EngineType::Rete
    }

    fn create_engine(&self) -> Result<Box<dyn ExecutionEngine>, String> {
        Ok(Box::new(ReteEngine::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rte_engine_creation() {
        let engine = ReteEngine::new();
        assert_eq!(engine.engine_type(), EngineType::Rete);
    }

    #[test]
    fn test_alpha_memory_operations() {
        let mut alpha = AlphaMemory::new();
        alpha.insert("person", "fact1".to_string());
        alpha.insert("person", "fact2".to_string());

        let facts = alpha.get_facts("person");
        assert_eq!(facts.len(), 2);
    }

    #[test]
    fn test_beta_memory_operations() {
        let mut beta = BetaMemory::new();
        let node_id = NodeId("join1".to_string());
        let nagram = vec![MemoryReference {
            fact_id: "fact1".to_string(),
            generation: 0,
        }];
        beta.insert(&node_id, nagram);

        let nagrams = beta.get_nagrams(&node_id);
        assert_eq!(nagrams.len(), 1);
    }

    #[test]
    fn test_network_propagation() {
        let mut engine = ReteEngine::new();

        // Add a test node for "person" facts
        let person_node = TestNode::new("test-person", "person");
        engine.network.add_test_node(person_node);

        // Insert a fact
        let mut fact = Fact::new("person", HashMap::new());
        fact.payload.insert("name".to_string(), crate::data_types::Value::String("Alice".to_string()));
        engine.assert_fact(fact);

        // Check that alpha memory was updated
        assert_eq!(engine.alpha_memory.get_facts("person").len(), 1);
    }

    #[test]
    fn test_engine_metrics() {
        let mut engine = ReteEngine::new();

        // Insert some facts
        for i in 0..3 {
            let mut fact = Fact::new("test", HashMap::new());
            fact.payload.insert(
                "value".to_string(),
                crate::data_types::Value::Integer(i),
            );
            engine.assert_fact(fact);
        }

        let metrics = engine.get_metrics();
        assert_eq!(metrics.facts_inserted, 3);
    }
}
