use async_trait::async_trait;
use std::any::Any;
use crate::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[async_trait]
pub trait JobHandler: Send + Sync + Any {
    fn id(&self) -> String;
    async fn handle(&self) -> Result<()>;
    fn as_any(&self) -> &dyn Any;
}

#[async_trait]
pub trait Queue: Send + Sync {
    async fn push(&self, job: Box<dyn JobHandler>) -> Result<()>;
    async fn pop(&self) -> Result<Option<Box<dyn JobHandler>>>;
    async fn size(&self) -> Result<usize>;
    async fn clear(&self) -> Result<()>;
    async fn get_by_tag(&self, tag: &str) -> Result<Vec<Box<dyn JobHandler>>>;
    async fn fail(&self, job_id: &str, error: String) -> Result<()>;
    async fn push_bulk(&self, jobs: Vec<Box<dyn JobHandler>>) -> Result<()>;
}
