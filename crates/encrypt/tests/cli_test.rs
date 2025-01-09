use std::process::Command;

fn run_cmd(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg("encrypt")
        .arg("--")
        .args(args)
        .env("APP_KEY", "base64:dGVzdGtleXRlc3RrZXl0ZXN0a2V5dGVzdGtleXRlc3Q=")
        .output()
        .expect("Failed to execute command")
}

#[test]
fn test_encrypt_decrypt() {
    let value = "test value";

    // Encrypt
    let output = run_cmd(&["encrypt", value]);
    assert!(output.status.success());
    let encrypted = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Decrypt
    let output = run_cmd(&["decrypt", &encrypted]);
    assert!(output.status.success());
    let decrypted = String::from_utf8_lossy(&output.stdout).trim().to_string();

    assert_eq!(decrypted, value);
}

#[test]
fn test_base64_encode_decode() {
    let value = "test value";

    // Encode
    let output = run_cmd(&["encode", value, "--base64"]);
    assert!(output.status.success());
    let encoded = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Decode
    let output = run_cmd(&["decode", &encoded, "--base64"]);
    assert!(output.status.success());
    let decoded = String::from_utf8_lossy(&output.stdout).trim().to_string();

    assert_eq!(decoded, value);
}

#[test]
fn test_key_generation() {
    let output = run_cmd(&["generate-key"]);
    assert!(output.status.success());
    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(key.starts_with("base64:"));
}

#[test]
fn test_key_rotation() {
    // First encrypt with old key
    let value = "test value";
    let output = run_cmd(&["encrypt", value]);
    assert!(output.status.success());
    let encrypted = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Generate new key and rotate
    let output = run_cmd(&["rotate"]);
    assert!(output.status.success());

    // Should still be able to decrypt with old key through rotation
    let output = run_cmd(&["decrypt", &encrypted]);
    assert!(output.status.success());
    let decrypted = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(decrypted, value);
}
