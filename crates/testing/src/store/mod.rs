//! Testing utilities for store implementations.
//!
//! This module provides core testing utilities and traits for store implementations,
//! including temporary directory management, test store creation, and test contexts.
//!
//! # Components
//!
//! - [`TempStoreDir`]: Manages temporary directories for store tests
//! - [`TestStore`]: Trait for creating test store instances
//! - [`StoreTestContext`]: Test context that manages store lifecycle
//!
//! # Submodules
//!
//! - [`bench`]: Benchmarking utilities for measuring store performance
//! - [`edge`]: Edge case testing for validating boundary conditions
//! - [`property`]: Property-based testing for validating store behavior
//! - [`stress`]: Stress testing for concurrent operations
//!
//! # Example
//!
//! ```rust,no_run
//! use testing::store::{TestStore, StoreTestContext};
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
//! #[tokio::test]
//! async fn test_with_context() {
//!     let context = StoreTestContext::<MyStore>::new();
//!     let store = context.store;
//!     // Test store operations...
//! }
//! ```

pub mod bench;
pub mod edge;
pub mod property;
pub mod stress;

use std::path::PathBuf;
use tempfile::TempDir;

/// A temporary directory for store tests.
///
/// This struct manages the lifecycle of a temporary directory used for store tests.
/// The directory is automatically cleaned up when the struct is dropped.
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::TempStoreDir;
///
/// let temp_dir = TempStoreDir::new();
/// assert!(temp_dir.path.exists());
/// // Use temp_dir.path for store operations...
/// ```
pub struct TempStoreDir {
    _dir: TempDir,
    /// Path to the temporary directory
    pub path: PathBuf,
}

impl TempStoreDir {
    /// Creates a new temporary directory for store tests.
    ///
    /// The directory is automatically cleaned up when the returned `TempStoreDir`
    /// is dropped.
    ///
    /// # Panics
    ///
    /// Panics if the temporary directory cannot be created.
    pub fn new() -> Self {
        let dir = TempDir::new().expect("Failed to create temporary directory");
        let path = dir.path().to_path_buf();
        Self { _dir: dir, path }
    }
}

impl Default for TempStoreDir {
    fn default() -> Self {
        Self::new()
    }
}

/// A trait for creating test store instances.
///
/// This trait should be implemented by store types to provide a standard way
/// of creating test instances. The test instance should be configured appropriately
/// for testing, possibly using a temporary directory.
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::TestStore;
///
/// #[derive(Clone)]
/// struct MyStore;
///
/// impl TestStore for MyStore {
///     fn new_test_store() -> Self {
///         // Create a new test instance
///         MyStore
///     }
/// }
/// ```
pub trait TestStore: Clone + Send + Sync + 'static {
    /// Creates a new test store instance.
    ///
    /// This method should create a new store instance configured appropriately
    /// for testing. The instance should be isolated from other test instances
    /// to prevent interference.
    fn new_test_store() -> Self;
}

/// A test context for store tests.
///
/// This struct manages the lifecycle of a store instance and its associated
/// temporary directory. It ensures proper cleanup when tests complete.
///
/// # Type Parameters
///
/// * `S` - The store type that implements [`TestStore`]
///
/// # Example
///
/// ```rust,no_run
/// use testing::store::{TestStore, StoreTestContext};
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
/// #[tokio::test]
/// async fn test_with_context() {
///     let context = StoreTestContext::<MyStore>::new();
///     let store = context.store;
///     // Test store operations...
/// }
/// ```
pub struct StoreTestContext<S: TestStore> {
    _temp_dir: TempStoreDir,
    /// The store instance being tested
    pub store: S,
}

impl<S: TestStore> StoreTestContext<S> {
    /// Creates a new test context with a store instance.
    ///
    /// This creates a new temporary directory and initializes a store instance
    /// within it. The directory and store are cleaned up when the context is dropped.
    pub fn new() -> Self {
        let temp_dir = TempStoreDir::new();
        let store = S::new_test_store();
        Self {
            _temp_dir: temp_dir,
            store,
        }
    }
}

impl<S: TestStore> Default for StoreTestContext<S> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct MockStore;

    impl TestStore for MockStore {
        fn new_test_store() -> Self {
            Self
        }
    }

    #[test]
    fn test_temp_store_dir() {
        let temp_dir = TempStoreDir::new();
        assert!(temp_dir.path.exists());
        assert!(temp_dir.path.is_dir());
    }

    #[test]
    fn test_store_test_context() {
        let context = StoreTestContext::<MockStore>::new();
        assert!(context._temp_dir.path.exists());
        assert!(context._temp_dir.path.is_dir());
    }
}
