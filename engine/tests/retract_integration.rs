//! Integration tests for retract_fact() across all three engine implementations.
//!
//! These tests verify the core contract:
//! - retract_fact returns Ok(true) for valid fact_id
//! - retract_fact returns Ok(false) for non-existent fact_id

use engine::data_types::Value;
use engine::engine::registry::ExecutionEngine;
use engine::fact::Fact;
use std::collections::HashMap;

/// Helper to create a fact with a specific type and payload.
fn make_fact(fact_type: &str, field_name: &str, value: Value) -> Fact {
    let mut payload = HashMap::new();
    payload.insert(field_name.to_string(), value);
    Fact::new(fact_type.to_string(), payload)
}

// ============================================================
// Cross-engine consistency tests
// ============================================================

#[test]
fn test_all_engines_retract_nonexistent_returns_false() {
    // Verify all three engines consistently return false for non-existent facts.

    let mut rete = engine::engine::rete::ReteEngine::new();
    assert!(!rete.retract_fact("nope").unwrap());

    let mut treat = engine::engine::treat::TreatEngine::new();
    assert!(!treat.retract_fact("nope").unwrap());

    let mut leaps = engine::engine::leaps::LeapsEngine::new();
    assert!(!leaps.retract_fact("nope").unwrap());
}

#[test]
fn test_all_engines_retract_valid_returns_true() {
    // Verify all three engines consistently return true for valid facts.

    let mut rete = engine::engine::rete::ReteEngine::new();
    let fact1 = make_fact("x", "v", Value::Integer(1));
    let id1 = fact1.id.to_string();
    rete.insert_fact(fact1).unwrap();
    assert!(rete.retract_fact(&id1).unwrap());

    let mut treat = engine::engine::treat::TreatEngine::new();
    let fact2 = make_fact("x", "v", Value::Integer(2));
    let id2 = fact2.id.to_string();
    treat.insert_fact(fact2).unwrap();
    assert!(treat.retract_fact(&id2).unwrap());

    let mut leaps = engine::engine::leaps::LeapsEngine::new();
    let fact3 = make_fact("x", "v", Value::Integer(3));
    let id3 = fact3.id.to_string();
    leaps.insert_fact(fact3).unwrap();
    assert!(leaps.retract_fact(&id3).unwrap());
}

#[test]
fn test_retract_nonexistent_does_not_change_facts_inserted() {
    // Retracting a non-existent fact should not change facts_inserted count.
    let mut engine = engine::engine::rete::ReteEngine::new();

    let initial_metrics = engine.get_metrics();
    let before_inserted = initial_metrics.facts_inserted;

    engine.retract_fact("nonexistent").unwrap();

    let after_metrics = engine.get_metrics();
    assert_eq!(after_metrics.facts_inserted, before_inserted);
}

// ============================================================
// Rete Engine Tests
// ============================================================

#[test]
fn test_rete_double_retract_second_returns_false() {
    // After retracting a fact, re-retracting should return false.
    let mut engine = engine::engine::rete::ReteEngine::new();

    let fact = make_fact("item", "value", Value::Integer(42));
    let id = fact.id.to_string();
    engine.insert_fact(fact).unwrap();

    // First retract should succeed.
    assert!(engine.retract_fact(&id).unwrap());

    // Second retract of same ID should fail.
    assert!(!engine.retract_fact(&id).unwrap());
}

// ============================================================
// TREAT Engine Tests
// ============================================================

#[test]
fn test_treat_double_retract_second_returns_false() {
    let mut engine = engine::engine::treat::TreatEngine::new();

    let fact = make_fact("item", "value", Value::Integer(42));
    let id = fact.id.to_string();
    engine.insert_fact(fact).unwrap();

    assert!(engine.retract_fact(&id).unwrap());
    assert!(!engine.retract_fact(&id).unwrap());
}

// ============================================================
// LEAPS Engine Tests
// ============================================================

#[test]
fn test_leaps_retract_multiple_facts() {
    use engine::engine::leaps::{AggregationNode, AggregationOp, LeapsEngine, LeapsNetwork};

    let mut network = LeapsNetwork::new();
    let agg_node = AggregationNode::new("agg1", AggregationOp::Sum, "amount");
    network.add_agg_node(agg_node);

    let mut engine = LeapsEngine::with_network(network);

    // Insert three facts.
    let mut fact_ids = Vec::new();
    for i in 0..3 {
        let fact = make_fact("item", "amount", Value::Integer(i * 10));
        let id = fact.id.to_string();
        engine.insert_fact(fact).unwrap();
        fact_ids.push(id);
    }

    // Retract all three one by one.
    for id in &fact_ids {
        let result = engine.retract_fact(id).unwrap();
        assert!(result, "retract_fact should succeed for existing fact");
    }
}

// ============================================================
// Edge case tests
// ============================================================

#[test]
fn test_retract_empty_engine_returns_false() {
    // Retracting from an engine with no facts should return false for all engines.
    let mut rete = engine::engine::rete::ReteEngine::new();
    assert!(!rete.retract_fact("anything").unwrap());

    let mut treat = engine::engine::treat::TreatEngine::new();
    assert!(!treat.retract_fact("anything").unwrap());

    let mut leaps = engine::engine::leaps::LeapsEngine::new();
    assert!(!leaps.retract_fact("anything").unwrap());
}

#[test]
fn test_retract_after_reset_returns_false() {
    // After resetting an engine, retracting previously-inserted fact IDs should fail.
    let mut rete = engine::engine::rete::ReteEngine::new();

    let fact = make_fact("x", "v", Value::Integer(1));
    let id = fact.id.to_string();
    rete.insert_fact(fact).unwrap();
    rete.reset();

    // After reset, the ID no longer exists.
    assert!(!rete.retract_fact(&id).unwrap());
}
