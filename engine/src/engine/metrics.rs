//! Metrics collection and reporting for engine implementations.
//!
//! This module provides traits for collecting performance metrics from different
//! engine algorithms, with support for Prometheus export.

#![allow(unexpected_cfgs, dead_code)]
#![allow(unused_imports, unreachable_pub, unused_variables, unused_mut)]
use std::collections::HashMap;
#[cfg(feature = "metrics")]
use prometheus::{self, Encoder, TextEncoder};

/// Metrics collected from a single engine execution.
#[derive(Debug, Clone, Default)]
pub struct EngineMetrics {
    /// Number of rules fired during execution
    pub rules_fired: usize,
    /// Total number of fact insertions
    pub facts_inserted: usize,
    /// Total number of fact retractions
    pub facts_retracted: usize,
    /// Alpha memory size (node entries)
    pub alpha_memory_size: usize,
    /// Beta memory size (join node partial matches)
    pub beta_memory_size: usize,
    /// Number of network traversals
    pub network_traversals: u64,
    /// Execution time in milliseconds
    pub execution_time_ms: f64,
    /// Cache hit count (for caching algorithms like TREAT)
    pub cache_hits: u64,
    /// Cache miss count
    pub cache_misses: u64,
}

impl EngineMetrics {
    /// Creates new empty metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the total number of operations (insertions + retractions).
    pub fn total_operations(&self) -> usize {
        self.facts_inserted + self.facts_retracted
    }

    /// Calculates firing rate (rules per second).
    pub fn firing_rate(&self) -> f64 {
        if self.execution_time_ms > 0.0 {
            (self.rules_fired as f64 / self.execution_time_ms) * 1000.0
        } else {
            0.0
        }
    }

    /// Gets total memory usage estimate.
    pub fn total_memory_estimate(&self) -> usize {
        // Rough estimate: alpha + beta memory sizes
        self.alpha_memory_size + self.beta_memory_size
    }
}

/// Trait for collecting engine metrics.
pub trait MetricsCollector {
    /// Returns current metrics.
    fn get_metrics(&self) -> EngineMetrics;

    /// Resets all counters to zero.
    fn reset_metrics(&mut self);

    /// Records a rule firing event.
    fn record_rule_fired(&mut self);

    /// Records a fact insertion.
    fn record_fact_inserted(&mut self);

    /// Records a fact retraction.
    fn record_fact_retracted(&mut self);

    /// Records alpha memory additions.
    fn record_alpha_memory_additions(&mut self, count: usize);

    /// Records beta memory additions.
    fn record_beta_memory_additions(&mut self, count: usize);

    /// Records a network traversal step.
    fn record_network_traversal(&mut self);

    /// Records a cache hit/miss.
    fn record_cache_hit(&mut self);
    fn record_cache_miss(&mut self);

    /// Sets the execution time for the current run.
    fn set_execution_time_ms(&mut self, ms: f64);
}

/// A simple metrics collector implementation.
#[derive(Debug, Clone)]
pub struct SimpleMetricsCollector {
    metrics: EngineMetrics,
}

impl SimpleMetricsCollector {
    /// Creates a new collector with zeroed metrics.
    pub fn new() -> Self {
        Self {
            metrics: EngineMetrics::new(),
        }
    }

    /// Consumes self and returns the collected metrics.
    pub fn into_metrics(self) -> EngineMetrics {
        self.metrics
    }
}

impl Default for SimpleMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector for SimpleMetricsCollector {
    fn get_metrics(&self) -> EngineMetrics {
        self.metrics.clone()
    }

    fn reset_metrics(&mut self) {
        self.metrics = EngineMetrics::new();
    }

    fn record_rule_fired(&mut self) {
        self.metrics.rules_fired += 1;
    }

    fn record_fact_inserted(&mut self) {
        self.metrics.facts_inserted += 1;
    }

    fn record_fact_retracted(&mut self) {
        self.metrics.facts_retracted += 1;
    }

    fn record_alpha_memory_additions(&mut self, count: usize) {
        self.metrics.alpha_memory_size += count;
    }

    fn record_beta_memory_additions(&mut self, count: usize) {
        self.metrics.beta_memory_size += count;
    }

    fn record_network_traversal(&mut self) {
        self.metrics.network_traversals += 1;
    }

    fn record_cache_hit(&mut self) {
        self.metrics.cache_hits += 1;
    }

    fn record_cache_miss(&mut self) {
        self.metrics.cache_misses += 1;
    }

    fn set_execution_time_ms(&mut self, ms: f64) {
        self.metrics.execution_time_ms = ms;
    }
}

/// Trait for exporting metrics in various formats.
pub trait MetricsExporter {
    /// Exports metrics as a Prometheus-formatted string.
    fn to_prometheus(&self, engine_name: &str, labels: Option<&HashMap<String, String>>) -> String;

    /// Exports metrics as JSON.
    fn to_json(&self) -> String;
}

impl MetricsExporter for EngineMetrics {
    fn to_prometheus(&self, engine_name: &str, labels: Option<&HashMap<String, String>>) -> String {
        let mut output = String::new();

        // Add labels
        let label_str = labels.map(|l| {
            l.iter()
                .map(|(k, v)| format!(r#"{}="{}""#, k, v))
                .collect::<Vec<_>>()
                .join(",")
        }).unwrap_or_default();

        if !label_str.is_empty() {
            output.push_str(&format!("# HELP {}_metrics Engine execution metrics\n", engine_name));
            output.push_str(&format!("# TYPE {}_metrics counter\n", engine_name));
        }

        // Rules fired
        output.push_str(&format!(
            "{}_rules_fired{} {}\n",
            engine_name,
            if !label_str.is_empty() { format!("{{{}}}", label_str) } else { String::new() },
            self.rules_fired
        ));

        // Facts operations
        output.push_str(&format!(
            "{}_facts_inserted{} {}\n",
            engine_name,
            if !label_str.is_empty() { format!("{{{}}}", label_str) } else { String::new() },
            self.facts_inserted
        ));

        output.push_str(&format!(
            "{}_facts_retracted{} {}\n",
            engine_name,
            if !label_str.is_empty() { format!("{{{}}}", label_str) } else { String::new() },
            self.facts_retracted
        ));

        // Memory sizes
        output.push_str(&format!(
            "{}_alpha_memory_size{} {}\n",
            engine_name,
            if !label_str.is_empty() { format!("{{{}}}", label_str) } else { String::new() },
            self.alpha_memory_size
        ));

        output.push_str(&format!(
            "{}_beta_memory_size{} {}\n",
            engine_name,
            if !label_str.is_empty() { format!("{{{}}}", label_str) } else { String::new() },
            self.beta_memory_size
        ));

        // Network traversals
        output.push_str(&format!(
            "{}_network_traversals{} {}\n",
            engine_name,
            if !label_str.is_empty() { format!("{{{}}}", label_str) } else { String::new() },
            self.network_traversals
        ));

        // Execution time (in seconds for Prometheus convention)
        output.push_str(&format!(
            "{}_execution_seconds{} {:.6}\n",
            engine_name,
            if !label_str.is_empty() { format!("{{{}}}", label_str) } else { String::new() },
            self.execution_time_ms / 1000.0
        ));

        // Cache stats (for TREAT)
        output.push_str(&format!(
            "{}_cache_hits{} {}\n",
            engine_name,
            if !label_str.is_empty() { format!("{{{}}}", label_str) } else { String::new() },
            self.cache_hits
        ));

        output.push_str(&format!(
            "{}_cache_misses{} {}\n",
            engine_name,
            if !label_str.is_empty() { format!("{{{}}}", label_str) } else { String::new() },
            self.cache_misses
        ));

        output
    }

    fn to_json(&self) -> String {
        serde_json::json!({
            "rules_fired": self.rules_fired,
            "facts_inserted": self.facts_inserted,
            "facts_retracted": self.facts_retracted,
            "alpha_memory_size": self.alpha_memory_size,
            "beta_memory_size": self.beta_memory_size,
            "network_traversals": self.network_traversals,
            "execution_time_ms": self.execution_time_ms,
            "cache_hits": self.cache_hits,
            "cache_misses": self.cache_misses,
            "firing_rate_per_second": self.firing_rate(),
            "total_memory_estimate": self.total_memory_estimate()
        })
        .to_string()
    }
}

/// Export metrics to Prometheus format using the prometheus crate (when enabled).
#[cfg(feature = "metrics")]
pub fn export_prometheus_metrics(
    engine_name: &str,
    labels: Option<&HashMap<String, String>>,
) -> String {
    let opts = prometheus::Opts {
        namespace: "rrules",
        subsystem: engine_name.to_string(),
        name: format!("{}_metrics", engine_name),
        help: format!("{} execution metrics", engine_name),
        const_labels: labels
            .map(|l| {
                l.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    };

    let counter = prometheus::IntCounterVec::new(opts, &[]).expect("Failed to create counter");
    // In production this would track metrics across requests
    counter.reset();
    String::from("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collection() {
        let mut collector = SimpleMetricsCollector::new();

        collector.record_rule_fired();
        collector.record_rule_fired();
        collector.record_fact_inserted();
        collector.record_alpha_memory_additions(10);
        collector.record_beta_memory_additions(5);

        let metrics = collector.get_metrics();
        assert_eq!(metrics.rules_fired, 2);
        assert_eq!(metrics.facts_inserted, 1);
        assert_eq!(metrics.alpha_memory_size, 10);
        assert_eq!(metrics.beta_memory_size, 5);
    }

    #[test]
    fn test_metrics_reset() {
        let mut collector = SimpleMetricsCollector::new();

        collector.record_rule_fired();
        collector.reset_metrics();

        let metrics = collector.get_metrics();
        assert_eq!(metrics.rules_fired, 0);
    }

    #[test]
    fn test_firing_rate_calculation() {
        let mut collector = SimpleMetricsCollector::new();

        // Fire 10 rules in 2 seconds
        for _ in 0..10 {
            collector.record_rule_fired();
        }
        collector.set_execution_time_ms(2000.0);

        let metrics = collector.get_metrics();
        assert_eq!(metrics.firing_rate(), 5.0); // 5 rules per second
    }

    #[test]
    fn test_prometheus_export() {
        let mut metrics = EngineMetrics::new();
        metrics.rules_fired = 42;
        metrics.alpha_memory_size = 100;

        let prometheus = metrics.to_prometheus("rete", None);
        assert!(prometheus.contains("rete_rules_fired"));
        assert!(prometheus.contains("42"));
    }

    #[test]
    fn test_json_export() {
        let mut metrics = EngineMetrics::new();
        metrics.rules_fired = 42;
        metrics.execution_time_ms = 100.0;

        let json = metrics.to_json();
        assert!(json.contains("\"rules_fired\":42"));
        assert!(json.contains("\"execution_time_ms\":100.0"));
    }
}
