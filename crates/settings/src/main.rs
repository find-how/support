use aes::Aes256;
use block_modes::{BlockMode, Cbc};
use block_modes::block_padding::Pkcs7;
use clap::{Parser, Subcommand};
use dirs::config_dir;
use lazy_static::lazy_static;
use petgraph::dot::{Config, Dot};
use petgraph::graphmap::DiGraphMap;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sled::{Db};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::sync::Mutex;
use std::io::{self, Write};

type Aes256Cbc = Cbc<Aes256, Pkcs7>;

lazy_static! {
    static ref ENCRYPTION_KEY: Mutex<[u8; 32]> = Mutex::new([0u8; 32]);
    static ref ENCRYPTION_IV: Mutex<[u8; 16]> = Mutex::new([0u8; 16]);
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

fn initialize_encryption() -> Result<(), String> {
    let key_str = env::var("ENCRYPTION_KEY").map_err(|_| "ENCRYPTION_KEY not set")?;
    let iv_str = env::var("ENCRYPTION_IV").map_err(|_| "ENCRYPTION_IV not set")?;

    let key_bytes = base64::decode(&key_str).map_err(|e| format!("Invalid ENCRYPTION_KEY: {}", e))?;
    let iv_bytes = base64::decode(&iv_str).map_err(|e| format!("Invalid ENCRYPTION_IV: {}", e))?;

    if key_bytes.len() != 32 {
        return Err("ENCRYPTION_KEY must be 32 bytes when decoded".to_string());
    }
    if iv_bytes.len() != 16 {
        return Err("ENCRYPTION_IV must be 16 bytes when decoded".to_string());
    }

    let mut key = ENCRYPTION_KEY.lock().unwrap();
    key.copy_from_slice(&key_bytes);

    let mut iv = ENCRYPTION_IV.lock().unwrap();
    iv.copy_from_slice(&iv_bytes);

    Ok(())
}

fn encrypt(plaintext: &str) -> Result<Vec<u8>, String> {
    let key = ENCRYPTION_KEY.lock().unwrap();
    let iv = ENCRYPTION_IV.lock().unwrap();
    let cipher = Aes256Cbc::new_from_slices(&key, &iv)
        .map_err(|e| e.to_string())?;
    Ok(cipher.encrypt_vec(plaintext.as_bytes()))
}

fn decrypt(ciphertext: &[u8]) -> Result<String, String> {
    let key = ENCRYPTION_KEY.lock().unwrap();
    let iv = ENCRYPTION_IV.lock().unwrap();
    let cipher = Aes256Cbc::new_from_slices(&key, &iv)
        .map_err(|e| e.to_string())?;
    let decrypted = cipher.decrypt_vec(ciphertext)
        .map_err(|e| e.to_string())?;
    String::from_utf8(decrypted)
        .map_err(|e| e.to_string())
}

fn open_db() -> sled::Result<Db> {
    let config_dir = config_dir().unwrap_or_else(|| PathBuf::from("."));
    let db_path = config_dir.join("settings").join("db");
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
            let decrypted = decrypt(&bytes)?;
            Ok(Some(decrypted))
        }
        None => Ok(None),
    }
}

fn set_setting(db: &Db, key: &str, value: &str) -> Result<(), String> {
    // Validate JSON if the value is meant to be JSON
    if value.starts_with('{') || value.starts_with('[') {
        serde_json::from_str::<Value>(value)
            .map_err(|e| format!("Invalid JSON: {}", e))?;
    }

    let encrypted = encrypt(value)?;
    db.insert(key, encrypted)
        .map_err(|e| e.to_string())?;
    db.flush()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn delete_setting(db: &Db, key: &str) -> Result<(), String> {
    db.remove(key)
        .map_err(|e| e.to_string())?;
    db.flush()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn list_settings(db: &Db, namespace: Option<&str>) -> Result<(), String> {
    let prefix = namespace.unwrap_or("");
    for item in db.scan_prefix(prefix.as_bytes()) {
        let (key, value) = item.map_err(|e| e.to_string())?;
        let key_str = String::from_utf8(key.to_vec())
            .map_err(|e| e.to_string())?;
        let value_str = decrypt(&value)?;
        println!("{}: {}", key_str, value_str);
    }
    Ok(())
}

fn backup_settings(db: &Db, file: &str) -> Result<(), String> {
    let snapshot = db.export()
        .map_err(|e| e.to_string())?;
    fs::write(file, snapshot)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn restore_settings(db: &Db, file: &str) -> Result<(), String> {
    let data = fs::read(file)
        .map_err(|e| e.to_string())?;
    db.import(&data)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn visualize_settings(db: &Db, namespace: Option<&str>) -> Result<(), String> {
    let mut graph = DiGraphMap::new();
    let prefix = namespace.unwrap_or("");

    for item in db.scan_prefix(prefix.as_bytes()) {
        let (key, value) = item.map_err(|e| e.to_string())?;
        let key_str = String::from_utf8(key.to_vec())
            .map_err(|e| e.to_string())?;
        let value_str = decrypt(&value)?;

        let parts: Vec<&str> = key_str.split('.').collect();
        for i in 0..parts.len() {
            let parent = if i > 0 {
                parts[..i].join(".")
            } else {
                String::new()
            };
            let child = parts[..=i].join(".");

            if !parent.is_empty() {
                graph.add_edge(&parent, &child, ());
            }

            if i == parts.len() - 1 {
                // Add value as node label for leaf nodes
                graph.add_edge(&child, &format!("{}: {}", child, value_str), ());
            }
        }
    }

    println!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
    Ok(())
}

fn interactive_mode(db: &Db) {
    println!("Entering interactive mode. Type 'help' for commands.");
    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            eprintln!("Failed to read input");
            continue;
        }

        let args: Vec<&str> = input.trim().split_whitespace().collect();
        if args.is_empty() {
            continue;
        }

        match args[0] {
            "exit" | "quit" => break,
            "help" => {
                println!("Available commands:");
                println!("  get <key>          Get a setting value");
                println!("  set <key> <value>  Set a setting value");
                println!("  delete <key>       Delete a setting");
                println!("  list [namespace]   List settings");
                println!("  backup <file>      Backup settings to file");
                println!("  restore <file>     Restore settings from file");
                println!("  visualize [ns]     Visualize settings hierarchy");
                println!("  exit               Exit interactive mode");
            }
            "get" => {
                if args.len() != 2 {
                    println!("Usage: get <key>");
                    continue;
                }
                match get_setting(db, args[1]) {
                    Ok(Some(value)) => println!("{}", value),
                    Ok(None) => println!("Setting '{}' not found", args[1]),
                    Err(e) => println!("Error: {}", e),
                }
            }
            "set" => {
                if args.len() < 3 {
                    println!("Usage: set <key> <value>");
                    continue;
                }
                let value = args[2..].join(" ");
                if let Err(e) = set_setting(db, args[1], &value) {
                    println!("Error: {}", e);
                } else {
                    println!("Setting '{}' updated", args[1]);
                }
            }
            "delete" => {
                if args.len() != 2 {
                    println!("Usage: delete <key>");
                    continue;
                }
                if let Err(e) = delete_setting(db, args[1]) {
                    println!("Error: {}", e);
                } else {
                    println!("Setting '{}' deleted", args[1]);
                }
            }
            "list" => {
                let namespace = args.get(1).copied();
                if let Err(e) = list_settings(db, namespace) {
                    println!("Error: {}", e);
                }
            }
            "backup" => {
                if args.len() != 2 {
                    println!("Usage: backup <file>");
                    continue;
                }
                if let Err(e) = backup_settings(db, args[1]) {
                    println!("Error: {}", e);
                } else {
                    println!("Settings backed up to '{}'", args[1]);
                }
            }
            "restore" => {
                if args.len() != 2 {
                    println!("Usage: restore <file>");
                    continue;
                }
                if let Err(e) = restore_settings(db, args[1]) {
                    println!("Error: {}", e);
                } else {
                    println!("Settings restored from '{}'", args[1]);
                }
            }
            "visualize" => {
                let namespace = args.get(1).copied();
                if let Err(e) = visualize_settings(db, namespace) {
                    println!("Error: {}", e);
                }
            }
            _ => println!("Unknown command: '{}'", args[0]),
        }
    }
}
