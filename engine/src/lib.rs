//! Core engine library for rRules Business Rules Management System
//!
//! This crate provides the foundational data structures, fact representation,
//! rule definitions, and execution engines (forward/backward chaining) for the rRules BRMS.

pub mod data_types;
pub mod environment;
pub mod fact;
pub mod rules;

// Forward and backward chaining execution
pub mod backward_chaining;
pub mod forward_chaining;

// Validation module for syntax and semantic checks
pub mod validation;

// Testing framework for rule engine verification
#[cfg(test)]
pub mod testing;

// Chaining mode selection
pub mod chaining {
    pub mod mode_selector;
}
pub use chaining::mode_selector::{ChainingMode, ChainingSelector, InferenceTraceCollector};

// Salience and conflict resolution
pub mod salience {
    pub mod agenda;
    pub mod conflict_resolution;
}
pub use salience::agenda::{Activation, Agenda};
pub use salience::conflict_resolution::{ConflictResolver, ConflictStrategy};

// Re-exports for convenient access to core types
pub use data_types::{FactType, Value};
pub use environment::EngineBuilder;
pub use fact::{Fact, WorkingMemory};
pub use rules::{Action, Condition, Rule, RuleSet};

// Re-export chaining entry points
pub use backward_chaining::execute_backward;
pub use forward_chaining::execute_forward;

// Re-export validation types
pub use validation::{ValidationError, ValidationResult, ValidationWarning};
