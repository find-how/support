#[cfg(test)]
mod tests {
    use crate::{BasePipeline, BaseHub, PipelineStop};
    use async_trait::async_trait;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::pin::Pin;
    use std::future::Future;

    struct AddPrefixStop {
        prefix: String,
    }

    #[async_trait]
    impl PipelineStop<Box<dyn std::any::Any + Send + Sync>> for AddPrefixStop {
        async fn process(
            &self,
            traveler: Box<dyn std::any::Any + Send + Sync>,
            next: Box<dyn FnOnce(Box<dyn std::any::Any + Send + Sync>) -> Pin<Box<dyn Future<Output = Box<dyn std::any::Any + Send + Sync>> + Send>> + Send>,
        ) -> Box<dyn std::any::Any + Send + Sync> {
            let string = traveler.downcast::<String>().expect("Expected String");
            let result = format!("{}{}", self.prefix, *string);
            next(Box::new(result) as Box<dyn std::any::Any + Send + Sync>).await
        }
    }

    #[derive(Clone)]
    struct TrackingStop {
        counter: Arc<AtomicUsize>,
    }

    impl TrackingStop {
        fn new() -> Self {
            Self {
                counter: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn get_count(&self) -> usize {
            self.counter.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl PipelineStop<Box<dyn std::any::Any + Send + Sync>> for TrackingStop {
        async fn process(
            &self,
            traveler: Box<dyn std::any::Any + Send + Sync>,
            next: Box<dyn FnOnce(Box<dyn std::any::Any + Send + Sync>) -> Pin<Box<dyn Future<Output = Box<dyn std::any::Any + Send + Sync>> + Send>> + Send>,
        ) -> Box<dyn std::any::Any + Send + Sync> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            next(traveler).await
        }
    }

    struct ValidatingStop;

    #[async_trait]
    impl PipelineStop<Box<dyn std::any::Any + Send + Sync>> for ValidatingStop {
        async fn process(
            &self,
            traveler: Box<dyn std::any::Any + Send + Sync>,
            next: Box<dyn FnOnce(Box<dyn std::any::Any + Send + Sync>) -> Pin<Box<dyn Future<Output = Box<dyn std::any::Any + Send + Sync>> + Send>> + Send>,
        ) -> Box<dyn std::any::Any + Send + Sync> {
            let string = traveler.downcast::<String>().expect("Expected String");
            next(Box::new((*string).clone()) as Box<dyn std::any::Any + Send + Sync>).await
        }
    }

    struct MethodAwareStop {
        method: String,
    }

    #[async_trait]
    impl PipelineStop<Box<dyn std::any::Any + Send + Sync>> for MethodAwareStop {
        async fn process(
            &self,
            traveler: Box<dyn std::any::Any + Send + Sync>,
            next: Box<dyn FnOnce(Box<dyn std::any::Any + Send + Sync>) -> Pin<Box<dyn Future<Output = Box<dyn std::any::Any + Send + Sync>> + Send>> + Send>,
        ) -> Box<dyn std::any::Any + Send + Sync> {
            let string = traveler.downcast::<String>().expect("Expected String");
            let result = format!("{}: {}", self.method, *string);
            next(Box::new(result) as Box<dyn std::any::Any + Send + Sync>).await
        }
    }

    #[tokio::test]
    async fn test_basic_pipeline() {
        let mut hub = BaseHub::new();
        let pipeline = BasePipeline::new()
            .send(Box::new("world".to_string()) as Box<dyn std::any::Any + Send + Sync>)
            .through(Box::new(AddPrefixStop { prefix: "hello ".to_string() }))
            .build();
        hub.register("test", pipeline);

        let result = hub.pipe(Box::new("world".to_string()) as Box<dyn std::any::Any + Send + Sync>, Some("test")).await;
        let result_str = result.downcast::<String>().expect("Failed to downcast result");
        assert_eq!(*result_str, "hello world");
    }

    #[tokio::test]
    async fn test_multiple_stops() {
        let mut hub = BaseHub::new();
        let first_pipeline = BasePipeline::new()
            .send(Box::new("world".to_string()) as Box<dyn std::any::Any + Send + Sync>)
            .through(Box::new(AddPrefixStop { prefix: "hello ".to_string() }))
            .build();
        hub.register("first", first_pipeline);

        let second_pipeline = BasePipeline::new()
            .send(Box::new("world".to_string()) as Box<dyn std::any::Any + Send + Sync>)
            .through(Box::new(AddPrefixStop { prefix: "goodbye ".to_string() }))
            .build();
        hub.register("second", second_pipeline);

        let result1 = hub.pipe(Box::new("world".to_string()) as Box<dyn std::any::Any + Send + Sync>, Some("first")).await;
        let result1_str = result1.downcast::<String>().expect("Failed to downcast result1");
        assert_eq!(*result1_str, "hello world");

        let result2 = hub.pipe(Box::new("world".to_string()) as Box<dyn std::any::Any + Send + Sync>, Some("second")).await;
        let result2_str = result2.downcast::<String>().expect("Failed to downcast result2");
        assert_eq!(*result2_str, "goodbye world");
    }

    #[tokio::test]
    async fn test_tracking_stop() {
        let mut hub = BaseHub::new();
        let tracking_stop = TrackingStop::new();
        let stop_ref = tracking_stop.clone();

        let pipeline = BasePipeline::new()
            .send(Box::new("world".to_string()) as Box<dyn std::any::Any + Send + Sync>)
            .through(Box::new(tracking_stop))
            .build();
        hub.register("test", pipeline);

        let result = hub.pipe(Box::new("world".to_string()) as Box<dyn std::any::Any + Send + Sync>, Some("test")).await;
        let result_str = result.downcast::<String>().expect("Failed to downcast result");
        assert_eq!(*result_str, "world");
        assert_eq!(stop_ref.get_count(), 1);
    }

    #[tokio::test]
    async fn test_validating_stop() {
        let mut hub = BaseHub::new();
        let pipeline = BasePipeline::new()
            .send(Box::new("hi".to_string()) as Box<dyn std::any::Any + Send + Sync>)
            .through(Box::new(ValidatingStop))
            .build();
        hub.register("test", pipeline);

        let result = hub.pipe(Box::new("hi".to_string()) as Box<dyn std::any::Any + Send + Sync>, Some("test")).await;
        let result_str = result.downcast::<String>().expect("Failed to downcast result");
        assert_eq!(*result_str, "hi");

        let result = hub.pipe(Box::new("world".to_string()) as Box<dyn std::any::Any + Send + Sync>, Some("test")).await;
        let result_str = result.downcast::<String>().expect("Failed to downcast result");
        assert_eq!(*result_str, "world");
    }

    #[tokio::test]
    async fn test_method_aware_stop() {
        let mut hub = BaseHub::new();
        let pipeline = BasePipeline::new()
            .send(Box::new("world".to_string()) as Box<dyn std::any::Any + Send + Sync>)
            .through(Box::new(MethodAwareStop { method: "GET".to_string() }))
            .build();
        hub.register("test", pipeline);

        let result = hub.pipe(Box::new("world".to_string()) as Box<dyn std::any::Any + Send + Sync>, Some("test")).await;
        let result_str = result.downcast::<String>().expect("Failed to downcast result");
        assert_eq!(*result_str, "GET: world");
    }
}
