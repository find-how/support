use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use std::time::Duration as StdDuration;

mod traits;
pub use traits::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Storage error: {0}")]
    Storage(#[from] sled::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    #[error("Job error: {0}")]
    Job(String),
    #[error("Queue error: {0}")]
    Queue(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    id: String,
    payload: String,
    priority: i32,
    delay: Option<StdDuration>,
    attempts: u32,
    max_attempts: u32,
    tags: Vec<String>,
    created_at: DateTime<Utc>,
    available_at: DateTime<Utc>,
    reserved_at: Option<DateTime<Utc>>,
}

impl Job {
    pub fn new(id: String, payload: String) -> Self {
        Self {
            id,
            payload,
            priority: 0,
            delay: None,
            attempts: 0,
            max_attempts: 3,
            tags: Vec::new(),
            created_at: Utc::now(),
            available_at: Utc::now(),
            reserved_at: None,
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_delay(mut self, delay: StdDuration) -> Self {
        self.delay = Some(delay);
        self.available_at = Utc::now() + chrono::Duration::from_std(delay).unwrap();
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub async fn handle(&self) -> Result<()> {
        // Default implementation just logs the job
        tracing::info!("Handling job {} with payload {}", self.id, self.payload);
        Ok(())
    }
}

#[async_trait]
impl JobHandler for Job {
    fn id(&self) -> String {
        self.id.clone()
    }

    async fn handle(&self) -> Result<()> {
        self.handle().await
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone)]
pub struct SledQueueImpl {
    db: Arc<sled::Db>,
    name: String,
    failed_jobs: Arc<Mutex<HashMap<String, (String, DateTime<Utc>)>>>,
}

impl SledQueueImpl {
    pub fn new(name: String, path: String) -> Result<Self> {
        let db = sled::open(path)?;
        Ok(Self {
            db: Arc::new(db),
            name,
            failed_jobs: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn job_key(&self, job_id: &str) -> Vec<u8> {
        format!("{}:job:{}", self.name, job_id).into_bytes()
    }

    fn queue_key(&self) -> Vec<u8> {
        format!("{}:queue", self.name).into_bytes()
    }

    fn tag_key(&self, tag: &str) -> Vec<u8> {
        format!("{}:tag:{}", self.name, tag).into_bytes()
    }
}

#[async_trait]
impl Queue for SledQueueImpl {
    async fn push(&self, job: Box<dyn JobHandler>) -> Result<()> {
        let job = job.as_any().downcast_ref::<Job>().ok_or_else(|| {
            Error::Job("Failed to downcast job".into())
        })?;

        // Store job data
        let job_key = self.job_key(&job.id);
        let job_data = bincode::serialize(job)?;
        self.db.insert(job_key, job_data)?;

        // Add to queue
        let queue_key = self.queue_key();
        let mut queue: Vec<String> = self.db
            .get(&queue_key)?
            .map(|data| bincode::deserialize(&data))
            .transpose()?
            .unwrap_or_default();

        queue.push(job.id.clone());
        self.db.insert(queue_key, bincode::serialize(&queue)?)?;

        // Add to tag indices
        for tag in &job.tags {
            let tag_key = self.tag_key(tag);
            let mut tag_jobs: Vec<String> = self.db
                .get(&tag_key)?
                .map(|data| bincode::deserialize(&data))
                .transpose()?
                .unwrap_or_default();

            tag_jobs.push(job.id.clone());
            self.db.insert(tag_key, bincode::serialize(&tag_jobs)?)?;
        }

        self.db.flush()?;
        Ok(())
    }

    async fn pop(&self) -> Result<Option<Box<dyn JobHandler>>> {
        let queue_key = self.queue_key();
        let mut queue: Vec<String> = self.db
            .get(&queue_key)?
            .map(|data| bincode::deserialize(&data))
            .transpose()?
            .unwrap_or_default();

        if queue.is_empty() {
            return Ok(None);
        }

        // Get next available job
        let now = Utc::now();
        let mut job_index = None;
        let mut job = None;

        for (i, job_id) in queue.iter().enumerate() {
            let job_key = self.job_key(job_id);
            if let Some(job_data) = self.db.get(&job_key)? {
                let mut job_item: Job = bincode::deserialize(&job_data)?;
                if job_item.available_at <= now {
                    job_item.reserved_at = Some(now);
                    job = Some(job_item);
                    job_index = Some(i);
                    break;
                }
            }
        }

        if let Some(job) = job {
            if let Some(index) = job_index {
                queue.remove(index);
                self.db.insert(queue_key, bincode::serialize(&queue)?)?;
                self.db.flush()?;
                return Ok(Some(Box::new(job) as Box<dyn JobHandler>));
            }
        }

        Ok(None)
    }

    async fn size(&self) -> Result<usize> {
        let queue_key = self.queue_key();
        let queue: Vec<String> = self.db
            .get(&queue_key)?
            .map(|data| bincode::deserialize(&data))
            .transpose()?
            .unwrap_or_default();

        Ok(queue.len())
    }

    async fn clear(&self) -> Result<()> {
        let queue_key = self.queue_key();
        self.db.remove(queue_key)?;
        self.db.flush()?;
        Ok(())
    }

    async fn get_by_tag(&self, tag: &str) -> Result<Vec<Box<dyn JobHandler>>> {
        let tag_key = self.tag_key(tag);
        let job_ids: Vec<String> = self.db
            .get(&tag_key)?
            .map(|data| bincode::deserialize(&data))
            .transpose()?
            .unwrap_or_default();

        let mut jobs = Vec::new();
        for job_id in job_ids {
            let job_key = self.job_key(&job_id);
            if let Some(job_data) = self.db.get(&job_key)? {
                let job: Job = bincode::deserialize(&job_data)?;
                jobs.push(Box::new(job) as Box<dyn JobHandler>);
            }
        }

        Ok(jobs)
    }

    async fn fail(&self, job_id: &str, error: String) -> Result<()> {
        let mut failed_jobs = self.failed_jobs.lock().await;
        failed_jobs.insert(job_id.to_string(), (error, Utc::now()));
        Ok(())
    }

    async fn push_bulk(&self, jobs: Vec<Box<dyn JobHandler>>) -> Result<()> {
        for job in jobs {
            self.push(job).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_job_priority() -> Result<()> {
        let queue = SledQueueImpl::new("test_priority".to_string(), "test_priority.db".to_string())?;
        queue.clear().await?;

        let job1 = Job::new("1".into(), "job1".into()).with_priority(0);
        let job2 = Job::new("2".into(), "job2".into()).with_priority(1);
        let job3 = Job::new("3".into(), "job3".into()).with_priority(-1);

        queue.push(Box::new(job1.clone())).await?;
        queue.push(Box::new(job2.clone())).await?;
        queue.push(Box::new(job3.clone())).await?;

        let job1 = queue.pop().await?.unwrap();
        let job2 = queue.pop().await?.unwrap();
        let job3 = queue.pop().await?.unwrap();

        let popped1 = job1.as_any().downcast_ref::<Job>().unwrap();
        let popped2 = job2.as_any().downcast_ref::<Job>().unwrap();
        let popped3 = job3.as_any().downcast_ref::<Job>().unwrap();

        assert_eq!(popped1.id, "2"); // Highest priority
        assert_eq!(popped2.id, "1"); // Medium priority
        assert_eq!(popped3.id, "3"); // Lowest priority

        Ok(())
    }

    #[tokio::test]
    async fn test_delayed_jobs() -> Result<()> {
        let queue = SledQueueImpl::new("test_delayed".to_string(), "test_delayed.db".to_string())?;
        queue.clear().await?;

        let job = Job::new("delayed".into(), "test".into())
            .with_delay(StdDuration::from_secs(2));

        queue.push(Box::new(job)).await?;

        assert!(queue.pop().await?.is_none());

        sleep(StdDuration::from_secs(2)).await;

        assert!(queue.pop().await?.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_bulk_push() -> Result<()> {
        let queue = SledQueueImpl::new("test_bulk".to_string(), "test_bulk.db".to_string())?;
        queue.clear().await?;

        let jobs: Vec<Box<dyn JobHandler>> = vec![
            Box::new(Job::new("1".into(), "job1".into())),
            Box::new(Job::new("2".into(), "job2".into())),
            Box::new(Job::new("3".into(), "job3".into())),
        ];

        queue.push_bulk(jobs).await?;

        assert_eq!(queue.size().await?, 3);

        Ok(())
    }

    #[tokio::test]
    async fn test_job_tags() -> Result<()> {
        let queue = SledQueueImpl::new("test_tags".to_string(), "test_tags.db".to_string())?;
        queue.clear().await?;

        let job1 = Job::new("1".into(), "job1".into())
            .with_tags(vec!["tag1".into(), "tag2".into()]);
        let job2 = Job::new("2".into(), "job2".into())
            .with_tags(vec!["tag2".into()]);

        queue.push(Box::new(job1)).await?;
        queue.push(Box::new(job2)).await?;

        let tag1_jobs = queue.get_by_tag("tag1").await?;
        let tag2_jobs = queue.get_by_tag("tag2").await?;

        assert_eq!(tag1_jobs.len(), 1);
        assert_eq!(tag2_jobs.len(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_retry_with_backoff() -> Result<()> {
        let queue = SledQueueImpl::new("test".to_string(), "test.db".to_string())?;
        queue.clear().await?;

        let job_id = "test_job".to_string();
        let job = Job::new(job_id.clone(), "test payload".to_string());

        // Push the job to the queue
        queue.push(Box::new(job)).await?;

        // Pop the job and verify it's the one we pushed
        let popped = queue.pop().await?.unwrap();
        assert_eq!(popped.id(), job_id);

        // Fail the job with an error message
        queue.fail(&job_id, "test error".to_string()).await?;

        // Wait for backoff (2^1 = 2 seconds for first failure)
        sleep(StdDuration::from_secs(4)).await;

        // Job should be available again
        let popped = queue.pop().await?.unwrap();
        assert_eq!(popped.id(), job_id);

        Ok(())
    }
}
