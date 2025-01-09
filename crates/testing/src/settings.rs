use std::env;
use std::path::PathBuf;
use assert_fs::TempDir;
use base64::{Engine, engine::general_purpose::STANDARD};
use rand::RngCore;

/// A test helper for managing settings in tests
pub struct SettingsTestHelper {
    pub temp_dir: TempDir,
    pub encryption_key: String,
    pub encryption_iv: String,
}

impl SettingsTestHelper {
    /// Create a new settings test helper with temporary directory and encryption keys
    pub fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let settings_dir = temp_dir.child("settings");
        settings_dir.create_dir_all().unwrap();

        // Generate random encryption keys
        let mut key = [0u8; 32];
        let mut iv = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut key);
        rand::thread_rng().fill_bytes(&mut iv);

        let encryption_key = STANDARD.encode(key);
        let encryption_iv = STANDARD.encode(iv);

        Self {
            temp_dir,
            encryption_key,
            encryption_iv,
        }
    }

    /// Get the path to the settings binary
    pub fn binary_path() -> PathBuf {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
        PathBuf::from(format!("{}/../../target/{}/settings", manifest_dir, profile))
    }

    /// Set up environment variables for encryption
    pub fn setup_env(&self) {
        env::set_var("ENCRYPTION_KEY", &self.encryption_key);
        env::set_var("ENCRYPTION_IV", &self.encryption_iv);
    }

    /// Clean up environment variables
    pub fn cleanup_env(&self) {
        env::remove_var("ENCRYPTION_KEY");
        env::remove_var("ENCRYPTION_IV");
    }

    /// Run a settings command with proper environment setup
    pub fn run_command(&self, args: &[&str]) -> std::process::Output {
        let binary = Self::binary_path();
        let output = std::process::Command::new(binary)
            .args(args)
            .env("ENCRYPTION_KEY", &self.encryption_key)
            .env("ENCRYPTION_IV", &self.encryption_iv)
            .current_dir(&self.temp_dir)
            .output()
            .expect("Failed to execute command");

        output
    }

    /// Assert that a command succeeded and return its output
    pub fn assert_success(&self, args: &[&str]) -> String {
        let output = self.run_command(args);
        assert!(output.status.success(),
            "Command failed with status: {}\nstdout: {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Assert that a command failed and return its error output
    pub fn assert_failure(&self, args: &[&str]) -> String {
        let output = self.run_command(args);
        assert!(!output.status.success());
        String::from_utf8_lossy(&output.stderr).trim().to_string()
    }
}
