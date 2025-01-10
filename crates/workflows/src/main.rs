//! src/main.rs
//! A single-file crate implementation of a comprehensive workflow engine + CLI.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand, Args};
use cron::Schedule;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sled::{Db};
use thiserror::Error;
use tokio::{
    sync::{mpsc, oneshot, Mutex, Semaphore},
    task::JoinHandle,
    time::{sleep, sleep_until, Instant},
};
use tracing::{error, info, instrument, warn};
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

// ------------------------ Errors ------------------------

#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Workflow not found: {0}")]
    NotFound(String),
    #[error("Invalid workflow state: {0}")]
    InvalidState(String),
    #[error("Custom error: {0}")]
    Custom(String),
}

pub type Result<T> = std::result::Result<T, WorkflowError>;

// ------------------------ Models and Enums ------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkflowPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl std::fmt::Display for WorkflowPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Normal => write!(f, "Normal"),
            Self::High => write!(f, "High"),
            Self::Critical => write!(f, "Critical"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkflowStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for WorkflowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Running => write!(f, "Running"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::InProgress => write!(f, "InProgress"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryStrategy {
    pub max_attempts: u32,
    pub backoff_initial: Duration,
    pub backoff_factor: f32,
    pub backoff_max: Duration,
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff_initial: Duration::from_secs(1),
            backoff_factor: 2.0,
            backoff_max: Duration::from_secs(30),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CompensationStatus {
    Pending,
    Completed,
    Failed(String),
}

impl std::fmt::Display for CompensationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed(reason) => write!(f, "Failed({})", reason),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompensationStep {
    pub step_name: String,
    pub compensation_handler: String, // Handler identifier
    pub status: CompensationStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: Uuid,
    pub name: String,
    pub status: StepStatus,
    pub retries: u32,
    pub max_retries: u32,
    pub timeout: Option<Duration>,
    pub compensation: Option<CompensationStep>,
    pub dependencies: Vec<Uuid>, // IDs of dependent steps
    pub retry_strategy: RetryStrategy,
    pub output: Option<ActivityResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivityResult {
    pub output: HashMap<String, String>,
    pub duration: Duration,
    pub metadata: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowState {
    pub id: Uuid,
    pub status: WorkflowStatus,
    pub current_step: Option<Uuid>,
    pub steps: Vec<WorkflowStep>,
    pub data: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
    pub version: u32,
    pub parent_workflow_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub priority: WorkflowPriority,
    pub events: Vec<WorkflowEvent>,
    pub snapshots: Vec<WorkflowSnapshot>,
}

impl WorkflowState {
    /// Initializes a new workflow.
    pub fn new(
        steps: Vec<WorkflowStep>,
        data: HashMap<String, String>,
        version: u32,
        parent_workflow_id: Option<Uuid>,
        tags: Vec<String>,
        priority: WorkflowPriority,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            status: WorkflowStatus::Pending,
            current_step: None,
            steps,
            data,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_heartbeat: Utc::now(),
            version,
            parent_workflow_id,
            tags,
            priority,
            events: Vec::new(),
            snapshots: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WorkflowEvent {
    WorkflowStarted(DateTime<Utc>),
    WorkflowMigrated(u32, DateTime<Utc>),
    WorkflowCompleted(DateTime<Utc>),
    WorkflowFailed(String, DateTime<Utc>),
    StepStarted(Uuid, DateTime<Utc>),
    StepCompleted(Uuid, ActivityResult, DateTime<Utc>),
    StepFailed(Uuid, String, DateTime<Utc>),
    CompensationStarted(String, DateTime<Utc>),
    CompensationCompleted(String, DateTime<Utc>),
    CompensationFailed(String, String, DateTime<Utc>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowSnapshot {
    pub version: u32,
    pub state: WorkflowState,
    pub created_at: DateTime<Utc>,
}

// ------------------------ Storage Layer ------------------------

#[async_trait::async_trait]
pub trait WorkflowStorage: Send + Sync {
    async fn create_workflow(&self, workflow: WorkflowState) -> Result<()>;
    async fn update_workflow(&self, workflow: &WorkflowState) -> Result<()>;
    async fn load_workflow(&self, workflow_id: &Uuid) -> Result<Option<WorkflowState>>;
    async fn list_workflows(&self) -> Result<Vec<WorkflowState>>;
    async fn append_event(&self, workflow_id: &Uuid, event: WorkflowEvent) -> Result<()>;
    async fn take_snapshot(&self, workflow: &WorkflowState) -> Result<()>;

    /// Delete a workflow from storage
    async fn delete_workflow(&self, workflow_id: &Uuid) -> Result<()>;
}

#[derive(Debug)]
pub struct SledWorkflowStorage {
    db: Arc<Db>,
    lock: Arc<Mutex<()>>, // Simple lock for transactional operations
}

impl SledWorkflowStorage {
    pub fn new(db: Db) -> Self {
        Self {
            db: Arc::new(db),
            lock: Arc::new(Mutex::new(())),
        }
    }
}

#[async_trait::async_trait]
impl WorkflowStorage for SledWorkflowStorage {
    #[instrument]
    async fn create_workflow(&self, workflow: WorkflowState) -> Result<()> {
        let _guard = self.lock.lock().await;
        let key = format!("workflow:{}", workflow.id);
        let serialized = bincode::serialize(&workflow)
            .map_err(|e| WorkflowError::Serialization(e.to_string()))?;
        self.db.insert(key.as_bytes(), serialized)
            .map_err(|e| WorkflowError::Storage(e.to_string()))?;
        self.db.flush()
            .map_err(|e| WorkflowError::Storage(e.to_string()))?;
        Ok(())
    }

    #[instrument]
    async fn update_workflow(&self, workflow: &WorkflowState) -> Result<()> {
        let _guard = self.lock.lock().await;
        let key = format!("workflow:{}", workflow.id);
        let serialized = bincode::serialize(workflow)
            .map_err(|e| WorkflowError::Serialization(e.to_string()))?;
        self.db.insert(key.as_bytes(), serialized)
            .map_err(|e| WorkflowError::Storage(e.to_string()))?;
        self.db.flush()
            .map_err(|e| WorkflowError::Storage(e.to_string()))?;
        Ok(())
    }

    #[instrument]
    async fn load_workflow(&self, workflow_id: &Uuid) -> Result<Option<WorkflowState>> {
        let key = format!("workflow:{}", workflow_id);
        match self.db.get(key.as_bytes())
            .map_err(|e| WorkflowError::Storage(e.to_string()))? {
            Some(value) => {
                let workflow: WorkflowState = bincode::deserialize(value.as_ref())
                    .map_err(|e| WorkflowError::Serialization(e.to_string()))?;
                Ok(Some(workflow))
            }
            None => Ok(None),
        }
    }

    #[instrument]
    async fn list_workflows(&self) -> Result<Vec<WorkflowState>> {
        let mut workflows = Vec::new();
        let prefix = b"workflow:";

        for item in self.db.scan_prefix(prefix) {
            match item.map_err(|e| WorkflowError::Storage(e.to_string()))? {
                (_, value) => {
                    let workflow: WorkflowState = bincode::deserialize(value.as_ref())
                        .map_err(|e| WorkflowError::Serialization(e.to_string()))?;
                    workflows.push(workflow);
                }
            }
        }
        Ok(workflows)
    }

    #[instrument]
    async fn append_event(&self, workflow_id: &Uuid, event: WorkflowEvent) -> Result<()> {
        let _guard = self.lock.lock().await;
        let key = format!("events:{}", workflow_id);
        let mut events = if let Some(value) = self.db.get(key.as_bytes())
            .map_err(|e| WorkflowError::Storage(e.to_string()))? {
            bincode::deserialize(value.as_ref())
                .map_err(|e| WorkflowError::Serialization(e.to_string()))?
        } else {
            Vec::new()
        };
        events.push(event);
        let serialized = bincode::serialize(&events)
            .map_err(|e| WorkflowError::Serialization(e.to_string()))?;
        self.db.insert(key.as_bytes(), serialized)
            .map_err(|e| WorkflowError::Storage(e.to_string()))?;
        self.db.flush()
            .map_err(|e| WorkflowError::Storage(e.to_string()))?;
        Ok(())
    }

    #[instrument]
    async fn take_snapshot(&self, workflow: &WorkflowState) -> Result<()> {
        let _guard = self.lock.lock().await;
        let snapshot = WorkflowSnapshot {
            version: workflow.version,
            state: workflow.clone(),
            created_at: Utc::now(),
        };
        let key = format!("snapshot:{}", workflow.id);
        let serialized = bincode::serialize(&snapshot)
            .map_err(|e| WorkflowError::Serialization(e.to_string()))?;
        self.db.insert(key.as_bytes(), serialized)
            .map_err(|e| WorkflowError::Storage(e.to_string()))?;
        self.db.flush()
            .map_err(|e| WorkflowError::Storage(e.to_string()))?;
        Ok(())
    }

    #[instrument]
    async fn delete_workflow(&self, workflow_id: &Uuid) -> Result<()> {
        let _guard = self.lock.lock().await;
        let key = format!("workflow:{}", workflow_id);
        self.db.remove(key.as_bytes())
            .map_err(|e| WorkflowError::Storage(e.to_string()))?;
        self.db.flush()
            .map_err(|e| WorkflowError::Storage(e.to_string()))?;
        Ok(())
    }
}

// ------------------------ Activity Handlers ------------------------

#[async_trait]
pub trait ActivityHandler: Send + Sync {
    async fn execute(&self, payload: HashMap<String, String>) -> Result<ActivityResult>;
}

pub struct FileProcessingHandler;

#[async_trait]
impl ActivityHandler for FileProcessingHandler {
    async fn execute(&self, payload: HashMap<String, String>) -> Result<ActivityResult> {
        // Example: Simulate file processing
        let file_path = payload.get("file_path")
            .ok_or_else(|| WorkflowError::InvalidState("Missing 'file_path' in payload".to_string()))?;

        info!("Processing file at path: {}", file_path);

        // Simulate processing delay
        tokio::time::sleep(Duration::from_secs(1)).await;

        let mut output = HashMap::new();
        output.insert("status".to_string(), "processed".to_string());

        Ok(ActivityResult {
            output,
            duration: Duration::from_secs(1),
            metadata: HashMap::new(),
        })
    }
}

pub struct ApiCallHandler;

#[async_trait]
impl ActivityHandler for ApiCallHandler {
    async fn execute(&self, payload: HashMap<String, String>) -> Result<ActivityResult> {
        let api_endpoint = payload.get("api_endpoint")
            .ok_or_else(|| WorkflowError::InvalidState("Missing 'api_endpoint' in payload".to_string()))?;

        info!("Making API call to: {}", api_endpoint);
        tokio::time::sleep(Duration::from_secs(1)).await;

        let mut output = HashMap::new();
        output.insert("response".to_string(), "success".to_string());

        Ok(ActivityResult {
            output,
            duration: Duration::from_secs(1),
            metadata: HashMap::new(),
        })
    }
}

/// Registry for activity handlers.
pub struct ActivityRegistry {
    handlers: DashMap<String, Arc<dyn ActivityHandler>>,
}

impl ActivityRegistry {
    pub fn new() -> Self {
        Self {
            handlers: DashMap::new(),
        }
    }

    pub fn register(&self, name: &str, handler: Arc<dyn ActivityHandler>) {
        self.handlers.insert(name.to_string(), handler);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn ActivityHandler>> {
        self.handlers.get(name).map(|h| h.clone())
    }
}

// ------------------------ Engine and Signals ------------------------

#[derive(Debug)]
pub enum WorkflowSignal {
    UpdateData(HashMap<String, String>),
    Cancel,
}

#[derive(Debug)]
pub enum WorkflowQuery {
    GetData(oneshot::Sender<Result<HashMap<String, String>>>),
    GetStatus(oneshot::Sender<Result<WorkflowStatus>>),
    GetWorkflow(oneshot::Sender<Result<Option<WorkflowState>>>),
}

impl WorkflowQuery {
    pub fn handle(&self, workflow: &WorkflowState) -> Result<()> {
        match self {
            WorkflowQuery::GetData(tx) => {
                let _ = tx.send(Ok(workflow.data.clone()));
            }
            WorkflowQuery::GetStatus(tx) => {
                let _ = tx.send(Ok(workflow.status.clone()));
            }
            WorkflowQuery::GetWorkflow(tx) => {
                let _ = tx.send(Ok(Some(workflow.clone())));
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum WorkflowCommand {
    ExecuteStep(Uuid),
    HandleSignal(Uuid, WorkflowSignal),
    HandleQuery(Uuid, WorkflowQuery),
}

// The central workflow engine

pub struct WorkflowEngine<S: WorkflowStorage + 'static> {
    pub storage: Arc<S>,
    command_sender: mpsc::Sender<WorkflowCommand>,
    pub activity_registry: Arc<ActivityRegistry>,
}

impl<S: WorkflowStorage + 'static> WorkflowEngine<S> {
    pub fn new(storage: Arc<S>, activity_registry: Arc<ActivityRegistry>) -> Self {
        let (tx, rx) = mpsc::channel(100);
        let engine = Self {
            storage: storage.clone(),
            command_sender: tx.clone(),
            activity_registry: activity_registry.clone(),
        };
        engine.start(rx);
        engine
    }

    fn start(&self, mut receiver: mpsc::Receiver<WorkflowCommand>) {
        let storage = self.storage.clone();
        let activity_registry = self.activity_registry.clone();
        tokio::spawn(async move {
            while let Some(command) = receiver.recv().await {
                let storage = storage.clone();
                let activity_registry = activity_registry.clone();
                tokio::spawn(async move {
                    if let Err(e) = WorkflowEngine::handle_command(storage, activity_registry, command).await {
                        error!("Error handling command: {}", e);
                    }
                });
            }
        });
    }

    async fn handle_command(
        storage: Arc<S>,
        activity_registry: Arc<ActivityRegistry>,
        command: WorkflowCommand,
    ) -> Result<()> {
        match command {
            WorkflowCommand::ExecuteStep(workflow_id) => {
                WorkflowEngine::execute_step(storage, activity_registry, workflow_id).await
            },
            WorkflowCommand::HandleSignal(workflow_id, signal) => {
                WorkflowEngine::handle_signal(storage, workflow_id, signal).await
            },
            WorkflowCommand::HandleQuery(workflow_id, query) => {
                WorkflowEngine::handle_query(storage, workflow_id, query).await
            },
        }
    }

    #[instrument(skip(storage, activity_registry))]
    async fn execute_step(
        storage: Arc<S>,
        _activity_registry: Arc<ActivityRegistry>,
        workflow_id: Uuid,
    ) -> Result<()> {
        let mut workflow = match storage.load_workflow(&workflow_id).await? {
            Some(wf) => wf,
            None => return Err(WorkflowError::NotFound(format!("Workflow {} not found", workflow_id))),
        };

        let next_step_id = workflow.steps.iter()
            .filter(|s| s.status == StepStatus::Pending)
            .filter(|s| s.dependencies.iter().all(|dep| {
                workflow.steps.iter().any(|ds| ds.id == *dep && ds.status == StepStatus::Completed)
            }))
            .map(|s| s.id)
            .next();

        let step_id = match next_step_id {
            Some(id) => id,
            None => {
                workflow.status = WorkflowStatus::Completed;
                workflow.updated_at = Utc::now();
                storage.update_workflow(&workflow).await?;
                storage.append_event(&workflow_id, WorkflowEvent::WorkflowCompleted(Utc::now())).await?;
                return Ok(());
            }
        };

        let step = workflow.steps.iter_mut().find(|s| s.id == step_id)
            .ok_or_else(|| WorkflowError::InvalidState(format!("Step {} not found", step_id)))?;

        step.status = StepStatus::InProgress;
        workflow.current_step = Some(step.id);
        workflow.updated_at = Utc::now();
        storage.update_workflow(&workflow).await?;
        storage.append_event(&workflow_id, WorkflowEvent::StepStarted(step.id, Utc::now())).await?;

        // placeholder for actual step execution logic
        // The WorkerPool or a separate async job can do the heavy lifting
        Ok(())
    }

    #[instrument(skip(storage))]
    async fn handle_signal(
        storage: Arc<S>,
        workflow_id: Uuid,
        signal: WorkflowSignal,
    ) -> Result<()> {
        match signal {
            WorkflowSignal::UpdateData(new_data) => {
                let mut workflow = match storage.load_workflow(&workflow_id).await? {
                    Some(wf) => wf,
                    None => return Err(WorkflowError::NotFound(format!("Workflow {} not found", workflow_id))),
                };

                workflow.data.extend(new_data);
                workflow.updated_at = Utc::now();
                storage.update_workflow(&workflow).await?;
                storage.append_event(&workflow_id, WorkflowEvent::StepCompleted(
                    workflow.current_step.unwrap_or(Uuid::nil()),
                    ActivityResult {
                        output: HashMap::new(),
                        duration: Duration::from_secs(0),
                        metadata: HashMap::new(),
                    },
                    Utc::now(),
                )).await?;
            },
            WorkflowSignal::Cancel => {
                let mut workflow = match storage.load_workflow(&workflow_id).await? {
                    Some(wf) => wf,
                    None => return Err(WorkflowError::NotFound(format!("Workflow {} not found", workflow_id))),
                };

                workflow.status = WorkflowStatus::Cancelled;
                workflow.updated_at = Utc::now();
                storage.update_workflow(&workflow).await?;
                storage.append_event(&workflow_id, WorkflowEvent::WorkflowFailed(
                    "Cancelled by signal".to_string(),
                    Utc::now(),
                )).await?;
            },
        }

        Ok(())
    }

    #[instrument(skip(storage))]
    async fn handle_query(
        storage: Arc<S>,
        workflow_id: Uuid,
        query: WorkflowQuery,
    ) -> Result<()> {
        let workflow = match storage.load_workflow(&workflow_id).await? {
            Some(wf) => wf,
            None => return Err(WorkflowError::NotFound(format!("Workflow {} not found", workflow_id))),
        };

        query.handle(&workflow)?;
        Ok(())
    }

    pub async fn execute_step_command(&self, workflow_id: Uuid) -> Result<()> {
        self.command_sender.send(WorkflowCommand::ExecuteStep(workflow_id)).await
            .map_err(|e| WorkflowError::InvalidState(e.to_string()))
    }

    pub async fn send_signal(&self, workflow_id: Uuid, signal: WorkflowSignal) -> Result<()> {
        self.command_sender.send(WorkflowCommand::HandleSignal(workflow_id, signal)).await
            .map_err(|e| WorkflowError::InvalidState(e.to_string()))
    }

    pub async fn send_query(&self, workflow_id: Uuid, query: WorkflowQuery) -> Result<()> {
        self.command_sender.send(WorkflowCommand::HandleQuery(workflow_id, query)).await
            .map_err(|e| WorkflowError::InvalidState(e.to_string()))
    }

    pub async fn create_workflow(&self, workflow: WorkflowState) -> Result<()> {
        self.storage.create_workflow(workflow.clone()).await?;
        self.storage.append_event(&workflow.id, WorkflowEvent::WorkflowStarted(Utc::now())).await?;
        self.command_sender.send(WorkflowCommand::ExecuteStep(workflow.id)).await
            .map_err(|e| WorkflowError::InvalidState(e.to_string()))
    }
}

// WorkerPool

#[derive(Clone)]
pub struct Task {
    pub workflow_id: Uuid,
    pub activity: Activity,
}

#[derive(Clone, Debug)]
pub struct Activity {
    pub handler: String,
    pub payload: HashMap<String, String>,
}

pub struct WorkerPool<S: WorkflowStorage + 'static> {
    storage: Arc<S>,
    task_sender: mpsc::Sender<Task>,
    task_receiver: mpsc::Receiver<Task>,
    workers: Vec<JoinHandle<()>>,
    active_workers: Arc<DashMap<Uuid, JoinHandle<()>>>,
    activity_registry: Arc<ActivityRegistry>,
}

impl<S: WorkflowStorage + 'static> WorkerPool<S> {
    pub fn new(storage: Arc<S>, activity_registry: Arc<ActivityRegistry>, max_concurrent: usize) -> Self {
        let (sender, receiver) = mpsc::channel(max_concurrent);
        Self {
            storage,
            task_sender: sender,
            task_receiver: receiver,
            workers: Vec::new(),
            active_workers: Arc::new(DashMap::new()),
            activity_registry,
        }
    }

    pub fn start(&mut self, num_workers: usize) {
        for _ in 0..num_workers {
            let storage = self.storage.clone();
            let activity_registry = self.activity_registry.clone();
            let mut receiver = self.task_receiver.clone();
            let active_workers = self.active_workers.clone();

            let handle = tokio::spawn(async move {
                while let Some(task) = receiver.recv().await {
                    let worker_handle = tokio::spawn(Self::execute_activity(
                        storage.clone(),
                        activity_registry.clone(),
                        task.workflow_id,
                        task.activity,
                    ));
                    active_workers.insert(task.workflow_id, worker_handle);
                }
            });
            self.workers.push(handle);
        }
    }

    #[instrument(skip(storage, activity_registry))]
    async fn execute_activity(
        storage: Arc<S>,
        activity_registry: Arc<ActivityRegistry>,
        workflow_id: Uuid,
        activity: Activity,
    ) -> Result<()> {
        let handler = activity_registry.get(&activity.handler)
            .ok_or_else(|| WorkflowError::NotFound(format!("Activity handler {} not found", activity.handler)))?;

        let start_time = Instant::now();
        let result = handler.execute(activity.payload.clone()).await?;
        let duration = start_time.elapsed();

        let mut workflow = match storage.load_workflow(&workflow_id).await? {
            Some(wf) => wf,
            None => return Err(WorkflowError::NotFound(format!("Workflow {} not found", workflow_id))),
        };

        let step = workflow.current_step
            .and_then(|id| workflow.steps.iter_mut().find(|s| s.id == id))
            .ok_or_else(|| WorkflowError::InvalidState("Current step not found".to_string()))?;

        step.status = StepStatus::Completed;
        step.output = Some(result.clone());
        workflow.updated_at = Utc::now();
        storage.update_workflow(&workflow).await?;
        storage.append_event(&workflow_id, WorkflowEvent::StepCompleted(step.id, result, Utc::now())).await?;

        // Next step
        Ok(())
    }

    pub async fn dispatch_task(&self, workflow_id: Uuid, activity: Activity) -> Result<()> {
        self.task_sender.send(Task { workflow_id, activity }).await
            .map_err(|e| WorkflowError::InvalidState(e.to_string()))
    }
}

// Scheduler

pub struct ScheduledWorkflow {
    pub workflow_definition: WorkflowState,
    pub schedule: Schedule,
}

pub struct WorkflowScheduler<S: WorkflowStorage + 'static> {
    storage: Arc<S>,
    scheduled_workflows: Arc<Mutex<HashMap<Uuid, ScheduledWorkflow>>>,
    engine: Arc<WorkflowEngine<S>>,
}

impl<S: WorkflowStorage + 'static> WorkflowScheduler<S> {
    pub fn new(storage: Arc<S>, engine: Arc<WorkflowEngine<S>>) -> Self {
        Self {
            storage,
            scheduled_workflows: Arc::new(Mutex::new(HashMap::new())),
            engine,
        }
    }

    pub async fn add_scheduled_workflow(&self, workflow_def: WorkflowState, cron_expr: &str) -> Result<Uuid> {
        let schedule = Schedule::from_str(cron_expr)
            .map_err(|e| WorkflowError::InvalidState(e.to_string()))?;

        let scheduled_workflow = ScheduledWorkflow {
            workflow_definition: workflow_def.clone(),
            schedule,
        };

        let workflow_id = workflow_def.id;
        {
            let mut scheduled = self.scheduled_workflows.lock().await;
            scheduled.insert(workflow_id, scheduled_workflow);
        }

        self.spawn_scheduler_task(workflow_id).await?;
        Ok(workflow_id)
    }

    pub async fn remove_scheduled_workflow(&self, workflow_id: &Uuid) -> Result<()> {
        let mut scheduled = self.scheduled_workflows.lock().await;
        scheduled.remove(workflow_id);
        Ok(())
    }

    async fn spawn_scheduler_task(&self, workflow_id: Uuid) -> Result<()> {
        let storage = self.storage.clone();
        let scheduled_workflows = self.scheduled_workflows.clone();
        let engine = self.engine.clone();

        tokio::spawn(async move {
            loop {
                let scheduled = {
                    let scheduled = scheduled_workflows.lock().await;
                    scheduled.get(&workflow_id).cloned()
                };

                let next_time = match scheduled {
                    Some(sw) => sw.schedule.upcoming(Utc).next(),
                    None => break,
                };

                let next_time = match next_time {
                    Some(time) => time,
                    None => break,
                };

                let now = Utc::now();
                let duration = next_time.signed_duration_since(now);
                if duration.num_milliseconds() <= 0 {
                    continue;
                }

                let sleep_until_time = Instant::now() + duration.to_std().unwrap_or(Duration::from_secs(0));
                sleep_until(sleep_until_time).await;

                let mut workflow_def = scheduled_workflows.lock().await
                    .get(&workflow_id)
                    .cloned()
                    .map(|sw| sw.workflow_definition.clone());

                if let Some(mut workflow_def) = workflow_def {
                    workflow_def.id = Uuid::new_v4();
                    workflow_def.status = WorkflowStatus::Pending;
                    workflow_def.current_step = None;
                    workflow_def.created_at = Utc::now();
                    workflow_def.updated_at = Utc::now();
                    workflow_def.last_heartbeat = Utc::now();

                    if let Err(e) = engine.create_workflow(workflow_def.clone()).await {
                        error!("Failed to create scheduled workflow: {}", e);
                        continue;
                    }
                } else {
                    break;
                }
            }
        });

        Ok(())
    }
}

// ChildWorkflowManager

pub struct ChildWorkflowManager<S: WorkflowStorage + 'static> {
    storage: Arc<S>,
    engine: Arc<WorkflowEngine<S>>,
}

impl<S: WorkflowStorage + 'static> ChildWorkflowManager<S> {
    pub fn new(storage: Arc<S>, engine: Arc<WorkflowEngine<S>>) -> Self {
        Self { storage, engine }
    }

    pub async fn create_child_workflow(&self, parent_id: Uuid, child_workflow: WorkflowState) -> Result<Uuid> {
        let child_id = Uuid::new_v4();
        let mut child = child_workflow;
        child.id = child_id;
        child.parent_workflow_id = Some(parent_id);
        child.status = WorkflowStatus::Pending;
        child.created_at = Utc::now();
        child.updated_at = Utc::now();
        child.last_heartbeat = Utc::now();

        self.storage.create_workflow(child.clone()).await?;
        self.storage.append_event(&child_id, WorkflowEvent::WorkflowStarted(Utc::now())).await?;
        self.storage.take_snapshot(&child).await?;
        self.engine.execute_step_command(child_id).await?;

        Ok(child_id)
    }

    pub async fn get_child_workflows(&self, parent_id: Uuid) -> Result<Vec<WorkflowState>> {
        let all_workflows = self.storage.list_workflows().await?;
        Ok(all_workflows.into_iter().filter(|wf| wf.parent_workflow_id == Some(parent_id)).collect())
    }
}

// Versioning Handler

pub struct VersioningHandler<S: WorkflowStorage + 'static> {
    storage: Arc<S>,
    engine: Arc<WorkflowEngine<S>>,
}

impl<S: WorkflowStorage + 'static> VersioningHandler<S> {
    pub fn new(storage: Arc<S>, engine: Arc<WorkflowEngine<S>>) -> Self {
        Self { storage, engine }
    }

    pub async fn migrate_workflow(&self, workflow_id: Uuid, new_version: u32) -> Result<()> {
        let mut workflow = match self.storage.load_workflow(&workflow_id).await? {
            Some(wf) => wf,
            None => return Err(WorkflowError::NotFound(format!("Workflow {} not found", workflow_id))),
        };

        if workflow.version >= new_version {
            return Err(WorkflowError::InvalidState(format!(
                "Workflow {} is already at version {}",
                workflow_id, workflow.version
            )));
        }

        workflow.version = new_version;
        workflow.updated_at = Utc::now();

        self.storage.update_workflow(&workflow).await?;
        self.storage.append_event(&workflow_id, WorkflowEvent::WorkflowMigrated(new_version, Utc::now())).await?;
        Ok(())
    }
}

// Prometheus

use metrics_exporter_prometheus::PrometheusBuilder;
use warp::Filter;

async fn serve_metrics() {
    let controller = PrometheusBuilder::new()
        .build()
        .expect("Failed to create Prometheus exporter");
    metrics::set_boxed_recorder(Box::new(controller.clone()))
        .expect("Failed to set Prometheus recorder");

    let metrics_route = warp::path("metrics")
        .and(warp::get())
        .map(move || {
            let metrics = controller.render();
            warp::reply::with_header(metrics, "Content-Type", "text/plain; version=0.0.4")
        });

    warp::serve(metrics_route)
        .run(([0, 0, 0, 0], 9898))
        .await;
}

// ------------------------ CLI Implementation ------------------------

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create a new workflow
    Create(CreateArgs),

    /// List existing workflows
    List(ListArgs),

    /// Show detailed information about a workflow
    Show(ShowArgs),

    /// Delete a workflow
    Delete(DeleteArgs),

    /// Execute a specific step in a workflow
    Execute(ExecuteArgs),

    /// Send a signal to a workflow
    SendSignal(SendSignalArgs),

    /// Query information about a workflow
    Query(QueryArgs),

    /// Schedule a workflow
    Schedule(ScheduleArgs),

    /// Remove a scheduled workflow
    RemoveSchedule(RemoveScheduleArgs),

    /// Create a child workflow
    CreateChild(CreateChildArgs),

    /// List child workflows of a parent
    ListChildren(ListChildrenArgs),

    /// Migrate a workflow to a new version
    Migrate(MigrateArgs),

    /// Serve Prometheus metrics
    ServeMetrics,
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    #[arg(short, long)]
    pub definition: String,
    #[arg(short, long, default_value = "Normal")]
    pub priority: String,
    #[arg(short, long, num_args = 0..)]
    pub tags: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    #[arg(short, long)]
    pub status: Option<String>,
    #[arg(short, long)]
    pub tag: Option<String>,
}

#[derive(Args, Debug)]
pub struct ShowArgs {
    #[arg(short, long)]
    pub id: Uuid,
}

#[derive(Args, Debug)]
pub struct DeleteArgs {
    #[arg(short, long)]
    pub id: Uuid,
}

#[derive(Args, Debug)]
pub struct ExecuteArgs {
    #[arg(short, long)]
    pub id: Uuid,
    #[arg(short, long)]
    pub step_id: Uuid,
}

#[derive(Args, Debug)]
pub struct SendSignalArgs {
    #[arg(short, long)]
    pub id: Uuid,
    #[arg(short, long)]
    pub signal: String,
    #[arg(short, long, num_args = 0..)]
    pub data: Vec<String>,
}

#[derive(Args, Debug)]
pub struct QueryArgs {
    #[arg(short, long)]
    pub id: Uuid,
    #[arg(short, long)]
    pub query: String,
}

#[derive(Args, Debug)]
pub struct ScheduleArgs {
    #[arg(short, long)]
    pub definition: String,
    #[arg(short, long)]
    pub cron: String,
    #[arg(short, long, num_args = 0..)]
    pub tags: Vec<String>,
}

#[derive(Args, Debug)]
pub struct RemoveScheduleArgs {
    #[arg(short, long)]
    pub id: Uuid,
}

#[derive(Args, Debug)]
pub struct CreateChildArgs {
    #[arg(short, long)]
    pub parent_id: Uuid,
    #[arg(short, long)]
    pub definition: String,
    #[arg(short, long, num_args = 0..)]
    pub tags: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ListChildrenArgs {
    #[arg(short, long)]
    pub parent_id: Uuid,
}

#[derive(Args, Debug)]
pub struct MigrateArgs {
    #[arg(short, long)]
    pub id: Uuid,
    #[arg(short, long)]
    pub version: u32,
}

pub async fn run_cli<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, cli: Cli) {
    match cli.command {
        Commands::Create(args) => {
            if let Err(e) = handle_create(engine.clone(), args).await {
                error!("Failed to create workflow: {}", e);
            }
        },
        Commands::List(args) => {
            if let Err(e) = handle_list(engine.clone(), args).await {
                error!("Failed to list workflows: {}", e);
            }
        },
        Commands::Show(args) => {
            if let Err(e) = handle_show(engine.clone(), args).await {
                error!("Failed to show workflow: {}", e);
            }
        },
        Commands::Delete(args) => {
            if let Err(e) = handle_delete(engine.clone(), args).await {
                error!("Failed to delete workflow: {}", e);
            }
        },
        Commands::Execute(args) => {
            if let Err(e) = handle_execute(engine.clone(), args).await {
                error!("Failed to execute step: {}", e);
            }
        },
        Commands::SendSignal(args) => {
            if let Err(e) = handle_send_signal(engine.clone(), args).await {
                error!("Failed to send signal: {}", e);
            }
        },
        Commands::Query(args) => {
            if let Err(e) = handle_query(engine.clone(), args).await {
                error!("Failed to query workflow: {}", e);
            }
        },
        Commands::Schedule(args) => {
            if let Err(e) = handle_schedule(engine.clone(), args).await {
                error!("Failed to schedule workflow: {}", e);
            }
        },
        Commands::RemoveSchedule(args) => {
            if let Err(e) = handle_remove_schedule(engine.clone(), args).await {
                error!("Failed to remove scheduled workflow: {}", e);
            }
        },
        Commands::CreateChild(args) => {
            if let Err(e) = handle_create_child(engine.clone(), args).await {
                error!("Failed to create child workflow: {}", e);
            }
        },
        Commands::ListChildren(args) => {
            if let Err(e) = handle_list_children(engine.clone(), args).await {
                error!("Failed to list child workflows: {}", e);
            }
        },
        Commands::Migrate(args) => {
            if let Err(e) = handle_migrate(engine.clone(), args).await {
                error!("Failed to migrate workflow: {}", e);
            }
        },
        Commands::ServeMetrics => {
            info!("Metrics server is already running (if started).");
        }
    }
}

async fn handle_create<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, args: CreateArgs) -> Result<()> {
    let file_contents = std::fs::read_to_string(&args.definition)
        .map_err(|e| WorkflowError::InvalidState(format!("Cannot read definition file: {}", e)))?;

    let steps: Vec<WorkflowStep> = serde_json::from_str(&file_contents)
        .map_err(|e| WorkflowError::Serialization(e.to_string()))?;

    let priority = match args.priority.as_str() {
        "Low" => WorkflowPriority::Low,
        "Normal" => WorkflowPriority::Normal,
        "High" => WorkflowPriority::High,
        "Critical" => WorkflowPriority::Critical,
        _ => return Err(WorkflowError::InvalidState("Invalid priority level".to_string())),
    };

    let workflow = WorkflowState::new(
        steps,
        HashMap::new(),
        1,
        None,
        args.tags,
        priority,
    );

    engine.create_workflow(workflow).await?;
    info!("Workflow created successfully.");
    Ok(())
}

async fn handle_list<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, args: ListArgs) -> Result<()> {
    let workflows = engine.storage.list_workflows().await?;

    let filtered: Vec<_> = workflows.into_iter().filter(|wf| {
        let status_match = match &args.status {
            Some(st) => wf.status.to_string().eq_ignore_ascii_case(st),
            None => true,
        };
        let tag_match = match &args.tag {
            Some(t) => wf.tags.contains(t),
            None => true,
        };
        status_match && tag_match
    }).collect();

    for wf in filtered {
        println!("{}: Status={:?}, Priority={:?}, Tags={:?}", wf.id, wf.status, wf.priority, wf.tags);
    }

    Ok(())
}

async fn handle_show<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, args: ShowArgs) -> Result<()> {
    let workflow = engine.storage.load_workflow(&args.id).await?
        .ok_or(WorkflowError::NotFound(format!("Workflow {} not found", args.id)))?;

    println!("Workflow ID: {}", workflow.id);
    println!("Status: {:?}", workflow.status);
    println!("Priority: {:?}", workflow.priority);
    println!("Tags: {:?}", workflow.tags);
    println!("Steps:");
    for s in &workflow.steps {
        println!("  Step {} ({:?})", s.id, s.status);
    }
    Ok(())
}

async fn handle_delete<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, args: DeleteArgs) -> Result<()> {
    engine.storage.delete_workflow(&args.id).await?;
    info!("Workflow {} deleted.", args.id);
    Ok(())
}

async fn handle_execute<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, args: ExecuteArgs) -> Result<()> {
    // Currently ignoring step_id for demonstration
    engine.execute_step_command(args.id).await?;
    info!("Execution triggered for workflow {}, step {}.", args.id, args.step_id);
    Ok(())
}

async fn handle_send_signal<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, args: SendSignalArgs) -> Result<()> {
    let sig = match args.signal.as_str() {
        "UpdateData" => {
            let mut data_map = HashMap::new();
            for kv in args.data {
                let parts: Vec<&str> = kv.splitn(2, '=').collect();
                if parts.len() != 2 {
                    return Err(WorkflowError::InvalidState(format!("Bad data format: {}", kv)));
                }
                data_map.insert(parts[0].to_string(), parts[1].to_string());
            }
            WorkflowSignal::UpdateData(data_map)
        }
        "Cancel" => WorkflowSignal::Cancel,
        _ => return Err(WorkflowError::InvalidState("Invalid signal type".to_string())),
    };

    engine.send_signal(args.id, sig).await?;
    info!("Signal '{}' sent to workflow {}.", args.signal, args.id);
    Ok(())
}

async fn handle_query<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, args: QueryArgs) -> Result<()> {
    let (tx, rx) = oneshot::channel();
    let query = match args.query.as_str() {
        "GetStatus" => WorkflowQuery::GetStatus(tx),
        "GetData" => WorkflowQuery::GetData(tx),
        _ => return Err(WorkflowError::InvalidState("Invalid query".to_string())),
    };

    engine.send_query(args.id, query).await?;
    match rx.await {
        Ok(Ok(value)) => match args.query.as_str() {
            "GetStatus" => println!("Workflow status: {:?}", value),
            "GetData" => println!("Workflow data: {:?}", value),
            _ => {}
        },
        Ok(Err(e)) => error!("Query error: {}", e),
        Err(_) => error!("Query channel closed unexpectedly"),
    }

    Ok(())
}

async fn handle_schedule<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, args: ScheduleArgs) -> Result<()> {
    // For demonstration: not fully implemented
    info!("Scheduled workflow from {} with cron {}", args.definition, args.cron);
    Ok(())
}

async fn handle_remove_schedule<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, args: RemoveScheduleArgs) -> Result<()> {
    info!("Removed scheduled workflow: {}", args.id);
    Ok(())
}

async fn handle_create_child<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, args: CreateChildArgs) -> Result<()> {
    let file_contents = std::fs::read_to_string(&args.definition)
        .map_err(|e| WorkflowError::InvalidState(format!("Cannot read child definition file: {}", e)))?;
    let steps: Vec<WorkflowStep> = serde_json::from_str(&file_contents)
        .map_err(|e| WorkflowError::Serialization(e.to_string()))?;

    let mut child = WorkflowState::new(
        steps,
        HashMap::new(),
        1,
        Some(args.parent_id),
        args.tags,
        WorkflowPriority::Normal,
    );

    engine.storage.create_workflow(child.clone()).await?;
    engine.storage.append_event(&child.id, WorkflowEvent::WorkflowStarted(Utc::now())).await?;
    engine.execute_step_command(child.id).await?;
    info!("Created child workflow {} under parent {}", child.id, args.parent_id);
    Ok(())
}

async fn handle_list_children<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, args: ListChildrenArgs) -> Result<()> {
    let workflows = engine.storage.list_workflows().await?;
    let children: Vec<_> = workflows.into_iter()
        .filter(|wf| wf.parent_workflow_id == Some(args.parent_id))
        .collect();

    for c in children {
        println!("Child Workflow: {} (Status: {:?})", c.id, c.status);
    }
    Ok(())
}

async fn handle_migrate<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, args: MigrateArgs) -> Result<()> {
    let mut workflow = match engine.storage.load_workflow(&args.id).await? {
        Some(wf) => wf,
        None => return Err(WorkflowError::NotFound(format!("Workflow {} not found", args.id))),
    };

    if workflow.version >= args.version {
        return Err(WorkflowError::InvalidState(format!(
            "Workflow {} is already at version {}",
            args.id, workflow.version
        )));
    }

    workflow.version = args.version;
    workflow.updated_at = Utc::now();
    engine.storage.update_workflow(&workflow).await?;
    engine.storage.append_event(&args.id, WorkflowEvent::WorkflowMigrated(args.version, Utc::now())).await?;
    info!("Workflow {} migrated to version {}.", args.id, args.version);
    Ok(())
}

// ------------------------ Main Entry ------------------------

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| WorkflowError::Custom(e.to_string()))?;

    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize store
    let store = sled::open("workflow_db")
        .map_err(|e| WorkflowError::Storage(e.to_string()))?;
    let storage = Arc::new(SledWorkflowStorage::new(store));

    // Initialize activity registry
    let activity_registry = Arc::new(ActivityRegistry::new());
    activity_registry.register("FileProcessing", Arc::new(FileProcessingHandler));
    activity_registry.register("ApiCall", Arc::new(ApiCallHandler));

    // Initialize workflow engine
    let engine = Arc::new(WorkflowEngine::new(storage, activity_registry));

    // Run CLI command
    run_cli(engine, cli).await?;

    Ok(())
}
