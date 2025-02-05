use async_trait::async_trait;
use aws_sdk_s3::{
    config::Region,
    types::{ByteStream, ObjectCannedAcl},
    Client,
};
use aws_types::Credentials;
use bytes::Bytes;
use http::HeaderMap;
use std::collections::HashMap;
use std::pin::Pin;
use tokio::io::AsyncRead;
use std::time::SystemTime;

use crate::error::StorageError;
use crate::traits::{StorageOps, StorageTrait};
use crate::types::{FileInfo, FileOptions, ListOptions, PutContent, UrlOptions, Visibility};

#[derive(Debug, Clone)]
pub struct S3Config {
    pub bucket: String,
    pub region: Option<String>,
    pub endpoint: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}

pub struct S3Storage {
    client: Client,
    bucket: String,
    config: S3Config,
    root: String,
}

impl S3Storage {
    pub async fn new(config: StorageConfig) -> Result<Self, StorageError> {
        let s3_config = config.s3.ok_or_else(|| StorageError::InvalidConfig("Missing S3 config".into()))?;

        let mut s3_config_builder = aws_sdk_s3::Config::builder()
            .region(Region::new(s3_config.region))
            .force_path_style(true);

        if let Some(endpoint) = s3_config.endpoint {
            s3_config_builder = s3_config_builder.endpoint_url(endpoint);
        }

        if let (Some(key), Some(secret)) = (s3_config.key, s3_config.secret) {
            let credentials_provider = aws_sdk_s3::config::Credentials::new(
                key,
                secret,
                s3_config.token,
                None,
                "s3-storage",
            );
            s3_config_builder = s3_config_builder.credentials_provider(credentials_provider);
        }

        let s3_config = s3_config_builder.build();
        let client = Client::from_conf(s3_config);

        Ok(Self {
            client,
            bucket: s3_config.bucket,
            config: s3_config,
            root: config.root,
        })
    }

    fn object_to_file_info(&self, obj: &Object) -> FileInfo {
        let path = obj
            .key()
            .map(|k| {
                if let Some(root) = self.root.strip_suffix('/') {
                    k.strip_prefix(root)
                        .map(|p| p.trim_start_matches('/'))
                        .unwrap_or(k)
                } else {
                    k
                }
            })
            .unwrap_or_default()
            .to_string();

        FileInfo {
            path,
            size: obj.size().unwrap_or(0) as u64,
            last_modified: obj
                .last_modified()
                .map(|dt| dt.as_secs_f64() as u64)
                .unwrap_or(0),
            content_type: obj.content_type().map(|s| s.to_string()),
            metadata: None,
            visibility: Visibility::Private, // Default to private, will be updated by get_visibility
        }
    }

    fn object_to_file_metadata(&self, obj: &GetObjectOutput) -> FileMetadata {
        FileMetadata {
            path: obj.key().unwrap_or_default().to_string(),
            size: obj.content_length() as u64,
            last_modified: obj
                .last_modified()
                .map(|dt| dt.secs())
                .unwrap_or_else(|| SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()),
            content_type: obj.content_type().map(|s| s.to_string()),
            metadata: None,
            visibility: "private".to_string(),
        }
    }

    fn normalize_path(&self, path: &str) -> String {
        let path = path.trim_start_matches('/');
        if self.root.is_empty() {
            path.to_string()
        } else {
            format!("{}/{}", self.root.trim_matches('/'), path)
        }
    }
}

#[async_trait]
impl StorageTrait for S3Storage {
    async fn exists(&self, path: &str) -> Result<bool, StorageError> {
        let key = self.normalize_path(path);
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(SdkError::ServiceError(err)) if err.err().is_not_found() => Ok(false),
            Err(err) => Err(StorageError::from(err)),
        }
    }

    async fn get(&self, path: &str) -> Result<Bytes, StorageError> {
        let key = self.normalize_path(path);
        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(StorageError::from)?;

        let data = output.body.collect().await.map_err(|e| StorageError::Io(e.into()))?;
        Ok(data.into_bytes())
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        let key = self.normalize_path(path);
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(StorageError::from)?;
        Ok(())
    }

    async fn list(&self, path: &str, options: Option<ListOptions>) -> Result<Vec<FileInfo>, StorageError> {
        let options = options.unwrap_or_default();
        let mut entries = Vec::new();

        let mut paginator = self.client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(path)
            .into_paginator()
            .send();

        while let Some(page) = paginator.next().await {
            let page = page.map_err(StorageError::S3)?;
            if let Some(objects) = page.contents {
                for obj in objects {
                    entries.push(FileInfo {
                        name: obj.key.unwrap_or_default(),
                        size: obj.size.unwrap_or(0) as u64,
                        last_modified: obj.last_modified.map(|t| t.secs()),
                        mime_type: obj.content_type().map(|s| s.to_string()),
                        metadata: None,
                        is_dir: false,
                    });
                }
            }
        }

        if let Some(limit) = options.limit {
            entries.truncate(limit);
        }

        Ok(entries)
    }

    async fn stream(&self, path: &str) -> Result<Pin<Box<dyn AsyncRead + Send>>, StorageError> {
        let resp = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("404") {
                    StorageError::NotFound(path.to_string())
                } else {
                    StorageError::S3(e)
                }
            })?;

        Ok(Box::pin(resp.body.into_async_read()))
    }

    async fn response(&self, path: &str) -> Result<(Pin<Box<dyn AsyncRead + Send>>, HeaderMap), StorageError> {
        let resp = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("404") {
                    StorageError::NotFound(path.to_string())
                } else {
                    StorageError::S3(e)
                }
            })?;

        let mut headers = HeaderMap::new();
        if let Some(content_type) = resp.content_type() {
            headers.insert("content-type", content_type.parse().unwrap());
        }
        if let Some(content_length) = resp.content_length() {
            headers.insert("content-length", content_length.to_string().parse().unwrap());
        }

        Ok((Box::pin(resp.body.into_async_read()), headers))
    }
}

#[async_trait]
impl StorageOps for S3Storage {
    async fn put<C: PutContent>(&self, path: &str, content: C, options: Option<FileOptions>) -> Result<(), StorageError> {
        let bytes = content.to_bytes().await.map_err(StorageError::Io)?;
        let mut req = self.client
            .put_object()
            .bucket(&self.bucket)
            .key(path)
            .body(ByteStream::from(bytes));

        if let Some(options) = options {
            if let Some(metadata) = options.metadata {
                req = req.metadata(metadata);
            }
            if let Some(visibility) = options.visibility {
                let acl = match visibility.as_str() {
                    "public" => ObjectCannedAcl::PublicRead,
                    _ => ObjectCannedAcl::Private,
                };
                req = req.acl(acl);
            }
        }

        req.send().await.map_err(StorageError::S3)?;
        Ok(())
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

    async fn temporary_url(&self, path: &str, options: UrlOptions) -> Result<String, StorageError> {
        let expires_in = options.expiry.unwrap_or(3600);
        let presigned_req = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(path)
            .presigned(aws_sdk_s3::presigning::PresigningConfig::expires_in(
                std::time::Duration::from_secs(expires_in),
            )?)
            .await?;

        Ok(presigned_req.uri().to_string())
    }

    fn url(&self, path: &str, _options: Option<UrlOptions>) -> Result<String, StorageError> {
        Ok(format!("https://{}.s3.amazonaws.com/{}", self.bucket, path))
    }

    async fn download(&self, path: &str, _options: Option<UrlOptions>) -> Result<(Bytes, HeaderMap), StorageError> {
        let resp = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("404") {
                    StorageError::NotFound(path.to_string())
                } else {
                    StorageError::S3(e)
                }
            })?;

        let mut headers = HeaderMap::new();
        if let Some(content_type) = resp.content_type() {
            headers.insert("content-type", content_type.parse().unwrap());
        }
        if let Some(content_length) = resp.content_length() {
            headers.insert("content-length", content_length.to_string().parse().unwrap());
        }

        let data = resp.body.collect().await.map_err(|e| StorageError::Other(e.to_string()))?;
        Ok((data.into_bytes(), headers))
    }

    async fn directories(&self, prefix: &str, options: Option<ListOptions>) -> Result<Vec<String>, StorageError> {
        let mut dirs = Vec::new();
        let options = options.unwrap_or_default();

        let mut paginator = self.client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(prefix)
            .delimiter("/")
            .into_paginator()
            .send();

        while let Some(page) = paginator.next().await {
            let page = page.map_err(StorageError::S3)?;
            if let Some(prefixes) = page.common_prefixes {
                for prefix in prefixes {
                    if let Some(prefix) = prefix.prefix {
                        dirs.push(prefix);
                    }
                }
            }
        }

        if let Some(limit) = options.limit {
            dirs.truncate(limit);
        }

        Ok(dirs)
    }

    async fn make_directory(&self, path: &str) -> Result<(), StorageError> {
        // S3 doesn't have real directories, but we can create an empty object with a trailing slash
        let path = format!("{}/", path.trim_end_matches('/'));
        self.put(&path, Bytes::new(), None).await
    }

    async fn delete_directory(&self, path: &str) -> Result<(), StorageError> {
        let mut objects = Vec::new();
        let mut paginator = self.client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(path)
            .into_paginator()
            .send();

        while let Some(page) = paginator.next().await {
            let page = page.map_err(StorageError::S3)?;
            if let Some(contents) = page.contents {
                for obj in contents {
                    if let Some(key) = obj.key {
                        objects.push(key);
                    }
                }
            }
        }

        for key in objects {
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(key)
                .send()
                .await
                .map_err(StorageError::S3)?;
        }

        Ok(())
    }

    async fn metadata(&self, path: &str) -> Result<FileInfo, StorageError> {
        let obj = self.client
            .head_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("404") {
                    StorageError::NotFound(path.to_string())
                } else {
                    StorageError::S3(e)
                }
            })?;

        Ok(FileInfo {
            name: path.to_string(),
            size: obj.content_length.unwrap_or(0) as u64,
            last_modified: obj.last_modified.map(|t| t.secs()),
            mime_type: obj.content_type().map(|s| s.to_string()),
            metadata: obj.metadata().map(|m| m.clone()),
            is_dir: false,
        })
    }

    async fn copy(&self, from: &str, to: &str, options: Option<FileOptions>) -> Result<(), StorageError> {
        let mut req = self.client
            .copy_object()
            .bucket(&self.bucket)
            .key(to)
            .copy_source(format!("{}/{}", self.bucket, from));

        if let Some(options) = options {
            if let Some(metadata) = options.metadata {
                req = req.metadata(metadata);
            }
            if let Some(visibility) = options.visibility {
                let acl = match visibility.as_str() {
                    "public" => ObjectCannedAcl::PublicRead,
                    _ => ObjectCannedAcl::Private,
                };
                req = req.acl(acl);
            }
        }

        req.send().await.map_err(StorageError::S3)?;
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

    async fn visibility(&self, path: &str) -> Result<Visibility, StorageError> {
        let key = self.normalize_path(path);
        let output = self
            .client
            .get_object_acl()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(StorageError::from)?;

        let is_public = output
            .grants()
            .iter()
            .any(|grant| {
                grant
                    .grantee()
                    .and_then(|g| g.uri())
                    .map(|uri| uri.contains("AllUsers"))
                    .unwrap_or(false)
                    && grant.permission().map(|p| p.as_str() == "READ").unwrap_or(false)
            });

        Ok(if is_public {
            Visibility::Public
        } else {
            Visibility::Private
        })
    }

    async fn set_visibility(&self, path: &str, visibility: Visibility) -> Result<(), StorageError> {
        let key = self.normalize_path(path);
        let acl = match visibility {
            Visibility::Public => ObjectCannedAcl::PublicRead,
            Visibility::Private => ObjectCannedAcl::Private,
        };

        self.client
            .put_object_acl()
            .bucket(&self.bucket)
            .key(&key)
            .acl(acl)
            .send()
            .await
            .map_err(StorageError::from)?;

        Ok(())
    }

    fn path(&self, path: &str) -> String {
        path.to_string()
    }

    fn get_driver(&self) -> &'static str {
        "s3"
    }
}
