//! Sled backend implementation for the store.
//!
//! This module provides a Sled-backed implementation of the store traits,
//! offering an embedded key-value store with good performance characteristics.

use std::path::Path;
use async_trait::async_trait;
use bytes::Bytes;
use sled::{Db, IVec};

use crate::{Store, RangeStore, BatchStore, BatchOperation, Result, Error};

pub struct SledStore {
    db: Db,
}

impl SledStore {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let db = sled::open(path).map_err(Error::Backend)?;
        Ok(Self { db })
    }
}

#[async_trait]
impl Store for SledStore {
    async fn get<K>(&self, key: K) -> Result<Option<Bytes>>
    where
        K: AsRef<[u8]> + Send + Sync,
    {
        let result = self.db.get(key.as_ref()).map_err(Error::Backend)?;
        Ok(result.map(|v| Bytes::from(v.to_vec())))
    }

    async fn set<K, V>(&self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]> + Send + Sync,
        V: AsRef<[u8]> + Send + Sync,
    {
        self.db
            .insert(key.as_ref(), value.as_ref())
            .map_err(Error::Backend)?;
        Ok(())
    }

    async fn delete<K>(&self, key: K) -> Result<()>
    where
        K: AsRef<[u8]> + Send + Sync,
    {
        self.db.remove(key.as_ref()).map_err(Error::Backend)?;
        Ok(())
    }

    async fn contains<K>(&self, key: K) -> Result<bool>
    where
        K: AsRef<[u8]> + Send + Sync,
    {
        let result = self.db.contains_key(key.as_ref()).map_err(Error::Backend)?;
        Ok(result)
    }

    async fn clear(&self) -> Result<()> {
        self.db.clear().map_err(Error::Backend)?;
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        self.db.flush().map_err(Error::Backend)?;
        Ok(())
    }
}

#[async_trait]
impl RangeStore for SledStore {
    type Range = std::ops::Range<Vec<u8>>;
    type Iter = Box<dyn Iterator<Item = Result<(Vec<u8>, Vec<u8>)>> + Send>;

    async fn range<R>(&self, range: R) -> Result<Self::Iter>
    where
        R: Into<Self::Range> + Send,
    {
        let range = range.into();
        let iter = self
            .db
            .range(range)
            .map(|r| r.map_err(Error::Backend))
            .map(|r| r.map(|(k, v)| (k.to_vec(), v.to_vec())));
        Ok(Box::new(iter))
    }

    async fn scan_prefix<P>(&self, prefix: P) -> Result<Self::Iter>
    where
        P: AsRef<[u8]> + Send + Sync,
    {
        let iter = self
            .db
            .scan_prefix(prefix.as_ref())
            .map(|r| r.map_err(Error::Backend))
            .map(|r| r.map(|(k, v)| (k.to_vec(), v.to_vec())));
        Ok(Box::new(iter))
    }
}

pub struct SledBatch {
    batch: sled::Batch,
}

impl SledBatch {
    fn new() -> Self {
        Self {
            batch: sled::Batch::default(),
        }
    }
}

impl BatchOperation for SledBatch {
    fn set<K, V>(&mut self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        self.batch.insert(key.as_ref(), value.as_ref());
        Ok(())
    }

    fn delete<K>(&mut self, key: K) -> Result<()>
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
        SledBatch::new()
    }

    async fn execute_batch(&self, batch: Self::Batch) -> Result<()> {
        self.db.apply_batch(batch.batch).map_err(Error::Backend)?;
        Ok(())
    }
}
