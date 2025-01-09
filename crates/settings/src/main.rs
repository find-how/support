use clap::{Parser, Subcommand};
use std::process;
use settings::{Settings, Error};
use encrypt::Crypt;

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

fn main() {
    dotenv::dotenv().ok();

    // Initialize encryption
    if let Err(e) = Crypt::initialize() {
        eprintln!("Failed to initialize encryption: {}", e);
        process::exit(1);
    }

    let cli = Cli::parse();
    let settings = match Settings::new() {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("Failed to initialize settings: {}", e);
            process::exit(1);
        }
    };

    match cli.command {
        Commands::Get { key } => {
            match settings.get::<serde_json::Value>(&key) {
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
            // Parse as JSON
            let json_value: serde_json::Value = match serde_json::from_str(&value) {
                Ok(v) => v,
                Err(_) => serde_json::Value::String(value),
            };

            if let Err(e) = settings.set(&key, json_value) {
                eprintln!("Failed to set setting: {}", e);
                process::exit(1);
            }
            println!("Setting '{}' updated", key);
        }
        Commands::Delete { key } => {
            if let Err(e) = settings.forget(&key) {
                eprintln!("Failed to delete setting: {}", e);
                process::exit(1);
            }
            println!("Setting '{}' deleted", key);
        }
        Commands::List { namespace } => {
            match settings.all(namespace.as_deref()) {
                Ok(values) => {
                    for (key, value) in values {
                        println!("{}: {}", key, value);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to list settings: {}", e);
                    process::exit(1);
                }
            }
        }
        Commands::Backup { file } => {
            // TODO: Implement backup
            eprintln!("Backup not yet implemented");
            process::exit(1);
        }
        Commands::Restore { file } => {
            // TODO: Implement restore
            eprintln!("Restore not yet implemented");
            process::exit(1);
        }
        Commands::Visualize { namespace } => {
            // TODO: Implement visualization
            eprintln!("Visualization not yet implemented");
            process::exit(1);
        }
        Commands::Shell => {
            // TODO: Implement interactive shell
            eprintln!("Interactive shell not yet implemented");
            process::exit(1);
        }
    }
}
