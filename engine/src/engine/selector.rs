//! Runtime engine selection via configuration and API endpoint.
//!
//! This module provides the EngineSelector for parsing configuration files
//! (YAML) and handling runtime engine switching via API.

use std::collections::HashMap;

/// Error types for engine selection.
#[derive(Debug, thiserror::Error)]
pub enum EngineSelectionError {
    #[error("Engine not found: {0}")]
    EngineNotFound(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Configuration file error: {0}")]
    ConfigFile(String),
    #[error("API request error: {0}")]
    ApiRequest(String),
}

/// Configuration for engine selection.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// The algorithm to use
    pub algorithm: String,
    /// Additional algorithm-specific options
    pub options: HashMap<String, String>,
}

impl EngineConfig {
    /// Creates a new config with default values.
    pub fn new() -> Self {
        Self {
            algorithm: "rete".to_string(),
            options: HashMap::new(),
        }
    }

    /// Sets the algorithm name.
    pub fn with_algorithm(mut self, algorithm: impl Into<String>) -> Self {
        self.algorithm = algorithm.into();
        self
    }

    /// Adds an option.
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Selector for choosing and switching between engines.
pub struct EngineSelector {
    /// Current engine type
    current: String,
    /// Available engine types
    available: Vec<String>,
}

impl EngineSelector {
    /// Creates a new selector with the given available engines.
    pub fn new(available_engines: &[&str]) -> Self {
        let available = available_engines.iter().map(|s| s.to_string()).collect();
        Self {
            current: "rete".to_string(), // Default to Rete
            available,
        }
    }

    /// Creates a selector with default engines.
    pub fn with_defaults() -> Self {
        Self::new(&["rete", "treat", "leaps", "phreak"])
    }

    /// Returns the currently selected engine.
    pub fn current_engine(&self) -> &str {
        &self.current
    }

    /// Returns available engine names.
    pub fn available_engines(&self) -> &[String] {
        &self.available
    }

    /// Selects a new engine by name.
    ///
    /// # Arguments
    /// * `engine_name` - The name of the engine to select
    ///
    /// # Errors
    /// Returns an error if the engine is not available or unknown.
    pub fn select_engine(&mut self, engine_name: &str) -> Result<(), EngineSelectionError> {
        let engine_lower = engine_name.to_lowercase();

        if !self.available.contains(&engine_lower) {
            return Err(EngineSelectionError::EngineNotFound(engine_name.to_string()));
        }

        // Basic validation for known algorithms
        match engine_lower.as_str() {
            "rete" | "treat" | "leaps" | "phreak" => {
                self.current = engine_lower;
                Ok(())
            }
            _ => Err(EngineSelectionError::InvalidConfig(format!(
                "Unknown algorithm: {}",
                engine_name
            ))),
        }
    }

    /// Parses configuration from a YAML string.
    ///
    /// # Arguments
    /// * `yaml_content` - YAML configuration content
    ///
    /// # Example
    /// ```ignore
    /// let config = EngineSelector::from_yaml("engine:\n  algorithm: treat")?;
    /// ```
    pub fn from_yaml(_yaml_content: &str) -> Result<EngineConfig, EngineSelectionError> {
        // In a full implementation, this would parse YAML using serde_yml or similar
        // For now, we return an error indicating the feature is not available
        Err(EngineSelectionError::InvalidConfig(
            "YAML parsing requires the 'yaml-config' feature".to_string(),
        ))
    }

    /// Creates a configuration from a JSON string.
    ///
    /// # Arguments
    /// * `json_content` - JSON configuration content
    pub fn from_json(_json_content: &str) -> Result<EngineConfig, EngineSelectionError> {
        // In a full implementation, this would parse JSON using serde_json
        Err(EngineSelectionError::InvalidConfig(
            "JSON parsing requires the 'json-config' feature".to_string(),
        ))
    }

    /// Creates an API response for engine selection.
    ///
    /// # Arguments
    /// * `engine_name` - The name of the selected engine
    pub fn api_response(&self, engine_name: &str) -> HashMap<String, String> {
        let mut response = HashMap::new();
        response.insert("status".to_string(), "success".to_string());
        response.insert("selected_engine".to_string(), engine_name.to_string());
        response.insert(
            "available_engines".to_string(),
            self.available.join(","),
        );
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selector_creation() {
        let selector = EngineSelector::with_defaults();
        assert_eq!(selector.current_engine(), "rete");
        assert!(!selector.available_engines().is_empty());
    }

    #[test]
    fn test_select_valid_engine() {
        let mut selector = EngineSelector::with_defaults();
        assert!(selector.select_engine("treat").is_ok());
        assert_eq!(selector.current_engine(), "treat");
    }

    #[test]
    fn test_select_invalid_engine() {
        let mut selector = EngineSelector::new(&["rete"]);
        let result = selector.select_engine("invalid");
        assert!(matches!(result, Err(EngineSelectionError::EngineNotFound(_))));
    }

    #[test]
    fn test_case_insensitive_selection() {
        let mut selector = EngineSelector::with_defaults();
        assert!(selector.select_engine("RETE").is_ok());
        assert_eq!(selector.current_engine(), "rete");
    }

    #[test]
    fn test_api_response() {
        let selector = EngineSelector::new(&["rete", "treat"]);
        let response = selector.api_response("treat");

        assert_eq!(response.get("status"), Some(&"success".to_string()));
        assert_eq!(response.get("selected_engine"), Some(&"treat".to_string()));
    }
}
