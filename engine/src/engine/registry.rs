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

    /// Phase 28 (D-01 / EXEC-04): enable/disable inference-trace collection
    /// on this engine. Called by `execute_single` BEFORE `engine.execute()`
    /// so the evaluator knows whether to allocate `TraceEvent`s. Default is
    /// a noop so existing engines keep compiling; engines that emit traces
    /// override this to store the flag in a field. When `false` (the
    /// default), the evaluator MUST skip all trace allocation (SC-3 latency
    /// neutrality — no `Vec`, no `String`, no `chrono` call).
    fn set_collect_trace(&mut self, _enabled: bool) {}

    /// Loads a ruleset into the engine so `execute` can fire rules against
    /// working memory. Default impl returns `Err` — engines that support
    /// real evaluation override this (Plan 03 wires `ReteEngine`, Plan 06
    /// wires the other three). This is the `RuleSet → engine` bridge (L-02,
    /// Option (b)) that lets `POST /v1/execute` run a user's authored rules
    /// instead of an empty network.
    fn load_ruleset(&mut self, _rules: &crate::rules::RuleSet) -> Result<(), String> {
        Err("load_ruleset not supported by this engine".into())
    }

    /// Returns the post-execution facts (working memory) so the API layer
    /// can serialize `facts_after` from real engine state rather than
    /// echoing the request input (Pitfall 11). Default impl returns an
    /// empty `Vec` — engines that expose working memory override this.
    fn get_facts(&self) -> Vec<crate::fact::Fact> {
        Vec::new()
    }
}

/// Phase 28 (D-01 / EXEC-04): a single inference-trace event emitted by the
/// live evaluator at the moment a rule fires. The engine crate owns this
/// type because the engine cannot import `api::v1::execution` types
/// (dependency direction is `api → engine` — Critical Finding 1). The API
/// layer maps it to the wire-schema `InferenceTraceEntry` in `execute_single`.
///
/// `event_type` is a `&'static str` (currently always `"rule_fired"`) so the
/// common case avoids a per-event allocation; the API mapping clones it into
/// an owned `String` only when `collect_trace` is true.
#[derive(Debug, Clone)]
pub struct TraceEvent {
    /// The id of the rule that fired (matches `Rule.id` / `fired_rule_ids`).
    pub rule_id: String,
    /// RFC3339 timestamp captured at fire time via `chrono::Utc::now()`.
    pub timestamp: String,
    /// Event kind — currently always `"rule_fired"`.
    pub event_type: &'static str,
}

/// Phase 28 (D-01 / EXEC-04): the ordered trace payload returned from the
/// evaluator. `events` is in FIRING order (the order the fixpoint loop fired
/// each rule), NOT `fired_rule_ids` order (`fired_rule_ids` is populated from
/// a `HashSet` and is therefore unordered — Critical Finding 3). The API
/// layer maps this to `InferenceTrace` in `execute_single`.
#[derive(Debug, Clone, Default)]
pub struct TraceData {
    /// Ordered trace events — one per fired rule, in firing order.
    pub events: Vec<TraceEvent>,
    /// Wall-clock duration of the traced fixpoint loop in milliseconds
    /// (captured via `Instant::now()` at loop entry / elapsed at end).
    pub total_duration_ms: f64,
    /// Honest count of facts that matched a fired rule's condition (D-01
    /// discretion — the count is computed at fire time from the working
    /// memory snapshot).
    pub facts_matched: usize,
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
    /// IDs of the rules that fired during this execution. Engines populate
    /// this via field mutation after `EngineResult::success` constructs the
    /// base value (the trait returns `EngineResult` by value and the engine
    /// owns the mutation before returning). The API layer derives
    /// `rules_not_fired` as the ruleset's rule ids minus this set. Empty for
    /// the failure constructor and for the default `success` constructor
    /// when the engine does not track ids yet.
    pub fired_rule_ids: Vec<String>,
    /// Phase 28 (D-01 / EXEC-04): ordered inference-trace payload. `None`
    /// when `collect_trace` is false (the SC-3 latency-neutral path — the
    /// evaluator skips all trace allocation). `Some(TraceData)` only when
    /// `collect_trace == true` AND at least one rule fired. Additive to
    /// `fired_rule_ids` (which remains populated for existing consumers).
    pub trace: Option<TraceData>,
}

impl EngineResult {
    /// Creates a new successful result.
    pub fn success(rules_fired: usize) -> Self {
        Self {
            success: true,
            rules_fired,
            message: format!("Executed {} rules", rules_fired),
            fired_rule_ids: Vec::new(),
            trace: None,
        }
    }

    /// Creates a new failed result.
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            rules_fired: 0,
            message: message.into(),
            fired_rule_ids: Vec::new(),
            trace: None,
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

    /// Registers all default engines available under the enabled cargo
    /// features. Each factory is registered only under its static
    /// `#[cfg(feature = "engine-X")]` gate — no runtime/user input selects
    /// which factories register (T-23-01 mitigation). With all four
    /// features enabled, `available_engines()` truthfully returns
    /// `["rete", "treat", "leaps", "phreak"]` (HashMap order not guaranteed).
    pub fn register_default_engines(&mut self) {
        #[cfg(feature = "engine-rete")]
        {
            self.register_factory(Box::new(ReteEngineFactory::new()));
        }
        #[cfg(feature = "engine-treat")]
        {
            self.register_factory(Box::new(TreatEngineFactory::new()));
        }
        #[cfg(feature = "engine-leaps")]
        {
            self.register_factory(Box::new(LeapsEngineFactory::new()));
        }
        #[cfg(feature = "engine-phreak")]
        {
            self.register_factory(Box::new(PhreakEngineFactory::new()));
        }
        #[cfg(not(feature = "engine-rete"))]
        {
            // No engines available without features
        }
    }

    /// Creates an engine instance by type.
    pub fn create_engine(
        &self,
        engine_type: EngineType,
    ) -> Option<Result<Box<dyn ExecutionEngine>, String>> {
        self.factories.get(&engine_type).map(|f| f.create_engine())
    }

    /// Creates an engine by name string.
    pub fn create_engine_by_name(
        &self,
        name: &str,
    ) -> Option<Result<Box<dyn ExecutionEngine>, String>> {
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
        self.factories.keys().map(|k| k.name()).collect()
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
pub use crate::engine::rete::{ReteEngine, ReteEngineFactory};

#[cfg(feature = "engine-leaps")]
pub use crate::engine::leaps::LeapsEngineFactory;
#[cfg(feature = "engine-phreak")]
pub use crate::engine::phreak::PhreakEngineFactory;
/// Re-export the non-Rete factory types under their feature gates so
/// `register_default_engines` can name them. The engine structs themselves
/// are not re-exported here (Rete's `ReteEngine` is the shared type the
/// others build on; the per-engine structs stay internal to their modules).
#[cfg(feature = "engine-treat")]
pub use crate::engine::treat::TreatEngineFactory;

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

    /// After `register_default_engines()` on a fresh registry with all four
    /// engine features enabled, `available_engines()` must contain all four
    /// names. HashMap iteration order is not guaranteed, so check membership
    /// via `.contains(...)` rather than positional indexing. This is the
    /// truthfulness guarantee for D-01 (register all four factories) that
    /// downstream `engine` param validation (Plan 05) depends on.
    #[cfg(all(
        feature = "engine-rete",
        feature = "engine-treat",
        feature = "engine-leaps",
        feature = "engine-phreak"
    ))]
    #[test]
    fn test_register_default_engines_advertises_all_four() {
        let mut registry = EngineRegistry::new();
        registry.register_default_engines();
        let names = registry.available_engines();
        // `available_engines` returns `Vec<&'static str>` (the static name
        // strings from `EngineType::name`), so compare against `&str` literals.
        assert!(
            names.contains(&"rete"),
            "available_engines missing 'rete': {:?}",
            names
        );
        assert!(
            names.contains(&"treat"),
            "available_engines missing 'treat': {:?}",
            names
        );
        assert!(
            names.contains(&"leaps"),
            "available_engines missing 'leaps': {:?}",
            names
        );
        assert!(
            names.contains(&"phreak"),
            "available_engines missing 'phreak': {:?}",
            names
        );
        assert_eq!(
            names.len(),
            4,
            "expected exactly four engines, got {:?}",
            names
        );
    }
}
