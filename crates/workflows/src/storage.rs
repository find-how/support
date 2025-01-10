use std::sync::Arc;
use tokio::sync::Mutex;
use store::{Store, backends::SledStore};
use crate::{WorkflowState, WorkflowEvent, WorkflowError, Result, WorkflowSnapshot};
use uuid::Uuid;
use serde_json;

pub struct StoreWorkflowStorage {
    store: Arc<SledStore>,
    lock: Arc<Mutex<()>>, // Simple lock for transactional operations
}

impl StoreWorkflowStorage {
    pub fn new(store: SledStore) -> Self {
        Self {
            store: Arc::new(store),
            lock: Arc::new(Mutex::new(())),
        }
    }

    fn workflow_key(workflow_id: &Uuid) -> String {
        format!("workflow:{}", workflow_id)
    }

    fn events_key(workflow_id: &Uuid) -> String {
        format!("events:{}", workflow_id)
    }

    fn snapshots_key(workflow_id: &Uuid) -> String {
        format!("snapshots:{}", workflow_id)
    }
}

#[async_trait::async_trait]
impl crate::WorkflowStorage for StoreWorkflowStorage {
    async fn create_workflow(&self, workflow: WorkflowState) -> Result<()> {
        let _guard = self.lock.lock().await;
        let key = Self::workflow_key(&workflow.id);
        let json = serde_json::to_string(&workflow)
            .map_err(|e| WorkflowError::SerializationError(e.to_string()))?;
        self.store.set(&key, json.as_bytes())
            .map_err(|e| WorkflowError::PersistenceError(e.to_string()))?;
        Ok(())
    }

    async fn update_workflow(&self, workflow: &WorkflowState) -> Result<()> {
        let _guard = self.lock.lock().await;
        let key = Self::workflow_key(&workflow.id);
        let json = serde_json::to_string(&workflow)
            .map_err(|e| WorkflowError::SerializationError(e.to_string()))?;
        self.store.set(&key, json.as_bytes())
            .map_err(|e| WorkflowError::PersistenceError(e.to_string()))?;
        Ok(())
    }

    async fn load_workflow(&self, workflow_id: &Uuid) -> Result<Option<WorkflowState>> {
        let key = Self::workflow_key(workflow_id);
        if let Some(bytes) = self.store.get(&key)
            .map_err(|e| WorkflowError::PersistenceError(e.to_string()))? {
            let json = String::from_utf8_lossy(&bytes);
            let workflow = serde_json::from_str(&json)
                .map_err(|e| WorkflowError::SerializationError(e.to_string()))?;
            Ok(Some(workflow))
        } else {
            Ok(None)
        }
    }

    async fn list_workflows(&self) -> Result<Vec<WorkflowState>> {
        let mut workflows = Vec::new();
        // TODO: Use store's range/scan functionality when implemented
        // For now, we'll need to iterate through all keys
        Ok(workflows)
    }

    async fn append_event(&self, workflow_id: &Uuid, event: WorkflowEvent) -> Result<()> {
        let _guard = self.lock.lock().await;
        let key = Self::events_key(workflow_id);
        let mut events = if let Some(bytes) = self.store.get(&key)
            .map_err(|e| WorkflowError::PersistenceError(e.to_string()))? {
            let json = String::from_utf8_lossy(&bytes);
            serde_json::from_str(&json)
                .map_err(|e| WorkflowError::SerializationError(e.to_string()))?
        } else {
            Vec::new()
        };
        events.push(event);
        let json = serde_json::to_string(&events)
            .map_err(|e| WorkflowError::SerializationError(e.to_string()))?;
        self.store.set(&key, json.as_bytes())
            .map_err(|e| WorkflowError::PersistenceError(e.to_string()))?;
        Ok(())
    }

    async fn take_snapshot(&self, workflow: &WorkflowState) -> Result<()> {
        let _guard = self.lock.lock().await;
        let key = Self::snapshots_key(&workflow.id);
        let mut snapshots = if let Some(bytes) = self.store.get(&key)
            .map_err(|e| WorkflowError::PersistenceError(e.to_string()))? {
            let json = String::from_utf8_lossy(&bytes);
            serde_json::from_str(&json)
                .map_err(|e| WorkflowError::SerializationError(e.to_string()))?
        } else {
            Vec::new()
        };
        let snapshot = WorkflowSnapshot {
            version: workflow.version,
            state: workflow.clone(),
            created_at: chrono::Utc::now(),
        };
        snapshots.push(snapshot);
        let json = serde_json::to_string(&snapshots)
            .map_err(|e| WorkflowError::SerializationError(e.to_string()))?;
        self.store.set(&key, json.as_bytes())
            .map_err(|e| WorkflowError::PersistenceError(e.to_string()))?;
        Ok(())
    }

    async fn delete_workflow(&self, workflow_id: &Uuid) -> Result<()> {
        let _guard = self.lock.lock().await;
        let workflow_key = Self::workflow_key(workflow_id);
        let events_key = Self::events_key(workflow_id);
        let snapshots_key = Self::snapshots_key(workflow_id);

        self.store.delete(&workflow_key)
            .map_err(|e| WorkflowError::PersistenceError(e.to_string()))?;
        self.store.delete(&events_key)
            .map_err(|e| WorkflowError::PersistenceError(e.to_string()))?;
        self.store.delete(&snapshots_key)
            .map_err(|e| WorkflowError::PersistenceError(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::collections::HashMap;
    use crate::{WorkflowStep, WorkflowStatus, WorkflowPriority};

    #[tokio::test]
    async fn test_workflow_storage() -> Result<()> {
        let dir = tempdir()?;
        let store = SledStore::new(dir.path())
            .map_err(|e| WorkflowError::PersistenceError(e.to_string()))?;
        let storage = StoreWorkflowStorage::new(store);

        // Create a test workflow
        let workflow = WorkflowState::new(
            vec![],
            HashMap::new(),
            1,
            None,
            vec!["test".to_string()],
            WorkflowPriority::Normal,
        );
        let workflow_id = workflow.id;

        // Test create and load
        storage.create_workflow(workflow.clone()).await?;
        let loaded = storage.load_workflow(&workflow_id).await?.unwrap();
        assert_eq!(loaded.id, workflow_id);

        // Test update
        let mut updated = workflow.clone();
        updated.status = WorkflowStatus::Running;
        storage.update_workflow(&updated).await?;
        let loaded = storage.load_workflow(&workflow_id).await?.unwrap();
        assert_eq!(loaded.status, WorkflowStatus::Running);

        // Test events
        let event = WorkflowEvent::WorkflowStarted(chrono::Utc::now());
        storage.append_event(&workflow_id, event).await?;

        // Test snapshots
        storage.take_snapshot(&workflow).await?;

        // Test delete
        storage.delete_workflow(&workflow_id).await?;
        assert!(storage.load_workflow(&workflow_id).await?.is_none());

        Ok(())
    }
}
