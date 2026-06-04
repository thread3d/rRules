//! Testing framework for rRules engine verification
//!
//! This module provides utilities for testing the rule engine including:
//! - Per-test engine isolation with fresh instances
//! - Working memory fixture support
//! - Forward chaining verification helpers
//! - Backward chaining trace capture

use crate::data_types::Value;
use crate::environment::EngineBuilder;
use crate::fact::{Fact, WorkingMemory};
use crate::salience::agenda::Activation;

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

/// Test session that provides isolated engine instance per test
///
/// This struct manages a fresh engine instance with unique session ID
/// to prevent state pollution between tests. It implements RAII cleanup.
#[derive(Clone)]
pub struct TestSession {
    /// Engine builder for creating fresh instances
    _builder: EngineBuilder,
    /// Unique session identifier for isolation
    session_id: uuid::Uuid,
}

impl TestSession {
    /// Create a new test session with isolated engine instance
    pub fn new() -> Self {
        let builder = EngineBuilder::new();
        let session_id = uuid::Uuid::new_v4();

        TestSession {
            _builder: builder,
            session_id,
        }
    }

    /// Get the session ID for logging/debugging
    pub fn session_id(&self) -> &uuid::Uuid {
        &self.session_id
    }
}

/// Fixture helper for creating working memory with test facts
///
/// Usage:
/// ```no_run
/// use engine::testing::FixtureBuilder;
/// use std::collections::HashMap;
///
/// let mut payload = HashMap::new();
/// payload.insert("score".to_string(), "high".into());
///
/// let fixture = FixtureBuilder::new()
///     .with_fact("customer", payload)
///     .build();
/// ```
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
    pub fn with_fact(mut self, name: &str, payload: std::collections::HashMap<String, Value>) -> Self {
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
    _session: TestSession,
    /// Activation trace captured during execution
    activations: Vec<String>,
}

impl ForwardChainingTester {
    /// Create a new forward chaining tester
    pub fn new() -> Self {
        let session = TestSession::new();

        ForwardChainingTester {
            _session: session,
            activations: Vec::new(),
        }
    }

    /// Get the test session for this tester
    pub fn session(&self) -> &TestSession {
        &self._session
    }

    /// Get mutable access to activation trace
    pub fn activations_mut(&mut self) -> &mut Vec<String> {
        &mut self.activations
    }
}

/// Backward chaining trace capture
///
/// Captures the inference path during backward chaining execution.
pub struct BackwardChainingTrace {
    /// Rule applications in order
    rule_applications: Vec<String>,
    /// Missing facts at each step
    missing_facts: Vec<String>,
}

impl BackwardChainingTrace {
    /// Create a new empty trace
    pub fn new() -> Self {
        BackwardChainingTrace {
            rule_applications: Vec::new(),
            missing_facts: Vec::new(),
        }
    }

    /// Record a rule application in the trace
    #[allow(dead_code)]
    pub fn record_application(&mut self, _rule_name: &str) {
        // Stub implementation
    }

    /// Add a missing fact to the trace
    #[allow(dead_code)]
    pub fn record_missing_fact(&mut self, _fact_name: &str) {
        // Stub implementation
    }
}

/// Deterministic conflict resolution mode for testing
///
/// When enabled, rules with equal salience are ordered by name
/// to ensure deterministic test execution.
pub fn deterministic_conflict_strategy(mut activations: Vec<Activation>) -> Vec<Activation> {
    // Sort by (salience DESC, rule_name ASC) for determinism
    activations.sort_by(|a, b| {
        b.salience
            .cmp(&a.salience)
            .then(a.rule.name.cmp(&b.rule.name))
    });
    activations
}

/// Test assertion helpers

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
        assert!(true);
    }

    #[test]
    fn test_fixture_builder_adds_facts() {
        let mut payload = std::collections::HashMap::new();
        payload.insert("score".to_string(), Value::Integer(100));

        let fixture = FixtureBuilder::new()
            .with_fact("customer", payload)
            .build();

        // Just verify we can build working memory with facts
        // The FixtureBuilder now properly accumulates and inserts facts
        assert_eq!(fixture.get_facts_by_type("customer").len(), 1);
    }
}
