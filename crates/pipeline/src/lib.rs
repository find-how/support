mod traits;
pub use traits::*;

use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

/// A basic implementation of the Pipeline trait
pub struct BasePipeline<T: Send + Sync> {
    traveler: Option<T>,
    stops: Vec<Arc<dyn PipelineStop<T>>>,
}

impl<T: Send + Sync + 'static> BasePipeline<T> {
    pub fn new() -> Self {
        Self {
            traveler: None,
            stops: Vec::new(),
        }
    }

    pub fn add_stop(&mut self, stop: Arc<dyn PipelineStop<T>>) {
        self.stops.push(stop);
    }

    pub fn send(&mut self, traveler: T) {
        self.traveler = Some(traveler);
    }

    pub async fn process(&mut self) -> T {
        let mut traveler = self.traveler.take().expect("Pipeline must have a traveler");

        for stop in &self.stops {
            traveler = stop.process(traveler).await;
        }

        traveler
    }
}

#[async_trait]
pub trait PipelineStop<T: Send + Sync>: Send + Sync {
    async fn process(&self, traveler: T) -> T;
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

    pub async fn pipe(&mut self, _object: Box<dyn Any + Send + Sync>, pipeline_name: Option<&str>) -> Box<dyn Any + Send + Sync> {
        let pipeline = self.pipelines.get_mut(pipeline_name.unwrap_or("default")).expect("Pipeline not found");
        pipeline.run_with_callback(Box::new(|t| Box::pin(async move { t }))).await
    }
}

impl Hub for BaseHub {
    fn pipe<T: 'static + Send + Sync>(&mut self, object: T, pipeline: Option<&str>) -> impl Future<Output = T> + Send {
        async move {
            let boxed_input = Box::new(object) as Box<dyn Any + Send + Sync>;
            let processed = self.pipe(boxed_input, pipeline).await;
            *processed.downcast::<T>().expect("Failed to downcast processed object")
        }
    }
}

pub struct PipelineHub {
    pipelines: HashMap<String, Arc<Mutex<BasePipeline<Box<dyn Any + Send + Sync>>>>>,
}

impl PipelineHub {
    pub fn new() -> Self {
        Self {
            pipelines: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, pipeline: BasePipeline<Box<dyn Any + Send + Sync>>) {
        self.pipelines.insert(name.to_string(), Arc::new(Mutex::new(pipeline)));
    }

    pub async fn process(&mut self, pipeline_name: Option<&str>, traveler: Box<dyn Any + Send + Sync>) -> Box<dyn Any + Send + Sync> {
        let pipeline = self.pipelines.get(pipeline_name.unwrap_or("default")).expect("Pipeline not found");
        let mut pipeline = pipeline.lock().unwrap();
        pipeline.send(traveler);
        pipeline.process().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        async fn process(&self, traveler: String) -> String {
            format!("{}{}", self.prefix, traveler)
        }
    }

    #[tokio::test]
    async fn test_basic_pipeline() {
        let mut pipeline = BasePipeline::new();
        pipeline.add_stop(Arc::new(AddPrefixStop::new("Hello ")));

        pipeline.send("World".to_string());

        let result = pipeline.process().await;
        assert_eq!(result, "Hello World");
    }
}
