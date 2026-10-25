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

    /// Drop every partial match (nagram) that references `fact_id`.
    ///
    /// Retraction must remove the retracted fact from both memories: leaving a
    /// nagram behind would let later assertions join against a fact that is no
    /// longer in working memory. Empty join-node buckets are pruned so
    /// `size()` stays meaningful.
    pub fn remove_fact(&mut self, fact_id: &str) {
        for nagrams in self.entries.values_mut() {
            nagrams.retain(|nagram| {
                nagram.iter().all(|reference| reference.fact_id != fact_id)
            });
        }
        self.entries.retain(|_, nagrams| !nagrams.is_empty());
    }

    /// True when at least one partial match still references `fact_id`.
    pub fn contains_fact(&self, fact_id: &str) -> bool {
        self.entries.values().any(|nagrams| {
            nagrams
                .iter()
                .any(|nagram| nagram.iter().any(|reference| reference.fact_id == fact_id))
        })
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
    /// Rules loaded via `load_ruleset` (L-02). The direct evaluator
    /// (`execute_rules`, Plan 23-03) iterates this Vec instead of the
    /// placeholder Rete network. Populated by cloning from a shared
    /// `RuleSet` snapshot; per-request engines carry their own copy.
    rules: Vec<crate::rules::Rule>,
    metrics: crate::engine::metrics::SimpleMetricsCollector,
    /// Phase 28 (D-01 / EXEC-04, C1 site 2): trace-collection flag. Rete
    /// keeps its own inlined `execute_rules` (it does NOT delegate to the
    /// shared `direct_eval::direct_execute` helper), so the trace emission
    /// MUST be mirrored here independently — the C1 two-site requirement.
    /// Set via `set_collect_trace` before `execute()`. Default `false`
    /// (SC-3 latency neutrality — off-by-default).
    collect_trace: bool,
    /// Fixpoint cap for this engine. `None` defers to the
    /// `RRULES_MAX_CHAIN_ITERATIONS` environment variable (default 1000);
    /// `set_config("max_iterations", …)` sets it explicitly.
    max_iterations: Option<usize>,
}

impl ReteEngine {
    /// Creates a new Rete engine with empty network.
    pub fn new() -> Self {
        Self {
            network: ReteNetwork::new(),
            alpha_memory: AlphaMemory::new(),
            beta_memory: BetaMemory::new(),
            working_facts: HashMap::new(),
            rules: Vec::new(),
            metrics: crate::engine::metrics::SimpleMetricsCollector::new(),
            collect_trace: false,
            max_iterations: None,
        }
    }

    /// Creates a new engine with the given network.
    pub fn with_network(network: ReteNetwork) -> Self {
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

        // NOTE: CoverageTracking::record_rule_eval used to be called here
        // (always against the first action node — a placeholder heuristic).
        // The direct evaluator (`execute_rules`) now owns once-per-rule
        // coverage recording after the fixpoint loop (L-06 / Pitfall 6), so
        // this site no longer records.

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

    /// Executes the rules loaded via `load_ruleset` against `working_facts`
    /// using a direct interpreter (D-04).
    ///
    /// This bypasses the placeholder Rete network entirely — it iterates
    /// `self.rules`, evaluates each `rule.condition` against
    /// `self.working_facts` via `Condition::evaluate`, applies matching
    /// `rule.actions` to `working_facts` (evaluating `Expression`s per
    /// `UpdateFact` field), and chains to fixpoint under the
    /// `RRULES_MAX_CHAIN_ITERATIONS` cap (D-07). Fired rule ids are
    /// collected in a `HashSet` and `CoverageTracking::record_rule_fire` is
    /// called exactly once per fired rule AFTER the loop (not inside any
    /// inner loop — fixes Pitfall 6). `record_rule_eval` is called once per
    /// evaluated rule (fired or not). Returns an `EngineResult` with
    /// `fired_rule_ids` populated.
    pub fn execute_rules(&mut self) -> EngineResult {
        let start = std::time::Instant::now();

        // Rete delegates to the same evaluator as TREAT/LEAPS/PHREAK. It used to
        // inline a copy of the fixpoint loop and of `apply_action`, and the two
        // copies had already diverged: the inlined loop honoured neither
        // agenda-group locking nor deterministic fact resolution. One evaluator
        // means one place to change and one set of semantics to test.
        let (fired_set, _evaluated_set, trace) =
            crate::engine::direct_eval::direct_execute_with_limit(
                &self.rules,
                &mut self.working_facts,
                self.collect_trace,
                self.max_iterations,
            );

        for _ in &fired_set {
            self.metrics.record_rule_fired();
        }
        self.metrics
            .set_execution_time_ms(start.elapsed().as_secs_f64() * 1000.0);

        let mut result = EngineResult::success(fired_set.len());
        result.fired_rule_ids = fired_set.iter().cloned().collect();
        result.trace = trace;
        result
    }
}

impl Default for ReteEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionEngine for ReteEngine {
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
        Ok(true)
    }

    fn set_config(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key {
            // Fixpoint cap for this engine, overriding `RRULES_MAX_CHAIN_ITERATIONS`.
            "max_iterations" => {
                let limit = value
                    .parse::<usize>()
                    .map_err(|_| format!("max_iterations must be a positive integer, got {value:?}"))?;
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
        EngineType::Rete
    }

    fn reset(&mut self) {
        self.network = ReteNetwork::new();
        self.alpha_memory = AlphaMemory::new();
        self.beta_memory = BetaMemory::new();
        self.working_facts.clear();
        self.rules.clear();
        self.metrics.reset_metrics();
        // `collect_trace` is a config flag, not state — leave it across reset.
    }

    /// Phase 28 (D-01 / EXEC-04, C1 site 2): store the trace-collection flag
    /// so `execute_rules` emits ordered `TraceEvent`s inline. Overrides the
    /// trait's default noop. Rete keeps its own inlined fixpoint loop, so
    /// this override is REQUIRED for Rete runs to trace (the shared
    /// `direct_execute` helper is never called).
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
    fn beta_memory_remove_fact_drops_matching_nagrams() {
        let mut beta = BetaMemory::new();
        let join1 = NodeId("join1".to_string());
        let join2 = NodeId("join2".to_string());

        // join1 holds [fact1] and [fact1, fact2]; join2 holds [fact3].
        beta.insert(
            &join1,
            vec![MemoryReference {
                fact_id: "fact1".to_string(),
                generation: 0,
            }],
        );
        beta.insert(
            &join1,
            vec![
                MemoryReference {
                    fact_id: "fact1".to_string(),
                    generation: 0,
                },
                MemoryReference {
                    fact_id: "fact2".to_string(),
                    generation: 0,
                },
            ],
        );
        beta.insert(
            &join2,
            vec![MemoryReference {
                fact_id: "fact3".to_string(),
                generation: 0,
            }],
        );
        assert_eq!(beta.size(), 3);

        beta.remove_fact("fact1");

        // Both nagrams mentioning fact1 are gone; the unrelated one survives and
        // the now-empty join1 bucket is pruned.
        assert_eq!(beta.size(), 1);
        assert!(!beta.contains_fact("fact1"));
        assert!(beta.contains_fact("fact3"));
        assert!(beta.get_nagrams(&join1).is_empty());
    }

    #[test]
    fn retracting_a_fact_removes_it_from_working_memory_and_alpha_memory() {
        let mut engine = ReteEngine::new();
        engine.network.add_test_node(TestNode::new("test-item", "item"));

        let fact = Fact::new("item", HashMap::new());
        let fact_id = fact.id.to_string();
        engine.insert_fact(fact).unwrap();
        assert_eq!(engine.get_facts().len(), 1);

        assert!(engine.retract_fact(&fact_id).unwrap());
        assert!(engine.get_facts().is_empty(), "working memory must drop the fact");
        assert!(
            !engine.beta_memory.contains_fact(&fact_id),
            "beta memory must not keep partial matches for a retracted fact"
        );

        // Idempotent re-retract.
        assert!(!engine.retract_fact(&fact_id).unwrap());
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

    // ---- Plan 23-03 Task 1: load_ruleset / get_facts overrides (L-02) ----

    /// `load_ruleset` with a ruleset containing one rule returns `Ok(())`
    /// (RED: default impl returns `Err`, so this fails until overridden).
    #[test]
    fn load_ruleset_returns_ok_with_one_rule() {
        use crate::rules::{Rule, RuleSet};

        let mut ruleset = RuleSet::new();
        let rule = Rule::builder("r1", "Rule 1").build();
        ruleset.add_rule(rule).unwrap();

        let mut engine = ReteEngine::new();
        let result = engine.load_ruleset(&ruleset);
        assert!(result.is_ok(), "load_ruleset should return Ok, got {:?}", result);
    }

    /// `get_facts` on a fresh engine returns an empty Vec.
    #[test]
    fn get_facts_empty_on_fresh_engine() {
        let engine = ReteEngine::new();
        assert!(engine.get_facts().is_empty());
    }

    /// `get_facts` after `assert_fact` returns a Vec containing the fact
    /// (RED: default `get_facts` returns empty Vec, so this fails until
    /// overridden to read `self.working_facts`).
    #[test]
    fn get_facts_returns_fact_after_assert_fact() {
        let mut engine = ReteEngine::new();
        let mut payload = HashMap::new();
        payload.insert(
            "score".to_string(),
            crate::data_types::Value::Integer(0),
        );
        let fact = Fact::new("scorecard", payload);
        engine.assert_fact(fact.clone());

        let facts = engine.get_facts();
        assert_eq!(facts.len(), 1, "expected one fact in working memory");
        assert_eq!(facts[0].fact_type_id, "scorecard");
        assert_eq!(
            facts[0].payload.get("score"),
            Some(&crate::data_types::Value::Integer(0))
        );
    }

    /// `load_ruleset` clones the ruleset's rules into `engine.rules`
    /// (GREEN: inspects the private field — only compiles after the field
    /// is added).
    #[test]
    fn load_ruleset_stores_rule_into_engine_rules() {
        use crate::rules::{Rule, RuleSet};

        let mut ruleset = RuleSet::new();
        let rule = Rule::builder("r1", "Rule 1").build();
        ruleset.add_rule(rule).unwrap();

        let mut engine = ReteEngine::new();
        engine.load_ruleset(&ruleset).unwrap();
        assert_eq!(engine.rules.len(), 1, "load_ruleset should clone one rule");
        assert_eq!(engine.rules[0].id, "r1");
    }
}
