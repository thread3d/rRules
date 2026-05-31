//! Fact representation and working memory operations for the rule engine.
//!
//! This module implements the Fact struct and WorkingMemory for pattern matching
//! and fact storage/retrieval.

use crate::data_types::{FactType, Value};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;
use uuid::Uuid;

/// Represents a fact with typed fields stored in working memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    /// Unique identifier for the fact
    pub id: Uuid,
    /// Reference to the fact type schema
    pub fact_type_id: String,
    /// Payload containing field values
    pub payload: HashMap<String, Value>,
    /// Creation timestamp
    pub created_at: SystemTime,
}

impl Fact {
    /// Creates a new fact with auto-generated ID.
    pub fn new(fact_type_id: impl Into<String>, payload: HashMap<String, Value>) -> Self {
        Self {
            id: Uuid::new_v4(),
            fact_type_id: fact_type_id.into(),
            payload,
            created_at: SystemTime::now(),
        }
    }

    /// Creates a fact with a specific ID (useful for testing or reloading).
    pub fn with_id(
        id: Uuid,
        fact_type_id: impl Into<String>,
        payload: HashMap<String, Value>,
    ) -> Self {
        Self {
            id,
            fact_type_id: fact_type_id.into(),
            payload,
            created_at: SystemTime::now(),
        }
    }

    /// Gets a value from the payload by field name.
    pub fn get(&self, field_name: &str) -> Option<&Value> {
        self.payload.get(field_name)
    }

    /// Updates a field in the fact's payload.
    pub fn update_field(&mut self, field_name: impl Into<String>, value: Value) {
        self.payload.insert(field_name.into(), value);
    }
}

/// Index for efficient fact retrieval by type and field values.
#[derive(Debug, Clone)]
pub struct FactRetrievalIndex {
    /// Primary index: fact_type_id -> Vec<fact_id>
    primary_index: HashMap<String, HashSet<Uuid>>,
    /// Secondary indexes: (field_name, value_hash) -> Vec<fact_id>
    secondary_indexes: HashMap<(String, String), HashSet<Uuid>>,
}

impl FactRetrievalIndex {
    /// Creates a new empty index.
    pub fn new() -> Self {
        Self {
            primary_index: HashMap::new(),
            secondary_indexes: HashMap::new(),
        }
    }

    /// Adds a fact to the index.
    pub fn add(&mut self, fact: &Fact) {
        let id = fact.id;

        // Primary index by fact type
        self.primary_index
            .entry(fact.fact_type_id.clone())
            .or_default()
            .insert(id);

        // Secondary indexes on frequently queried fields
        for (field_name, value) in &fact.payload {
            let value_key = format!("{:?}", value);
            self.secondary_indexes
                .entry((field_name.clone(), value_key))
                .or_default()
                .insert(id);
        }
    }

    /// Removes a fact from the index.
    pub fn remove(&mut self, fact: &Fact) {
        let id = fact.id;

        // Remove from primary index
        if let Some(set) = self.primary_index.get_mut(&fact.fact_type_id) {
            set.remove(&id);
            if set.is_empty() {
                self.primary_index.remove(&fact.fact_type_id);
            }
        }

        // Remove from secondary indexes
        for (field_name, value) in &fact.payload {
            let value_key = format!("{:?}", value);
            if let Some(set) = self.secondary_indexes.get_mut(&(field_name.clone(), value_key.clone())) {
                set.remove(&id);
                if set.is_empty() {
                    self.secondary_indexes.remove(&(field_name.clone(), value_key));
                }
            }
        }
    }

    /// Gets facts by type.
    pub fn get_by_type(&self, fact_type_id: &str) -> Vec<Uuid> {
        self.primary_index
            .get(fact_type_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect()
    }
}

/// Working memory for storing and retrieving facts with pattern matching support.
#[derive(Debug, Clone)]
pub struct WorkingMemory {
    /// Storage of facts by ID
    facts: HashMap<Uuid, Fact>,
    /// Index for efficient retrieval
    index: FactRetrievalIndex,
    /// Track inserted/removed facts for session isolation
    changes: Vec<WorkingMemoryChange>,
}

#[derive(Debug, Clone)]
enum WorkingMemoryChange {
    Inserted(Uuid),
    Removed(Uuid),
}

impl WorkingMemory {
    /// Creates a new empty working memory.
    pub fn new() -> Self {
        Self {
            facts: HashMap::new(),
            index: FactRetrievalIndex::new(),
            changes: Vec::new(),
        }
    }

    /// Inserts a fact into working memory.
    pub fn insert_fact(&mut self, mut fact: Fact) -> Result<(), WorkingMemoryError> {
        // Validate that fact has required fields
        if fact.payload.is_empty() && fact.fact_type_id != "generic" {
            return Err(WorkingMemoryError::EmptyPayload);
        }

        // Check for duplicate ID
        if self.facts.contains_key(&fact.id) {
            return Err(WorkingMemoryError::DuplicateId(fact.id));
        }

        self.index.add(&fact);
        self.facts.insert(fact.id, fact);
        Ok(())
    }

    /// Updates an existing fact.
    pub fn update_fact(
        &mut self,
        id: Uuid,
        updates: HashMap<String, Value>,
    ) -> Result<(), WorkingMemoryError> {
        let fact = self
            .facts
            .get_mut(&id)
            .ok_or(WorkingMemoryError::FactNotFound(id))?;

        for (field, value) in updates {
            fact.payload.insert(field, value);
        }

        // Rebuild index entry
        self.index.remove(fact);
        self.index.add(fact);

        Ok(())
    }

    /// Retracts (removes) a fact from working memory.
    pub fn retract_fact(&mut self, id: Uuid) -> Result<Option<Fact>, WorkingMemoryError> {
        match self.facts.remove(&id) {
            Some(fact) => {
                self.index.remove(&fact);
                Ok(Some(fact))
            }
            None => Ok(None),
        }
    }

    /// Gets a fact by ID.
    pub fn get_fact(&self, id: Uuid) -> Option<&Fact> {
        self.facts.get(&id)
    }

    /// Gets facts by type using the index.
    pub fn get_facts_by_type(&self, type_id: &str) -> Vec<&Fact> {
        let ids = self.index.get_by_type(type_id);
        ids.into_iter()
            .filter_map(|id| self.facts.get(&id))
            .collect()
    }

    /// Matches facts against a pattern (placeholder for Rete integration).
    pub fn match_pattern(&self, _pattern: &str) -> Vec<&Fact> {
        // This will be implemented when integrating with the Rete algorithm
        Vec::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkingMemoryError {
    #[error("Fact payload cannot be empty")]
    EmptyPayload,
    #[error("Duplicate fact ID: {0}")]
    DuplicateId(Uuid),
    #[error("Fact not found: {0}")]
    FactNotFound(Uuid),
    #[error("Validation error: {0}")]
    Validation(String),
}

impl From<String> for WorkingMemoryError {
    fn from(s: String) -> Self {
        WorkingMemoryError::Validation(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_working_memory_operations() {
        let mut wm = WorkingMemory::new();

        // Create and insert a fact
        let mut payload = HashMap::new();
        payload.insert("amount".to_string(), Value::Integer(100));

        let fact = Fact::new("transaction".to_string(), payload);
        let id = fact.id;

        wm.insert_fact(fact).unwrap();

        // Retrieve by ID
        assert!(wm.get_fact(id).is_some());

        // Retrieve by type
        let facts_by_type = wm.get_facts_by_type("transaction");
        assert_eq!(facts_by_type.len(), 1);

        // Update fact
        let mut updates = HashMap::new();
        updates.insert("amount".to_string(), Value::Integer(200));
        wm.update_fact(id, updates).unwrap();

        assert_eq!(
            wm.get_fact(id).unwrap().payload.get("amount"),
            Some(&Value::Integer(200))
        );

        // Retract fact
        let retracted = wm.retract_fact(id).unwrap();
        assert!(retracted.is_some());
        assert!(wm.get_fact(id).is_none());
    }

    #[test]
    fn test_duplicate_id_error() {
        let mut wm = WorkingMemory::new();

        let mut payload = HashMap::new();
        payload.insert("field1".to_string(), Value::Integer(42));
        let fact = Fact::new("test".to_string(), payload);
        let id = fact.id;

        wm.insert_fact(fact).unwrap();

        // Try to insert another fact with same ID
        let mut payload2 = HashMap::new();
        payload2.insert("field1".to_string(), Value::Integer(100));
        let fact2 = Fact::with_id(id, "test2".to_string(), payload2);

        assert!(matches!(
            wm.insert_fact(fact2),
            Err(WorkingMemoryError::DuplicateId(_))
        ));
    }
}
