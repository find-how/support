use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;

/// A pipeline builder that can configure a pipeline
pub trait PipelineBuilder<T>: Send + Sync {
    /// Set the traveler object being sent on the pipeline
    fn send(self, traveler: T) -> Self;

    /// Set the stops (middleware) of the pipeline
    fn through<I>(self, stops: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Box<dyn PipelineStop<T>>>;

    /// Set the method to call on the stops
    fn via(self, method: &str) -> Self;

    /// Build the pipeline
    fn build(self) -> Box<dyn Pipeline<T>>;
}

/// A pipeline that can process a value
#[async_trait]
pub trait Pipeline<T: Send + Sync>: Send + Sync {
    /// Run the pipeline with a final destination callback
    async fn run_with_callback(&mut self, destination: Box<dyn FnOnce(T) -> Pin<Box<dyn Future<Output = T> + Send>> + Send>) -> T;

    /// Set the traveler object being sent on the pipeline
    fn send(&mut self, traveler: T);
}

/// A stop in the pipeline that can process the traveling value
#[async_trait]
pub trait PipelineStop<T: Send + Sync>: Send + Sync {
    /// Process the value and pass it to the next stop
    async fn process(
        &self,
        traveler: T,
        next: Box<dyn FnOnce(T) -> Pin<Box<dyn Future<Output = T> + Send>> + Send>,
    ) -> T;
}

/// A hub that can route objects through different pipelines
pub trait Hub: Send + Sync {
    /// Send an object through one of the available pipelines
    fn pipe<T: 'static + Send + Sync>(&mut self, object: T, pipeline: Option<&str>) -> impl Future<Output = T> + Send;
}
