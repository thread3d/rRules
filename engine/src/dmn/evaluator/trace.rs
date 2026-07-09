//! Inference trace for DMN decision evaluations (Phase 33).
//!
//! Mirrors Phase 28's `TraceEvent`/`TraceData` pattern but scoped to the DMN path.
//! Each evaluation emits a sequence of trace events; when `trace: true` is set on the
//! evaluate request, these are stored in-memory and served via `GET /v1/decisions/{id}/trace`.

use std::time::Instant;

use crate::data_types::Value;

/// A single event in a decision evaluation trace.
#[derive(Debug, Clone)]
pub struct TraceEvent {
    /// Timestamp when this event was emitted (relative to evaluation start).
    pub elapsed_ms: f64,
    /// Event type: `evaluate_table`, `match_rule`, `aggregate`, `resolve_reference`, etc.
    pub event_type: String,
    /// Human-readable description of what happened.
    pub message: String,
    /// Optional: the rule index that was matched (for hit-policy events).
    pub rule_index: Option<usize>,
    /// Optional: input values evaluated for this event.
    pub inputs: Vec<(String, Value)>,
    /// Optional: output value produced.
    pub output: Option<Value>,
}

/// A complete inference trace for a single decision evaluation.
#[derive(Debug, Clone)]
pub struct InferenceTrace {
    /// Decision ID being evaluated.
    pub decision_id: String,
    /// Start time of the evaluation (absolute).
    pub started_at: Instant,
    /// All events emitted during this evaluation.
    pub events: Vec<TraceEvent>,
}

impl InferenceTrace {
    pub fn new(decision_id: impl Into<String>) -> Self {
        Self {
            decision_id: decision_id.into(),
            started_at: Instant::now(),
            events: Vec::new(),
        }
    }

    /// Emit a trace event with the current elapsed time.
    pub fn emit(&mut self, event_type: impl Into<String>, message: impl Into<String>) {
        let elapsed_ms = self.started_at.elapsed().as_secs_f64() * 1000.0;
        self.events.push(TraceEvent {
            elapsed_ms,
            event_type: event_type.into(),
            message: message.into(),
            rule_index: None,
            inputs: Vec::new(),
            output: None,
        });
    }

    /// Emit a trace event with additional context.
    pub fn emit_with(
        &mut self,
        event_type: impl Into<String>,
        message: impl Into<String>,
        rule_index: Option<usize>,
        inputs: Vec<(String, Value)>,
        output: Option<Value>,
    ) {
        let elapsed_ms = self.started_at.elapsed().as_secs_f64() * 1000.0;
        self.events.push(TraceEvent {
            elapsed_ms,
            event_type: event_type.into(),
            message: message.into(),
            rule_index,
            inputs,
            output,
        });
    }

    /// Total duration of the evaluation in milliseconds.
    pub fn duration_ms(&self) -> f64 {
        self.started_at.elapsed().as_secs_f64() * 1000.0
    }
}
