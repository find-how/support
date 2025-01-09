//! Property-based testing utilities for stores.

use proptest::prelude::*;
use std::collections::HashMap;

/// A store operation for property testing
#[derive(Debug, Clone)]
pub enum StoreOperation {
    Set { key: Vec<u8>, value: Vec<u8> },
    Get { key: Vec<u8> },
    Delete { key: Vec<u8> },
    Clear,
    Batch(Vec<StoreOperation>),
    Scan { prefix: Vec<u8> },
}

/// Generate valid store keys
pub fn arb_key() -> impl Strategy<Value = Vec<u8>> {
    // Generate valid UTF-8 keys with reasonable length
    "[a-zA-Z][a-zA-Z0-9_]{0,31}".prop_map(|s| s.into_bytes())
}

/// Generate valid store values
pub fn arb_value() -> impl Strategy<Value = Vec<u8>> {
    // Generate values with reasonable size
    prop::collection::vec(any::<u8>(), 0..1024)
}

/// Generate a sequence of store operations
pub fn arb_operations(max_ops: usize) -> impl Strategy<Value = Vec<StoreOperation>> {
    let leaf = prop_oneof![
        (arb_key(), arb_value()).prop_map(|(k, v)| StoreOperation::Set { key: k, value: v }),
        arb_key().prop_map(|k| StoreOperation::Get { key: k }),
        arb_key().prop_map(|k| StoreOperation::Delete { key: k }),
        Just(StoreOperation::Clear),
        arb_key().prop_map(|k| StoreOperation::Scan { prefix: k }),
    ];

    leaf.prop_recursive(
        8,   // 8 levels deep
        256, // Maximum size
        10,  // Maximum number of items per collection
        |inner| {
            prop::collection::vec(inner.clone(), 0..5)
                .prop_map(|ops| StoreOperation::Batch(ops))
        },
    )
    .prop_collection(0..max_ops)
}

/// A model of the expected store state
#[derive(Debug, Clone, Default)]
pub struct StoreModel {
    data: HashMap<Vec<u8>, Vec<u8>>,
}

impl StoreModel {
    /// Apply an operation to the model
    pub fn apply(&mut self, op: &StoreOperation) {
        match op {
            StoreOperation::Set { key, value } => {
                self.data.insert(key.clone(), value.clone());
            }
            StoreOperation::Delete { key } => {
                self.data.remove(key);
            }
            StoreOperation::Clear => {
                self.data.clear();
            }
            StoreOperation::Batch(ops) => {
                for op in ops {
                    self.apply(op);
                }
            }
            // Get and Scan don't modify state
            StoreOperation::Get { .. } | StoreOperation::Scan { .. } => {}
        }
    }

    /// Get a value from the model
    pub fn get(&self, key: &[u8]) -> Option<&Vec<u8>> {
        self.data.get(key)
    }

    /// Get all entries with a given prefix
    pub fn scan_prefix(&self, prefix: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_key_generation(key in arb_key()) {
            // Keys should be non-empty and start with a letter
            assert!(!key.is_empty());
            assert!(key[0].is_ascii_alphabetic());
        }

        #[test]
        fn test_value_generation(value in arb_value()) {
            // Values should be within size limits
            assert!(value.len() <= 1024);
        }

        #[test]
        fn test_model_consistency(ops in arb_operations(100)) {
            let mut model = StoreModel::default();

            // Apply all operations
            for op in &ops {
                model.apply(op);
            }

            // Verify basic invariants
            match ops.last() {
                Some(StoreOperation::Clear) => assert!(model.data.is_empty()),
                Some(StoreOperation::Set { key, value }) => {
                    assert_eq!(model.get(key), Some(value));
                }
                Some(StoreOperation::Delete { key }) => {
                    assert_eq!(model.get(key), None);
                }
                _ => {}
            }
        }
    }
}
