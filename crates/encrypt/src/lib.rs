//! Encryption utilities with Laravel-inspired patterns
//!
//! This crate provides encryption and decryption utilities using AES-256-CBC.

use async_trait::async_trait;
use thiserror::Error;
use aes::Aes256;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use cbc::{Encryptor as CbcEncryptor, Decryptor as CbcDecryptor, cipher::{block_padding::Pkcs7, BlockEncryptMut, BlockDecryptMut, KeyIvInit}};
use std::env;

type Aes256CbcEnc = CbcEncryptor<Aes256>;
type Aes256CbcDec = CbcDecryptor<Aes256>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Encryption error: {0}")]
    Encryption(String),
    #[error("Decryption error: {0}")]
    Decryption(String),
    #[error("Key error: {0}")]
    Key(String),
    #[error("Environment error: {0}")]
    Environment(String),
}

/// Encryption interface
#[async_trait]
pub trait Encryptor: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Encrypt data
    async fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, Self::Error>;

    /// Decrypt data
    async fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, Self::Error>;
}

/// Memory encryptor for testing
#[derive(Debug, Default)]
pub struct MemoryEncryptor;

#[async_trait]
impl Encryptor for MemoryEncryptor {
    type Error = Error;

    async fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, Self::Error> {
        Ok(data.to_vec())
    }

    async fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, Self::Error> {
        Ok(data.to_vec())
    }
}

pub struct Crypt {
    key: [u8; 32],
    iv: [u8; 16],
}

impl Crypt {
    pub fn initialize() -> Result<(), Error> {
        let key = env::var("APP_KEY").map_err(|_| Error::Environment("APP_KEY not set".into()))?;

        // Handle base64: prefix
        let key_data = if key.starts_with("base64:") {
            key[7..].to_string()
        } else {
            key
        };

        // Check the actual base64 data length
        if key_data.len() != 44 {
            return Err(Error::Key(format!("Invalid key length: {}. Expected 44 characters after base64: prefix", key_data.len())));
        }

        // Validate that it's valid base64
        BASE64.decode(&key_data)
            .map_err(|_| Error::Key("Invalid base64 encoding".into()))?;

        Ok(())
    }

    pub fn new(key: [u8; 32], iv: [u8; 16]) -> Self {
        Self { key, iv }
    }

    pub fn generate_key() -> String {
        let mut key = [0u8; 32];
        getrandom::getrandom(&mut key).expect("Failed to generate random key");
        BASE64.encode(key)
    }

    pub fn encrypt(&self, value: &str) -> Result<String, Error> {
        let cipher = Aes256CbcEnc::new(&self.key.into(), &self.iv.into());
        let ciphertext = cipher.encrypt_padded_vec_mut::<Pkcs7>(value.as_bytes());

        let mut combined = Vec::with_capacity(self.iv.len() + ciphertext.len());
        combined.extend_from_slice(&self.iv);
        combined.extend_from_slice(&ciphertext);

        Ok(BASE64.encode(combined))
    }

    pub fn decrypt(&self, value: &str) -> Result<String, Error> {
        let encrypted = BASE64.decode(value).map_err(|_| Error::Decryption("Invalid base64".into()))?;
        if encrypted.len() < 16 {
            return Err(Error::Decryption("Invalid ciphertext length".into()));
        }

        let (iv, ciphertext) = encrypted.split_at(16);
        if iv != self.iv {
            return Err(Error::Decryption("IV mismatch".into()));
        }

        let cipher = Aes256CbcDec::new(&self.key.into(), &self.iv.into());
        let plaintext = cipher.decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
            .map_err(|e| Error::Decryption(e.to_string()))?;

        String::from_utf8(plaintext).map_err(|_| Error::Decryption("Invalid UTF-8".into()))
    }

    pub fn encrypt_string(value: &str) -> Result<String, Error> {
        let key = env::var("APP_KEY").map_err(|_| Error::Environment("APP_KEY not set".into()))?;

        // Handle base64: prefix
        let key_data = if key.starts_with("base64:") {
            key[7..].to_string()
        } else {
            key
        };

        let key = BASE64.decode(&key_data).map_err(|_| Error::Key("Invalid key format".into()))?;
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&key);

        let mut iv = [0u8; 16];
        getrandom::getrandom(&mut iv).map_err(|_| Error::Encryption("Failed to generate IV".into()))?;

        let cipher = Aes256CbcEnc::new(&key_bytes.into(), &iv.into());
        let ciphertext = cipher.encrypt_padded_vec_mut::<Pkcs7>(value.as_bytes());

        let mut combined = Vec::with_capacity(iv.len() + ciphertext.len());
        combined.extend_from_slice(&iv);
        combined.extend_from_slice(&ciphertext);

        Ok(BASE64.encode(combined))
    }

    pub fn decrypt_string(value: &str) -> Result<String, Error> {
        let key = env::var("APP_KEY").map_err(|_| Error::Environment("APP_KEY not set".into()))?;

        // Handle base64: prefix
        let key_data = if key.starts_with("base64:") {
            key[7..].to_string()
        } else {
            key
        };

        let key = BASE64.decode(&key_data).map_err(|_| Error::Key("Invalid key format".into()))?;
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&key);

        let encrypted = BASE64.decode(value).map_err(|_| Error::Decryption("Invalid base64".into()))?;
        if encrypted.len() < 16 {
            return Err(Error::Decryption("Invalid ciphertext length".into()));
        }

        let (iv, ciphertext) = encrypted.split_at(16);
        let mut iv_bytes = [0u8; 16];
        iv_bytes.copy_from_slice(iv);

        let cipher = Aes256CbcDec::new(&key_bytes.into(), &iv_bytes.into());
        let plaintext = cipher.decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
            .map_err(|e| Error::Decryption(e.to_string()))?;

        String::from_utf8(plaintext).map_err(|_| Error::Decryption("Invalid UTF-8".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_key() {
        let key = Crypt::generate_key();
        assert_eq!(key.len(), 44); // Base64 encoded 32 bytes
        assert!(BASE64.decode(&key).is_ok());
    }

    #[tokio::test]
    async fn test_encryption_roundtrip() {
        let original = "Hello, World!";
        env::set_var("APP_KEY", Crypt::generate_key());

        let encrypted = Crypt::encrypt_string(original).unwrap();
        let decrypted = Crypt::decrypt_string(&encrypted).unwrap();

        assert_eq!(original, decrypted);
    }

    #[tokio::test]
    async fn test_instance_encryption_roundtrip() {
        let original = "Hello, World!";
        let mut key = [0u8; 32];
        let mut iv = [0u8; 16];
        getrandom::getrandom(&mut key).unwrap();
        getrandom::getrandom(&mut iv).unwrap();

        let crypt = Crypt::new(key, iv);
        let encrypted = crypt.encrypt(original).unwrap();
        let decrypted = crypt.decrypt(&encrypted).unwrap();

        assert_eq!(original, decrypted);
    }
}
