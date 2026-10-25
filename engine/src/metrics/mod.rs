#![allow(unexpected_cfgs)]
/// Trait for collecting runtime metrics from the rule engine.
pub trait MetricsCollector: Send + Sync {
    /// Current rate of rule firings per second (moving average).
    fn firing_rate(&self) -> f64;

    /// Total memory usage in bytes.
    fn memory_bytes(&self) -> u64;

    /// Number of facts in alpha memory (production memories).
    fn alpha_memory_size(&self) -> u64;

    /// Number of nodes in beta memory (join memories).
    fn beta_memory_size(&self) -> u64;

    /// Total number of network traversals since engine start.
    fn network_traversals(&self) -> u64;

    /// Number of cache hits during rule matching.
    fn cache_hits(&self) -> u64;

    /// Number of cache misses during rule matching.
    fn cache_misses(&self) -> u64;
}

/// A simple metrics collector implementation for basic tracking.
#[derive(Debug, Default)]
pub struct SimpleMetricsCollector {
    rules_fired: usize,
    facts_inserted: usize,
    facts_retracted: usize,
    alpha_memory_size: usize,
    beta_memory_size: usize,
    network_traversals: u64,
    execution_time_ms: f64,
    cache_hits: u64,
    cache_misses: u64,
}

impl SimpleMetricsCollector {
    /// Creates a new collector with zeroed metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a rule firing event.
    pub fn record_rule_fired(&mut self) {
        self.rules_fired += 1;
    }

    /// Records a fact insertion.
    pub fn record_fact_inserted(&mut self) {
        self.facts_inserted += 1;
    }

    /// Records a fact retraction.
    pub fn record_fact_retracted(&mut self) {
        self.facts_retracted += 1;
    }

    /// Records alpha memory additions.
    pub fn record_alpha_memory_additions(&mut self, count: usize) {
        self.alpha_memory_size = self.alpha_memory_size.saturating_add(count);
    }

    /// Records beta memory additions.
    pub fn record_beta_memory_additions(&mut self, count: usize) {
        self.beta_memory_size = self.beta_memory_size.saturating_add(count);
    }

    /// Records a network traversal step.
    pub fn record_network_traversal(&mut self) {
        self.network_traversals = self.network_traversals.saturating_add(1);
    }

    /// Records a cache hit/miss.
    pub fn record_cache_hit(&mut self) {
        self.cache_hits = self.cache_hits.saturating_add(1);
    }

    pub fn record_cache_miss(&mut self) {
        self.cache_misses = self.cache_misses.saturating_add(1);
    }

    /// Sets the execution time for the current run.
    pub fn set_execution_time_ms(&mut self, ms: f64) {
        self.execution_time_ms = ms;
    }
}

impl MetricsCollector for SimpleMetricsCollector {
    fn firing_rate(&self) -> f64 {
        if self.execution_time_ms > 0.0 {
            (self.rules_fired as f64 / self.execution_time_ms) * 1000.0
        } else {
            0.0
        }
    }

    fn memory_bytes(&self) -> u64 {
        // Rough estimate: alpha + beta memory sizes in bytes
        ((self.alpha_memory_size + self.beta_memory_size) * 8) as u64
    }

    fn alpha_memory_size(&self) -> u64 {
        self.alpha_memory_size as u64
    }

    fn beta_memory_size(&self) -> u64 {
        self.beta_memory_size as u64
    }

    fn network_traversals(&self) -> u64 {
        self.network_traversals
    }

    fn cache_hits(&self) -> u64 {
        self.cache_hits
    }

    fn cache_misses(&self) -> u64 {
        self.cache_misses
    }
}

// Re-export Prometheus exporter for use in API
#[cfg(feature = "metrics")]
pub mod prometheus;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_firing_rate_zero_time() {
        let collector = SimpleMetricsCollector::new();
        // When execution time is 0, should return 0.0
        assert_eq!(collector.firing_rate(), 0.0);
    }

    #[test]
    fn test_metrics_collector_firing_rate_with_time() {
        let mut collector = SimpleMetricsCollector::new();
        collector.record_rule_fired();
        collector.record_rule_fired();
        collector.set_execution_time_ms(1000.0); // 1 second
        // 2 rules fired in 1 second = 2.0 rules/second
        assert_eq!(collector.firing_rate(), 2.0);
    }

    #[test]
    fn test_metrics_collector_memory_bytes() {
        let mut collector = SimpleMetricsCollector::new();
        collector.alpha_memory_size = 10;
        collector.beta_memory_size = 5;
        // (10 + 5) * 8 = 120 bytes
        assert_eq!(collector.memory_bytes(), 120);
    }

    #[test]
    fn test_metrics_collector_alpha_memory_size() {
        let mut collector = SimpleMetricsCollector::new();
        collector.alpha_memory_size = 42;
        assert_eq!(collector.alpha_memory_size(), 42);
    }

    #[test]
    fn test_metrics_collector_beta_memory_size() {
        let mut collector = SimpleMetricsCollector::new();
        collector.beta_memory_size = 17;
        assert_eq!(collector.beta_memory_size(), 17);
    }

    #[test]
    fn test_metrics_collector_network_traversals() {
        let mut collector = SimpleMetricsCollector::new();
        collector.network_traversals = 999;
        assert_eq!(collector.network_traversals(), 999);
    }

    #[test]
    fn test_metrics_collector_cache_hits() {
        let mut collector = SimpleMetricsCollector::new();
        collector.cache_hits = 123;
        assert_eq!(collector.cache_hits(), 123);
    }

    #[test]
    fn test_metrics_collector_cache_misses() {
        let mut collector = SimpleMetricsCollector::new();
        collector.cache_misses = 456;
        assert_eq!(collector.cache_misses(), 456);
    }

    #[test]
    fn test_simple_metrics_collector_recorders() {
        let mut collector = SimpleMetricsCollector::new();

        collector.record_rule_fired();
        collector.record_fact_inserted();
        collector.record_fact_retracted();
        collector.record_alpha_memory_additions(5);
        collector.record_beta_memory_additions(3);
        collector.record_network_traversal();
        collector.record_cache_hit();
        collector.record_cache_miss();
        collector.set_execution_time_ms(100.0);

        assert_eq!(collector.rules_fired, 1);
        assert_eq!(collector.facts_inserted, 1);
        assert_eq!(collector.facts_retracted, 1);
        assert_eq!(collector.alpha_memory_size, 5);
        assert_eq!(collector.beta_memory_size, 3);
        assert_eq!(collector.network_traversals, 1);
        assert_eq!(collector.cache_hits, 1);
        assert_eq!(collector.cache_misses, 1);
    }
}
