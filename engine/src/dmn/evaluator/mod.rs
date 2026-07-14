//! Decision evaluation service layer (Phase 33).
//!
//! Orchestrates DRD resolution and decision-table evaluation for a parsed `DmnModel`.
//! This is the "decision service" surface (§10.4) that connects Phases 29–32 to the REST API.

pub mod trace;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use rayon::prelude::*;

use crate::data_types::Value;
use crate::dmn::decision_table::DecisionTableEvaluator;
use crate::dmn::feel::cache::FeelAstCache;
use crate::dmn::feel::eval::Scope;
use crate::dmn::model::{DmnModel, DefinitionsChild, Decision};

use self::trace::InferenceTrace;

// =========================================================================
// Metrics (Phase 35, DMN-27)
// =========================================================================

use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, Opts};
use std::sync::LazyLock;

/// Per-output evaluation counter: total output clause evaluations per decision.
pub static METRIC_OUTPUT_EVALUATIONS_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        Opts::new("rrules_dmn_output_evaluations_total", "Total number of output clause evaluations during decision service invocation"),
        &["decision_id", "output_name"],
    ).expect("metrics to be valid")
});

/// DRD chain depth histogram: distribution of decisions resolved per evaluation.
pub static METRIC_DRD_CHAIN_DEPTH: LazyLock<HistogramVec> = LazyLock::new(|| {
    HistogramVec::new(
        HistogramOpts::new(
            "rrules_dmn_drd_chain_depth_seconds",
            "Distribution of DRD chain depths (number of decisions resolved) per evaluation",
        ),
        &["decision_id"],
    )
    .expect("metrics to be valid")
});

// =========================================================================
// Public types
// =========================================================================

/// Input to a decision evaluation: the `Scope<Value>` plus optional trace request.
#[derive(Debug, Clone)]
pub struct EvaluateRequest {
    /// Input values (keyed by input name).
    pub inputs: HashMap<String, Value>,
    /// Whether to emit an inference trace (Phase 28 pattern).
    pub trace: bool,
}

/// Result of a decision evaluation.
#[derive(Debug, Clone)]
pub struct EvaluateResult {
    /// Output values produced by the decision.
    pub outputs: HashMap<String, Value>,
    /// Number of rules that fired (for hit-policy diagnostics).
    pub rules_fired: u64,
    /// Evaluation duration in milliseconds.
    pub duration_ms: f64,
    /// Optional inference trace (only if `trace: true` on the request).
    pub trace: Option<InferenceTrace>,
    /// Number of decisions resolved in the DRD topological order.
    pub drd_chain_depth: usize,
    /// Output clause names that were evaluated.
    pub outputs_evaluated: Vec<String>,
}

/// Shared snapshot of a loaded DMN model + its AST cache (Phase 25 pattern).
#[derive(Debug)]
pub struct SharedDmnModel {
    /// The parsed IR.
    pub model: DmnModel,
    /// Pre-parsed FEEL ASTs for all cells in the model.
    pub ast_cache: FeelAstCache,
    /// Generation counter (incremented on reload).
    generation: u64,
    /// Cached resolved DRD topological orders, keyed by target decision ID
    /// (PERF-08 hardening). The topological order for a target is computed
    /// once and reused across (recursive) `evaluate` calls, avoiding an
    /// O(V+E) DRD re-walk for every upstream decision. Invalidated on
    /// reload via [`Self::bump_generation`].
    drd_order_cache: RwLock<HashMap<String, Vec<String>>>,
}

impl SharedDmnModel {
    pub fn new(model: DmnModel, ast_cache: FeelAstCache) -> Self {
        Self {
            model,
            ast_cache,
            generation: 1,
            drd_order_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Increment the generation counter (called after a successful reload).
    /// Clears the DRD order cache so stale topological orders are never
    /// reused after the model is reloaded (PERF-08).
    pub fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if let Ok(mut cache) = self.drd_order_cache.write() {
            cache.clear();
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Resolve the DRD topological order for `target_id`, memoizing the
    /// result in [`Self::drd_order_cache`] (PERF-08 hardening). The first
    /// call computes the order via [`resolve_drd_order`]; subsequent calls
    /// (including recursive upstream evaluations) return the cached order,
    /// eliminating redundant O(V+E) DRD walks per `evaluate`.
    ///
    /// The cache is a pure optimization: on a poisoned lock it is skipped
    /// and the order is recomputed, so correctness never depends on it.
    pub fn resolve_drd_order_cached(
        &self,
        target_id: &str,
    ) -> Result<Vec<String>, DecisionEvalError> {
        // Fast path: read lock, return clone on hit.
        if let Ok(cache) = self.drd_order_cache.read() {
            if let Some(order) = cache.get(target_id) {
                return Ok(order.clone());
            }
        }
        // Miss: compute (no inner lock held during the O(V+E) walk), then
        // store. A concurrent caller may compute the same order twice; that
        // is harmless (idempotent insert of an equal value).
        let order = resolve_drd_order(self, target_id)?;
        if let Ok(mut cache) = self.drd_order_cache.write() {
            cache.insert(target_id.to_string(), order.clone());
        }
        Ok(order)
    }

    /// Number of cached DRD orders (for diagnostics / tests).
    pub fn drd_order_cache_len(&self) -> usize {
        self.drd_order_cache.read().map(|c| c.len()).unwrap_or(0)
    }
}

// =========================================================================
// Topological DRD resolution (Phase 33 — §10.4)
// =========================================================================

/// Resolve decisions in topological order so that upstream dependencies are
/// evaluated before dependents. Uses DFS with gray/black coloring for cycle
/// detection (O(V+E) time). Returns the target decision ID last (it's the root).
fn resolve_drd_order(
    shared: &SharedDmnModel,
    target_id: &str,
) -> Result<Vec<String>, DecisionEvalError> {
    // Build decision map: id -> &Decision for all decisions in model.
    let decision_map: Vec<(&str, &Decision)> = extract_decisions_for_drd(&shared.model);

    if decision_map.is_empty() {
        return Ok(vec![target_id.to_string()]);
    }

    // DFS with gray/black coloring.
    #[derive(Clone, Copy, PartialEq)]
    enum Color { White, Gray, Black }
    let mut colors: HashMap<String, Color> = decision_map.iter().map(|(id, _)| (id.to_string(), Color::White)).collect();
    let mut order = Vec::new();

    fn dfs<'a>(
        node: &str,
        decision_map: &[(&'a str, &'a Decision)],
        colors: &mut HashMap<String, Color>,
        order: &mut Vec<String>,
    ) -> Result<(), DecisionEvalError> {
        match colors.get(node).copied() {
            Some(Color::Gray) => return Err(DecisionEvalError::DrdCycle(collect_cycle(node, decision_map))),
            Some(Color::Black) => return Ok(()),
            _ => {}
        }
        colors.insert(node.to_string(), Color::Gray);

        // Find the decision for this node and recurse into its required_decisions.
        if let Some((_, decision)) = decision_map.iter().find(|(id, _)| *id == node) {
            for dep_id in &decision.required_decisions {
                // Only recurse into decisions that exist in the model (skip external refs).
                if decision_map.iter().any(|(id, _)| *id == dep_id.as_str()) {
                    dfs(dep_id, decision_map, colors, order)?;
                }
            }
        }

        colors.insert(node.to_string(), Color::Black);
        order.push(node.to_string());
        Ok(())
    }

    // Sort all decisions for deterministic ordering, then run DFS.
    let mut sorted_ids: Vec<&str> = decision_map.iter().map(|(id, _)| *id).collect();
    sorted_ids.sort();
    for id in &sorted_ids {
        dfs(id, &decision_map, &mut colors, &mut order)?;
    }

    // Filter to only decisions reachable from target_id.
    let mut result = Vec::new();
    fn collect_reachable(
        id: &str, decision_map: &[(&str, &Decision)], reachable: &mut HashSet<String>
    ) {
        if reachable.contains(id) { return; }
        reachable.insert(id.to_string());
        if let Some((_, d)) = decision_map.iter().find(|(id2, _)| id2 == &id) {
            for dep in &d.required_decisions {
                collect_reachable(dep, decision_map, reachable);
            }
        }
    }
    let mut reachable = HashSet::new();
    collect_reachable(target_id, &decision_map, &mut reachable);

    // Return topological order filtered to reachable set.
    for id in order {
        if reachable.contains(&id) { result.push(id); }
    }

    Ok(result)
}

/// Helper: extract all decisions from the model for DRD resolution.
fn extract_decisions_for_drd(model: &DmnModel) -> Vec<(&str, &Decision)> {
    model.definitions.children.iter()
        .filter_map(|child| match child {
            DefinitionsChild::Decision(d) => Some((d.id.as_str(), d)),
            _ => None,
        })
        .collect()
}

/// Collect the cycle path for error reporting.
fn collect_cycle(start: &str, decision_map: &[(&str, &Decision)]) -> Vec<String> {
    let mut cycle = vec![start.to_string()];
    let mut current = start;
    let mut visited = HashSet::new();
    while !visited.contains(current) && visited.len() < 100 {
        visited.insert(current.to_string());
        if let Some((_, d)) = decision_map.iter().find(|(id, _)| id == &current) {
            for dep in &d.required_decisions {
                if visited.contains(dep.as_str()) || dep == start {
                    cycle.push(dep.clone());
                    return cycle;
                }
            }
        }
        // Move to first dependency.
        if let Some((_, d)) = decision_map.iter().find(|(id, _)| id == &current) {
            if !d.required_decisions.is_empty() {
                current = &d.required_decisions[0];
            } else { break; }
        } else { break; }
    }
    cycle
}

// =========================================================================
// Topological levels for parallel DRD evaluation (Phase 39 — PERF-09)
// =========================================================================

/// Group the reachable DRD decision set into topological **levels**.
///
/// `level(d) = 1 + max(level(dep) for dep in required_decisions ∩ reachable)`,
/// or `0` for decisions with no in-reach dependency. Decisions sharing a level
/// have no inter-level dependency and may be evaluated concurrently — this is
/// the correctness invariant that makes within-level rayon parallelism safe.
///
/// `drd_order` is the cycle-safe topological order from [`resolve_drd_order`]
/// (deps before dependents), so a single forward pass assigns levels. The
/// reachable set is closed under dependencies, so every dependency of a
/// reachable decision is already assigned a level when we reach it.
fn resolve_drd_levels(
    shared: &SharedDmnModel,
    drd_order: &[String],
) -> Vec<Vec<String>> {
    let dmap: HashMap<&str, &Decision> = extract_decisions_for_drd(&shared.model)
        .into_iter()
        .map(|(id, d)| (id, d))
        .collect();

    // Forward pass: deps precede dependents in drd_order, so their level is known.
    let mut level: HashMap<&str, usize> = HashMap::new();
    let mut max_level = 0usize;
    for id in drd_order {
        let dep_max = dmap
            .get(id.as_str())
            .and_then(|d| {
                d.required_decisions
                    .iter()
                    .filter_map(|dep| level.get(dep.as_str()).copied())
                    .max()
            })
            .map(|m| m + 1)
            .unwrap_or(0);
        if dep_max > max_level {
            max_level = dep_max;
        }
        level.insert(id.as_str(), dep_max);
    }

    let mut levels: Vec<Vec<String>> = (0..=max_level).map(|_| Vec::new()).collect();
    for id in drd_order {
        if let Some(&l) = level.get(id.as_str()) {
            if let Some(bucket) = levels.get_mut(l) {
                bucket.push(id.clone());
            }
        }
    }
    // Deterministic within-level ordering for stable trace output.
    for bucket in levels.iter_mut() {
        bucket.sort();
    }
    levels
}

/// Output of evaluating a single decision (no recursion, no shared lock).
/// Built by [`evaluate_single_decision`] and gathered by the level-by-level
/// `evaluate` loop (PERF-09).
struct SingleDecisionOutput {
    /// Output-name → value (one entry per output clause, or scope bindings when
    /// the decision has no table).
    outputs: HashMap<String, Value>,
    /// Output names in clause order (for `EvaluateResult::outputs_evaluated`).
    outputs_evaluated: Vec<String>,
    /// 1 if a decision table was evaluated, else 0 (matches pre-PERF-09 behavior).
    rules_fired: u64,
}

/// Evaluate one decision's table (or scope bindings) against `scope`, using the
/// shared AST cache. Pure / lock-free beyond the `DashMap` cache and Prometheus
/// counters (both `Sync`); safe to call from rayon tasks that borrow `shared`.
fn evaluate_single_decision(
    shared: &SharedDmnModel,
    decision: &Decision,
    scope: &Scope,
) -> Result<SingleDecisionOutput, DecisionEvalError> {
    let mut outputs = HashMap::new();
    let mut outputs_evaluated = Vec::new();
    let rules_fired: u64;

    if let Some(table) = &decision.decision_table {
        // Single aggregated result (current simplified semantics: every output
        // clause receives the same result value).
        let result = DecisionTableEvaluator::evaluate(table, scope, &shared.ast_cache)?;
        for (i, output_clause) in table.output_clauses.iter().enumerate() {
            let name = output_clause
                .name
                .as_deref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("output_{}", i));
            outputs.insert(name.clone(), result.clone());
            METRIC_OUTPUT_EVALUATIONS_TOTAL
                .with_label_values(&[decision.id.as_str(), name.as_str()])
                .inc();
            outputs_evaluated.push(name);
        }
        rules_fired = 1;
    } else {
        // No decision table — surface the scope bindings as outputs.
        for (key, value) in &scope.bindings {
            outputs.insert(key.clone(), value.clone());
            METRIC_OUTPUT_EVALUATIONS_TOTAL
                .with_label_values(&[decision.id.as_str(), key.as_str()])
                .inc();
            outputs_evaluated.push(key.clone());
        }
        rules_fired = 0;
    }

    Ok(SingleDecisionOutput {
        outputs,
        outputs_evaluated,
        rules_fired,
    })
}

// =========================================================================
// Decision evaluator (the "decision service" — §10.4)
// =========================================================================

/// Stateful decision evaluation service.
///
/// Holds the shared DMN model snapshot. Thread-safe via `Arc<RwLock<>>`.
pub struct DecisionEvaluator {
    /// Shared snapshot of the loaded DMN model + AST cache.
    pub shared: Arc<RwLock<SharedDmnModel>>,
}

impl DecisionEvaluator {
    /// Create a new decision evaluator with the given shared snapshot.
    pub fn new(shared: Arc<RwLock<SharedDmnModel>>) -> Self {
        Self { shared }
    }

    /// Evaluate a single decision (the MVP: evaluate all outputs of one decision).
    ///
    /// This is the core "decision service" (§10.4) — takes an input scope and
    /// returns per-output values + trace. Resolves DRD edges topologically
    /// before evaluation; cycles produce a `DrdCycle` error.
    ///
    /// Upstream decisions are resolved **level-by-level** (PERF-09): the
    /// reachable DRD subgraph is grouped into topological levels, and
    /// independent decisions within a level are evaluated concurrently via
    /// rayon. Levels are processed in order — the barrier between levels
    /// guarantees every dependency of level N+1 is already computed — so
    /// within-level parallelism preserves sequential semantics. Levels with a
    /// single decision stay sequential to avoid rayon overhead on the common
    /// dependency-chain case.
    pub fn evaluate(
        &self,
        decision_id: &str,
        request: EvaluateRequest,
    ) -> Result<EvaluateResult, DecisionEvalError> {
        let start = std::time::Instant::now();

        // 1. Lock the shared snapshot (read-only) for the whole call. The
        //    parallel tasks below borrow `&SharedDmnModel` (Sync); no inner
        //    lock acquisition happens in the parallel path, and the DRD order
        //    is precomputed before parallelism begins.
        let guard = self.shared.read().map_err(|e| {
            DecisionEvalError::Internal(format!("failed to acquire read lock: {}", e))
        })?;

        // 2. Resolve DRD order (memoized, PERF-08) and derive topological levels.
        let drd_order = guard.resolve_drd_order_cached(decision_id)?;
        let levels = resolve_drd_levels(&guard, &drd_order);

        // Observe DRD chain depth for metrics (Phase 35).
        METRIC_DRD_CHAIN_DEPTH
            .with_label_values(&[decision_id])
            .observe(drd_order.len() as f64);

        // 3. Base inputs cloned once; each level's scopes are built from these
        //    plus the merged context of all earlier levels.
        let base_inputs: HashMap<String, Value> = request.inputs.clone();

        // 4. Decision lookup map (hoist — used for scope merge + target lookup).
        let decision_map: HashMap<String, &Decision> = extract_decisions_for_drd(&guard.model)
            .into_iter()
            .map(|(id, d)| (id.to_string(), d))
            .collect();

        // 5. Walk levels in order. merged_context accumulates upstream outputs
        //    (variable.name -> value); decision_results holds each decision's
        //    raw output (id -> SingleDecisionOutput).
        let mut merged_context: HashMap<String, Value> = HashMap::new();
        let mut decision_results: HashMap<String, SingleDecisionOutput> = HashMap::new();
        let mut trace = if request.trace {
            Some(InferenceTrace::new(decision_id))
        } else {
            None
        };

        for level in &levels {
            // Build (id, &Decision, Scope) for each decision in this level. A
            // decision's scope is base inputs + all earlier-level outputs (a
            // superset of its actual dependencies — consistent with the
            // pre-PERF-09 recursive behavior, which passed the full context).
            let work: Vec<(String, &Decision, Scope)> = level
                .iter()
                .filter_map(|id| {
                    let decision = *decision_map.get(id)?;
                    let mut scope = Scope::empty();
                    for (k, v) in &base_inputs {
                        scope.bindings.insert(k.clone(), v.clone());
                    }
                    for (k, v) in &merged_context {
                        scope.bindings.insert(k.clone(), v.clone());
                    }
                    Some((id.clone(), decision, scope))
                })
                .collect();

            // Evaluate: parallel when the level has >1 independent decision,
            // sequential otherwise. Both paths produce a Vec gathered by value
            // (no shared mutable state across tasks).
            let results: Vec<(String, Result<SingleDecisionOutput, DecisionEvalError>)> =
                if work.len() > 1 {
                    work.par_iter()
                        .map(|(id, decision, scope)| {
                            (id.clone(), evaluate_single_decision(&guard, decision, scope))
                        })
                        .collect()
                } else {
                    work.iter()
                        .map(|(id, decision, scope)| {
                            (id.clone(), evaluate_single_decision(&guard, decision, scope))
                        })
                        .collect()
                };

            // Barrier: surface any error, then merge this level's outputs
            // before advancing to the next level.
            for (id, res) in results {
                let out = res?;
                if id != decision_id {
                    if let Some(ref mut t) = trace {
                        t.emit(
                            "drd_resolve",
                            format!("resolved upstream decision '{}'", id),
                        );
                    }
                }
                // Merge under variable.name (first output), or all outputs if
                // the decision has no variable — matches pre-PERF-09 behavior.
                if let Some(decision) = decision_map.get(&id) {
                    if let Some(ref var) = decision.variable {
                        let name_str: &str = &var.name;
                        if let Some((_, val)) = out.outputs.iter().next() {
                            merged_context.insert(name_str.to_string(), val.clone());
                        }
                    } else {
                        merged_context.extend(out.outputs.clone());
                    }
                }
                decision_results.insert(id, out);
            }
        }

        // 6. The target decision's output is the final result. It was evaluated
        //    in its level (the last) with a scope containing only earlier-level
        //    outputs, exactly as in the recursive version.
        let target = decision_results.remove(decision_id).ok_or_else(|| {
            DecisionEvalError::NotFound(format!("decision '{}' not found", decision_id))
        })?;
        let target_decision = decision_map.get(decision_id).ok_or_else(|| {
            DecisionEvalError::NotFound(format!("decision '{}' not found", decision_id))
        })?;

        let outputs = target.outputs;
        let outputs_evaluated = target.outputs_evaluated;
        let rules_fired = target.rules_fired;

        if let Some(ref mut t) = trace {
            if let Some(ref table) = target_decision.decision_table {
                let hp_str = table.hit_policy.as_str().to_string();
                t.emit_with(
                    "evaluate_table",
                    format!("evaluated table (hit policy: {})", hp_str),
                    None,
                    Vec::new(),
                    Some(outputs.values().next().cloned().unwrap_or(Value::Null)),
                );
            }
            if !merged_context.is_empty() {
                t.emit_with(
                    "merge_scope",
                    format!(
                        "merged {} outputs from upstream decisions",
                        merged_context.len()
                    ),
                    None,
                    Vec::new(),
                    Some(Value::Object(merged_context.clone())),
                );
            }
        }

        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

        // Record evaluation in DMN decision tracker (Phase 35, DMN-27).
        if let Ok(ref mut tracker) = crate::dmn::tracker::DMN_DECISION_TRACKER.lock() {
            tracker.record_evaluation(decision_id);
        }

        Ok(EvaluateResult {
            outputs,
            rules_fired,
            duration_ms,
            trace: trace.filter(|_| request.trace),
            drd_chain_depth: drd_order.len(),
            outputs_evaluated,
        })
    }

    /// Reload the DMN model from a new file (hot-reload — Phase 25 pattern).
    pub fn reload(&self, dmn_content: &str) -> Result<(), DecisionEvalError> {
        // Parse the new DMN content with AST cache.
        let (model, ast_cache) = crate::dmn::parser::parse_dmn_with_cache(dmn_content)?;

        // Acquire write lock and swap.
        let mut guard = self.shared.write().map_err(|e| {
            DecisionEvalError::Internal(format!("failed to acquire write lock: {}", e))
        })?;

        *guard = SharedDmnModel::new(model, ast_cache);
        guard.bump_generation();

        Ok(())
    }
}

// =========================================================================
// Errors
// =========================================================================

/// Errors produced by decision evaluation.
#[derive(Debug, thiserror::Error)]
pub enum DecisionEvalError {
    #[error("decision not found: {0}")]
    NotFound(String),

    #[error("evaluation failed: {0}")]
    EvaluationFailed(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("DRD cycle detected involving decisions: {0:?}")]
    DrdCycle(Vec<String>),

    #[error(transparent)]
    ParseError(#[from] crate::dmn::model::DmnParseError),

    #[error(transparent)]
    TableError(#[from] crate::dmn::decision_table::DecisionTableError),
}

#[cfg(test)]
mod drd_cache_tests {
    use super::*;
    use crate::dmn::feel::cache::FeelAstCache;
    use crate::dmn::model::{Decision, Definitions, DefinitionsChild, DmnModel};

    /// Build a minimal 2-decision DRD: `d2` requires `d1`. No decision tables —
    /// the DRD resolver only walks `required_decisions`, so tables are not
    /// needed to exercise the order cache.
    fn two_decision_model() -> DmnModel {
        let mk = |id: &str, req: Vec<String>| Decision {
            id: id.into(),
            name: None,
            description: None,
            variable: None,
            question: None,
            allowed_answers: None,
            decision_table: None,
            required_decisions: req,
            required_inputs: vec![],
            required_knowledge: vec![],
            annotations: vec![],
        };
        let mut defs = Definitions::default();
        defs.children.push(DefinitionsChild::Decision(mk("d1", vec![])));
        defs.children.push(DefinitionsChild::Decision(mk("d2", vec!["d1".into()])));
        DmnModel { definitions: defs, source_namespace: None }
    }

    /// PERF-08: the DRD topological order is memoized — a second
    /// `resolve_drd_order_cached` for the same target returns the cached
    /// order without recomputation, and the cache holds exactly one entry.
    #[test]
    fn drd_order_is_memoized() {
        let model = two_decision_model();
        let shared = SharedDmnModel::new(model, FeelAstCache::new());

        let order1 = shared.resolve_drd_order_cached("d2").expect("resolve d2");
        assert_eq!(order1, vec!["d1".to_string(), "d2".to_string()]);
        assert_eq!(shared.drd_order_cache_len(), 1, "one cached order for d2");

        // Second call must return the SAME order from the cache (no new entry).
        let order2 = shared.resolve_drd_order_cached("d2").expect("resolve d2 again");
        assert_eq!(order2, order1, "cached order identical to first");
        assert_eq!(shared.drd_order_cache_len(), 1, "cache not grown on hit");

        // Resolving d1 caches a second, distinct order.
        let order_d1 = shared.resolve_drd_order_cached("d1").expect("resolve d1");
        assert_eq!(order_d1, vec!["d1".to_string()]);
        assert_eq!(shared.drd_order_cache_len(), 2, "d1 adds a second cache entry");
    }

    /// PERF-08: `bump_generation` (reload) invalidates the DRD order cache so
    /// a stale topological order is never reused after the model changes.
    #[test]
    fn bump_generation_clears_drd_order_cache() {
        let model = two_decision_model();
        let mut shared = SharedDmnModel::new(model, FeelAstCache::new());
        let _ = shared.resolve_drd_order_cached("d2").unwrap();
        assert_eq!(shared.drd_order_cache_len(), 1);

        shared.bump_generation();
        assert_eq!(shared.drd_order_cache_len(), 0, "reload clears the DRD cache");
        assert_eq!(shared.generation(), 2);
    }
}

#[cfg(test)]
mod parallel_eval_tests {
    use super::*;
    use crate::dmn::feel::cache::FeelAstCache;
    use crate::dmn::model::{
        Decision, DecisionTable, Definitions, DefinitionsChild, DmnModel, DmnRule, HitPolicy,
        InformationItem, InputClause, OutputClause, RuleEntry, TextContent,
    };
    use crate::data_types::Value;
    use rust_decimal::Decimal;
    use std::sync::{Arc, RwLock};

    /// Build a decision with a single-input / single-output table that maps
    /// `input_expr >= 0` → `output_expr`. Used to construct a deterministic
    /// diamond DRD whose level-1 decisions are independent (parallelizable).
    fn table_decision(
        id: &str,
        var_name: &str,
        req: Vec<String>,
        input_expr: &str,
        output_name: &str,
        output_expr: &str,
    ) -> Decision {
        let table = DecisionTable {
            hit_policy: HitPolicy::Unique,
            input_clauses: vec![InputClause {
                id: None,
                label: None,
                input_expression: input_expr.into(),
                input_values: None,
                description: None,
            }],
            output_clauses: vec![OutputClause {
                id: None,
                label: None,
                name: Some(output_name.into()),
                type_ref: None,
                output_values: None,
                default_output_entry: None,
                description: None,
            }],
            rules: vec![DmnRule {
                id: None,
                description: None,
                input_entries: vec![RuleEntry {
                    text: TextContent::plain(">= 0"),
                }],
                output_entries: vec![RuleEntry {
                    text: TextContent::plain(output_expr),
                }],
                annotation_entries: vec![],
            }],
            ..DecisionTable::default()
        };
        Decision {
            id: id.into(),
            name: None,
            description: None,
            variable: Some(InformationItem {
                id: None,
                name: var_name.into(),
                description: None,
                type_ref: None,
                annotations: vec![],
            }),
            question: None,
            allowed_answers: None,
            decision_table: Some(table),
            required_decisions: req,
            required_inputs: vec![],
            required_knowledge: vec![],
            annotations: vec![],
        }
    }

    /// Diamond DRD: d1 → {d2a, d2b} → d3. Level 1 (d2a, d2b) is the parallel
    /// level — the two decisions are independent and must evaluate concurrently
    /// via rayon without corrupting each other's scope or the merged context.
    ///
    /// a = x + 1; b = a * 2; c = a + 100; result = b + c.
    /// For x = 5: a = 6, b = 12, c = 106, result = 118.
    fn diamond_model() -> DmnModel {
        let mut defs = Definitions::default();
        defs.children.push(DefinitionsChild::Decision(table_decision(
            "d1", "a", vec![], "x", "a", "x + 1",
        )));
        defs.children.push(DefinitionsChild::Decision(table_decision(
            "d2a", "b", vec!["d1".into()], "a", "b", "a * 2",
        )));
        defs.children.push(DefinitionsChild::Decision(table_decision(
            "d2b", "c", vec!["d1".into()], "a", "c", "a + 100",
        )));
        defs.children.push(DefinitionsChild::Decision(table_decision(
            "d3", "result", vec!["d2a".into(), "d2b".into()], "b", "result", "b + c",
        )));
        DmnModel {
            definitions: defs,
            source_namespace: None,
        }
    }

    /// PERF-09: `resolve_drd_levels` groups the reachable DRD into topological
    /// levels. The diamond produces exactly three levels: [d1], [d2a, d2b],
    /// [d3] — the middle level holds the two independent decisions.
    #[test]
    fn resolve_drd_levels_groups_diamond() {
        let model = diamond_model();
        let shared = SharedDmnModel::new(model, FeelAstCache::new());
        let order = shared.resolve_drd_order_cached("d3").expect("resolve d3");
        let levels = resolve_drd_levels(&shared, &order);

        assert_eq!(levels.len(), 3, "diamond has three topological levels");
        assert_eq!(levels[0], vec!["d1".to_string()]);
        assert_eq!(levels[1], vec!["d2a".to_string(), "d2b".to_string()]);
        assert_eq!(levels[2], vec!["d3".to_string()]);
    }

    /// PERF-09: end-to-end evaluation through the level-by-level parallel path.
    /// The level-1 decisions (d2a, d2b) evaluate concurrently; their outputs
    /// must merge correctly into d3's scope. Verifies parallel == sequential
    /// semantics: result = b + c = 3a + 100 = 118 for x = 5.
    #[test]
    fn diamond_parallel_eval_produces_correct_result() {
        let model = diamond_model();
        let shared = Arc::new(RwLock::new(SharedDmnModel::new(
            model,
            FeelAstCache::new(),
        )));
        let evaluator = DecisionEvaluator::new(shared);

        let mut inputs = HashMap::new();
        inputs.insert("x".to_string(), Value::Number(Decimal::from(5)));
        let result = evaluator
            .evaluate(
                "d3",
                EvaluateRequest {
                    inputs,
                    trace: false,
                },
            )
            .expect("evaluate d3");

        assert_eq!(result.drd_chain_depth, 4, "all four decisions in the chain");
        assert_eq!(result.rules_fired, 1, "target has a decision table");

        let got = result.outputs.get("result").expect("result output present");
        match got {
            Value::Number(n) => assert_eq!(*n, Decimal::from(118), "b + c = 118"),
            other => panic!("expected Number result, got {other:?}"),
        }
    }

    /// PERF-09: a single-decision-per-level chain (d1 → d2) must still evaluate
    /// correctly through the sequential (non-rayon) level path.
    #[test]
    fn chain_eval_sequential_levels() {
        let mut defs = Definitions::default();
        defs.children.push(DefinitionsChild::Decision(table_decision(
            "d1", "a", vec![], "x", "a", "x + 1",
        )));
        defs.children.push(DefinitionsChild::Decision(table_decision(
            "d2", "b", vec!["d1".into()], "a", "b", "a + 10",
        )));
        let model = DmnModel {
            definitions: defs,
            source_namespace: None,
        };
        let shared = Arc::new(RwLock::new(SharedDmnModel::new(
            model,
            FeelAstCache::new(),
        )));
        let evaluator = DecisionEvaluator::new(shared);

        let mut inputs = HashMap::new();
        inputs.insert("x".to_string(), Value::Number(Decimal::from(5)));
        let result = evaluator
            .evaluate("d2", EvaluateRequest { inputs, trace: false })
            .expect("evaluate d2");

        // a = x + 1 = 6; b = a + 10 = 16.
        let got = result.outputs.get("b").expect("b output present");
        match got {
            Value::Number(n) => assert_eq!(*n, Decimal::from(16), "a + 10 = 16"),
            other => panic!("expected Number b, got {other:?}"),
        }
    }
}
