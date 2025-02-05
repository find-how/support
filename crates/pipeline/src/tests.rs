use std::sync::Arc;
use async_trait::async_trait;
use futures::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::{PipelineStop, BasePipeline, Pipeline};
use std::any::Any;

#[derive(Clone)]
struct AddPrefixStop {
    prefix: String,
}

impl AddPrefixStop {
    fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
        }
    }
}

#[async_trait]
impl PipelineStop<String> for AddPrefixStop {
    async fn process(&self, traveler: String, next: Box<dyn FnOnce(String) -> Pin<Box<dyn Future<Output = String> + Send>> + Send>) -> String {
        let result = format!("{}{}", self.prefix, traveler);
        next(result).await
    }
}

#[derive(Clone)]
struct TrackingStop {
    count: Arc<std::sync::atomic::AtomicUsize>,
}

impl TrackingStop {
    fn new() -> Self {
        Self {
            count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl PipelineStop<Box<dyn Any + Send + Sync>> for TrackingStop {
    async fn process(&self, traveler: Box<dyn Any + Send + Sync>) -> Box<dyn Any + Send + Sync> {
        self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        traveler
    }
}

#[derive(Clone)]
struct ValidatingStop;

#[async_trait]
impl PipelineStop<Box<dyn Any + Send + Sync>> for ValidatingStop {
    async fn process(&self, traveler: Box<dyn Any + Send + Sync>) -> Box<dyn Any + Send + Sync> {
        if let Ok(string) = traveler.downcast::<String>() {
            if string.is_empty() {
                panic!("Empty string not allowed");
            }
            Box::new(*string)
        } else {
            panic!("Expected String");
        }
    }
}

#[derive(Clone)]
struct MethodAwareStop {
    method: String,
}

#[async_trait]
impl PipelineStop<Box<dyn Any + Send + Sync>> for MethodAwareStop {
    async fn process(&self, traveler: Box<dyn Any + Send + Sync>) -> Box<dyn Any + Send + Sync> {
        if let Ok(string) = traveler.downcast::<String>() {
            Box::new(format!("{} {}", self.method, *string))
        } else {
            panic!("Expected String");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_basic_pipeline() {
        let mut pipeline = BasePipeline::new();
        pipeline.stops.push(Box::new(AddPrefixStop::new("Hello ")));

        pipeline.send("World".to_string());

        let result = pipeline.run_with_callback(Box::new(|t| Box::pin(async move { t }))).await;
        assert_eq!(result, "Hello World");
    }

    #[tokio::test]
    async fn test_tracking_pipeline() {
        let mut pipeline = BasePipeline::new();
        let tracking_stop = Arc::new(TrackingStop::new());
        pipeline.add_stop(tracking_stop.clone());

        pipeline.send(Box::new("test".to_string()) as Box<dyn Any + Send + Sync>);

        let result = pipeline.process().await;
        let string = result.downcast::<String>().expect("Expected String");
        assert_eq!(*string, "test");
        assert_eq!(tracking_stop.count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_validating_pipeline() {
        let mut pipeline = BasePipeline::new();
        pipeline.add_stop(Arc::new(ValidatingStop));

        pipeline.send(Box::new("test".to_string()) as Box<dyn Any + Send + Sync>);

        let result = pipeline.process().await;
        let string = result.downcast::<String>().expect("Expected String");
        assert_eq!(*string, "test");
    }

    #[tokio::test]
    #[should_panic(expected = "Empty string not allowed")]
    async fn test_validating_pipeline_empty_string() {
        let mut pipeline = BasePipeline::new();
        pipeline.add_stop(Arc::new(ValidatingStop));

        pipeline.send(Box::new("".to_string()) as Box<dyn Any + Send + Sync>);

        let _ = pipeline.process().await;
    }

    #[tokio::test]
    async fn test_method_aware_pipeline() {
        let mut pipeline = BasePipeline::new();
        pipeline.add_stop(Arc::new(MethodAwareStop { method: "GET".to_string() }));

        pipeline.send(Box::new("/users".to_string()) as Box<dyn Any + Send + Sync>);

        let result = pipeline.process().await;
        let string = result.downcast::<String>().expect("Expected String");
        assert_eq!(*string, "GET /users");
    }
}
