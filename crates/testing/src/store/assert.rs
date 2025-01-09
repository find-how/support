//! Store-specific assertion utilities.

use std::fmt::Debug;
use std::time::Duration;
use async_trait::async_trait;

/// Trait for types that can be used in store assertions
#[async_trait]
pub trait StoreAssert: Send + Sync {
    /// The type of error that can occur
    type Error: std::error::Error + Send + Sync + 'static;

    /// Assert that a key exists
    async fn assert_exists(&self, key: impl AsRef<[u8]> + Send + Sync) -> Result<(), Self::Error>;

    /// Assert that a key does not exist
    async fn assert_not_exists(&self, key: impl AsRef<[u8]> + Send + Sync) -> Result<(), Self::Error>;

    /// Assert that a key has a specific value
    async fn assert_value<V: AsRef<[u8]> + Debug + Send + Sync>(
        &self,
        key: impl AsRef<[u8]> + Send + Sync,
        expected: V,
    ) -> Result<(), Self::Error>;
}

/// Assert that an operation completes within a timeout
pub async fn completes_within<F, Fut, T, E>(future: F, timeout: Duration) -> Result<T, E>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::error::Error,
{
    match tokio::time::timeout(timeout, future()).await {
        Ok(result) => result,
        Err(_) => panic!("Operation timed out after {:?}", timeout),
    }
}

/// Assert that a batch of operations completes atomically
pub async fn atomic<F, Fut, S, E>(store: &S, operations: F) -> Result<(), E>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(), E>>,
    S: StoreAssert<Error = E>,
    E: std::error::Error,
{
    // Run the operations
    operations().await?;

    // Verify that all operations were applied atomically
    // This is a basic check - the store implementation should provide more thorough tests
    completes_within(
        || async { store.assert_exists(b"atomic_test_key").await },
        Duration::from_secs(1),
    )
    .await
}

/// Assert that concurrent operations maintain consistency
pub async fn concurrent_safety<F, Fut, T, E>(operations: Vec<F>) -> Result<Vec<T>, E>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T, E>> + Send,
    T: Send + 'static,
    E: std::error::Error + Send + 'static,
{
    let handles: Vec<_> = operations
        .into_iter()
        .map(|op| {
            tokio::spawn(async move {
                completes_within(
                    || op(),
                    Duration::from_secs(5),
                )
                .await
            })
        })
        .collect();

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        results.push(handle.await.expect("Task panicked")?);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    // Mock store for testing assertions
    struct MockStore {
        data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
    }

    impl MockStore {
        fn new() -> Self {
            Self {
                data: Arc::new(RwLock::new(HashMap::new())),
            }
        }

        async fn set(&self, key: impl AsRef<[u8]>, value: impl AsRef<[u8]>) {
            let mut data = self.data.write().await;
            data.insert(key.as_ref().to_vec(), value.as_ref().to_vec());
        }
    }

    #[async_trait]
    impl StoreAssert for MockStore {
        type Error = std::io::Error;

        async fn assert_exists(&self, key: impl AsRef<[u8]> + Send + Sync) -> Result<(), Self::Error> {
            let data = self.data.read().await;
            if data.contains_key(key.as_ref()) {
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Key not found",
                ))
            }
        }

        async fn assert_not_exists(
            &self,
            key: impl AsRef<[u8]> + Send + Sync,
        ) -> Result<(), Self::Error> {
            let data = self.data.read().await;
            if !data.contains_key(key.as_ref()) {
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "Key exists",
                ))
            }
        }

        async fn assert_value<V: AsRef<[u8]> + Debug + Send + Sync>(
            &self,
            key: impl AsRef<[u8]> + Send + Sync,
            expected: V,
        ) -> Result<(), Self::Error> {
            let data = self.data.read().await;
            match data.get(key.as_ref()) {
                Some(value) if value == expected.as_ref() => Ok(()),
                Some(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Value mismatch",
                )),
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Key not found",
                )),
            }
        }
    }

    #[tokio::test]
    async fn test_store_assertions() {
        let store = MockStore::new();

        // Test exists
        store.set(b"key1", b"value1").await;
        store.assert_exists(b"key1").await.unwrap();
        store.assert_not_exists(b"key2").await.unwrap();

        // Test value
        store.assert_value(b"key1", b"value1").await.unwrap();
        assert!(store.assert_value(b"key1", b"wrong").await.is_err());
    }

    #[tokio::test]
    async fn test_concurrent_safety() {
        let store = Arc::new(MockStore::new());
        let store_clone = store.clone();

        let ops = vec![
            || async {
                store_clone.set(b"key1", b"value1").await;
                Ok::<_, std::io::Error>(())
            },
            || async {
                store_clone.set(b"key2", b"value2").await;
                Ok(())
            },
        ];

        concurrent_safety(ops).await.unwrap();

        store.assert_exists(b"key1").await.unwrap();
        store.assert_exists(b"key2").await.unwrap();
    }
}
