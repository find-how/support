use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Backend error: {0}")]
    Backend(#[from] sled::Error),

    #[error("Invalid path: {0}")]
    InvalidPath(PathBuf),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Custom error: {0}")]
    Custom(String),

    #[error("Key not found: {0:?}")]
    KeyNotFound(Vec<u8>),

    #[error("Invalid key: {0}")]
    InvalidKey(String),

    #[error("Invalid value: {0}")]
    InvalidValue(String),

    #[error("Batch operation failed: {0}")]
    BatchError(String),

    #[error("Store error: {0}")]
    Other(String),
}

impl Error {
    pub fn custom<T: ToString>(msg: T) -> Self {
        Self::Custom(msg.to_string())
    }

    pub fn invalid_path<P: AsRef<Path>>(path: P) -> Self {
        Self::InvalidPath(path.as_ref().to_path_buf())
    }
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Self::Custom(msg)
    }
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self {
        Self::Custom(msg.to_string())
    }
}
