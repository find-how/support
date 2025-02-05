use aws_sdk_s3::{
    error::SdkError,
    operation::{
        copy_object::CopyObjectError,
        delete_object::DeleteObjectError,
        get_object::GetObjectError,
        get_object_acl::GetObjectAclError,
        head_object::HeadObjectError,
        list_objects_v2::ListObjectsV2Error,
        put_object::PutObjectError,
        put_object_acl::PutObjectAclError,
    },
    primitives::ByteStream,
    types::SdkError as AwsSdkError,
};
use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Storage driver '{0}' not found")]
    InvalidDriver(String),
    #[error("Storage disk '{0}' not found")]
    DiskNotFound(String),
    #[error("File not found: {0}")]
    NotFound(String),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("AWS S3 error: {0}")]
    S3(String),
    #[error("Other error: {0}")]
    Other(String),
}

impl From<SdkError<HeadObjectError, aws_smithy_runtime_api::http::Response>> for StorageError {
    fn from(err: SdkError<HeadObjectError, aws_smithy_runtime_api::http::Response>) -> Self {
        StorageError::S3(err.to_string())
    }
}

impl From<SdkError<GetObjectError, aws_smithy_runtime_api::http::Response>> for StorageError {
    fn from(err: SdkError<GetObjectError, aws_smithy_runtime_api::http::Response>) -> Self {
        StorageError::S3(err.to_string())
    }
}

impl From<SdkError<DeleteObjectError, aws_smithy_runtime_api::http::Response>> for StorageError {
    fn from(err: SdkError<DeleteObjectError, aws_smithy_runtime_api::http::Response>) -> Self {
        StorageError::S3(err.to_string())
    }
}

impl From<SdkError<ListObjectsV2Error, aws_smithy_runtime_api::http::Response>> for StorageError {
    fn from(err: SdkError<ListObjectsV2Error, aws_smithy_runtime_api::http::Response>) -> Self {
        StorageError::S3(err.to_string())
    }
}

impl From<SdkError<PutObjectError, aws_smithy_runtime_api::http::Response>> for StorageError {
    fn from(err: SdkError<PutObjectError, aws_smithy_runtime_api::http::Response>) -> Self {
        StorageError::S3(err.to_string())
    }
}

impl From<SdkError<CopyObjectError, aws_smithy_runtime_api::http::Response>> for StorageError {
    fn from(err: SdkError<CopyObjectError, aws_smithy_runtime_api::http::Response>) -> Self {
        StorageError::S3(err.to_string())
    }
}

impl From<SdkError<GetObjectAclError, aws_smithy_runtime_api::http::Response>> for StorageError {
    fn from(err: SdkError<GetObjectAclError, aws_smithy_runtime_api::http::Response>) -> Self {
        StorageError::S3(err.to_string())
    }
}

impl From<SdkError<PutObjectAclError, aws_smithy_runtime_api::http::Response>> for StorageError {
    fn from(err: SdkError<PutObjectAclError, aws_smithy_runtime_api::http::Response>) -> Self {
        StorageError::S3(err.to_string())
    }
}

impl From<aws_sdk_s3::presigning::PresigningConfigError> for StorageError {
    fn from(err: aws_sdk_s3::presigning::PresigningConfigError) -> Self {
        StorageError::S3(err.to_string())
    }
}
