//! Utility functions for storage operations

use std::path::Path;
use bytes::Bytes;
use serde::{de::DeserializeOwned, Serialize};

use crate::{Error, Result};

/// Convert a byte slice to a string key representation
pub fn key_to_string(key: impl AsRef<[u8]>) -> String {
    hex::encode(key.as_ref())
}

/// Convert a string to bytes
pub fn string_to_bytes(s: impl AsRef<str>) -> Bytes {
    Bytes::copy_from_slice(s.as_ref().as_bytes())
}

/// Serialize a value to bytes
pub fn serialize<T: Serialize>(value: &T) -> Result<Bytes> {
    let bytes = serde_json::to_vec(value).map_err(Error::Serialization)?;
    Ok(Bytes::from(bytes))
}

/// Deserialize bytes to a value
pub fn deserialize<T: DeserializeOwned>(bytes: impl AsRef<[u8]>) -> Result<T> {
    serde_json::from_slice(bytes.as_ref()).map_err(Error::Serialization)
}

/// Ensure a path exists, creating parent directories if needed
pub fn ensure_path_exists(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::Io)?;
    }
    Ok(())
}

/// Join path components safely
pub fn join_paths(base: impl AsRef<Path>, components: &[impl AsRef<Path>]) -> Result<std::path::PathBuf> {
    let mut path = base.as_ref().to_path_buf();
    for component in components {
        let component = component.as_ref();
        if component.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(Error::InvalidPath(path));
        }
        path.push(component);
    }
    Ok(path)
}

/// Create a temporary path for atomic writes
pub fn temp_path(path: impl AsRef<Path>) -> std::path::PathBuf {
    let path = path.as_ref();
    let mut temp = path.to_path_buf();
    temp.set_extension("tmp");
    temp
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Test {
        field: String,
    }

    #[test]
    fn test_key_to_string() {
        assert_eq!(key_to_string(b"test"), "74657374");
        assert_eq!(key_to_string(vec![0, 1, 2]), "000102");
    }

    #[test]
    fn test_string_to_bytes() {
        assert_eq!(string_to_bytes("test"), Bytes::from("test"));
    }

    #[test]
    fn test_serialize_deserialize() {
        let test = Test {
            field: "test".to_string(),
        };
        let bytes = serialize(&test).unwrap();
        let deserialized: Test = deserialize(&bytes).unwrap();
        assert_eq!(test, deserialized);
    }

    #[test]
    fn test_ensure_path_exists() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test").join("path");
        ensure_path_exists(&path).unwrap();
        assert!(path.parent().unwrap().exists());
    }

    #[test]
    fn test_join_paths() {
        let base = PathBuf::from("/base");
        let components = &["sub", "path"];
        let joined = join_paths(&base, components).unwrap();
        assert_eq!(joined, PathBuf::from("/base/sub/path"));

        // Test parent directory traversal prevention
        let components = &["sub", "..", "path"];
        assert!(join_paths(&base, components).is_err());
    }

    #[test]
    fn test_temp_path() {
        let path = PathBuf::from("test.db");
        let temp = temp_path(&path);
        assert_eq!(temp, PathBuf::from("test.db.tmp"));
    }
}
