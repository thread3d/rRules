//! Salience-based rule ordering and conflict resolution.
//!
//! This module provides facilities for managing rule activations in priority order
//! based on salience values and applying conflict resolution strategies when multiple
//! rules have the same priority.

pub mod agenda;
pub mod conflict_resolution;

// Re-export main types for convenience
pub use agenda::{Activation, Agenda};
pub use conflict_resolution::{ConflictResolver, ConflictStrategy};
