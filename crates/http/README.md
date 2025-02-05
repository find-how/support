# HTTP Client Facade

A Laravel-inspired HTTP client facade providing a clean interface for making
HTTP requests with middleware support, mocking capabilities, and robust error
handling.

## Features

- Middleware pipeline for request/response processing
- Mocking support for testing
- Comprehensive error handling
- Connection pooling and reuse
- Async/await support
- Integration with workspace crates (config, encrypt, store)

## Usage

Add to your Cargo.toml:

```toml
[dependencies]
http = { path = "../http" }
```

Basic example:

```rust
use http::{HttpFacade, LoggingMiddleware};

#[tokio::main]
async fn main() {
    let mut client = HttpFacade::new();
    client.add_middleware(LoggingMiddleware::new(log::Level::Info));
    
    // Make requests
    let response = client.get("https://api.example.com/users")
        .await
        .expect("Failed to make request");
}
```

## Middleware

The middleware system allows you to inject custom logic before and after
requests:

```rust
use http::{Middleware, Request, Response, HttpFacadeError};
use async_trait::async_trait;

struct CustomMiddleware;

#[async_trait]
impl Middleware for CustomMiddleware {
    async fn handle_request(&self, request: &mut Request) -> Result<(), HttpFacadeError> {
        // Modify request
        Ok(())
    }

    async fn handle_response(&self, request: &Request, response: &mut Response) -> Result<(), HttpFacadeError> {
        // Modify response
        Ok(())
    }
}
```

## Testing

The facade includes comprehensive mocking support:

```rust
let facade = HttpFacade::new();

// Mock exact URL
facade.mock_url("/users/1")
    .respond_with_json(200, &serde_json::json!({"id": 1}))
    .await;

// Mock pattern
facade.mock_pattern(r"/users/\d+")
    .unwrap()
    .respond_with_json(200, &serde_json::json!({"id": 1}))
    .await;
```

## Error Handling

The crate uses thiserror for comprehensive error handling:

```rust
match client.get("/users").await {
    Ok(response) => println!("Success: {}", response.status),
    Err(HttpFacadeError::ConnectionError { kind, details }) => {
        println!("Connection failed: {:?} - {}", kind, details);
    }
    Err(e) => println!("Other error: {}", e),
}
```

## Laravel Equivalents

This crate aims to provide similar functionality to Laravel's HTTP client:

| Laravel Feature | Rust Equivalent    |
| --------------- | ------------------ |
| Middleware      | `Middleware` trait |
| Macros          | Builder pattern    |
| Responses       | `Response` struct  |
| Fake Responses  | Mock system        |
| Retry           | Retryable errors   |

## Benchmarks

Run benchmarks with:

```bash
cargo bench
```

## Testing

Run tests with:

```bash
cargo test
```

## Contributing

1. Ensure tests pass
2. Add tests for new features
3. Update documentation
4. Follow Rust naming conventions
5. Maintain Laravel-inspired interfaces
