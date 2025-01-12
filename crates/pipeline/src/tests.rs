use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use async_trait::async_trait;
use crate::{Pipeline, PipelineStop, BasePipeline, BaseHub};
use std::any::Any;
use std::sync::Arc;

#[derive(Clone)]
pub struct AddPrefixStop {
    prefix: String,
}

#[async_trait]
impl PipelineStop<Box<dyn Any + Send + Sync>> for AddPrefixStop {
    async fn process(
        &self,
        traveler: Box<dyn Any + Send + Sync>,
        next: Box<dyn FnOnce(Box<dyn Any + Send + Sync>) -> Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + Send>> + Send>,
    ) -> Box<dyn Any + Send + Sync> {
        let string = traveler.downcast::<String>().expect("Expected String");
        let result = format!("{}{}", self.prefix, *string);
        next(Box::new(result)).await
    }
}

#[derive(Clone)]
pub struct TrackingStop {
    count: Arc<AtomicUsize>,
}

impl TrackingStop {
    pub fn new() -> Self {
        Self {
            count: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl PipelineStop<Box<dyn Any + Send + Sync>> for TrackingStop {
    async fn process(
        &self,
        traveler: Box<dyn Any + Send + Sync>,
        next: Box<dyn FnOnce(Box<dyn Any + Send + Sync>) -> Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + Send>> + Send>,
    ) -> Box<dyn Any + Send + Sync> {
        self.count.fetch_add(1, Ordering::SeqCst);
        next(traveler).await
    }
}

#[derive(Clone)]
pub struct ValidatingStop;

#[async_trait]
impl PipelineStop<Box<dyn Any + Send + Sync>> for ValidatingStop {
    async fn process(
        &self,
        traveler: Box<dyn Any + Send + Sync>,
        next: Box<dyn FnOnce(Box<dyn Any + Send + Sync>) -> Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + Send>> + Send>,
    ) -> Box<dyn Any + Send + Sync> {
        let string = traveler.downcast::<String>().expect("Expected String");
        if string.len() < 3 {
            panic!("String too short");
        }
        next(Box::new((*string).clone())).await
    }
}

#[derive(Clone)]
pub struct MethodAwareStop {
    method: String,
}

#[async_trait]
impl PipelineStop<Box<dyn Any + Send + Sync>> for MethodAwareStop {
    async fn process(
        &self,
        traveler: Box<dyn Any + Send + Sync>,
        next: Box<dyn FnOnce(Box<dyn Any + Send + Sync>) -> Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + Send>> + Send>,
    ) -> Box<dyn Any + Send + Sync> {
        let string = traveler.downcast::<String>().expect("Expected String");
        let result = format!("{} {}", self.method, *string);
        next(Box::new(result)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_basic_pipeline() {
        let mut pipeline = BasePipeline::new()
            .send(Box::new("world".to_string()) as Box<dyn Any + Send + Sync>)
            .through(Box::new(AddPrefixStop { prefix: "hello ".to_string() }))
            .build();

        let result = pipeline.run_with_callback(Box::new(|t| Box::pin(async move { t }))).await;
        let string = result.downcast::<String>().expect("Expected String");
        assert_eq!(*string, "hello world");
    }

    #[tokio::test]
    async fn test_tracking_stop() {
        let tracking_stop = TrackingStop::new();
        let tracking_stop_clone = tracking_stop.clone();

        let mut pipeline = BasePipeline::new()
            .send(Box::new("world".to_string()) as Box<dyn Any + Send + Sync>)
            .through(Box::new(tracking_stop))
            .build();

        let result = pipeline.run_with_callback(Box::new(|t| Box::pin(async move { t }))).await;
        let string = result.downcast::<String>().expect("Expected String");
        assert_eq!(*string, "world");
        assert_eq!(tracking_stop_clone.count(), 1);
    }

    #[tokio::test]
    async fn test_validating_stop() {
        let mut pipeline = BasePipeline::new()
            .send(Box::new("world".to_string()) as Box<dyn Any + Send + Sync>)
            .through(Box::new(ValidatingStop))
            .build();

        let result = pipeline.run_with_callback(Box::new(|t| Box::pin(async move { t }))).await;
        let string = result.downcast::<String>().expect("Expected String");
        assert_eq!(*string, "world");
    }

    #[tokio::test]
    async fn test_method_aware_stop() {
        let mut pipeline = BasePipeline::new()
            .send(Box::new("world".to_string()) as Box<dyn Any + Send + Sync>)
            .through(Box::new(MethodAwareStop { method: "GET".to_string() }))
            .build();

        let result = pipeline.run_with_callback(Box::new(|t| Box::pin(async move { t }))).await;
        let string = result.downcast::<String>().expect("Expected String");
        assert_eq!(*string, "GET world");
    }
}
