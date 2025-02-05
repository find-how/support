use std::process::Command;

fn run_cmd(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg("encrypt")
        .arg("--")
        .args(args)
        .env("APP_KEY", "base64:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=")
        .output()
        .expect("Failed to execute command")
}

#[test]
fn test_encrypt_decrypt() {
    // Set a test key
    std::env::set_var("APP_KEY", "base64:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=");

    // Test encryption
    let output = run_cmd(&["encrypt", "test value"]);
    assert!(output.status.success(), "Encrypt failed: {}", String::from_utf8_lossy(&output.stderr));
    let encrypted = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Test decryption
    let output = run_cmd(&["decrypt", &encrypted]);
    assert!(output.status.success(), "Decrypt failed: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "test value");
}

#[test]
fn test_base64_encode_decode() {
    let value = "test value";

    // Encode
    let output = run_cmd(&["encode", value, "--base64"]);
    assert!(output.status.success(), "Encode failed: {}", String::from_utf8_lossy(&output.stderr));
    let encoded = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Decode
    let output = run_cmd(&["decode", &encoded, "--base64"]);
    assert!(output.status.success(), "Decode failed: {}", String::from_utf8_lossy(&output.stderr));
    let decoded = String::from_utf8_lossy(&output.stdout).trim().to_string();

    assert_eq!(decoded, value);
}

#[test]
fn test_key_generation() {
    let output = run_cmd(&["generate-key"]);
    assert!(output.status.success(), "Key generation failed: {}", String::from_utf8_lossy(&output.stderr));
    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(key.len() >= 44, "Key should be at least 44 characters long");
}

#[test]
fn test_key_rotation() {
    // Set initial key
    std::env::set_var("APP_KEY", "base64:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=");

    // Test encryption with initial key
    let output = run_cmd(&["encrypt", "test value"]);
    assert!(output.status.success(), "Encryption failed: {}", String::from_utf8_lossy(&output.stderr));
    let encrypted = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Rotate key
    let output = run_cmd(&["rotate"]);
    assert!(output.status.success(), "Key rotation failed: {}", String::from_utf8_lossy(&output.stderr));
    let output_str = String::from_utf8_lossy(&output.stdout);

    // Extract new key from output
    let new_key = output_str
        .lines()
        .find(|line| line.starts_with("APP_KEY="))
        .expect("No APP_KEY in output")
        .strip_prefix("APP_KEY=")
        .unwrap();

    // Set new key and previous key
    std::env::set_var("APP_KEY", new_key);
    std::env::set_var(
        "APP_PREVIOUS_KEYS",
        "base64:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
    );

    // Test decryption with new key setup
    let output = run_cmd(&["decrypt", &encrypted]);
    assert!(output.status.success(), "Decryption after rotation failed: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "test value");
}
