//! HTTP Client Facade
//!
//! A Laravel-inspired HTTP client facade providing a clean interface for making HTTP requests
//! with middleware support, mocking capabilities, and robust error handling.
//!
//! # Features
//!
//! - Middleware pipeline for request/response processing
//! - Mocking support for testing
//! - Comprehensive error handling
//! - Connection pooling and reuse
//! - Async/await support
//!
//! # Examples
//!
//! ```rust
//! use http::{HttpFacade, LoggingMiddleware, Request, Response};
//! use std::collections::HashMap;
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut client = HttpFacade::new();
//!     client.add_middleware(LoggingMiddleware::new(log::Level::Info));
//!
//!     // Set up a mock response
//!     let mock_response = Response {
//!         status: 200,
//!         headers: HashMap::new(),
//!         body: Some(r#"{"message": "Hello, World!"}"#.to_string()),
//!     };
//!     client.mock_exact_url("https://api.example.com/users", mock_response);
//!
//!     // Make request
//!     let request = Request {
//!         method: "GET".to_string(),
//!         url: "https://api.example.com/users".to_string(),
//!         headers: HashMap::new(),
//!         body: None,
//!     };
//!
//!     let response = client.send(request)
//!         .await
//!         .expect("Failed to make request");
//!
//!     assert_eq!(response.status, 200);
//! }
//! ```

use async_trait::async_trait;
use bytes::Bytes;
use log::{error, info};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex},
};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{error as tracing_error, info as tracing_info};

// Workspace dependencies
use config::Config as ConfigTrait;
use encrypt::Encryptor as EncryptorTrait;
use store::Store as StoreTrait;

// Define traits for workspace dependencies
#[async_trait::async_trait]
pub trait Config: Send + Sync {
    type Error;
    async fn get(&self, _key: &str) -> Result<Option<String>, Self::Error>;
    async fn set(&self, _key: &str, _value: &str) -> Result<(), Self::Error>;
}

#[async_trait::async_trait]
pub trait Store: Send + Sync {
    type Error;
    async fn get(&self, _key: &[u8]) -> Result<Option<Bytes>, Self::Error>;
    async fn set(&self, _key: &[u8], _value: Bytes) -> Result<(), Self::Error>;
    async fn delete(&self, _key: &[u8]) -> Result<(), Self::Error>;
    async fn batch_set(&self, _items: Vec<(Vec<u8>, Bytes)>) -> Result<(), Self::Error>;
    async fn batch_delete(&self, _keys: Vec<Vec<u8>>) -> Result<(), Self::Error>;
    async fn range(&self, _range: std::ops::Range<&[u8]>) -> Result<Vec<(Vec<u8>, Bytes)>, Self::Error>;
}

#[async_trait::async_trait]
pub trait Encryptor: Send + Sync {
    type Error;
    async fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, Self::Error>;
    async fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, Self::Error>;
}

// Mock implementations for testing
#[derive(Debug, Default)]
pub struct MockStore;

#[async_trait]
impl Store for MockStore {
    type Error = Error;

    async fn get(&self, _key: &[u8]) -> Result<Option<Bytes>, Self::Error> {
        Ok(None)
    }

    async fn set(&self, _key: &[u8], _value: Bytes) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn delete(&self, _key: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn batch_set(&self, _items: Vec<(Vec<u8>, Bytes)>) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn batch_delete(&self, _keys: Vec<Vec<u8>>) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn range(&self, _range: std::ops::Range<&[u8]>) -> Result<Vec<(Vec<u8>, Bytes)>, Self::Error> {
        Ok(vec![])
    }
}

#[derive(Debug, Default)]
pub struct MockConfig;

#[async_trait]
impl Config for MockConfig {
    type Error = Error;

    async fn get(&self, _key: &str) -> Result<Option<String>, Self::Error> {
        Ok(None)
    }

    async fn set(&self, _key: &str, _value: &str) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct MockEncryptor;

#[async_trait]
impl Encryptor for MockEncryptor {
    type Error = Error;

    async fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, Self::Error> {
        Ok(data.to_vec())
    }

    async fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, Self::Error> {
        Ok(data.to_vec())
    }
}

#[derive(Debug, Default)]
pub struct NullStore;

#[async_trait::async_trait]
impl Store for NullStore {
    type Error = Error;

    async fn get(&self, _key: &[u8]) -> Result<Option<Bytes>, Self::Error> {
        Ok(None)
    }

    async fn set(&self, _key: &[u8], _value: Bytes) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn delete(&self, _key: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn batch_set(&self, _items: Vec<(Vec<u8>, Bytes)>) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn batch_delete(&self, _keys: Vec<Vec<u8>>) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn range(&self, _range: std::ops::Range<&[u8]>) -> Result<Vec<(Vec<u8>, Bytes)>, Self::Error> {
        Ok(vec![])
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Connection error: {0}")]
    Connection(ConnectionErrorKind),

    #[error("Config error: {0}")]
    Config(Box<dyn std::error::Error + Send + Sync>),

    #[error("Store error: {0}")]
    Store(Box<dyn std::error::Error + Send + Sync>),

    #[error("Encryption error: {0}")]
    Encryption(Box<dyn std::error::Error + Send + Sync>),

    #[error("Serialization error: {0}")]
    Serialization(serde_json::Error),

    #[error("Invalid method: {0}")]
    InvalidMethod(String),

    #[error("Invalid pattern: {0}")]
    InvalidPattern(#[from] regex::Error),
}

impl Error {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Connection(ConnectionErrorKind::Timeout) |
            Self::Connection(ConnectionErrorKind::ConnectionReset) => true,
            Self::Http(e) => {
                e.is_connect() ||
                e.is_timeout() ||
                e.is_request() ||
                e.is_builder() ||  // Add this for URL parse errors
                e.status().map_or(false, |s| s.as_u16() >= 500)
            },
            _ => false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectionErrorKind {
    #[error("Connection timed out")]
    Timeout,
    #[error("Connection was reset")]
    ConnectionReset,
}

/// Represents an HTTP request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

/// Represents an HTTP response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

/// Middleware trait for processing requests and responses
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Determines if this middleware should execute for a given request
    fn should_execute(&self, _request: &Request) -> bool {
        true
    }

    /// Process the request before it is sent
    async fn handle_request(&self, request: &mut Request) -> Result<(), Error>;

    /// Process the response after it is received
    async fn handle_response(&self, request: &Request, response: &mut Response) -> Result<(), Error>;
}

/// Different types of mock matchers
pub enum MockMatcher {
    UrlExact(String),
    UrlPattern(Regex),
    Custom(Box<dyn MockMatcherFn>),
}

/// Trait for custom mock matchers
pub trait MockMatcherFn: Send + Sync + fmt::Debug {
    fn matches(&self, request: &Request) -> bool;
}

// Implement Debug for MockMatcher
impl fmt::Debug for MockMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UrlExact(url) => write!(f, "UrlExact({})", url),
            Self::UrlPattern(pattern) => write!(f, "UrlPattern({})", pattern),
            Self::Custom(matcher) => write!(f, "Custom({:?})", matcher),
        }
    }
}

/// Main HTTP client facade implementing Laravel-style patterns
pub struct HttpFacade {
    client: Client,
    middlewares: Vec<Arc<Box<dyn Middleware + Send + Sync>>>,
    mocks: Arc<Mutex<HashMap<String, Response>>>,
    config: Arc<dyn Config<Error = Error>>,
    store: Arc<dyn Store<Error = Error>>,
    encryptor: Arc<dyn Encryptor<Error = Error>>,
}

/// Logging middleware implementation
#[derive(Clone)]
pub struct LoggingMiddleware {
    log_level: log::Level,
}

impl LoggingMiddleware {
    pub fn new(log_level: log::Level) -> Self {
        Self { log_level }
    }
}

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn handle_request(&self, request: &mut Request) -> Result<(), Error> {
        info!(target: "http_client", "-> {} {}", request.method, request.url);
        Ok(())
    }

    async fn handle_response(&self, request: &Request, response: &mut Response) -> Result<(), Error> {
        info!(
            target: "http_client",
            "<- {} {} {}",
            request.method,
            request.url,
            response.status
        );
        Ok(())
    }
}

/// Mock response for testing
pub struct MockResponse {
    matcher: Arc<Box<dyn MockMatcherFn>>,
    response: Response,
}

impl HttpFacade {
    /// Creates a new HTTP facade instance
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            middlewares: Vec::new(),
            mocks: Arc::new(Mutex::new(HashMap::new())),
            config: Arc::new(MockConfig),
            store: Arc::new(MockStore),
            encryptor: Arc::new(MockEncryptor),
        }
    }

    /// Adds a middleware to the processing pipeline
    pub fn add_middleware<M: Middleware + 'static>(&mut self, middleware: M) {
        self.middlewares.push(Arc::new(Box::new(middleware)));
    }

    /// Creates a mock response for testing
    pub fn mock_response(&self, matcher: impl MockMatcherFn + 'static) -> ResponseBuilder {
        ResponseBuilder {
            facade: self.clone(),
            matcher: Box::new(matcher),
        }
    }

    /// Mocks responses for exact URL matches
    pub fn mock_url(&self, url: impl Into<String>) -> ResponseBuilder {
        let url = url.into();
        let matcher = UrlMatcher(url);
        self.mock_response(matcher)
    }

    /// Mocks responses for URL pattern matches
    pub fn mock_pattern(&self, pattern: &str) -> Result<MockBuilder, Error> {
        let regex = Regex::new(pattern)?;
        let mock = MockBuilder::new(self.mocks.clone(), pattern.to_string(), regex);
        Ok(mock)
    }

    pub fn mock_exact_url(&self, url: &str, response: Response) {
        self.mocks.lock().unwrap().insert(url.to_string(), response);
    }

    pub async fn test_mock_exact_url(&self, url: &str) -> bool {
        self.mocks.lock().unwrap().contains_key(url)
    }

    pub async fn send(&self, request: Request) -> Result<Response, Error> {
        // Check if we have a mock for this URL
        if let Some(response) = self.mocks.lock().unwrap().get(&request.url).cloned() {
            return Ok(response);
        }

        // Convert method string to reqwest::Method
        let method = match request.method.to_uppercase().as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "DELETE" => reqwest::Method::DELETE,
            "HEAD" => reqwest::Method::HEAD,
            "OPTIONS" => reqwest::Method::OPTIONS,
            "CONNECT" => reqwest::Method::CONNECT,
            "PATCH" => reqwest::Method::PATCH,
            "TRACE" => reqwest::Method::TRACE,
            _ => return Err(Error::InvalidMethod(request.method)),
        };

        // Build reqwest request
        let mut req_builder = self.client.request(method, &request.url);

        // Add headers
        for (key, value) in request.headers {
            req_builder = req_builder.header(key, value);
        }

        // Add body if present
        if let Some(body) = request.body {
            req_builder = req_builder.body(body);
        }

        // Send request
        let resp = req_builder.send().await?;

        // Convert response
        let mut headers = HashMap::new();
        for (key, value) in resp.headers() {
            if let (Ok(key), Ok(value)) = (key.to_string().parse(), value.to_str()) {
                headers.insert(key, value.to_string());
            }
        }

        let status = resp.status().as_u16();
        let body = resp.text().await.ok();

        Ok(Response {
            status,
            headers,
            body,
        })
    }

    fn get_url(&self) -> String {
        // This is a placeholder - in a real implementation, we'd track the current URL
        // being mocked through the builder pattern
        String::new()
    }

    pub async fn matches_mock(&self, url: &str) -> bool {
        let mocks = self.mocks.lock().unwrap();
        for (pattern, _) in mocks.iter() {
            if let Ok(regex) = Regex::new(pattern) {
                if regex.is_match(url) {
                    return true;
                }
            }
        }
        false
    }
}

impl Clone for HttpFacade {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            middlewares: self.middlewares.clone(),
            mocks: self.mocks.clone(),
            config: self.config.clone(),
            store: self.store.clone(),
            encryptor: self.encryptor.clone(),
        }
    }
}

/// URL exact matcher implementation
#[derive(Debug)]
struct UrlMatcher(String);

impl MockMatcherFn for UrlMatcher {
    fn matches(&self, request: &Request) -> bool {
        request.url == self.0
    }
}

/// Pattern matcher implementation
#[derive(Debug)]
struct PatternMatcher(Regex);

impl MockMatcherFn for PatternMatcher {
    fn matches(&self, request: &Request) -> bool {
        self.0.is_match(&request.url)
    }
}

/// Builder for mock responses
pub struct ResponseBuilder {
    facade: HttpFacade,
    matcher: Box<dyn MockMatcherFn>,
}

impl ResponseBuilder {
    /// Responds with a specific response
    pub async fn respond_with(self, response: Response) {
        let _mock = MockResponse {
            matcher: Arc::new(self.matcher),
            response: response.clone(),
        };
        let mut mocks = self.facade.mocks.lock().unwrap();
        mocks.insert(self.facade.get_url(), response);
    }

    /// Responds with a JSON payload
    pub async fn respond_with_json<T: Serialize>(self, status: u16, body: &T) {
        let response = Response {
            status,
            headers: HashMap::new(),
            body: Some(serde_json::to_string(body).unwrap()),
        };
        self.respond_with(response).await;
    }
}

pub struct MockBuilder {
    mocks: Arc<Mutex<HashMap<String, Response>>>,
    pattern: String,
    regex: Regex,
}

impl MockBuilder {
    pub fn new(mocks: Arc<Mutex<HashMap<String, Response>>>, pattern: String, regex: Regex) -> Self {
        Self {
            mocks,
            pattern,
            regex,
        }
    }

    pub fn respond_with(self, response: Response) {
        let mut mocks = self.mocks.lock().unwrap();
        mocks.insert(self.pattern, response);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;
    use tokio::test;

    #[test_case("/users/1" => true; "exact match")]
    #[test_case("/posts/1" => false; "no match")]
    async fn test_mock_exact_url(url: &str) -> bool {
        let facade = HttpFacade::new();
        facade.mock_url("/users/1")
            .respond_with_json(200, &serde_json::json!({"id": 1}))
            .await;

        let _request = Request {
            method: "GET".into(),
            url: url.into(),
            headers: HashMap::new(),
            body: None,
        };

        let mocks = facade.mocks.lock().unwrap();
        mocks.contains_key(url)
    }

    #[test]
    async fn test_middleware_pipeline() {
        let mut facade = HttpFacade::new();
        facade.add_middleware(LoggingMiddleware::new(log::Level::Info));

        let mut request = Request {
            method: "GET".into(),
            url: "https://api.example.com".into(),
            headers: HashMap::new(),
            body: None,
        };

        for middleware in &facade.middlewares {
            middleware.handle_request(&mut request).await.unwrap();
        }
    }

    #[test]
    async fn test_mock_response() {
        let facade = HttpFacade::new();
        let request = Request {
            url: "https://example.com".to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            body: None,
        };

        // Test mock response
        let response = Response {
            status: 200,
            headers: HashMap::new(),
            body: Some("Hello World".to_string()),
        };

        facade.mock_exact_url("https://example.com", response.clone());

        // Verify mock was added correctly
        let matches = facade.test_mock_exact_url("https://example.com").await;
        assert!(matches);

        // Test sending request
        let result = facade.send(request).await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, Some("Hello World".to_string()));
    }

    #[test]
    async fn test_real_request() {
        let facade = HttpFacade::new();
        let request = Request {
            url: "https://httpbin.org/get".to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            body: None,
        };

        let result = facade.send(request).await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.status, 200);
    }

    #[test]
    async fn test_error_handling() {
        let facade = HttpFacade::new();
        let request = Request {
            url: "https://invalid-domain-that-does-not-exist.com".to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            body: None,
        };

        let result = facade.send(request).await;
        assert!(result.is_err());
        match result {
            Err(Error::Http(_)) => (),
            _ => panic!("Expected Http error"),
        }
    }
}

#[cfg(test)]
mod benchmarks {
    use super::*;
    use criterion::{criterion_group, criterion_main, Criterion};

    pub fn http_facade_benchmark(c: &mut Criterion) {
        let facade = HttpFacade::new();

        c.bench_function("mock_response", |b| {
            b.iter(|| {
                facade.mock_url("https://api.example.com")
            })
        });
    }

    criterion_group!(benches, http_facade_benchmark);
    criterion_main!(benches);
}

pub struct HttpClient {
    config: Arc<dyn Config<Error = Error>>,
    store: Arc<dyn Store<Error = Error>>,
    encryptor: Arc<dyn Encryptor<Error = Error>>,
    client: reqwest::Client,
}

impl HttpClient {
    pub fn new(
        config: Arc<dyn Config<Error = Error>>,
        store: Arc<dyn Store<Error = Error>>,
        encryptor: Arc<dyn Encryptor<Error = Error>>,
    ) -> Self {
        Self {
            config,
            store,
            encryptor,
            client: reqwest::Client::new(),
        }
    }
}

impl Default for HttpFacade {
    fn default() -> Self {
        Self::new()
    }
}

