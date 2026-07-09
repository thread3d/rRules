//! Decision evaluation service layer (Phase 33).
//!
//! Orchestrates DRD resolution and decision-table evaluation for a parsed `DmnModel`.
//! This is the "decision service" surface (§10.4) that connects Phases 29–32 to the REST API.

pub mod trace;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use crate::data_types::Value;
use crate::dmn::decision_table::DecisionTableEvaluator;
use crate::dmn::feel::cache::FeelAstCache;
use crate::dmn::feel::eval::Scope;
use crate::dmn::model::{DmnModel, DefinitionsChild, Decision};

use self::trace::InferenceTrace;

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
}

impl SharedDmnModel {
    pub fn new(model: DmnModel, ast_cache: FeelAstCache) -> Self {
        Self {
            model,
            ast_cache,
            generation: 1,
        }
    }

    /// Increment the generation counter (called after a successful reload).
    pub fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

// =========================================================================
// Helper: extract decisions from Definitions children
// =========================================================================

/// Extract all decisions from a `Definitions` node (iterate through `children`).
fn extract_decisions(definitions: &DmnModel) -> Vec<&Decision> {
    definitions
        .definitions
        .children
        .iter()
        .filter_map(|child| match child {
            DefinitionsChild::Decision(d) => Some(d),
            _ => None,
        })
        .collect()
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
    /// This is the core "decision service" (§10.4) — takes an input scope and returns outputs + trace.
    pub fn evaluate(
        &self,
        decision_id: &str,
        request: EvaluateRequest,
    ) -> Result<EvaluateResult, DecisionEvalError> {
        let start = std::time::Instant::now();

        // 1. Lock the shared snapshot (read-only access).
        let guard = self.shared.read().map_err(|e| {
            DecisionEvalError::Internal(format!("failed to acquire read lock: {}", e))
        })?;

        // 2. Find the decision in the model's definitions children.
        let decision = extract_decisions(&guard.model)
            .into_iter()
            .find(|d| d.id == decision_id)
            .ok_or_else(|| {
                DecisionEvalError::NotFound(format!("decision '{}' not found", decision_id))
            })?;

        // 3. Build the input scope from request inputs.
        let mut scope = Scope::empty();
        for (key, value) in &request.inputs {
            scope.bindings.insert(key.clone(), value.clone());
        }

        // 4. Initialize trace if requested.
        let mut trace = if request.trace {
            Some(InferenceTrace::new(decision_id))
        } else {
            None
        };

        // 5. Evaluate the decision table (if present).
        let mut outputs = HashMap::new();
        let mut rules_fired = 0u64;

        if let Some(ref table) = decision.decision_table {
            // Evaluate the decision table against the scope.
            let result = DecisionTableEvaluator::evaluate(
                table,
                &scope,
                &guard.ast_cache,
            )?;

            // Map output clause names to their values.
            for (i, output_clause) in table.output_clauses.iter().enumerate() {
                let name = output_clause.name.as_deref()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("output_{}", i));
                outputs.insert(name, result.clone());
            }

            rules_fired = 1; // Simplified: count tables as "rules fired" for metrics.

            if let Some(ref mut t) = trace {
                let hp_str = decision
                    .decision_table
                    .as_ref()
                    .map(|t| t.hit_policy.as_str().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                let hp_owned = hp_str.clone();
                t.emit_with(
                    "evaluate_table",
                    format!("evaluated table (hit policy: {})", hp_owned),
                    None,
                    Vec::new(),
                    Some(result.clone()),
                );
            }
        } else {
            // No decision table — return the scope values as outputs.
            for (key, value) in request.inputs {
                outputs.insert(key, value);
            }
        }

        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

        Ok(EvaluateResult {
            outputs,
            rules_fired,
            duration_ms,
            trace: trace.filter(|_| request.trace),
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
