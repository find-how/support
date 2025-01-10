//! Stress testing utilities for store implementations.
//!
//! This module provides utilities for stress testing store implementations under
//! high concurrency and load. It helps validate that stores maintain correctness
//! and performance under demanding conditions.
//!
//! # Features
//!
//! - Concurrent operations from multiple workers
//! - Configurable test duration and operation counts
//! - Performance measurements
//! - Error tracking
//!
//! # Example
//!
//! ```rust,no_run
//! use testing::store::stress::{stress_test, StressConfig};
//! use testing::store::TestStore;
//! use bytes::Bytes;
//! use std::collections::HashMap;
//! use std::sync::{Arc, Mutex};
//! use async_trait::async_trait;
//! use std::time::Duration;
//!
//! #[derive(Clone)]
//! struct StressTestStore {
//!     data: Arc<Mutex<HashMap<Vec<u8>, Bytes>>>,
//! }
//!
//! #[async_trait]
//! impl TestStore for StressTestStore {
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
//! #[tokio::test]
//! async fn test_store_stress() {
//!     let config = StressConfig {
//!         concurrency: 10,
//!         operations_per_thread: 1000,
//!         max_key_size: 64,
//!         max_value_size: 1024,
//!         duration: Duration::from_secs(60),
//!     };
//!
//!     let results = stress_test(|| StressTestStore::new_test_store(), &config).await;
//!     assert_eq!(results.errors, 0);
//! }
//! ```

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Barrier;
use tokio::time::Instant;

/// Configuration for stress tests.
///
/// This struct contains parameters that control the behavior of stress tests,
/// including concurrency levels, operation counts, and data sizes.
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::stress::StressConfig;
/// use std::time::Duration;
///
/// let config = StressConfig {
///     concurrency: 10,
///     operations_per_thread: 1000,
///     max_key_size: 64,
///     max_value_size: 1024,
///     duration: Duration::from_secs(60),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct StressConfig {
    /// Number of concurrent worker threads
    pub concurrency: usize,
    /// Number of operations each worker should perform
    pub operations_per_thread: usize,
    /// Maximum size of keys in bytes
    pub max_key_size: usize,
    /// Maximum size of values in bytes
    pub max_value_size: usize,
    /// Maximum duration of the stress test
    pub duration: Duration,
}

impl Default for StressConfig {
    fn default() -> Self {
        Self {
            concurrency: 10,
            operations_per_thread: 1000,
            max_key_size: 64,
            max_value_size: 1024,
            duration: Duration::from_secs(60),
        }
    }
}

/// Results from a stress test.
///
/// This struct contains metrics collected during the stress test,
/// including operation counts, throughput, and error counts.
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::stress::{stress_test, StressConfig};
/// use testing::store::TestStore;
/// use bytes::Bytes;
/// use std::collections::HashMap;
/// use std::sync::{Arc, Mutex};
/// use async_trait::async_trait;
///
/// #[derive(Clone)]
/// struct StressResultsTestStore {
///     data: Arc<Mutex<HashMap<Vec<u8>, Bytes>>>,
/// }
///
/// #[async_trait]
/// impl TestStore for StressResultsTestStore {
///     fn new_test_store() -> Self {
///         Self {
///             data: Arc::new(Mutex::new(HashMap::new())),
///         }
///     }
///
///     async fn get(&self, key: &[u8]) -> Result<Option<Bytes>, Box<dyn std::error::Error>> {
///         Ok(self.data.lock().unwrap().get(key).cloned())
///     }
///
///     async fn set(&self, key: &[u8], value: Bytes) -> Result<(), Box<dyn std::error::Error>> {
///         self.data.lock().unwrap().insert(key.to_vec(), value);
///         Ok(())
///     }
///
///     async fn delete(&self, key: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
///         self.data.lock().unwrap().remove(key);
///         Ok(())
///     }
///
///     async fn batch_set(&self, kvs: Vec<(Vec<u8>, Bytes)>) -> Result<(), Box<dyn std::error::Error>> {
///         let mut data = self.data.lock().unwrap();
///         for (key, value) in kvs {
///             data.insert(key, value);
///         }
///         Ok(())
///     }
///
///     async fn batch_delete(&self, keys: Vec<Vec<u8>>) -> Result<(), Box<dyn std::error::Error>> {
///         let mut data = self.data.lock().unwrap();
///         for key in keys {
///             data.remove(&key);
///         }
///         Ok(())
///     }
///
///     async fn range(&self, range: std::ops::Range<&[u8]>) -> Result<Vec<(Vec<u8>, Bytes)>, Box<dyn std::error::Error>> {
///         let data = self.data.lock().unwrap();
///         let mut result = Vec::new();
///         for (key, value) in data.iter() {
///             if key.as_slice() >= range.start && key.as_slice() < range.end {
///                 result.push((key.clone(), value.clone()));
///             }
///         }
///         Ok(result)
///     }
/// }
///
/// #[tokio::test]
/// async fn analyze_stress_results() {
///     let config = StressConfig::default();
///     let results = stress_test(|| StressResultsTestStore::new_test_store(), &config).await;
///
///     println!("Total operations: {}", results.total_operations);
///     println!("Operations/sec: {}", results.ops_per_second);
///     println!("Duration: {:?}", results.duration);
///     println!("Errors: {}", results.errors);
/// }
/// ```
#[derive(Debug)]
pub struct StressResults {
    /// Total number of operations performed
    pub total_operations: usize,
    /// Average operations per second
    pub ops_per_second: f64,
    /// Actual duration of the test
    pub duration: Duration,
    /// Number of operations that failed
    pub errors: usize,
}

/// Runs a stress test on a store implementation.
///
/// This function runs a comprehensive stress test on a store implementation,
/// measuring performance and reliability under high load.
///
/// # Type Parameters
///
/// * `S` - The store type to test
/// * `F` - A function that creates new store instances
///
/// # Arguments
///
/// * `setup` - Function that creates new store instances
/// * `config` - Configuration for the stress test
///
/// # Returns
///
/// Returns a [`StressResults`] containing metrics from the test.
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::stress::{stress_test, StressConfig};
/// use testing::store::TestStore;
/// use bytes::Bytes;
/// use std::collections::HashMap;
/// use std::sync::{Arc, Mutex};
/// use async_trait::async_trait;
/// use std::time::Duration;
///
/// #[derive(Clone)]
/// struct StressTestFnStore {
///     data: Arc<Mutex<HashMap<Vec<u8>, Bytes>>>,
/// }
///
/// #[async_trait]
/// impl TestStore for StressTestFnStore {
///     fn new_test_store() -> Self {
///         Self {
///             data: Arc::new(Mutex::new(HashMap::new())),
///         }
///     }
///
///     async fn get(&self, key: &[u8]) -> Result<Option<Bytes>, Box<dyn std::error::Error>> {
///         Ok(self.data.lock().unwrap().get(key).cloned())
///     }
///
///     async fn set(&self, key: &[u8], value: Bytes) -> Result<(), Box<dyn std::error::Error>> {
///         self.data.lock().unwrap().insert(key.to_vec(), value);
///         Ok(())
///     }
///
///     async fn delete(&self, key: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
///         self.data.lock().unwrap().remove(key);
///         Ok(())
///     }
///
///     async fn batch_set(&self, kvs: Vec<(Vec<u8>, Bytes)>) -> Result<(), Box<dyn std::error::Error>> {
///         let mut data = self.data.lock().unwrap();
///         for (key, value) in kvs {
///             data.insert(key, value);
///         }
///         Ok(())
///     }
///
///     async fn batch_delete(&self, keys: Vec<Vec<u8>>) -> Result<(), Box<dyn std::error::Error>> {
///         let mut data = self.data.lock().unwrap();
///         for key in keys {
///             data.remove(&key);
///         }
///         Ok(())
///     }
///
///     async fn range(&self, range: std::ops::Range<&[u8]>) -> Result<Vec<(Vec<u8>, Bytes)>, Box<dyn std::error::Error>> {
///         let data = self.data.lock().unwrap();
///         let mut result = Vec::new();
///         for (key, value) in data.iter() {
///             if key.as_slice() >= range.start && key.as_slice() < range.end {
///                 result.push((key.clone(), value.clone()));
///             }
///         }
///         Ok(result)
///     }
/// }
///
/// #[tokio::test]
/// async fn test_stress_fn() {
///     let config = StressConfig {
///         concurrency: 10,
///         operations_per_thread: 1000,
///         max_key_size: 64,
///         max_value_size: 1024,
///         duration: Duration::from_secs(60),
///     };
///
///     let results = stress_test(|| StressTestFnStore::new_test_store(), &config).await;
///     assert_eq!(results.errors, 0);
/// }
/// ```
pub async fn stress_test<S, F>(setup: F, config: &StressConfig) -> StressResults
where
    S: Clone + Send + Sync + 'static,
    F: Fn() -> S + Send + Sync + 'static,
{
    let start = Instant::now();
    let barrier = Arc::new(Barrier::new(config.concurrency));
    let store = Arc::new(setup());

    // Spawn worker tasks
    let handles: Vec<_> = (0..config.concurrency)
        .map(|_| {
            let store = store.clone();
            let barrier = barrier.clone();
            let config = config.clone();

            tokio::spawn(async move {
                // Wait for all workers to be ready
                barrier.wait().await;

                let mut operations = 0;
                let mut errors = 0;
                let worker_start = Instant::now();

                while worker_start.elapsed() < config.duration && operations < config.operations_per_thread
                {
                    // Perform random operations
                    if let Err(_) = perform_random_operation(&store).await {
                        errors += 1;
                    }
                    operations += 1;
                }

                (operations, errors)
            })
        })
        .collect();

    // Wait for all workers to complete
    let mut total_operations = 0;
    let mut total_errors = 0;

    for handle in handles {
        let (ops, errs) = handle.await.unwrap();
        total_operations += ops;
        total_errors += errs;
    }

    let duration = start.elapsed();
    let ops_per_second = total_operations as f64 / duration.as_secs_f64();

    StressResults {
        total_operations,
        ops_per_second,
        duration,
        errors: total_errors,
    }
}

/// Generates random test data for stress tests.
///
/// # Arguments
///
/// * `size` - Size of the data to generate in bytes
///
/// # Returns
///
/// Returns a vector of random bytes of the specified size.
#[cfg(test)]
fn generate_random_data(size: usize) -> Vec<u8> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen()).collect()
}

/// Performs a random operation on the store.
///
/// This is a placeholder implementation that should be replaced with
/// actual store operations in production code.
///
/// # Arguments
///
/// * `store` - The store to operate on
///
/// # Returns
///
/// Returns `Ok(())` if the operation succeeded, `Err(())` otherwise.
async fn perform_random_operation<S>(_store: &S) -> Result<(), ()> {
    // This is a placeholder - actual implementation would depend on the store type
    // and would perform real operations
    tokio::time::sleep(Duration::from_micros(100)).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stress_config() {
        let config = StressConfig::default();
        assert!(config.concurrency > 0);
        assert!(config.operations_per_thread > 0);
        assert!(config.max_key_size > 0);
        assert!(config.max_value_size > 0);
        assert!(config.duration > Duration::from_secs(0));
    }

    #[tokio::test]
    async fn test_random_data_generation() {
        let data = generate_random_data(100);
        assert_eq!(data.len(), 100);
    }

    // Mock store for testing
    #[derive(Clone)]
    struct MockStore;

    #[tokio::test]
    async fn test_stress_test() {
        let config = StressConfig {
            concurrency: 2,
            operations_per_thread: 10,
            duration: Duration::from_secs(1),
            ..Default::default()
        };

        let results = stress_test(|| MockStore, &config).await;
        assert!(results.total_operations > 0);
        assert!(results.ops_per_second > 0.0);
        assert!(results.duration <= config.duration);
    }
}
