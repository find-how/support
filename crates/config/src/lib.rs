//! Configuration management for Securely server.
//!
//! This crate provides a Laravel-inspired configuration system with:
//! - Dot notation access to nested values
//! - Environment variable overrides
//! - Type-safe getters
//! - Default values
//! - Array operations

use std::collections::HashMap;
use serde::{Serialize};
use serde_json::Value;
use thiserror::Error;
use std::convert::TryFrom;

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigString(String);

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigBool(bool);

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigInt(i32);

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigInt64(i64);

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigFloat(f64);

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigArray(Vec<Value>);

impl TryFrom<Value> for ConfigString {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self> {
        match value {
            Value::String(s) => Ok(ConfigString(s)),
            Value::Number(n) => Ok(ConfigString(n.to_string())),
            Value::Bool(b) => Ok(ConfigString(b.to_string())),
            Value::Null => Ok(ConfigString("null".to_string())),
            _ => Ok(ConfigString(value.to_string())),
        }
    }
}

impl TryFrom<Value> for ConfigBool {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self> {
        match value {
            Value::Bool(b) => Ok(ConfigBool(b)),
            Value::String(s) => Ok(ConfigBool(s.parse().unwrap_or(false))),
            Value::Number(n) => Ok(ConfigBool(n.as_i64().map(|i| i != 0).unwrap_or(false))),
            Value::Null => Ok(ConfigBool(false)),
            _ => Err(Error::TypeConversion("Cannot convert to bool".to_string())),
        }
    }
}

impl TryFrom<Value> for ConfigInt {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self> {
        match value {
            Value::Number(n) => Ok(ConfigInt(n.as_i64().unwrap_or(0) as i32)),
            Value::String(s) => Ok(ConfigInt(s.parse().unwrap_or(0))),
            Value::Bool(b) => Ok(ConfigInt(if b { 1 } else { 0 })),
            Value::Null => Ok(ConfigInt(0)),
            _ => Err(Error::TypeConversion("Cannot convert to int".to_string())),
        }
    }
}

impl TryFrom<Value> for ConfigInt64 {
    type Error = serde_json::Error;

    fn try_from(value: Value) -> std::result::Result<Self, Self::Error> {
        match value {
            Value::Number(n) => Ok(ConfigInt64(n.as_i64().unwrap_or(0))),
            Value::String(s) => Ok(ConfigInt64(s.parse().unwrap_or(0))),
            Value::Bool(b) => Ok(ConfigInt64(if b { 1 } else { 0 })),
            Value::Null => Ok(ConfigInt64(0)),
            _ => serde_json::from_value(value).map(ConfigInt64),
        }
    }
}

impl TryFrom<Value> for ConfigFloat {
    type Error = serde_json::Error;

    fn try_from(value: Value) -> std::result::Result<Self, Self::Error> {
        match value {
            Value::Number(n) => Ok(ConfigFloat(n.as_f64().unwrap_or(0.0))),
            Value::String(s) => Ok(ConfigFloat(s.parse().unwrap_or(0.0))),
            Value::Bool(b) => Ok(ConfigFloat(if b { 1.0 } else { 0.0 })),
            Value::Null => Ok(ConfigFloat(0.0)),
            _ => serde_json::from_value(value).map(ConfigFloat),
        }
    }
}

impl TryFrom<Value> for ConfigArray {
    type Error = serde_json::Error;

    fn try_from(value: Value) -> std::result::Result<Self, Self::Error> {
        match value {
            Value::Array(arr) => Ok(ConfigArray(arr)),
            _ => Ok(ConfigArray(vec![value])),
        }
    }
}

impl From<ConfigString> for String {
    fn from(s: ConfigString) -> Self {
        s.0
    }
}

impl From<ConfigBool> for bool {
    fn from(b: ConfigBool) -> Self {
        b.0
    }
}

impl From<ConfigInt> for i32 {
    fn from(i: ConfigInt) -> Self {
        i.0
    }
}

impl From<ConfigInt64> for i64 {
    fn from(i: ConfigInt64) -> Self {
        i.0
    }
}

impl From<ConfigFloat> for f64 {
    fn from(f: ConfigFloat) -> Self {
        f.0
    }
}

impl From<ConfigArray> for Vec<Value> {
    fn from(a: ConfigArray) -> Self {
        a.0
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Custom(s.to_string())
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
    #[error("Custom error: {0}")]
    Custom(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A configuration repository that provides access to configuration values
pub struct Repository {
    store: HashMap<String, Value>,
}

impl Repository {
    /// Create a new configuration repository
    pub fn new(initial: HashMap<String, Value>) -> Self {
        Self { store: initial }
    }

    /// Check if a configuration key exists
    pub async fn has(&self, key: impl Into<String>) -> bool {
        self.store.contains_key(&key.into())
    }

    /// Get a typed configuration value
    pub async fn get<T>(&self, key: impl Into<String>) -> Option<T>
    where
        T: TryFrom<Value> + Clone,
        T::Error: std::fmt::Debug,
    {
        self.store
            .get(&key.into())
            .and_then(|v| T::try_from(v.clone()).ok())
    }

    /// Get multiple configuration values
    pub async fn get_many<T>(&self, keys: Vec<String>) -> HashMap<String, T>
    where
        T: TryFrom<Value> + Clone,
        T::Error: std::fmt::Debug,
    {
        let mut result = HashMap::new();
        for key in keys {
            if let Some(value) = self.get(&key).await {
                result.insert(key, value);
            }
        }
        result
    }

    /// Set a configuration value
    pub async fn set(&mut self, key: impl Into<String>, value: impl Into<Value>) -> Result<()> {
        self.store.insert(key.into(), value.into());
        Ok(())
    }

    /// Get all configuration values
    pub async fn all(&self) -> HashMap<String, Value> {
        self.store.clone()
    }

    /// Prepend a value to an array configuration
    pub async fn prepend(&self, key: &str, value: impl Serialize) -> Result<()> {
        let mut items = self.store.clone();
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
        let mut items = self.store.clone();
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
        self.get::<ConfigString>(key).await
            .map(String::from)
            .ok_or_else(|| Error::InvalidKey(key.to_string()))
    }

    /// Get an array configuration value
    pub async fn array(&self, key: &str) -> Result<Vec<Value>> {
        self.get::<ConfigArray>(key).await
            .map(Vec::<Value>::from)
            .ok_or_else(|| Error::InvalidKey(key.to_string()))
    }

    /// Get a boolean configuration value
    pub async fn boolean(&self, key: &str) -> Result<bool> {
        self.get::<ConfigBool>(key).await
            .map(bool::from)
            .ok_or_else(|| Error::InvalidKey(key.to_string()))
    }

    /// Get an integer configuration value
    pub async fn integer(&self, key: &str) -> Result<i32> {
        self.get::<ConfigInt>(key).await
            .map(i32::from)
            .ok_or_else(|| Error::InvalidKey(key.to_string()))
    }

    /// Get a float configuration value
    pub async fn float(&self, key: &str) -> Result<f64> {
        self.get::<ConfigFloat>(key).await
            .map(f64::from)
            .ok_or_else(|| Error::InvalidKey(key.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_operations() {
        let mut initial = HashMap::new();
        initial.insert("app.name".to_string(), Value::String("MyApp".to_string()));
        let mut config = Repository::new(initial);

        // Test get
        let value: Option<ConfigString> = config.get("app.name").await;
        assert_eq!(String::from(value.unwrap()), "MyApp");

        // Test has
        assert!(config.has("app.name").await);
        assert!(!config.has("nonexistent").await);

        // Test get nonexistent
        let value: Option<ConfigString> = config.get("nonexistent").await;
        assert_eq!(value, None);

        // Test set
        config.set("new.key", "new value").await.expect("Failed to set value");
        let value: Option<ConfigString> = config.get("new.key").await;
        assert_eq!(String::from(value.unwrap()), "new value");
    }
}
