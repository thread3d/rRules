//! Goal-driven backward chaining for interactive debugging.
//!
//! This module implements the execute_backward function that starts from a query
//! and traces the reasoning path to supporting facts.

use crate::environment::{Engine, Session};

/// Result of a backward chaining inference.
pub struct InferenceTrace {
    /// Whether the query succeeded
    pub success: bool,
    /// Full reasoning path showing how conclusion was reached
    pub path: Vec<InferenceStep>,
    /// Facts that matched at each step
    pub fact_matches: Vec<String>,
}

impl InferenceTrace {
    pub fn new(success: bool) -> Self {
        Self {
            success,
            path: Vec::new(),
            fact_matches: Vec::new(),
        }
    }

    pub fn with_step(mut self, step: InferenceStep) -> Self {
        self.path.push(step);
        self
    }

    /// Generates a human-readable explanation of the reasoning.
    pub fn explain(&self) -> String {
        let mut lines = vec!["Inference Trace:".to_string()];

        for (i, step) in self.path.iter().enumerate() {
            lines.push(format!("  {}. {}", i + 1, step.description));
        }

        if !self.fact_matches.is_empty() {
            lines.push("\nMatching Facts:".to_string());
            for fact in &self.fact_matches {
                lines.push(format!("  - {}", fact));
            }
        }

        lines.join("\n")
    }
}

/// A single step in the inference trace.
#[derive(Debug, Clone)]
pub struct InferenceStep {
    /// Step description
    pub description: String,
    /// Rule that was applied (if any)
    pub rule_id: Option<String>,
    /// Type of step
    pub step_type: StepType,
}

/// Types of inference steps.
#[derive(Debug, Clone, PartialEq)]
pub enum StepType {
    /// Starting from a query/goal
    Query,
    /// Applying a rule's condition
    RuleApplication,
    /// Sub-goals created from rule conditions
    SubGoal,
    /// Fact match that satisfied a condition
    FactMatch,
    /// Backtracking due to failed sub-goal
    Backtrack,
}

/// Executes backward chaining from a goal/query.
///
/// This is the main entry point for backward rule execution as exported from lib.rs.
pub fn execute_backward(_engine: &Engine, _session: &Session, goal: &str) -> InferenceTrace {
    // Placeholder implementation - actual backward chaining would go here
    InferenceTrace::new(false)
        .with_step(InferenceStep {
            description: format!("Query: {}", goal),
            rule_id: None,
            step_type: StepType::Query,
        })
        .with_step(InferenceStep {
            description: "No rules match the query".to_string(),
            rule_id: None,
            step_type: StepType::Backtrack,
        })
}

/// Interactive trace explorer for debugging.
pub struct TraceExplorer<'a> {
    trace: &'a InferenceTrace,
    /// Current position in the trace
    position: usize,
    /// Maximum depth to explore
    max_depth: usize,
}

impl<'a> TraceExplorer<'a> {
    pub fn new(trace: &'a InferenceTrace) -> Self {
        Self {
            trace,
            position: 0,
            max_depth: 10, // Default limit
        }
    }

    /// Moves to the next step in the trace.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&InferenceStep> {
        if self.position < self.trace.path.len() && self.position < self.max_depth {
            let step = &self.trace.path[self.position];
            self.position += 1;
            Some(step)
        } else {
            None
        }
    }

    /// Moves back to the previous step.
    pub fn prev(&mut self) -> Option<&InferenceStep> {
        if self.position > 0 {
            self.position -= 1;
            Some(&self.trace.path[self.position])
        } else {
            None
        }
    }

    /// Gets the current step without advancing.
    pub fn current(&self) -> Option<&InferenceStep> {
        if self.position == 0 && !self.trace.path.is_empty() {
            // Before any next() call, return first item
            Some(&self.trace.path[0])
        } else if self.position > 0
            && self.position <= self.trace.path.len()
            && self.position <= self.max_depth
        {
            // After next(), return the last step returned (position - 1)
            Some(&self.trace.path[self.position - 1])
        } else {
            None
        }
    }

    /// Resets to the beginning of the trace.
    pub fn reset(&mut self) {
        self.position = 0;
    }

    /// Checks if there are more steps to explore.
    pub fn has_more(&self) -> bool {
        self.position < self.trace.path.len() && self.position < self.max_depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inference_trace_explanation() {
        let trace = InferenceTrace::new(true)
            .with_step(InferenceStep {
                description: "Query: Is customer eligible for discount?".to_string(),
                rule_id: None,
                step_type: StepType::Query,
            })
            .with_step(InferenceStep {
                description: "Rule 'DiscountEligible' matched with customer facts".to_string(),
                rule_id: Some("discount-1".to_string()),
                step_type: StepType::RuleApplication,
            });

        let explanation = trace.explain();
        assert!(explanation.contains("Inference Trace:"));
        assert!(explanation.contains("Query: Is customer eligible for discount?"));
    }

    #[test]
    fn test_trace_explorer() {
        let mut trace = InferenceTrace::new(true);
        trace.path.push(InferenceStep {
            description: "Step 1".to_string(),
            rule_id: None,
            step_type: StepType::Query,
        });
        trace.path.push(InferenceStep {
            description: "Step 2".to_string(),
            rule_id: None,
            step_type: StepType::RuleApplication,
        });

        let mut explorer = TraceExplorer::new(&trace);

        assert!(explorer.has_more());
        assert_eq!(explorer.next().unwrap().description, "Step 1");
        assert_eq!(explorer.current().unwrap().description, "Step 1");

        assert!(explorer.has_more());
        assert_eq!(explorer.next().unwrap().description, "Step 2");
        assert!(!explorer.has_more());

        explorer.reset();
        assert!(explorer.has_more());
        assert_eq!(explorer.current().unwrap().description, "Step 1");
    }
}
