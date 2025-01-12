pub mod traits;
pub use traits::{JobHandler, Queue, Result};

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sled::Batch;
use std::collections::HashSet;
use std::any::Any;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Job {
    pub id: String,
    pub payload: String,
    pub attempts: u32,
    pub max_attempts: u32,
    pub priority: i32,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub available_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    pub tags: HashSet<String>,
    pub in_progress: bool,
}

impl Job {
    pub fn new(id: String, payload: String) -> Self {
        Self {
            id,
            payload,
            attempts: 0,
            max_attempts: 3,
            priority: 0,
            available_at: Utc::now(),
            created_at: Utc::now(),
            tags: HashSet::new(),
            in_progress: false,
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.available_at = Utc::now() + delay;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags.into_iter().collect();
        self
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }
}

#[async_trait]
impl JobHandler for Job {
    fn id(&self) -> String {
        self.id()
    }

    async fn handle(&self) -> Result<()> {
        println!("Handling job {} with payload {}", self.id, self.payload);
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct SledQueueImpl {
    db: sled::Db,
    queue_name: String,
}

impl Clone for SledQueueImpl {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            queue_name: self.queue_name.clone(),
        }
    }
}

impl SledQueueImpl {
    pub fn new(queue_name: String, db_path: String) -> Result<Self> {
        let db = sled::open(db_path)?;
        Ok(Self {
            db,
            queue_name,
        })
    }

    fn make_job_key(&self, job: &Job) -> String {
        format!("{}:jobs:{}:{}:{}",
            self.queue_name,
            -job.priority,
            job.available_at.timestamp_millis(),
            job.id
        )
    }

    fn make_prefix(&self) -> String {
        format!("{}:jobs:", self.queue_name)
    }

    fn make_failed_key(&self, job_id: &str) -> String {
        format!("{}:failed:{}", self.queue_name, job_id)
    }

    async fn find_job_by_id(&self, job_id: &str) -> Result<Option<(sled::IVec, Job)>> {
        let prefix = self.make_prefix();
        println!("Scanning for job {} with prefix {}", job_id, prefix);
        for result in self.db.scan_prefix(prefix.as_bytes()) {
            let (key, value) = result?;
            let job: Job = serde_json::from_slice(&value)?;
            println!("Found job with id: {} at key {:?}", job.id, key);
            if job.id == job_id {
                println!("Found matching job: {:?}", job);
                return Ok(Some((key, job)));
            }
        }
        println!("No job found with id: {}", job_id);
        Ok(None)
    }

    async fn serialize_job(&self, job: &Job) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&job)?)
    }
}

#[async_trait]
impl Queue for SledQueueImpl {
    async fn push(&self, job: Box<dyn JobHandler>) -> Result<()> {
        let job = job.as_any().downcast_ref::<Job>().ok_or("Invalid job type")?;
        let key = self.make_job_key(job);
        let value = self.serialize_job(job).await?;
        self.db.insert(key.as_bytes(), value)?;
        Ok(())
    }

    async fn push_bulk(&self, jobs: Vec<Box<dyn JobHandler>>) -> Result<()> {
        let mut batch = Batch::default();
        for job in jobs {
            let job = job.as_any().downcast_ref::<Job>().ok_or("Invalid job type")?;
            let key = self.make_job_key(job);
            let value = self.serialize_job(job).await?;
            batch.insert(key.as_bytes(), value);
        }
        self.db.apply_batch(batch)?;
        Ok(())
    }

    async fn pop(&self) -> Result<Option<Box<dyn JobHandler>>> {
        let now = Utc::now();
        let prefix = self.make_prefix();

        println!("Scanning queue with prefix: {}", prefix);
        for res in self.db.scan_prefix(prefix.as_bytes()) {
            let (key, value) = res?;
            let key_str = String::from_utf8_lossy(&key);
            println!("Found key: {}", key_str);

            let mut job: Job = serde_json::from_slice(&value)?;
            println!("Job available_at: {}, now: {}", job.available_at, now);

            if job.available_at <= now && !job.in_progress {
                job.in_progress = true;
                let serialized = serde_json::to_vec(&job)?;
                self.db.insert(&key, serialized)?;
                println!("Popping job: {:?}", job);
                return Ok(Some(Box::new(job)));
            }
        }
        println!("No available jobs found");
        Ok(None)
    }

    async fn peek(&self) -> Result<Option<Box<dyn JobHandler>>> {
        let now = Utc::now();
        let prefix = self.make_prefix();
        for result in self.db.scan_prefix(prefix.as_bytes()) {
            let (_, value) = result?;
            let job: Job = serde_json::from_slice(&value)?;
            if job.available_at <= now {
                return Ok(Some(Box::new(job)));
            }
        }
        Ok(None)
    }

    async fn size(&self) -> Result<usize> {
        let prefix = self.make_prefix();
        Ok(self.db.scan_prefix(prefix.as_bytes()).count())
    }

    async fn complete(&self, job_id: &str) -> Result<()> {
        if let Some((key, _)) = self.find_job_by_id(job_id).await? {
            self.db.remove(key)?;
        }
        Ok(())
    }

    async fn fail(&self, job_id: &str, error: String) -> Result<()> {
        println!("Attempting to fail job with id: {}", job_id);

        if let Some((key, mut job)) = self.find_job_by_id(job_id).await? {
            println!("Found job to fail: {:?} at key {:?}", job, key);

            // Increment attempts
            job.attempts += 1;
            job.in_progress = false;

            // Prepare batch operations
            let mut batch = Batch::default();
            batch.remove(&key);

            if job.attempts >= job.max_attempts {
                println!("Job exceeded max attempts, moving to failed queue");
                let failed_key = self.make_failed_key(job_id);
                let failed_value = serde_json::to_vec(&(job, error))?;
                batch.insert(failed_key.as_bytes(), failed_value);
            } else {
                // Calculate backoff delay
                let backoff = Duration::seconds(2i64.pow(job.attempts));
                job.available_at = Utc::now() + backoff;
                println!("New available_at: {}", job.available_at);

                // Generate new key with updated available_at
                let new_key = self.make_job_key(&job);
                println!("Inserting job at new key: {}", new_key);

                // Add to batch
                let serialized = serde_json::to_vec(&job)?;
                batch.insert(new_key.as_bytes(), serialized);
                println!("Job re-added with backoff");
            }

            // Apply all operations atomically
            self.db.apply_batch(batch)?;
            println!("Job updated successfully");
            Ok(())
        } else {
            println!("No job found to fail with id: {}", job_id);
            println!("Current jobs in queue:");
            for result in self.db.scan_prefix(self.make_prefix().as_bytes()) {
                let (key, value) = result?;
                let job: Job = serde_json::from_slice(&value)?;
                println!("Key: {:?}, Job: {:?}", key, job);
            }
            Ok(())
        }
    }

    async fn get_failed(&self) -> Result<Vec<(Box<dyn JobHandler>, String)>> {
        let prefix = format!("{}:failed:", self.queue_name);
        let mut failed = Vec::new();
        for result in self.db.scan_prefix(prefix.as_bytes()) {
            let (_, value) = result?;
            let (job, error): (Job, String) = serde_json::from_slice(&value)?;
            failed.push((Box::new(job) as Box<dyn JobHandler>, error));
        }
        Ok(failed)
    }

    async fn retry(&self, job_id: &str) -> Result<bool> {
        let failed_key = self.make_failed_key(job_id);
        if let Some(value) = self.db.get(failed_key.as_bytes())? {
            let (mut job, _): (Job, String) = serde_json::from_slice(&value)?;
            job.attempts = 0;
            job.available_at = Utc::now();
            let key = self.make_job_key(&job);
            let value = self.serialize_job(&job).await?;
            self.db.insert(key.as_bytes(), value)?;
            self.db.remove(failed_key.as_bytes())?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn get_by_tag(&self, tag: &str) -> Result<Vec<Box<dyn JobHandler>>> {
        let prefix = self.make_prefix();
        let mut tagged_jobs = Vec::new();
        for result in self.db.scan_prefix(prefix.as_bytes()) {
            let (_, value) = result?;
            let job: Job = serde_json::from_slice(&value)?;
            if job.tags.contains(tag) {
                tagged_jobs.push(Box::new(job) as Box<dyn JobHandler>);
            }
        }
        Ok(tagged_jobs)
    }

    async fn clear(&self) -> Result<()> {
        let prefix = self.make_prefix();
        for result in self.db.scan_prefix(prefix.as_bytes()) {
            let (key, _) = result?;
            self.db.remove(key)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
