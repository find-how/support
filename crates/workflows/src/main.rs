//! src/main.rs
//! A single-file crate implementation of a comprehensive workflow engine + CLI.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use clap::{Parser, Args, Subcommand};
use cron::Schedule;
use dashmap::DashMap;
use env_logger;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusRecorder};
use serde::{Deserialize, Serialize};
use sled::Db;
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::{error, info};
use uuid::Uuid;

use bincode;

// ------------------------ Errors ------------------------

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Activity error: {0}")]
    Activity(String),
    #[error("Invalid step: {0}")]
    InvalidStep(String),
    #[error("Invalid workflow: {0}")]
    InvalidWorkflow(String),
    #[error("Workflow not found: {0}")]
    NotFound(String),
    #[error("Metrics error: {0}")]
    Metrics(String),
    #[error("Invalid state: {0}")]
    InvalidState(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Custom error: {0}")]
    Custom(String),
}

impl From<sled::Error> for WorkflowError {
    fn from(err: sled::Error) -> Self {
        WorkflowError::Storage(err.to_string())
    }
}

impl From<uuid::Error> for WorkflowError {
    fn from(err: uuid::Error) -> Self {
        WorkflowError::InvalidInput(err.to_string())
    }
}

impl From<cron::error::Error> for WorkflowError {
    fn from(err: cron::error::Error) -> Self {
        WorkflowError::InvalidInput(err.to_string())
    }
}

impl From<bincode::Error> for WorkflowError {
    fn from(err: bincode::Error) -> Self {
        WorkflowError::Storage(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, WorkflowError>;

// ------------------------ Models and Enums ------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityResult {
    pub success: bool,
    pub output: HashMap<String, String>,
    pub error_message: Option<String>,
    pub duration: Duration,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug)]
pub enum WorkflowQuery {
    GetWorkflow(oneshot::Sender<Result<Option<WorkflowState>>>),
    GetStatus(oneshot::Sender<Result<WorkflowStatus>>),
    GetSteps(oneshot::Sender<Result<Vec<Step>>>),
    GetStep(Uuid, oneshot::Sender<Result<Option<Step>>>),
}

impl WorkflowQuery {
    pub fn handle(self, workflow: &mut WorkflowState) -> Result<()> {
        match self {
            WorkflowQuery::GetStatus(sender) => {
                sender.send(Ok(workflow.status.clone()))
                    .map_err(|_| WorkflowError::Custom("Failed to send status".into()))?;
            }
            WorkflowQuery::GetSteps(sender) => {
                sender.send(Ok(workflow.steps.clone()))
                    .map_err(|_| WorkflowError::Custom("Failed to send steps".into()))?;
            }
            WorkflowQuery::GetStep(id, sender) => {
                let step = workflow.steps.iter().find(|s| s.id == id).cloned();
                sender.send(Ok(step))
                    .map_err(|_| WorkflowError::Custom("Failed to send step".into()))?;
            }
            WorkflowQuery::GetWorkflow(sender) => {
                sender.send(Ok(Some(workflow.clone())))
                    .map_err(|_| WorkflowError::Custom("Failed to send workflow".into()))?;
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct MetricsController {
    recorder: Arc<PrometheusRecorder>,
}

impl MetricsController {
    pub fn new(addr: SocketAddr) -> Result<Self> {
        let (recorder, server_future) = PrometheusBuilder::new()
            .with_http_listener(addr)
            .build()
            .map_err(|e| WorkflowError::Metrics(e.to_string()))?;

        // Spawn the server task
        tokio::spawn(server_future);

        Ok(Self {
            recorder: Arc::new(recorder),
        })
    }
}

// ------------------------ CLI Implementation ------------------------

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Start(StartArgs),
    Stop(StopArgs),
    Schedule(ScheduleArgs),
    RemoveSchedule(RemoveScheduleArgs),
    List(ListArgs),
    Status(StatusArgs),
}

#[derive(Args)]
pub struct StartArgs {
    #[arg(short, long)]
    pub workflow_type: String,
    #[arg(short, long, value_parser = parse_key_val)]
    pub params: Vec<(String, String)>,
}

#[derive(Args)]
pub struct StopArgs {
    #[arg(short, long)]
    pub id: String,
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args)]
pub struct ScheduleArgs {
    #[arg(short, long)]
    pub name: String,
    #[arg(short, long)]
    pub cron: String,
    #[arg(short, long, value_parser = parse_key_val)]
    pub params: Vec<(String, String)>,
}

#[derive(Args)]
pub struct RemoveScheduleArgs {
    #[arg(short, long)]
    pub id: String,
}

#[derive(Args)]
pub struct ListArgs {}

#[derive(Args)]
pub struct StatusArgs {
    #[arg(short, long)]
    pub id: String,
}

fn parse_key_val(s: &str) -> Result<(String, String)> {
    let pos = s.find('=').ok_or_else(|| WorkflowError::Custom("Invalid key=value pair".into()))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

pub async fn run_cli<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Start(args) => {
            handle_start(engine.clone(), args).await?;
        },
        Commands::Stop(args) => {
            handle_stop(engine.clone(), args).await?;
        },
        Commands::Schedule(args) => {
            handle_schedule(engine.clone(), args).await?;
        },
        Commands::RemoveSchedule(args) => {
            handle_remove_schedule(engine.clone(), args).await?;
        },
        Commands::List(args) => {
            handle_list(engine.clone(), args).await?;
        },
        Commands::Status(args) => {
            handle_status(engine.clone(), args).await?;
        },
    }
    Ok(())
}

async fn handle_start<S: WorkflowStorage>(engine: Arc<WorkflowEngine<S>>, args: StartArgs) -> Result<()> {
    let params: HashMap<String, String> = args.params.into_iter().collect();
    let workflow = WorkflowState::new(args.workflow_type, params);
    engine.create_workflow(workflow).await?;
    info!("Started workflow");
    Ok(())
}

async fn handle_stop<S: WorkflowStorage>(engine: Arc<WorkflowEngine<S>>, args: StopArgs) -> Result<()> {
    engine.stop_workflow(args.id.clone(), args.force).await?;
    info!("Stopped workflow {}", args.id);
    Ok(())
}

async fn handle_schedule<S: WorkflowStorage>(engine: Arc<WorkflowEngine<S>>, args: ScheduleArgs) -> Result<()> {
    let params: HashMap<String, String> = args.params.into_iter().collect();
    engine.create_schedule(args.name.clone(), args.cron, params).await?;
    info!("Scheduled workflow {}", args.name);
    Ok(())
}

async fn handle_remove_schedule<S: WorkflowStorage>(engine: Arc<WorkflowEngine<S>>, args: RemoveScheduleArgs) -> Result<()> {
    engine.remove_schedule(args.id.clone()).await?;
    info!("Removed schedule {}", args.id);
    Ok(())
}

async fn handle_list<S: WorkflowStorage>(engine: Arc<WorkflowEngine<S>>, _args: ListArgs) -> Result<()> {
    let workflows = engine.storage.list_workflows().await?;
    for workflow in workflows {
        info!("Workflow: {:?}", workflow);
    }
    Ok(())
}

async fn handle_status<S: WorkflowStorage + 'static>(engine: Arc<WorkflowEngine<S>>, args: StatusArgs) -> Result<()> {
    let workflow = engine.storage.load_workflow(&Uuid::parse_str(&args.id)
        .map_err(|e| WorkflowError::InvalidState(e.to_string()))?).await?;

    match workflow {
        Some(wf) => {
            println!("Workflow ID: {}, Status: {:?}", args.id, wf.status);
            Ok(())
        }
        None => {
            println!("Workflow not found: {}", args.id);
            Ok(())
        }
    }
}

// ------------------------ Main Entry ------------------------

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    let db = sled::open("workflow.db").map_err(|e| WorkflowError::Storage(e.to_string()))?;
    let storage = Arc::new(SledWorkflowStorage::new(db));
    let activity_registry = Arc::new(ActivityRegistry::new());

    activity_registry.register("file_processing", Arc::new(FileProcessingHandler));
    activity_registry.register("api_call", Arc::new(ApiCallHandler));

    let engine = Arc::new(WorkflowEngine::new(storage, activity_registry));

    let metrics_addr = SocketAddr::from(([127, 0, 0, 1], 9898));
    let _metrics = MetricsController::new(metrics_addr)?;

    run_cli(engine, cli).await?;

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: Uuid,
    pub activity_type: String,
    pub params: HashMap<String, String>,
    pub status: StepStatus,
    pub result: Option<ActivityResult>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    pub id: Uuid,
    pub workflow_type: String,
    pub params: HashMap<String, String>,
    pub status: WorkflowStatus,
    pub steps: Vec<Step>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl WorkflowState {
    pub fn new(workflow_type: String, params: HashMap<String, String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            workflow_type,
            params,
            status: WorkflowStatus::Pending,
            steps: Vec::new(),
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }
}

#[async_trait]
pub trait ActivityHandler: Send + Sync {
    async fn execute(&self, params: HashMap<String, String>) -> Result<ActivityResult>;
}

pub struct FileProcessingHandler;

#[async_trait]
impl ActivityHandler for FileProcessingHandler {
    async fn execute(&self, params: HashMap<String, String>) -> Result<ActivityResult> {
        let start = Instant::now();
        // Simulate file processing
        tokio::time::sleep(Duration::from_secs(1)).await;

        Ok(ActivityResult {
            success: true,
            output: params,
            error_message: None,
            duration: start.elapsed(),
            metadata: HashMap::new(),
        })
    }
}

pub struct ApiCallHandler;

#[async_trait]
impl ActivityHandler for ApiCallHandler {
    async fn execute(&self, params: HashMap<String, String>) -> Result<ActivityResult> {
        let start = Instant::now();
        // Simulate API call
        tokio::time::sleep(Duration::from_secs(1)).await;

        Ok(ActivityResult {
            success: true,
            output: params,
            error_message: None,
            duration: start.elapsed(),
            metadata: HashMap::new(),
        })
    }
}

pub struct ActivityRegistry {
    handlers: DashMap<String, Arc<dyn ActivityHandler>>,
}

impl ActivityRegistry {
    pub fn new() -> Self {
        Self {
            handlers: DashMap::new(),
        }
    }

    pub fn register(&self, activity_type: &str, handler: Arc<dyn ActivityHandler>) {
        self.handlers.insert(activity_type.to_string(), handler);
    }

    pub fn get(&self, activity_type: &str) -> Option<Arc<dyn ActivityHandler>> {
        self.handlers.get(activity_type).map(|h| h.clone())
    }
}

#[async_trait]
pub trait WorkflowStorage: Send + Sync {
    async fn save_workflow(&self, workflow: &WorkflowState) -> Result<()>;
    async fn load_workflow(&self, id: &Uuid) -> Result<Option<WorkflowState>>;
    async fn list_workflows(&self) -> Result<Vec<WorkflowState>>;
    async fn delete_workflow(&self, id: &Uuid) -> Result<()>;
}

pub struct SledWorkflowStorage {
    db: Db,
}

impl SledWorkflowStorage {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl WorkflowStorage for SledWorkflowStorage {
    async fn save_workflow(&self, workflow: &WorkflowState) -> Result<()> {
        let key = workflow.id.as_bytes();
        let value = bincode::serialize(workflow)?;
        self.db.insert(key, value)?;
        self.db.flush()?;
        Ok(())
    }

    async fn load_workflow(&self, id: &Uuid) -> Result<Option<WorkflowState>> {
        if let Some(value) = self.db.get(id.as_bytes())? {
            let workflow = bincode::deserialize(&value)?;
            Ok(Some(workflow))
        } else {
            Ok(None)
        }
    }

    async fn list_workflows(&self) -> Result<Vec<WorkflowState>> {
        let mut workflows = Vec::new();
        for item in self.db.iter() {
            let (_, value) = item?;
            let workflow = bincode::deserialize(&value)?;
            workflows.push(workflow);
        }
        Ok(workflows)
    }

    async fn delete_workflow(&self, id: &Uuid) -> Result<()> {
        self.db.remove(id.as_bytes())?;
        self.db.flush()?;
        Ok(())
    }
}

pub struct WorkflowEngine<S: WorkflowStorage + 'static> {
    storage: Arc<S>,
    activity_registry: Arc<ActivityRegistry>,
    active_workflows: Arc<DashMap<Uuid, JoinHandle<()>>>,
}

impl<S: WorkflowStorage + 'static> WorkflowEngine<S> {
    pub fn new(storage: Arc<S>, activity_registry: Arc<ActivityRegistry>) -> Self {
        Self {
            storage,
            activity_registry,
            active_workflows: Arc::new(DashMap::new()),
        }
    }

    pub async fn create_workflow(&self, mut workflow: WorkflowState) -> Result<()> {
        workflow.status = WorkflowStatus::Running;
        workflow.started_at = Some(Utc::now());

        self.storage.save_workflow(&workflow).await?;

        let storage = self.storage.clone();
        let activity_registry = self.activity_registry.clone();
        let workflow_id = workflow.id;

        let handle = tokio::spawn(async move {
            let mut workflow = match storage.load_workflow(&workflow_id).await {
                Ok(Some(w)) => w,
                Ok(None) => {
                    error!("Failed to load workflow: not found");
                    return;
                }
                Err(e) => {
                    error!("Failed to load workflow: {}", e);
                    return;
                }
            };

            let step_count = workflow.steps.len();
            for i in 0..step_count {
                if let Some(handler) = activity_registry.get(&workflow.steps[i].activity_type) {
                    workflow.steps[i].status = StepStatus::Running;
                    workflow.steps[i].started_at = Some(Utc::now());

                    if let Err(e) = storage.save_workflow(&workflow).await {
                        error!("Failed to save workflow: {}", e);
                        continue;
                    }

                    let params = workflow.steps[i].params.clone();
                    match handler.execute(params).await {
                        Ok(result) => {
                            workflow.steps[i].status = StepStatus::Completed;
                            workflow.steps[i].result = Some(result);
                        }
                        Err(e) => {
                            workflow.steps[i].status = StepStatus::Failed;
                            workflow.steps[i].result = Some(ActivityResult {
                                success: false,
                                output: HashMap::new(),
                                error_message: Some(e.to_string()),
                                duration: Duration::from_secs(0),
                                metadata: HashMap::new(),
                            });
                        }
                    }
                    workflow.steps[i].completed_at = Some(Utc::now());

                    if let Err(e) = storage.save_workflow(&workflow).await {
                        error!("Failed to save workflow: {}", e);
                    }
                }
            }
            info!("Workflow {} completed", workflow_id);
        });

        self.active_workflows.insert(workflow_id, handle);
        Ok(())
    }

    pub async fn stop_workflow(&self, id: String, force: bool) -> Result<()> {
        let workflow_id = Uuid::parse_str(&id)?;
        if let Some((_, handle)) = self.active_workflows.remove(&workflow_id) {
            if force {
                handle.abort();
            }
            info!("Workflow {} stopped", workflow_id);
        }
        Ok(())
    }

    pub async fn create_schedule(&self, name: String, cron: String, _params: HashMap<String, String>) -> Result<()> {
        let _schedule = Schedule::from_str(&cron)?;
        // Schedule implementation would go here
        info!("Created schedule {} with cron {}", name, cron);
        Ok(())
    }

    pub async fn remove_schedule(&self, id: String) -> Result<()> {
        // Schedule removal implementation would go here
        info!("Removed schedule {}", id);
        Ok(())
    }
}
