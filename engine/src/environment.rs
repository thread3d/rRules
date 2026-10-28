//! Engine initialization and configuration with chaining mode selection.
//!
//! This module provides the EngineBuilder for configuration and the Engine struct
//! that manages rule execution with automatic or explicit chaining modes.

use crate::fact::WorkingMemory;
use crate::rules::RuleSet;
use crate::ChainingMode;

/// Builder for creating configured engine instances.
#[derive(Clone)]
pub struct EngineBuilder {
    /// Chaining mode selection strategy
    chaining_mode: ChainingMode,
    /// Maximum iterations before loop detection triggers
    max_iterations: usize,
    /// Enable detailed debug tracing
    enable_debug_trace: bool,
}

impl EngineBuilder {
    /// Creates a new engine builder with default settings.
    pub fn new() -> Self {
        Self {
            chaining_mode: ChainingMode::Automatic,
            max_iterations: 1000,
            enable_debug_trace: false,
        }
    }

    /// Sets the chaining mode selection strategy.
    pub fn chaining_mode(mut self, mode: ChainingMode) -> Self {
        self.chaining_mode = mode;
        self
    }

    /// Sets maximum iterations before loop detection triggers.
    pub fn max_iterations(mut self, limit: usize) -> Self {
        self.max_iterations = limit;
        self
    }

    /// Enables or disables debug tracing.
    pub fn enable_debug_trace(mut self, enabled: bool) -> Self {
        self.enable_debug_trace = enabled;
        self
    }

    /// Builds the configured engine instance.
    pub fn build(self) -> Engine {
        Engine {
            working_memory: WorkingMemory::new(),
            rule_set: RuleSet::new(),
            chaining_mode: self.chaining_mode,
            max_iterations: self.max_iterations,
            enable_debug_trace: self.enable_debug_trace,
            last_trace: None,
        }
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// The main engine instance managing rules and execution.
#[derive(Clone, Debug)]
pub struct Engine {
    /// Working memory for facts
    working_memory: WorkingMemory,
    /// Set of compiled rules
    rule_set: RuleSet,
    /// Chaining mode selection strategy
    chaining_mode: ChainingMode,
    /// Maximum iterations before loop detection triggers
    max_iterations: usize,
    /// Enable detailed debug tracing
    enable_debug_trace: bool,
    /// The trace of the most recent execution, when `enable_debug_trace` is set.
    last_trace: Option<crate::engine::TraceData>,
}

impl Engine {
    /// Creates a new session from the engine.
    ///
    /// # Arguments
    /// * `stateful` - If true, creates a long-lived session (days to weeks),
    ///   or false for a stateless session used for single-use execution.
    /// * `session_id` - Optional explicit ID for session lifecycle management.
    pub fn create_session(&self, stateful: bool, session_id: Option<String>) -> Session {
        Session::new(self, stateful, session_id)
    }

    /// Returns the current chaining mode.
    pub fn get_chaining_mode(&self) -> ChainingMode {
        self.chaining_mode
    }

    /// Gets a reference to working memory.
    pub fn working_memory(&self) -> &WorkingMemory {
        &self.working_memory
    }

    /// Gets mutable access to working memory for direct manipulation.
    pub fn working_memory_mut(&mut self) -> &mut WorkingMemory {
        &mut self.working_memory
    }

    /// Loads rules from a string representation and compiles them.
    /// Load rules from their JSON form — a rule document, a bare rule, or an array of either.
    ///
    /// This is the same shape the API stores (`build_ruleset_from_dir` reads `.rules` files as JSON)
    /// and the same `Rule` type the engines execute, so a rule written for one path works in the
    /// other. It is not a text DSL parser: `dsl_body` is the FEEL expression inside a rule, not a
    /// second syntax for rules themselves.
    pub fn load_rules(&mut self, rules_text: &str) -> Result<(), String> {
        let parsed: Result<LoadedRules, _> = serde_json::from_str(rules_text);

        // Files in a rules directory are `{ "rules": [ … ] }`; a caller holding one rule passes it
        // bare, and a caller holding several passes an array.
        let rules = match parsed {
            Ok(LoadedRules::Document { rules }) => rules,
            Ok(LoadedRules::Many(rules)) => rules,
            Ok(LoadedRules::One(rule)) => vec![*rule],
            Err(error) => return Err(format!("rules are not valid JSON: {error}")),
        };

        self.rule_set
            .add_rules(rules)
            .map_err(|error| format!("rules were rejected: {error}"))
    }

    /// Insert a fact into this engine's working memory.
    ///
    /// Distinct from [`Session::insert_fact`]: a session accumulates into its own clone, while this
    /// writes to the engine the caller holds — which is what a simulation variant needs.
    pub fn insert_fact_for_simulation(&mut self, fact: crate::fact::Fact) -> Result<(), String> {
        self.working_memory_mut()
            .insert_fact(fact)
            .map_err(|error| format!("the fact was rejected: {error}"))
    }

    /// Whether this engine records a trace of the fixpoint run (`enable_debug_trace`).
    pub(crate) fn debug_trace_enabled(&self) -> bool {
        self.enable_debug_trace
    }

    /// Record the trace of a run (used by the fixpoint loop, which owns the evaluator call).
    pub(crate) fn set_last_trace(&mut self, trace: Option<crate::engine::TraceData>) {
        self.last_trace = trace;
    }

    /// The trace of the most recent execution, when tracing is enabled.
    pub fn last_trace(&self) -> Option<&crate::engine::TraceData> {
        self.last_trace.as_ref()
    }

    /// Executes forward chaining with the current rules and facts.
    pub(crate) fn rules(&self) -> &[crate::rules::Rule] {
        &self.rule_set.rules
    }

    /// Add one rule, for the what-if simulation's `AddRule` change.
    pub fn add_rule(&mut self, rule: crate::rules::Rule) -> Result<(), String> {
        self.rule_set
            .add_rules(vec![rule])
            .map_err(|error| format!("the rule was rejected: {error}"))
    }

    /// Mutable access to this engine's rules, for the what-if simulation's variant changes.
    pub(crate) fn rules_mut(&mut self) -> &mut Vec<crate::rules::Rule> {
        &mut self.rule_set.rules
    }

    /// The fixpoint cap this engine was built with.
    pub(crate) fn max_iterations(&self) -> Option<usize> {
        Some(self.max_iterations)
    }

    pub fn execute_forward(&mut self) -> ExecutionResult {
        // The session-taking entry point and this one run the same loop; see
        // `forward_chaining::run`, which exists so they cannot answer differently.
        let session = Session::new(self, false, None);
        let report = crate::forward_chaining::execute_forward(self, &session);
        if report.rules_fired == 0 && !report.errors.is_empty() {
            return ExecutionResult::new(false, report.errors.join("; "));
        }
        ExecutionResult::new(
            true,
            format!(
                "{} rule(s) fired, {} fact(s) changed",
                report.rules_fired,
                report.facts_added.len() + report.facts_retracted.len()
            ),
        )
    }

    /// Executes backward chaining from a goal — `"<fact_type>.<field>"`.
    ///
    /// Delegates to the same search the module exposes, so the engine and the free function cannot
    /// answer differently; this one renders the trace as the human-readable path its result carries.
    pub fn execute_backward(&self, goal: &str) -> InferenceTrace {
        let session = Session::new(self, false, None);
        let trace = crate::backward_chaining::execute_backward(self, &session, goal);
        InferenceTrace::new(trace.success, trace.explain())
    }
}

/// A session manages execution state for a single rule application.
pub struct Session {
    /// The engine this session holds.
    ///
    /// A session used to keep a raw `*const Engine` that was never dereferenced, so it could neither
    /// read nor change anything. It owns a clone instead: a session accumulates facts and runs the
    /// rules against them, which is what makes it a session rather than a handle to somebody else's
    /// state.
    engine: Engine,
    /// Stateful vs stateless mode
    stateful: bool,
    /// Optional explicit ID
    session_id: Option<String>,
    /// Track if session has been destroyed
    destroyed: bool,
}

impl Session {
    /// Creates a new session from the engine.
    pub fn new(engine: &Engine, stateful: bool, session_id: Option<String>) -> Self {
        Self {
            engine: engine.clone(),
            stateful,
            session_id,
            destroyed: false,
        }
    }

    /// The engine this session is running, with the facts it has accumulated.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Inserts a fact into this session's working memory.
    /// Insert a fact into this session's engine — a session is a handle to the rules, not a copy of
    /// them, so inserting here is what makes the accumulated facts visible to the next `execute`.
    pub fn insert_fact(&mut self, fact: crate::fact::Fact) -> Result<(), String> {
        if self.destroyed {
            return Err("Session has been destroyed".to_string());
        }
        self.engine
            .working_memory_mut()
            .insert_fact(fact)
            .map_err(|error| format!("the fact was rejected: {error}"))
    }

    /// Executes the rules this session holds, over the facts it has accumulated.
    ///
    /// `stateful` now means what its documentation says. A **stateless** session is for single-use
    /// execution: its working memory is cleared once the run completes, so the next `execute` starts
    /// from the facts inserted since (none, unless the caller adds more). A **stateful** session keeps
    /// its facts, which is what makes it long-lived. The flag was stored and never read, so both modes
    /// behaved identically and "stateless" was simply a word in a signature.
    pub fn execute(&mut self) -> Result<ExecutionResult, String> {
        if self.destroyed {
            return Err("Session has been destroyed".to_string());
        }
        let report = crate::forward_chaining::run(&mut self.engine);
        if !self.stateful {
            self.engine.working_memory_mut().replace_all(Vec::new());
        }
        Ok(ExecutionResult::new(
            report.errors.is_empty(),
            format!(
                "{} rule(s) fired, {} fact(s) changed",
                report.rules_fired,
                report.facts_added.len() + report.facts_retracted.len()
            ),
        ))
    }

    /// Destroys the session and releases resources.
    pub fn destroy(&mut self) {
        self.destroyed = true;
    }

    /// Checks if the session is still valid.
    pub fn is_valid(&self) -> bool {
        !self.destroyed
    }

    /// Gets the session ID if set.
    pub fn id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }
}

/// Parse a rules document into rules.
///
/// Accepted shapes, all of them formats this project has written over time:
///
/// - `{ "rules": [ … ] }` — what a `.rules` file holds and what `POST /v1/rules` writes;
/// - `[ … ]` — several rules;
/// - `{ … }` — one rule.
///
/// **This is the only parser for that format.** `build_ruleset_from_dir` in the API had its own, accepting
/// only the bare-rule and array shapes, so a file written in the envelope form — by this project's own
/// `POST /v1/rules` — was silently skipped at reload (9.0.74). One implementation, two callers.
pub fn parse_rules_document(text: &str) -> Result<Vec<crate::rules::Rule>, String> {
    match serde_json::from_str::<LoadedRules>(text) {
        Ok(LoadedRules::Document { rules }) => Ok(rules),
        Ok(LoadedRules::Many(rules)) => Ok(rules),
        Ok(LoadedRules::One(rule)) => Ok(vec![*rule]),
        Err(error) => Err(format!("rules are not valid JSON: {error}")),
    }
}

/// The JSON shapes a rule document may take.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum LoadedRules {
    /// `{ "rules": [ … ] }` — what a `.rules` file holds.
    Document { rules: Vec<crate::rules::Rule> },
    /// `[ … ]` — several rules.
    Many(Vec<crate::rules::Rule>),
    /// `{ … }` — one rule.
    One(Box<crate::rules::Rule>),
}

/// Result of a rule execution.
pub struct ExecutionResult {
    /// Whether execution succeeded
    success: bool,
    /// Message or error details
    message: String,
}

impl ExecutionResult {
    fn new(success: bool, message: String) -> Self {
        Self { success, message }
    }

    /// Checks if execution was successful.
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Gets the result message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Trace of inference steps for backward chaining.
pub struct InferenceTrace {
    /// Whether the query succeeded
    success: bool,
    /// Reasoning path
    path: String,
}

impl InferenceTrace {
    fn new(success: bool, path: String) -> Self {
        Self { success, path }
    }

    /// Checks if inference succeeded.
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Gets the reasoning path.
    pub fn path(&self) -> &str {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_builder_defaults() {
        let engine = EngineBuilder::new().build();
        assert_eq!(engine.chaining_mode, ChainingMode::Automatic);
        assert_eq!(engine.max_iterations, 1000);
    }

    #[test]
    fn test_engine_builder_custom() {
        let engine = EngineBuilder::new()
            .chaining_mode(ChainingMode::ForwardOnly)
            .max_iterations(500)
            .enable_debug_trace(true)
            .build();

        assert_eq!(engine.chaining_mode, ChainingMode::ForwardOnly);
        assert_eq!(engine.max_iterations, 500);
    }

    #[test]
    fn test_session_lifecycle() {
        let engine = EngineBuilder::new().build();
        let mut session = engine.create_session(true, Some("test-session".to_string()));

        assert!(session.is_valid());
        assert_eq!(session.id(), Some("test-session"));

        session.destroy();
        assert!(!session.is_valid());
    }

    #[test]
    fn test_stateless_session() {
        let engine = EngineBuilder::new().build();
        let session = engine.create_session(false, None);

        // Stateless sessions should have no ID
        assert_eq!(session.id(), None);
    }
}
