# Securely Encrypt

A secure encryption service for the Securely server, inspired by Laravel's
encryption system. This crate provides a simple, convenient interface for
encrypting and decrypting text using AES-256-CBC with HMAC-SHA256 message
authentication.

## Features

- AES-256-CBC encryption
- HMAC-SHA256 message authentication
- Base64-encoded keys and ciphertexts
- Graceful key rotation support
- Tamper detection
- Secure random key generation

## Usage

### Configuration

Before using the encrypter, you must set up your encryption keys:

```bash
# Current encryption key (required)
APP_KEY=base64:YourBase64EncodedKey

# Previous keys for graceful rotation (optional)
APP_PREVIOUS_KEYS=base64:OldKey1,base64:OldKey2
```

You can generate a secure key using the provided utility:

```rust
use encrypt::Crypt;

let new_key = Crypt::generate_key();
println!("Generated key: {}", new_key);
```

### Basic Usage

```rust
use encrypt::{Crypt, Error};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the encrypter (reads from environment)
    Crypt::initialize()?;

    // Encrypt a value
    let encrypted = Crypt::encrypt_string("my secret value")?;
    println!("Encrypted: {}", encrypted);

    // Decrypt a value
    let decrypted = Crypt::decrypt_string(&encrypted)?;
    assert_eq!(decrypted, "my secret value");

    Ok(())
}
```

### Key Rotation

You can gracefully rotate encryption keys by:

1. Generate a new key
2. Set it as the current `APP_KEY`
3. Move the old key to `APP_PREVIOUS_KEYS`

```rust
// Generate new key
let new_key = Crypt::generate_key();

// Update environment
std::env::set_var("APP_KEY", new_key);
std::env::set_var("APP_PREVIOUS_KEYS", old_key);

// Reinitialize the encrypter
Crypt::initialize()?;

// Old encrypted values can still be decrypted
let decrypted = Crypt::decrypt_string(&old_encrypted)?;
```

### Security Features

1. **Message Authentication**: All encrypted values are signed with HMAC-SHA256
   to prevent tampering
2. **Secure Key Format**: Keys must be 32 bytes (256 bits) and base64 encoded
3. **Random IVs**: Each encryption operation uses a secure random IV
4. **Tamper Detection**: Modified ciphertexts will fail to decrypt
5. **Type Safety**: Strong Rust types and error handling

## Error Handling

The crate provides typed errors for different failure cases:

```rust
use encrypt::{Crypt, Error};

match Crypt::decrypt_string(&encrypted) {
    Ok(value) => println!("Decrypted: {}", value),
    Err(Error::MissingKey) => eprintln!("APP_KEY not set"),
    Err(Error::InvalidFormat) => eprintln!("Invalid ciphertext format"),
    Err(Error::InvalidMac) => eprintln!("Ciphertext has been tampered with"),
    Err(Error::Decryption) => eprintln!("Decryption failed"),
    Err(e) => eprintln!("Other error: {}", e),
}
```

## Testing

Run the test suite:

```bash
cargo test -p encrypt
```

The test suite includes:

- Basic encryption/decryption
- Key rotation
- Tamper detection
- Key generation
- Error cases

## Development

### Building

```bash
cargo build -p encrypt
```

### Linting

```bash
cargo clippy -p encrypt
```
