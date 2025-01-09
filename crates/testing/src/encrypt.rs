use std::sync::Once;

static INIT: Once = Once::new();

/// Helper struct for encryption testing
pub struct EncryptTestHelper;

impl EncryptTestHelper {
    /// Initialize encryption with test keys
    pub fn initialize() {
        INIT.call_once(|| {
            std::env::set_var("APP_KEY", "base64:dGVzdGtleXRlc3RrZXl0ZXN0a2V5dGVzdGtleXRlc3Q=");
            encrypt::Crypt::initialize().expect("Failed to initialize encryption");
        });
    }

    /// Get a test encryption key
    pub fn test_key() -> String {
        "base64:dGVzdGtleXRlc3RrZXl0ZXN0a2V5dGVzdGtleXRlc3Q=".to_string()
    }

    /// Get a test previous key
    pub fn test_previous_key() -> String {
        "base64:cHJldmlvdXNrZXlwcmV2aW91c2tleXByZXZpb3Vza2V5cHI=".to_string()
    }

    /// Set up key rotation test environment
    pub fn setup_key_rotation() {
        std::env::set_var("APP_KEY", Self::test_key());
        std::env::set_var("APP_PREVIOUS_KEYS", Self::test_previous_key());
        encrypt::Crypt::initialize().expect("Failed to initialize encryption");
    }

    /// Clean up test environment
    pub fn cleanup() {
        std::env::remove_var("APP_KEY");
        std::env::remove_var("APP_PREVIOUS_KEYS");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use encrypt::Crypt;

    #[test]
    fn test_helper_initialization() {
        EncryptTestHelper::initialize();
        let value = "test value";
        let encrypted = Crypt::encrypt_string(value).unwrap();
        let decrypted = Crypt::decrypt_string(&encrypted).unwrap();
        assert_eq!(decrypted, value);
    }

    #[test]
    fn test_helper_key_rotation() {
        EncryptTestHelper::setup_key_rotation();
        let value = "test value";
        let encrypted = Crypt::encrypt_string(value).unwrap();
        let decrypted = Crypt::decrypt_string(&encrypted).unwrap();
        assert_eq!(decrypted, value);
        EncryptTestHelper::cleanup();
    }
}
