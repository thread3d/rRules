//! Hybrid typed/dynamic data structures supporting both compile-time types and runtime dynamic capabilities.
//!
//! This module implements the Value enum for representing any rule-relevant value,
//! along with DynamicCast trait for runtime type conversion.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a value in the rule engine, supporting both strongly-typed Rust values
/// and dynamic key-value lookups for business users.
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
}
