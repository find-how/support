# Securely Settings

A secure settings management system for Securely server extensions and plugins.
This crate provides encrypted storage of settings with hierarchical organization
and dot notation access, specifically designed for managing server extension
state and configuration.

> **Note**: This crate is distinct from `securely_config`, which handles
> standard Laravel-style application configuration. The settings crate is
> specifically for managing server extension state, plugins, and modules with
> strict schema validation and automatic API generation.

## Features

Current:

- AES-256-CBC encryption for all stored values
- JSON value support with type preservation
- Dot notation for hierarchical settings access
- Backup and restore functionality
- Settings hierarchy visualization
- Interactive shell mode
- Comprehensive test coverage

Planned:

- Schema validation using `schemars`
- Lifecycle hooks for get/set operations
- Field-level metadata via custom derive macros
- Automatic generation of:
  - JSON Schema
  - OpenAPI specifications
  - GraphQL schema
  - Protocol Buffer definitions
  - Visualization graphs (using `petgraph`)
  - EGUI admin interfaces
- Validation rules and constraints
- Extension state management
- Plugin configuration validation
- Module dependency resolution

## Extension Development

When developing extensions for the Securely server, you'll define your settings
using Rust structs with schema validation:

```rust
use schemars::JsonSchema;
use securely_settings_derive::Setting;

#[derive(Setting, JsonSchema)]
#[setting(name = "email", description = "Email service configuration")]
struct EmailSettings {
    #[setting(name = "SMTP Host", description = "SMTP server hostname")]
    host: String,

    #[setting(name = "SMTP Port", description = "SMTP server port")]
    #[setting(validate = "port > 0 && port < 65536")]
    port: u16,

    #[setting(name = "Username", description = "SMTP authentication username")]
    username: String,

    #[setting(secret = true, name = "Password", description = "SMTP authentication password")]
    password: String,

    #[setting(name = "TLS", description = "Use TLS for connection")]
    #[setting(default = true)]
    use_tls: bool,
}
```

This will automatically:

1. Generate JSON Schema for validation
2. Create OpenAPI endpoints for configuration
3. Add GraphQL types and queries
4. Generate Protocol Buffer messages
5. Create visualization nodes
6. Build EGUI interface components

## Usage

### Environment Setup

The following environment variables are required:

```bash
ENCRYPTION_KEY=<base64-encoded-32-byte-key>
ENCRYPTION_IV=<base64-encoded-16-byte-iv>
```

### Using in Extensions

Add to your `Cargo.toml`:

```toml
[dependencies]
settings = { path = "../crates/settings" }
securely_settings_derive = { path = "../crates/settings_derive" }
schemars = "0.8"
```

Basic usage:

```rust
use settings::{Settings, Error};
use serde::{Serialize, Deserialize};
use schemars::JsonSchema;
use securely_settings_derive::Setting;

#[derive(Serialize, Deserialize, JsonSchema, Setting)]
#[setting(name = "Database", description = "Database connection settings")]
struct DatabaseConfig {
    #[setting(name = "Host", description = "Database hostname")]
    host: String,

    #[setting(name = "Port", description = "Database port")]
    #[setting(validate = "port > 0 && port < 65536")]
    port: u16,

    #[setting(name = "Username", description = "Database username")]
    username: String,

    #[setting(name = "Password", description = "Database password", secret = true)]
    password: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize settings with default sled database
    let settings = Settings::new()?;

    // Settings will be validated against schema
    let db_config = DatabaseConfig {
        host: "localhost".to_string(),
        port: 5432,
        username: "admin".to_string(),
        password: "secret".to_string(),
    };

    // This will:
    // 1. Validate against schema
    // 2. Run pre-set hooks
    // 3. Encrypt and store
    // 4. Run post-set hooks
    settings.set("database", db_config)?;

    // This will:
    // 1. Run pre-get hooks
    // 2. Decrypt and deserialize
    // 3. Validate against schema
    // 4. Run post-get hooks
    let config: DatabaseConfig = settings.get("database")?.unwrap();

    Ok(())
}
```

Using with custom database driver:

```rust
use settings::Settings;
use sled::Config;

// Use custom sled configuration
let db = Config::new()
    .path("custom/path")
    .cache_capacity(1024 * 1024)
    .open()?;

let settings = Settings::driver(db);
```

Using in async contexts:

```rust
use settings::Settings;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::new()?;

    // Settings operations are sync, wrap in spawn_blocking
    let value = tokio::task::spawn_blocking(move || {
        settings.get::<String>("app.name")
    }).await??;

    println!("Value: {:?}", value);
    Ok(())
}
```

### Command Line Interface

| Command                 | Description                          | Example                                   |
| ----------------------- | ------------------------------------ | ----------------------------------------- |
| `set <key> <value>`     | Set a setting value                  | `securely settings set app.name "My App"` |
| `get <key>`             | Get a setting value                  | `securely settings get app.name`          |
| `delete <key>`          | Delete a setting                     | `securely settings delete app.name`       |
| `list [namespace]`      | List all settings or under namespace | `securely settings list app`              |
| `backup <file>`         | Backup settings to file              | `securely settings backup settings.bak`   |
| `restore <file>`        | Restore settings from file           | `securely settings restore settings.bak`  |
| `visualize [namespace]` | Visualize settings hierarchy         | `securely settings visualize app`         |
| `shell`                 | Enter interactive shell              | `securely settings shell`                 |

### Interactive Shell

The interactive shell provides a REPL interface for managing settings:

```bash
> set app.name "My App"
Setting updated
> get app.name
"My App"
> list
app.name: "My App"
> exit
```

## Security

- All values are encrypted using AES-256-CBC before storage
- Encryption keys are required via environment variables
- No plaintext storage of sensitive data
- Secure backup format with encrypted values

## Testing

Run the test suite:

```bash
cargo test -p settings
```

The test suite includes:

- Unit tests for core functionality
- Integration tests for CLI commands
- JSON value handling tests
- Backup/restore tests
- Error case tests

## Development

### Building

```bash
cargo build -p settings
```

### Running Tests with Coverage

```bash
cargo tarpaulin -p settings
```

### Linting

```bash
cargo clippy -p settings
```
