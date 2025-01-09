//! Property-based testing utilities for store implementations.
//!
//! This module provides utilities for property-based testing of store implementations,
//! including generators for test data and operations, and a model implementation
//! for validating store behavior.
//!
//! # Features
//!
//! - Generate arbitrary key-value pairs
//! - Generate sequences of store operations
//! - Generate range queries
//! - Model-based testing with a reference implementation
//!
//! # Example
//!
//! ```rust,no_run
//! use testing::store::property::{test_kvs_strategy, test_batch_ops_strategy, TestStoreState};
//! use proptest::prelude::*;
//!
//! proptest! {
//!     #[test]
//!     fn test_store_operations(
//!         kvs in test_kvs_strategy(10),
//!         ops in test_batch_ops_strategy(10)
//!     ) {
//!         let mut state = TestStoreState::new();
//!
//!         // Apply operations to both store and model
//!         for kv in kvs {
//!             state.apply_batch_op(TestBatchOp::Set(kv));
//!         }
//!
//!         // Verify state matches expectations
//!         prop_assert!(!state.kvs.is_empty());
//!     }
//! }
//! ```

use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A key-value pair for testing.
///
/// This struct represents a single key-value pair used in property-based tests.
/// Both key and value are arbitrary byte sequences.
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::property::TestKeyValue;
///
/// let kv = TestKeyValue::new(
///     vec![1, 2, 3],
///     vec![4, 5, 6],
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TestKeyValue {
    /// The key as a byte sequence
    pub key: Vec<u8>,
    /// The value as a byte sequence
    pub value: Vec<u8>,
}

impl TestKeyValue {
    /// Creates a new test key-value pair.
    ///
    /// # Arguments
    ///
    /// * `key` - The key as a byte sequence
    /// * `value` - The value as a byte sequence
    pub fn new(key: Vec<u8>, value: Vec<u8>) -> Self {
        Self { key, value }
    }
}

/// Generates a strategy for test key-value pairs.
///
/// The generated key-value pairs have the following properties:
/// - Keys are 1-64 bytes long
/// - Values are 1-1024 bytes long
/// - Both keys and values are arbitrary byte sequences
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::property::test_kv_strategy;
/// use proptest::prelude::*;
///
/// proptest! {
///     #[test]
///     fn test_kv_generation(kv in test_kv_strategy()) {
///         prop_assert!(!kv.key.is_empty());
///         prop_assert!(!kv.value.is_empty());
///     }
/// }
/// ```
pub fn test_kv_strategy() -> impl Strategy<Value = TestKeyValue> {
    (
        prop::collection::vec(any::<u8>(), 1..64),  // key
        prop::collection::vec(any::<u8>(), 1..1024), // value
    ).prop_map(|(key, value)| TestKeyValue::new(key, value))
}

/// Generates a strategy for collections of test key-value pairs.
///
/// # Arguments
///
/// * `max_pairs` - Maximum number of key-value pairs to generate
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::property::test_kvs_strategy;
/// use proptest::prelude::*;
///
/// proptest! {
///     #[test]
///     fn test_kvs_generation(kvs in test_kvs_strategy(10)) {
///         prop_assert!(!kvs.is_empty());
///         prop_assert!(kvs.len() <= 10);
///     }
/// }
/// ```
pub fn test_kvs_strategy(max_pairs: usize) -> impl Strategy<Value = Vec<TestKeyValue>> {
    prop::collection::vec(test_kv_strategy(), 1..max_pairs)
}

/// A batch operation for testing.
///
/// This enum represents operations that can be performed in a batch,
/// either setting or deleting key-value pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestBatchOp {
    /// Set a key-value pair
    Set(TestKeyValue),
    /// Delete a key
    Delete(Vec<u8>),
}

/// Generates a strategy for test batch operations.
///
/// # Arguments
///
/// * `max_ops` - Maximum number of operations to generate
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::property::test_batch_ops_strategy;
/// use proptest::prelude::*;
///
/// proptest! {
///     #[test]
///     fn test_batch_ops_generation(ops in test_batch_ops_strategy(10)) {
///         prop_assert!(!ops.is_empty());
///         prop_assert!(ops.len() <= 10);
///     }
/// }
/// ```
pub fn test_batch_ops_strategy(max_ops: usize) -> impl Strategy<Value = Vec<TestBatchOp>> {
    let leaf = prop_oneof![
        test_kv_strategy().prop_map(TestBatchOp::Set),
        prop::collection::vec(any::<u8>(), 1..64).prop_map(TestBatchOp::Delete),
    ];
    prop::collection::vec(leaf, 1..max_ops)
}

/// A range query for testing.
///
/// This struct represents a range query with start and end keys.
/// The range is inclusive of start and exclusive of end.
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::property::TestRange;
///
/// let range = TestRange::new(
///     vec![1, 2, 3],
///     vec![4, 5, 6],
/// );
/// assert!(range.contains(&vec![1, 2, 3]));
/// assert!(!range.contains(&vec![4, 5, 6]));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRange {
    /// Start key (inclusive)
    pub start: Vec<u8>,
    /// End key (exclusive)
    pub end: Vec<u8>,
}

impl TestRange {
    /// Creates a new test range.
    ///
    /// # Arguments
    ///
    /// * `start` - Start key (inclusive)
    /// * `end` - End key (exclusive)
    pub fn new(start: Vec<u8>, end: Vec<u8>) -> Self {
        Self { start, end }
    }

    /// Checks if a key is within this range.
    ///
    /// The range is inclusive of start and exclusive of end.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to check
    pub fn contains(&self, key: &[u8]) -> bool {
        key >= self.start.as_slice() && key < self.end.as_slice()
    }
}

/// Generates a strategy for test ranges.
///
/// The generated ranges have the following properties:
/// - Start and end keys are 1-64 bytes long
/// - Start key is always less than or equal to end key
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::property::test_range_strategy;
/// use proptest::prelude::*;
///
/// proptest! {
///     #[test]
///     fn test_range_generation(range in test_range_strategy()) {
///         prop_assert!(!range.start.is_empty());
///         prop_assert!(!range.end.is_empty());
///         prop_assert!(range.start <= range.end);
///     }
/// }
/// ```
pub fn test_range_strategy() -> impl Strategy<Value = TestRange> {
    (
        prop::collection::vec(any::<u8>(), 1..64),
        prop::collection::vec(any::<u8>(), 1..64),
    ).prop_map(|(mut start, mut end)| {
        if start > end {
            std::mem::swap(&mut start, &mut end);
        }
        TestRange::new(start, end)
    })
}

/// A model implementation for testing store behavior.
///
/// This struct provides a simple in-memory implementation of a key-value store
/// that can be used to validate the behavior of actual store implementations.
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::property::{TestStoreState, TestBatchOp, TestKeyValue};
///
/// let mut state = TestStoreState::new();
///
/// // Apply operations
/// state.apply_batch_op(TestBatchOp::Set(TestKeyValue::new(
///     vec![1, 2, 3],
///     vec![4, 5, 6],
/// )));
///
/// // Verify state
/// assert_eq!(state.kvs.get(&vec![1, 2, 3]), Some(&vec![4, 5, 6]));
/// ```
#[derive(Debug, Clone, Default)]
pub struct TestStoreState {
    /// The current key-value pairs in the store
    pub kvs: HashMap<Vec<u8>, Vec<u8>>,
}

impl TestStoreState {
    /// Creates a new empty test store state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies a batch operation to the state.
    ///
    /// # Arguments
    ///
    /// * `op` - The operation to apply
    pub fn apply_batch_op(&mut self, op: TestBatchOp) {
        match op {
            TestBatchOp::Set(kv) => {
                self.kvs.insert(kv.key, kv.value);
            }
            TestBatchOp::Delete(key) => {
                self.kvs.remove(&key);
            }
        }
    }

    /// Applies a sequence of batch operations.
    ///
    /// # Arguments
    ///
    /// * `ops` - The operations to apply
    pub fn apply_batch_ops(&mut self, ops: Vec<TestBatchOp>) {
        for op in ops {
            self.apply_batch_op(op);
        }
    }

    /// Gets all key-value pairs within a range.
    ///
    /// # Arguments
    ///
    /// * `range` - The range to query
    pub fn get_range(&self, range: &TestRange) -> Vec<TestKeyValue> {
        self.kvs
            .iter()
            .filter(|(k, _)| range.contains(k))
            .map(|(k, v)| TestKeyValue::new(k.clone(), v.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_kv_generation(kv in test_kv_strategy()) {
            prop_assert!(!kv.key.is_empty());
            prop_assert!(!kv.value.is_empty());
            prop_assert!(kv.key.len() <= 64);
            prop_assert!(kv.value.len() <= 1024);
        }

        #[test]
        fn test_batch_ops_generation(ops in test_batch_ops_strategy(10)) {
            prop_assert!(!ops.is_empty());
            prop_assert!(ops.len() <= 10);
        }

        #[test]
        fn test_range_generation(range in test_range_strategy()) {
            prop_assert!(!range.start.is_empty());
            prop_assert!(!range.end.is_empty());
            prop_assert!(range.start <= range.end);
        }

        #[test]
        fn test_store_state_operations(ops in test_batch_ops_strategy(10)) {
            let mut state = TestStoreState::new();
            state.apply_batch_ops(ops.clone());

            // Verify that all Set operations are reflected in the state
            for op in ops {
                if let TestBatchOp::Set(kv) = op {
                    prop_assert_eq!(state.kvs.get(&kv.key), Some(&kv.value));
                }
            }
        }

        #[test]
        fn test_range_queries(
            kvs in test_kvs_strategy(10),
            range in test_range_strategy()
        ) {
            let mut state = TestStoreState::new();
            for kv in kvs {
                state.apply_batch_op(TestBatchOp::Set(kv));
            }

            let range_kvs = state.get_range(&range);
            for kv in range_kvs {
                prop_assert!(range.contains(&kv.key));
            }
        }
    }
}
