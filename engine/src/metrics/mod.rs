/// Trait for collecting runtime metrics from the rule engine.
pub trait MetricsCollector {
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

// Re-export Prometheus exporter for use in API
#[cfg(feature = "metrics")]
pub mod prometheus;
