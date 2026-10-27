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
    /// Default salience for rules without explicit value
    default_salience: i32,
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
            default_salience: 0,
            max_iterations: 1000,
            enable_debug_trace: false,
        }
    }

    /// Sets the chaining mode selection strategy.
    pub fn chaining_mode(mut self, mode: ChainingMode) -> Self {
        self.chaining_mode = mode;
        self
    }

    /// Sets default salience for rules without explicit value.
    pub fn default_salience(mut self, salience: i32) -> Self {
        self.default_salience = salience;
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
            default_salience: self.default_salience,
            max_iterations: self.max_iterations,
            enable_debug_trace: self.enable_debug_trace,
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
#[allow(dead_code)]
pub struct Engine {
    /// Working memory for facts
    working_memory: WorkingMemory,
    /// Set of compiled rules
    rule_set: RuleSet,
    /// Chaining mode selection strategy
    chaining_mode: ChainingMode,
    /// Default salience for rules without explicit value
    default_salience: i32,
    /// Maximum iterations before loop detection triggers
    max_iterations: usize,
    /// Enable detailed debug tracing
    enable_debug_trace: bool,
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

    /// Executes forward chaining with the current rules and facts.
    /// The rules this engine holds.
    pub(crate) fn rules(&self) -> &[crate::rules::Rule] {
        &self.rule_set.rules
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

    /// Executes backward chaining from a goal.
    pub fn execute_backward(&self, _goal: &str) -> InferenceTrace {
        // This is a placeholder - full implementation would:
        // 1. Start from the query/goal
        // 2. Recursively match rule conditions
        // 3. Build inference tree showing reasoning path
        InferenceTrace::new(false, "Backward chaining not yet implemented".to_string())
    }
}

/// A session manages execution state for a single rule application.
#[allow(dead_code)]
pub struct Session {
    /// Reference to the engine
    engine: *const Engine,
    /// Stateful vs stateless mode
    stateful: bool,
    /// Optional explicit ID
    session_id: Option<String>,
    /// Track if session has been destroyed
    destroyed: bool,
}

impl Session {
    /// Creates a new session from the engine.
    #[allow(dead_code)]
    pub fn new(engine: &Engine, stateful: bool, session_id: Option<String>) -> Self {
        Self {
            engine: engine as *const Engine,
            stateful,
            session_id,
            destroyed: false,
        }
    }

    /// Inserts a fact into this session's working memory.
    pub fn insert_fact(&mut self, _fact: crate::fact::Fact) -> Result<(), String> {
        if self.destroyed {
            return Err("Session has been destroyed".to_string());
        }
        // Get engine from raw pointer and call its methods
        // Safety: This assumes the engine lives longer than the session
        Ok(())
    }

    /// Executes rules in this session.
    pub fn execute(&mut self) -> Result<ExecutionResult, String> {
        if self.destroyed {
            return Err("Session has been destroyed".to_string());
        }
        // Execute using the engine's forward chaining
        Ok(ExecutionResult::new(true, "Not implemented".to_string()))
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
        assert_eq!(engine.default_salience, 0);
        assert_eq!(engine.max_iterations, 1000);
    }

    #[test]
    fn test_engine_builder_custom() {
        let engine = EngineBuilder::new()
            .chaining_mode(ChainingMode::ForwardOnly)
            .default_salience(10)
            .max_iterations(500)
            .enable_debug_trace(true)
            .build();

        assert_eq!(engine.chaining_mode, ChainingMode::ForwardOnly);
        assert_eq!(engine.default_salience, 10);
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
