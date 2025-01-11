pub mod queue;
pub use queue::Queue;

#[cfg(test)]
mod tests {
    use super::*;
    use queue::MockQueue;

    #[tokio::test]
    async fn test_queue() {
        let queue = MockQueue::new();
        assert!(queue.is_empty().await.unwrap());

        queue.push(vec![1, 2, 3]).await.unwrap();
        assert_eq!(queue.len().await.unwrap(), 1);

        let item = queue.pop().await.unwrap().unwrap();
        assert_eq!(item, vec![1, 2, 3]);
        assert!(queue.is_empty().await.unwrap());
    }
}
