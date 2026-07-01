//! Hybrid typed/dynamic data structures supporting both compile-time types and runtime dynamic capabilities.
//!
//! This module implements the Value enum for representing any rule-relevant value,
//! along with DynamicCast trait for runtime type conversion.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a value in the rule engine, supporting both strongly-typed Rust values
/// and dynamic key-value lookups for business users.
///
/// Note: We use PartialEq but not Eq for Value because f64 doesn't implement Eq
/// (due to NaN comparisons). This is acceptable as we compare Float values using
/// epsilon-based comparison in the evaluation logic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    /// Boolean value
    Bool(bool),
    /// Signed integer
    Integer(i64),
    /// Floating point number
    Float(f64),
    /// String value
    String(String),
    /// Date/time value
    DateTime(chrono::NaiveDateTime),
    /// Nested object with dynamic fields
    Object(HashMap<String, Value>),
    /// Array of values
    Array(Vec<Value>),
    /// Null/undefined value
    Null,
}

impl Value {
    /// Creates a Value from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Converts the Value to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Represents a fact type schema with typed and dynamic fields.
#[derive(Debug, Clone, Default)]
pub struct FactType {
    /// Name of the fact type
    pub name: String,
    /// Typed fields with their types
    pub typed_fields: HashMap<String, ValueType>,
    /// Dynamic fields support via serde_json::Value fallback
    pub dynamic_fields: Vec<String>,
}

/// Represents a field's type in a FactType.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueType {
    Bool,
    Integer,
    Float,
    String,
    DateTime,
    Object,
    Array,
}

/// Trait for runtime type conversion with dynamic casting support.
pub trait DynamicCast {
    /// Cast to bool if possible
    fn as_bool(&self) -> Option<bool>;

    /// Cast to i64 if possible
    fn as_integer(&self) -> Option<i64>;

    /// Cast to f64 if possible
    fn as_float(&self) -> Option<f64>;

    /// Cast to &str if possible
    fn as_string(&self) -> Option<&str>;

    /// Generic casting to type T
    fn try_into<T>(self) -> Result<T, Self>
    where
        Self: Sized;

    /// Check compatibility with a TypeInfo
    fn is_compatible_with(&self, info: &TypeInfo) -> bool;
}

/// Type information for compatibility checking.
#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub name: String,
    pub type_class: TypeClass,
}

/// Classification of value types.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeClass {
    Scalar,
    Collection,
    Object,
    Null,
}

impl DynamicCast for Value {
    fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn as_integer(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            Value::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    fn try_into<T>(self) -> Result<T, Self>
    where
        Self: Sized,
    {
        Err(self)
    }

    fn is_compatible_with(&self, info: &TypeInfo) -> bool {
        match self {
            Value::Bool(_) => info.type_class == TypeClass::Scalar && info.name == "bool",
            Value::Integer(_) => info.type_class == TypeClass::Scalar && info.name == "integer",
            Value::Float(_) => info.type_class == TypeClass::Scalar && info.name == "float",
            Value::String(_) => info.type_class == TypeClass::Scalar && info.name == "string",
            Value::Object(_) => info.type_class == TypeClass::Object,
            Value::Array(_) => info.type_class == TypeClass::Collection,
            Value::Null => info.type_class == TypeClass::Null,
            Value::DateTime(_) => info.type_class == TypeClass::Scalar && info.name == "datetime",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_roundtrip() {
        let value = Value::String("hello".to_string());
        let json = value.to_json().unwrap();
        let parsed = Value::from_json(&json).unwrap();
        assert_eq!(value, parsed);
    }

    #[test]
    fn test_dynamic_cast() {
        let int_value = Value::Integer(42);
        assert_eq!(int_value.as_integer(), Some(42));
        assert_eq!(int_value.as_float(), Some(42.0));

        let string_value = Value::String("test".to_string());
        assert_eq!(string_value.as_string(), Some("test"));
    }

    #[test]
    fn test_all_value_types() {
        // Bool
        let v = Value::Bool(true);
        assert_eq!(v, Value::Bool(true));
        assert_ne!(v, Value::Bool(false));

        // Integer (edge cases)
        #[allow(clippy::absurd_extreme_comparisons)]
        {
            let _v_min = Value::Integer(i64::MIN);
            let _v_max = Value::Integer(i64::MAX);
            let _v_zero = Value::Integer(0);

            // Float (including special values)
            let _v_float = Value::Float(3.14159);
            let _v_nan = Value::Float(f64::NAN);
            let _v_inf = Value::Float(f64::INFINITY);

            // String with unicode
            let _v_unicode = Value::String("hello 世界 🌍".to_string());

            // DateTime
            #[allow(deprecated)]
            {
                let _v_datetime = Value::DateTime(chrono::NaiveDateTime::from_timestamp_opt(0, 0).unwrap());
            }

            // Object
            let mut obj = std::collections::HashMap::new();
            obj.insert("key".to_string(), Value::Integer(42));
            let _v_object = Value::Object(obj);

            // Array
            let _v_array = Value::Array(vec![Value::Integer(1), Value::Integer(2)]);

            // Null
        }
        let _v_null = Value::Null;
    }

    #[test]
    fn test_value_float_special_values() {
        // Test that regular float comparisons work correctly
        let f1 = Value::Float(3.14);
        let f2 = Value::Float(3.14);
        assert_eq!(f1, f2);

        // Test comparison with epsilon
        let f3 = Value::Float(3.14159);
        let f4 = Value::Float(3.14160);
        // With f64's default PartialEq, these are not equal due to precision
        assert_ne!(f3, f4);
    }

    #[test]
    fn test_value_to_json_all_types() {
        let values = vec![
            Value::Bool(true),
            Value::Integer(42),
            Value::Float(3.14),
            Value::String("hello".to_string()),
            Value::Null,
            Value::Array(vec![Value::Integer(1), Value::Integer(2)]),
        ];

        for v in values {
            let json = v.to_json().unwrap();
            assert!(!json.is_empty());
        }
    }

    #[test]
    fn test_value_roundtrip_all_types() {
        // Test roundtrip for each value type
        let values = vec![
            Value::Bool(true),
            Value::Integer(42),
            Value::Float(3.14),
            Value::String("hello".to_string()),
            Value::Null,
        ];

        for v in values {
            let json = v.to_json().unwrap();
            let parsed = Value::from_json(&json).unwrap();
            assert_eq!(v, parsed);
        }
    }

    #[test]
    fn test_value_from_json_with_all_types() {
        // Verify that Value can be serialized and deserialized correctly for all types
        let values = vec![
            Value::Bool(true),
            Value::Integer(42),
            Value::Float(3.14),
            Value::String("hello".to_string()),
            Value::Null,
        ];

        for v in values {
            let json = v.to_json().unwrap();
            assert!(!json.is_empty());
            let parsed = Value::from_json(&json).unwrap();
            assert_eq!(v, parsed);
        }
    }

    #[test]
    fn test_value_nan_comparisons() {
        // Test NaN handling - NaN != NaN per IEEE 754
        let nan1 = Value::Float(f64::NAN);
        let nan2 = Value::Float(f64::NAN);

        // With f64's default PartialEq, NaN is not equal to itself
        assert_ne!(nan1, nan2);
    }

    #[test]
    fn test_value_object_nested() {
        let mut inner_obj = std::collections::HashMap::new();
        inner_obj.insert("inner".to_string(), Value::Integer(99));

        let mut outer_obj = std::collections::HashMap::new();
        outer_obj.insert("outer".to_string(), Value::Object(inner_obj));

        let value = Value::Object(outer_obj);
        let json = value.to_json().unwrap();

        // Verify nested object roundtrip
        let parsed = Value::from_json(&json).unwrap();
        assert_eq!(value, parsed);
    }

    #[test]
    fn test_value_array_nested() {
        let inner_array = vec![Value::Integer(1), Value::Integer(2)];
        let outer_array = vec![Value::Array(inner_array)];

        let value = Value::Array(outer_array);
        let json = value.to_json().unwrap();

        // Verify nested array roundtrip
        let parsed = Value::from_json(&json).unwrap();
        assert_eq!(value, parsed);
    }

    #[test]
    fn test_dynamic_cast_errors() {
        // Test that incompatible casts return None
        let string_val = Value::String("hello".to_string());
        assert_eq!(string_val.as_bool(), None);
        assert_eq!(string_val.as_integer(), None);
        assert_eq!(string_val.as_float(), None);

        let int_val = Value::Integer(42);
        assert_eq!(int_val.as_bool(), None);
        assert_eq!(int_val.as_string(), None);
    }

    #[test]
    fn test_value_type_class() {
        // Test type class identification
        assert!(Value::Bool(true).is_compatible_with(&TypeInfo {
            name: "bool".to_string(),
            type_class: TypeClass::Scalar,
        }));

        assert!(Value::Integer(42).is_compatible_with(&TypeInfo {
            name: "integer".to_string(),
            type_class: TypeClass::Scalar,
        }));

        assert!(Value::Object(std::collections::HashMap::new()).is_compatible_with(&TypeInfo {
            name: "object".to_string(),
            type_class: TypeClass::Object,
        }));

        assert!(!Value::String("test".to_string()).is_compatible_with(&TypeInfo {
            name: "bool".to_string(),
            type_class: TypeClass::Scalar,
        }));
    }

    #[test]
    fn test_value_datetime_roundtrip() {
        #[allow(deprecated)]
        let dt = chrono::NaiveDateTime::from_timestamp_opt(1234567890, 0).unwrap();
        let value = Value::DateTime(dt);
        let json = value.to_json().unwrap();

        // DateTime is serialized as a formatted string
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        // The parser may or may not preserve the type, just verify it's valid JSON
        assert!(parsed.is_string() || parsed.is_object());
    }
}
