use assert_fs::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_basic_operations() {
    // Set up encryption keys
    std::env::set_var("ENCRYPTION_KEY", "MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI="); // 32 bytes base64
    std::env::set_var("ENCRYPTION_IV", "MTIzNDU2Nzg5MDEyMzQ1Ng=="); // 16 bytes base64

    let temp = assert_fs::TempDir::new().unwrap();
    let settings_dir = temp.child("settings");
    settings_dir.create_dir_all().unwrap();

    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("set")
        .arg("test.key")
        .arg("\"test value\"")
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());

    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("get")
        .arg("test.key")
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "\"test value\""
    );
}

#[test]
fn test_json_values() {
    // Set up encryption keys
    std::env::set_var("ENCRYPTION_KEY", "MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=");
    std::env::set_var("ENCRYPTION_IV", "MTIzNDU2Nzg5MDEyMzQ1Ng==");

    let temp = assert_fs::TempDir::new().unwrap();
    let settings_dir = temp.child("settings");
    settings_dir.create_dir_all().unwrap();

    let json_value = r#"{"name": "test", "value": 123}"#;

    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("set")
        .arg("test.json")
        .arg(json_value)
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());

    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("get")
        .arg("test.json")
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        json_value
    );
}

#[test]
fn test_backup_restore() {
    // Set up encryption keys
    std::env::set_var("ENCRYPTION_KEY", "MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=");
    std::env::set_var("ENCRYPTION_IV", "MTIzNDU2Nzg5MDEyMzQ1Ng==");

    let temp = assert_fs::TempDir::new().unwrap();
    let settings_dir = temp.child("settings");
    settings_dir.create_dir_all().unwrap();

    // Set some values
    Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("set")
        .arg("test.key1")
        .arg("\"value1\"")
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("set")
        .arg("test.key2")
        .arg("\"value2\"")
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    // Create backup
    let backup_file = temp.child("backup.dat");
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("backup")
        .arg(backup_file.path().to_str().unwrap())
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());

    // Clear settings
    settings_dir.remove_dir_all().unwrap();
    settings_dir.create_dir_all().unwrap();

    // Restore from backup
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("restore")
        .arg(backup_file.path().to_str().unwrap())
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());

    // Verify values
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("get")
        .arg("test.key1")
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "\"value1\""
    );

    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("get")
        .arg("test.key2")
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "\"value2\""
    );
}

#[test]
fn test_list_and_delete() {
    // Set up encryption keys
    std::env::set_var("ENCRYPTION_KEY", "MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=");
    std::env::set_var("ENCRYPTION_IV", "MTIzNDU2Nzg5MDEyMzQ1Ng==");

    let temp = assert_fs::TempDir::new().unwrap();
    let settings_dir = temp.child("settings");
    settings_dir.create_dir_all().unwrap();

    // Set some values
    Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("set")
        .arg("test.key1")
        .arg("\"value1\"")
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("set")
        .arg("test.key2")
        .arg("\"value2\"")
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    // List all settings
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("list")
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let output_str = String::from_utf8_lossy(&output.stdout);
    assert!(output_str.contains("test.key1: \"value1\""));
    assert!(output_str.contains("test.key2: \"value2\""));

    // Delete a setting
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("delete")
        .arg("test.key1")
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());

    // Verify deletion
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("get")
        .arg("test.key1")
        .current_dir(&temp)
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
}
