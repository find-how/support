use super::MockQueue;
use proptest::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub enum QueueOp {
    Push(Vec<u8>),
    Pop,
    Clear,
}

#[derive(Debug, Clone)]
pub struct QueueState {
    items: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl QueueState {
    pub fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn apply(&mut self, op: QueueOp) -> bool {
        match op {
            QueueOp::Push(data) => {
                let id = uuid::Uuid::new_v4().to_string();
                self.items.lock().await.insert(id, data);
                true
            }
            QueueOp::Pop => {
                if let Some((id, _)) = self.items.lock().await.iter().next() {
                    self.items.lock().await.remove(&id.clone());
                    true
                } else {
                    false
                }
            }
            QueueOp::Clear => {
                self.items.lock().await.clear();
                true
            }
        }
    }
}

pub fn test_queue_strategy() -> impl Strategy<Value = Vec<QueueOp>> {
    prop::collection::vec(
        prop_oneof![
            prop::collection::vec(any::<u8>(), 0..100).prop_map(QueueOp::Push),
            Just(QueueOp::Pop),
            Just(QueueOp::Clear),
        ],
        0..100,
    )
}

pub async fn test_queue_operations(ops: Vec<QueueOp>) -> bool {
    let queue = MockQueue::new();
    let mut state = QueueState::new();

    for op in ops {
        match op.clone() {
            QueueOp::Push(data) => {
                let queue_result = queue.push(data.clone()).await.is_ok();
                let state_result = state.apply(op).await;
                if queue_result != state_result {
                    return false;
                }
            }
            QueueOp::Pop => {
                let queue_result = queue.pop().await.unwrap().is_some();
                let state_result = state.apply(op).await;
                if queue_result != state_result {
                    return false;
                }
            }
            QueueOp::Clear => {
                let queue_result = queue.clear().await.is_ok();
                let state_result = state.apply(op).await;
                if !queue_result || !state_result {
                    return false;
                }
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_queue_operations_prop(ops in test_queue_strategy()) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                assert!(test_queue_operations(ops).await);
            });
        }
    }
}
