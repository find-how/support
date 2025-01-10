use bytes::Bytes;
use store::Store;
use std::sync::Arc;
use thiserror::Error;

pub struct Settings {
    store: Arc<dyn Store<Error = store::Error>>,
}

impl Settings {
    pub fn new(store: Arc<dyn Store<Error = store::Error>>) -> Self {
        Self { store }
    }

    pub async fn set(&self, key: &str, value: &str) -> Result<()> {
        let encrypted = self.encrypt(value)?;
        self.store.set(key.as_bytes(), Bytes::from(encrypted)).await?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let value = self.store.get(key.as_bytes()).await?;
        match value {
            Some(bytes) => {
                let decrypted = self.decrypt(&bytes)?;
                Ok(Some(decrypted))
            }
            None => Ok(None),
        }
    }

    pub async fn has(&self, key: &str) -> Result<bool> {
        Ok(self.store.get(key.as_bytes()).await?.is_some())
    }

    fn encrypt(&self, value: &str) -> Result<Vec<u8>> {
        // TODO: Implement encryption
        Ok(value.as_bytes().to_vec())
    }

    fn decrypt(&self, value: &[u8]) -> Result<String> {
        // TODO: Implement decryption
        String::from_utf8(value.to_vec()).map_err(|e| Error::Decryption(e.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Store error: {0}")]
    Store(#[from] store::Error),
    #[error("Encryption error: {0}")]
    Encryption(String),
    #[error("Decryption error: {0}")]
    Decryption(String),
}

pub type Result<T> = std::result::Result<T, Error>;
