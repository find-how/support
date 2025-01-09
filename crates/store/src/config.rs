use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ops::{Index, IndexMut};

/// Repository for configuration values
#[derive(Debug, Clone)]
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
        if key.contains('.') {
            self.get(key).await.is_some()
        } else {
            items.contains_key(key)
        }
    }

    /// Get a configuration value
    pub async fn get<T>(&self, key: impl Into<ConfigKey>) -> Option<T>
    where
        T: TryFrom<Value> + Clone,
    {
        let key = key.into();
        let items = self.items.read().await;

        match key {
            ConfigKey::Single(key) => {
                let parts: Vec<&str> = key.split('.').collect();
                let mut current = items.get(&parts[0].to_string())?.clone();

                for part in parts.iter().skip(1) {
                    current = current.get(part)?.clone();
                }

                T::try_from(current).ok()
            }
            ConfigKey::Multiple(keys) => {
                let mut result = HashMap::new();
                for key in keys {
                    match key {
                        ConfigKeyWithDefault::Key(k) => {
                            result.insert(k.clone(), self.get::<T>(&k).await);
                        }
                        ConfigKeyWithDefault::KeyWithDefault(k, default) => {
                            result.insert(k.clone(), self.get::<T>(&k).await.unwrap_or(default));
                        }
                    }
                }
                T::try_from(Value::Object(result.into())).ok()
            }
        }
    }

    /// Get multiple configuration values
    pub async fn get_many<T>(&self, keys: impl Into<Vec<ConfigKeyWithDefault>>) -> HashMap<String, T>
    where
        T: TryFrom<Value> + Clone,
    {
        let keys = keys.into();
        let mut result = HashMap::new();

        for key in keys {
            match key {
                ConfigKeyWithDefault::Key(k) => {
                    result.insert(k.clone(), self.get::<T>(&k).await.unwrap_or_default());
                }
                ConfigKeyWithDefault::KeyWithDefault(k, default) => {
                    result.insert(k.clone(), self.get::<T>(&k).await.unwrap_or(default));
                }
            }
        }

        result
    }

    /// Set a configuration value
    pub async fn set(&self, key: impl Into<ConfigKey>, value: impl Serialize) -> crate::Result<()> {
        let key = key.into();
        let mut items = self.items.write().await;

        match key {
            ConfigKey::Single(key) => {
                let parts: Vec<&str> = key.split('.').collect();
                if parts.len() == 1 {
                    items.insert(key, serde_json::to_value(value)?);
                } else {
                    let mut current = items.entry(parts[0].to_string())
                        .or_insert(Value::Object(serde_json::Map::new()));

                    for part in parts[1..parts.len()-1].iter() {
                        current = current.as_object_mut()
                            .ok_or_else(|| crate::Error::configuration("Invalid nested path"))?
                            .entry(part.to_string())
                            .or_insert(Value::Object(serde_json::Map::new()));
                    }

                    current.as_object_mut()
                        .ok_or_else(|| crate::Error::configuration("Invalid nested path"))?
                        .insert(parts.last().unwrap().to_string(), serde_json::to_value(value)?);
                }
            }
            ConfigKey::Multiple(items_to_set) => {
                for item in items_to_set {
                    match item {
                        ConfigKeyWithDefault::Key(k) => self.set(k, Value::Null).await?,
                        ConfigKeyWithDefault::KeyWithDefault(k, v) => self.set(k, v).await?,
                    }
                }
            }
        }

        Ok(())
    }

    /// Get all configuration values
    pub async fn all(&self) -> HashMap<String, Value> {
        self.items.read().await.clone()
    }

    /// Prepend a value to an array configuration value
    pub async fn prepend(&self, key: &str, value: impl Serialize) -> crate::Result<()> {
        let mut items = self.items.write().await;
        let array = items.entry(key.to_string())
            .or_insert(Value::Array(vec![]));

        if let Value::Array(vec) = array {
            vec.insert(0, serde_json::to_value(value)?);
        } else {
            *array = Value::Array(vec![serde_json::to_value(value)?]);
        }

        Ok(())
    }

    /// Push a value to an array configuration value
    pub async fn push(&self, key: &str, value: impl Serialize) -> crate::Result<()> {
        let mut items = self.items.write().await;
        let array = items.entry(key.to_string())
            .or_insert(Value::Array(vec![]));

        if let Value::Array(vec) = array {
            vec.push(serde_json::to_value(value)?);
        } else {
            *array = Value::Array(vec![serde_json::to_value(value)?]);
        }

        Ok(())
    }

    /// Get a string configuration value
    pub async fn string(&self, key: &str) -> crate::Result<String> {
        let value = self.get::<Value>(key).await
            .ok_or_else(|| crate::Error::configuration(format!("Key not found: {}", key)))?;

        match value {
            Value::String(s) => Ok(s),
            _ => Err(crate::Error::invalid_configuration(
                format!("Configuration value for key [{}] must be a string, {:?} given", key, value)
            )),
        }
    }

    /// Get an array configuration value
    pub async fn array(&self, key: &str) -> crate::Result<Vec<Value>> {
        let value = self.get::<Value>(key).await
            .ok_or_else(|| crate::Error::configuration(format!("Key not found: {}", key)))?;

        match value {
            Value::Array(arr) => Ok(arr),
            _ => Err(crate::Error::invalid_configuration(
                format!("Configuration value for key [{}] must be an array, {:?} given", key, value)
            )),
        }
    }

    /// Get a boolean configuration value
    pub async fn boolean(&self, key: &str) -> crate::Result<bool> {
        let value = self.get::<Value>(key).await
            .ok_or_else(|| crate::Error::configuration(format!("Key not found: {}", key)))?;

        match value {
            Value::Bool(b) => Ok(b),
            _ => Err(crate::Error::invalid_configuration(
                format!("Configuration value for key [{}] must be a boolean, {:?} given", key, value)
            )),
        }
    }

    /// Get an integer configuration value
    pub async fn integer(&self, key: &str) -> crate::Result<i64> {
        let value = self.get::<Value>(key).await
            .ok_or_else(|| crate::Error::configuration(format!("Key not found: {}", key)))?;

        match value {
            Value::Number(n) if n.is_i64() => Ok(n.as_i64().unwrap()),
            _ => Err(crate::Error::invalid_configuration(
                format!("Configuration value for key [{}] must be an integer, {:?} given", key, value)
            )),
        }
    }

    /// Get a float configuration value
    pub async fn float(&self, key: &str) -> crate::Result<f64> {
        let value = self.get::<Value>(key).await
            .ok_or_else(|| crate::Error::configuration(format!("Key not found: {}", key)))?;

        match value {
            Value::Number(n) if n.is_f64() => Ok(n.as_f64().unwrap()),
            _ => Err(crate::Error::invalid_configuration(
                format!("Configuration value for key [{}] must be a float, {:?} given", key, value)
            )),
        }
    }
}

impl Index<&str> for Repository {
    type Output = Value;

    fn index(&self, key: &str) -> &Self::Output {
        todo!("Implement sync indexing")
    }
}

impl IndexMut<&str> for Repository {
    fn index_mut(&mut self, key: &str) -> &mut Self::Output {
        todo!("Implement sync indexing")
    }
}

#[derive(Debug, Clone)]
enum ConfigKey {
    Single(String),
    Multiple(Vec<ConfigKeyWithDefault>),
}

impl<T: Into<String>> From<T> for ConfigKey {
    fn from(key: T) -> Self {
        ConfigKey::Single(key.into())
    }
}

impl From<Vec<&str>> for ConfigKey {
    fn from(keys: Vec<&str>) -> Self {
        ConfigKey::Multiple(keys.into_iter()
            .map(|k| ConfigKeyWithDefault::Key(k.to_string()))
            .collect())
    }
}

#[derive(Debug, Clone)]
enum ConfigKeyWithDefault {
    Key(String),
    KeyWithDefault(String, Value),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn setup_repository() -> Repository {
        let config = HashMap::from([
            ("foo".to_string(), json!("bar")),
            ("bar".to_string(), json!("baz")),
            ("baz".to_string(), json!("bat")),
            ("null".to_string(), json!(null)),
            ("boolean".to_string(), json!(true)),
            ("integer".to_string(), json!(1)),
            ("float".to_string(), json!(1.1)),
            ("associate".to_string(), json!({
                "x": "xxx",
                "y": "yyy"
            })),
            ("array".to_string(), json!(["aaa", "zzz"])),
            ("x".to_string(), json!({
                "z": "zoo"
            })),
            ("a.b".to_string(), json!("c")),
            ("a".to_string(), json!({
                "b.c": "d"
            })),
        ]);

        Repository::new(config)
    }

    #[tokio::test]
    async fn test_get_value_when_key_contain_dot() {
        let repository = setup_repository().await;
        assert_eq!(repository.get::<String>("a.b").await.unwrap(), "c");
        assert!(repository.get::<String>("a.b.c").await.is_none());
        assert!(repository.get::<String>("x.y.z").await.is_none());
        assert!(repository.get::<String>(".").await.is_none());
    }

    #[tokio::test]
    async fn test_get_boolean_value() {
        let repository = setup_repository().await;
        assert!(repository.get::<bool>("boolean").await.unwrap());
    }

    #[tokio::test]
    async fn test_get_null_value() {
        let repository = setup_repository().await;
        assert!(repository.get::<Value>("null").await.unwrap().is_null());
    }

    #[tokio::test]
    async fn test_has_is_true() {
        let repository = setup_repository().await;
        assert!(repository.has("foo").await);
    }

    #[tokio::test]
    async fn test_has_is_false() {
        let repository = setup_repository().await;
        assert!(!repository.has("not-exist").await);
    }

    #[tokio::test]
    async fn test_get() {
        let repository = setup_repository().await;
        assert_eq!(repository.get::<String>("foo").await.unwrap(), "bar");
    }

    #[tokio::test]
    async fn test_get_with_array_of_keys() {
        let repository = setup_repository().await;
        let result = repository.get_many::<Value>(vec![
            ConfigKeyWithDefault::Key("foo".to_string()),
            ConfigKeyWithDefault::Key("bar".to_string()),
            ConfigKeyWithDefault::Key("none".to_string()),
        ]).await;

        assert_eq!(result.get("foo").unwrap(), &json!("bar"));
        assert_eq!(result.get("bar").unwrap(), &json!("baz"));
        assert!(result.get("none").unwrap().is_null());
    }

    #[tokio::test]
    async fn test_set() {
        let repository = setup_repository().await;
        repository.set("key", "value").await.unwrap();
        assert_eq!(repository.get::<String>("key").await.unwrap(), "value");
    }

    #[tokio::test]
    async fn test_set_array() {
        let repository = setup_repository().await;
        repository.set("nested", json!({
            "key1": "value1",
            "key2": {
                "inner": "value2"
            }
        })).await.unwrap();

        assert_eq!(repository.get::<String>("nested.key1").await.unwrap(), "value1");
        assert_eq!(repository.get::<String>("nested.key2.inner").await.unwrap(), "value2");
    }

    #[tokio::test]
    async fn test_array_operations() {
        let repository = setup_repository().await;

        // Test initial state
        let array = repository.array("array").await.unwrap();
        assert_eq!(array[0], json!("aaa"));
        assert_eq!(array[1], json!("zzz"));

        // Test prepend
        repository.prepend("array", "xxx").await.unwrap();
        let array = repository.array("array").await.unwrap();
        assert_eq!(array[0], json!("xxx"));
        assert_eq!(array[1], json!("aaa"));
        assert_eq!(array[2], json!("zzz"));

        // Test push
        repository.push("array", "yyy").await.unwrap();
        let array = repository.array("array").await.unwrap();
        assert_eq!(array[0], json!("xxx"));
        assert_eq!(array[3], json!("yyy"));
    }

    #[tokio::test]
    async fn test_type_specific_getters() {
        let repository = setup_repository().await;

        assert_eq!(repository.string("foo").await.unwrap(), "bar");
        assert_eq!(repository.boolean("boolean").await.unwrap(), true);
        assert_eq!(repository.integer("integer").await.unwrap(), 1);
        assert_eq!(repository.float("float").await.unwrap(), 1.1);
        assert!(repository.array("array").await.unwrap().len() == 2);
    }
}
