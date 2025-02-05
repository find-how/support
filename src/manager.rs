use crate::{
    local::LocalStorage, s3::S3Storage, StorageConfig, StorageError, StorageOps, StorageWriter,
};
use aws_sdk_s3::Client;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

pub struct StorageManager {
    disks: RwLock<HashMap<String, Arc<dyn StorageOps + Send + Sync>>>,
    writers: RwLock<HashMap<String, Arc<dyn StorageWriter + Send + Sync>>>,
}

impl StorageManager {
    pub fn new() -> Self {
        Self {
            disks: RwLock::new(HashMap::new()),
            writers: RwLock::new(HashMap::new()),
        }
    }

    pub async fn add_disk(&self, name: &str, config: StorageConfig) -> Result<(), StorageError> {
        let disk: Arc<dyn StorageOps + Send + Sync> = match config.driver.as_str() {
            "local" => {
                let storage = LocalStorage::new(
                    config.root,
                    config.url,
                    config.visibility,
                )?;
                Arc::new(storage)
            }
            "s3" => {
                let s3_config = config.s3.ok_or_else(|| {
                    StorageError::InvalidDriver("S3 configuration is required".to_string())
                })?;

                let aws_config = aws_sdk_s3::Config::builder()
                    .region(aws_sdk_s3::config::Region::new(s3_config.region))
                    .force_path_style(true);

                let aws_config = if let Some(endpoint) = s3_config.endpoint {
                    aws_config.endpoint_url(endpoint)
                } else {
                    aws_config
                };

                let aws_config = aws_config.build();
                let client = Client::from_conf(aws_config);

                let storage = S3Storage::new(client, s3_config.bucket, config.root);
                Arc::new(storage)
            }
            _ => return Err(StorageError::InvalidDriver(config.driver)),
        };

        let writer = disk.clone() as Arc<dyn StorageWriter + Send + Sync>;

        self.disks.write().await.insert(name.to_string(), disk);
        self.writers.write().await.insert(name.to_string(), writer);

        Ok(())
    }

    pub async fn disk(
        &self,
        name: Option<&str>,
    ) -> Result<Arc<dyn StorageOps + Send + Sync>, StorageError> {
        let name = name.unwrap_or("local");
        let disks = self.disks.read().await;
        disks
            .get(name)
            .cloned()
            .ok_or_else(|| StorageError::DiskNotFound(name.to_string()))
    }

    pub async fn disk_writer(
        &self,
        name: Option<&str>,
    ) -> Result<Arc<dyn StorageWriter + Send + Sync>, StorageError> {
        let name = name.unwrap_or("local");
        let writers = self.writers.read().await;
        writers
            .get(name)
            .cloned()
            .ok_or_else(|| StorageError::DiskNotFound(name.to_string()))
    }
}

impl Default for StorageManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_storage_manager() {
        let manager = StorageManager::new();
        let temp_dir = tempdir().unwrap();

        // Test local storage
        let config = StorageConfig {
            driver: "local".to_string(),
            root: temp_dir.path().to_string_lossy().to_string(),
            url: "http://localhost".to_string(),
            visibility: crate::Visibility::Public,
            options: None,
            s3: None,
        };

        manager.add_disk("local", config).await.unwrap();

        let disk = manager.disk(Some("local")).await.unwrap();
        let writer = manager.disk_writer(Some("local")).await.unwrap();

        writer.put("test.txt", "Hello, World!", None).await.unwrap();
        assert!(disk.exists("test.txt").await.unwrap());
        assert_eq!(
            disk.get("test.txt").await.unwrap(),
            bytes::Bytes::from("Hello, World!")
        );

        // Test invalid driver
        let config = StorageConfig {
            driver: "invalid".to_string(),
            root: "".to_string(),
            url: "".to_string(),
            visibility: crate::Visibility::Private,
            options: None,
            s3: None,
        };

        assert!(manager.add_disk("invalid", config).await.is_err());

        // Test non-existent disk
        assert!(manager.disk(Some("non-existent")).await.is_err());
    }
}
