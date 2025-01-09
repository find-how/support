use clap::{Parser, Subcommand};
use std::process;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

#[derive(Parser)]
#[command(name = "crypt")]
#[command(about = "Encryption utilities for Securely", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Encrypt a string value
    Encrypt {
        /// The string to encrypt
        value: String,
    },
    /// Decrypt a string value
    Decrypt {
        /// The encrypted string
        value: String,
    },
    /// Generate a new encryption key
    GenerateKey,
    /// Encode a string value
    Encode {
        /// The string to encode
        value: String,
        /// Use base64 encoding
        #[arg(long)]
        base64: bool,
    },
    /// Decode a string value
    Decode {
        /// The string to decode
        value: String,
        /// Use base64 decoding
        #[arg(long)]
        base64: bool,
    },
    /// Rotate encryption keys
    Rotate {
        /// The new key to rotate to (optional, will generate if not provided)
        new_key: Option<String>,
    },
}

fn main() {
    // Load environment variables
    dotenv::dotenv().ok();

    let cli = Cli::parse();

    // Initialize encryption unless we're just doing encoding/decoding operations
    match &cli.command {
        Commands::Encode { .. } | Commands::Decode { .. } => {}
        _ => if let Err(e) = encrypt::Crypt::initialize() {
            eprintln!("Failed to initialize encryption: {}", e);
            process::exit(1);
        }
    }

    match cli.command {
        Commands::Encrypt { value } => {
            match encrypt::Crypt::encrypt_string(&value) {
                Ok(encrypted) => println!("{}", encrypted),
                Err(e) => {
                    eprintln!("Failed to encrypt: {}", e);
                    process::exit(1);
                }
            }
        }
        Commands::Decrypt { value } => {
            match encrypt::Crypt::decrypt_string(&value) {
                Ok(decrypted) => println!("{}", decrypted),
                Err(e) => {
                    eprintln!("Failed to decrypt: {}", e);
                    process::exit(1);
                }
            }
        }
        Commands::GenerateKey => {
            let key = encrypt::Crypt::generate_key();
            println!("{}", key);
        }
        Commands::Encode { value, base64 } => {
            if base64 {
                println!("{}", BASE64.encode(value.as_bytes()));
            } else {
                eprintln!("Please specify an encoding format (--base64)");
                process::exit(1);
            }
        }
        Commands::Decode { value, base64 } => {
            if base64 {
                match BASE64.decode(&value) {
                    Ok(decoded) => match String::from_utf8(decoded) {
                        Ok(string) => println!("{}", string),
                        Err(_) => {
                            eprintln!("Decoded value is not valid UTF-8");
                            process::exit(1);
                        }
                    },
                    Err(e) => {
                        eprintln!("Failed to decode base64: {}", e);
                        process::exit(1);
                    }
                }
            } else {
                eprintln!("Please specify a decoding format (--base64)");
                process::exit(1);
            }
        }
        Commands::Rotate { new_key } => {
            // Get current key
            let old_key = match std::env::var("APP_KEY") {
                Ok(key) => key,
                Err(_) => {
                    eprintln!("No current APP_KEY found");
                    process::exit(1);
                }
            };

            // Generate or use provided new key
            let new_key = new_key.unwrap_or_else(|| encrypt::Crypt::generate_key());

            println!("Rotating encryption keys...");
            println!("Old key: {}", old_key);
            println!("New key: {}", new_key);
            println!("\nUpdate your environment with:");
            println!("APP_KEY={}", new_key);
            println!("APP_PREVIOUS_KEYS={}", old_key);
        }
    }
}
