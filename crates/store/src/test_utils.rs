//! Test utilities for store implementations.
//!
//! This module provides testing utilities for store implementations.
//! It is only available when the "test-utils" feature is enabled.

use std::fmt::Debug;
use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use testing::{TestRuntime, store::TestStore as BaseTestStore};

use crate::{Store, TypedStore, RangeStore, BatchStore, Result, Error};

/// Test data structure for typed store tests
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestData {
    pub field1: String,
    pub field2: i32,
    pub field3: Vec<u8>,
}

impl TestData {
    /// Create test data with default values
    pub fn new(id: i32) -> Self {
        Self {
            field1: format!("test_{}", id),
            field2: id,
            field3: vec![id as u8; 3],
        }
    }
}

/// Trait for creating test store instances
#[async_trait]
pub trait TestStore: Store + TypedStore + RangeStore + BatchStore + BaseTestStore<Error = Error> {
    /// Create a new test store instance with default configuration
    async fn new_test_store() -> Result<Self>
    where
        Self: Sized,
    {
        let dir = testing::store::TempStoreDir::new();
        Self::open(dir.path).await
    }
}

/// Run a comprehensive test suite against a store implementation
pub async fn test_store_implementation<S: TestStore + Clone>(store: S) -> Result<()> {
    test_basic_operations(&store).await?;
    test_typed_operations(&store).await?;
    test_range_operations(&store).await?;
    test_batch_operations(&store).await?;
    Ok(())
}

/// Test basic key-value operations
pub async fn test_basic_operations<S: Store>(store: &S) -> Result<()> {
    // Set and get
    store.set(b"key1", b"value1").await?;
    assert_eq!(store.get(b"key1").await?.unwrap(), Bytes::from("value1"));

    // Contains
    assert!(store.contains(b"key1").await?);
    assert!(!store.contains(b"nonexistent").await?);

    // Delete
    store.delete(b"key1").await?;
    assert!(!store.contains(b"key1").await?);

    // Clear
    store.set(b"key2", b"value2").await?;
    store.clear().await?;
    assert!(!store.contains(b"key2").await?);

    Ok(())
}

/// Test typed store operations
pub async fn test_typed_operations<S: TypedStore>(store: &S) -> Result<()> {
    let data = TestData::new(1);

    // Set and get typed
    store.set_typed("typed_key1", &data).await?;
    let retrieved: TestData = store.get_typed("typed_key1").await?.unwrap();
    assert_eq!(data, retrieved);

    // Nonexistent key
    let none: Option<TestData> = store.get_typed("nonexistent").await?;
    assert!(none.is_none());

    Ok(())
}

/// Test range and prefix operations
pub async fn test_range_operations<S: RangeStore>(store: &S) -> Result<()> {
    // Setup test data
    store.set(b"a:1", b"value1").await?;
    store.set(b"a:2", b"value2").await?;
    store.set(b"b:1", b"value3").await?;

    // Test prefix scan
    let mut entries: Vec<_> = store
        .scan_prefix(b"a:")
        .await?
        .collect::<Result<Vec<_>>>()?;
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));

    assert_eq!(entries.len(), 2);
    assert_eq!(&entries[0].1[..], b"value1");
    assert_eq!(&entries[1].1[..], b"value2");

    Ok(())
}

/// Test batch operations
pub async fn test_batch_operations<S: BatchStore>(store: &S) -> Result<()> {
    let mut batch = store.batch();
    batch.set(b"batch1", b"value1")?;
    batch.set(b"batch2", b"value2")?;
    store.execute_batch(batch).await?;

    assert_eq!(
        store.get(b"batch1").await?.unwrap(),
        Bytes::from("value1")
    );
    assert_eq!(
        store.get(b"batch2").await?.unwrap(),
        Bytes::from("value2")
    );

    Ok(())
}

/// Property-based test generator for store operations
#[cfg(test)]
pub mod proptest_helpers {
    use super::*;
    use proptest::prelude::*;

    prop_compose! {
        pub fn arb_test_data()(
            id in 0..100i32,
        ) -> TestData {
            TestData::new(id)
        }
    }

    prop_compose! {
        pub fn arb_key()(
            s in "[a-z][a-z0-9_]{0,9}"
        ) -> Vec<u8> {
            s.into_bytes()
        }
    }

    prop_compose! {
        pub fn arb_value()(
            s in "[a-z0-9_]{0,20}"
        ) -> Vec<u8> {
            s.into_bytes()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use crate::test_utils::proptest_helpers::*;

    proptest! {
        #[test]
        fn test_data_serialization(data in arb_test_data()) {
            let serialized = serde_json::to_string(&data).unwrap();
            let deserialized: TestData = serde_json::from_str(&serialized).unwrap();
            assert_eq!(data, deserialized);
        }

        #[test]
        fn test_key_value_format(
            key in arb_key(),
            value in arb_value()
        ) {
            assert!(!key.is_empty());
            assert!(key.len() <= 10);
            assert!(value.len() <= 20);
        }
    }
}
