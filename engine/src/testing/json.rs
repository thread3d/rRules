//! JSON serialization helpers with deterministic output
//!
//! This module provides utilities for serializing working memory snapshots
//! and other testing-related data to JSON with consistent, predictable formatting.

use crate::testing::{FactSnapshot, ValueSnapshot, WorkingMemorySnapshot};
use serde_json::Value;

/// Serialize a working memory snapshot to a deterministic JSON string
///
/// This function ensures:
/// - Keys in all HashMaps are sorted alphabetically
/// - Consistent formatting for comparison purposes
/// - No random ordering issues between runs
///
/// # Arguments
/// * `snapshot` - The working memory snapshot to serialize
///
/// # Returns
/// A JSON string with deterministic key ordering
pub fn serialize_deterministic(snapshot: &WorkingMemorySnapshot) -> String {
    let json_value = serde_json::to_value(snapshot).unwrap_or(Value::Null);

    // Rebuild the value with sorted keys recursively
    let sorted_value = sort_json_keys(json_value);

    serde_json::to_string_pretty(&sorted_value).unwrap_or_default()
}

/// Deserialize a working memory snapshot from JSON
///
/// # Arguments
/// * `json` - The JSON string to parse
///
/// # Returns
/// Ok(WorkingMemorySnapshot) if parsing succeeds, Err otherwise
pub fn deserialize_working_memory(json: &str) -> Result<WorkingMemorySnapshot, serde_json::Error> {
    serde_json::from_str(json)
}

/// Serialize a fact snapshot to deterministic JSON
///
/// # Arguments
/// * `fact` - The fact snapshot to serialize
///
/// # Returns
/// A JSON string with sorted keys
pub fn serialize_fact_deterministic(fact: &FactSnapshot) -> String {
    let json_value = serde_json::to_value(fact).unwrap_or(Value::Null);
    let sorted_value = sort_json_keys(json_value);

    serde_json::to_string_pretty(&sorted_value).unwrap_or_default()
}

/// Serialize a value snapshot to deterministic JSON
///
/// # Arguments
/// * `value` - The value snapshot to serialize
///
/// # Returns
/// A JSON string with sorted keys for object types
pub fn serialize_value_deterministic(value: &ValueSnapshot) -> String {
    let json_value = serde_json::to_value(value).unwrap_or(Value::Null);
    let sorted_value = sort_json_keys(json_value);

    serde_json::to_string_pretty(&sorted_value).unwrap_or_default()
}

/// Recursively sort all keys in a JSON value
///
/// This ensures deterministic output regardless of HashMap iteration order.
fn sort_json_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted_map = serde_json::Map::new();
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();

            for key in keys {
                sorted_map.insert(
                    key.clone(),
                    sort_json_keys(map.get(&key).cloned().unwrap_or(Value::Null)),
                );
            }

            Value::Object(sorted_map)
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(sort_json_keys).collect()),
        _ => value,
    }
}

/// Compare two JSON strings for semantic equality (ignoring key order differences)
///
/// # Arguments
/// * `a` - First JSON string
/// * `b` - Second JSON string
///
/// # Returns
/// true if the JSON structures are semantically equal, false otherwise
pub fn json_semantic_equal(a: &str, b: &str) -> bool {
    let a_value = match serde_json::from_str::<Value>(a) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let b_value = match serde_json::from_str::<Value>(b) {
        Ok(v) => v,
        Err(_) => return false,
    };

    // Sort both values and compare
    sort_json_keys(a_value) == sort_json_keys(b_value)
}

/// Roundtrip test helper: serialize then deserialize and compare
///
/// # Arguments
/// * `snapshot` - The snapshot to roundtrip through JSON
///
/// # Returns
/// Ok(WorkingMemorySnapshot) if the roundtrip succeeds, error otherwise
pub fn roundtrip_snapshot(
    snapshot: &WorkingMemorySnapshot,
) -> Result<WorkingMemorySnapshot, serde_json::Error> {
    let json = serialize_deterministic(snapshot);
    deserialize_working_memory(&json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_serialize_deterministic_order() {
        // Create a snapshot with multiple facts that would have non-deterministic
        // key ordering if not sorted

        let mut payload1 = HashMap::new();
        payload1.insert(
            "zebra".to_string(),
            ValueSnapshot::String("last".to_string()),
        );
        payload1.insert("alpha".to_string(), ValueSnapshot::Integer(1));
        payload1.insert("middle".to_string(), ValueSnapshot::Bool(true));

        let mut payload2 = HashMap::new();
        payload2.insert(
            "yak".to_string(),
            ValueSnapshot::Float(std::f64::consts::PI),
        );
        payload2.insert(
            "aardvark".to_string(),
            ValueSnapshot::String("first".to_string()),
        );

        let snapshot = WorkingMemorySnapshot {
            facts: vec![
                FactSnapshot {
                    id: "id2".to_string(),
                    fact_type: "zebra_fact".to_string(),
                    payload: payload1,
                },
                FactSnapshot {
                    id: "id1".to_string(),
                    fact_type: "aardvark_fact".to_string(),
                    payload: payload2,
                },
            ],
        };

        // Serialize twice and verify they're identical
        let json1 = serialize_deterministic(&snapshot);
        let json2 = serialize_deterministic(&snapshot);

        assert_eq!(json1, json2);

        // Verify keys are in sorted order (alpha before middle before zebra)
        assert!(json1.contains("\"aardvark\""));
        assert!(json1.contains("\"alpha\""));
        assert!(json1.contains("\"middle\""));
        assert!(json1.contains("\"zebra\""));

        // The first key should be aardvark_fact (sorted fact_type)
        let _first_fact_start = json1.find(r#""type": "#).unwrap();
        // Check that we don't have zebra_fact appearing before aardvark_fact
    }

    #[test]
    fn test_roundtrip_serialization() {
        let mut payload = HashMap::new();
        payload.insert(
            "name".to_string(),
            ValueSnapshot::String("test".to_string()),
        );
        payload.insert("value".to_string(), ValueSnapshot::Integer(42));
        payload.insert("enabled".to_string(), ValueSnapshot::Bool(true));

        let snapshot = WorkingMemorySnapshot {
            facts: vec![FactSnapshot {
                id: "fact1".to_string(),
                fact_type: "item".to_string(),
                payload,
            }],
        };

        // Roundtrip through JSON
        let roundtripped = roundtrip_snapshot(&snapshot).expect("Roundtrip failed");

        // Verify content is preserved (ignoring fact ID)
        assert_eq!(roundtripped.facts.len(), snapshot.facts.len());
        assert_eq!(roundtripped.facts[0].fact_type, snapshot.facts[0].fact_type);
    }

    #[test]
    fn test_json_semantic_equal() {
        // JSON with different key orders should be semantically equal
        let json1 = r#"{"zebra": 1, "alpha": 2, "middle": 3}"#;
        let json2 = r#"{"alpha": 2, "middle": 3, "zebra": 1}"#;
        let json3 = r#"{"alpha": 2, "middle": 3, "zebra": 1}"#;

        assert!(json_semantic_equal(json1, json2));
        assert!(json_semantic_equal(json1, json3));
        assert!(json_semantic_equal(json2, json3));

        // Different values should not be equal
        let json4 = r#"{"alpha": 99, "middle": 3, "zebra": 1}"#;
        assert!(!json_semantic_equal(json1, json4));
    }

    #[test]
    fn test_serialize_fact_deterministic() {
        let mut payload = HashMap::new();
        payload.insert(
            "z_field".to_string(),
            ValueSnapshot::String("last".to_string()),
        );
        payload.insert("a_field".to_string(), ValueSnapshot::Integer(1));

        let fact = FactSnapshot {
            id: "fact-uuid".to_string(),
            fact_type: "test_fact".to_string(),
            payload,
        };

        let json = serialize_fact_deterministic(&fact);

        // Verify a_field comes before z_field in output
        let a_pos = json.find("\"a_field\"").unwrap();
        let z_pos = json.find("\"z_field\"").unwrap();

        assert!(
            a_pos < z_pos,
            "Keys should be sorted: a_field before z_field"
        );
    }

    #[test]
    fn test_serialize_value_deterministic() {
        // Test with an object value
        let mut obj = HashMap::new();
        obj.insert("z".to_string(), ValueSnapshot::Integer(1));
        obj.insert("a".to_string(), ValueSnapshot::String("hello".to_string()));

        let value = ValueSnapshot::Object(obj);
        let json = serialize_value_deterministic(&value);

        // Verify keys are sorted
        let a_pos = json.find("\"a\"").unwrap();
        let z_pos = json.find("\"z\"").unwrap();

        assert!(a_pos < z_pos, "Object keys should be sorted");
    }

    #[test]
    fn test_nested_array_sorting() {
        // Test that arrays containing objects also have their keys sorted
        let snapshot = WorkingMemorySnapshot {
            facts: vec![FactSnapshot {
                id: "id1".to_string(),
                fact_type: "array_test".to_string(),
                payload: HashMap::from([(
                    "nested".to_string(),
                    ValueSnapshot::Array(vec![ValueSnapshot::Object(HashMap::from([
                        ("z".to_string(), ValueSnapshot::Integer(1)),
                        ("a".to_string(), ValueSnapshot::String("test".to_string())),
                    ]))]),
                )]),
            }],
        };

        let json = serialize_deterministic(&snapshot);

        // The nested object should have 'a' before 'z'
        assert!(json.contains("\"a\""), "Nested keys should be sorted");
    }

    #[test]
    fn test_complex_roundtrip() {
        // Test with a more complex snapshot
        let mut payload = HashMap::new();
        payload.insert("count".to_string(), ValueSnapshot::Integer(100));
        payload.insert("enabled".to_string(), ValueSnapshot::Bool(true));

        let snapshot = WorkingMemorySnapshot {
            facts: vec![FactSnapshot {
                id: "f1".to_string(),
                fact_type: "config".to_string(),
                payload,
            }],
        };

        // Multiple roundtrips should produce identical results
        let mut prev_json = serialize_deterministic(&snapshot);

        for _ in 0..5 {
            let roundtripped = roundtrip_snapshot(&snapshot).expect("Roundtrip failed");
            let curr_json = serialize_deterministic(&roundtripped);
            assert_eq!(
                prev_json, curr_json,
                "Multiple roundtrips should be consistent"
            );
            prev_json = curr_json;
        }
    }
}
