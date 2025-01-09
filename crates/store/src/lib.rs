//! Store abstraction layer for securely support crates.
//!
//! This crate provides a flexible key-value store interface that can be implemented
//! by various backend engines. It includes built-in support for:
//! - Sled (default): An embedded key-value store
//! - SQLite (optional): A relational database with key-value capabilities
//! - More backends can be added by implementing the traits
//!
//! # Features
//!
//! - Async-first design with `async-trait`
//! - Type-safe operations with serde integration
//! - Batch operations for atomic updates
//! - Range and prefix scanning
//! - Configurable persistence options
//!
//! # Example
//!
//! ```rust,no_run
//! use store::{Store, TypedStore};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct User {
//!     name: String,
//!     age: u32,
//! }
//!
//! #[tokio::main]
//! async fn main() -> store::Result<()> {
//!     let store = store::backends::SledStore::open("users.db").await?;
//!
//!     // Store a typed value
//!     let user = User {
//!         name: "Alice".into(),
//!         age: 30,
//!     };
//!     store.set_typed("user:1", &user).await?;
//!
//!     // Retrieve the value
//!     if let Some(user) = store.get_typed::<_, User>("user:1").await? {
//!         println!("Found user: {} ({})", user.name, user.age);
//!     }
//!
//!     Ok(())
//! }
//! ```

use std::path::PathBuf;
use async_trait::async_trait;
use bytes::Bytes;
use serde::{de::DeserializeOwned, Serialize};

pub mod error;
pub mod config;
pub mod backends;
mod utils;

pub use error::{Error, Result};
pub use config::Repository as Config;

/// Core storage interface
#[async_trait]
pub trait Store: Send + Sync + 'static {
    /// Get a value by key
    async fn get<K>(&self, key: K) -> Result<Option<Bytes>>
    where
        K: AsRef<[u8]> + Send + Sync;

    /// Set a value by key
    async fn set<K, V>(&self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]> + Send + Sync,
        V: AsRef<[u8]> + Send + Sync;

    /// Delete a value by key
    async fn delete<K>(&self, key: K) -> Result<()>
    where
        K: AsRef<[u8]> + Send + Sync;

    /// Check if a key exists
    async fn contains<K>(&self, key: K) -> Result<bool>
    where
        K: AsRef<[u8]> + Send + Sync;

    /// Clear all entries
    async fn clear(&self) -> Result<()>;

    /// Flush changes to disk
    async fn flush(&self) -> Result<()>;
}

/// Range operations interface
#[async_trait]
pub trait RangeStore: Store {
    /// Type representing a range
    type Range: Send;
    /// Type representing an iterator over entries
    type Iter: Iterator<Item = Result<(Vec<u8>, Vec<u8>)>> + Send;

    /// Get a range of entries
    async fn range<R>(&self, range: R) -> Result<Self::Iter>
    where
        R: Into<Self::Range> + Send;

    /// Get entries matching a prefix
    async fn scan_prefix<P>(&self, prefix: P) -> Result<Self::Iter>
    where
        P: AsRef<[u8]> + Send + Sync;
}

/// High-level interface for storing serializable types
#[async_trait]
pub trait TypedStore: Store {
    /// Get a typed value by key
    async fn get_typed<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]> + Send + Sync,
        V: DeserializeOwned + Send;

    /// Set a typed value by key
    async fn set_typed<K, V>(&self, key: K, value: &V) -> Result<()>
    where
        K: AsRef<[u8]> + Send + Sync,
        V: Serialize + Send + Sync;
}

/// Batch operations interface
#[async_trait]
pub trait BatchStore: Store {
    /// Type representing a batch of operations
    type Batch: BatchOperation + Send;

    /// Create a new batch
    fn batch(&self) -> Self::Batch;

    /// Execute a batch of operations
    async fn execute_batch(&self, batch: Self::Batch) -> Result<()>;
}

/// Interface for batch operations
pub trait BatchOperation {
    /// Set a value in the batch
    fn set<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>;

    /// Delete a value in the batch
    fn delete<K>(&mut self, key: K) -> Result<()>
    where
        K: AsRef<[u8]>;

    /// Clear all operations in the batch
    fn clear(&mut self);
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use testing::TestRuntime;

    pub async fn test_store_implementation<S: Store>(store: S) -> Result<()> {
        // Basic operations
        store.set(b"key1", b"value1").await?;
        assert_eq!(store.get(b"key1").await?.unwrap(), Bytes::from("value1"));

        // Delete
        store.delete(b"key1").await?;
        assert!(store.get(b"key1").await?.is_none());

        // Contains
        store.set(b"key2", b"value2").await?;
        assert!(store.contains(b"key2").await?);

        // Clear
        store.clear().await?;
        assert!(!store.contains(b"key2").await?);

        Ok(())
    }
}
