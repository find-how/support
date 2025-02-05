use async_trait::async_trait;
use bytes::Bytes;
use http::HeaderMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt};
use std::time::SystemTime;
use std::fs;

use crate::error::StorageError;
use crate::traits::{StorageOps, StorageTrait, StorageWriter};
use crate::types::{FileInfo, FileOptions, ListOptions, PutContent, UrlOptions, Visibility};
use tokio::fs as async_fs;

pub struct LocalStorage {
    root: PathBuf,
    url: String,
    visibility: Visibility,
}

impl LocalStorage {
    pub fn new(root: impl Into<PathBuf>, url: String, visibility: Visibility) -> Result<Self, StorageError> {
        let root = root.into();
        if !root.exists() {
            fs::create_dir_all(&root).map_err(StorageError::Io)?;
        }
        Ok(Self { root, url, visibility })
    }

    fn ensure_within_root(&self, path: &Path) -> Result<PathBuf, StorageError> {
        let full_path = self.root.join(path);
        let canonical = fs::canonicalize(&full_path).map_err(StorageError::Io)?;
        if !canonical.starts_with(&self.root) {
            return Err(StorageError::InvalidPath(path.to_string_lossy().to_string()));
        }
        Ok(full_path)
    }

    fn path_to_file_info(&self, path: &Path) -> Result<FileInfo, StorageError> {
        let metadata = fs::metadata(path).map_err(StorageError::Io)?;
        let relative_path = path.strip_prefix(&self.root).unwrap();

        Ok(FileInfo {
            path: relative_path.to_string_lossy().to_string(),
            size: metadata.len(),
            last_modified: metadata
                .modified()
                .map_err(StorageError::Io)?
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            content_type: None,
            metadata: None,
            visibility: self.visibility.clone(),
        })
    }
}

#[async_trait]
impl StorageTrait for LocalStorage {
    async fn exists(&self, path: &str) -> Result<bool, StorageError> {
        let full_path = self.ensure_within_root(Path::new(path))?;
        Ok(full_path.exists())
    }

    async fn get(&self, path: &str) -> Result<Bytes, StorageError> {
        let full_path = self.ensure_within_root(Path::new(path))?;
        if !full_path.exists() {
            return Err(StorageError::NotFound(path.to_string()));
        }
        let content = async_fs::read(&full_path).await.map_err(StorageError::Io)?;
        Ok(Bytes::from(content))
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        let full_path = self.ensure_within_root(Path::new(path))?;
        if !full_path.exists() {
            return Err(StorageError::NotFound(path.to_string()));
        }
        async_fs::remove_file(&full_path).await.map_err(StorageError::Io)?;
        Ok(())
    }

    async fn list(&self, prefix: Option<&str>, _options: Option<&ListOptions>) -> Result<Vec<FileInfo>, StorageError> {
        let search_path = match prefix {
            Some(prefix) => self.ensure_within_root(Path::new(prefix))?,
            None => self.root.clone(),
        };

        let mut files = Vec::new();
        let read_dir = async_fs::read_dir(&search_path).await.map_err(StorageError::Io)?;
        let mut entries = read_dir;
        while let Some(entry) = entries.next_entry().await.map_err(StorageError::Io)? {
            let path = entry.path();
            if path.is_file() {
                files.push(self.path_to_file_info(&path)?);
            }
        }
        Ok(files)
    }

    async fn stream(&self, path: &str) -> Result<Pin<Box<dyn AsyncRead + Send>>, StorageError> {
        let path = self.ensure_within_root(Path::new(path))?;
        match File::open(&path).await {
            Ok(file) => Ok(Box::pin(file)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(StorageError::NotFound(path.to_string_lossy().into_owned()))
            }
            Err(e) => Err(StorageError::Io(e)),
        }
    }

    async fn response(&self, path: &str) -> Result<(Pin<Box<dyn AsyncRead + Send>>, HeaderMap), StorageError> {
        let stream = self.stream(path).await?;
        let headers = HeaderMap::new();
        Ok((stream, headers))
    }
}

#[async_trait]
impl StorageOps for LocalStorage {
    async fn put<C: PutContent>(&self, path: &str, content: C, _options: Option<FileOptions>) -> Result<(), StorageError> {
        let full_path = self.ensure_within_root(Path::new(path))?;
        if let Some(parent) = full_path.parent() {
            async_fs::create_dir_all(parent).await.map_err(StorageError::Io)?;
        }
        let bytes = content.to_bytes().await.map_err(|e| StorageError::Io(e))?;
        async_fs::write(&full_path, bytes).await.map_err(|e| StorageError::Io(e))
    }

    async fn put_file<C: PutContent>(&self, path: &str, content: C, options: Option<FileOptions>) -> Result<String, StorageError> {
        let filename = format!("{}.bin", uuid::Uuid::new_v4());
        self.put_file_as(path, &filename, content, options).await
    }

    async fn put_file_as<C: PutContent>(&self, path: &str, name: &str, content: C, options: Option<FileOptions>) -> Result<String, StorageError> {
        let full_path = format!("{}/{}", path.trim_end_matches('/'), name);
        self.put(&full_path, content, options).await?;
        Ok(full_path)
    }

    async fn temporary_url(&self, path: &str, _options: UrlOptions) -> Result<String, StorageError> {
        let full_path = self.ensure_within_root(Path::new(path))?;
        if !full_path.exists() {
            return Err(StorageError::NotFound(path.to_string()));
        }
        Ok(format!("{}/{}", self.url.trim_end_matches('/'), path))
    }

    fn url(&self, path: &str, _options: Option<UrlOptions>) -> Result<String, StorageError> {
        Ok(self.ensure_within_root(Path::new(path))?.to_string_lossy().into_owned())
    }

    async fn download(&self, path: &str, _options: Option<UrlOptions>) -> Result<(Bytes, HeaderMap), StorageError> {
        let content = self.get(path).await?;
        let headers = HeaderMap::new();
        Ok((content, headers))
    }

    async fn directories(&self, prefix: &str, options: Option<ListOptions>) -> Result<Vec<String>, StorageError> {
        let entries = self.list(Some(prefix), options).await?;
        Ok(entries.into_iter().filter(|e| e.is_dir).map(|e| e.name).collect())
    }

    async fn make_directory(&self, path: &str) -> Result<(), StorageError> {
        let full_path = self.ensure_within_root(Path::new(path))?;
        async_fs::create_dir_all(full_path).await.map_err(StorageError::Io)?;
        Ok(())
    }

    async fn delete_directory(&self, path: &str) -> Result<(), StorageError> {
        let full_path = self.ensure_within_root(Path::new(path))?;
        if full_path.exists() {
            async_fs::remove_dir_all(full_path).await.map_err(StorageError::Io)?;
        }
        Ok(())
    }

    async fn metadata(&self, path: &str) -> Result<FileInfo, StorageError> {
        let path = self.ensure_within_root(Path::new(path))?;
        let metadata = fs::metadata(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound(path.to_string_lossy().into_owned())
            } else {
                StorageError::Io(e)
            }
        })?;

        Ok(FileInfo {
            name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            size: metadata.len(),
            last_modified: metadata.modified().ok().map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()),
            mime_type: None,
            metadata: None,
            is_dir: metadata.is_dir(),
            visibility: self.visibility.clone(),
        })
    }

    async fn copy(&self, from: &str, to: &str, _options: Option<FileOptions>) -> Result<(), StorageError> {
        let source_path = self.ensure_within_root(Path::new(from))?;
        if !source_path.exists() {
            return Err(StorageError::NotFound(from.to_string()));
        }

        let dest_path = self.ensure_within_root(Path::new(to))?;
        if let Some(parent) = dest_path.parent() {
            async_fs::create_dir_all(parent).await.map_err(StorageError::Io)?;
        }

        async_fs::copy(&source_path, &dest_path).await.map_err(StorageError::Io)?;
        Ok(())
    }

    async fn move_file(&self, from: &str, to: &str, options: Option<FileOptions>) -> Result<(), StorageError> {
        self.copy(from, to, options).await?;
        self.delete(from).await?;
        Ok(())
    }

    async fn prepend(&self, path: &str, data: Bytes, options: Option<FileOptions>) -> Result<(), StorageError> {
        let existing = if self.exists(path).await? {
            self.get(path).await?
        } else {
            Bytes::new()
        };

        let mut new_content = Vec::with_capacity(data.len() + existing.len());
        new_content.extend_from_slice(&data);
        new_content.extend_from_slice(&existing);

        self.put(path, Bytes::from(new_content), options).await
    }

    async fn append(&self, path: &str, data: Bytes, options: Option<FileOptions>) -> Result<(), StorageError> {
        let existing = if self.exists(path).await? {
            self.get(path).await?
        } else {
            Bytes::new()
        };

        let mut new_content = Vec::with_capacity(existing.len() + data.len());
        new_content.extend_from_slice(&existing);
        new_content.extend_from_slice(&data);

        self.put(path, Bytes::from(new_content), options).await
    }

    async fn visibility(&self, _path: &str) -> Result<Visibility, StorageError> {
        Ok(self.visibility.clone())
    }

    async fn set_visibility(&self, _path: &str, _visibility: Visibility) -> Result<(), StorageError> {
        // Local storage doesn't support changing visibility
        Ok(())
    }

    fn path(&self, path: &str) -> String {
        self.ensure_within_root(Path::new(path)).unwrap().to_string_lossy().into_owned()
    }

    fn get_driver(&self) -> &'static str {
        "local"
    }
}

#[async_trait]
impl StorageWriter for LocalStorage {
    async fn put<T: AsRef<[u8]> + Send + Sync>(
        &self,
        path: &str,
        content: T,
        _options: Option<&FileOptions>,
    ) -> Result<(), StorageError> {
        let full_path = self.ensure_within_root(Path::new(path))?;
        if let Some(parent) = full_path.parent() {
            async_fs::create_dir_all(parent).await.map_err(StorageError::Io)?;
        }
        async_fs::write(&full_path, content).await.map_err(|e| StorageError::Io(e))
    }

    async fn put_file_as(
        &self,
        source: &str,
        path: &str,
        options: Option<FileOptions>,
    ) -> Result<(), StorageError> {
        let source_path = Path::new(source);
        if !source_path.exists() {
            return Err(StorageError::FileNotFound(source.to_string()));
        }

        let content = async_fs::read(source_path).await.map_err(|e| StorageError::Io(e))?;
        self.put(path, content, options.as_ref()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_local_storage() {
        let temp_dir = tempdir().unwrap();
        let storage = LocalStorage::new(
            temp_dir.path(),
            "http://localhost".to_string(),
            Visibility::Public,
        )
        .unwrap();

        // Test put and get
        storage.put("test.txt", "Hello World", None).await.unwrap();
        assert!(storage.exists("test.txt").await.unwrap());
        assert_eq!(
            storage.get("test.txt").await.unwrap(),
            Bytes::from("Hello World")
        );

        // Test list
        let files = storage.list(None, None).await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "test.txt");

        // Test delete
        storage.delete("test.txt").await.unwrap();
        assert!(!storage.exists("test.txt").await.unwrap());

        // Test directory operations
        storage.make_directory("dir").await.unwrap();
        storage.put("dir/file1.txt", "Hello", None).await.unwrap();
        storage.put("dir/file2.txt", "World", None).await.unwrap();

        let files = storage.list(Some("dir"), None).await.unwrap();
        assert_eq!(files.len(), 2);

        storage.delete_directory("dir").await.unwrap();
        assert!(!storage.exists("dir/file1.txt").await.unwrap());
        assert!(!storage.exists("dir/file2.txt").await.unwrap());

        // Test copy and move
        storage.put("source.txt", "Test", None).await.unwrap();
        storage.copy("source.txt", "copy.txt", None).await.unwrap();
        assert_eq!(
            storage.get("copy.txt").await.unwrap(),
            Bytes::from("Test")
        );

        storage
            .move_file("copy.txt", "moved.txt", None)
            .await
            .unwrap();
        assert!(!storage.exists("copy.txt").await.unwrap());
        assert_eq!(
            storage.get("moved.txt").await.unwrap(),
            Bytes::from("Test")
        );
    }
}
