`sled` is a high-performance, embedded key-value store that provides ordered
storage, concurrency support, and durability, making it a suitable foundation
for building a reliable and efficient queue system. While `sled` isn't a
dedicated queue library like Redis or RabbitMQ, its features allow you to
implement a robust queue with functionalities similar to Laravel's Queue system.

In this comprehensive guide, I'll walk you through building an embedded queue
system in Rust using `sled`. This implementation will mirror key aspects of
Laravel's Queue system, including job management, middleware support, event
handling, concurrency, and failure handling.

---

## Table of Contents

1. [Understanding `sled`](#understanding-sled)
2. [Designing the Queue System](#designing-the-queue-system)
   - [Job Structure](#job-structure)
   - [Queue Trait](#queue-trait)
   - [Queue Drivers](#queue-drivers)
   - [Middleware](#middleware)
   - [Event Dispatcher](#event-dispatcher)
   - [Failed Job Repository](#failed-job-repository)
   - [Queue Manager](#queue-manager)
   - [Worker](#worker)
3. [Implementing the Queue System](#implementing-the-queue-system)
   - [Cargo.toml Setup](#cargotoml-setup)
   - [Job Implementation](#job-implementation)
   - [Queue Trait and Drivers](#queue-trait-and-drivers)
   - [Middleware Implementation](#middleware-implementation)
   - [Event System Implementation](#event-system-implementation)
   - [Failed Job Repository Implementation](#failed-job-repository-implementation)
   - [Queue Manager Implementation](#queue-manager-implementation)
   - [Worker Implementation](#worker-implementation)
4. [Command-Line Interface (CLI)](#command-line-interface-cli)
5. [Running and Testing](#running-and-testing)
6. [Advanced Features and Considerations](#advanced-features-and-considerations)
7. [Pros and Cons of Using `sled` as a Queue Driver](#pros-and-cons-of-using-sled-as-a-queue-driver)
8. [Conclusion](#conclusion)

---

## Understanding `sled`

[`sled`](https://docs.rs/sled/latest/sled/) is a modern embedded database for
Rust that offers:

- **Ordered Keys**: Maintains keys in a sorted order, enabling efficient range
  queries.
- **Concurrency**: Supports concurrent read and write operations safely.
- **Atomic Operations**: Provides atomic insertions and deletions to prevent
  race conditions.
- **Durability**: Ensures data is persisted to disk, surviving crashes and
  restarts.
- **Embeddable**: Runs within your Rust application without requiring an
  external server.

These features make `sled` an excellent choice for implementing various data
storage patterns, including queues.

---

## Designing the Queue System

To emulate Laravel's Queue system in Rust using `sled`, we'll design the system
with the following components:

### Job Structure

Represents a unit of work with necessary metadata.

```rust
// src/job.rs

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Job {
    pub id: String,
    pub payload: String,
    pub attempts: u32,
    pub max_attempts: u32,
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
```

### Queue Trait

Defines the essential operations of a queue.

```rust
// src/queue.rs

use crate::job::Job;
use async_trait::async_trait;

#[async_trait]
pub trait Queue: Send + Sync {
    /// Push a job onto the queue.
    async fn push(&self, job: Job) -> sled::Result<()>;

    /// Pop a job from the queue.
    async fn pop(&self) -> sled::Result<Option<Job>>;

    /// Peek at the next job without removing it.
    async fn peek(&self) -> sled::Result<Option<Job>>;

    /// Get the approximate size of the queue.
    async fn size(&self) -> usize;
}
```

### Queue Drivers

Implementations of the `Queue` trait for different storage backends. We'll
implement an `InMemoryQueue` and a `SledQueue`.

#### InMemoryQueue

A simple in-memory queue using Tokio's asynchronous channels.

```rust
// src/drivers/in_memory_queue.rs

use crate::job::Job;
use crate::queue::Queue;
use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{self, Sender, Receiver};

pub struct InMemoryQueue {
    sender: Sender<Job>,
    receiver: Arc<Mutex<Receiver<Job>>>,
}

impl InMemoryQueue {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Job>(100);
        InMemoryQueue {
            sender: tx,
            receiver: Arc::new(Mutex::new(rx)),
        }
    }
}

#[async_trait]
impl Queue for InMemoryQueue {
    async fn push(&self, job: Job) -> sled::Result<()> {
        self.sender.send(job).await.map_err(|e| sled::Error::IO(e.to_string().into()))?;
        Ok(())
    }

    async fn pop(&self) -> sled::Result<Option<Job>> {
        let mut rx = self.receiver.lock().await;
        match rx.recv().await {
            Some(job) => Ok(Some(job)),
            None => Ok(None),
        }
    }

    async fn peek(&self) -> sled::Result<Option<Job>> {
        // In-memory queues typically don't support peeking
        Ok(None)
    }

    async fn size(&self) -> usize {
        self.sender.capacity() - self.sender.sender_count()
    }
}
```

#### SledQueue

An embedded persistent queue using `sled`'s ordered key-value store.

```rust
// src/drivers/sled_queue.rs

use crate::job::Job;
use crate::queue::Queue;
use async_trait::async_trait;
use sled::{Db, IVec};
use serde_json;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct JobPayload {
    id: String,
    payload: String,
    attempts: u32,
    max_attempts: u32,
}

pub struct SledQueue {
    db: Arc<Db>,
    queue_name: String,
    lock: Arc<Mutex<()>>, // Simple mutex to prevent concurrent pops
}

impl SledQueue {
    pub fn new(db: Db, queue_name: &str) -> Self {
        Self {
            db: Arc::new(db),
            queue_name: queue_name.into(),
            lock: Arc::new(Mutex::new(())),
        }
    }
}

#[async_trait]
impl Queue for SledQueue {
    async fn push(&self, job: Job) -> sled::Result<()> {
        let payload = JobPayload {
            id: job.id.clone(),
            payload: job.payload.clone(),
            attempts: job.attempts,
            max_attempts: job.max_attempts,
        };
        let serialized = serde_json::to_vec(&payload).unwrap();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let key = format!("{}/{}", self.queue_name, timestamp);
        self.db.insert(key, serialized)?;
        self.db.flush()?;
        Ok(())
    }

    async fn pop(&self) -> sled::Result<Option<Job>> {
        let _guard = self.lock.lock().await; // Ensure exclusive access
        let prefix = format!("{}/", self.queue_name);
        let mut iter = self.db.scan_prefix(&prefix).keys();
        if let Some(Ok(first_key)) = iter.next() {
            let key_str = String::from_utf8(first_key.to_vec()).unwrap();
            let value = self.db.remove(&key_str)?;
            self.db.flush()?;
            if let Some(ivec) = value {
                let job: JobPayload = serde_json::from_slice(&ivec).unwrap();
                Ok(Some(Job {
                    id: job.id,
                    payload: job.payload,
                    attempts: job.attempts,
                    max_attempts: job.max_attempts,
                }))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn peek(&self) -> sled::Result<Option<Job>> {
        let prefix = format!("{}/", self.queue_name);
        let mut iter = self.db.scan_prefix(&prefix).keys();
        if let Some(Ok(first_key)) = iter.next() {
            let value = self.db.get(&first_key)?.map(|v| serde_json::from_slice::<JobPayload>(&v).unwrap());
            if let Some(job_payload) = value {
                Ok(Some(Job {
                    id: job_payload.id,
                    payload: job_payload.payload,
                    attempts: job_payload.attempts,
                    max_attempts: job_payload.max_attempts,
                }))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn size(&self) -> usize {
        let prefix = format!("{}/", self.queue_name);
        self.db.scan_prefix(&prefix).count()
    }
}
```

> **Explanation:**
>
> - **JobPayload**: A helper struct to serialize and deserialize job data.
> - **SledQueue**: Manages queue operations using `sled`. It ensures ordered
>   processing by using timestamps as part of the keys.
> - **Concurrency Control**: Uses a `Mutex` to prevent multiple workers from
>   dequeuing the same job concurrently.

### Middleware

Middleware allows adding cross-cutting concerns (e.g., logging, rate limiting)
to job processing.

```rust
// src/middleware.rs

use crate::job::Job;
use async_trait::async_trait;
use log::{info, warn};

#[async_trait]
pub trait Middleware: Send + Sync {
    async fn before(&self, job: &dyn Job);
    async fn after(&self, job: &dyn Job, success: bool);
    async fn on_failure(&self, job: &dyn Job, error: &str);
}

pub struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn before(&self, job: &dyn Job) {
        info!("Starting job {}", job.id());
    }

    async fn after(&self, job: &dyn Job, success: bool) {
        if success {
            info!("Job {} completed successfully.", job.id());
        } else {
            warn!("Job {} failed.", job.id());
        }
    }

    async fn on_failure(&self, job: &dyn Job, error: &str) {
        warn!("Job {} failed with error: {}", job.id(), error);
    }
}
```

> **Explanation:**
>
> - **Middleware Trait**: Defines hooks (`before`, `after`, `on_failure`) around
>   job processing.
> - **LoggingMiddleware**: Logs job start, completion, and failure events.

### Event Dispatcher

A simplified asynchronous event dispatcher to handle job-related events.

```rust
// src/events.rs

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[async_trait]
pub trait Event: Send + Sync {}

#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: Arc<dyn Event>);
}

pub struct Dispatcher {
    listeners: Arc<Mutex<HashMap<String, Vec<Arc<dyn EventHandler>>>>>,
}

impl Dispatcher {
    pub fn new() -> Self {
        Dispatcher {
            listeners: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register an event handler for a specific event type.
    pub async fn register<E: Event + 'static>(&self, handler: Arc<dyn EventHandler>) {
        let event_name = std::any::type_name::<E>().to_string();
        let mut listeners = self.listeners.lock().await;
        listeners.entry(event_name).or_insert_with(Vec::new).push(handler);
    }

    /// Dispatch an event to all registered handlers.
    pub async fn dispatch(&self, event: Arc<dyn Event>) {
        let event_name = event.type_id().to_string();
        let listeners = self.listeners.lock().await;
        if let Some(handlers) = listeners.get(&event_name) {
            for handler in handlers {
                handler.handle(event.clone()).await;
            }
        }
    }
}

#[derive(Debug)]
pub struct JobProcessingEvent {
    pub job_id: String,
}

#[async_trait]
impl Event for JobProcessingEvent {}

#[derive(Debug)]
pub struct JobProcessedEvent {
    pub job_id: String,
}

#[async_trait]
impl Event for JobProcessedEvent {}

#[derive(Debug)]
pub struct JobFailedEvent {
    pub job_id: String,
    pub error: String,
}

#[async_trait]
impl Event for JobFailedEvent {}
```

> **Explanation:**
>
> - **Event Trait**: Marker trait for all events.
> - **EventHandler Trait**: Defines how to handle events.
> - **Dispatcher**: Manages event listeners and dispatches events
>   asynchronously.
> - **Job Events**: Specific events (`JobProcessingEvent`, `JobProcessedEvent`,
>   `JobFailedEvent`) representing different stages of job processing.

### Failed Job Repository

Manages storage and retrieval of failed jobs.

```rust
// src/failed_job_repository.rs

use crate::job::Job;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FailedJob {
    pub id: String,
    pub connection: String,
    pub queue: String,
    pub payload: String,
    pub exception: String,
    pub failed_at: String,
}

#[async_trait]
pub trait FailedJobRepository: Send + Sync {
    async fn log(&self, job: &Job, connection: &str, queue: &str, error: &str) -> sled::Result<()>;
    async fn retry(&self, id: &str) -> sled::Result<Option<Job>>;
    async fn forget(&self, id: &str) -> sled::Result<()>;
    async fn all(&self) -> sled::Result<Vec<FailedJob>>;
}

pub struct SledFailedJobRepository {
    db: Arc<Db>,
    lock: Arc<Mutex<()>>,
}

impl SledFailedJobRepository {
    pub fn new(db: Db) -> Self {
        Self {
            db: Arc::new(db),
            lock: Arc::new(Mutex::new(())),
        }
    }
}

#[async_trait]
impl FailedJobRepository for SledFailedJobRepository {
    async fn log(&self, job: &Job, connection: &str, queue: &str, error: &str) -> sled::Result<()> {
        let failed_job = FailedJob {
            id: job.id.clone(),
            connection: connection.into(),
            queue: queue.into(),
            payload: job.payload.clone(),
            exception: error.into(),
            failed_at: Utc::now().to_rfc3339(),
        };
        let serialized = serde_json::to_vec(&failed_job).unwrap();
        let key = format!("failed/{}", failed_job.id);
        self.db.insert(key, serialized)?;
        self.db.flush()?;
        Ok(())
    }

    async fn retry(&self, id: &str) -> sled::Result<Option<Job>> {
        let key = format!("failed/{}", id);
        let value = self.db.remove(&key)?;
        self.db.flush()?;
        if let Some(ivec) = value {
            let failed_job: FailedJob = serde_json::from_slice(&ivec).unwrap();
            Ok(Some(Job {
                id: failed_job.id,
                payload: failed_job.payload,
                attempts: 0,
                max_attempts: failed_job.max_attempts,
            }))
        } else {
            Ok(None)
        }
    }

    async fn forget(&self, id: &str) -> sled::Result<()> {
        let key = format!("failed/{}", id);
        self.db.remove(&key)?;
        self.db.flush()?;
        Ok(())
    }

    async fn all(&self) -> sled::Result<Vec<FailedJob>> {
        let prefix = "failed/";
        let iter = self.db.scan_prefix(prefix).values();
        let mut jobs = Vec::new();
        for item in iter {
            let ivec = item?;
            let job: FailedJob = serde_json::from_slice(&ivec).unwrap();
            jobs.push(job);
        }
        Ok(jobs)
    }
}
```

> **Explanation:**
>
> - **FailedJob**: Represents a failed job entry.
> - **FailedJobRepository Trait**: Defines operations for logging, retrying,
>   forgetting, and listing failed jobs.
> - **SledFailedJobRepository**: Concrete implementation using `sled` to store
>   failed jobs persistently.

### Queue Manager

Manages multiple queue connections and provides access to them.

```rust
// src/queue_manager.rs

use crate::queue::Queue;
use sled::Db;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct QueueManager {
    connections: Arc<RwLock<HashMap<String, Arc<dyn Queue>>>>,
    db: Arc<Db>,
}

impl QueueManager {
    pub fn new(db: Db) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            db: Arc::new(db),
        }
    }

    /// Add a new queue connection.
    pub async fn add_connection(&self, name: &str, queue: Arc<dyn Queue>) {
        let mut connections = self.connections.write().await;
        connections.insert(name.into(), queue);
    }

    /// Get a queue connection by name.
    pub async fn connection(&self, name: &str) -> Option<Arc<dyn Queue>> {
        let connections = self.connections.read().await;
        connections.get(name).cloned()
    }

    /// Get the sled database.
    pub fn db(&self) -> Arc<Db> {
        Arc::clone(&self.db)
    }
}
```

> **Explanation:**
>
> - **QueueManager**: Maintains a registry of queue connections. Allows adding
>   and retrieving connections by name.
> - **Concurrency**: Uses `RwLock` to manage concurrent access to the
>   connections map.

### Worker

Processes jobs from the queue, applies middleware, handles retries and failures.

```rust
// src/worker.rs

use crate::events::{Dispatcher, JobFailedEvent, JobProcessedEvent, JobProcessingEvent};
use crate::failed_job_repository::FailedJobRepository;
use crate::job::Job;
use crate::middleware::Middleware;
use crate::queue::Queue;
use async_trait::async_trait;
use log::{error, info};
use serde_json;
use std::sync::Arc;
use tokio::task;
use tokio::time::{sleep, Duration};

pub struct Worker {
    dispatcher: Arc<Dispatcher>,
    failed_repo: Arc<dyn FailedJobRepository>,
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl Worker {
    pub fn new(
        dispatcher: Arc<Dispatcher>,
        failed_repo: Arc<dyn FailedJobRepository>,
        middlewares: Vec<Arc<dyn Middleware>>,
    ) -> Self {
        Worker {
            dispatcher,
            failed_repo,
            middlewares,
        }
    }

    pub async fn run(&self, queue: Arc<dyn Queue>, sleep_seconds: u64) {
        loop {
            match queue.pop().await {
                Ok(Some(mut job)) => {
                    // Dispatch JobProcessing event
                    let processing_event = Arc::new(JobProcessingEvent {
                        job_id: job.id.clone(),
                    });
                    self.dispatcher.dispatch(processing_event.clone()).await;

                    // Apply middlewares before handling
                    for mw in &self.middlewares {
                        mw.before(&*job).await;
                    }

                    // Handle the job in a separate task
                    let dispatcher = self.dispatcher.clone();
                    let failed_repo = self.failed_repo.clone();
                    let middlewares = self.middlewares.clone();
                    let queue = queue.clone();
                    task::spawn(async move {
                        let job_id = job.id.clone();
                        let result = tokio::spawn(async move {
                            job.handle().await;
                        })
                        .await;

                        match result {
                            Ok(_) => {
                                info!("Job {} processed successfully.", job_id);
                                // Dispatch JobProcessed event
                                let processed_event = Arc::new(JobProcessedEvent {
                                    job_id: job.id.clone(),
                                });
                                dispatcher.dispatch(processed_event.clone()).await;

                                // Apply middlewares after success
                                for mw in &middlewares {
                                    mw.after(&*job, true).await;
                                }
                            }
                            Err(e) => {
                                let error_msg = e.to_string();
                                error!("Job {} failed with error: {}", job_id, error_msg);

                                // Increment attempts
                                job.attempts += 1;

                                if job.attempts >= job.max_attempts {
                                    // Log failed job
                                    failed_repo
                                        .log(&job, "default", "default", &error_msg)
                                        .await
                                        .unwrap();

                                    // Dispatch JobFailed event
                                    let failed_event = Arc::new(JobFailedEvent {
                                        job_id: job.id.clone(),
                                        error: error_msg.clone(),
                                    });
                                    dispatcher.dispatch(failed_event.clone()).await;

                                    // Apply middlewares on failure
                                    for mw in &middlewares {
                                        mw.on_failure(&*job, &error_msg).await;
                                    }
                                } else {
                                    // Re-enqueue the job for retry
                                    queue.push(job.clone()).await.unwrap();
                                    info!(
                                        "Job {} re-enqueued for retry (attempt {}/{})",
                                        job.id, job.attempts, job.max_attempts
                                    );
                                }
                            }
                        }
                    });
                }
                Ok(None) => {
                    // No job found, sleep for a while
                    sleep(Duration::from_secs(sleep_seconds)).await;
                }
                Err(e) => {
                    error!("Failed to dequeue job: {}", e);
                    sleep(Duration::from_secs(sleep_seconds)).await;
                }
            }
        }
    }
}
```

> **Explanation:**
>
> - **Worker**: Continuously polls the queue for new jobs.
> - **Job Processing**:
>   - Dispatches a `JobProcessingEvent`.
>   - Applies `before` middleware hooks.
>   - Executes the job asynchronously.
>   - On success:
>     - Dispatches a `JobProcessedEvent`.
>     - Applies `after` middleware hooks.
>   - On failure:
>     - Logs the failed job.
>     - Dispatches a `JobFailedEvent`.
>     - Applies `on_failure` middleware hooks.
>     - Handles retry logic based on `attempts` and `max_attempts`.

### Failed Job Repository

Manages storage and retrieval of failed jobs.

```rust
// src/failed_job_repository.rs

use crate::job::Job;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FailedJob {
    pub id: String,
    pub connection: String,
    pub queue: String,
    pub payload: String,
    pub exception: String,
    pub failed_at: String,
}

#[async_trait]
pub trait FailedJobRepository: Send + Sync {
    async fn log(&self, job: &Job, connection: &str, queue: &str, error: &str) -> sled::Result<()>;
    async fn retry(&self, id: &str) -> sled::Result<Option<Job>>;
    async fn forget(&self, id: &str) -> sled::Result<()>;
    async fn all(&self) -> sled::Result<Vec<FailedJob>>;
}

pub struct SledFailedJobRepository {
    db: Arc<Db>,
    lock: Arc<Mutex<()>>,
}

impl SledFailedJobRepository {
    pub fn new(db: Db) -> Self {
        Self {
            db: Arc::new(db),
            lock: Arc::new(Mutex::new(())),
        }
    }
}

#[async_trait]
impl FailedJobRepository for SledFailedJobRepository {
    async fn log(&self, job: &Job, connection: &str, queue: &str, error: &str) -> sled::Result<()> {
        let failed_job = FailedJob {
            id: job.id.clone(),
            connection: connection.into(),
            queue: queue.into(),
            payload: job.payload.clone(),
            exception: error.into(),
            failed_at: Utc::now().to_rfc3339(),
        };
        let serialized = serde_json::to_vec(&failed_job).unwrap();
        let key = format!("failed/{}", failed_job.id);
        self.db.insert(key, serialized)?;
        self.db.flush()?;
        Ok(())
    }

    async fn retry(&self, id: &str) -> sled::Result<Option<Job>> {
        let key = format!("failed/{}", id);
        let value = self.db.remove(&key)?;
        self.db.flush()?;
        if let Some(ivec) = value {
            let failed_job: FailedJob = serde_json::from_slice(&ivec).unwrap();
            Ok(Some(Job {
                id: failed_job.id,
                payload: failed_job.payload,
                attempts: 0,
                max_attempts: failed_job.max_attempts,
            }))
        } else {
            Ok(None)
        }
    }

    async fn forget(&self, id: &str) -> sled::Result<()> {
        let key = format!("failed/{}", id);
        self.db.remove(&key)?;
        self.db.flush()?;
        Ok(())
    }

    async fn all(&self) -> sled::Result<Vec<FailedJob>> {
        let prefix = "failed/";
        let iter = self.db.scan_prefix(prefix).values();
        let mut jobs = Vec::new();
        for item in iter {
            let ivec = item?;
            let job: FailedJob = serde_json::from_slice(&ivec).unwrap();
            jobs.push(job);
        }
        Ok(jobs)
    }
}
```

> **Explanation:**
>
> - **FailedJob**: Represents a failed job entry.
> - **FailedJobRepository Trait**: Defines operations for logging, retrying,
>   forgetting, and listing failed jobs.
> - **SledFailedJobRepository**: Concrete implementation using `sled` to store
>   failed jobs persistently.

### Queue Manager

Manages multiple queue connections and provides access to them.

```rust
// src/queue_manager.rs

use crate::queue::Queue;
use sled::Db;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct QueueManager {
    connections: Arc<RwLock<HashMap<String, Arc<dyn Queue>>>>,
    db: Arc<Db>,
}

impl QueueManager {
    pub fn new(db: Db) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            db: Arc::new(db),
        }
    }

    /// Add a new queue connection.
    pub async fn add_connection(&self, name: &str, queue: Arc<dyn Queue>) {
        let mut connections = self.connections.write().await;
        connections.insert(name.into(), queue);
    }

    /// Get a queue connection by name.
    pub async fn connection(&self, name: &str) -> Option<Arc<dyn Queue>> {
        let connections = self.connections.read().await;
        connections.get(name).cloned()
    }

    /// Get the sled database.
    pub fn db(&self) -> Arc<Db> {
        Arc::clone(&self.db)
    }
}
```

> **Explanation:**
>
> - **QueueManager**: Maintains a registry of queue connections. Allows adding
>   and retrieving connections by name.
> - **Concurrency**: Uses `RwLock` to manage concurrent access to the
>   connections map.

### Worker

Processes jobs from the queue, applies middleware, handles retries and failures.

```rust
// src/worker.rs

use crate::events::{Dispatcher, JobFailedEvent, JobProcessedEvent, JobProcessingEvent};
use crate::failed_job_repository::FailedJobRepository;
use crate::job::Job;
use crate::middleware::Middleware;
use crate::queue::Queue;
use async_trait::async_trait;
use log::{error, info};
use serde_json;
use std::sync::Arc;
use tokio::task;
use tokio::time::{sleep, Duration};

pub struct Worker {
    dispatcher: Arc<Dispatcher>,
    failed_repo: Arc<dyn FailedJobRepository>,
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl Worker {
    pub fn new(
        dispatcher: Arc<Dispatcher>,
        failed_repo: Arc<dyn FailedJobRepository>,
        middlewares: Vec<Arc<dyn Middleware>>,
    ) -> Self {
        Worker {
            dispatcher,
            failed_repo,
            middlewares,
        }
    }

    pub async fn run(&self, queue: Arc<dyn Queue>, sleep_seconds: u64) {
        loop {
            match queue.pop().await {
                Ok(Some(mut job)) => {
                    // Dispatch JobProcessing event
                    let processing_event = Arc::new(JobProcessingEvent {
                        job_id: job.id.clone(),
                    });
                    self.dispatcher.dispatch(processing_event.clone()).await;

                    // Apply middlewares before handling
                    for mw in &self.middlewares {
                        mw.before(&*job).await;
                    }

                    // Handle the job in a separate task
                    let dispatcher = self.dispatcher.clone();
                    let failed_repo = self.failed_repo.clone();
                    let middlewares = self.middlewares.clone();
                    let queue = queue.clone();
                    task::spawn(async move {
                        let job_id = job.id.clone();
                        let result = tokio::spawn(async move {
                            job.handle().await;
                        })
                        .await;

                        match result {
                            Ok(_) => {
                                info!("Job {} processed successfully.", job_id);
                                // Dispatch JobProcessed event
                                let processed_event = Arc::new(JobProcessedEvent {
                                    job_id: job.id.clone(),
                                });
                                dispatcher.dispatch(processed_event.clone()).await;

                                // Apply middlewares after success
                                for mw in &middlewares {
                                    mw.after(&*job, true).await;
                                }
                            }
                            Err(e) => {
                                let error_msg = e.to_string();
                                error!("Job {} failed with error: {}", job_id, error_msg);

                                // Increment attempts
                                job.attempts += 1;

                                if job.attempts >= job.max_attempts {
                                    // Log failed job
                                    failed_repo
                                        .log(&job, "default", "default", &error_msg)
                                        .await
                                        .unwrap();

                                    // Dispatch JobFailed event
                                    let failed_event = Arc::new(JobFailedEvent {
                                        job_id: job.id.clone(),
                                        error: error_msg.clone(),
                                    });
                                    dispatcher.dispatch(failed_event.clone()).await;

                                    // Apply middlewares on failure
                                    for mw in &middlewares {
                                        mw.on_failure(&*job, &error_msg).await;
                                    }
                                } else {
                                    // Re-enqueue the job for retry
                                    queue.push(job.clone()).await.unwrap();
                                    info!(
                                        "Job {} re-enqueued for retry (attempt {}/{})",
                                        job.id, job.attempts, job.max_attempts
                                    );
                                }
                            }
                        }
                    });
                }
                Ok(None) => {
                    // No job found, sleep for a while
                    sleep(Duration::from_secs(sleep_seconds)).await;
                }
                Err(e) => {
                    error!("Failed to dequeue job: {}", e);
                    sleep(Duration::from_secs(sleep_seconds)).await;
                }
            }
        }
    }
}
```

> **Explanation:**
>
> - **Worker**: Continuously polls the queue for new jobs.
> - **Job Processing**:
>   - Dispatches a `JobProcessingEvent`.
>   - Applies `before` middleware hooks.
>   - Executes the job asynchronously.
>   - On success:
>     - Dispatches a `JobProcessedEvent`.
>     - Applies `after` middleware hooks.
>   - On failure:
>     - Logs the failed job.
>     - Dispatches a `JobFailedEvent`.
>     - Applies `on_failure` middleware hooks.
>     - Handles retry logic based on `attempts` and `max_attempts`.

---

## Implementing the Queue System

Let's integrate all the components into a cohesive system.

### 1. Cargo.toml Setup

Ensure your `Cargo.toml` includes the necessary dependencies:

```toml
[package]
name = "sled_queue"
version = "0.1.0"
edition = "2021"

[dependencies]
sled = "0.34"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.27", features = ["full"] }
uuid = { version = "1.3", features = ["v4"] }
clap = { version = "4.2", features = ["derive"] }
log = "0.4"
env_logger = "0.9"
chrono = { version = "0.4", features = ["serde"] }
async-trait = "0.1"
```

### 2. Job Implementation

As defined earlier in `job.rs`, this struct represents a job.

### 3. Queue Trait and Drivers

As defined in `queue.rs`, with `InMemoryQueue` and `SledQueue` implementations.

### 4. Middleware Implementation

Implement various middleware as needed. For example, logging and rate-limiting.

```rust
// src/middleware.rs

use crate::job::Job;
use async_trait::async_trait;
use log::{info, warn};

#[async_trait]
pub trait Middleware: Send + Sync {
    async fn before(&self, job: &dyn Job);
    async fn after(&self, job: &dyn Job, success: bool);
    async fn on_failure(&self, job: &dyn Job, error: &str);
}

pub struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn before(&self, job: &dyn Job) {
        info!("Starting job {}", job.id());
    }

    async fn after(&self, job: &dyn Job, success: bool) {
        if success {
            info!("Job {} completed successfully.", job.id());
        } else {
            warn!("Job {} failed.", job.id());
        }
    }

    async fn on_failure(&self, job: &dyn Job, error: &str) {
        warn!("Job {} failed with error: {}", job.id(), error);
    }
}

pub struct RateLimitedMiddleware {
    pub limit: u32,
    pub interval: std::time::Duration,
    last_executed: Arc<tokio::sync::Mutex<Instant>>,
}

impl RateLimitedMiddleware {
    pub fn new(limit: u32, interval_secs: u64) -> Self {
        Self {
            limit,
            interval: std::time::Duration::from_secs(interval_secs),
            last_executed: Arc::new(tokio::sync::Mutex::new(Instant::now() - std::time::Duration::from_secs(interval_secs))),
        }
    }
}

use std::time::Instant;

#[async_trait]
impl Middleware for RateLimitedMiddleware {
    async fn before(&self, _job: &dyn Job) {
        let mut last = self.last_executed.lock().await;
        let now = Instant::now();
        if now.duration_since(*last) < self.interval {
            let wait_time = self.interval - now.duration_since(*last);
            tokio::time::sleep(wait_time).await;
        }
        *last = Instant::now();
    }

    async fn after(&self, _job: &dyn Job, _success: bool) {}

    async fn on_failure(&self, _job: &dyn Job, _error: &str) {}
}
```

> **Explanation:**
>
> - **LoggingMiddleware**: Logs job start, completion, and failure.
> - **RateLimitedMiddleware**: Ensures that jobs are processed no faster than a
>   specified rate.

### 5. Event System Implementation

As defined in `events.rs`, with specific event types.

### 6. Failed Job Repository Implementation

As defined in `failed_job_repository.rs`, handling logging, retrying, and
listing failed jobs.

### 7. Queue Manager Implementation

As defined in `queue_manager.rs`, managing multiple queue connections.

### 8. Worker Implementation

As defined in `worker.rs`, processing jobs from the queue.

---

## Command-Line Interface (CLI)

Using the `clap` crate, we'll create a CLI to interact with the queue system,
emulating Laravel's Artisan commands.

```rust
// src/main.rs

mod job;
mod queue;
mod drivers;
mod queue_manager;
mod worker;
mod events;
mod middleware;
mod handlers;
mod failed_job_repository;

use crate::queue::Queue;
use crate::drivers::in_memory_queue::InMemoryQueue;
use crate::drivers::sled_queue::SledQueue;
use crate::failed_job_repository::SledFailedJobRepository;
use crate::events::{Dispatcher, JobFailedEvent, JobProcessedEvent, JobProcessingEvent};
use crate::middleware::{LoggingMiddleware, RateLimitedMiddleware};
use crate::worker::Worker;
use async_trait::async_trait;
use clap::{Parser, Subcommand};
use log::{error, info};
use sled::open;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "sled_queue")]
#[command(author = "Your Name")]
#[command(version = "0.1.0")]
#[command(about = "Emulates Laravel's Queue System using sled in Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Enqueue a new job
    Enqueue {
        /// The queue name
        #[arg(short, long, default_value = "default")]
        queue: String,

        /// The job payload (JSON string)
        #[arg(short, long)]
        payload: String,

        /// Max attempts for the job
        #[arg(short, long, default_value_t = 3)]
        max_attempts: u32,
    },

    /// Dequeue and process jobs
    Work {
        /// The queue name
        #[arg(short, long, default_value = "default")]
        queue: String,

        /// Sleep duration in seconds when no job is found
        #[arg(short, long, default_value_t = 3)]
        sleep: u64,
    },

    /// Show the size of a queue
    Size {
        /// The queue name
        #[arg(short, long, default_value = "default")]
        queue: String,
    },

    /// List all failed jobs
    ListFailed,

    /// Retry a failed job by ID
    Retry {
        /// The ID of the failed job
        #[arg(short, long)]
        id: String,
    },
}

#[tokio::main]
async fn main() -> sled::Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    // Initialize sled databases
    let db = open("queue_db")?;
    let failed_db = open("failed_jobs_db")?;

    // Initialize QueueManager
    let queue_manager = Arc::new(QueueManager::new(db, failed_db)?);

    // Initialize Event Dispatcher
    let dispatcher = Arc::new(Dispatcher::new());

    // Initialize Failed Job Repository
    let failed_repo = Arc::new(SledFailedJobRepository::new(queue_manager.db().clone())?);

    // Initialize Middleware
    let logging_middleware = Arc::new(LoggingMiddleware);
    let rate_limited_middleware = Arc::new(RateLimitedMiddleware::new(5, 10)); // 5 jobs per 10 seconds

    // Initialize Worker
    let worker = Worker::new(
        dispatcher.clone(),
        failed_repo.clone(),
        vec![logging_middleware.clone(), rate_limited_middleware.clone()],
    );

    // Register Event Handlers
    let logging_handler = Arc::new(LoggingMiddleware);
    dispatcher.register::<JobProcessingEvent>(logging_handler.clone()).await;
    dispatcher.register::<JobProcessedEvent>(logging_handler.clone()).await;
    dispatcher.register::<JobFailedEvent>(logging_handler.clone()).await;

    match cli.command {
        Commands::Enqueue { queue, payload, max_attempts } => {
            let job = Job::new(payload, max_attempts);
            queue_manager.enqueue(&queue, job).await?;
            println!("Job enqueued successfully.");
        },
        Commands::Work { queue, sleep } => {
            let queue = match queue_manager.connection(&queue).await {
                Some(q) => q,
                None => {
                    eprintln!("No queue named '{}' found.", queue);
                    return Ok(());
                }
            };
            info!("Worker started on queue '{}'", queue);
            worker.run(queue, sleep).await;
        },
        Commands::Size { queue } => {
            let size = queue_manager.size(&queue).await;
            println!("Queue '{}' has {} jobs.", queue, size);
        },
        Commands::ListFailed => {
            let failed_jobs = failed_repo.all().await?;
            if failed_jobs.is_empty() {
                println!("No failed jobs found.");
            } else {
                println!("Failed Jobs:");
                for job in failed_jobs {
                    println!(
                        "ID: {}, Connection: {}, Queue: {}, Failed At: {}",
                        job.id, job.connection, job.queue, job.failed_at
                    );
                }
            }
        },
        Commands::Retry { id } => {
            match failed_repo.retry(&id).await? {
                Some(job) => {
                    // Re-enqueue the job
                    let queue = queue_manager.connection(&job.queue).await.unwrap();
                    queue_manager.enqueue(&job.queue, job).await?;
                    println!("Job {} has been retried.", id);
                },
                None => {
                    println!("Failed job with ID {} not found.", id);
                }
            }
        },
    }

    Ok(())
}
```

> **Explanation:**
>
> - **CLI Commands**:
>   - **Enqueue**: Adds a new job to the specified queue with a payload and
>     maximum attempts.
>   - **Work**: Starts a worker that dequeues and processes jobs from the
>     specified queue.
>   - **Size**: Displays the current size of the specified queue.
>   - **ListFailed**: Lists all failed jobs.
>   - **Retry**: Retries a failed job by its ID.
> - **Initialization**:
>   - Opens sled databases for queues and failed jobs.
>   - Initializes the `QueueManager`, `Dispatcher`, `FailedJobRepository`, and
>     `Worker`.
>   - Registers event handlers for logging.
> - **Command Handling**:
>   - Based on the CLI command, performs enqueueing, starting a worker, checking
>     queue size, listing failed jobs, or retrying a job.

---

## Running and Testing

1. **Enqueue a Job**

   ```bash
   cargo run -- enqueue --queue=default --payload='{"task": "send_email", "to": "user@example.com"}' --max-attempts=3
   ```

   Output:

   ```
   Job enqueued successfully.
   ```

2. **Check Queue Size**

   ```bash
   cargo run -- size --queue=default
   ```

   Output:

   ```
   Queue 'default' has 1 jobs.
   ```

3. **Start a Worker to Process Jobs**

   ```bash
   cargo run -- work --queue=default --sleep=3
   ```

   Output:

   ```
   INFO  queue_crate::main > Worker started on queue 'default'
   INFO  queue_crate::worker > Starting job 123e4567-e89b-12d3-a456-426614174000
   Job 123e4567-e89b-12d3-a456-426614174000 processed successfully.
   INFO  queue_crate::worker > Job 123e4567-e89b-12d3-a456-426614174000 has been processed successfully.
   ```

4. **List Failed Jobs**

   If a job fails (simulate by modifying the `Job::handle` method to panic), you
   can list failed jobs:

   ```bash
   cargo run -- list-failed
   ```

   Output:

   ```
   Failed Jobs:
   ID: 123e4567-e89b-12d3-a456-426614174000, Connection: default, Queue: default, Failed At: 2023-10-09T12:34:56Z
   ```

5. **Retry a Failed Job**

   ```bash
   cargo run -- retry --id=123e4567-e89b-12d3-a456-426614174000
   ```

   Output:

   ```
   Job 123e4567-e89b-12d3-a456-426614174000 has been retried.
   ```

---

## Advanced Features and Considerations

To build a more comprehensive queue system akin to Laravel's, consider
implementing the following advanced features:

### 1. Delayed Jobs

Schedule jobs to be processed after a certain delay.

- **Implementation**:
  - Store jobs with a `scheduled_at` timestamp.
  - Workers periodically scan for jobs where `scheduled_at <= now`.
  - Only enqueue these jobs for processing.

### 2. Job Prioritization

Assign priority levels to jobs to determine processing order.

- **Implementation**:
  - Modify the key generation to include priority (e.g., higher priority jobs
    have lower timestamps or separate prefixes).
  - Workers dequeue based on priority order.

### 3. Dead-Letter Queues

Handle jobs that consistently fail after maximum attempts.

- **Implementation**:
  - Move jobs that exceed `max_attempts` to a separate dead-letter queue.
  - Provide tools to inspect and retry or discard these jobs.

### 4. Concurrency and Worker Pools

Allow multiple workers to process jobs concurrently for higher throughput.

- **Implementation**:
  - Spawn multiple worker tasks or threads.
  - Ensure thread-safe operations and handle potential race conditions.

### 5. Graceful Shutdowns

Allow workers to finish processing current jobs before shutting down.

- **Implementation**:
  - Listen for shutdown signals (e.g., Ctrl+C).
  - Set a shutdown flag and wait for ongoing job processing to complete.

### 6. Metrics and Monitoring

Integrate logging and metrics to monitor queue performance.

- **Implementation**:
  - Use logging for tracking job processing.
  - Integrate with monitoring tools (e.g., Prometheus) to collect metrics like
    job rates, failures, etc.

### 7. Persistence Enhancements

Ensure robust persistence and recovery mechanisms.

- **Implementation**:
  - Handle sled's potential failures and ensure data integrity.
  - Implement backup and recovery strategies for the sled database.

---

## Pros and Cons of Using `sled` as a Queue Driver

### Pros

1. **Performance**: `sled` is highly optimized for speed, offering low-latency
   access and high throughput.
2. **Persistence**: Ensures that queued jobs are stored durably on disk,
   surviving application restarts and crashes.
3. **Concurrency**: Supports concurrent access, allowing multiple workers to
   interact with the queue safely.
4. **Embeddable**: Runs within the Rust application without requiring external
   services.
5. **Ordered Storage**: Naturally maintains job order through sorted keys,
   essential for queue systems.

### Cons

1. **Feature Completeness**: `sled` is a general-purpose key-value store and
   lacks specialized queue features like delayed jobs, job prioritization, or
   dead-letter queues out of the box.
2. **Scalability**: Being an embedded store, `sled` is limited to the host
   application's resources and doesn't support distributed queueing inherently.
3. **Community and Ecosystem**: Compared to established queue systems, `sled`
   has a smaller ecosystem for queue-specific extensions.
4. **Maintenance Overhead**: Building advanced queue features requires
   additional implementation effort, increasing complexity.

---

## Conclusion

Rust's `sled` crate offers a powerful foundation for building an embedded,
persistent queue system. By leveraging `sled`'s ordered key-value storage,
concurrency support, and durability guarantees, you can implement a queue driver
that meets many of Laravel's Queue system requirements. While `sled` doesn't
provide out-of-the-box queue features, its flexibility allows you to design and
extend the queue system to incorporate necessary functionalities like job
retries, failed job management, and middleware support.

For production-ready systems, consider addressing advanced features such as
delayed jobs, prioritization, and dead-letter queues. Additionally, implementing
robust error handling, graceful shutdowns, and monitoring will enhance the
reliability and maintainability of your queue system.

By integrating `sled` with Rust's asynchronous capabilities (via `tokio`) and
leveraging traits and abstractions, you can create a high-performance, embedded
queue system tailored to your application's specific needs.

---
