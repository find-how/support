use std::env;
use std::sync::Arc;
use sled::Db;
use thiserror::Error;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use encrypt::Crypt;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sled::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Encryption error: {0}")]
    Encryption(#[from] encrypt::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Value not found")]
    NotFound,
}

pub type Result<T> = std::result::Result<T, Error>;

/// The main Settings facade
pub struct Settings {
    db: Arc<Db>,
}

impl Settings {
    /// Create a new Settings instance with default sled database
    pub fn new() -> Result<Self> {
        let db_path = env::current_dir()?.join("settings").join("db");
        std::fs::create_dir_all(&db_path)?;
        Ok(Self {
            db: Arc::new(sled::open(db_path)?),
        })
    }

    /// Use a custom database driver
    pub fn driver(db: Db) -> Self {
        Self { db: Arc::new(db) }
    }

    /// Get a value by key
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.db.get(key.as_bytes())? {
            Some(bytes) => {
                let decrypted = Crypt::decrypt_string(&String::from_utf8_lossy(&bytes))?;
                let value = serde_json::from_str(&decrypted)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Set a value by key
    pub fn set<T: Serialize>(&self, key: &str, value: T) -> Result<()> {
        let json = serde_json::to_string(&value)?;
        let encrypted = Crypt::encrypt_string(&json)?;
        self.db.insert(key.as_bytes(), encrypted.as_bytes())?;
        Ok(())
    }

    /// Check if a key exists
    pub fn has(&self, key: &str) -> Result<bool> {
        Ok(self.db.contains_key(key.as_bytes())?)
    }

    /// Get an environment variable with optional default
    pub fn env<T: DeserializeOwned>(&self, key: &str, default: Option<T>) -> Result<T> {
        match env::var(key) {
            Ok(value) => Ok(serde_json::from_str(&value)?),
            Err(_) => match default {
                Some(value) => Ok(value),
                None => Err(Error::NotFound),
            },
        }
    }

    /// Store a secret value (automatically encrypted)
    pub fn secret<T: Serialize>(&self, key: &str, value: T) -> Result<()> {
        self.set(key, value)
    }

    /// Delete a value by key
    pub fn forget(&self, key: &str) -> Result<()> {
        self.db.remove(key.as_bytes())?;
        Ok(())
    }

    /// List all settings or under a namespace
    pub fn all(&self, namespace: Option<&str>) -> Result<Vec<(String, Value)>> {
        let mut settings = Vec::new();
        for result in self.db.iter() {
            let (key, value) = result?;
            let key_str = String::from_utf8_lossy(&key).into_owned();

            if let Some(ns) = namespace {
                if !key_str.starts_with(ns) {
                    continue;
                }
            }

            let decrypted = Crypt::decrypt_string(&String::from_utf8_lossy(&value))?;
            let value: Value = serde_json::from_str(&decrypted)?;
            settings.push((key_str, value));
        }
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    fn setup_test_encryption() {
        env::set_var("APP_KEY", "base64:dGVzdGtleXRlc3RrZXl0ZXN0a2V5dGVzdGtleXRlc3Q=");
        Crypt::initialize().unwrap();
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Config {
        name: String,
        port: u16,
    }

    #[test]
    fn test_basic_operations() -> Result<()> {
        setup_test_encryption();
        let dir = tempdir()?;
        let db = sled::open(dir.path())?;
        let settings = Settings::driver(db);

        // Test set and get
        settings.set("app.name", "Test App")?;
        assert_eq!(settings.get::<String>("app.name")?.unwrap(), "Test App");

        // Test has
        assert!(settings.has("app.name")?);
        assert!(!settings.has("nonexistent")?);

        // Test forget
        settings.forget("app.name")?;
        assert!(!settings.has("app.name")?);

        Ok(())
    }

    #[test]
    fn test_json_values() -> Result<()> {
        setup_test_encryption();
        let dir = tempdir()?;
        let db = sled::open(dir.path())?;
        let settings = Settings::driver(db);

        let config = Config {
            name: "Test".to_string(),
            port: 8080,
        };

        settings.set("app.config", &config)?;
        let stored: Config = settings.get("app.config")?.unwrap();
        assert_eq!(stored, config);

        Ok(())
    }

    #[test]
    fn test_env_values() -> Result<()> {
        setup_test_encryption();
        let settings = Settings::new()?;

        env::set_var("TEST_KEY", "\"test value\"");
        assert_eq!(
            settings.env::<String>("TEST_KEY", None)?,
            "test value"
        );

        assert_eq!(
            settings.env::<String>("NONEXISTENT", Some("default".to_string()))?,
            "default"
        );

        Ok(())
    }
}
