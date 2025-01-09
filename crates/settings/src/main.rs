use aes::Aes256;
use base64::Engine;
use block_modes::{BlockMode, Cbc};
use block_modes::block_padding::Pkcs7;
use clap::{Parser, Subcommand};
use lazy_static::lazy_static;
use petgraph::Graph;
use petgraph::dot::{Config, Dot};
use serde_json::Value;
use sled::Db;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::process;
use std::io::{self, Write};
use std::sync::Mutex;
use bincode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sled::Error),
    #[error("Serialization error")]
    SerializationError(#[from] bincode::Error),
    #[error("Missing encryption key")]
    MissingKey,
    #[error("Encryption error")]
    EncryptionError,
    #[error("Decryption error")]
    DecryptionError,
    #[error("Invalid key format")]
    InvalidKeyFormat,
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
}

type Aes256Cbc = Cbc<Aes256, Pkcs7>;

lazy_static! {
    static ref ENCRYPTION_KEY: Mutex<Vec<u8>> = Mutex::new(vec![0u8; 32]);
    static ref ENCRYPTION_IV: Mutex<Vec<u8>> = Mutex::new(vec![0u8; 16]);
}

fn initialize_encryption() -> Result<(), String> {
    let key_str = env::var("ENCRYPTION_KEY")
        .map_err(|_| "ENCRYPTION_KEY environment variable not set".to_string())?;
    let iv_str = env::var("ENCRYPTION_IV")
        .map_err(|_| "ENCRYPTION_IV environment variable not set".to_string())?;

    let key_bytes = base64::engine::general_purpose::STANDARD.decode(&key_str)
        .map_err(|e| format!("Invalid ENCRYPTION_KEY: {}", e))?;
    let iv_bytes = base64::engine::general_purpose::STANDARD.decode(&iv_str)
        .map_err(|e| format!("Invalid ENCRYPTION_IV: {}", e))?;

    if key_bytes.len() != 32 {
        return Err("ENCRYPTION_KEY must be 32 bytes when decoded".to_string());
    }
    if iv_bytes.len() != 16 {
        return Err("ENCRYPTION_IV must be 16 bytes when decoded".to_string());
    }

    *ENCRYPTION_KEY.lock().unwrap() = key_bytes;
    *ENCRYPTION_IV.lock().unwrap() = iv_bytes;

    Ok(())
}

fn encrypt_value(value: &[u8]) -> Result<String, SettingsError> {
    let key = ENCRYPTION_KEY.lock().unwrap();
    let _iv = ENCRYPTION_IV.lock().unwrap();

    let cipher = Aes256Cbc::new_from_slices(&key, &_iv)
        .map_err(|_| SettingsError::EncryptionError)?;

    let ciphertext = cipher.encrypt_vec(value);
    let mut combined = _iv.to_vec();
    combined.extend_from_slice(&ciphertext);

    Ok(base64::engine::general_purpose::STANDARD.encode(combined))
}

fn decrypt_value(encrypted: &str) -> Result<Vec<u8>, SettingsError> {
    let key = ENCRYPTION_KEY.lock().unwrap();
    let iv = ENCRYPTION_IV.lock().unwrap();

    let encrypted_data = base64::engine::general_purpose::STANDARD.decode(encrypted)
        .map_err(|_| SettingsError::DecryptionError)?;

    if encrypted_data.len() < 16 {
        return Err(SettingsError::DecryptionError);
    }

    let (iv, ciphertext) = encrypted_data.split_at(16);
    let cipher = Aes256Cbc::new_from_slices(&key, iv)
        .map_err(|_| SettingsError::DecryptionError)?;

    cipher.decrypt_vec(ciphertext)
        .map_err(|_| SettingsError::DecryptionError)
}

#[derive(Parser)]
#[command(name = "settings")]
#[command(about = "Secure settings management CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Get a setting value
    Get {
        /// The dot notation key
        key: String,
    },
    /// Set a setting value
    Set {
        /// The dot notation key
        key: String,
        /// The value (must be valid JSON)
        value: String,
    },
    /// Delete a setting
    Delete {
        /// The dot notation key
        key: String,
    },
    /// List all settings or settings under a namespace
    List {
        /// Optional namespace
        namespace: Option<String>,
    },
    /// Backup settings to a file
    Backup {
        /// The backup file path
        file: String,
    },
    /// Restore settings from a backup file
    Restore {
        /// The backup file path
        file: String,
    },
    /// Visualize settings hierarchy
    Visualize {
        /// Optional namespace to visualize
        namespace: Option<String>,
    },
    /// Enter interactive shell mode
    Shell,
}

fn open_db() -> sled::Result<Db> {
    let db_path = env::current_dir()
        .expect("Failed to get current directory")
        .join("settings")
        .join("db");
    fs::create_dir_all(&db_path).expect("Failed to create config directory");
    sled::open(db_path)
}

fn main() {
    dotenv::dotenv().ok();

    if let Err(e) = initialize_encryption() {
        eprintln!("Failed to initialize encryption: {}", e);
        process::exit(1);
    }

    let cli = Cli::parse();
    let db = match open_db() {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open database: {}", e);
            process::exit(1);
        }
    };

    match cli.command {
        Commands::Get { key } => {
            match get_setting(&db, &key) {
                Ok(Some(value)) => println!("{}", value),
                Ok(None) => {
                    eprintln!("Setting '{}' not found", key);
                    process::exit(1);
                }
                Err(e) => {
                    eprintln!("Failed to get setting: {}", e);
                    process::exit(1);
                }
            }
        }
        Commands::Set { key, value } => {
            if let Err(e) = set_setting(&db, &key, &value) {
                eprintln!("Failed to set setting: {}", e);
                process::exit(1);
            }
            println!("Setting '{}' updated", key);
        }
        Commands::Delete { key } => {
            if let Err(e) = delete_setting(&db, &key) {
                eprintln!("Failed to delete setting: {}", e);
                process::exit(1);
            }
            println!("Setting '{}' deleted", key);
        }
        Commands::List { namespace } => {
            if let Err(e) = list_settings(&db, namespace.as_deref()) {
                eprintln!("Failed to list settings: {}", e);
                process::exit(1);
            }
        }
        Commands::Backup { file } => {
            if let Err(e) = backup_settings(&db, &file) {
                eprintln!("Failed to backup settings: {}", e);
                process::exit(1);
            }
            println!("Settings backed up to '{}'", file);
        }
        Commands::Restore { file } => {
            if let Err(e) = restore_settings(&db, &file) {
                eprintln!("Failed to restore settings: {}", e);
                process::exit(1);
            }
            println!("Settings restored from '{}'", file);
        }
        Commands::Visualize { namespace } => {
            if let Err(e) = visualize_settings(&db, namespace.as_deref()) {
                eprintln!("Failed to visualize settings: {}", e);
                process::exit(1);
            }
        }
        Commands::Shell => {
            interactive_mode(&db);
        }
    }
}

fn get_setting(db: &Db, key: &str) -> Result<Option<String>, String> {
    match db.get(key).map_err(|e| e.to_string())? {
        Some(bytes) => {
            let decrypted = decrypt_value(&String::from_utf8_lossy(&bytes))
                .map_err(|e| e.to_string())?;
            Ok(Some(String::from_utf8_lossy(&decrypted).into_owned()))
        }
        None => Ok(None),
    }
}

fn set_setting(db: &Db, key: &str, mut value: &str) -> Result<(), String> {
    // Handle quoted strings
    if value.starts_with('"') && value.ends_with('"') {
        value = &value[1..value.len() - 1];
    }

    // Try to parse as JSON, if it fails, treat as plain string
    let json_value = match serde_json::from_str::<Value>(value) {
        Ok(v) => v,
        Err(_) => Value::String(value.to_string()),
    };

    let value_str = json_value.to_string();
    let encrypted = encrypt_value(value_str.as_bytes())
        .map_err(|e| e.to_string())?;

    db.insert(key.as_bytes(), encrypted.as_bytes())
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn delete_setting(db: &Db, key: &str) -> Result<(), String> {
    match db.remove(key.as_bytes()) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(format!("Setting '{}' not found", key)),
        Err(e) => Err(e.to_string()),
    }
}

fn list_settings(db: &Db, namespace: Option<&str>) -> Result<(), String> {
    let mut settings = HashMap::new();

    for result in db.iter() {
        let (key, value) = result.map_err(|e| e.to_string())?;
        let key_str = String::from_utf8_lossy(&key).into_owned();

        if let Some(ns) = namespace {
            if !key_str.starts_with(ns) {
                continue;
            }
        }

        let decrypted = decrypt_value(&String::from_utf8_lossy(&value))
            .map_err(|e| e.to_string())?;
        let value_str = String::from_utf8_lossy(&decrypted).into_owned();

        settings.insert(key_str, value_str);
    }

    for (key, value) in settings.iter() {
        println!("{}: {}", key, value);
    }

    Ok(())
}

fn backup_settings(db: &Db, file: &str) -> Result<(), String> {
    let mut backup = HashMap::new();

    for result in db.iter() {
        let (key, value) = result.map_err(|e| e.to_string())?;
        backup.insert(
            String::from_utf8_lossy(&key).into_owned(),
            String::from_utf8_lossy(&value).into_owned(),
        );
    }

    let serialized = bincode::serialize(&backup)
        .map_err(|e| e.to_string())?;

    fs::write(file, serialized)
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn restore_settings(db: &Db, file: &str) -> Result<(), String> {
    let data = fs::read(file)
        .map_err(|e| e.to_string())?;

    let backup: HashMap<String, String> = bincode::deserialize(&data)
        .map_err(|e| e.to_string())?;

    db.clear()
        .map_err(|e| e.to_string())?;

    for (key, value) in backup {
        db.insert(key.as_bytes(), value.as_bytes())
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn visualize_settings(db: &Db, namespace: Option<&str>) -> Result<(), String> {
    let mut graph = Graph::<String, ()>::new();
    let mut nodes = HashMap::new();

    // Add root node
    let root = graph.add_node("root".to_string());
    nodes.insert("root".to_string(), root);

    for result in db.iter() {
        let (key, _) = result.map_err(|e| e.to_string())?;
        let key_str = String::from_utf8_lossy(&key).into_owned();

        if let Some(ns) = namespace {
            if !key_str.starts_with(ns) {
                continue;
            }
        }

        let parts: Vec<&str> = key_str.split('.').collect();
        let mut parent = root;

        for (i, _part) in parts.iter().enumerate() {
            let path = parts[..=i].join(".");
            if !nodes.contains_key(&path) {
                let node = graph.add_node(path.clone());
                nodes.insert(path.clone(), node);
                graph.add_edge(parent, node, ());
            }
            parent = nodes[&path];
        }
    }

    println!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
    Ok(())
}

fn interactive_mode(db: &Db) {
    println!("Welcome to settings interactive shell");
    println!("Enter commands in the format: <get|set|delete|list|exit> [key] [value]");

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut buffer = String::new();

    loop {
        print!("> ");
        stdout.flush().unwrap();
        buffer.clear();

        if stdin.read_line(&mut buffer).is_err() {
            continue;
        }

        let parts: Vec<&str> = buffer.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "exit" => break,
            "get" if parts.len() == 2 => {
                match get_setting(db, parts[1]) {
                    Ok(Some(value)) => println!("{}", value),
                    Ok(None) => println!("Setting not found"),
                    Err(e) => println!("Error: {}", e),
                }
            },
            "set" if parts.len() >= 3 => {
                let value = parts[2..].join(" ");
                if let Err(e) = set_setting(db, parts[1], &value) {
                    println!("Error: {}", e);
                } else {
                    println!("Setting updated");
                }
            },
            "delete" if parts.len() == 2 => {
                if let Err(e) = delete_setting(db, parts[1]) {
                    println!("Error: {}", e);
                } else {
                    println!("Setting deleted");
                }
            },
            "list" => {
                let namespace = if parts.len() > 1 { Some(parts[1]) } else { None };
                if let Err(e) = list_settings(db, namespace) {
                    println!("Error: {}", e);
                }
            },
            _ => println!("Invalid command. Use: get <key> | set <key> <value> | delete <key> | list [namespace] | exit"),
        }
    }
}
