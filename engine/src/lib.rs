//! Core engine library for rRules Business Rules Management System
//!
//! This crate provides the foundational data structures, fact representation,
//! rule definitions, and execution engines (forward/backward chaining) for the rRules BRMS.

pub mod data_types;
pub mod environment;
pub mod fact;
pub mod rules;

// Engine modules (pluggable architecture)
pub mod engine;

// Forward and backward chaining execution
pub mod backward_chaining;
pub mod forward_chaining;

// Validation module for syntax and semantic checks
pub mod validation;

// Testing framework for rule engine verification
pub mod testing;

// Governance modules (Phase 18)
pub mod governance {
    pub mod approval;
    pub mod audit;
    pub mod rbac;
    pub mod versioning;
}

// Coverage tracking module (Phase 19)
pub mod coverage;

// Engine metrics module (Phase 19)
pub mod metrics;

// Monitoring modules (Phase 19)
pub mod monitoring {
    pub mod coverage;
    pub mod dead_rules;
    pub mod detection;
}

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
pub use backward_chaining::{InferenceStep, InferenceTrace, StepType};
pub use forward_chaining::execute_forward;

// Re-export validation types
pub use validation::{ValidationError, ValidationResult, ValidationWarning};

// Simulation module for What-If analysis (Phase 5)
pub mod simulation {
    pub mod runner;
    pub mod types;
}
