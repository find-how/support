use std::env;
use thiserror::Error;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use aes::Aes256;
use block_modes::{BlockMode, Cbc};
use block_modes::block_padding::Pkcs7;
use lazy_static::lazy_static;
use std::sync::Mutex;
use rand::RngCore;

type Aes256Cbc = Cbc<Aes256, Pkcs7>;
type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Missing encryption key")]
    MissingKey,
    #[error("Invalid key format")]
    InvalidKeyFormat,
    #[error("Encryption error")]
    Encryption,
    #[error("Decryption error")]
    Decryption,
    #[error("MAC mismatch")]
    InvalidMac,
    #[error("Invalid data format")]
    InvalidFormat,
}

pub type Result<T> = std::result::Result<T, Error>;

lazy_static! {
    static ref ENCRYPTION_KEY: Mutex<Vec<u8>> = Mutex::new(vec![0u8; 32]);
    static ref PREVIOUS_KEYS: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
}

/// The main encryption facade
pub struct Crypt;

impl Crypt {
    /// Initialize encryption with the current key and optional previous keys
    pub fn initialize() -> Result<()> {
        // Get current key
        let key_str = env::var("APP_KEY").map_err(|_| Error::MissingKey)?;
        let key_bytes = decode_key(&key_str)?;
        *ENCRYPTION_KEY.lock().unwrap() = key_bytes;

        // Get previous keys if any
        if let Ok(prev_keys) = env::var("APP_PREVIOUS_KEYS") {
            let mut previous = PREVIOUS_KEYS.lock().unwrap();
            for key in prev_keys.split(',') {
                if let Ok(key_bytes) = decode_key(key) {
                    previous.push(key_bytes);
                }
            }
        }

        Ok(())
    }

    /// Generate a new encryption key
    pub fn generate_key() -> String {
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        format!("base64:{}", BASE64.encode(key))
    }

    /// Encrypt a string value
    pub fn encrypt_string(value: &str) -> Result<String> {
        let key = ENCRYPTION_KEY.lock().unwrap();

        // Generate random IV
        let mut iv = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut iv);

        // Encrypt
        let cipher = Aes256Cbc::new_from_slices(&key, &iv)
            .map_err(|_| Error::Encryption)?;
        let ciphertext = cipher.encrypt_vec(value.as_bytes());

        // Combine IV and ciphertext
        let mut payload = iv.to_vec();
        payload.extend_from_slice(&ciphertext);

        // Generate MAC
        let mac = generate_mac(&payload, &key)?;

        // Combine everything and base64 encode
        let mut final_payload = payload;
        final_payload.extend_from_slice(&mac);
        Ok(BASE64.encode(final_payload))
    }

    /// Decrypt a string value
    pub fn decrypt_string(encrypted: &str) -> Result<String> {
        let encrypted = BASE64.decode(encrypted).map_err(|_| Error::InvalidFormat)?;
        if encrypted.len() < 48 { // 16 (IV) + 32 (MAC) minimum
            return Err(Error::InvalidFormat);
        }

        // Split into parts
        let payload_len = encrypted.len() - 32;
        let (payload, mac) = encrypted.split_at(payload_len);
        let (iv, ciphertext) = payload.split_at(16);

        // Try current key first
        let key = ENCRYPTION_KEY.lock().unwrap();
        if let Ok(decrypted) = decrypt_with_key(payload, mac, iv, ciphertext, &key) {
            return Ok(decrypted);
        }

        // Try previous keys
        let previous = PREVIOUS_KEYS.lock().unwrap();
        for prev_key in previous.iter() {
            if let Ok(decrypted) = decrypt_with_key(payload, mac, iv, ciphertext, prev_key) {
                return Ok(decrypted);
            }
        }

        Err(Error::Decryption)
    }
}

// Helper functions

fn decode_key(key: &str) -> Result<Vec<u8>> {
    if !key.starts_with("base64:") {
        return Err(Error::InvalidKeyFormat);
    }
    let key = &key[7..]; // Skip "base64:" prefix
    let key_bytes = BASE64.decode(key).map_err(|_| Error::InvalidKeyFormat)?;
    if key_bytes.len() != 32 {
        eprintln!("Invalid key length: {}", key_bytes.len());
        return Err(Error::InvalidKeyFormat);
    }
    Ok(key_bytes)
}

fn generate_mac(payload: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|_| Error::Encryption)?;
    mac.update(payload);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn verify_mac(payload: &[u8], mac: &[u8], key: &[u8]) -> Result<()> {
    let mut hmac = HmacSha256::new_from_slice(key)
        .map_err(|_| Error::Decryption)?;
    hmac.update(payload);
    hmac.verify_slice(mac).map_err(|_| Error::InvalidMac)
}

fn decrypt_with_key(payload: &[u8], mac: &[u8], iv: &[u8], ciphertext: &[u8], key: &[u8]) -> Result<String> {
    // Verify MAC
    verify_mac(payload, mac, key)?;

    // Decrypt
    let cipher = Aes256Cbc::new_from_slices(key, iv)
        .map_err(|_| Error::Decryption)?;
    let decrypted = cipher.decrypt_vec(ciphertext)
        .map_err(|_| Error::Decryption)?;

    String::from_utf8(decrypted)
        .map_err(|_| Error::Decryption)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_key() {
        env::set_var("APP_KEY", "base64:dGVzdGtleXRlc3RrZXl0ZXN0a2V5dGVzdGtleXRlc3Q="); // 32 bytes key
    }

    #[test]
    fn test_encryption_roundtrip() -> Result<()> {
        setup_test_key();
        Crypt::initialize()?;

        let original = "test value";
        let encrypted = Crypt::encrypt_string(original)?;
        let decrypted = Crypt::decrypt_string(&encrypted)?;
        assert_eq!(decrypted, original);
        Ok(())
    }

    #[test]
    fn test_key_rotation() -> Result<()> {
        // Set up old key
        setup_test_key();
        Crypt::initialize()?;
        let value = "test value";
        let encrypted = Crypt::encrypt_string(value)?;

        // Rotate to new key (must be 32 bytes when decoded)
        let old_key = env::var("APP_KEY").unwrap();
        let new_key = "base64:dGVzdGtleTJ0ZXN0a2V5MnRlc3RrZXkydGVzdGtleTI="; // "testkey2testkey2testkey2testkey2"
        env::set_var("APP_KEY", new_key);
        env::set_var("APP_PREVIOUS_KEYS", &old_key);
        Crypt::initialize()?;

        // Should still be able to decrypt with old key
        let decrypted = Crypt::decrypt_string(&encrypted)?;
        assert_eq!(decrypted, value);
        Ok(())
    }

    #[test]
    fn test_mac_tampering() -> Result<()> {
        setup_test_key();
        Crypt::initialize()?;

        let encrypted = Crypt::encrypt_string("test value")?;
        let mut bytes = BASE64.decode(&encrypted).unwrap();
        // Tamper with the ciphertext
        bytes[20] ^= 1;
        let tampered = BASE64.encode(bytes);

        assert!(Crypt::decrypt_string(&tampered).is_err());
        Ok(())
    }

    #[test]
    fn test_generate_key() {
        let key = Crypt::generate_key();
        assert!(key.starts_with("base64:"));
        let decoded = decode_key(&key);
        assert!(decoded.is_ok());
        assert_eq!(decoded.unwrap().len(), 32);
    }
}
