use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json;
use sled::{Db, IVec};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tempfile;
use testing;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task;
use uuid::Uuid;

mod traits;
pub use traits::*;

// ============================
// ==== Job Implementation ====
// ============================

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Job {
    id: String,
    payload: String,
    attempts: u32,
    max_attempts: u32,
}

impl Job {
    pub fn new(payload: String, max_attempts: u32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            payload,
            attempts: 0,
            max_attempts,
        }
    }
}

#[async_trait]
impl JobHandler for Job {
    async fn handle(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Handling job {} with payload: {}", self.id, self.payload);
        if self.payload.contains("fail") {
            Err("Simulated job failure.".into())
        } else {
            tokio::time::sleep(Duration::from_secs(2)).await;
            Ok(())
        }
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn payload(&self) -> &str {
        &self.payload
    }

    fn attempts(&self) -> u32 {
        self.attempts
    }

    fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    fn increment_attempts(&mut self) {
        self.attempts += 1;
    }
}

// ============================
// ==== SledQueue Implementation
// ============================

#[derive(Clone)]
pub struct SledQueueImpl {
    db: Arc<Db>,
    queue_name: String,
    lock: Arc<Mutex<()>>,
}

impl SledQueueImpl {
    pub fn new(db: Db, queue_name: &str) -> Self {
        Self {
            db: Arc::new(db),
            queue_name: queue_name.into(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn new_test_queue() -> Self {
        let temp_dir = tempfile::tempdir().unwrap();
        let db = sled::open(temp_dir.path()).unwrap();
        Self::new(db, "test-queue")
    }
}

#[async_trait]
impl Queue for SledQueueImpl {
    async fn push(&self, job: Box<dyn JobHandler>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let key = format!("{}/{}", self.queue_name, timestamp);

        // Serialize job data
        let job_data = serde_json::to_vec(&Job {
            id: job.id().to_string(),
            payload: job.payload().to_string(),
            attempts: job.attempts(),
            max_attempts: job.max_attempts(),
        })?;

        self.db.insert(key, job_data)?;
        self.db.flush()?;
        Ok(())
    }

    async fn pop(&self) -> Result<Option<Box<dyn JobHandler>>, Box<dyn std::error::Error + Send + Sync>> {
        let _guard = self.lock.lock().await;
        let prefix = format!("{}/", self.queue_name);
        let mut iter = self.db.scan_prefix(&prefix).keys();

        if let Some(Ok(first_key)) = iter.next() {
            let key_str = String::from_utf8(first_key.to_vec())?;
            if let Some(value) = self.db.remove(&key_str)? {
                let job: Job = serde_json::from_slice(&value)?;
                return Ok(Some(Box::new(job)));
            }
        }

        Ok(None)
    }

    async fn peek(&self) -> Result<Option<Box<dyn JobHandler>>, Box<dyn std::error::Error + Send + Sync>> {
        let prefix = format!("{}/", self.queue_name);
        let mut iter = self.db.scan_prefix(&prefix);

        if let Some(Ok((_, value))) = iter.next() {
            let job: Job = serde_json::from_slice(&value)?;
            return Ok(Some(Box::new(job)));
        }

        Ok(None)
    }

    async fn size(&self) -> usize {
        let prefix = format!("{}/", self.queue_name);
        self.db.scan_prefix(&prefix).count()
    }

    async fn complete(&self, _job_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Jobs are automatically removed when popped
        Ok(())
    }

    async fn fail(&self, job_id: &str, error: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let failed_key = format!("{}/failed/{}", self.queue_name, job_id);
        let failed_job = FailedJob {
            id: job_id.to_string(),
            connection: "default".to_string(),
            queue: self.queue_name.clone(),
            payload: "".to_string(), // TODO: Get from original job
            exception: error,
            failed_at: Utc::now().to_rfc3339(),
        };

        let serialized = serde_json::to_vec(&failed_job)?;
        self.db.insert(failed_key, serialized)?;
        self.db.flush()?;
        Ok(())
    }

    async fn get_failed(&self) -> Result<Vec<(Box<dyn JobHandler>, String)>, Box<dyn std::error::Error + Send + Sync>> {
        let prefix = format!("{}/failed/", self.queue_name);
        let mut failed_jobs = Vec::new();

        for result in self.db.scan_prefix(&prefix) {
            let (_, value) = result?;
            let failed_job: FailedJob = serde_json::from_slice(&value)?;
            let job = Job::new(failed_job.payload, 3); // TODO: Use original max_attempts
            failed_jobs.push((Box::new(job) as Box<dyn JobHandler>, failed_job.exception));
        }

        Ok(failed_jobs)
    }

    async fn retry(&self, job_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let failed_key = format!("{}/failed/{}", self.queue_name, job_id);

        if let Some(failed_data) = self.db.remove(failed_key)? {
            let failed_job: FailedJob = serde_json::from_slice(&failed_data)?;
            let job = Job::new(failed_job.payload, 3); // Reset attempts
            self.push(Box::new(job)).await?;
            return Ok(true);
        }

        Ok(false)
    }
}

// ============================
// ==== Failed Job Structure ==
// ============================

#[derive(Serialize, Deserialize, Debug)]
struct FailedJob {
    id: String,
    connection: String,
    queue: String,
    payload: String,
    exception: String,
    failed_at: String,
}
