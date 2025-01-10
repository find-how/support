use clap::{Parser, Subcommand};
use std::sync::Arc;
use store::backends::sled::SledStore;
use settings::Settings;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let db_path = std::env::var("SETTINGS_DB").unwrap_or_else(|_| "settings.db".to_string());
    let store = SledStore::new(db_path).expect("Failed to create store");
    let settings = Settings::new(Arc::new(store));

    match cli.command {
        Commands::Get { key } => {
            match settings.get(&key).await {
                Ok(Some(value)) => println!("{}", value),
                Ok(None) => println!("Key not found"),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        Commands::Set { key, value } => {
            if let Err(e) = settings.set(&key, &value).await {
                eprintln!("Error: {}", e);
            }
        }
        Commands::Has { key } => {
            match settings.has(&key).await {
                Ok(exists) => println!("{}", exists),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Get a setting value
    Get {
        /// The key to get
        key: String,
    },
    /// Set a setting value
    Set {
        /// The key to set
        key: String,
        /// The value to set
        value: String,
    },
    /// Check if a setting exists
    Has {
        /// The key to check
        key: String,
    },
}
