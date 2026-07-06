//! The Rete algorithm engine implementation.
//!
//! This module implements the classic Rete algorithm with:
//! - Party tokens (alpha nodes) for fact type matching
//! - Beta memory for join node partial matches
//! - Network propagation for efficient pattern matching

use crate::engine::registry::{EngineFactory, EngineResult, EngineType, ExecutionEngine};
use crate::engine::metrics::MetricsCollector;
use crate::fact::Fact;
use crate::rules::Action;
use std::collections::{HashMap, HashSet};

use crate::monitoring::coverage::CoverageTracking;
use crate::monitoring::detection::RuleExecutionTracker;

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

        // D-07: fixpoint cap — configurable via env var, default 1000.
        let max_iter = std::env::var("RRULES_MAX_CHAIN_ITERATIONS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1000);

        let mut fired_set: HashSet<String> = HashSet::new();
        let mut evaluated_set: HashSet<String> = HashSet::new();

        // Phase 28 (D-01 / EXEC-04, C1 site 2): Rete keeps its own inlined
        // fixpoint loop (it does NOT delegate to `direct_eval::direct_execute`),
        // so trace emission is mirrored here independently. When
        // `self.collect_trace` is false, no `Vec`/`String`/`chrono` work
        // happens (SC-3 latency neutrality). `trace_start` is captured once
        // at loop entry; events are pushed in firing order inside the loop.
        let trace_start = if self.collect_trace { Some(std::time::Instant::now()) } else { None };
        let mut trace_events: Vec<crate::engine::registry::TraceEvent> = Vec::new();
        let mut facts_matched: usize = 0;

        // Fixpoint loop: repeat passes until no rule newly fires in a full
        // pass (D-07). Each rule fires at most once per execute() call
        // (fired_set guard) — this bounds the loop at |rules| passes and is
        // the once-per-rule semantic that makes coverage recording truthful
        // (L-06 / Pitfall 6). The max_iter cap is the ultimate safety net
        // for pathological rule sets.
        for _pass in 0..max_iter {
            let mut newly_fired_this_pass = false;
            // Index-based iteration so we can mutate `working_facts` while
            // reading `rules` without conflicting borrows.
            for rule_idx in 0..self.rules.len() {
                let rule_id = self.rules[rule_idx].id.clone();
                if fired_set.contains(&rule_id) {
                    continue;
                }
                // Evaluate the condition against the current working memory.
                let facts: Vec<&Fact> = self.working_facts.values().collect();
                evaluated_set.insert(rule_id.clone());
                let matched = self.rules[rule_idx].condition.evaluate(&facts);
                if matched {
                    // Phase 28 (D-01, C1 site 2): emit a TraceEvent at the
                    // moment the rule fires — IN FIRING ORDER. Guarded by
                    // `self.collect_trace` so the off path skips the chrono
                    // call + String allocation entirely (SC-3). `facts.len()`
                    // is the honest count of facts in working memory that the
                    // condition matched against (D-01 discretion).
                    if self.collect_trace {
                        facts_matched += facts.len();
                        trace_events.push(crate::engine::registry::TraceEvent {
                            rule_id: rule_id.clone(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            event_type: "rule_fired",
                        });
                    }
                    // Apply actions. Clone the actions Vec so the immutable
                    // borrow of `self.rules` ends before we mutably borrow
                    // `self.working_facts`.
                    let actions = self.rules[rule_idx].actions.clone();
                    for action in &actions {
                        apply_action(action, &mut self.working_facts);
                    }
                    fired_set.insert(rule_id);
                    newly_fired_this_pass = true;
                    self.metrics.record_rule_fired();
                }
            }
            if !newly_fired_this_pass {
                break;
            }
        }

        // L-06 / Pitfall 6: record coverage ONCE per rule OUTSIDE the inner
        // loop. `record_rule_eval` once per evaluated rule (fired or not);
        // `record_rule_fire` once per fired rule. This replaces the old
        // per-nagram double-count at the placeholder site.
        //
        // Plan 26-02 / C1: also record into the `RuleExecutionTracker`
        // singleton (D-03, L-02) at the same call sites. Rete keeps its
        // own inlined `execute_rules` (C1 — it does NOT delegate to
        // `direct_eval.rs`), so this site must be wired independently.
        // The id→name lookup is built once from `self.rules` (Pitfall 4).
        let id_to_name: HashMap<&String, &String> =
            self.rules.iter().map(|r| (&r.id, &r.name)).collect();
        for id in &evaluated_set {
            CoverageTracking::record_rule_eval(id);
            RuleExecutionTracker::record_evaluation(id, id_to_name[id]);
        }
        for id in &fired_set {
            CoverageTracking::record_rule_fire(id);
            RuleExecutionTracker::record_fire(id, id_to_name[id]);
        }

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        self.metrics.set_execution_time_ms(elapsed_ms);

        // Phase 28 (D-01, C1 site 2): build the TraceData payload only when
        // `collect_trace` is true AND at least one rule fired. When
        // `collect_trace` is false, `trace_events` is empty and `trace_start`
        // is None — `result.trace` stays `None` (SC-3 latency neutrality).
        let trace = if self.collect_trace && !trace_events.is_empty() {
            let total_duration_ms = trace_start
                .map(|s| s.elapsed().as_secs_f64() * 1000.0)
                .unwrap_or(0.0);
            Some(crate::engine::registry::TraceData {
                events: trace_events,
                total_duration_ms,
                facts_matched,
            })
        } else {
            None
        };

        let mut result = EngineResult::success(fired_set.len());
        result.fired_rule_ids = fired_set.iter().cloned().collect();
        result.trace = trace;
        result
    }
}

/// Apply a single `Action` to the engine's working memory (D-04). Mirrors
/// `WorkingMemory::insert_fact`/`update_fact`/`retract_fact` (fact.rs:169-216)
/// but operates on the Rete engine's `HashMap<FactId, Fact>` keyed by
/// `fact.id.to_string()`.
///
/// For `Action::UpdateFact { id, fields }`: the target fact is located by
/// exact `FactId` key first, then by the first fact whose `fact_type_id`
/// matches `id` (the keystone scenario uses `id: "scorecard"`, a fact type).
/// Each field's `Expression` is evaluated against the current working memory
/// snapshot and written into the fact's payload. `Action::CallFunction` and
/// `Action::LogMessage` are no-ops in Phase 23 (function dispatch and logging
/// are out of scope for the direct evaluator).
fn apply_action(action: &Action, working_facts: &mut HashMap<FactId, Fact>) {
    match action {
        Action::UpdateFact { id, fields } => {
            // Snapshot the facts for Expression evaluation (FieldRef reads).
            // Cloning is O(facts) and acceptable per action application.
            let facts_snapshot: Vec<Fact> = working_facts.values().cloned().collect();
            let facts_refs: Vec<&Fact> = facts_snapshot.iter().collect();

            // Locate the target fact: exact FactId key first, else first
            // fact of the named fact_type_id.
            let key_opt = if working_facts.contains_key(id) {
                Some(id.clone())
            } else {
                working_facts
                    .iter()
                    .find(|(_, f)| f.fact_type_id == *id)
                    .map(|(k, _)| k.clone())
            };
            let key = match key_opt {
                Some(k) => k,
                None => return,
            };
            let fact = match working_facts.get_mut(&key) {
                Some(f) => f,
                None => return,
            };
            for (field, expr) in fields {
                let value = expr.evaluate(&facts_refs);
                fact.payload.insert(field.clone(), value);
            }
        }
        Action::InsertFact { fact_type, fields } => {
            let fact = Fact::new(fact_type.clone(), fields.clone());
            working_facts.insert(fact.id.to_string(), fact);
        }
        Action::RetractFact { id } => {
            if working_facts.contains_key(id) {
                working_facts.remove(id);
            } else {
                let key_opt = working_facts
                    .iter()
                    .find(|(_, f)| f.fact_type_id == *id)
                    .map(|(k, _)| k.clone());
                if let Some(k) = key_opt {
                    working_facts.remove(&k);
                }
            }
        }
        Action::CallFunction { .. } => {
            // No-op in Phase 23 — function dispatch is out of scope (D-06).
        }
        Action::LogMessage { .. } => {
            // No-op — logging is not wired in the direct evaluator.
        }
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

    fn retract_fact(&mut self, _fact_id: &str) -> Result<bool, String> {
        // TODO: Implement retraction with proper network cleanup
        Ok(false)
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
