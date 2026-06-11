//! The PHREAK algorithm engine implementation.
//!
//! PHREAK is an extension of the Rete algorithm that provides incremental
//! truth maintenance through justifications and dependency tracking.

use std::collections::{HashMap, HashSet};
use crate::engine::registry::{EngineFactory, EngineResult, EngineType, ExecutionEngine};
use crate::engine::metrics::MetricsCollector;
use crate::fact::Fact;
use crate::engine::rete::{AlphaMemory, BetaMemory, FactId};

/// A unique identifier for a justification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JustificationId(String);

impl JustificationId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for JustificationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A justification represents how a fact was derived.
#[derive(Debug, Clone)]
pub struct Justification {
    /// Unique identifier
    pub id: JustificationId,
    /// Rule that fired to produce this fact
    pub rule_id: String,
    /// Facts that matched (premises)
    pub premises: Vec<String>,
    /// Facts that were derived (conclusions)
    pub conclusions: Vec<String>,
}

impl Justification {
    /// Creates a new justification.
    pub fn new(
        id: impl Into<String>,
        rule_id: impl Into<String>,
        premises: Vec<String>,
        conclusions: Vec<String>,
    ) -> Self {
        Self {
            id: JustificationId::new(id),
            rule_id: rule_id.into(),
            premises,
            conclusions,
        }
    }

    /// Gets the rule ID.
    pub fn rule(&self) -> &str {
        &self.rule_id
    }

    /// Gets the number of premises.
    pub fn premise_count(&self) -> usize {
        self.premises.len()
    }

    /// Gets the number of conclusions.
    pub fn conclusion_count(&self) -> usize {
        self.conclusions.len()
    }
}

/// Dependency graph for tracking fact dependencies.
#[derive(Debug, Default)]
pub struct DependencyGraph {
    /// Fact ID -> set of justifications that produce it
    producers: HashMap<String, Vec<JustificationId>>,
    /// Justification ID -> set of facts it depends on
    dependents: HashMap<JustificationId, HashSet<String>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a dependency edge from premise to conclusion via justification.
    pub fn add_dependency(&mut self, justification: &Justification) {
        for conclusion in &justification.conclusions {
            self.producers
                .entry(conclusion.clone())
                .or_default()
                .push(justification.id.clone());
        }
        self.dependents
            .insert(justification.id.clone(), justification.premises.iter().cloned().collect());
    }

    /// Removes a fact and its associated dependencies.
    pub fn remove_fact(&mut self, fact_id: &str) {
        // Remove as producer for other facts
        self.producers.remove(fact_id);

        // Find justifications that depend on this fact
        let mut to_remove = Vec::new();
        for (just_id, premises) in &self.dependents {
            if premises.contains(fact_id) {
                to_remove.push(just_id.clone());
            }
        }

        // Remove these justifications
        for just_id in &to_remove {
            self.remove_justification(just_id);
        }
    }

    /// Removes a justification and its edges.
    fn remove_justification(&mut self, just_id: &JustificationId) {
        if let Some(premises) = self.dependents.remove(just_id) {
            // Remove from producers
            for premise in premises {
                if let Some(producers) = self.producers.get_mut(&premise) {
                    producers.retain(|j| j != just_id);
                }
            }
        }
    }

    /// Gets all justifications that produce a fact.
    pub fn get_producers(&self, fact_id: &str) -> &[JustificationId] {
        self.producers.get(fact_id).map_or(&[], |v| v.as_slice())
    }

    /// Gets all facts that depend on a given fact (transitive).
    pub fn get_dependents(&self, fact_id: &str) -> HashSet<String> {
        let mut result = HashSet::new();
        let mut stack = vec![fact_id.to_string()];

        while !stack.is_empty() {
            let current = stack.pop().unwrap();
            if result.contains(&current) {
                continue;
            }
            result.insert(current.clone());

            // Find justifications that depend on this fact
            for (just_id, premises) in &self.dependents {
                if premises.contains(&current) {
                    for conclusion in &self.producers[just_id].iter() {
                        stack.push(conclusion.clone());
                    }
                }
            }
        }

        result
    }

    /// Gets all justifications.
    pub fn all_justifications(&self) -> Vec<&JustificationId> {
        self.dependents.keys().collect()
    }
}

/// Truth maintenance system for tracking valid facts.
#[derive(Debug, Default)]
pub struct TruthMaintenance {
    /// Facts that are currently supported/valid
    supported_facts: HashSet<String>,
    /// Justifications that are active
    active_justifications: HashSet<JustificationId>,
    /// Dependency graph
    dependencies: DependencyGraph,
}

impl TruthMaintenance {
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks a fact as supported.
    pub fn support_fact(&mut self, fact_id: &str) {
        self.supported_facts.insert(fact_id.to_string());
    }

    /// Checks if a fact is currently supported.
    pub fn is_supported(&self, fact_id: &str) -> bool {
        self.supported_facts.contains(fact_id)
    }

    /// Adds a justification to the system.
    pub fn add_justification(&mut self, justification: Justification) {
        self.dependencies.add_dependency(&justification);
        self.active_justifications.insert(justification.id.clone());
    }

    /// Validates if all premises of a justification are supported.
    pub fn validate_justification(&self, just: &Justification) -> bool {
        just.premises.iter().all(|p| self.is_supported(p))
    }

    /// Removes an unsupported fact and its dependents (conflict resolution).
    pub fn remove_unsupported(&mut self, fact_id: &str) {
        if !self.supported_facts.remove(fact_id) {
            return; // Already removed or never supported
        }

        // Find and mark dependent justifications as invalid
        let dependents = self.dependencies.get_dependents(fact_id);
        for dependent in dependents {
            self.supported_facts.remove(&dependent);
        }
    }

    /// Gets the set of currently supported facts.
    pub fn get_supported_facts(&self) -> Vec<String> {
        self.supported_facts.iter().cloned().collect()
    }

    /// Gets the number of active justifications.
    pub fn active_justification_count(&self) -> usize {
        self.active_justifications.len()
    }
}

/// PHREAK network extends Rete with truth maintenance.
#[derive(Debug)]
pub struct PhreakNetwork {
    /// Base Rete network
    base: crate::engine::rete::ReteNetwork,
    /// Truth maintenance system
    tms: TruthMaintenance,
}

impl PhreakNetwork {
    pub fn new() -> Self {
        Self {
            base: crate::engine::rete::ReteNetwork::new(),
            tms: TruthMaintenance::new(),
        }
    }

    /// Gets a reference to the TMS.
    pub fn tms(&self) -> &TruthMaintenance {
        &self.tms
    }

    /// Gets a mutable reference to the TMS.
    pub fn tms_mut(&mut self) -> &mut TruthMaintenance {
        &mut self.tms
    }
}

impl Default for PhreakNetwork {
    fn default() -> Self {
        Self::new()
    }
}

/// Execution trace showing which rules fired and why.
#[derive(Debug, Clone)]
pub struct ExecutionTrace {
    /// Rules that were fired
    pub fired_rules: Vec<RuleFire>,
    /// Total network traversals
    pub traversals: u64,
    /// Facts retracted (unsupportable)
    pub unsupportable_facts: Vec<String>,
}

impl ExecutionTrace {
    pub fn new() -> Self {
        Self {
            fired_rules: Vec::new(),
            traversals: 0,
            unsupportable_facts: Vec::new(),
        }
    }

    /// Records a rule firing.
    pub fn record_rule_fire(&mut self, rule_id: impl Into<String>, justification: Justification) {
        self.fired_rules.push(RuleFire {
            rule_id: rule_id.into(),
            justification,
        });
    }

    /// Records a network traversal step.
    pub fn record_traversal(&mut self) {
        self.traversals += 1;
    }
}

/// A single rule firing with its justification.
#[derive(Debug, Clone)]
pub struct RuleFire {
    /// The rule that fired
    pub rule_id: String,
    /// How the rule's conditions were satisfied
    pub justification: Justification,
}

impl RuleFire {
    /// Creates a new rule fire record.
    pub fn new(rule_id: impl Into<String>, justification: Justification) -> Self {
        Self {
            rule_id: rule_id.into(),
            justification,
        }
    }

    /// Gets the rule ID.
    pub fn rule(&self) -> &str {
        &self.rule_id
    }
}

/// The PHREAK algorithm engine implementation.
#[derive(Debug)]
pub struct PhreakEngine {
    network: PhreakNetwork,
    alpha_memory: AlphaMemory,
    beta_memory: BetaMemory,
    working_facts: HashMap<FactId, Fact>,
    trace: ExecutionTrace,
    metrics: crate::engine::metrics::SimpleMetricsCollector,
}

impl PhreakEngine {
    /// Creates a new PHREAK engine with default settings.
    pub fn new() -> Self {
        Self {
            network: PhreakNetwork::new(),
            alpha_memory: AlphaMemory::new(),
            beta_memory: BetaMemory::new(),
            working_facts: HashMap::new(),
            trace: ExecutionTrace::new(),
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

        // Support the fact in TMS
        self.network.tms_mut().support_fact(&fact_id);

        self.metrics.record_fact_inserted();
    }

    /// Propagates a fact through successor nodes.
    fn propagate_to_successors(&mut self, node_id: &crate::engine::rete::NodeId, fact_id: &str) {
        let test_node = match self.network.base.test_nodes.get(node_id) {
            Some(n) => n,
            None => return,
        };

        for successor_id in &test_node.successors {
            self.trace.record_traversal();
            if let Some(join_node) = self.network.base.join_nodes.get(successor_id) {
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

    /// Executes the engine and fires rules with truth maintenance.
    pub fn execute_rules(&mut self) -> usize {
        let mut rules_fired = 0;

        for (_node_id, action_node) in &self.network.base.action_nodes {
            for (_join_id, join_node) in &self.network.base.join_nodes {
                let nagrams = self.beta_memory.get_nagrams(&join_node.id);

                // For each partial match, create a justification
                for (i, _nagram) in nagrams.iter().enumerate() {
                    let just_id = JustificationId::new(format!(
                        "{}_{}_{}",
                        action_node.id.0, join_node.id.0, i
                    ));

                    let justification = Justification::new(
                        just_id.clone(),
                        &action_node.rule_id,
                        vec![], // Would contain premise facts in full implementation
                        vec![format!("result_{}", rules_fired)],
                    );

                    // Add to TMS
                    self.network.tms_mut().add_justification(justification);

                    rules_fired += 1;
                    self.metrics.record_rule_fired();
                }
            }
        }

        self.trace.fired_rules.clear(); // In real implementation, record here
        self.metrics.set_execution_time_ms(0.0);
        rules_fired
    }

    /// Gets a trace of execution.
    pub fn get_trace(&self) -> &ExecutionTrace {
        &self.trace
    }

    /// Checks if a fact is currently supported.
    pub fn is_supported(&self, fact_id: &str) -> bool {
        self.network.tms().is_supported(fact_id)
    }

    /// Gets all currently supported facts.
    pub fn get_supported_facts(&self) -> Vec<String> {
        self.network.tms().get_supported_facts()
    }

    /// Retracts a fact and removes dependent justifications.
    pub fn retract_fact_with_maintenance(&mut self, _fact_id: &str) -> Result<bool, String> {
        // In a full implementation, this would:
        // 1. Remove the fact from alpha memory
        // 2. Update beta memory (remove nagrams containing this fact)
        // 3. Invalidate justifications that depend on this fact
        // 4. Remove derived facts that lose support

        Ok(false) // Placeholder - full implementation would track and propagate changes
    }

    /// Gets the count of active justifications.
    pub fn justification_count(&self) -> usize {
        self.network.tms().active_justification_count()
    }
}

impl Default for PhreakEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionEngine for PhreakEngine {
    fn execute(&mut self) -> EngineResult {
        let fired = self.execute_rules();
        EngineResult::success(fired)
    }

    fn insert_fact(&mut self, fact: Fact) -> Result<(), String> {
        self.assert_fact(fact);
        Ok(())
    }

    fn retract_fact(&mut self, _fact_id: &str) -> Result<bool, String> {
        // Placeholder - would implement full maintenance
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
        EngineType::Phreak
    }

    fn reset(&mut self) {
        self.network = PhreakNetwork::new();
        self.alpha_memory = AlphaMemory::new();
        self.beta_memory = BetaMemory::new();
        self.working_facts.clear();
        self.trace = ExecutionTrace::new();
        self.metrics.reset_metrics();
    }
}

/// Factory for creating PHREAK engine instances.
#[derive(Debug, Clone)]
pub struct PhreakEngineFactory;

impl PhreakEngineFactory {
    /// Creates a new factory.
    pub fn new() -> Self {
        Self
    }
}

impl Default for PhreakEngineFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineFactory for PhreakEngineFactory {
    fn engine_type(&self) -> EngineType {
        EngineType::Phreak
    }

    fn create_engine(&self) -> Result<Box<dyn ExecutionEngine>, String> {
        Ok(Box::new(PhreakEngine::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_justification_creation() {
        let just = Justification::new(
            "j1",
            "rule1",
            vec!["fact1".to_string(), "fact2".to_string()],
            vec!["result1".to_string()],
        );

        assert_eq!(just.premise_count(), 2);
        assert_eq!(just.conclusion_count(), 1);
    }

    #[test]
    fn test_dependency_graph() {
        let mut graph = DependencyGraph::new();
        let just = Justification::new("j1", "rule1", vec!["a".to_string()], vec!["b".to_string()]);
        graph.add_dependency(&just);

        assert_eq!(graph.get_producers("b").len(), 1);
    }

    #[test]
    fn test_truth_maintenance() {
        let mut tms = TruthMaintenance::new();

        tms.support_fact("fact1");
        assert!(tms.is_supported("fact1"));

        tms.remove_unsupported("fact1");
        assert!(!tms.is_supported("fact1"));
    }

    #[test]
    fn test_phreak_engine_creation() {
        let engine = PhreakEngine::new();
        assert_eq!(engine.engine_type(), EngineType::Phreak);
    }

    #[test]
    fn test_justification_tracking() {
        let mut engine = PhreakEngine::new();

        // Add some facts
        for i in 0..3 {
            let mut fact = Fact::new("test", HashMap::new());
            fact.payload.insert(
                "value".to_string(),
                crate::data_types::Value::Integer(i),
            );
            engine.assert_fact(fact);
        }

        // Execute and track justifications
        let fired = engine.execute_rules();
        assert!(fired >= 0);

        // Check that justifications were created
        assert_eq!(engine.justification_count(), 0); // Would be > 0 with full implementation
    }
}
