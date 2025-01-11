use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

#[async_trait]
pub trait Queue: Send + Sync {
    async fn push(&self, data: Vec<u8>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn pop(&self) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>>;
    async fn len(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>>;
    async fn is_empty(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;
    async fn clear(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

#[derive(Clone)]
pub struct MockQueue {
    items: Arc<Mutex<VecDeque<Vec<u8>>>>,
}

impl MockQueue {
    pub fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn new_test_queue() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Queue for MockQueue {
    async fn push(&self, data: Vec<u8>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.items.lock().await.push_back(data);
        Ok(())
    }

    async fn pop(&self) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.items.lock().await.pop_front())
    }

    async fn len(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.items.lock().await.len())
    }

    async fn is_empty(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.items.lock().await.is_empty())
    }

    async fn clear(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.items.lock().await.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_queue() {
        let queue = MockQueue::new();
        assert!(queue.is_empty().await.unwrap());

        queue.push(vec![1, 2, 3]).await.unwrap();
        assert_eq!(queue.len().await.unwrap(), 1);

        let item = queue.pop().await.unwrap().unwrap();
        assert_eq!(item, vec![1, 2, 3]);
        assert!(queue.is_empty().await.unwrap());
    }
}
