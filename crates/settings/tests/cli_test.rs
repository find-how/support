use assert_fs::prelude::*;
use std::process::Command;
use std::env;
use serde_json;

fn get_binary_path() -> String {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    format!("{}/../../target/{}/settings", manifest_dir, profile)
}

#[test]
fn test_basic_operations() {
    // Set up encryption keys
    let key = "MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI="; // 32 bytes base64
    let iv = "MTIzNDU2Nzg5MDEyMzQ1Ng=="; // 16 bytes base64

    let temp = assert_fs::TempDir::new().unwrap();
    let settings_dir = temp.child("settings");
    settings_dir.create_dir_all().unwrap();

    let binary = get_binary_path();
    let output = Command::new(&binary)
        .arg("set")
        .arg("test.key")
        .arg("\"test value\"")
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());

    let output = Command::new(&binary)
        .arg("get")
        .arg("test.key")
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "\"test value\""
    );
}

#[test]
fn test_json_values() {
    let key = "MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
    let iv = "MTIzNDU2Nzg5MDEyMzQ1Ng==";

    let temp = assert_fs::TempDir::new().unwrap();
    let settings_dir = temp.child("settings");
    settings_dir.create_dir_all().unwrap();

    let binary = get_binary_path();
    let json_value = r#"{"name": "test", "value": 123}"#;

    let output = Command::new(&binary)
        .arg("set")
        .arg("test.json")
        .arg(json_value)
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());

    let output = Command::new(&binary)
        .arg("get")
        .arg("test.json")
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());

    // Parse and compare JSON values to ignore formatting differences
    let expected: serde_json::Value = serde_json::from_str(json_value).unwrap();
    let actual: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn test_backup_restore() {
    let key = "MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
    let iv = "MTIzNDU2Nzg5MDEyMzQ1Ng==";

    let temp = assert_fs::TempDir::new().unwrap();
    let settings_dir = temp.child("settings");
    settings_dir.create_dir_all().unwrap();

    let binary = get_binary_path();

    // Set some values
    let output = Command::new(&binary)
        .arg("set")
        .arg("test.key1")
        .arg("\"value1\"")
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());

    let output = Command::new(&binary)
        .arg("set")
        .arg("test.key2")
        .arg("\"value2\"")
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());

    // Create backup
    let backup_file = temp.child("backup.dat");
    let output = Command::new(&binary)
        .arg("backup")
        .arg(backup_file.path().to_str().unwrap())
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());

    // Clear settings
    std::fs::remove_dir_all(settings_dir.path()).unwrap();
    settings_dir.create_dir_all().unwrap();

    // Restore from backup
    let output = Command::new(&binary)
        .arg("restore")
        .arg(backup_file.path().to_str().unwrap())
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());

    // Verify values
    let output = Command::new(&binary)
        .arg("get")
        .arg("test.key1")
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "\"value1\""
    );

    let output = Command::new(&binary)
        .arg("get")
        .arg("test.key2")
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "\"value2\""
    );
}

#[test]
fn test_list_and_delete() {
    let key = "MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
    let iv = "MTIzNDU2Nzg5MDEyMzQ1Ng==";

    let temp = assert_fs::TempDir::new().unwrap();
    let settings_dir = temp.child("settings");
    settings_dir.create_dir_all().unwrap();

    let binary = get_binary_path();

    // Set some values
    let output = Command::new(&binary)
        .arg("set")
        .arg("test.key1")
        .arg("\"value1\"")
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());

    let output = Command::new(&binary)
        .arg("set")
        .arg("test.key2")
        .arg("\"value2\"")
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());

    // List all settings
    let output = Command::new(&binary)
        .arg("list")
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());
    let output_str = String::from_utf8_lossy(&output.stdout);
    assert!(output_str.contains("test.key1: \"value1\""));
    assert!(output_str.contains("test.key2: \"value2\""));

    // Delete a setting
    let output = Command::new(&binary)
        .arg("delete")
        .arg("test.key1")
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(output.status.success());

    // Verify deletion
    let output = Command::new(&binary)
        .arg("get")
        .arg("test.key1")
        .env("ENCRYPTION_KEY", key)
        .env("ENCRYPTION_IV", iv)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
}
