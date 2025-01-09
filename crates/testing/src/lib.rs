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
//! use testing::{TestStore, StoreTestContext};
//!
//! // Implement TestStore for your store type
//! impl TestStore for MyStore {
//!     fn new_test_store() -> Self {
//!         // Create a new test instance
//!         MyStore::new()
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
//!     store.set(b"key", b"value").await.unwrap();
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
    bench::{bench_store, BenchConfig, BenchData},
    edge::{run_edge_cases, EdgeCase, EdgeCaseConfig, EdgeCaseResults},
    property::{test_kv_strategy, test_kvs_strategy, test_batch_ops_strategy, test_range_strategy, TestKeyValue, TestBatchOp, TestRange, TestStoreState},
    stress::{stress_test, StressConfig, StressResults},
    TempStoreDir, TestStore, StoreTestContext,
};

pub use settings::SettingsTestHelper;
pub use encrypt::EncryptTestHelper;

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
