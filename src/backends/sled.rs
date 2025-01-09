use crate::error::Error;
use crate::Store;
use bytes::Bytes;
use sled::Db;
use std::path::Path;

pub struct SledStore {
    db: Db,
}

impl SledStore {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }
}

impl Store for SledStore {
    fn get(&self, key: &str) -> Result<Option<Bytes>, Error> {
        Ok(self.db.get(key)?.map(|v| Bytes::copy_from_slice(&v)))
    }

    fn set(&self, key: &str, value: Bytes) -> Result<(), Error> {
        self.db.insert(key, value.as_ref())?;
        self.db.flush()?;
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), Error> {
        self.db.remove(key)?;
        self.db.flush()?;
        Ok(())
    }

    fn exists(&self, key: &str) -> Result<bool, Error> {
        Ok(self.db.contains_key(key)?)
    }

    fn clear(&self) -> Result<(), Error> {
        self.db.clear()?;
        self.db.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sled_store() -> Result<(), Error> {
        let dir = tempdir()?;
        let store = SledStore::new(dir.path())?;

        // Test set and get
        let key = "test_key";
        let value = Bytes::from("test_value");
        store.set(key, value.clone())?;
        assert_eq!(store.get(key)?, Some(value));

        // Test exists
        assert!(store.exists(key)?);
        assert!(!store.exists("nonexistent")?);

        // Test delete
        store.delete(key)?;
        assert_eq!(store.get(key)?, None);
        assert!(!store.exists(key)?);

        // Test clear
        store.set(key, value)?;
        store.clear()?;
        assert_eq!(store.get(key)?, None);

        Ok(())
    }
}
