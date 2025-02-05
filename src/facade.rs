use crate::{
    FileInfo, FileMetadata, FileOptions, ListOptions, StorageError, StorageManager, StorageOps,
    StorageWriter, UrlOptions, Visibility,
};
use bytes::Bytes;
use std::sync::Arc;

pub struct Storage {
    manager: Arc<StorageManager>,
}

impl Storage {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(StorageManager::new()),
        }
    }

    pub async fn add_disk(&self, name: &str, config: crate::StorageConfig) -> Result<(), StorageError> {
        self.manager.add_disk(name, config).await
    }

    pub async fn disk(&self, name: Option<&str>) -> Result<Arc<dyn StorageOps + Send + Sync>, StorageError> {
        self.manager.disk(name).await
    }

    pub async fn disk_writer(&self, name: Option<&str>) -> Result<Arc<dyn StorageWriter + Send + Sync>, StorageError> {
        self.manager.disk_writer(name).await
    }

    pub async fn default(&self) -> Result<Arc<dyn StorageOps + Send + Sync>, StorageError> {
        self.disk(None).await
    }

    pub async fn default_writer(&self) -> Result<Arc<dyn StorageWriter + Send + Sync>, StorageError> {
        self.disk_writer(None).await
    }

    pub async fn exists(&self, path: &str) -> Result<bool, StorageError> {
        self.default().await?.exists(path).await
    }

    pub async fn get(&self, path: &str) -> Result<Bytes, StorageError> {
        self.default().await?.get(path).await
    }

    pub async fn put<T: AsRef<[u8]> + Send + Sync>(
        &self,
        path: &str,
        content: T,
        options: Option<&FileOptions>,
    ) -> Result<(), StorageError> {
        self.default_writer().await?.put(path, content, options).await
    }

    pub async fn delete(&self, path: &str) -> Result<(), StorageError> {
        self.default().await?.delete(path).await
    }

    pub async fn list(
        &self,
        prefix: Option<&str>,
        options: Option<&ListOptions>,
    ) -> Result<Vec<FileInfo>, StorageError> {
        self.default().await?.list(prefix, options).await
    }

    pub async fn temporary_url(
        &self,
        path: &str,
        options: Option<&UrlOptions>,
    ) -> Result<String, StorageError> {
        self.default().await?.temporary_url(path, options).await
    }

    pub async fn copy(
        &self,
        from: &str,
        to: &str,
        options: Option<&FileOptions>,
    ) -> Result<(), StorageError> {
        self.default_writer().await?.copy(from, to, options).await
    }

    pub async fn move_file(
        &self,
        from: &str,
        to: &str,
        options: Option<&FileOptions>,
    ) -> Result<(), StorageError> {
        self.default_writer().await?.move_file(from, to, options).await
    }

    pub async fn make_directory(&self, path: &str) -> Result<(), StorageError> {
        self.default_writer().await?.make_directory(path).await
    }

    pub async fn delete_directory(&self, path: &str) -> Result<(), StorageError> {
        self.default_writer().await?.delete_directory(path).await
    }

    pub async fn visibility(&self, path: &str) -> Result<Visibility, StorageError> {
        self.default().await?.visibility(path).await
    }

    pub async fn set_visibility(&self, path: &str, visibility: Visibility) -> Result<(), StorageError> {
        self.default().await?.set_visibility(path, visibility).await
    }
}

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StorageConfig;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_local_storage() {
        let storage = Storage::new();
        let temp_dir = tempdir().unwrap();

        let config = StorageConfig {
            driver: "local".to_string(),
            root: temp_dir.path().to_string_lossy().to_string(),
            url: "http://localhost".to_string(),
            visibility: Visibility::Public,
            options: None,
            s3: None,
        };

        storage.add_disk("local", config).await.unwrap();

        // Test string content
        let disk = storage.disk(Some("local")).await.unwrap();
        let disk_writer = storage.disk_writer(Some("local")).await.unwrap();

        let mut options = FileOptions::default();
        options.content_type = Some("text/plain".to_string());
        disk_writer
            .put("string.txt", "Hello World", Some(&options))
            .await
            .unwrap();

        assert!(disk.exists("string.txt").await.unwrap());
        assert_eq!(
            disk.get("string.txt").await.unwrap(),
            Bytes::from("Hello World")
        );

        // Test bytes content
        let bytes = Bytes::from("Hello World");
        disk_writer.put("bytes.txt", bytes, None).await.unwrap();

        assert!(disk.exists("bytes.txt").await.unwrap());
        assert_eq!(
            disk.get("bytes.txt").await.unwrap(),
            Bytes::from("Hello World")
        );

        // Test JSON content
        let data = json!({
            "hello": "world"
        });
        disk_writer.put("data.json", &data, None).await.unwrap();

        assert!(disk.exists("data.json").await.unwrap());

        // Test list
        let files = disk.list(None, None).await.unwrap();
        assert_eq!(files.len(), 3);

        // Test delete
        disk.delete("string.txt").await.unwrap();
        assert!(!disk.exists("string.txt").await.unwrap());

        // Test invalid disk
        assert!(storage
            .add_disk(
                "invalid",
                StorageConfig {
                    driver: "invalid".to_string(),
                    root: "".to_string(),
                    s3: None,
                    url: "".to_string(),
                    visibility: Visibility::Private,
                    options: None,
                }
            )
            .await
            .is_err());
    }
}
