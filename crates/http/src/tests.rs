#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::test;

    #[test]
    async fn test_mock_response() {
        let mut facade = HttpFacade::new();
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
