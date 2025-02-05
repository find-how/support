use async_trait::async_trait;
use bytes::Bytes;
use http::HeaderMap;
use std::pin::Pin;
use tokio::io::AsyncRead;

use crate::error::StorageError;
use crate::types::{FileInfo, FileOptions, ListOptions, PutContent, UrlOptions, Visibility};

/// Core storage operations that are object-safe
#[async_trait]
pub trait StorageOps: Send + Sync {
    /// Check if a file exists at the given path
    async fn exists(&self, path: &str) -> Result<bool, StorageError>;

    /// Get metadata for a file at the given path
    async fn get(&self, path: &str) -> Result<Bytes, StorageError>;

    /// Delete a file at the given path
    async fn delete(&self, path: &str) -> Result<(), StorageError>;

    /// List files in storage with optional prefix and options
    async fn list(&self, prefix: Option<&str>, options: Option<&ListOptions>) -> Result<Vec<FileInfo>, StorageError>;

    /// Get a temporary URL for a file
    async fn temporary_url(&self, path: &str, options: Option<&UrlOptions>) -> Result<String, StorageError>;

    /// Get the visibility of a file
    async fn visibility(&self, path: &str) -> Result<Visibility, StorageError>;

    /// Set the visibility of a file
    async fn set_visibility(&self, path: &str, visibility: Visibility) -> Result<(), StorageError>;
}

/// Extended storage operations that include generic methods
#[async_trait]
pub trait StorageWriter: Send + Sync {
    /// Put content into storage at the given path
    async fn put<T: AsRef<[u8]> + Send + Sync>(
        &self,
        path: &str,
        content: T,
        options: Option<&FileOptions>,
    ) -> Result<(), StorageError>;

    /// Copy a file from one path to another
    async fn copy(
        &self,
        from: &str,
        to: &str,
        options: Option<&FileOptions>,
    ) -> Result<(), StorageError>;

    /// Move a file from one path to another
    async fn move_file(
        &self,
        from: &str,
        to: &str,
        options: Option<&FileOptions>,
    ) -> Result<(), StorageError>;

    /// Make a directory at the given path
    async fn make_directory(&self, path: &str) -> Result<(), StorageError>;

    /// Delete a directory at the given path
    async fn delete_directory(&self, path: &str) -> Result<(), StorageError>;
}

/// Legacy trait for backwards compatibility
pub trait StorageTrait: StorageOps + StorageWriter {}

// Implement StorageTrait for anything that implements both StorageOps and StorageWriter
#[async_trait::async_trait]
impl<T: StorageOps + StorageWriter + Send + Sync> StorageTrait for T {
    async fn exists(&self, path: &str) -> Result<bool, StorageError> {
        StorageOps::exists(self, path).await
    }

    async fn get(&self, path: &str) -> Result<FileInfo, StorageError> {
        StorageOps::get(self, path).await
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        StorageOps::delete(self, path).await
    }

    async fn list(&self, prefix: Option<&str>, options: Option<&ListOptions>) -> Result<Vec<FileInfo>, StorageError> {
        StorageOps::list(self, prefix, options).await
    }

    async fn temporary_url(&self, path: &str, options: Option<&UrlOptions>) -> Result<String, StorageError> {
        StorageOps::temporary_url(self, path, options).await
    }

    async fn put<U: AsRef<[u8]> + Send + Sync>(&self, path: &str, content: U, options: Option<&FileOptions>) -> Result<(), StorageError> {
        StorageWriter::put(self, path, content, options).await
    }

    async fn copy(&self, from: &str, to: &str, options: Option<&FileOptions>) -> Result<(), StorageError> {
        StorageWriter::copy(self, from, to, options).await
    }

    async fn move_file(&self, from: &str, to: &str, options: Option<&FileOptions>) -> Result<(), StorageError> {
        StorageWriter::move_file(self, from, to, options).await
    }

    async fn make_directory(&self, path: &str) -> Result<(), StorageError> {
        StorageWriter::make_directory(self, path).await
    }

    async fn delete_directory(&self, path: &str) -> Result<(), StorageError> {
        StorageWriter::delete_directory(self, path).await
    }
}
