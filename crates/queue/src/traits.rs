use std::error::Error;
use std::any::Any;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[async_trait::async_trait]
pub trait JobHandler: Send + Sync + Any + 'static {
    async fn handle(&self) -> Result<()>;
    fn id(&self) -> String;
    fn as_any(&self) -> &dyn Any;
}

#[async_trait::async_trait]
pub trait Queue: Send + Sync {
    async fn push(&self, job: Box<dyn JobHandler>) -> Result<()>;
    async fn push_bulk(&self, jobs: Vec<Box<dyn JobHandler>>) -> Result<()>;
    async fn pop(&self) -> Result<Option<Box<dyn JobHandler>>>;
    async fn peek(&self) -> Result<Option<Box<dyn JobHandler>>>;
    async fn size(&self) -> Result<usize>;
    async fn complete(&self, job_id: &str) -> Result<()>;
    async fn fail(&self, job_id: &str, error: String) -> Result<()>;
    async fn get_failed(&self) -> Result<Vec<(Box<dyn JobHandler>, String)>>;
    async fn retry(&self, job_id: &str) -> Result<bool>;
    async fn get_by_tag(&self, tag: &str) -> Result<Vec<Box<dyn JobHandler>>>;
    async fn clear(&self) -> Result<()>;
}
