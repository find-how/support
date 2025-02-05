# Middleware Guide

The HTTP facade supports a powerful middleware system that allows you to modify
requests and responses at various stages of their lifecycle.

## Creating Custom Middleware

To create custom middleware, implement the `Middleware` trait:

```rust
use http::{Middleware, Request, Response, HttpFacadeError};
use async_trait::async_trait;

struct AuthMiddleware {
    api_key: String,
}

#[async_trait]
impl Middleware for AuthMiddleware {
    async fn handle_request(&self, request: &mut Request) -> Result<(), HttpFacadeError> {
        request.headers.push(("Authorization".into(), format!("Bearer {}", self.api_key)));
        Ok(())
    }

    async fn handle_response(&self, request: &Request, response: &mut Response) -> Result<(), HttpFacadeError> {
        // Process response if needed
        Ok(())
    }
}
```

## Middleware Execution Order

Middlewares are executed in the order they are added to the facade:

```rust
let mut client = HttpFacade::new();

// First middleware to execute
client.add_middleware(LoggingMiddleware::new(log::Level::Info));

// Second middleware to execute
client.add_middleware(AuthMiddleware { 
    api_key: "secret".into() 
});
```

## Conditional Middleware

You can control when middleware executes by implementing `should_execute`:

```rust
#[async_trait]
impl Middleware for AuthMiddleware {
    fn should_execute(&self, request: &Request) -> bool {
        // Only execute for authenticated endpoints
        request.url.starts_with("/api/")
    }
    
    // ... rest of implementation
}
```

## Built-in Middlewares

1. LoggingMiddleware - Logs requests and responses
2. RetryMiddleware - Automatically retries failed requests
3. CacheMiddleware - Caches responses for improved performance

## Best Practices

1. Keep middleware focused on a single responsibility
2. Use the `should_execute` method to optimize performance
3. Handle errors appropriately and provide context
4. Consider middleware ordering carefully
5. Document middleware behavior and requirements
