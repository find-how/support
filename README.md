# Support Crates

[![Crates.io](https://img.shields.io/crates/v/support.svg)](https://crates.io/crates/support)
[![Documentation](https://docs.rs/support/badge.svg)](https://docs.rs/support)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

This repository contains support crates for
[securely](https://github.com/find-how/securely) projects. It provides a
collection of utilities and tools designed to enhance security and
maintainability across our ecosystem.

## Structure

The repository is organized as a Rust workspace with multiple crates:

- `settings`: Configuration management with encryption support
- `testing`: Common testing utilities, fixtures, and helpers
- _(More crates will be added as needed)_

## Getting Started

Add the desired crate to your `Cargo.toml`:

```toml
[dependencies]
settings = { version = "0.1.0" }

[dev-dependencies]
testing = { version = "0.1.0" }
```

### Using the Testing Utilities

The `testing` crate provides common utilities for testing securely projects:

````rust
use testing::{TestRuntime, assert, temp};

#[test]
fn my_test() {
    // Use the test runtime for async tests
    let rt = TestRuntime::new();
    
    // Create temporary test directories/files
    let test_dir = temp::dir(Some("my-test"));
    
    // Use enhanced assertions
    assert::completes_within(async { /* ... */ }, Duration::from_secs(1));
}

## Development

### Prerequisites

- Rust 1.70.0 or higher
- Cargo

### Building

```bash
cargo build --workspace
````

### Testing

```bash
cargo test --workspace
```

### Documentation

Generate and view the documentation locally:

```bash
cargo doc --workspace --no-deps --open
```

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -am 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file
for details.
