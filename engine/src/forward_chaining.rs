//! Forward chaining execution with Rete algorithm integration.
//!
//! This module implements the execute_forward function for rule evaluation.

use crate::environment::{Engine, Session};
use crate::fact::Fact;

/// Result of a forward chaining execution.
pub struct ExecutionResult {
    /// Number of rules fired
    pub rules_fired: usize,
    /// Facts added during execution
    pub facts_added: Vec<Fact>,
    /// Facts removed during execution
    pub facts_retracted: Vec<String>,
    /// Any errors encountered
    pub errors: Vec<String>,
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionResult {
    pub fn new() -> Self {
        Self {
            rules_fired: 0,
            facts_added: Vec::new(),
            facts_retracted: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn with_rule_fired(mut self) -> Self {
        self.rules_fired += 1;
        self
    }
}

/// Execute the engine's rules over its working memory until nothing new fires.
///
/// Forward chaining *is* the fixpoint loop: evaluate every rule, fire the ones whose conditions hold,
/// let their actions change working memory, and repeat until a pass changes nothing or the iteration
/// cap is reached. `engine::direct_eval` already implements exactly that — salience ordering, agenda
/// groups, once-per-rule firing, chaining under `RRULES_MAX_CHAIN_ITERATIONS` — so this runs that
/// rather than a second implementation of it.
///
/// Both entry points delegate here, which is what makes the answer the same whichever one a caller
/// uses.
#[allow(dead_code)]
fn run(engine: &mut Engine) -> ExecutionResult {
    use std::collections::{HashMap, HashSet};

    // The evaluator takes a plain fact map, keyed the way rule actions name their targets.
    let mut facts: HashMap<String, crate::fact::Fact> = engine
        .working_memory()
        .get_all_facts()
        .into_iter()
        .map(|fact| (fact.id.to_string(), fact.clone()))
        .collect();
    let before: HashSet<String> = facts.keys().cloned().collect();

    let (fired, _evaluated, _trace) = crate::engine::direct_eval::direct_execute_with_limit(
        engine.rules(),
        &mut facts,
        false,
        engine.max_iterations(),
    );

    // Write the result back, so working memory and the map cannot disagree afterwards.
    let after: HashSet<String> = facts.keys().cloned().collect();
    let facts_added: Vec<crate::fact::Fact> = after
        .difference(&before)
        .filter_map(|id| facts.get(id).cloned())
        .collect();
    let facts_retracted: Vec<String> = before.difference(&after).cloned().collect();
    engine
        .working_memory_mut()
        .replace_all(facts.into_values().collect());

    ExecutionResult {
        rules_fired: fired.len(),
        facts_added,
        facts_retracted,
        errors: Vec::new(),
    }
}

/// Executes forward chaining over the engine's rules and working memory.
#[allow(dead_code)]
pub fn execute_forward(engine: &mut Engine, _session: &Session) -> ExecutionResult {
    run(engine)
}

/// Executes forward chaining using just the engine (no session needed).
#[allow(dead_code)]
pub fn execute_forward_simple(engine: &mut Engine) -> ExecutionResult {
    run(engine)
}
