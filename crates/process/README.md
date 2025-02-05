Below is a comprehensive overview of the key components of the `process` crate,
including **traits**, **input structs**, **output structs**, and **error
enums**. Each component is accompanied by detailed explanations and code
snippets to facilitate understanding and integration.

---

## 1. Traits

Traits define shared behavior across different types. In the `process_facade`
crate, the primary traits are `Workload` and `Executor`.

### a. `Workload` Trait

The `Workload` trait represents a unit of work that can be executed. It
encapsulates the execution logic, estimated resource costs, and specific
requirements.

```rust
// src/process.rs
use async_trait::async_trait;
use std::fmt::Debug;

/// Represents a unit of work that can be executed.
#[async_trait]
pub trait Workload: Send + Debug {
    /// The output type produced after executing the workload.
    type Output: Send;

    /// Executes the workload asynchronously.
    async fn execute(&self) -> Result<Self::Output, crate::errors::ProcessError>;

    /// Provides an estimate of the resource costs associated with the workload.
    fn estimated_cost(&self) -> WorkloadCost;

    /// Specifies the requirements or constraints for executing the workload.
    fn requirements(&self) -> WorkloadRequirements;
}
```

### b. `Executor` Trait

The `Executor` trait defines how workloads are executed. It allows for different
execution strategies (e.g., threads, processes) to be implemented
interchangeably.

```rust
// src/pool.rs
use crate::process::{Workload, WorkloadCost, WorkloadRequirements};
use crate::errors::ProcessError;
use async_trait::async_trait;

/// Defines how workloads are executed.
#[async_trait]
pub trait Executor: Send + Sync {
    /// Executes a given workload.
    async fn execute<W: Workload + 'static>(&self, work: W) -> Result<W::Output, ProcessError>;

    /// Returns the available capacity of the executor (0.0 - 1.0).
    fn available_capacity(&self) -> f32;

    /// Returns the execution strategy used by the executor.
    fn strategy(&self) -> ExecutionStrategy;

    /// Determines if the executor supports the given workload requirements.
    fn supports_requirements(&self, requirements: &WorkloadRequirements) -> bool;
}
```

---

## 2. Input Structs

Input structs are used to configure and manage processes and jobs within the
`process_facade` crate.

### a. `Process` Struct

The `Process` struct encapsulates the details of an external process to be
executed. It manages the process lifecycle, configurations, and interactions.

```rust
// src/process.rs
use crate::errors::ProcessError;
use crate::logging::LOGGER;
use crate::config::ProcessConfig;
use crate::pool::Signal;
use anyhow::{anyhow, Result};
use log::{error, info, debug};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader, AsyncWriteExt};
use tokio::process::{Child, Command};
use uuid::Uuid;

/// Holds various configuration options for a process.
#[derive(Debug, Clone)]
pub struct ProcessOptions {
    pub working_dir: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub input: Option<String>,
    pub timeout: Option<Duration>,
    pub idle_timeout: Option<Duration>,
    pub tty: bool,
}

impl Default for ProcessOptions {
    fn default() -> Self {
        Self {
            working_dir: None,
            env: HashMap::new(),
            input: None,
            timeout: Some(Duration::from_secs(60)),
            idle_timeout: None,
            tty: false,
        }
    }
}

/// Represents the result of a process execution.
#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub exit_code: Option<i32>,
    pub output: String,
    pub error_output: String,
}

/// Represents an external process to be executed.
#[derive(Debug, Clone)]
pub struct Process {
    pub id: Uuid,                        // Unique identifier for the process
    command: String,                     // Command to execute
    options: ProcessOptions,            // Configuration options
    child: Option<Child>,                // Handle to the spawned child process
}

impl Process {
    /// Creates a new Process with the given command.
    pub fn new(command: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            command: command.to_string(),
            options: ProcessOptions::default(),
            child: None,
        }
    }

    /// Initializes the process with external configurations.
    pub fn with_config(mut self, config: ProcessConfig) -> Self {
        self.options.working_dir = config.working_dir;
        self.options.env = config.env;
        self.options.input = config.input;
        self.options.timeout = config.timeout;
        self.options.idle_timeout = config.idle_timeout;
        self.options.tty = config.tty.unwrap_or(false);
        self
    }

    /// Sets the working directory.
    pub fn path(mut self, path: impl Into<PathBuf>) -> Self {
        self.options.working_dir = Some(path.into());
        self
    }

    /// Sets the input for the process.
    pub fn input(mut self, input: &str) -> Self {
        self.options.input = Some(input.to_string());
        self
    }

    /// Sets the timeout duration.
    pub fn timeout(mut self, secs: u64) -> Self {
        self.options.timeout = Some(Duration::from_secs(secs));
        self
    }

    /// Disables the timeout.
    pub fn forever(mut self) -> Self {
        self.options.timeout = None;
        self
    }

    /// Sets the idle timeout.
    pub fn idle_timeout(mut self, secs: u64) -> Self {
        self.options.idle_timeout = Some(Duration::from_secs(secs));
        self
    }

    /// Sets environment variables.
    pub fn env(mut self, vars: HashMap<String, String>) -> Self {
        self.options.env.extend(vars);
        self
    }

    /// Removes an environment variable.
    pub fn remove_env(mut self, key: &str) -> Self {
        self.options.env.insert(key.to_string(), "__REMOVE__".to_string());
        self
    }

    /// Enables TTY mode.
    pub fn tty(mut self) -> Self {
        self.options.tty = true;
        self
    }

    /// Runs the process synchronously and waits for it to finish.
    pub async fn run(mut self) -> Result<ProcessResult, ProcessError> {
        let mut cmd = parse_command(&self.command)?;
        apply_options(&mut cmd, &self.options)?;

        if self.options.tty {
            // TTY implementation is platform-specific and beyond this scope.
            // Placeholder for TTY functionality.
        }

        if self.options.input.is_some() {
            cmd.stdin(Stdio::piped());
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        info!("Starting process [{}]: {}", self.id, self.command);
        let mut child = cmd.spawn().map_err(ProcessError::IoError)?;
        self.child = Some(child);

        // Write to stdin if input is provided.
        if let Some(input) = &self.options.input {
            if let Some(mut stdin) = self.child.as_mut().unwrap().stdin.take() {
                tokio::spawn(async move {
                    if let Err(e) = stdin.write_all(input.as_bytes()).await {
                        error!("Failed to write to stdin: {}", e);
                    }
                });
            }
        }

        // Monitor idle timeout if specified.
        if let Some(idle_duration) = self.options.idle_timeout {
            Self::monitor_idle_timeout(&mut self.child.as_mut().unwrap(), idle_duration).await?;
        }

        // Apply overall timeout if specified.
        let output = if let Some(timeout_duration) = self.options.timeout {
            match tokio::time::timeout(timeout_duration, self.child.as_mut().unwrap().wait_with_output()).await {
                Ok(Ok(out)) => out,
                Ok(Err(e)) => return Err(ProcessError::IoError(e)),
                Err(_) => {
                    // Process timed out; attempt to kill it.
                    self.child.as_mut().unwrap().kill().await.map_err(ProcessError::IoError)?;
                    return Err(ProcessError::Timeout(timeout_duration));
                }
            }
        } else {
            self.child.as_mut().unwrap().wait_with_output().await.map_err(ProcessError::IoError)?
        };

        info!("Process [{}] exited with code {:?}", self.id, output.status.code());

        Ok(ProcessResult {
            exit_code: output.status.code(),
            output: String::from_utf8_lossy(&output.stdout).to_string(),
            error_output: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    /// Starts the process asynchronously and returns the Process instance.
    pub async fn start(mut self) -> Result<Self, ProcessError> {
        let mut cmd = parse_command(&self.command)?;
        apply_options(&mut cmd, &self.options)?;

        if self.options.tty {
            // TTY implementation is platform-specific and beyond this scope.
            // Placeholder for TTY functionality.
        }

        if self.options.input.is_some() {
            cmd.stdin(Stdio::piped());
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        info!("Starting process [{}]: {}", self.id, self.command);
        let mut child = cmd.spawn().map_err(ProcessError::IoError)?;
        self.child = Some(child);

        // Write to stdin if input is provided.
        if let Some(input) = &self.options.input {
            if let Some(mut stdin) = self.child.as_mut().unwrap().stdin.take() {
                tokio::spawn(async move {
                    if let Err(e) = stdin.write_all(input.as_bytes()).await {
                        error!("Failed to write to stdin: {}", e);
                    }
                });
            }
        }

        Ok(self)
    }

    /// Sends a signal to the running process.
    pub async fn send_signal(&self, signal: Signal) -> Result<(), ProcessError> {
        #[cfg(unix)]
        {
            use nix::sys::signal::kill;
            use nix::unistd::Pid;

            if let Some(child) = &self.child {
                if let Some(pid) = child.id() {
                    let nix_signal: nix::sys::signal::Signal = signal.into();
                    kill(Pid::from_raw(pid as i32), nix_signal).map_err(|e| {
                        ProcessError::SignalError(format!("Failed to send signal: {}", e))
                    })?;
                    info!("Sent signal {:?} to process [{}]", signal, self.id);
                    Ok(())
                } else {
                    Err(ProcessError::Other(anyhow!("Process PID not available")))
                }
            } else {
                Err(ProcessError::Other(anyhow!("Process not running")))
            }
        }

        #[cfg(windows)]
        {
            use std::io::Write;

            if let Some(child) = &self.child {
                match signal {
                    Signal::SIGTERM | Signal::SIGINT | Signal::SIGKILL => {
                        // Windows does not support POSIX signals.
                        // Use child.kill() as an alternative for termination.
                        child.kill().await.map_err(ProcessError::IoError)?;
                        info!("Forcefully killed process [{}] on Windows", self.id);
                        Ok(())
                    }
                    // Add more Windows-specific signals if necessary.
                }
            } else {
                Err(ProcessError::Other(anyhow!("Process not running")))
            }
        }
    }

    /// Retrieves the PID of the running process.
    pub fn get_pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(|child| child.id())
    }

    /// Monitors the process for idle timeouts.
    async fn monitor_idle_timeout(child: &mut Child, idle_timeout: Duration) -> Result<(), ProcessError> {
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut stderr = BufReader::new(child.stderr.take().unwrap());

        let (tx, mut rx) = mpsc::unbounded_channel::<()>();

        // Spawn tasks to monitor stdout and stderr.
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let mut line = String::new();
            loop {
                match stdout.read_line(&mut line).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let _ = tx_clone.send(());
                        line.clear();
                    }
                    Err(_) => break,
                }
            }
        });

        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let mut line = String::new();
            loop {
                match stderr.read_line(&mut line).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let _ = tx_clone.send(());
                        line.clear();
                    }
                    Err(_) => break,
                }
            }
        });

        let mut last_activity = Instant::now();

        loop {
            tokio::select! {
                _ = rx.recv() => {
                    last_activity = Instant::now();
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    if last_activity.elapsed() > idle_timeout {
                        // Kill the process due to idle timeout.
                        child.kill().await.map_err(ProcessError::IoError)?;
                        error!("Process exceeded idle timeout of {:?}", idle_timeout);
                        return Err(ProcessError::IdleTimeout(idle_timeout));
                    }
                    // Check if the process has exited.
                    match child.try_wait()? {
                        Some(_) => break, // Process has exited
                        None => {}
                    }
                }
            }
        }

        Ok(())
    }

    /// Streams stdout and stderr asynchronously to provided channels.
    pub async fn stream_output(
        &mut self,
        tx_stdout: UnboundedSender<String>,
        tx_stderr: UnboundedSender<String>,
    ) -> Result<(), ProcessError> {
        if let Some(child) = &mut self.child {
            if let Some(stdout) = child.stdout.take() {
                let mut reader = BufReader::new(stdout).lines();
                let tx_clone = tx_stdout.clone();
                tokio::spawn(async move {
                    while let Ok(Some(line)) = reader.next_line().await {
                        let _ = tx_clone.send(line);
                    }
                });
            }

            if let Some(stderr) = child.stderr.take() {
                let mut reader = BufReader::new(stderr).lines();
                let tx_clone = tx_stderr.clone();
                tokio::spawn(async move {
                    while let Ok(Some(line)) = reader.next_line().await {
                        let _ = tx_clone.send(line);
                    }
                });
            }

            Ok(())
        } else {
            Err(ProcessError::Other(anyhow!("Process not running")))
        }
    }
}
```

### b. `ProcessConfig` Struct

The `ProcessConfig` struct allows for external configuration of processes,
enhancing flexibility and adaptability.

```rust
// src/config.rs
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;

/// Configuration structure for initializing a Process.
#[derive(Debug, Deserialize, Clone)]
pub struct ProcessConfig {
    pub working_dir: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub input: Option<String>,
    pub timeout: Option<u64>,        // in seconds
    pub idle_timeout: Option<u64>,   // in seconds
    pub tty: Option<bool>,
}

impl ProcessConfig {
    /// Loads configuration from a file and environment variables.
    pub fn load() -> Result<Self> {
        let mut settings = config::Config::default();

        // Add configuration file (optional)
        settings.merge(config::File::with_name("config").required(false))?;

        // Add environment variables with a prefix (optional)
        settings.merge(config::Environment::with_prefix("PROCESS"))?;

        // Deserialize into ProcessConfig
        let config: ProcessConfig = settings.try_into()?;
        Ok(config)
    }
}
```

### c. `Job` Struct

The `Job` struct represents a task to be executed, encapsulating the command and
its configuration.

```rust
// src/pool.rs
use crate::process::{Process, ProcessResult, ProcessConfig};
use crate::errors::ProcessError;
use uuid::Uuid;

/// Represents a job to be executed by the ProcessPool.
#[derive(Debug)]
pub struct Job {
    pub id: Uuid,                  // Unique identifier for the job
    pub command: String,           // Command to execute
    pub config: Option<ProcessConfig>, // Optional configuration for the process
}
```

---

## 3. Output Structs

Output structs encapsulate the results of process executions, providing
structured access to outputs and exit statuses.

### `ProcessResult` Struct

The `ProcessResult` struct holds the outcome of a process execution, including
exit codes and captured outputs.

```rust
// src/process.rs
/// Represents the result of a process execution.
#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub exit_code: Option<i32>,      // Exit code of the process
    pub output: String,              // Captured standard output
    pub error_output: String,        // Captured standard error output
}
```

---

## 4. Error Enums

Error enums provide detailed and specific error types, enhancing error handling
and debugging capabilities.

### a. `ProcessError` Enum

The `ProcessError` enum encapsulates all possible errors that can occur during
process management.

```rust
// src/errors.rs
use thiserror::Error;
use std::io;

/// Represents errors that can occur during process execution and management.
#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("Process timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Process exceeded idle timeout of {0:?}")]
    IdleTimeout(std::time::Duration),

    #[error("Process failed with exit code {0}")]
    NonZeroExit(i32),

    #[error("Failed to send signal: {0}")]
    SignalError(String),

    #[error("Process pool is full ({0} processes)")]
    PoolFull(usize),

    #[error("Process '{0}' failed: {1}")]
    ProcessFailed(String, String),

    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}
```

### b. `ThreadError` Enum

The `ThreadError` enum captures errors related to thread operations within the
process management system.

```rust
// src/errors.rs
use thiserror::Error;

/// Represents errors that can occur during thread operations.
#[derive(Error, Debug)]
pub enum ThreadError {
    #[error("Thread operation timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Thread pool is at capacity ({0} threads)")]
    PoolFull(usize),

    #[error("Thread '{0}' not found")]
    ThreadNotFound(String),

    #[error("Channel send error: {0}")]
    ChannelError(#[from] tokio::sync::mpsc::error::SendError<ThreadMessage>),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```

---

## 5. Additional Components

### a. `Signal` Enum

The `Signal` enum abstracts platform-specific signals, enabling cross-platform
signal handling.

```rust
// src/pool.rs
use crate::process::ProcessError;
use nix::sys::signal::Signal as NixSignal;

/// Represents various signals that can be sent to processes.
#[derive(Debug, Clone, Copy)]
pub enum Signal {
    SIGTERM,
    SIGINT,
    SIGKILL,
    // Add more signals as needed.
}

impl From<Signal> for NixSignal {
    fn from(signal: Signal) -> Self {
        match signal {
            Signal::SIGTERM => NixSignal::SIGTERM,
            Signal::SIGINT => NixSignal::SIGINT,
            Signal::SIGKILL => NixSignal::SIGKILL,
        }
    }
}
```

---

## 6. Example Implementations

To illustrate how these components interact, below are example usages
demonstrating various functionalities.

### a. Executing a Single Process with Configuration

```rust
use process_facade::{Process, init_logging, ProcessConfig};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    init_logging();

    // Load configuration
    let config = ProcessConfig::load()?;

    // Create a new process with configuration
    let process = Process::new("ls -la")
        .with_config(config)
        .path("/usr/bin")
        .env(HashMap::from([
            ("KEY".to_string(), "value".to_string()),
        ]))
        .timeout(30)
        .idle_timeout(10)
        .start()
        .await?;

    // Optionally, stream output
    let (tx_stdout, mut rx_stdout) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (tx_stderr, mut rx_stderr) = tokio::sync::mpsc::unbounded_channel::<String>();
    process.stream_output(tx_stdout.clone(), tx_stderr.clone()).await?;

    // Handle stdout
    tokio::spawn(async move {
        while let Some(line) = rx_stdout.recv().await {
            println!("STDOUT: {}", line);
        }
    });

    // Handle stderr
    tokio::spawn(async move {
        while let Some(line) = rx_stderr.recv().await {
            eprintln!("STDERR: {}", line);
        }
    });

    // Run the process and capture the result
    let result = process.run().await?;

    println!("Exit Code: {:?}", result.exit_code);
    println!("Output: {}", result.output);
    println!("Error Output: {}", result.error_output);

    Ok(())
}
```

### b. Managing a Pool of Processes

```rust
use process_facade::{Process, ProcessPool, init_logging, ProcessConfig, Job};
use std::collections::HashMap;
use uuid::Uuid;
use tokio::signal::ctrl_c;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    init_logging();

    // Load configuration
    let config = ProcessConfig::load()?;

    // Initialize ProcessPool with a maximum of 3 concurrent processes
    let pool = ProcessPool::new(3);

    // Define jobs with configurations
    let jobs = vec![
        Job {
            id: Uuid::new_v4(),
            command: "php artisan migrate".to_string(),
            config: Some(config.clone()),
        },
        Job {
            id: Uuid::new_v4(),
            command: "php artisan cache:clear".to_string(),
            config: Some(ProcessConfig {
                working_dir: Some("/path/to/laravel".into()),
                env: HashMap::from([
                    ("APP_ENV".to_string(), "production".to_string()),
                    ("CACHE_DRIVER".to_string(), "redis".to_string()),
                ]),
                input: None,
                timeout: Some(60),
                idle_timeout: Some(15),
                tty: Some(false),
            }),
        },
        Job {
            id: Uuid::new_v4(),
            command: "composer install".to_string(),
            config: Some(ProcessConfig {
                working_dir: Some("/path/to/laravel".into()),
                env: HashMap::from([
                    ("APP_ENV".to_string(), "production".to_string()),
                    ("CACHE_DRIVER".to_string(), "redis".to_string()),
                ]),
                input: None,
                timeout: Some(120),
                idle_timeout: Some(30),
                tty: Some(false),
            }),
        },
        // Add more jobs as needed
    ];

    // Enqueue jobs
    for job in jobs {
        pool.enqueue_job(job)?;
        info!("Enqueued job [{}].", job.id);
    }

    // Handle graceful shutdown on Ctrl+C
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        info!("Received Ctrl+C, initiating shutdown.");
        if let Err(e) = pool_clone.shutdown().await {
            error!("Error during shutdown: {}", e);
        }
    });

    // Wait indefinitely or until shutdown
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
```

---

## 7. Summary of Components

Below is a consolidated view of all the traits, structs, and enums discussed.

### a. Traits

```rust
// src/process.rs and src/pool.rs

use async_trait::async_trait;
use crate::errors::ProcessError;
use std::fmt::Debug;

/// Represents a unit of work that can be executed.
#[async_trait]
pub trait Workload: Send + Debug {
    type Output: Send;

    async fn execute(&self) -> Result<Self::Output, ProcessError>;
    fn estimated_cost(&self) -> WorkloadCost;
    fn requirements(&self) -> WorkloadRequirements;
}

/// Defines how workloads are executed.
#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute<W: Workload + 'static>(&self, work: W) -> Result<W::Output, ProcessError>;
    fn available_capacity(&self) -> f32;
    fn strategy(&self) -> ExecutionStrategy;
    fn supports_requirements(&self, requirements: &WorkloadRequirements) -> bool;
}
```

### b. Input Structs

```rust
// src/process.rs

use crate::config::ProcessConfig;
use crate::pool::Signal;
use crate::errors::ProcessError;
use uuid::Uuid;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Child;

/// Holds various configuration options for a process.
#[derive(Debug, Clone)]
pub struct ProcessOptions {
    pub working_dir: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub input: Option<String>,
    pub timeout: Option<Duration>,
    pub idle_timeout: Option<Duration>,
    pub tty: bool,
}

/// Represents an external process to be executed.
#[derive(Debug, Clone)]
pub struct Process {
    pub id: Uuid,                        // Unique identifier for the process
    command: String,                     // Command to execute
    options: ProcessOptions,            // Configuration options
    child: Option<Child>,                // Handle to the spawned child process
}
```

```rust
// src/config.rs

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;

/// Configuration structure for initializing a Process.
#[derive(Debug, Deserialize, Clone)]
pub struct ProcessConfig {
    pub working_dir: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub input: Option<String>,
    pub timeout: Option<u64>,        // in seconds
    pub idle_timeout: Option<u64>,   // in seconds
    pub tty: Option<bool>,
}

impl ProcessConfig {
    /// Loads configuration from a file and environment variables.
    pub fn load() -> Result<Self> {
        let mut settings = config::Config::default();

        // Add configuration file (optional)
        settings.merge(config::File::with_name("config").required(false))?;

        // Add environment variables with a prefix (optional)
        settings.merge(config::Environment::with_prefix("PROCESS"))?;

        // Deserialize into ProcessConfig
        let config: ProcessConfig = settings.try_into()?;
        Ok(config)
    }
}
```

```rust
// src/pool.rs

use crate::process::{Process, ProcessResult, ProcessConfig};
use crate::errors::ProcessError;
use crate::logging::LOGGER;
use anyhow::Result;
use log::{error, info, debug};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::process::Child;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

/// Represents a job to be executed by the ProcessPool.
#[derive(Debug)]
pub struct Job {
    pub id: Uuid,                  // Unique identifier for the job
    pub command: String,           // Command to execute
    pub config: Option<ProcessConfig>, // Optional configuration for the process
}
```

### c. Output Structs

```rust
// src/process.rs

/// Represents the result of a process execution.
#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub exit_code: Option<i32>,      // Exit code of the process
    pub output: String,              // Captured standard output
    pub error_output: String,        // Captured standard error output
}
```

### d. Error Enums

```rust
// src/errors.rs

use thiserror::Error;
use std::io;

/// Represents errors that can occur during process execution and management.
#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("Process timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Process exceeded idle timeout of {0:?}")]
    IdleTimeout(std::time::Duration),

    #[error("Process failed with exit code {0}")]
    NonZeroExit(i32),

    #[error("Failed to send signal: {0}")]
    SignalError(String),

    #[error("Process pool is full ({0} processes)")]
    PoolFull(usize),

    #[error("Process '{0}' failed: {1}")]
    ProcessFailed(String, String),

    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Represents errors that can occur during thread operations.
#[derive(Error, Debug)]
pub enum ThreadError {
    #[error("Thread operation timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Thread pool is at capacity ({0} threads)")]
    PoolFull(usize),

    #[error("Thread '{0}' not found")]
    ThreadNotFound(String),

    #[error("Channel send error: {0}")]
    ChannelError(#[from] tokio::sync::mpsc::error::SendError<ThreadMessage>),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```

---

## 8. Traits Summary

Below is a summary of the traits defined in the crate.

### a. `Workload` Trait

```rust
// src/process.rs

use async_trait::async_trait;
use std::fmt::Debug;

/// Represents a unit of work that can be executed.
#[async_trait]
pub trait Workload: Send + Debug {
    type Output: Send;

    /// Executes the workload asynchronously.
    async fn execute(&self) -> Result<Self::Output, crate::errors::ProcessError>;

    /// Provides an estimate of the resource costs associated with the workload.
    fn estimated_cost(&self) -> WorkloadCost;

    /// Specifies the requirements or constraints for executing the workload.
    fn requirements(&self) -> WorkloadRequirements;
}
```

### b. `Executor` Trait

```rust
// src/pool.rs

use crate::process::{Workload, WorkloadCost, WorkloadRequirements};
use crate::errors::ProcessError;
use async_trait::async_trait;

/// Defines how workloads are executed.
#[async_trait]
pub trait Executor: Send + Sync {
    /// Executes a given workload.
    async fn execute<W: Workload + 'static>(&self, work: W) -> Result<W::Output, ProcessError>;

    /// Returns the available capacity of the executor (0.0 - 1.0).
    fn available_capacity(&self) -> f32;

    /// Returns the execution strategy used by the executor.
    fn strategy(&self) -> ExecutionStrategy;

    /// Determines if the executor supports the given workload requirements.
    fn supports_requirements(&self, requirements: &WorkloadRequirements) -> bool;
}
```

---

## 9. Example Implementations of Traits

### a. `ComputeWorkload` Implementation

An example implementation of the `Workload` trait, representing a CPU-intensive
task.

```rust
// src/workloads.rs

use crate::process::{Workload, WorkloadCost, WorkloadRequirements};
use crate::errors::ProcessError;
use anyhow::Result;
use std::time::Duration;
use std::fmt::Debug;

/// A workload that performs a CPU-intensive computation.
#[derive(Debug)]
pub struct ComputeWorkload {
    pub iterations: usize,
    pub memory_required: usize,
}

#[async_trait::async_trait]
impl Workload for ComputeWorkload {
    type Output = u64;

    async fn execute(&self) -> Result<Self::Output, ProcessError> {
        let mut result = 0;
        for i in 0..self.iterations {
            result += i as u64;
        }
        Ok(result)
    }

    fn estimated_cost(&self) -> WorkloadCost {
        WorkloadCost {
            cpu_intensity: 0.8,
            memory_usage: self.memory_required,
            io_intensity: 0.1,
            estimated_duration: Duration::from_secs(1),
        }
    }

    fn requirements(&self) -> WorkloadRequirements {
        WorkloadRequirements {
            isolation_required: false,
            realtime_priority: false,
            memory_limit: Some(self.memory_required),
            timeout: Some(Duration::from_secs(30)),
        }
    }
}
```

### b. `ThreadExecutor` Implementation

An example implementation of the `Executor` trait, managing a pool of threads.

```rust
// src/executors.rs

use crate::pool::{Executor, ExecutionStrategy};
use crate::process::{Workload, WorkloadCost, WorkloadRequirements};
use crate::errors::ProcessError;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

/// An executor that runs workloads on separate threads.
pub struct ThreadExecutor {
    max_threads: usize,
    active_threads: Arc<Mutex<usize>>,
}

impl ThreadExecutor {
    pub fn new(max_threads: usize) -> Self {
        Self {
            max_threads,
            active_threads: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl Executor for ThreadExecutor {
    async fn execute<W: Workload + 'static>(&self, work: W) -> Result<W::Output, ProcessError> {
        let mut active = self.active_threads.lock().await;
        if *active >= self.max_threads {
            return Err(ProcessError::PoolFull(self.max_threads));
        }
        *active += 1;
        drop(active);

        tokio::spawn(async move {
            let result = work.execute().await;
            let mut active = self.active_threads.lock().await;
            *active -= 1;
            result
        })
        .await
        .map_err(|e| ProcessError::Other(anyhow::anyhow!("Task join error: {}", e)))?
    }

    fn available_capacity(&self) -> f32 {
        let active = tokio::sync::Mutex::try_lock(&self.active_threads).ok().map_or(0.0, |guard| {
            1.0 - (*guard as f32 / self.max_threads as f32)
        });
        active
    }

    fn strategy(&self) -> ExecutionStrategy {
        ExecutionStrategy::Thread
    }

    fn supports_requirements(&self, requirements: &WorkloadRequirements) -> bool {
        !requirements.isolation_required
    }
}
```

---

## 10. Summary of Enums

### a. `Signal` Enum

```rust
// src/pool.rs

use crate::process::ProcessError;
use nix::sys::signal::Signal as NixSignal;

/// Represents various signals that can be sent to processes.
#[derive(Debug, Clone, Copy)]
pub enum Signal {
    SIGTERM,
    SIGINT,
    SIGKILL,
    // Add more signals as needed.
}

impl From<Signal> for NixSignal {
    fn from(signal: Signal) -> Self {
        match signal {
            Signal::SIGTERM => NixSignal::SIGTERM,
            Signal::SIGINT => NixSignal::SIGINT,
            Signal::SIGKILL => NixSignal::SIGKILL,
        }
    }
}
```

### b. `ExecutionStrategy` Enum

Defines the strategy used by executors.

```rust
// src/pool.rs

/// Represents the execution strategy used by an Executor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExecutionStrategy {
    Thread,
    ThreadPool,
    Process,
    ProcessPool,
    Auto,
}
```

---

## 11. Conclusion

The `process_facade` crate now encompasses a robust set of traits, structs, and
enums that facilitate comprehensive process management in Rust. Here's a quick
recap:

- **Traits:**
  - `Workload`: Defines executable units of work.
  - `Executor`: Outlines strategies for executing workloads.

- **Input Structs:**
  - `Process`: Manages individual processes with unique identifiers.
  - `ProcessConfig`: External configuration for processes.
  - `Job`: Represents tasks to be executed within a process pool.

- **Output Structs:**
  - `ProcessResult`: Captures the outcome of process executions.

- **Error Enums:**
  - `ProcessError`: Detailed error types for process management.
  - `ThreadError`: Error types related to thread operations.

- **Additional Components:**
  - `Signal`: Abstracts platform-specific signals.
  - `ExecutionStrategy`: Enumerates different execution strategies for
    workloads.

These components collectively provide a powerful and flexible foundation for
managing external processes in Rust, mirroring the elegance and functionality of
Laravel's `Process` facade. The design emphasizes type safety, error handling,
and asynchronous operations, leveraging Rust's strengths to deliver a reliable
and efficient process management library.

# Process Facade Code Review & Improvements

Let's analyze this process management library and suggest improvements while
maintaining its core strengths. I'll organize this review by key aspects,
explaining both what works well and what could be enhanced.

## Architectural Strengths

The codebase demonstrates several excellent architectural decisions:

1. **Clear Separation of Concerns**: The code cleanly separates process
   management, configuration, error handling, and execution strategies. This
   makes the system both maintainable and extensible.

2. **Comprehensive Error Handling**: The error system uses thiserror
   effectively, providing detailed context for different failure modes. This
   helps with debugging and proper error recovery.

3. **Async-First Design**: The pervasive use of async/await and tokio shows
   modern Rust best practices for handling concurrent operations.

## Suggested Improvements

### 1. Enhanced Process Configuration

The current ProcessConfig could be more flexible. Here's an improved version:

```rust
use std::time::Duration;

#[derive(Debug, Deserialize, Clone)]
pub struct ProcessConfig {
    pub working_dir: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub input: Option<String>,
    #[serde(with = "duration_serde")] // Custom serde module for Duration
    pub timeout: Option<Duration>,    // Changed from u64 to Duration
    #[serde(with = "duration_serde")]
    pub idle_timeout: Option<Duration>,
    pub tty: Option<bool>,
    // New fields
    pub priority: Option<i32>,        // Process priority/nice value
    pub resource_limits: Option<ResourceLimits>,
    pub retry_policy: Option<RetryPolicy>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResourceLimits {
    pub max_memory: Option<usize>,    // Maximum memory in bytes
    pub max_cpu_time: Option<Duration>,
    pub max_file_size: Option<u64>,   // Maximum file size in bytes
    pub max_open_files: Option<u32>,  // Maximum number of open file descriptors
}

#[derive(Debug, Deserialize, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_initial: Duration,
    pub backoff_multiplier: f32,
    pub backoff_max: Duration,
}
```

This enhancement provides:

- More precise timeout handling using Duration
- Process resource management capabilities
- Automated retry handling for failed processes

### 2. Improved Process Monitoring

The current monitoring system could be enhanced with metrics collection:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

pub struct ProcessMetrics {
    start_time: Instant,
    cpu_time: AtomicU64,
    memory_usage: AtomicU64,
    io_operations: AtomicU64,
}

impl Process {
    pub async fn get_metrics(&self) -> Result<ProcessMetrics, ProcessError> {
        // Implementation for collecting process metrics
        // This would use platform-specific APIs (e.g., procfs on Linux)
        // to gather detailed process statistics
    }
    
    async fn monitor_resources(&self) -> Result<(), ProcessError> {
        let metrics = self.get_metrics().await?;
        
        // Check against resource limits
        if let Some(limits) = &self.options.resource_limits {
            if let (Some(max_memory), Some(current_memory)) = (limits.max_memory, metrics.memory_usage) {
                if current_memory > max_memory {
                    return Err(ProcessError::ResourceLimitExceeded("memory"));
                }
            }
            // Similar checks for other resources
        }
        
        Ok(())
    }
}
```

### 3. Enhanced Error Handling

Let's expand the error system to provide more context:

```rust
#[derive(Error, Debug)]
pub enum ProcessError {
    // Existing variants...
    
    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(&'static str),
    
    #[error("Process {0} failed after {1} retry attempts")]
    RetryLimitExceeded(String, u32),
    
    #[error("Process initialization failed: {0}")]
    InitializationError(String),
    
    #[error("Invalid process state transition from {from} to {to}")]
    InvalidStateTransition {
        from: ProcessState,
        to: ProcessState,
    },
}
```

### 4. Process State Management

Adding explicit state management improves reliability:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Initial,
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

impl Process {
    fn transition_state(&mut self, new_state: ProcessState) -> Result<(), ProcessError> {
        let current_state = self.state;
        
        // Validate state transition
        match (current_state, new_state) {
            (ProcessState::Initial, ProcessState::Starting) => Ok(()),
            (ProcessState::Starting, ProcessState::Running) => Ok(()),
            (ProcessState::Running, ProcessState::Stopping) => Ok(()),
            (ProcessState::Stopping, ProcessState::Stopped) => Ok(()),
            (current, new) => Err(ProcessError::InvalidStateTransition {
                from: current,
                to: new,
            }),
        }?;
        
        self.state = new_state;
        Ok(())
    }
}
```

### 5. Enhanced Workload Management

The Workload trait could be improved with better resource management:

```rust
#[async_trait]
pub trait Workload: Send + Debug {
    type Output: Send;
    
    async fn execute(&self) -> Result<Self::Output, ProcessError>;
    
    fn estimated_cost(&self) -> WorkloadCost;
    
    fn requirements(&self) -> WorkloadRequirements;
    
    // New methods
    async fn prepare(&self) -> Result<(), ProcessError> {
        Ok(()) // Default implementation
    }
    
    async fn cleanup(&self) -> Result<(), ProcessError> {
        Ok(()) // Default implementation
    }
    
    fn priority(&self) -> i32 {
        0 // Default priority
    }
}
```

## Implementation Notes

When implementing these improvements, consider:

1. **Backward Compatibility**: The enhanced features should be optional,
   allowing existing code to work without modification.

2. **Platform Specifics**: Resource monitoring and limit enforcement might need
   platform-specific implementations (Windows/Unix).

3. **Performance Impact**: The additional monitoring and state management should
   have minimal overhead on process execution.

4. **Error Recovery**: The retry system should be intelligent enough to
   distinguish between retryable and non-retryable errors.

## Security Considerations

The improvements should maintain or enhance security:

1. **Resource Limits**: Prevent resource exhaustion attacks by enforcing limits
   early.

2. **Environment Sanitization**: Carefully validate and sanitize environment
   variables.

3. **Working Directory Safety**: Validate working directory paths against a
   whitelist of allowed locations.

4. **Signal Handling**: Ensure signal handling cannot be used for privilege
   escalation.

## Testing Recommendations

These improvements should be accompanied by comprehensive tests:

1. **State Transition Tests**: Verify all valid state transitions and ensure
   invalid transitions are properly rejected.

2. **Resource Monitoring Tests**: Verify resource limits are properly enforced.

3. **Retry Logic Tests**: Ensure retry policies work correctly with different
   error conditions.

4. **Platform-Specific Tests**: Verify behavior across different operating
   systems.

## Conclusion

These improvements enhance the robustness and functionality of the process
management system while maintaining its clean architecture. The additions focus
on:

- More precise process control and monitoring
- Better error handling and recovery
- Explicit state management
- Enhanced resource control
- Improved security

Implementation should be gradual, with careful testing at each stage to ensure
stability and backward compatibility.

Remember to document all new features thoroughly and provide migration guides
for users updating from previous versions.
