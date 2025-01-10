//! Testing utilities for the project.
//!
//! This crate provides comprehensive testing utilities for store implementations,
//! including property-based testing, stress testing, edge case testing, and benchmarking.
//!
//! # Features
//!
//! - **Property Testing**: Generate and validate arbitrary store operations
//! - **Stress Testing**: Test concurrent operations under load
//! - **Edge Case Testing**: Validate behavior with boundary conditions
//! - **Benchmarking**: Measure performance of store operations
//!
//! # Example
//!
//! ```rust,no_run
//! use testing::store::{TestStore, StoreTestContext};
//! use bytes::Bytes;
//! use std::collections::HashMap;
//! use std::sync::{Arc, Mutex};
//! use async_trait::async_trait;
//!
//! #[derive(Clone)]
//! struct MyStore {
//!     data: Arc<Mutex<HashMap<Vec<u8>, Bytes>>>,
//! }
//!
//! #[async_trait]
//! impl TestStore for MyStore {
//!     fn new_test_store() -> Self {
//!         Self {
//!             data: Arc::new(Mutex::new(HashMap::new())),
//!         }
//!     }
//!
//!     async fn get(&self, key: &[u8]) -> Result<Option<Bytes>, Box<dyn std::error::Error>> {
//!         Ok(self.data.lock().unwrap().get(key).cloned())
//!     }
//!
//!     async fn set(&self, key: &[u8], value: Bytes) -> Result<(), Box<dyn std::error::Error>> {
//!         self.data.lock().unwrap().insert(key.to_vec(), value);
//!         Ok(())
//!     }
//!
//!     async fn delete(&self, key: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
//!         self.data.lock().unwrap().remove(key);
//!         Ok(())
//!     }
//!
//!     async fn batch_set(&self, kvs: Vec<(Vec<u8>, Bytes)>) -> Result<(), Box<dyn std::error::Error>> {
//!         let mut data = self.data.lock().unwrap();
//!         for (key, value) in kvs {
//!             data.insert(key, value);
//!         }
//!         Ok(())
//!     }
//!
//!     async fn batch_delete(&self, keys: Vec<Vec<u8>>) -> Result<(), Box<dyn std::error::Error>> {
//!         let mut data = self.data.lock().unwrap();
//!         for key in keys {
//!             data.remove(&key);
//!         }
//!         Ok(())
//!     }
//!
//!     async fn range(&self, range: std::ops::Range<&[u8]>) -> Result<Vec<(Vec<u8>, Bytes)>, Box<dyn std::error::Error>> {
//!         let data = self.data.lock().unwrap();
//!         let mut result = Vec::new();
//!         for (key, value) in data.iter() {
//!             if key.as_slice() >= range.start && key.as_slice() < range.end {
//!                 result.push((key.clone(), value.clone()));
//!             }
//!         }
//!         Ok(result)
//!     }
//! }
//!
//! // Use in tests
//! #[tokio::test]
//! async fn test_store_operations() {
//!     let context = StoreTestContext::<MyStore>::new();
//!     let store = context.store;
//!
//!     // Test store operations
//!     store.set(b"key", b"value".into()).await.unwrap();
//!     let value = store.get(b"key").await.unwrap();
//!     assert_eq!(value.as_deref(), Some(b"value".as_ref()));
//! }
//! ```
//!
//! # Modules
//!
//! - [`store`]: Core store testing utilities and traits
//!   - [`store::bench`]: Benchmarking utilities
//!   - [`store::edge`]: Edge case testing
//!   - [`store::property`]: Property-based testing
//!   - [`store::stress`]: Stress testing
//!
//! # Re-exports
//!
//! For convenience, commonly used types and functions are re-exported at the crate root:
//!
//! - Benchmarking: [`bench_store`], [`BenchConfig`], [`BenchData`]
//! - Edge Cases: [`run_edge_cases`], [`EdgeCase`], [`EdgeCaseConfig`], [`EdgeCaseResults`]
//! - Property Testing: [`test_kv_strategy`], [`test_kvs_strategy`], [`test_batch_ops_strategy`],
//!   [`test_range_strategy`], [`TestKeyValue`], [`TestBatchOp`], [`TestRange`], [`TestStoreState`]
//! - Stress Testing: [`stress_test`], [`StressConfig`], [`StressResults`]
//! - Core Types: [`TempStoreDir`], [`TestStore`], [`StoreTestContext`]

pub mod store;
pub mod settings;
pub mod encrypt;

pub use store::{
    bench::{bench_store, BenchConfig},
    property::{test_kv_strategy, test_kvs_strategy, test_batch_ops_strategy, test_range_strategy},
    stress::stress_test,
    TestStore,
    StoreTestContext,
};

pub use settings::setup_test_settings;
pub use encrypt::setup_test_encryption;

/// A test runtime for async tests
pub struct TestRuntime;

impl TestRuntime {
    /// Creates a new test runtime
    pub fn new() -> Self {
        Self
    }

    /// Runs a future to completion
    pub async fn run<F: std::future::Future>(&self, future: F) -> F::Output {
        future.await
    }
}
