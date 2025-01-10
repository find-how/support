use async_trait::async_trait;
use std::error::Error;
use std::fmt::Debug;

/// Represents a job that can be processed by the queue system
#[async_trait]
pub trait JobHandler: Send + Sync + Debug {
    /// Process the job
    async fn handle(&self) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Get the job ID
    fn id(&self) -> &str;

    /// Get the job payload
    fn payload(&self) -> &str;

    /// Get the number of attempts
    fn attempts(&self) -> u32;

    /// Get the maximum number of attempts
    fn max_attempts(&self) -> u32;

    /// Increment the attempt counter
    fn increment_attempts(&mut self);

    /// Check if the job has exceeded max attempts
    fn has_exceeded_max_attempts(&self) -> bool {
        self.attempts() >= self.max_attempts()
    }
}

/// Worker that processes jobs from the queue
#[async_trait]
pub trait Worker: Send + Sync {
    /// Start the worker
    async fn start(&self) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Stop the worker
    async fn stop(&self) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Process a single job
    async fn process_job(&self, job: Box<dyn JobHandler>) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Get the worker ID
    fn id(&self) -> &str;

    /// Get the number of jobs processed
    fn jobs_processed(&self) -> usize;

    /// Get the number of jobs failed
    fn jobs_failed(&self) -> usize;
}

/// Middleware for intercepting and modifying queue operations
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Called before a job is processed
    async fn before_process(&self, job: &Box<dyn JobHandler>) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Called after a job is processed
    async fn after_process(&self, job: &Box<dyn JobHandler>, result: &Result<(), Box<dyn Error + Send + Sync>>) -> Result<(), Box<dyn Error + Send + Sync>>;
}

/// Handler for queue events
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Called when a job is queued
    async fn on_queued(&self, job: &Box<dyn JobHandler>);

    /// Called when a job starts processing
    async fn on_processing(&self, job: &Box<dyn JobHandler>);

    /// Called when a job completes successfully
    async fn on_complete(&self, job: &Box<dyn JobHandler>);

    /// Called when a job fails
    async fn on_failed(&self, job: &Box<dyn JobHandler>, error: &Box<dyn Error + Send + Sync>);

    /// Called when a job is retried
    async fn on_retry(&self, job: &Box<dyn JobHandler>);
}

/// Storage backend for the queue
#[async_trait]
pub trait Storage: Send + Sync {
    /// Store a job
    async fn store_job(&self, job: Box<dyn JobHandler>) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Retrieve a job by ID
    async fn get_job(&self, id: &str) -> Result<Option<Box<dyn JobHandler>>, Box<dyn Error + Send + Sync>>;

    /// Remove a job
    async fn remove_job(&self, id: &str) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Store a failed job
    async fn store_failed_job(&self, job: Box<dyn JobHandler>, error: String) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Get all failed jobs
    async fn get_failed_jobs(&self) -> Result<Vec<(Box<dyn JobHandler>, String)>, Box<dyn Error + Send + Sync>>;

    /// Clear all jobs
    async fn clear(&self) -> Result<(), Box<dyn Error + Send + Sync>>;
}

/// Queue interface for job operations
#[async_trait]
pub trait Queue: Send + Sync {
    /// Push a job onto the queue
    async fn push(&self, job: Box<dyn JobHandler>) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Pop a job from the queue
    async fn pop(&self) -> Result<Option<Box<dyn JobHandler>>, Box<dyn Error + Send + Sync>>;

    /// Peek at the next job without removing it
    async fn peek(&self) -> Result<Option<Box<dyn JobHandler>>, Box<dyn Error + Send + Sync>>;

    /// Get the approximate size of the queue
    async fn size(&self) -> usize;

    /// Mark a job as complete
    async fn complete(&self, job_id: &str) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Mark a job as failed
    async fn fail(&self, job_id: &str, error: String) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Get all failed jobs
    async fn get_failed(&self) -> Result<Vec<(Box<dyn JobHandler>, String)>, Box<dyn Error + Send + Sync>>;

    /// Retry a failed job
    async fn retry(&self, job_id: &str) -> Result<bool, Box<dyn Error + Send + Sync>>;
}
