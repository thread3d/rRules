//! Prometheus exporter for engine metrics.
//!
//! This module provides functionality to expose engine metrics in Prometheus format.

use prometheus::{Encoder, TextEncoder};

use super::MetricsCollector;

/// Prometheus metrics for engine statistics.
#[derive(Debug, Clone)]
pub struct EnginePrometheusMetrics {
    /// Firing rate metric
    pub firing_rate: f64,
    /// Memory usage in bytes
    pub memory_bytes: u64,
    /// Alpha memory size (production memories)
    pub alpha_memory_size: u64,
    /// Beta memory size (join memories)
    pub beta_memory_size: u64,
    /// Total network traversals
    pub network_traversals: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
}

impl EnginePrometheusMetrics {
    /// Create Prometheus metrics from a MetricsCollector.
    pub fn from_collector(collector: &dyn MetricsCollector) -> Self {
        Self {
            firing_rate: collector.firing_rate(),
            memory_bytes: collector.memory_bytes(),
            alpha_memory_size: collector.alpha_memory_size(),
            beta_memory_size: collector.beta_memory_size(),
            network_traversals: collector.network_traversals(),
            cache_hits: collector.cache_hits(),
            cache_misses: collector.cache_misses(),
        }
    }

    /// Export metrics to Prometheus text format.
    pub fn export_prometheus_text(&self) -> String {
        let mut buffer = Vec::new();
        let encoder = TextEncoder::new();

        // Firing rate (gauge - rate per second)
        buffer.extend_from_slice(
            b"# HELP engine_firing_rate_current Current firing rate (rules/sec)\n",
        );
        buffer.extend_from_slice(b"# TYPE engine_firing_rate_current gauge\n");
        buffer.extend_from_slice(
            format!("engine_firing_rate_current {}\n", self.firing_rate)
                .into_bytes()
                .as_slice(),
        );

        // Memory usage
        buffer.extend_from_slice(b"# HELP engine_memory_bytes Current memory usage in bytes\n");
        buffer.extend_from_slice(b"# TYPE engine_memory_bytes gauge\n");
        buffer.extend_from_slice(
            format!("engine_memory_bytes {}\n", self.memory_bytes)
                .into_bytes()
                .as_slice(),
        );

        // Alpha memory size
        buffer.extend_from_slice(
            b"# HELP engine_alpha_memory_size Number of facts in alpha memory\n",
        );
        buffer.extend_from_slice(b"# TYPE engine_alpha_memory_size gauge\n");
        buffer.extend_from_slice(
            format!("engine_alpha_memory_size {}\n", self.alpha_memory_size)
                .into_bytes()
                .as_slice(),
        );

        // Beta memory size
        buffer
            .extend_from_slice(b"# HELP engine_beta_memory_size Number of nodes in beta memory\n");
        buffer.extend_from_slice(b"# TYPE engine_beta_memory_size gauge\n");
        buffer.extend_from_slice(
            format!("engine_beta_memory_size {}\n", self.beta_memory_size)
                .into_bytes()
                .as_slice(),
        );

        // Network traversals
        buffer.extend_from_slice(
            b"# HELP engine_network_traversals_total Total network traversals\n",
        );
        buffer.extend_from_slice(b"# TYPE engine_network_traversals_total counter\n");
        buffer.extend_from_slice(
            format!(
                "engine_network_traversals_total {}\n",
                self.network_traversals
            )
            .into_bytes()
            .as_slice(),
        );

        // Cache hits
        buffer.extend_from_slice(b"# HELP engine_cache_hits_total Total cache hits\n");
        buffer.extend_from_slice(b"# TYPE engine_cache_hits_total counter\n");
        buffer.extend_from_slice(
            format!("engine_cache_hits_total {}\n", self.cache_hits)
                .into_bytes()
                .as_slice(),
        );

        // Cache misses
        buffer.extend_from_slice(b"# HELP engine_cache_misses_total Total cache misses\n");
        buffer.extend_from_slice(b"# TYPE engine_cache_misses_total counter\n");
        buffer.extend_from_slice(
            format!("engine_cache_misses_total {}\n", self.cache_misses)
                .into_bytes()
                .as_slice(),
        );

        String::from_utf8(buffer).unwrap_or_else(|_| String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prometheus_export_empty() {
        // Create a minimal implementation for testing
        struct DummyCollector;

        impl MetricsCollector for DummyCollector {
            fn firing_rate(&self) -> f64 {
                0.0
            }
            fn memory_bytes(&self) -> u64 {
                0
            }
            fn alpha_memory_size(&self) -> u64 {
                0
            }
            fn beta_memory_size(&self) -> u64 {
                0
            }
            fn network_traversals(&self) -> u64 {
                0
            }
            fn cache_hits(&self) -> u64 {
                0
            }
            fn cache_misses(&self) -> u64 {
                0
            }
        }

        let metrics = EnginePrometheusMetrics::from_collector(&DummyCollector);
        let text = metrics.export_prometheus_text();

        assert!(text.contains("engine_firing_rate_current"));
        assert!(text.contains("engine_memory_bytes"));
    }
}
