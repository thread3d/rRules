//! Pluggable Engine Architecture for rRules Business Rules Management System
//!
//! This module provides a trait-based interface for multiple Rete-family algorithms
//! with runtime selection via configuration or API endpoint.
//!
//! # Overview
//!
//! The pluggable engine architecture supports:
//! - `Rete`: Classic Rete algorithm with party tokens, alpha/beta memory
//! - `TREAT`: Optimized left-memory management with LRU caching
//! - `LEAPS`: Aggregation node support for Sum/Count/Max/Min operations
//! - `PHREAK`: Incremental truth maintenance with justifications
//!
//! # Usage
//!
//! ```ignore
//! use engine::engine::{EngineRegistry, ExecutionEngine};
//!
//! // Create registry and register engines
//! let mut registry = EngineRegistry::new();
//! registry.register_default_engines();
//!
//! // Select an engine by name
//! let engine = registry.create_engine("rete").unwrap();
//!
//! // Execute rules
//! let result = engine.execute();
//! ```

#[cfg(feature = "engine-rete")]
pub mod rete;

// TREAT and LEAPS require engine-rete for shared types
#[cfg(any(feature = "engine-treat", feature = "engine-leaps"))]
pub mod treat;
#[cfg(feature = "engine-leaps")]
pub mod leaps;
#[cfg(feature = "engine-phreak")]
pub mod phreak;

pub mod metrics;
pub mod registry;
pub mod selector;

// Re-export core types for convenience
#[cfg(feature = "engine-rete")]
pub use metrics::{EngineMetrics, MetricsCollector};
pub use registry::{EngineFactory, EngineRegistry, EngineType};
pub use selector::EngineSelector;
