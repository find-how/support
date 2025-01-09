# Settings CLI

A secure settings management CLI for securely projects. This tool provides
encrypted storage of settings with a hierarchical structure and dot notation
access.

## Features

- Secure storage with AES-256-CBC encryption
- Hierarchical settings with dot notation access
- JSON value support
- Interactive shell mode
- Settings backup and restore
- Hierarchical visualization
- Namespace-based filtering

## Installation

```bash
cargo install --path .
```

## Configuration

Before using the CLI, you need to set up encryption keys:

```bash
# Generate a random 32-byte key and 16-byte IV
export ENCRYPTION_KEY=$(openssl rand -base64 32)
export ENCRYPTION_IV=$(openssl rand -base64 16)
```

## Usage

### Command Line Interface

```bash
# Get a setting
settings get app.name

# Set a setting
settings set app.name "MyApp"
settings set database.config '{"host": "localhost", "port": 5432}'

# Delete a setting
settings delete app.name

# List all settings
settings list

# List settings under a namespace
settings list app

# Backup settings
settings backup settings.bak

# Restore settings
settings restore settings.bak

# Visualize settings hierarchy
settings visualize

# Enter interactive shell
settings shell
```

### Interactive Shell

The interactive shell provides a more convenient way to manage settings:

```bash
$ settings shell
> help
Available commands:
  get <key>          Get a setting value
  set <key> <value>  Set a setting value
  delete <key>       Delete a setting
  list [namespace]   List settings
  backup <file>      Backup settings to file
  restore <file>     Restore settings from file
  visualize [ns]     Visualize settings hierarchy
  exit               Exit interactive mode
```

### Dot Notation

Settings use dot notation to represent hierarchy:

```bash
settings set app.server.host localhost
settings set app.server.port 8080
settings set app.log.level debug
```

This creates a structure like:

```
app
  ├── server
  │   ├── host: localhost
  │   └── port: 8080
  └── log
      └── level: debug
```

### JSON Values

Values can be plain strings or JSON objects:

```bash
# String value
settings set app.name "MyApp"

# JSON object
settings set app.database '{
  "host": "localhost",
  "port": 5432,
  "credentials": {
    "username": "admin",
    "password": "secret"
  }
}'
```

## Security

- All settings are encrypted using AES-256-CBC
- Encryption keys must be provided via environment variables
- Keys are never stored on disk
- Backups contain encrypted data only

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Documentation

```bash
cargo doc --open
```

## License

MIT License
