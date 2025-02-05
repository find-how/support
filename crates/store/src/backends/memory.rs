use crate::{Store, Error, Result};
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;
use tokio::sync::RwLock;

/// An in-memory store implementation using BTreeMap
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    data: Arc<RwLock<BTreeMap<Vec<u8>, Bytes>>>,
}

impl MemoryStore {
    /// Creates a new empty memory store
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Store for MemoryStore {
    type Error = Error;

    async fn get(&self, key: &[u8]) -> Result<Option<Bytes>> {
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }

    async fn set(&self, key: &[u8], value: Bytes) -> Result<()> {
        let mut data = self.data.write().await;
        data.insert(key.to_vec(), value);
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        let mut data = self.data.write().await;
        data.remove(key);
        Ok(())
    }

    async fn batch_set(&self, kvs: Vec<(Vec<u8>, Bytes)>) -> Result<()> {
        let mut data = self.data.write().await;
        for (key, value) in kvs {
            data.insert(key, value);
        }
        Ok(())
    }

    async fn batch_delete(&self, keys: Vec<Vec<u8>>) -> Result<()> {
        let mut data = self.data.write().await;
        for key in keys {
            data.remove(&key);
        }
        Ok(())
    }

    async fn range(&self, range: Range<&[u8]>) -> Result<Vec<(Vec<u8>, Bytes)>> {
        let data = self.data.read().await;
        Ok(data
            .range(range.start.to_vec()..range.end.to_vec())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    async fn test_basic_operations() {
        let store = MemoryStore::new();
        let key = b"test-key".to_vec();
        let value = Bytes::from("test-value");

        // Test set and get
        store.set(&key, value.clone()).await.unwrap();
        let result = store.get(&key).await.unwrap();
        assert_eq!(result, Some(value));

        // Test delete
        store.delete(&key).await.unwrap();
        let result = store.get(&key).await.unwrap();
        assert_eq!(result, None);
    }

    #[test]
    async fn test_batch_operations() {
        let store = MemoryStore::new();
        let kvs = vec![
            (b"key1".to_vec(), Bytes::from("value1")),
            (b"key2".to_vec(), Bytes::from("value2")),
        ];

        // Test batch set
        store.batch_set(kvs.clone()).await.unwrap();
        for (key, value) in &kvs {
            let result = store.get(key).await.unwrap();
            assert_eq!(result, Some(value.clone()));
        }

        // Test batch delete
        let keys = vec![b"key1".to_vec(), b"key2".to_vec()];
        store.batch_delete(keys).await.unwrap();
        let result = store.get(b"key1").await.unwrap();
        assert_eq!(result, None);
    }

    #[test]
    async fn test_range() {
        let store = MemoryStore::new();
        let kvs = vec![
            (b"a1".to_vec(), Bytes::from("1")),
            (b"a2".to_vec(), Bytes::from("2")),
            (b"b1".to_vec(), Bytes::from("3")),
        ];

        store.batch_set(kvs).await.unwrap();
        let range = store.range(b"a"..b"b").await.unwrap();
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].0, b"a1");
        assert_eq!(range[1].0, b"a2");
    }
}
