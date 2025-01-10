use crate::queue::{TestQueue, MockQueue};
use proptest::prelude::*;

#[derive(Debug, Clone)]
pub enum QueueOp {
    Push(Vec<u8>),
    Pop,
    Complete(String),
    Fail(String),
    Retry(String),
}

impl Arbitrary for QueueOp {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        prop_oneof![
            any::<Vec<u8>>().prop_map(QueueOp::Push),
            Just(QueueOp::Pop),
            any::<String>().prop_map(QueueOp::Complete),
            any::<String>().prop_map(QueueOp::Fail),
            any::<String>().prop_map(QueueOp::Retry)
        ].boxed()
    }
}

#[derive(Debug, Clone)]
pub struct QueueState {
    pub size: usize,
    pub failed: Vec<(String, Vec<u8>)>,
}

pub async fn test_queue_operations<Q: TestQueue>(queue: &Q, ops: Vec<QueueOp>) -> QueueState {
    let mut state = QueueState {
        size: 0,
        failed: Vec::new(),
    };

    for op in ops {
        match op {
            QueueOp::Push(payload) => {
                queue.push(payload.into()).await;
                state.size = queue.size().await;
            }
            QueueOp::Pop => {
                if queue.pop().await.is_some() {
                    state.size = queue.size().await;
                }
            }
            QueueOp::Complete(id) => {
                if queue.complete(&id).await {
                    state.size = queue.size().await;
                }
            }
            QueueOp::Fail(id) => {
                if queue.fail(&id).await {
                    state.size = queue.size().await;
                    if let Some(job) = queue.pop().await {
                        state.failed.push((job.0, job.1.to_vec()));
                        state.size = queue.size().await;
                    }
                }
            }
            QueueOp::Retry(id) => {
                if queue.retry(&id).await {
                    state.size = queue.size().await;
                }
            }
        }
    }

    state
}

pub fn test_queue_strategy() -> impl Strategy<Value = Vec<QueueOp>> {
    prop::collection::vec(any::<QueueOp>(), 0..100)
}

proptest! {
    #[test]
    fn test_queue_properties(ops in test_queue_strategy()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let queue = MockQueue::new_test_queue();
            let state = test_queue_operations(&queue, ops).await;

            // Verify queue state
            assert_eq!(queue.size().await, state.size);
            let failed = queue.get_failed().await;
            assert_eq!(failed.len(), state.failed.len());

            // Verify each failed job matches
            for (id, payload) in state.failed {
                let found = failed.iter().any(|(fid, fpayload)| {
                    *fid == id && fpayload.to_vec() == payload
                });
                assert!(found, "Failed job not found in queue");
            }
        });
    }
}
