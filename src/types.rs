use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageConfig {
    pub driver: String,
    pub root: Option<String>,
    pub url: Option<String>,
    pub visibility: Option<String>,
    pub options: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub last_modified: Option<u64>,
    pub mime_type: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListOptions {
    pub recursive: bool,
    pub include_directories: bool,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UrlOptions {
    pub expiry: Option<u64>,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileOptions {
    pub visibility: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
}

#[async_trait]
pub trait PutContent: Send + Sync {
    async fn to_bytes(&self) -> Result<Bytes, std::io::Error>;
}

#[async_trait]
impl PutContent for Bytes {
    async fn to_bytes(&self) -> Result<Bytes, std::io::Error> {
        Ok(self.clone())
    }
}

#[async_trait]
impl PutContent for String {
    async fn to_bytes(&self) -> Result<Bytes, std::io::Error> {
        Ok(Bytes::from(self.clone()))
    }
}

#[async_trait]
impl PutContent for &str {
    async fn to_bytes(&self) -> Result<Bytes, std::io::Error> {
        Ok(Bytes::from(self.to_string()))
    }
}

#[async_trait]
impl<T: Serialize + Send + Sync> PutContent for T
where
    T: 'static,
    Bytes: From<T>,
{
    async fn to_bytes(&self) -> Result<Bytes, std::io::Error> {
        Ok(Bytes::from(serde_json::to_vec(self)?))
    }
}
