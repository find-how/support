use std::collections::HashMap;
use std::ops::{Index, IndexMut};
use serde::ser::Serialize;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Repository {
    store: HashMap<String, Value>,
}

impl Repository {
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
        }
    }

    pub fn has(&self, key: impl Into<ConfigKey>) -> bool {
        let key = key.into();
        self.store.contains_key(&key.to_string())
    }

    pub fn get<T>(&self, key: impl Into<ConfigKey>) -> Option<T>
    where
        T: FromValue,
    {
        let key = key.into();
        self.store
            .get(&key.to_string())
            .cloned()
            .and_then(|v| T::from_value(v))
    }

    pub fn set<T>(&mut self, key: impl Into<ConfigKey>, value: T)
    where
        T: Serialize,
    {
        let key = key.into();
        let value = serde_json::to_value(value).unwrap();
        self.store.insert(key.to_string(), value);
    }

    pub fn get_many<T>(&self, keys: impl Into<Vec<String>>) -> HashMap<String, T>
    where
        T: FromValue + Default,
    {
        let keys = keys.into();
        let mut result = HashMap::new();
        for key in keys {
            if let Some(value) = self.get::<T>(&key) {
                result.insert(key, value);
            } else {
                result.insert(key, T::default());
            }
        }
        result
    }

    pub fn set_many<T>(&mut self, values: HashMap<String, T>)
    where
        T: Serialize,
    {
        for (key, value) in values {
            self.set(key, value);
        }
    }

    pub fn forget(&mut self, key: impl Into<ConfigKey>) {
        let key = key.into();
        self.store.remove(&key.to_string());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfigKey(String);

impl ConfigKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl<T: Into<String>> From<T> for ConfigKey {
    fn from(key: T) -> Self {
        Self::new(key)
    }
}

impl Index<&str> for Repository {
    type Output = Value;

    fn index(&self, key: &str) -> &Self::Output {
        &self.store[key]
    }
}

impl IndexMut<&str> for Repository {
    fn index_mut(&mut self, key: &str) -> &mut Self::Output {
        self.store.get_mut(key).unwrap()
    }
}

pub trait FromValue: Sized {
    fn from_value(value: Value) -> Option<Self>;
}

impl FromValue for String {
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
}

impl FromValue for bool {
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Bool(b) => Some(b),
            _ => None,
        }
    }
}

impl FromValue for i32 {
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Number(n) => n.as_i64().map(|x| x as i32),
            _ => None,
        }
    }
}

impl FromValue for i64 {
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Number(n) => n.as_i64(),
            _ => None,
        }
    }
}

impl FromValue for f64 {
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Number(n) => n.as_f64(),
            _ => None,
        }
    }
}

impl FromValue for Value {
    fn from_value(value: Value) -> Option<Self> {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_has() {
        let mut repo = Repository::new();
        repo.set("app.name", "MyApp");
        assert!(repo.has("app.name"));
        assert!(!repo.has("nonexistent"));
    }

    #[test]
    fn test_get() {
        let mut repo = Repository::new();
        repo.set("app.name", "MyApp");
        repo.set("app.debug", true);
        repo.set("database.port", 5432);

        assert_eq!(repo.get::<String>("app.name"), Some("MyApp".to_string()));
        assert_eq!(repo.get::<bool>("app.debug"), Some(true));
        assert_eq!(repo.get::<i32>("database.port"), Some(5432));
        assert_eq!(repo.get::<String>("nonexistent"), None);
    }

    #[test]
    fn test_set() {
        let mut repo = Repository::new();
        repo.set("key", "value");
        assert_eq!(repo.get::<String>("key"), Some("value".to_string()));
    }

    #[test]
    fn test_get_many() {
        let mut repo = Repository::new();
        repo.set("app.name", "MyApp");
        repo.set("database.host", "localhost");

        let values = repo.get_many::<String>(vec!["app.name".to_string(), "database.host".to_string()]);
        assert_eq!(values.get("app.name"), Some(&"MyApp".to_string()));
        assert_eq!(values.get("database.host"), Some(&"localhost".to_string()));
    }

    #[test]
    fn test_set_many() {
        let mut repo = Repository::new();
        let mut values = HashMap::new();
        values.insert("key1".to_string(), "value1");
        values.insert("key2".to_string(), "value2");
        repo.set_many(values);
        assert_eq!(repo.get::<String>("key1"), Some("value1".to_string()));
        assert_eq!(repo.get::<String>("key2"), Some("value2".to_string()));
    }

    #[test]
    fn test_forget() {
        let mut repo = Repository::new();
        repo.set("app.name", "MyApp");
        repo.forget("app.name");
        assert!(!repo.has("app.name"));
    }

    #[test]
    fn test_index_access() {
        let mut repo = Repository::new();
        repo.set("app.name", "MyApp");
        assert_eq!(repo["app.name"], json!("MyApp"));
    }

    #[test]
    fn test_index_mut_access() {
        let mut repo = Repository::new();
        repo.set("app.name", "MyApp");
        repo["app.name"] = json!("NewApp");
        assert_eq!(repo.get::<String>("app.name"), Some("NewApp".to_string()));
    }

    #[test]
    fn test_type_conversions() {
        let mut repo = Repository::new();
        repo.set("string", "test");
        repo.set("bool", true);
        repo.set("int", 42);
        repo.set("float", 3.14);

        assert_eq!(repo.get::<String>("string"), Some("test".to_string()));
        assert_eq!(repo.get::<bool>("bool"), Some(true));
        assert_eq!(repo.get::<i32>("int"), Some(42));
        assert_eq!(repo.get::<f64>("float"), Some(3.14));

        // Test type mismatches
        assert_eq!(repo.get::<bool>("string"), None);
        assert_eq!(repo.get::<i32>("bool"), None);
        assert_eq!(repo.get::<String>("int"), None);
    }

    #[test]
    fn test_nested_values() {
        let mut repo = Repository::new();
        repo.set("a.b", "c");
        assert_eq!(repo.get::<String>("a.b"), Some("c".to_string()));
        assert_eq!(repo.get::<String>("a.b.c"), None);
        assert_eq!(repo.get::<String>("x.y.z"), None);
        assert_eq!(repo.get::<String>("."), None);
    }

    #[test]
    fn test_boolean_values() {
        let mut repo = Repository::new();
        repo.set("boolean", true);
        assert_eq!(repo.get::<bool>("boolean"), Some(true));
    }

    #[test]
    fn test_simple_values() {
        let mut repo = Repository::new();
        repo.set("foo", "bar");
        assert_eq!(repo.get::<String>("foo"), Some("bar".to_string()));
    }

    #[test]
    fn test_nested_structure() {
        let mut repo = Repository::new();
        repo.set("nested.key1", "value1");
        repo.set("nested.key2.inner", "value2");

        assert_eq!(repo.get::<String>("nested.key1"), Some("value1".to_string()));
        assert_eq!(repo.get::<String>("nested.key2.inner"), Some("value2".to_string()));
    }
}
