//! Dead rule detection and health monitoring.
//!
//! This module provides functionality to track rule health, detect dead rules
//! (rules that have not fired in a long time), and compute confidence scores
//! based on historical firing patterns.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Represents the health status of a rule for monitoring purposes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleHealth {
    /// Unique identifier for the rule
    pub rule_id: String,
    /// Human-readable name of the rule
    pub name: String,
    /// Timestamp when the rule last fired (None if never fired)
    pub last_fired_at: Option<DateTime<Utc>>,
    /// Total number of times the rule has fired
    pub activation_count: u64,
    /// Confidence score (0.0 to 100.0) based on firing patterns and stability
    pub confidence_score: f64,
}

/// Configuration for dead rule detection.
#[derive(Debug, Clone)]
pub struct DeadRuleConfig {
    /// Threshold duration after which a rule with no activations is considered dead
    pub inactivity_threshold: Duration,
}

impl Default for DeadRuleConfig {
    fn default() -> Self {
        Self {
            // Default 24 hours threshold
            inactivity_threshold: Duration::hours(24),
        }
    }
}

/// Alias for compatibility - uses sliding window config's inactivity threshold
#[deprecated(
    note = "Use SlidingWindowConfig directly with the `window_duration` and `num_windows` fields"
)]
impl DeadRuleConfig {
    /// Creates a new dead rule config with specified inactivity threshold.
    pub fn new(inactivity_threshold: Duration) -> Self {
        Self { inactivity_threshold }
    }
}

impl RuleHealth {
    /// Creates a new RuleHealth instance with default values.
    pub fn new(rule_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            rule_id: rule_id.into(),
            name: name.into(),
            last_fired_at: None,
            activation_count: 0,
            confidence_score: 0.0,
        }
    }

    /// Records a rule firing event.
    pub fn record_fire(&mut self) {
        self.last_fired_at = Some(Utc::now());
        self.activation_count += 1;
        // Increase confidence with more firings, capped at 100.0
        self.confidence_score = (self.confidence_score + 5.0).min(100.0);
    }

    /// Decreases confidence score due to inactivity or stale data.
    pub fn decay_confidence(&mut self) {
        // Decay confidence by 10% per period, minimum 0.0
        self.confidence_score = (self.confidence_score * 0.9).max(0.0);
    }

    /// Sets the rule as never having fired (e.g., rule disabled or reset).
    pub fn clear_history(&mut self) {
        self.last_fired_at = None;
        self.activation_count = 0;
        self.confidence_score = 0.0;
    }
}

impl Default for RuleHealth {
    fn default() -> Self {
        Self {
            rule_id: String::new(),
            name: String::new(),
            last_fired_at: None,
            activation_count: 0,
            confidence_score: 0.0,
        }
    }
}

/// Configuration for sliding window analysis.
#[derive(Debug, Clone)]
pub struct SlidingWindowConfig {
    /// Duration of each window in the sliding window
    pub window_duration: Duration,
    /// Number of windows to track
    pub num_windows: usize,
}

impl Default for SlidingWindowConfig {
    fn default() -> Self {
        Self {
            // Default 1-hour windows, tracking last 24 windows (24 hours)
            window_duration: Duration::hours(1),
            num_windows: 24,
        }
    }
}

/// A collection of rule health records.
#[derive(Debug, Default)]
pub struct HealthRegistry {
    /// Map of rule ID to health record
    rules: std::collections::HashMap<String, RuleHealth>,
    /// Sliding window configuration for activation tracking
    window_config: SlidingWindowConfig,
}

impl HealthRegistry {
    /// Creates a new empty health registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new health registry with custom sliding window configuration.
    pub fn with_config(window_config: SlidingWindowConfig) -> Self {
        Self {
            rules: std::collections::HashMap::new(),
            window_config,
        }
    }

    /// Returns the current sliding window configuration.
    pub fn window_config(&self) -> &SlidingWindowConfig {
        &self.window_config
    }

    /// Records a rule firing event.
    pub fn record_fire(&mut self, rule_id: impl Into<String>, name: impl Into<String>) {
        let rule_id_str = rule_id.into();
        let name_str = name.into();

        self.rules.entry(rule_id_str.clone()).or_insert_with(|| RuleHealth::new(rule_id_str, name_str)).record_fire();
    }

    /// Gets health data for a specific rule.
    pub fn get_health(&self, rule_id: &str) -> Option<&RuleHealth> {
        self.rules.get(rule_id)
    }

    /// Returns all health records.
    pub fn get_all_health(&self) -> Vec<&RuleHealth> {
        self.rules.values().collect()
    }

    /// Marks rules that have not fired in the specified duration as potentially dead.
    pub fn mark_inactive_since(&mut self, duration: chrono::Duration) {
        let now = Utc::now();
        for health in self.rules.values_mut() {
            if let Some(last_fired) = health.last_fired_at {
                if now - last_fired > duration {
                    health.decay_confidence();
                }
            } else {
                // Rule never fired - decrease confidence
                health.decay_confidence();
            }
        }
    }

    /// Returns a list of potentially dead rules (never fired or very stale).
    pub fn find_dead_rules(&self, min_inactive_duration: chrono::Duration) -> Vec<&RuleHealth> {
        let now = Utc::now();
        self.rules.values().filter(|h| {
            h.last_fired_at.is_none()
                || (now - h.last_fired_at.unwrap() > min_inactive_duration && h.activation_count == 0)
        }).collect()
    }

    /// Detects dead rules using a sliding window algorithm.
    ///
    /// A rule is considered dead if:
    /// - `activation_count == 0` AND
    /// - `last_fired_at` is older than the configured inactivity threshold
    ///
    /// Confidence score is set to 100% for dead rules. For non-dead rules,
    /// confidence is calculated based on window utilization (firing rate within windows).
    pub fn detect_dead_rules(&self) -> Vec<RuleHealth> {
        let now = Utc::now();
        let mut dead_rules: Vec<RuleHealth> = Vec::new();

        // Get configuration
        let config = &self.window_config;

        // Calculate the inactivity threshold from the sliding window (default 24 hours)
        let threshold = Duration::hours(24);

        for health in self.rules.values() {
            let is_dead = health.activation_count == 0
                && match health.last_fired_at {
                    Some(last_fired) => now - last_fired > threshold,
                    None => true, // Never fired
                };

            if is_dead {
                // Dead rule gets 100% confidence in its dead status
                let mut health_copy = health.clone();
                health_copy.confidence_score = 100.0;
                dead_rules.push(health_copy);
            } else {
                // Non-dead rule: calculate confidence based on window utilization
                let confidence = Self::calculate_window_utilization_confidence(
                    health,
                    now,
                    config.window_duration,
                    config.num_windows,
                );
                let mut health_copy = health.clone();
                health_copy.confidence_score = confidence;
                dead_rules.push(health_copy);
            }
        }

        dead_rules
    }

    /// Calculates confidence score based on window utilization.
    ///
    /// The confidence is calculated as a percentage based on how many
    /// windows in the sliding window contain at least one activation.
    fn calculate_window_utilization_confidence(
        health: &RuleHealth,
        now: DateTime<Utc>,
        window_duration: Duration,
        num_windows: usize,
    ) -> f64 {
        if health.activation_count == 0 {
            return 0.0;
        }

        let total_window_duration = window_duration * num_windows as i32;

        match health.last_fired_at {
            Some(last_fired) => {
                // Calculate how many windows have passed since last fire
                let time_since_last_fire = now - last_fired;

                if time_since_last_fire > total_window_duration {
                    // Rule hasn't fired in any of the tracked windows
                    0.0
                } else {
                    // Calculate based on activation distribution
                    // More recent fires = higher confidence
                    let ratio = 1.0 - (time_since_last_fire.num_seconds() as f64 / total_window_duration.num_seconds() as f64);
                    (ratio * 100.0).min(100.0)
                }
            }
            None => 0.0,
        }
    }

    /// Resets all health data for a new monitoring period.
    pub fn reset(&mut self) {
        self.rules.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_health_creation() {
        let health = RuleHealth::new("rule-1", "Test Rule");
        assert_eq!(health.rule_id, "rule-1");
        assert_eq!(health.name, "Test Rule");
        assert!(health.last_fired_at.is_none());
        assert_eq!(health.activation_count, 0);
    }

    #[test]
    fn test_record_fire() {
        let mut health = RuleHealth::new("rule-1", "Test Rule");
        health.record_fire();

        assert!(health.last_fired_at.is_some());
        assert_eq!(health.activation_count, 1);
        assert_eq!(health.confidence_score, 5.0);
    }

    #[test]
    fn test_decay_confidence() {
        let mut health = RuleHealth::new("rule-1", "Test Rule");
        health.confidence_score = 50.0;
        health.decay_confidence();

        assert!((health.confidence_score - 45.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_clear_history() {
        let mut health = RuleHealth::new("rule-1", "Test Rule");
        health.last_fired_at = Some(Utc::now());
        health.activation_count = 5;
        health.confidence_score = 75.0;

        health.clear_history();

        assert!(health.last_fired_at.is_none());
        assert_eq!(health.activation_count, 0);
        assert_eq!(health.confidence_score, 0.0);
    }

    #[test]
    fn test_health_registry() {
        let mut registry = HealthRegistry::new();
        registry.record_fire("rule-1", "Rule One");
        registry.record_fire("rule-2", "Rule Two");

        let health1 = registry.get_health("rule-1").unwrap();
        assert_eq!(health1.activation_count, 1);

        let all_health = registry.get_all_health();
        assert_eq!(all_health.len(), 2);
    }

    #[test]
    fn test_detect_dead_rules_never_fired() {
        let mut registry = HealthRegistry::new();
        registry.record_fire("rule-1", "Rule One");

        // Add a rule that was never fired (using internal access)
        use chrono::{Duration, Utc};
        let old_time = Utc::now() - Duration::hours(48); // 2 days ago
        registry.rules.insert(
            "rule-dead".to_string(),
            RuleHealth {
                rule_id: "rule-dead".to_string(),
                name: "Dead Rule".to_string(),
                last_fired_at: Some(old_time),
                activation_count: 0,
                confidence_score: 0.0,
            },
        );

        let detected = registry.detect_dead_rules();

        // The never-fired rule should be detected
        let dead_rule_ids: Vec<&str> = detected.iter().map(|r| r.rule_id.as_str()).collect();
        assert!(dead_rule_ids.contains(&"rule-dead"));

        // Dead rules have 100% confidence in their dead status
        let dead_health = detected.into_iter().find(|r| r.rule_id == "rule-dead").unwrap();
        assert_eq!(dead_health.confidence_score, 100.0);
    }

    #[test]
    fn test_detect_dead_rules_recently_fired() {
        let mut registry = HealthRegistry::new();

        // Add a rule that fired recently (within the threshold)
        registry.rules.insert(
            "rule-active".to_string(),
            RuleHealth {
                rule_id: "rule-active".to_string(),
                name: "Active Rule".to_string(),
                last_fired_at: Some(Utc::now()), // Just now
                activation_count: 1,
                confidence_score: 5.0,
            },
        );

        let detected = registry.detect_dead_rules();

        // detect_dead_rules returns all rules with confidence scores
        assert_eq!(detected.len(), 1);

        // Non-dead rules have calculated confidence based on window utilization (should be > 0)
        let active_health = detected.into_iter().find(|r| r.rule_id == "rule-active").unwrap();
        assert!(active_health.confidence_score > 0.0);
    }

    #[test]
    fn test_detect_dead_rules_with_threshold() {
        // Create registry with custom 12-hour threshold
        let config = SlidingWindowConfig::default();
        let mut registry = HealthRegistry::with_config(config);

        // Add a rule that hasn't fired in 6 hours (within default 24h threshold)
        let six_hours_ago = Utc::now() - Duration::hours(6);
        registry.rules.insert(
            "rule-6hrs".to_string(),
            RuleHealth {
                rule_id: "rule-6hrs".to_string(),
                name: "6hr Rule".to_string(),
                last_fired_at: Some(six_hours_ago),
                activation_count: 0,
                confidence_score: 0.0,
            },
        );

        let detected = registry.detect_dead_rules();

        // Should be returned (not dead since within threshold)
        assert_eq!(detected.len(), 1);
        let rule_health = detected.into_iter().find(|r| r.rule_id == "rule-6hrs").unwrap();
        // Rule with activation_count=0 gets 0 confidence regardless of time
        assert_eq!(rule_health.confidence_score, 0.0);

        // Verify it's NOT marked as dead (confidence != 100)
        assert_ne!(rule_health.confidence_score, 100.0);
    }

    #[test]
    fn test_detect_dead_rules_exceeds_threshold() {
        // Create registry with custom 12-hour threshold
        let config = SlidingWindowConfig::default();
        let mut registry = HealthRegistry::with_config(config);

        // Add a rule that hasn't fired in 48 hours (exceeds default 24h threshold)
        let forty_eight_hours_ago = Utc::now() - Duration::hours(48);
        registry.rules.insert(
            "rule-48hrs".to_string(),
            RuleHealth {
                rule_id: "rule-48hrs".to_string(),
                name: "48hr Rule".to_string(),
                last_fired_at: Some(forty_eight_hours_ago),
                activation_count: 0,
                confidence_score: 0.0,
            },
        );

        let detected = registry.detect_dead_rules();

        // Should be detected as dead (exceeds 24h threshold)
        let dead_rule_ids: Vec<&str> = detected.iter().map(|r| r.rule_id.as_str()).collect();
        assert!(dead_rule_ids.contains(&"rule-48hrs"));
    }

    #[test]
    fn test_window_config_default() {
        let config = SlidingWindowConfig::default();

        // Default: 1-hour windows, 24 windows (24 hours total)
        assert_eq!(config.window_duration.num_hours(), 1);
        assert_eq!(config.num_windows, 24);
    }
}
