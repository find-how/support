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
//! use bytes::Bytes;
//! use std::collections::HashMap;
//! use std::sync::{Arc, Mutex};
//! use async_trait::async_trait;
//!
//! #[derive(Clone)]
//! struct BasicTestStore {
//!     data: Arc<Mutex<HashMap<Vec<u8>, Bytes>>>,
//! }
//!
//! #[async_trait]
//! impl TestStore for BasicTestStore {
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
//! async fn test_with_context() {
//!     let context = StoreTestContext::<BasicTestStore>::new();
//!     let store = context.store;
//!
//!     // Test store operations
//!     store.set(b"key", b"value".into()).await.unwrap();
//!     let value = store.get(b"key").await.unwrap();
//!     assert_eq!(value.as_deref(), Some(b"value".as_ref()));
//! }
//! ```

pub mod bench;
pub mod edge;
pub mod property;
pub mod stress;

use bytes::Bytes;
use std::result::Result;
use std::path::PathBuf;
use tempfile::TempDir;

#[async_trait::async_trait]
pub trait TestStore: Clone + Send + Sync + 'static {
    fn new_test_store() -> Self;

    async fn get(&self, key: &[u8]) -> Result<Option<Bytes>, Box<dyn std::error::Error>>;
    async fn set(&self, key: &[u8], value: Bytes) -> Result<(), Box<dyn std::error::Error>>;
    async fn delete(&self, key: &[u8]) -> Result<(), Box<dyn std::error::Error>>;
    async fn batch_set(&self, kvs: Vec<(Vec<u8>, Bytes)>) -> Result<(), Box<dyn std::error::Error>>;
    async fn batch_delete(&self, keys: Vec<Vec<u8>>) -> Result<(), Box<dyn std::error::Error>>;
    async fn range(&self, range: std::ops::Range<&[u8]>) -> Result<Vec<(Vec<u8>, Bytes)>, Box<dyn std::error::Error>>;
}

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
/// use bytes::Bytes;
/// use std::collections::HashMap;
/// use std::sync::{Arc, Mutex};
/// use async_trait::async_trait;
///
/// #[derive(Clone)]
/// struct ContextTestStore {
///     data: Arc<Mutex<HashMap<Vec<u8>, Bytes>>>,
/// }
///
/// #[async_trait]
/// impl TestStore for ContextTestStore {
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
/// async fn test_with_context() {
///     let context = StoreTestContext::<ContextTestStore>::new();
///     let store = context.store;
///
///     // Test store operations
///     store.set(b"key", b"value".into()).await.unwrap();
///     let value = store.get(b"key").await.unwrap();
///     assert_eq!(value.as_deref(), Some(b"value".as_ref()));
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
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Clone)]
    struct MockStore {
        data: std::sync::Arc<Mutex<HashMap<Vec<u8>, Bytes>>>,
    }

    #[async_trait::async_trait]
    impl TestStore for MockStore {
        fn new_test_store() -> Self {
            Self {
                data: std::sync::Arc::new(Mutex::new(HashMap::new())),
            }
        }

        async fn get(&self, key: &[u8]) -> Result<Option<Bytes>, Box<dyn std::error::Error>> {
            Ok(self.data.lock().unwrap().get(key).cloned())
        }

        async fn set(&self, key: &[u8], value: Bytes) -> Result<(), Box<dyn std::error::Error>> {
            self.data.lock().unwrap().insert(key.to_vec(), value);
            Ok(())
        }

        async fn delete(&self, key: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
            self.data.lock().unwrap().remove(key);
            Ok(())
        }

        async fn batch_set(&self, kvs: Vec<(Vec<u8>, Bytes)>) -> Result<(), Box<dyn std::error::Error>> {
            let mut data = self.data.lock().unwrap();
            for (key, value) in kvs {
                data.insert(key, value);
            }
            Ok(())
        }

        async fn batch_delete(&self, keys: Vec<Vec<u8>>) -> Result<(), Box<dyn std::error::Error>> {
            let mut data = self.data.lock().unwrap();
            for key in keys {
                data.remove(&key);
            }
            Ok(())
        }

        async fn range(&self, range: std::ops::Range<&[u8]>) -> Result<Vec<(Vec<u8>, Bytes)>, Box<dyn std::error::Error>> {
            let data = self.data.lock().unwrap();
            let mut result = Vec::new();
            for (key, value) in data.iter() {
                if key.as_slice() >= range.start && key.as_slice() < range.end {
                    result.push((key.clone(), value.clone()));
                }
            }
            Ok(result)
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
