use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub mod bench;
pub mod property;

pub use bench::{bench_queue, QueueBenchConfig, QueueBenchResults};
pub use property::{test_queue_operations, test_queue_strategy, QueueOp, QueueState};

#[async_trait]
pub trait TestQueue: Send + Sync + Clone {
    fn new_test_queue() -> Self where Self: Sized;
    async fn push(&self, payload: Bytes) -> bool;
    async fn pop(&self) -> Option<(String, Bytes)>;
    async fn size(&self) -> usize;
    async fn complete(&self, id: &str) -> bool;
    async fn fail(&self, id: &str) -> bool;
    async fn get_failed(&self) -> Vec<(String, Bytes)>;
    async fn retry(&self, id: &str) -> bool;
}

#[derive(Clone)]
pub struct MockQueue {
    jobs: Arc<Mutex<Vec<(String, Bytes)>>>,
    failed: Arc<Mutex<HashMap<String, Bytes>>>,
}

impl MockQueue {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(Vec::new())),
            failed: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl TestQueue for MockQueue {
    fn new_test_queue() -> Self {
        Self::new()
    }

    async fn push(&self, payload: Bytes) -> bool {
        let id = uuid::Uuid::new_v4().to_string();
        self.jobs.lock().unwrap().push((id, payload));
        true
    }

    async fn pop(&self) -> Option<(String, Bytes)> {
        self.jobs.lock().unwrap().pop()
    }

    async fn size(&self) -> usize {
        self.jobs.lock().unwrap().len()
    }

    async fn complete(&self, _id: &str) -> bool {
        true
    }

    async fn fail(&self, id: &str) -> bool {
        if let Some(pos) = self.jobs.lock().unwrap().iter().position(|(jid, _)| jid == id) {
            let (_, payload) = self.jobs.lock().unwrap().remove(pos);
            self.failed.lock().unwrap().insert(id.to_string(), payload);
            true
        } else {
            false
        }
    }

    async fn get_failed(&self) -> Vec<(String, Bytes)> {
        self.failed.lock().unwrap()
            .iter()
            .map(|(id, payload)| (id.clone(), payload.clone()))
            .collect()
    }

    async fn retry(&self, id: &str) -> bool {
        if let Some(payload) = self.failed.lock().unwrap().remove(id) {
            self.jobs.lock().unwrap().push((id.to_string(), payload));
            true
        } else {
            false
        }
    }
}
