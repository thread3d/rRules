//! The PHREAK algorithm engine implementation.
//!
//! PHREAK is an extension of the Rete algorithm that provides incremental
//! truth maintenance through justifications and dependency tracking.

use std::collections::{HashMap, HashSet};
use crate::engine::registry::{EngineFactory, EngineResult, EngineType, ExecutionEngine};
use crate::engine::metrics::MetricsCollector;
use crate::fact::Fact;
use crate::engine::rete::NodeId;

// NOTE: `CoverageTracking` is no longer imported here — the shared direct
// evaluator (`engine::direct_eval`) owns once-per-rule coverage recording
// after the fixpoint loop (L-06 / Pitfall 6), so the placeholder per-nagram
// `record_rule_eval`/`record_rule_fire` calls in the old `execute_rules`
// are gone.

/// Nagram type (sequence of memory references forming a partial match)
pub type Nagram = Vec<MemoryReference>;

/// Type for fact IDs
pub type FactId = String;

/// Alpha memory stores fact type activations.
#[derive(Debug, Clone, Default)]
pub struct AlphaMemory {
    entries: HashMap<String, HashSet<String>>,
}

impl AlphaMemory {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, fact_type_id: &str, fact_id: String) {
        self.entries.entry(fact_type_id.to_string()).or_default().insert(fact_id);
    }

    pub fn get_facts(&self, fact_type_id: &str) -> Vec<String> {
        self.entries.get(fact_type_id).map(|s| s.iter().cloned().collect()).unwrap_or_default()
    }

    /// Removes one fact from its fact type, pruning the bucket when it empties.
    pub fn remove(&mut self, fact_type_id: &str, fact_id: &str) {
        if let Some(bucket) = self.entries.get_mut(fact_type_id) {
            bucket.remove(fact_id);
            if bucket.is_empty() {
                self.entries.remove(fact_type_id);
            }
        }
    }

    pub fn size(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }
}

/// Beta memory stores partial matches (nagrams).
#[derive(Debug, Clone, Default)]
pub struct BetaMemory {
    entries: HashMap<String, Vec<Nagram>>,
}

impl BetaMemory {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, join_node_id: impl Into<String>, nagram: Nagram) {
        self.entries.entry(join_node_id.into()).or_default().push(nagram);
    }

    pub fn get_nagrams(&self, join_node_id: impl Into<String>) -> Vec<Nagram> {
        self.entries.get(&join_node_id.into()).cloned().unwrap_or_default()
    }

    /// Drops every partial match that references `fact_id`, pruning empty
    /// join-node buckets. Mirrors the Rete `BetaMemory` contract so all four
    /// engines retract identically.
    pub fn remove_fact(&mut self, fact_id: &str) {
        for nagrams in self.entries.values_mut() {
            nagrams.retain(|nagram| nagram.iter().all(|reference| reference.fact_id != fact_id));
        }
        self.entries.retain(|_, nagrams| !nagrams.is_empty());
    }

    pub fn size(&self) -> usize {
        self.entries.values().map(|v| v.len()).sum()
    }
}

/// A unique identifier for a justification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JustificationId(String);

impl JustificationId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the inner string reference
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<JustificationId> for String {
    fn from(jid: JustificationId) -> Self {
        jid.0
    }
}

// For backwards compatibility with &str and String conversion
impl From<String> for JustificationId {
    fn from(s: String) -> Self {
        JustificationId(s)
    }
}

impl std::fmt::Display for JustificationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// NodeId from rete module - implement Into<String> for convenience
impl From<&NodeId> for String {
    fn from(node_id: &NodeId) -> Self {
        node_id.0.clone()
    }
}

/// Reference to a fact stored in beta memory (nagram).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryReference {
    pub fact_id: String,
    pub generation: usize,
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

        while let Some(current) = stack.pop() {
            if result.contains(&current) {
                continue;
            }
            result.insert(current.clone());

            // Find justifications that depend on this fact
            for (just_id_str, premises) in &self.dependents {
                if premises.contains(&current) {
                    // Convert JustificationId to String - clone first since just_id_str is &String
                    let just_id_val = JustificationId::new(just_id_str.clone());
                    let just_id_str_val: String = just_id_val.into();
                    if let Some(producers) = self.producers.get(&just_id_str_val) {
                        for conclusion in producers.iter() {
                            // Convert JustificationId to String for the stack
                            let concl_id: String = conclusion.clone().into();
                            stack.push(concl_id);
                        }
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

    /// Forgets a fact entirely: it loses support, its dependents lose support
    /// with it, and the dependency edges that mention it are pruned.
    ///
    /// This is the truth-maintenance half of retraction — without it a
    /// retracted fact keeps justifying conclusions that were derived from it.
    pub fn forget_fact(&mut self, fact_id: &str) {
        self.remove_unsupported(fact_id);
        self.supported_facts.remove(fact_id);
        self.dependencies.remove_fact(fact_id);
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

impl Default for ExecutionTrace {
    fn default() -> Self {
        Self::new()
    }
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
    /// Rules loaded via `load_ruleset` (L-02). The direct evaluator
    /// (`execute_rules`, Plan 23-06) iterates this Vec instead of the
    /// placeholder PHREAK network. Populated by cloning from a shared
    /// `RuleSet` snapshot; per-request engines carry their own copy.
    rules: Vec<crate::rules::Rule>,
    trace: ExecutionTrace,
    metrics: crate::engine::metrics::SimpleMetricsCollector,
    /// Phase 28 (D-01 / EXEC-04): trace-collection flag (distinct from the
    /// existing `ExecutionTrace` node-trace field above). Set via
    /// `set_collect_trace` before `execute()`; threaded into `direct_execute`
    /// so the shared helper emits ordered `TraceEvent`s when true. Default
    /// `false` (SC-3 latency neutrality — off-by-default).
    collect_trace: bool,
    /// Fixpoint cap for this engine. `None` defers to the
    /// `RRULES_MAX_CHAIN_ITERATIONS` environment variable (default 1000);
    /// `set_config("max_iterations", …)` sets it explicitly.
    max_iterations: Option<usize>,
}

impl PhreakEngine {
    /// Creates a new PHREAK engine with default settings.
    pub fn new() -> Self {
        Self {
            network: PhreakNetwork::new(),
            alpha_memory: AlphaMemory::new(),
            beta_memory: BetaMemory::new(),
            working_facts: HashMap::new(),
            rules: Vec::new(),
            trace: ExecutionTrace::new(),
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

        // Store the fact
        self.working_facts.insert(fact_id.clone(), fact);

        // Collect test nodes that need propagation first (before mutable borrow)
        let node_ids: Vec<_> = {
            self.network.base.test_nodes.values()
                .filter(|n| n.fact_type_id == fact_type)
                .map(|n| n.id.clone())
                .collect()
        };

        for node_id in node_ids {
            self.alpha_memory.insert(&fact_type, fact_id.clone());
            self.propagate_to_successors(&node_id, &fact_id);
        }

        // Support the fact in TMS
        self.network.tms_mut().support_fact(&fact_id);

        self.metrics.record_fact_inserted();
    }

    /// Propagates a fact through successor nodes.
    fn propagate_to_successors(&mut self, node_id: &crate::engine::rete::NodeId, fact_id: &str) {
        // Collect all join_nodes first (before mutable borrow)
        let join_nodes: Vec<_> = {
            let test_node = match self.network.base.test_nodes.get(node_id) {
                Some(n) => n,
                None => return,
            };

            test_node.successors.iter()
                .filter_map(|s| self.network.base.join_nodes.get(s))
                .cloned()
                .collect()
        };

        for join_node in join_nodes {
            self.trace.record_traversal();
            self.join_propagation(&join_node, fact_id);
        }
    }

    /// Propagates from join node to action nodes.
    fn join_propagation(&mut self, join_node: &crate::engine::rete::JoinNode, _fact_id: &str) {
        // Collect all data before any mutation - get test_facts and existing_nagrams in one scope
        let (test_facts_from_alpha, existing_nagrams, predecessor_facts_for_beta): (
            Vec<String>,
            Vec<Nagram>,
            Vec<String>,
        ) = {
            let join_node_id = &join_node.id.0;

            // Get test node info first
            let test_node = match self.network.base.test_nodes.get(&join_node.predecessor) {
                Some(n) => n,
                None => return,
            };

            // Collect all data needed
            let facts_from_alpha = self.alpha_memory.get_facts(&test_node.fact_type_id);
            let existing_nagrams = self.beta_memory.get_nagrams(join_node_id);

            // Clone the alpha facts since we need them for both iteration and insertion
            (facts_from_alpha.clone(), existing_nagrams, facts_from_alpha)
        };

        // Now process with mutable borrows allowed
        // Combine to form new partial matches
        for fact_from_beta in &existing_nagrams {
            for alpha_fact_id in &test_facts_from_alpha {
                let mut new_nagram = fact_from_beta.clone();
                new_nagram.push(MemoryReference {
                    fact_id: alpha_fact_id.clone(),
                    generation: 0,
                });
                self.beta_memory.insert(&join_node.id.0, new_nagram);
            }
        }

        for alpha_fact_id in predecessor_facts_for_beta {
            let nagram = vec![MemoryReference {
                fact_id: alpha_fact_id.clone(),
                generation: 0,
            }];
            self.beta_memory.insert(&join_node.id.0, nagram);
        }

        self.metrics.record_beta_memory_additions(test_facts_from_alpha.len() + existing_nagrams.len());
    }

    /// Executes the rules loaded via `load_ruleset` against `working_facts`
    /// using the shared direct interpreter (D-04, Plan 23-06).
    ///
    /// This bypasses the placeholder PHREAK network entirely — it delegates
    /// to `engine::direct_eval::direct_execute`, which iterates `self.rules`,
    /// evaluates each `rule.condition` against `self.working_facts` via
    /// `Condition::evaluate`, applies matching `rule.actions`, and chains to
    /// fixpoint under the `RRULES_MAX_CHAIN_ITERATIONS` cap (D-07). Coverage
    /// is recorded once per evaluated/fired rule AFTER the loop (L-06 /
    /// Pitfall 6 — the per-nagram double-count bug in the placeholder
    /// implementation is removed). Returns an `EngineResult` with
    /// `fired_rule_ids` populated.
    ///
    /// The truth-maintenance system (`PhreakNetwork::tms`) is not touched by
    /// the direct evaluator — placeholder justification tracking was a
    /// network-propagation heuristic; wiring real TMS support is deferred to
    /// a future phase (per the v3.0 pluggable-engine deferral).
    pub fn execute_rules(&mut self) -> EngineResult {
        let start = std::time::Instant::now();

        let (fired_set, _evaluated_set, trace) = crate::engine::direct_eval::direct_execute_with_limit(
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
        self.execute_rules()
    }

    fn insert_fact(&mut self, fact: Fact) -> Result<(), String> {
        self.assert_fact(fact);
        Ok(())
    }

    fn retract_fact(&mut self, fact_id: &str) -> Result<bool, String> {
        // Retraction: drop the fact from working memory, from alpha memory and
        // every beta-memory partial match that references it, then forget its
        // PHREAK justifications so the truth-maintenance graph does not keep
        // treating a retracted fact as a premise.
        let Some(fact) = self.working_facts.remove(fact_id) else {
            return Ok(false);
        };
        self.alpha_memory.remove(&fact.fact_type_id, fact_id);
        self.beta_memory.remove_fact(fact_id);
        // Truth maintenance: a retracted fact is no longer supported, its
        // dependents lose support with it, and the dependency edges that
        // mention it are pruned.
        self.network.tms_mut().forget_fact(fact_id);
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
        EngineType::Phreak
    }

    fn reset(&mut self) {
        self.network = PhreakNetwork::new();
        self.alpha_memory = AlphaMemory::new();
        self.beta_memory = BetaMemory::new();
        self.working_facts.clear();
        self.rules.clear();
        self.trace = ExecutionTrace::new();
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
        let result = engine.execute_rules();
        assert!(result.success);

        // Check that justifications were created
        assert_eq!(engine.justification_count(), 0); // Would be > 0 with full implementation
    }
}
