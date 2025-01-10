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

use bytes::Bytes;
use std::result::Result as StdResult;

pub mod error;
pub mod backends;

pub use error::Error;
pub type Result<T, E = Error> = StdResult<T, E>;

#[async_trait::async_trait]
pub trait Store: Send + Sync + 'static {
    type Error;

    async fn get(&self, key: &[u8]) -> StdResult<Option<Bytes>, Self::Error>;
    async fn set(&self, key: &[u8], value: Bytes) -> StdResult<(), Self::Error>;
    async fn delete(&self, key: &[u8]) -> StdResult<(), Self::Error>;
    async fn batch_set(&self, kvs: Vec<(Vec<u8>, Bytes)>) -> StdResult<(), Self::Error>;
    async fn batch_delete(&self, keys: Vec<Vec<u8>>) -> StdResult<(), Self::Error>;
    async fn range(&self, range: std::ops::Range<&[u8]>) -> StdResult<Vec<(Vec<u8>, Bytes)>, Self::Error>;
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

/// Batch operations interface
#[async_trait::async_trait]
pub trait BatchStore: Store {
    /// Type representing a batch of operations
    type Batch: BatchOperation + Send;

    /// Create a new batch
    fn batch(&self) -> Self::Batch;

    /// Execute a batch of operations
    async fn execute_batch(&self, batch: Self::Batch) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use testing::store::TestStore;

    #[async_trait::async_trait]
    impl TestStore for backends::sled::SledStore {
        fn new_test_store() -> Self {
            Self::new_test_store()
        }

        async fn get(&self, key: &[u8]) -> std::result::Result<Option<Bytes>, Box<dyn std::error::Error>> {
            Ok(Store::get(self, key).await?)
        }

        async fn set(&self, key: &[u8], value: Bytes) -> std::result::Result<(), Box<dyn std::error::Error>> {
            Ok(Store::set(self, key, value).await?)
        }

        async fn delete(&self, key: &[u8]) -> std::result::Result<(), Box<dyn std::error::Error>> {
            Ok(Store::delete(self, key).await?)
        }

        async fn batch_set(&self, kvs: Vec<(Vec<u8>, Bytes)>) -> std::result::Result<(), Box<dyn std::error::Error>> {
            Ok(Store::batch_set(self, kvs).await?)
        }

        async fn batch_delete(&self, keys: Vec<Vec<u8>>) -> std::result::Result<(), Box<dyn std::error::Error>> {
            Ok(Store::batch_delete(self, keys).await?)
        }

        async fn range(&self, range: std::ops::Range<&[u8]>) -> std::result::Result<Vec<(Vec<u8>, Bytes)>, Box<dyn std::error::Error>> {
            Ok(Store::range(self, range).await?)
        }
    }
}
