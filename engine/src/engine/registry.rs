//! Engine registry for dynamic engine registration and creation.
//!
//! This module provides an engine factory pattern with support for
//! registering and creating different Rete-family algorithm implementations.

use std::collections::HashMap;

/// Type identifier for different engine implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineType {
    /// Classic Rete algorithm
    Rete,
    /// TREAT algorithm with caching
    Treat,
    /// LEAPS algorithm with aggregation nodes
    Leaps,
    /// PHREAK algorithm with truth maintenance
    Phreak,
}

impl EngineType {
    /// Returns the display name for this engine type.
    pub fn name(&self) -> &'static str {
        match self {
            EngineType::Rete => "rete",
            EngineType::Treat => "treat",
            EngineType::Leaps => "leaps",
            EngineType::Phreak => "phreak",
        }
    }

    /// Returns a list of all available engine types.
    pub fn all_variants() -> Vec<EngineType> {
        vec![
            EngineType::Rete,
            EngineType::Treat,
            EngineType::Leaps,
            EngineType::Phreak,
        ]
    }
}

impl std::fmt::Display for EngineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Trait for creating engine instances.
pub trait EngineFactory: Send + Sync {
    /// Returns the type of engine this factory creates.
    fn engine_type(&self) -> EngineType;

    /// Creates a new engine instance with default configuration.
    fn create_engine(&self) -> Result<Box<dyn ExecutionEngine>, String>;

    /// Creates a new engine instance with custom configuration.
    fn create_engine_with_config(&self, _config: &str) -> Result<Box<dyn ExecutionEngine>, String> {
        self.create_engine()
    }
}

/// The main execution engine trait that all implementations must satisfy.
pub trait ExecutionEngine: Send + Sync + std::fmt::Debug {
    /// Executes the rule engine and returns the result.
    fn execute(&mut self) -> EngineResult;

    /// Inserts a fact into working memory.
    fn insert_fact(&mut self, _fact: crate::fact::Fact) -> Result<(), String>;

    /// Retracts a fact from working memory by ID.
    fn retract_fact(&mut self, _fact_id: &str) -> Result<bool, String>;

    /// Runs all applicable rules until fixpoint.
    fn run(&mut self) -> usize;

    /// Gets the current engine metrics.
    fn get_metrics(&self) -> crate::engine::metrics::EngineMetrics;

    /// Returns the engine type.
    fn engine_type(&self) -> EngineType;

    /// Resets the engine state (clears working memory and metrics).
    fn reset(&mut self);

    /// Sets a configuration option.
    fn set_config(&mut self, _key: &str, _value: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Result of engine execution.
#[derive(Debug, Clone)]
pub struct EngineResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Number of rules fired
    pub rules_fired: usize,
    /// Execution message or error details
    pub message: String,
}

impl EngineResult {
    /// Creates a new successful result.
    pub fn success(rules_fired: usize) -> Self {
        Self {
            success: true,
            rules_fired,
            message: format!("Executed {} rules", rules_fired),
        }
    }

    /// Creates a new failed result.
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            rules_fired: 0,
            message: message.into(),
        }
    }

    /// Checks if execution was successful.
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Gets the number of rules fired.
    pub fn rules_fired(&self) -> usize {
        self.rules_fired
    }

    /// Gets the result message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Registry for managing engine factories and creating instances.
pub struct EngineRegistry {
    factories: HashMap<EngineType, Box<dyn EngineFactory>>,
}

impl EngineRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Registers an engine factory.
    pub fn register_factory(&mut self, factory: Box<dyn EngineFactory>) {
        let engine_type = factory.engine_type();
        self.factories.insert(engine_type, factory);
    }

    /// Registers all default engines (Rete only in initial implementation).
    pub fn register_default_engines(&mut self) {
        #[cfg(feature = "engine-rete")]
        {
            self.register_factory(Box::new(ReteEngineFactory::new()));
        }
        #[cfg(not(feature = "engine-rete"))]
        {
            // No engines available without features
        }
    }

    /// Creates an engine instance by type.
    pub fn create_engine(&self, engine_type: EngineType) -> Option<Result<Box<dyn ExecutionEngine>, String>> {
        self.factories
            .get(&engine_type)
            .map(|f| f.create_engine())
    }

    /// Creates an engine by name string.
    pub fn create_engine_by_name(&self, name: &str) -> Option<Result<Box<dyn ExecutionEngine>, String>> {
        let engine_type = match name.to_lowercase().as_str() {
            "rete" => EngineType::Rete,
            "treat" => EngineType::Treat,
            "leaps" => EngineType::Leaps,
            "phreak" => EngineType::Phreak,
            _ => return None,
        };
        self.create_engine(engine_type)
    }

    /// Returns a list of available engine names.
    pub fn available_engines(&self) -> Vec<&'static str> {
        self.factories
            .keys()
            .map(|k| k.name())
            .collect()
    }

    /// Checks if an engine type is registered.
    pub fn has_engine(&self, engine_type: EngineType) -> bool {
        self.factories.contains_key(&engine_type)
    }
}

impl Default for EngineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Re-export engine types from the rete module when enabled.
#[cfg(feature = "engine-rete")]
pub use crate::engines::rete::{ReteEngine, ReteEngineFactory};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_type_names() {
        assert_eq!(EngineType::Rete.name(), "rete");
        assert_eq!(EngineType::Treat.name(), "treat");
        assert_eq!(EngineType::Leaps.name(), "leaps");
        assert_eq!(EngineType::Phreak.name(), "phreak");
    }

    #[test]
    fn test_engine_registry_creation() {
        let registry = EngineRegistry::new();
        assert!(registry.available_engines().is_empty());
    }

    #[test]
    fn test_engine_type_display() {
        assert_eq!(format!("{}", EngineType::Rete), "rete");
    }
}
