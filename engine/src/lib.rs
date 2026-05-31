//! Core engine library for rRules Business Rules Management System
//!
//! This crate provides the foundational data structures, fact representation,
//! rule definitions, and execution engines (forward/backward chaining) for the rRules BRMS.

pub mod data_types;
pub mod fact;
pub mod rules;
pub mod environment;

// Forward and backward chaining execution
pub mod forward_chaining;
pub mod backward_chaining;

// Validation module for syntax and semantic checks
pub mod validation;

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
pub use salience::agenda::{Agenda, Activation};
pub use salience::conflict_resolution::{ConflictResolver, ConflictStrategy};

// Re-exports for convenient access to core types
pub use data_types::{Value, FactType};
pub use fact::{Fact, WorkingMemory};
pub use rules::{Rule, RuleSet, Condition, Action};
pub use environment::EngineBuilder;

// Re-export chaining entry points
pub use forward_chaining::execute_forward;
pub use backward_chaining::execute_backward;

// Re-export validation types
pub use validation::{ValidationResult, ValidationError, ValidationWarning};
