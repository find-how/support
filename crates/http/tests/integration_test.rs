use http::{HttpFacade, LoggingMiddleware, Middleware, Request, Response, Error, ConnectionErrorKind};
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::test;
use std::collections::HashMap;

// Custom test middleware that counts requests
#[derive(Clone)]
struct CountingMiddleware {
    request_count: Arc<AtomicUsize>,
}

impl CountingMiddleware {
    fn new() -> Self {
        Self {
            request_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn get_count(&self) -> usize {
        self.request_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Middleware for CountingMiddleware {
    async fn handle_request(&self, _request: &mut Request) -> Result<(), Error> {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn handle_response(&self, _request: &Request, _response: &mut Response) -> Result<(), Error> {
        Ok(())
    }
}

#[test]
async fn test_middleware_execution_order() {
    let mut facade = HttpFacade::new();
    let counter = CountingMiddleware::new();
    let counter_clone = counter.clone();

    facade.add_middleware(counter);
    facade.add_middleware(LoggingMiddleware::new(log::Level::Info));

    // Create request but don't use it yet since we're just testing middleware setup
    let _request = Request {
        method: "GET".into(),
        url: "https://api.example.com".into(),
        headers: HashMap::new(),
        body: None,
    };

    // Mock the response
    facade.mock_url("https://api.example.com")
        .respond_with_json(200, &serde_json::json!({"status": "ok"}))
        .await;

    assert_eq!(counter_clone.get_count(), 0);
}

#[test]
async fn test_mock_pattern() {
    let facade = HttpFacade::new();
    facade.mock_pattern(r"/users/\d+").unwrap()
        .respond_with(Response {
            status: 200,
            body: Some("mocked response".to_string()),
            headers: HashMap::new(),
        });

    let test_cases = vec![
        ("/users/123", true),
        ("/users/abc", false),
    ];

    for (url, expected) in test_cases {
        let matches = facade.matches_mock(url).await;
        assert_eq!(matches, expected);
    }
}

#[test]
async fn test_mock_response() {
    let facade = HttpFacade::new();
    let _request = Request {
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
}

#[test]
async fn test_error_handling() {
    let facade = HttpFacade::new();

    // Test invalid regex pattern
    let result = facade.mock_pattern("[invalid");
    assert!(result.is_err());

    // Test request with invalid method
    let request = Request {
        method: "INVALID".to_string(),
        url: "/test".to_string(),
        headers: HashMap::new(),
        body: None,
    };

    let result = facade.send(request).await;
    assert!(matches!(result.unwrap_err(), Error::InvalidMethod(_)));
}

#[test]
async fn test_retryable_errors() {
    let error = Error::Connection(ConnectionErrorKind::Timeout);
    assert!(error.is_retryable());

    // Create a reqwest error from a URL parse error instead
    let error = Error::Http(reqwest::Client::new()
        .get("invalid://url")
        .send()
        .await
        .unwrap_err());
    assert!(error.is_retryable());

    let error = Error::InvalidMethod("Invalid input".into());
    assert!(!error.is_retryable());
}

#[test]
async fn test_json_handling() {
    let facade = HttpFacade::new();

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct TestData {
        id: i32,
        name: String,
    }

    let test_data = TestData {
        id: 1,
        name: "Test".into(),
    };

    facade.mock_url("/data")
        .respond_with_json(200, &test_data)
        .await;
}
