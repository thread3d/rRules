//! Trace for DMN decision evaluations.
//!
//! **What this records, measured** (`engine/tests/dmn_trace_events.rs` pins it), because the description
//! here used to promise more:
//!
//! - `evaluate_table` — a decision table was evaluated; the event carries the hit policy in its message
//!   and the first output value in `output`.
//! - `merge_scope` — upstream outputs were merged into the evaluating decision's scope; `output` carries
//!   the merged context.
//! - `drd_resolve` — an upstream decision of a decision-requirement graph was resolved.
//!
//! A single-decision evaluation with no upstream requirements still records `evaluate_table` followed by
//! `merge_scope`. The only consumer is the evaluation response: with `trace: true` on
//! `POST /v1/decisions/{id}/evaluate`, `TraceResponse` reports `event_count` (`events.len()`) and
//! `duration_ms` — the events themselves, and the outputs they carry, are not published.
//!
//! It does **not** record `match_rule`, `aggregate` or `resolve_reference` events, and there is no
//! `GET /v1/decisions/{id}/trace` endpoint — that sentence described a route nobody wrote. The
//! `rule_index` and `inputs` fields existed for events that are never emitted and are deleted; `output`
//! stays, because two event kinds populate it.

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
            output: None,
        });
    }

    /// Emit a trace event that carries an output value.
    pub fn emit_with(
        &mut self,
        event_type: impl Into<String>,
        message: impl Into<String>,
        output: Option<Value>,
    ) {
        let elapsed_ms = self.started_at.elapsed().as_secs_f64() * 1000.0;
        self.events.push(TraceEvent {
            elapsed_ms,
            event_type: event_type.into(),
            message: message.into(),
            output,
        });
    }

    /// Total duration of the evaluation in milliseconds.
    pub fn duration_ms(&self) -> f64 {
        self.started_at.elapsed().as_secs_f64() * 1000.0
    }
}
