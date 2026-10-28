//! Testing framework for rRules engine verification
//!
//! This module provides utilities for testing the rule engine including:
//! - Per-test engine isolation with fresh instances
//! - Working memory fixture support
//! - Forward chaining verification helpers
//! - Backward chaining trace capture

use crate::data_types::Value;
use crate::environment::{Engine, EngineBuilder};
use crate::fact::{Fact, WorkingMemory};
use crate::salience::agenda::Activation;
use serde_json;

// Re-export backward chaining tracing types for testing use
pub use crate::backward_chaining::{InferenceStep, InferenceTrace, StepType};

// Re-export snapshot-related types from their modules
pub use crate::testing::json::{
    deserialize_working_memory, roundtrip_snapshot, serialize_deterministic,
};
pub use crate::testing::snapshot::{SnapshotError, SnapshotManager, SnapshotMode, SnapshotResult};

// Module declarations (must be after re-exports for visibility)
pub mod json;
pub mod snapshot;

/// Test error type for testing framework
#[derive(Debug, thiserror::Error)]
pub enum TestError {
    #[error("Test assertion failed: {0}")]
    AssertionFailed(String),
    #[error("Firing limit exceeded (max {0} firings)")]
    FiringLimitExceeded(usize),
}

/// Result type for testing framework
pub type TestResult<T> = Result<T, TestError>;

// Working memory snapshot types for serialization/testing
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum ValueSnapshot {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    DateTime(String),
    #[default]
    Null,
    Array(Vec<ValueSnapshot>),
    Object(std::collections::HashMap<String, ValueSnapshot>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactSnapshot {
    pub id: String,
    #[serde(rename = "type")]
    pub fact_type: String,
    pub payload: std::collections::HashMap<String, ValueSnapshot>,
}

impl FactSnapshot {
    /// Create a FactSnapshot from a Fact
    pub fn from_fact(fact: &crate::fact::Fact) -> Self {
        let payload = fact
            .payload
            .iter()
            .map(|(k, v)| (k.clone(), ValueSnapshot::from_value(v)))
            .collect();

        FactSnapshot {
            id: fact.id.to_string(),
            fact_type: fact.fact_type_id.clone(),
            payload,
        }
    }
}

impl ValueSnapshot {
    /// Convert a Value to ValueSnapshot
    pub fn from_value(value: &crate::data_types::Value) -> Self {
        match value {
            crate::data_types::Value::Bool(b) => ValueSnapshot::Bool(*b),
            crate::data_types::Value::Integer(i) => ValueSnapshot::Integer(*i),
            crate::data_types::Value::Float(f) => ValueSnapshot::Float(*f),
            crate::data_types::Value::String(s) => ValueSnapshot::String(s.clone()),
            crate::data_types::Value::Null => ValueSnapshot::Null,
            crate::data_types::Value::DateTime(dt) => ValueSnapshot::DateTime(format!("{:?}", dt)),
            crate::data_types::Value::Array(arr) => {
                ValueSnapshot::Array(arr.iter().map(ValueSnapshot::from_value).collect())
            }
            crate::data_types::Value::Object(obj) => ValueSnapshot::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), ValueSnapshot::from_value(v)))
                    .collect(),
            ),
            // FEEL variants (Phase 30) — no natural ValueSnapshot
            // representation. Map to a readable string marker via Debug so the
            // snapshot carries a human-readable value rather than panicking.
            // The compiler forces these arms (no wildcard — exhaustive match
            // is the safety net, 30-RESEARCH.md Pitfall 1).
            crate::data_types::Value::Number(d) => ValueSnapshot::String(format!("{:?}", d)),
            crate::data_types::Value::Date(d) => ValueSnapshot::String(format!("{:?}", d)),
            crate::data_types::Value::Time(t) => ValueSnapshot::String(format!("{:?}", t)),
            crate::data_types::Value::DateTimeTz(dt) => ValueSnapshot::String(format!("{:?}", dt)),
            crate::data_types::Value::DaysTimeDuration(td) => {
                ValueSnapshot::String(format!("{:?}", td))
            }
            crate::data_types::Value::YearsMonthsDuration(ymd) => {
                ValueSnapshot::String(format!("{:?}", ymd))
            }
            crate::data_types::Value::Range(r) => ValueSnapshot::String(format!("{:?}", r)),
            crate::data_types::Value::Function(f) => ValueSnapshot::String(format!("{:?}", f)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemorySnapshot {
    pub facts: Vec<FactSnapshot>,
}

impl WorkingMemorySnapshot {
    /// Create a snapshot from working memory
    pub fn from_working_memory(memory: &WorkingMemory) -> Self {
        let facts = memory
            .get_all_facts()
            .into_iter()
            .map(FactSnapshot::from_fact)
            .collect();
        WorkingMemorySnapshot { facts }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(_json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(_json)
    }

    pub fn compare(&self, other: &Self) -> SnapshotDiff {
        let mut diff = SnapshotDiff::new();

        // Create a map of fact_type to payload for easier comparison
        let self_map: std::collections::HashMap<
            String,
            &std::collections::HashMap<String, ValueSnapshot>,
        > = self
            .facts
            .iter()
            .map(|f| (f.fact_type.clone(), &f.payload))
            .collect();
        let other_map: std::collections::HashMap<
            String,
            &std::collections::HashMap<String, ValueSnapshot>,
        > = other
            .facts
            .iter()
            .map(|f| (f.fact_type.clone(), &f.payload))
            .collect();

        // Check for added facts
        for fact in &self.facts {
            if !other_map.contains_key(&fact.fact_type) {
                diff.added.push(fact.fact_type.clone());
            }
        }

        // Check for removed facts
        for fact in &other.facts {
            if !self_map.contains_key(&fact.fact_type) {
                diff.removed.push(fact.fact_type.clone());
            }
        }

        // Check for changed payloads
        for fact in &self.facts {
            if let Some(other_payload) = other_map.get(&fact.fact_type) {
                let self_json = serde_json::to_string_pretty(&fact.payload).unwrap_or_default();
                let other_json = serde_json::to_string_pretty(other_payload).unwrap_or_default();
                if self_json != other_json {
                    diff.changed.push(SnapshotChange {
                        fact_type: fact.fact_type.clone(),
                        old_payload: fact.payload.clone(),
                        new_payload: (*other_payload).clone(),
                    });
                }
            }
        }

        diff
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<SnapshotChange>,
}

impl Default for SnapshotDiff {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotDiff {
    pub fn new() -> Self {
        SnapshotDiff {
            added: Vec::new(),
            removed: Vec::new(),
            changed: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    pub fn describe(&self) -> String {
        if self.is_empty() {
            return "No differences".to_string();
        }
        let mut lines = Vec::new();
        if !self.added.is_empty() {
            lines.push(format!("Added: {}", self.added.join(", ")));
        }
        if !self.removed.is_empty() {
            lines.push(format!("Removed: {}", self.removed.join(", ")));
        }
        for change in &self.changed {
            lines.push(format!(
                "Changed '{}': {} -> {}",
                change.fact_type,
                format_payload(&change.old_payload),
                format_payload(&change.new_payload)
            ));
        }
        lines.join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotChange {
    pub fact_type: String,
    pub old_payload: std::collections::HashMap<String, ValueSnapshot>,
    pub new_payload: std::collections::HashMap<String, ValueSnapshot>,
}

// Helper function to format payload for display
fn format_payload(payload: &std::collections::HashMap<String, ValueSnapshot>) -> String {
    let entries: Vec<String> = payload
        .iter()
        .map(|(k, v)| format!("{}={}", k, value_snapshot_to_string(v)))
        .collect();
    format!("{{{}}}", entries.join(", "))
}

fn value_snapshot_to_string(value: &ValueSnapshot) -> String {
    match value {
        ValueSnapshot::Bool(b) => b.to_string(),
        ValueSnapshot::Integer(i) => i.to_string(),
        ValueSnapshot::Float(f) => format!("{:.2}", f),
        ValueSnapshot::String(s) => s.clone(),
        ValueSnapshot::DateTime(dt) => dt.clone(),
        ValueSnapshot::Null => "null".to_string(),
        ValueSnapshot::Array(arr) => {
            let items: Vec<String> = arr.iter().map(value_snapshot_to_string).collect();
            format!("[{}]", items.join(", "))
        }
        ValueSnapshot::Object(obj) => {
            let entries: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{}: {}", k, value_snapshot_to_string(v)))
                .collect();
            format!("{{{}}}", entries.join(", "))
        }
    }
}

/// Test session that provides isolated engine instance per test
///
/// This struct manages a fresh engine instance with unique session ID
/// to prevent state pollution between tests. It implements RAII cleanup.
#[derive(Clone)]
pub struct TestSession {
    /// Engine instance for rule execution
    engine: Engine,
    /// Unique session identifier for isolation
    session_id: uuid::Uuid,
    /// Deterministic mode setting for conflict resolution
    deterministic_mode: DeterministicMode,
}

impl Default for TestSession {
    fn default() -> Self {
        Self::new()
    }
}

impl TestSession {
    /// Create a new test session with default (non-deterministic) conflict resolution
    pub fn new() -> Self {
        Self::new_with_deterministic(DeterministicMode::Disabled)
    }

    /// Create a new test session with specified deterministic mode
    pub fn new_with_deterministic(mode: DeterministicMode) -> Self {
        let builder = EngineBuilder::new();
        let session_id = uuid::Uuid::new_v4();

        TestSession {
            engine: builder.build(),
            session_id,
            deterministic_mode: mode,
        }
    }

    /// Get the current deterministic mode
    pub fn deterministic_mode(&self) -> DeterministicMode {
        self.deterministic_mode
    }

    /// Get the session ID for logging/debugging
    pub fn session_id(&self) -> &uuid::Uuid {
        &self.session_id
    }

    /// Access the underlying engine instance (immutable)
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Get mutable access to the underlying engine instance for testing
    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    /// Insert a fact into this session's working memory
    pub fn insert_fact(&mut self, fact: Fact) {
        let _ = self.engine.working_memory_mut().insert_fact(fact);
    }

    /// Execute backward chaining from a goal and capture the trace
    pub fn execute_backward_chaining(&self, goal: &str) -> InferenceTrace {
        // Clone engine for execution (Engine implements Clone)
        let temp_engine = self.engine.clone();
        let session = crate::environment::Session::new(&temp_engine, false, None);

        // Use the public execute_backward from backward_chaining
        crate::backward_chaining::execute_backward(&temp_engine, &session, goal)
    }
}

/// Fixture helper for creating working memory with test facts
///
/// Usage:
/// ```no_run
/// use engine::testing::FixtureBuilder;
/// use engine::data_types::Value;
/// use std::collections::HashMap;
///
/// let mut payload = HashMap::new();
/// payload.insert("score".to_string(), Value::String("high".to_string()));
///
/// let fixture = FixtureBuilder::new()
///     .with_fact("customer", payload)
///     .build();
/// ```
#[derive(Default)]
pub struct FixtureBuilder {
    facts: Vec<Fact>,
}

impl FixtureBuilder {
    /// Create a new fixture builder
    pub fn new() -> Self {
        FixtureBuilder { facts: Vec::new() }
    }

    /// Add a fact to the fixture with the given name and payload
    ///
    /// The fact will be assigned a unique ID and its type derived from the name.
    pub fn with_fact(
        mut self,
        name: &str,
        payload: std::collections::HashMap<String, Value>,
    ) -> Self {
        let fact = Fact::new(name.to_string(), payload);
        self.facts.push(fact);
        self
    }

    /// Build the fixture with all added facts into a WorkingMemory instance
    pub fn build(self) -> WorkingMemory {
        let mut memory = WorkingMemory::new();
        for fact in self.facts {
            let _ = memory.insert_fact(fact);
        }
        memory
    }
}

/// Forward chaining test helper
///
/// Provides utilities to verify forward chaining execution:
/// - Track activation sequence
/// - Verify salience ordering
/// - Capture firing traces
pub struct ForwardChainingTester {
    /// Session for isolated engine instance
    session: TestSession,
    /// Activation trace captured during execution (rule names in order)
    activations: Vec<String>,
}

impl Default for ForwardChainingTester {
    fn default() -> Self {
        Self::new()
    }
}

impl ForwardChainingTester {
    /// Create a new forward chaining tester with an isolated engine instance
    pub fn new() -> Self {
        let session = TestSession::new();

        ForwardChainingTester {
            session,
            activations: Vec::new(),
        }
    }

    /// Get the test session for this tester
    pub fn session(&self) -> &TestSession {
        &self.session
    }

    /// Get mutable access to the underlying engine instance
    pub fn engine_mut(&mut self) -> &mut Engine {
        self.session.engine_mut()
    }

    /// Get mutable access to activation trace
    pub fn activations_mut(&mut self) -> &mut Vec<String> {
        &mut self.activations
    }

    /// Execute forward chaining with the current rules and capture activations
    ///
    /// This method calls the engine's execute_forward function.
    /// The activation trace is populated during execution.
    pub fn execute_forward(&mut self) -> crate::forward_chaining::ExecutionResult {
        // Reset activation trace before new execution
        self.activations.clear();

        // Clone the engine for execution (Engine implements Clone)
        let mut temp_engine = self.session.engine.clone();
        // Create a temporary session for the engine
        let session = crate::environment::Session::new(&temp_engine, false, None);

        let report = crate::forward_chaining::execute_forward(&mut temp_engine, &session);

        // Populate the trace the assertions read. This was the whole point of the type and it never
        // happened: the report was returned and the trace left empty, so `assert_fired_count(0)` and
        // `assert_rule_not_fired(anything)` passed for every engine, however it behaved.
        //
        // `activation_trace` documents rule *names*, while the report carries ids, so map them here.
        let names: std::collections::HashMap<&str, &str> = temp_engine
            .rules()
            .iter()
            .map(|rule| (rule.id.as_str(), rule.name.as_str()))
            .collect();
        for id in &report.fired_rules {
            let name = names.get(id.as_str()).copied().unwrap_or(id.as_str());
            self.activations.push(name.to_string());
        }

        report
    }

    /// Add a rule to the engine's rule set for testing.
    ///
    /// This used to build the **DSL** text `"rule <id>"` and hand it to `load_rules`, which parses
    /// **JSON** — so the call always failed, the error was discarded with `let _ =`, and the rule was
    /// never added. A tester whose rules silently do not exist cannot fail a test, which makes it worse
    /// than no tester at all.
    pub fn add_rule(&mut self, rule: crate::rules::Rule) -> TestResult<()> {
        self.engine_mut()
            .add_rule(rule)
            .map_err(TestError::AssertionFailed)
    }

    /// Get the activation trace (rule names in firing order)
    pub fn activation_trace(&self) -> &[String] {
        &self.activations
    }

    /// Get the count of activations captured
    pub fn firing_count(&self) -> usize {
        self.activations.len()
    }

    /// Assert that rules fired in the expected sequence
    ///
    /// Returns an error if the actual trace doesn't match the expected sequence.
    pub fn assert_rule_sequence(&mut self, expected: &[&str]) -> TestResult<()> {
        let actual: Vec<&str> = self.activations.iter().map(|s| s.as_str()).collect();
        if actual != expected {
            return Err(TestError::AssertionFailed(format!(
                "Rule sequence mismatch.\nExpected: {:?}\nActual:   {:?}",
                expected, actual
            )));
        }
        Ok(())
    }

    /// Assert that the exact number of rules fired
    ///
    /// Returns an error if the count doesn't match.
    pub fn assert_fired_count(&mut self, expected: usize) -> TestResult<()> {
        let actual = self.activations.len();
        if actual != expected {
            return Err(TestError::AssertionFailed(format!(
                "Fired count mismatch.\nExpected: {}\nActual:   {}",
                expected, actual
            )));
        }
        Ok(())
    }

    /// Assert that rules fired in the specified order (not necessarily consecutive)
    ///
    /// Returns an error if the relative ordering is incorrect.
    pub fn assert_fired_in_order(&mut self, expected: &[&str]) -> TestResult<()> {
        let actual: Vec<&str> = self.activations.iter().map(|s| s.as_str()).collect();

        let mut last_pos = 0;
        for rule in expected {
            match actual[last_pos..].iter().position(|&r| r == *rule) {
                Some(pos) => last_pos += pos + 1,
                None => {
                    return Err(TestError::AssertionFailed(format!(
                        "Rule '{}' not found or out of order. Expected: {:?}, Actual: {:?}",
                        rule, expected, actual
                    )));
                }
            }
        }
        Ok(())
    }

    /// Assert that a specific rule did NOT fire
    ///
    /// Returns an error if the rule appears in the activation trace.
    pub fn assert_rule_not_fired(&mut self, rule_name: &str) -> TestResult<()> {
        if self.activations.contains(&rule_name.to_string()) {
            return Err(TestError::AssertionFailed(format!(
                "Rule '{}' was not expected to fire but appeared in trace",
                rule_name
            )));
        }
        Ok(())
    }

    /// Add multiple rules to the engine from an iterator for fluent API usage
    pub fn with_rules<I>(&mut self, rules: I) -> &mut Self
    where
        I: IntoIterator<Item = crate::rules::Rule>,
    {
        for rule in rules {
            if let Err(error) = self.add_rule(rule) {
                // `with_rules` is a fluent builder; it stops at the first rule the engine rejects and
                // leaves the tester holding the rules it did accept.
                eprintln!("ForwardChainingTester::with_rules: {error}");
                break;
            }
        }
        self
    }

    /// Clear the activation trace (for multiple test runs)
    pub fn clear_trace(&mut self) {
        self.activations.clear();
    }

    /// Verify that a specific step exists in the trace
    pub fn assert_trace_contains_step(
        trace: &InferenceTrace,
        expected_desc: &str,
    ) -> TestResult<()> {
        if !trace.path.iter().any(|s| s.description == expected_desc) {
            return Err(TestError::AssertionFailed(format!(
                "Trace missing step with description '{}'",
                expected_desc
            )));
        }
        Ok(())
    }

    /// Verify the trace represents a successful inference
    pub fn assert_trace_success(trace: &InferenceTrace) -> TestResult<()> {
        if !trace.success {
            return Err(TestError::AssertionFailed(
                "Trace does not represent success".to_string(),
            ));
        }
        Ok(())
    }

    /// Verify the trace path has expected length
    pub fn assert_trace_path_length(trace: &InferenceTrace, expected_len: usize) -> TestResult<()> {
        if trace.path.len() != expected_len {
            return Err(TestError::AssertionFailed(format!(
                "Trace path length mismatch. Expected {}, got {}",
                expected_len,
                trace.path.len()
            )));
        }
        Ok(())
    }

    /// Find a rule application step by rule ID
    pub fn find_rule_application<'a>(
        trace: &'a InferenceTrace,
        rule_id: &str,
    ) -> Option<&'a InferenceStep> {
        trace
            .path
            .iter()
            .find(|s| s.rule_id.as_deref() == Some(rule_id))
    }

    /// Check if trace contains a fact match step
    pub fn has_fact_match_step(trace: &InferenceTrace) -> bool {
        trace
            .path
            .iter()
            .any(|s| s.step_type == StepType::FactMatch)
    }
}

/// Deterministic conflict resolution mode for testing
///
/// When enabled, rules with equal salience are ordered by name
/// to ensure deterministic test execution.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DeterministicMode {
    /// Use default conflict resolution (non-deterministic for equal salience)
    #[default]
    Disabled,
    /// Sort by rule name when salience is equal
    EnabledByName,
}

pub fn deterministic_conflict_strategy(mut activations: Vec<Activation>) -> Vec<Activation> {
    // Sort by (salience DESC, rule_name ASC) for determinism
    activations.sort_by(|a, b| {
        b.salience
            .cmp(&a.salience)
            .then(a.rule.name.cmp(&b.rule.name))
    });
    activations
}

/// Test assertion helpers.
///
/// Assert that two values are equal, returning a test result
#[macro_export]
macro_rules! assert_eq_test {
    ($left:expr, $right:expr) => {{
        let left = $left;
        let right = $right;
        if left != right {
            return Err(TestError::AssertionFailed(format!(
                "assertion failed: `(left == right)`\n  left: `{:?}`\n right: `{:?}`",
                left, right
            )));
        }
    }};
    ($left:expr, $right:expr, $($arg:tt)+) => {{
        let left = $left;
        let right = $right;
        if left != right {
            return Err(TestError::AssertionFailed(format!(
                "assertion failed: `(left == right)`\n  left: `{:?}`\n right: `{:?}`\n  {}",
                left, right, format_args!($($arg)+)
            )));
        }
    }};
}

/// Assert that an expression is true, returning a test result
#[macro_export]
macro_rules! assert_test {
    ($cond:expr) => {{
        if !$cond {
            return Err(TestError::AssertionFailed(
                "assertion failed: condition was false".to_string(),
            ));
        }
    }};
    ($cond:expr, $($arg:tt)+) => {{
        if !$cond {
            return Err(TestError::AssertionFailed(format!(
                "assertion failed: {}",
                format_args!($($arg)+)
            )));
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creates_unique_ids() {
        let session1 = TestSession::new();
        let session2 = TestSession::new();

        assert_ne!(session1.session_id(), session2.session_id());
    }

    #[test]
    fn test_fixture_builder_creates_working_memory() {
        // Just verify we can create a working memory
        WorkingMemory::new();
    }

    #[test]
    fn test_fixture_builder_adds_facts() {
        let mut payload = std::collections::HashMap::new();
        payload.insert("score".to_string(), Value::Integer(100));

        let fixture = FixtureBuilder::new().with_fact("customer", payload).build();

        // Just verify we can build working memory with facts
        // The FixtureBuilder now properly accumulates and inserts facts
        assert_eq!(fixture.get_facts_by_type("customer").len(), 1);
    }

    #[test]
    fn test_forward_chaining_tester_new() {
        let tester = ForwardChainingTester::new();
        assert_eq!(tester.firing_count(), 0);
    }

    #[test]
    fn test_assert_rule_sequence() {
        let mut tester = ForwardChainingTester::new();
        tester
            .activations_mut()
            .extend(vec!["a".to_string(), "b".to_string(), "c".to_string()]);

        assert!(tester.assert_rule_sequence(&["a", "b", "c"]).is_ok());
        assert!(tester.assert_rule_sequence(&["a", "b"]).is_err());
    }

    #[test]
    fn test_assert_fired_count() {
        let mut tester = ForwardChainingTester::new();
        tester
            .activations_mut()
            .extend(vec!["a".to_string(), "b".to_string()]);

        assert!(tester.assert_fired_count(2).is_ok());
        assert!(tester.assert_fired_count(3).is_err());
    }

    #[test]
    fn test_assert_fired_in_order() {
        let mut tester = ForwardChainingTester::new();
        tester.activations_mut().extend(vec![
            "a".to_string(),
            "x".to_string(),
            "b".to_string(),
            "y".to_string(),
            "c".to_string(),
        ]);

        // Partial order should pass
        assert!(tester.assert_fired_in_order(&["a", "b", "c"]).is_ok());
        // Wrong order should fail
        assert!(tester.assert_fired_in_order(&["b", "a", "c"]).is_err());
    }

    #[test]
    fn test_assert_rule_not_fired() {
        let mut tester = ForwardChainingTester::new();
        tester
            .activations_mut()
            .extend(vec!["a".to_string(), "b".to_string()]);

        assert!(tester.assert_rule_not_fired("c").is_ok());
        assert!(tester.assert_rule_not_fired("a").is_err());
    }

    /// A rule the tester is given must reach the engine and fire.
    ///
    /// The tester's own tests used to *inject* activations by hand, which is why the type passed them
    /// while `add_rule` silently discarded every rule and `execute_forward` never recorded anything:
    /// the assertions were tested, the tester was not.
    #[test]
    fn a_rule_added_to_the_tester_fires_and_is_recorded() {
        use crate::data_types::Value;
        use crate::rules::{Action, Condition, LogLevel, Rule};
        use std::collections::HashMap;

        let mut tester = ForwardChainingTester::new();
        tester
            .add_rule(Rule {
                id: "gold".to_string(),
                name: "gold discount".to_string(),
                salience: 0,
                no_loop: false,
                lock_on_active: false,
                agenda_group: None,
                condition: Condition::Pattern {
                    fact_type: "customer".to_string(),
                    field: "tier".to_string(),
                    value: Value::String("Gold".to_string()),
                    op: crate::rules::ComparisonOp::Eq,
                },
                actions: vec![Action::LogMessage {
                    level: LogLevel::Info,
                    message: "gold".to_string(),
                }],
            })
            .expect("the tester must accept a valid rule");

        let mut payload = HashMap::new();
        payload.insert("tier".to_string(), Value::String("Gold".to_string()));
        tester
            .engine_mut()
            .insert_fact_for_simulation(crate::fact::Fact::new("customer", payload))
            .expect("fact");

        let report = tester.execute_forward();

        assert_eq!(report.rules_fired, 1, "the rule the facts satisfy fired");
        assert_eq!(
            tester.activation_trace(),
            ["gold discount"],
            "the trace records the rule's *name*, which is what the assertions compare"
        );
        tester.assert_fired_count(1).expect("one rule fired");
        tester
            .assert_rule_sequence(&["gold discount"])
            .expect("in that order");
        tester
            .assert_rule_not_fired("bronze discount")
            .expect("a rule that does not exist cannot have fired");
        assert!(
            tester.assert_fired_count(2).is_err(),
            "and the assertions can still fail"
        );
    }

    /// A rule the facts do not satisfy must not appear in the trace — the assertion that used to pass
    /// for every engine, however it behaved.
    #[test]
    fn a_rule_the_facts_do_not_satisfy_is_not_recorded() {
        use crate::data_types::Value;
        use crate::rules::{Action, ComparisonOp, Condition, LogLevel, Rule};
        use std::collections::HashMap;

        let mut tester = ForwardChainingTester::new();
        tester
            .add_rule(Rule {
                id: "platinum".to_string(),
                name: "platinum discount".to_string(),
                salience: 0,
                no_loop: false,
                lock_on_active: false,
                agenda_group: None,
                condition: Condition::Pattern {
                    fact_type: "customer".to_string(),
                    field: "tier".to_string(),
                    value: Value::String("Platinum".to_string()),
                    op: ComparisonOp::Eq,
                },
                actions: vec![Action::LogMessage {
                    level: LogLevel::Info,
                    message: "platinum".to_string(),
                }],
            })
            .expect("rule");

        let mut payload = HashMap::new();
        payload.insert("tier".to_string(), Value::String("Gold".to_string()));
        tester
            .engine_mut()
            .insert_fact_for_simulation(crate::fact::Fact::new("customer", payload))
            .expect("fact");

        tester.execute_forward();

        assert_eq!(
            tester.firing_count(),
            0,
            "nothing matched, so nothing fired"
        );
        tester
            .assert_fired_count(0)
            .expect("zero is the right count");
        tester
            .assert_rule_not_fired("platinum discount")
            .expect("the rule did not fire");
    }

    #[test]
    fn test_clear_trace() {
        let mut tester = ForwardChainingTester::new();
        tester.activations_mut().push("test".to_string());
        assert_eq!(tester.firing_count(), 1);

        tester.clear_trace();
        assert_eq!(tester.firing_count(), 0);
    }
}
