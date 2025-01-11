mod traits;
pub use traits::*;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use async_trait::async_trait;

/// A basic implementation of the Pipeline trait
pub struct BasePipeline<T> {
    traveler: Option<T>,
    stops: Vec<Box<dyn PipelineStop<T>>>,
    method: String,
}

impl<T: 'static + Send + Sync> BasePipeline<T> {
    pub fn new() -> Self {
        Self {
            traveler: None,
            stops: Vec::new(),
            method: String::from("process"),
        }
    }

    pub fn send(mut self, traveler: T) -> Self {
        self.traveler = Some(traveler);
        self
    }

    pub fn through(mut self, stop: Box<dyn PipelineStop<T>>) -> Self {
        self.stops.push(stop);
        self
    }

    pub fn via(mut self, method: &str) -> Self {
        self.method = method.to_string();
        self
    }

    pub fn build(self) -> Box<dyn Pipeline<T>> {
        Box::new(self)
    }
}

#[async_trait]
impl<T: 'static + Send + Sync> Pipeline<T> for BasePipeline<T> {
    async fn run_with_callback(&mut self, destination: Box<dyn FnOnce(T) -> Pin<Box<dyn Future<Output = T> + Send>> + Send>) -> T {
        let traveler = self.traveler.take().expect("Pipeline must have a traveler");
        let mut current = traveler;

        for stop in &mut self.stops {
            let next = Box::new(|t: T| Box::pin(async move { t }) as Pin<Box<dyn Future<Output = T> + Send>>);
            current = stop.process(current, next).await;
        }

        destination(current).await
    }
}

/// A basic implementation of the Hub trait
pub struct BaseHub {
    pipelines: HashMap<String, Box<dyn Pipeline<Box<dyn std::any::Any + Send + Sync>>>>,
}

impl BaseHub {
    pub fn new() -> Self {
        Self {
            pipelines: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, pipeline: Box<dyn Pipeline<Box<dyn std::any::Any + Send + Sync>>>) {
        self.pipelines.insert(name.to_string(), pipeline);
    }

    pub async fn pipe(&mut self, object: Box<dyn std::any::Any + Send + Sync>, pipeline_name: Option<&str>) -> Box<dyn std::any::Any + Send + Sync> {
        let pipeline = self.pipelines.get_mut(pipeline_name.unwrap_or("default")).expect("Pipeline not found");
        pipeline.run_with_callback(Box::new(|_| Box::pin(async move { object }))).await
    }
}

#[async_trait]
impl Hub for BaseHub {
    async fn pipe<T>(&mut self, object: T, pipeline: Option<&str>) -> T
    where
        T: Send + Sync + 'static,
    {
        let boxed_input = Box::new(object) as Box<dyn std::any::Any + Send + Sync>;
        let processed = self.pipe(boxed_input, pipeline).await;
        *processed.downcast::<T>().expect("Failed to downcast processed object")
    }
}

#[cfg(test)]
mod tests;
