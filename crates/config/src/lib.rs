//! Configuration management with Laravel-inspired patterns
//! This crate provides a configuration system similar to Laravel's Config facade.

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Key not found: {0}")]
    NotFound(String),
}

/// Configuration interface
#[async_trait]
pub trait Config: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Get a configuration value by key
    async fn get(&self, key: &str) -> Result<Option<String>, Self::Error>;

    /// Set a configuration value
    async fn set(&self, key: &str, value: &str) -> Result<(), Self::Error>;
}
