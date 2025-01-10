//! Configuration management for Securely server.
//!
//! This crate provides a Laravel-inspired configuration system with:
//! - Dot notation access to nested values
//! - Environment variable overrides
//! - Type-safe getters
//! - Default values
//! - Array operations

use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use serde::{Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ConfigString(String);

impl TryFrom<Value> for ConfigString {
    type Error = serde_json::Error;

    fn try_from(value: Value) -> std::result::Result<Self, Self::Error> {
        match value {
            Value::String(s) => Ok(ConfigString(s)),
            Value::Number(n) => Ok(ConfigString(n.to_string())),
            Value::Bool(b) => Ok(ConfigString(b.to_string())),
            Value::Null => Ok(ConfigString("null".to_string())),
            _ => Ok(ConfigString(value.to_string())),
        }
    }
}

impl From<ConfigString> for String {
    fn from(s: ConfigString) -> Self {
        s.0
    }
}

impl AsRef<str> for ConfigString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Store error: {0}")]
    Store(#[from] store::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Type conversion error: {0}")]
    TypeConversion(String),
    #[error("Invalid key: {0}")]
    InvalidKey(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A configuration repository that provides access to configuration values
pub struct Repository {
    items: Arc<RwLock<HashMap<String, Value>>>,
}

impl Repository {
    /// Create a new configuration repository
    pub fn new(items: HashMap<String, Value>) -> Self {
        Self {
            items: Arc::new(RwLock::new(items)),
        }
    }

    /// Check if a configuration key exists
    pub async fn has(&self, key: &str) -> bool {
        let items = self.items.read().await;
        items.contains_key(key)
    }

    /// Get a typed configuration value
    pub async fn get<T>(&self, key: impl Into<String>) -> Option<T>
    where
        T: TryFrom<Value> + Clone,
    {
        let items = self.items.read().await;
        let key = key.into();
        items.get(&key).and_then(|v| T::try_from(v.clone()).ok())
    }

    /// Get multiple configuration values
    pub async fn get_many<T>(&self, keys: Vec<String>) -> HashMap<String, T>
    where
        T: TryFrom<Value> + Clone,
    {
        let mut result = HashMap::new();
        let items = self.items.read().await;
        for key in keys {
            if let Some(v) = items.get(&key) {
                if let Ok(val) = T::try_from(v.clone()) {
                    result.insert(key, val);
                }
            }
        }
        result
    }

    /// Set a configuration value
    pub async fn set(&self, key: impl Into<String>, value: impl Serialize) -> Result<()> {
        let mut items = self.items.write().await;
        let value = serde_json::to_value(value)?;
        items.insert(key.into(), value);
        Ok(())
    }

    /// Get all configuration values
    pub async fn all(&self) -> HashMap<String, Value> {
        self.items.read().await.clone()
    }

    /// Prepend a value to an array configuration
    pub async fn prepend(&self, key: &str, value: impl Serialize) -> Result<()> {
        let mut items = self.items.write().await;
        let value = serde_json::to_value(value)?;
        let entry = items.entry(key.to_string()).or_insert(Value::Array(vec![]));

        if let Value::Array(ref mut arr) = entry {
            arr.insert(0, value);
        } else {
            *entry = Value::Array(vec![value]);
        }
        Ok(())
    }

    /// Push a value to an array configuration
    pub async fn push(&self, key: &str, value: impl Serialize) -> Result<()> {
        let mut items = self.items.write().await;
        let value = serde_json::to_value(value)?;
        let entry = items.entry(key.to_string()).or_insert(Value::Array(vec![]));

        if let Value::Array(ref mut arr) = entry {
            arr.push(value);
        } else {
            *entry = Value::Array(vec![value]);
        }
        Ok(())
    }

    /// Get a string configuration value
    pub async fn string(&self, key: &str) -> Result<String> {
        let items = self.items.read().await;
        match items.get(key) {
            Some(Value::String(s)) => Ok(s.clone()),
            Some(v) => Err(Error::TypeConversion(format!("Value at key '{}' is not a string: {:?}", key, v))),
            None => Err(Error::InvalidKey(key.to_string())),
        }
    }

    /// Get an array configuration value
    pub async fn array(&self, key: &str) -> Result<Vec<Value>> {
        let items = self.items.read().await;
        match items.get(key) {
            Some(Value::Array(arr)) => Ok(arr.clone()),
            Some(v) => Err(Error::TypeConversion(format!("Value at key '{}' is not an array: {:?}", key, v))),
            None => Err(Error::InvalidKey(key.to_string())),
        }
    }

    /// Get a boolean configuration value
    pub async fn boolean(&self, key: &str) -> Result<bool> {
        let items = self.items.read().await;
        match items.get(key) {
            Some(Value::Bool(b)) => Ok(*b),
            Some(v) => Err(Error::TypeConversion(format!("Value at key '{}' is not a boolean: {:?}", key, v))),
            None => Err(Error::InvalidKey(key.to_string())),
        }
    }

    /// Get an integer configuration value
    pub async fn integer(&self, key: &str) -> Result<i64> {
        let items = self.items.read().await;
        match items.get(key) {
            Some(Value::Number(n)) => n.as_i64().ok_or_else(|| Error::TypeConversion(format!("Value at key '{}' is not an integer", key))),
            Some(v) => Err(Error::TypeConversion(format!("Value at key '{}' is not an integer: {:?}", key, v))),
            None => Err(Error::InvalidKey(key.to_string())),
        }
    }

    /// Get a float configuration value
    pub async fn float(&self, key: &str) -> Result<f64> {
        let items = self.items.read().await;
        match items.get(key) {
            Some(Value::Number(n)) => n.as_f64().ok_or_else(|| Error::TypeConversion(format!("Value at key '{}' is not a float", key))),
            Some(v) => Err(Error::TypeConversion(format!("Value at key '{}' is not a float: {:?}", key, v))),
            None => Err(Error::InvalidKey(key.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_repository() -> Repository {
        let mut items = HashMap::new();
        items.insert(
            "app.name".to_string(),
            Value::String("Test App".to_string()),
        );
        items.insert("app.debug".to_string(), Value::Bool(true));
        items.insert(
            "database.host".to_string(),
            Value::String("localhost".to_string()),
        );
        items.insert("database.port".to_string(), Value::Number(5432.into()));
        Repository::new(items)
    }

    #[tokio::test]
    async fn test_get_value_when_key_contain_dot() {
        let config = setup_repository().await;
        let value: Option<String> = config.get("app.name").await;
        assert_eq!(value, Some("Test App".to_string()));
    }

    #[tokio::test]
    async fn test_get_boolean_value() {
        let config = setup_repository().await;
        let value = config.boolean("app.debug").await.unwrap();
        assert!(value);
    }

    #[tokio::test]
    async fn test_get_null_value() {
        let config = setup_repository().await;
        let value: Option<String> = config.get("nonexistent").await;
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_has_is_true() {
        let config = setup_repository().await;
        assert!(config.has("app.name").await);
    }

    #[tokio::test]
    async fn test_has_is_false() {
        let config = setup_repository().await;
        assert!(!config.has("nonexistent").await);
    }

    #[tokio::test]
    async fn test_get() {
        let config = setup_repository().await;
        let value: Option<String> = config.get("app.name").await;
        assert_eq!(value, Some("Test App".to_string()));
    }

    #[tokio::test]
    async fn test_get_with_array_of_keys() {
        let config = setup_repository().await;
        let keys = vec![
            "database.host".to_string(),
            "database.port".to_string(),
        ];
        let values = config.get_many::<Value>(keys).await;
        assert_eq!(values.len(), 2);
        assert_eq!(values["database.host"], Value::String("localhost".to_string()));
        assert_eq!(values["database.port"], Value::Number(5432.into()));
    }

    #[tokio::test]
    async fn test_set() {
        let config = setup_repository().await;
        config.set("new.key", "new value").await.unwrap();
        let value: Option<String> = config.get("new.key").await;
        assert_eq!(value, Some("new value".to_string()));
    }

    #[tokio::test]
    async fn test_set_array() {
        let config = setup_repository().await;
        let arr = vec!["value1", "value2"];
        config.set("array.key", arr).await.unwrap();
        let value = config.array("array.key").await.unwrap();
        assert_eq!(value.len(), 2);
        assert_eq!(value[0], Value::String("value1".to_string()));
        assert_eq!(value[1], Value::String("value2".to_string()));
    }

    #[tokio::test]
    async fn test_array_operations() {
        let config = setup_repository().await;

        // Test prepend
        config.prepend("list", "first").await.unwrap();
        config.prepend("list", "new first").await.unwrap();
        let arr = config.array("list").await.unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], Value::String("new first".to_string()));
        assert_eq!(arr[1], Value::String("first".to_string()));

        // Test push
        config.push("list", "last").await.unwrap();
        let arr = config.array("list").await.unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[2], Value::String("last".to_string()));
    }

    #[tokio::test]
    async fn test_type_specific_getters() {
        let config = setup_repository().await;

        assert_eq!(config.string("app.name").await.unwrap(), "Test App");
        assert!(config.boolean("app.debug").await.unwrap());
        assert_eq!(config.integer("database.port").await.unwrap(), 5432);

        // Test error cases
        assert!(config.string("app.debug").await.is_err());
        assert!(config.boolean("app.name").await.is_err());
        assert!(config.integer("database.host").await.is_err());
    }
}
