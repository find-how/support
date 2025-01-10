use assert_fs::TempDir;
use std::process::Command;

#[test]
fn test_basic_operations() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("settings.db");

    // Test set
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("set")
        .arg("test.key")
        .arg("test value")
        .env("SETTINGS_DB", db_path.to_str().unwrap())
        .output()
        .unwrap();
    assert!(output.status.success());

    // Test get
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("get")
        .arg("test.key")
        .env("SETTINGS_DB", db_path.to_str().unwrap())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "test value");

    // Test has
    let output = Command::new("cargo")
        .arg("run")
        .arg("--")
        .arg("has")
        .arg("test.key")
        .env("SETTINGS_DB", db_path.to_str().unwrap())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "true");
}
