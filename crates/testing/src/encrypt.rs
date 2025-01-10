use std::env;
use std::sync::Once;
use base64::{Engine, engine::general_purpose::STANDARD};
use rand::RngCore;

static INIT: Once = Once::new();

pub fn setup_test_encryption() {
    INIT.call_once(|| {
        // Generate a random 32-byte key
        let mut key = [0u8; 32];
        let mut iv = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut key);
        rand::thread_rng().fill_bytes(&mut iv);

        // Encode as base64
        let key_b64 = STANDARD.encode(&key);
        let iv_b64 = STANDARD.encode(&iv);

        // Set environment variables
        env::set_var("APP_KEY", format!("base64:{}", key_b64));
        env::set_var("APP_IV", format!("base64:{}", iv_b64));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_encryption() {
        setup_test_encryption();
        assert!(env::var("APP_KEY").is_ok());
        assert!(env::var("APP_IV").is_ok());
    }
}
