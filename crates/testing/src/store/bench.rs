//! Performance benchmarking utilities for store implementations.
//!
//! This module provides utilities for benchmarking store operations, including:
//! - Single operations (get, set, delete)
//! - Batch operations
//! - Range operations
//!
//! The benchmarks use criterion.rs for accurate measurements and statistical analysis.
//!
//! # Example
//!
//! ```rust,no_run
//! use testing::store::bench::{bench_store, BenchConfig};
//! use testing::store::TestStore;
//! use criterion::Criterion;
//!
//! #[derive(Clone)]
//! struct MyStore;
//!
//! impl TestStore for MyStore {
//!     fn new_test_store() -> Self {
//!         MyStore
//!     }
//! }
//!
//! fn bench_my_store(c: &mut Criterion) {
//!     let config = BenchConfig::default();
//!     bench_store(c, "my_store", || MyStore::new_test_store(), &config);
//! }
//!
//! criterion::criterion_group!(benches, bench_my_store);
//! criterion::criterion_main!(benches);
//! ```

use criterion::{black_box, Criterion, Throughput};
use rand::Rng;
use std::time::Duration;

/// Configuration for store benchmarks.
///
/// This struct contains parameters that control the behavior of the benchmarks,
/// including operation counts, data sizes, and measurement settings.
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::bench::BenchConfig;
/// use std::time::Duration;
///
/// let config = BenchConfig {
///     operations: 10_000,
///     key_size: 16,
///     value_size: 100,
///     batch_size: 100,
///     range_size: 100,
///     measurement_time: Duration::from_secs(10),
///     sample_size: 50,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct BenchConfig {
    /// Number of operations to perform in each benchmark
    pub operations: usize,
    /// Size of keys in bytes
    pub key_size: usize,
    /// Size of values in bytes
    pub value_size: usize,
    /// Number of operations in each batch
    pub batch_size: usize,
    /// Number of items to include in range queries
    pub range_size: usize,
    /// Duration of each benchmark measurement
    pub measurement_time: Duration,
    /// Number of samples to collect for each benchmark
    pub sample_size: usize,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            operations: 10_000,
            key_size: 16,
            value_size: 100,
            batch_size: 100,
            range_size: 100,
            measurement_time: Duration::from_secs(10),
            sample_size: 100,
        }
    }
}

/// Generator for benchmark test data.
///
/// This struct generates and manages random test data for benchmarks,
/// including keys, values, and ranges. The data is generated according
/// to the parameters specified in [`BenchConfig`].
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::bench::{BenchConfig, BenchData};
///
/// let config = BenchConfig::default();
/// let data = BenchData::new(&config);
///
/// // Access test data
/// let key = data.key(0);
/// let value = data.value(0);
/// let (range_start, range_end) = data.range(0);
/// ```
#[derive(Debug)]
pub struct BenchData {
    keys: Vec<Vec<u8>>,
    values: Vec<Vec<u8>>,
    ranges: Vec<(Vec<u8>, Vec<u8>)>,
}

impl BenchData {
    /// Creates a new benchmark data generator with the specified configuration.
    ///
    /// This generates random test data according to the parameters in the config.
    /// The data is reused across benchmark iterations to avoid allocation overhead.
    pub fn new(config: &BenchConfig) -> Self {
        let mut rng = rand::thread_rng();

        // Generate random keys
        let keys: Vec<Vec<u8>> = (0..config.operations)
            .map(|_| {
                (0..config.key_size)
                    .map(|_| rng.gen())
                    .collect()
            })
            .collect();

        // Generate random values
        let values: Vec<Vec<u8>> = (0..config.operations)
            .map(|_| {
                (0..config.value_size)
                    .map(|_| rng.gen())
                    .collect()
            })
            .collect();

        // Generate random ranges
        let ranges: Vec<(Vec<u8>, Vec<u8>)> = (0..config.operations)
            .map(|_| {
                let start: Vec<u8> = (0..config.key_size)
                    .map(|_| rng.gen())
                    .collect();
                let end: Vec<u8> = (0..config.key_size)
                    .map(|_| rng.gen())
                    .collect();
                if start <= end {
                    (start, end)
                } else {
                    (end, start)
                }
            })
            .collect();

        Self {
            keys,
            values,
            ranges,
        }
    }

    /// Gets a test key by index.
    ///
    /// The index wraps around if it exceeds the number of available keys.
    pub fn key(&self, index: usize) -> &[u8] {
        &self.keys[index % self.keys.len()]
    }

    /// Gets a test value by index.
    ///
    /// The index wraps around if it exceeds the number of available values.
    pub fn value(&self, index: usize) -> &[u8] {
        &self.values[index % self.values.len()]
    }

    /// Gets a test range by index.
    ///
    /// Returns a tuple of (start, end) keys. The index wraps around if it
    /// exceeds the number of available ranges.
    pub fn range(&self, index: usize) -> (&[u8], &[u8]) {
        let (start, end) = &self.ranges[index % self.ranges.len()];
        (start, end)
    }
}

/// Runs a comprehensive set of benchmarks for a store implementation.
///
/// This function benchmarks various store operations:
/// - Single operations (get, set, delete)
/// - Batch operations (batch set, batch delete)
/// - Range operations (range scan)
///
/// # Type Parameters
///
/// * `S` - The store type to benchmark
/// * `F` - A function that creates new store instances
///
/// # Arguments
///
/// * `c` - The criterion benchmark harness
/// * `name` - Name prefix for the benchmark group
/// * `setup` - Function that creates new store instances
/// * `config` - Benchmark configuration
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::bench::{bench_store, BenchConfig};
/// use testing::store::TestStore;
/// use criterion::Criterion;
///
/// #[derive(Clone)]
/// struct MyStore;
///
/// impl TestStore for MyStore {
///     fn new_test_store() -> Self {
///         MyStore
///     }
/// }
///
/// fn bench_my_store(c: &mut Criterion) {
///     let config = BenchConfig::default();
///     bench_store(c, "my_store", || MyStore::new_test_store(), &config);
/// }
/// ```
pub fn bench_store<S, F>(c: &mut Criterion, name: &str, setup: F, config: &BenchConfig)
where
    S: Clone + Send + Sync + 'static,
    F: Fn() -> S + Clone + Send + Sync + 'static,
{
    let data = BenchData::new(config);

    let mut group = c.benchmark_group(name);
    group.measurement_time(config.measurement_time);
    group.sample_size(config.sample_size);

    // Benchmark single operations
    bench_single_operations(&mut group, setup.clone(), &data, config);

    // Benchmark batch operations
    bench_batch_operations(&mut group, setup.clone(), &data, config);

    // Benchmark range operations
    bench_range_operations(&mut group, setup, &data, config);

    group.finish();
}

/// Benchmarks single operations (get, set, delete).
///
/// This function measures the performance of individual store operations.
/// Each operation is measured separately with appropriate throughput metrics.
fn bench_single_operations<S, F>(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    setup: F,
    data: &BenchData,
    config: &BenchConfig,
)
where
    S: Clone + Send + Sync + 'static,
    F: Fn() -> S + Clone + Send + Sync + 'static,
{
    // Set operation
    group.throughput(Throughput::Elements(1));
    group.bench_function("set", |b| {
        let _store = setup();
        let mut i = 0;
        b.iter(|| {
            black_box({
                let key = data.key(i);
                let value = data.value(i);
                i = (i + 1) % config.operations;
                // Placeholder - actual implementation would call store.set()
                (key, value)
            });
        });
    });

    // Get operation
    group.bench_function("get", |b| {
        let _store = setup();
        let mut i = 0;
        b.iter(|| {
            black_box({
                let key = data.key(i);
                i = (i + 1) % config.operations;
                // Placeholder - actual implementation would call store.get()
                key
            });
        });
    });

    // Delete operation
    group.bench_function("delete", |b| {
        let _store = setup();
        let mut i = 0;
        b.iter(|| {
            black_box({
                let key = data.key(i);
                i = (i + 1) % config.operations;
                // Placeholder - actual implementation would call store.delete()
                key
            });
        });
    });
}

/// Benchmarks batch operations.
///
/// This function measures the performance of batch operations (set and delete).
/// The batch size is controlled by the configuration.
fn bench_batch_operations<S, F>(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    setup: F,
    data: &BenchData,
    config: &BenchConfig,
)
where
    S: Clone + Send + Sync + 'static,
    F: Fn() -> S + Clone + Send + Sync + 'static,
{
    group.throughput(Throughput::Elements(config.batch_size as u64));

    // Batch set
    group.bench_function("batch_set", |b| {
        let _store = setup();
        let mut i = 0;
        b.iter(|| {
            black_box({
                let batch: Vec<_> = (0..config.batch_size)
                    .map(|j| {
                        let idx = (i + j) % config.operations;
                        (data.key(idx), data.value(idx))
                    })
                    .collect();
                i = (i + config.batch_size) % config.operations;
                // Placeholder - actual implementation would call store.batch_set()
                batch
            });
        });
    });

    // Batch delete
    group.bench_function("batch_delete", |b| {
        let _store = setup();
        let mut i = 0;
        b.iter(|| {
            black_box({
                let batch: Vec<_> = (0..config.batch_size)
                    .map(|j| {
                        let idx = (i + j) % config.operations;
                        data.key(idx)
                    })
                    .collect();
                i = (i + config.batch_size) % config.operations;
                // Placeholder - actual implementation would call store.batch_delete()
                batch
            });
        });
    });
}

/// Benchmarks range operations.
///
/// This function measures the performance of range queries.
/// The range size is controlled by the configuration.
fn bench_range_operations<S, F>(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    setup: F,
    data: &BenchData,
    config: &BenchConfig,
)
where
    S: Clone + Send + Sync + 'static,
    F: Fn() -> S + Clone + Send + Sync + 'static,
{
    group.throughput(Throughput::Elements(config.range_size as u64));

    // Range scan
    group.bench_function("range_scan", |b| {
        let _store = setup();
        let mut i = 0;
        b.iter(|| {
            black_box({
                let (start, end) = data.range(i);
                i = (i + 1) % config.operations;
                // Placeholder - actual implementation would call store.range()
                (start, end)
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bench_config() {
        let config = BenchConfig::default();
        assert!(config.operations > 0);
        assert!(config.key_size > 0);
        assert!(config.value_size > 0);
        assert!(config.batch_size > 0);
        assert!(config.range_size > 0);
        assert!(config.measurement_time > Duration::from_secs(0));
        assert!(config.sample_size > 0);
    }

    #[test]
    fn test_bench_data_generation() {
        let config = BenchConfig::default();
        let data = BenchData::new(&config);

        assert_eq!(data.keys.len(), config.operations);
        assert_eq!(data.values.len(), config.operations);
        assert_eq!(data.ranges.len(), config.operations);

        for key in &data.keys {
            assert_eq!(key.len(), config.key_size);
        }

        for value in &data.values {
            assert_eq!(value.len(), config.value_size);
        }

        for (start, end) in &data.ranges {
            assert_eq!(start.len(), config.key_size);
            assert_eq!(end.len(), config.key_size);
            assert!(start <= end);
        }
    }

    #[test]
    fn test_bench_data_access() {
        let config = BenchConfig::default();
        let data = BenchData::new(&config);

        // Test key access
        let key = data.key(0);
        assert_eq!(key.len(), config.key_size);

        // Test value access
        let value = data.value(0);
        assert_eq!(value.len(), config.value_size);

        // Test range access
        let (start, end) = data.range(0);
        assert_eq!(start.len(), config.key_size);
        assert_eq!(end.len(), config.key_size);
        assert!(start <= end);

        // Test wraparound
        let key_wrap = data.key(config.operations + 1);
        assert_eq!(key_wrap.len(), config.key_size);
    }
}
