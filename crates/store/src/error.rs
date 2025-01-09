//! Error types for the store crate.
//!
//! This module provides a comprehensive error type system for store operations,
//! including backend-specific error conversion traits.

use std::path::PathBuf;
use std::io;
use thiserror::Error;

/// Result type for store operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for store operations
#[derive(Debug, Error)]
pub enum Error {
    /// Backend-specific error
    #[error("Backend error: {0}")]
    Backend(#[from] sled::Error),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Invalid path
    #[error("Invalid path: {0}")]
    InvalidPath(PathBuf),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Store is in maintenance mode
    #[error("Store is in maintenance mode")]
    MaintenanceMode,

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
}

/// Helper methods for creating errors
impl Error {
    /// Create a new backend error with a custom message
    pub fn backend(msg: impl Into<String>) -> Self {
        Self::Backend(sled::Error::from(msg))
    }

    /// Create a new key not found error with hex-encoded key
    pub fn key_not_found(key: impl AsRef<[u8]>) -> Self {
        Self::InvalidPath(PathBuf::from(hex::encode(key.as_ref())))
    }

    /// Create a new invalid key error with custom message
    pub fn invalid_key(msg: impl Into<String>) -> Self {
        Self::InvalidPath(PathBuf::from(msg))
    }

    /// Create a new invalid value error with custom message
    pub fn invalid_value(msg: impl Into<String>) -> Self {
        Self::InvalidPath(PathBuf::from(msg))
    }

    /// Create a new transaction error with custom message
    pub fn transaction(msg: impl Into<String>) -> Self {
        Self::InvalidPath(PathBuf::from(msg))
    }

    /// Create a new configuration error with custom message
    pub fn configuration<T: ToString>(msg: T) -> Self {
        Error::Configuration(msg.to_string())
    }

    /// Create a new not supported error with custom message
    pub fn not_supported(msg: impl Into<String>) -> Self {
        Self::InvalidPath(PathBuf::from(msg))
    }

    /// Create an invalid configuration error with custom message
    pub fn invalid_configuration<T: ToString>(msg: T) -> Self {
        Error::InvalidConfiguration(msg.to_string())
    }
}

/// Helper trait for converting backend-specific errors to store errors
pub trait IntoStoreError {
    /// Convert to a store error
    fn into_store_error(self) -> Error;
}

#[cfg(feature = "sled-store")]
impl IntoStoreError for sled::Error {
    fn into_store_error(self) -> Error {
        Error::backend(self.to_string())
    }
}

#[cfg(feature = "sqlite-store")]
impl IntoStoreError for rusqlite::Error {
    fn into_store_error(self) -> Error {
        Error::backend(self.to_string())
    }
}
