mod traits;
pub use traits::*;

use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use async_trait::async_trait;

/// A basic implementation of the Pipeline trait
pub struct BasePipeline<T: Send + Sync> {
    traveler: Option<T>,
    stops: Vec<Box<dyn PipelineStop<T> + Send + Sync>>,
}

impl<T: Send + Sync + 'static> BasePipeline<T> {
    pub fn new() -> Self {
        Self {
            traveler: None,
            stops: Vec::new(),
        }
    }

    pub fn send(mut self, traveler: T) -> Self {
        self.traveler = Some(traveler);
        self
    }

    pub fn through(mut self, stop: Box<dyn PipelineStop<T> + Send + Sync>) -> Self {
        self.stops.push(stop);
        self
    }

    pub fn build(self) -> Self {
        self
    }
}

#[async_trait]
pub trait Pipeline<T: Send + Sync>: Send + Sync {
    async fn run_with_callback(&mut self, destination: Box<dyn FnOnce(T) -> Pin<Box<dyn Future<Output = T> + Send>> + Send>) -> T;
}

#[async_trait]
pub trait PipelineStop<T: Send + Sync>: Send + Sync {
    async fn process(&self, traveler: T, next: Box<dyn FnOnce(T) -> Pin<Box<dyn Future<Output = T> + Send>> + Send>) -> T;
}

pub trait Hub: Send + Sync {
    async fn pipe<T: 'static + Send + Sync>(&mut self, object: T, pipeline: Option<&str>) -> T;
}

#[async_trait]
impl<T: Send + Sync + 'static> Pipeline<T> for BasePipeline<T> {
    async fn run_with_callback(&mut self, destination: Box<dyn FnOnce(T) -> Pin<Box<dyn Future<Output = T> + Send>> + Send>) -> T {
        let mut traveler = self.traveler.take().expect("Pipeline must have a traveler");

        for stop in &self.stops {
            let next: Box<dyn FnOnce(T) -> Pin<Box<dyn Future<Output = T> + Send>> + Send> =
                Box::new(|t| Box::pin(async move { t }) as Pin<Box<dyn Future<Output = T> + Send>>);
            traveler = stop.process(traveler, next).await;
        }

        destination(traveler).await
    }
}

/// A basic implementation of the Hub trait
pub struct BaseHub {
    pipelines: HashMap<String, Box<dyn Pipeline<Box<dyn Any + Send + Sync>>>>,
}

impl BaseHub {
    pub fn new() -> Self {
        Self {
            pipelines: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, pipeline: Box<dyn Pipeline<Box<dyn Any + Send + Sync>>>) {
        self.pipelines.insert(name.to_string(), pipeline);
    }

    pub async fn pipe(&mut self, object: Box<dyn Any + Send + Sync>, pipeline_name: Option<&str>) -> Box<dyn Any + Send + Sync> {
        let pipeline = self.pipelines.get_mut(pipeline_name.unwrap_or("default")).expect("Pipeline not found");
        pipeline.run_with_callback(Box::new(|t| Box::pin(async move { t }) as Pin<Box<dyn Future<Output = Box<dyn Any + Send + Sync>> + Send>>)).await
    }
}

impl Hub for BaseHub {
    async fn pipe<T: 'static + Send + Sync>(&mut self, object: T, pipeline: Option<&str>) -> T {
        let boxed_input = Box::new(object) as Box<dyn Any + Send + Sync>;
        let processed = self.pipe(boxed_input, pipeline).await;
        *processed.downcast::<T>().expect("Failed to downcast processed object")
    }
}

#[cfg(test)]
mod tests;
