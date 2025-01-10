use crate::{Error, Store, StdResult, BatchOperation, BatchStore};
use async_trait::async_trait;
use bytes::Bytes;
use sled::Db;
use std::sync::Arc;

#[derive(Clone)]
pub struct SledStore {
    db: Arc<Db>,
}

impl SledStore {
    pub fn new(path: impl AsRef<std::path::Path>) -> StdResult<Self, Error> {
        let db = sled::open(path)?;
        Ok(Self { db: Arc::new(db) })
    }

    pub fn new_test_store() -> Self {
        let db = sled::Config::new()
            .temporary(true)
            .open()
            .expect("Failed to create test store");
        Self { db: Arc::new(db) }
    }
}

#[async_trait]
impl Store for SledStore {
    type Error = Error;

    async fn get(&self, key: &[u8]) -> StdResult<Option<Bytes>, Self::Error> {
        match self.db.get(key)? {
            Some(value) => Ok(Some(Bytes::copy_from_slice(&value))),
            None => Ok(None),
        }
    }

    async fn set(&self, key: &[u8], value: Bytes) -> StdResult<(), Self::Error> {
        self.db.insert(key, value.as_ref())?;
        self.db.flush()?;
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> StdResult<(), Self::Error> {
        self.db.remove(key)?;
        self.db.flush()?;
        Ok(())
    }

    async fn batch_set(&self, kvs: Vec<(Vec<u8>, Bytes)>) -> StdResult<(), Self::Error> {
        let mut batch = sled::Batch::default();
        for (key, value) in kvs {
            batch.insert(key.as_slice(), value.as_ref());
        }
        self.db.apply_batch(batch)?;
        self.db.flush()?;
        Ok(())
    }

    async fn batch_delete(&self, keys: Vec<Vec<u8>>) -> StdResult<(), Self::Error> {
        let mut batch = sled::Batch::default();
        for key in keys {
            batch.remove(key.as_slice());
        }
        self.db.apply_batch(batch)?;
        self.db.flush()?;
        Ok(())
    }

    async fn range(&self, range: std::ops::Range<&[u8]>) -> StdResult<Vec<(Vec<u8>, Bytes)>, Self::Error> {
        let mut result = Vec::new();
        for item in self.db.range(range) {
            let (key, value) = item?;
            result.push((key.to_vec(), Bytes::copy_from_slice(&value)));
        }
        Ok(result)
    }
}

pub struct SledBatch {
    batch: sled::Batch,
}

impl BatchOperation for SledBatch {
    fn set<K, V>(&mut self, key: K, value: V) -> crate::Result<()>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        self.batch.insert(key.as_ref(), value.as_ref());
        Ok(())
    }

    fn delete<K>(&mut self, key: K) -> crate::Result<()>
    where
        K: AsRef<[u8]>,
    {
        self.batch.remove(key.as_ref());
        Ok(())
    }

    fn clear(&mut self) {
        self.batch = sled::Batch::default();
    }
}

#[async_trait]
impl BatchStore for SledStore {
    type Batch = SledBatch;

    fn batch(&self) -> Self::Batch {
        SledBatch {
            batch: sled::Batch::default(),
        }
    }

    async fn execute_batch(&self, batch: Self::Batch) -> crate::Result<()> {
        self.db.apply_batch(batch.batch)?;
        self.db.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_basic_operations() -> crate::Result<()> {
        let dir = tempdir()?;
        let store = SledStore::new(dir.path())?;

        // Test set and get
        store.set(b"key1", Bytes::from("value1")).await?;
        assert_eq!(store.get(b"key1").await?.unwrap(), Bytes::from("value1"));

        // Test existence
        assert!(store.get(b"key1").await?.is_some());
        assert!(store.get(b"nonexistent").await?.is_none());

        // Test delete
        store.delete(b"key1").await?;
        assert!(store.get(b"key1").await?.is_none());

        // Test batch operations
        store.set(b"key2", Bytes::from("value2")).await?;
        store.batch_delete(vec![b"key2".to_vec()]).await?;
        assert!(store.get(b"key2").await?.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_batch_operations() -> crate::Result<()> {
        let dir = tempdir()?;
        let store = SledStore::new(dir.path())?;
        let mut batch = store.batch();

        // Add operations to batch
        batch.set(b"batch1", b"value1")?;
        batch.set(b"batch2", b"value2")?;

        // Execute batch
        store.execute_batch(batch).await?;

        // Verify results
        assert_eq!(store.get(b"batch1").await?.unwrap(), Bytes::from("value1"));
        assert_eq!(store.get(b"batch2").await?.unwrap(), Bytes::from("value2"));

        Ok(())
    }
}
