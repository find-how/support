
// ============================
// ==== CLI Implementation =====
// ============================

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

// ============================
// ==== Main Function =========
// ============================

#[tokio::main]
async fn main() -> sled::Result<()> {
    // Initialize logger
    env_logger::init();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize sled databases
    let db = sled::open("queue_db")?;
    let failed_db = sled::open("failed_jobs_db")?;

    // Initialize QueueManager
    let queue_manager = Arc::new(QueueManager::new(db, failed_db)?);

    // Initialize Event Dispatcher
    let dispatcher = Arc::new(Dispatcher::new());

    // Initialize Failed Job Repository
    let failed_repo = Arc::new(SledFailedJobRepository::new(queue_manager.failed_db.clone()));

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
    let logging_handler = Arc::new(LoggingEventHandler);
    dispatcher.register::<JobProcessingEvent>(logging_handler.clone()).await;
    dispatcher.register::<JobProcessedEvent>(logging_handler.clone()).await;
    dispatcher.register::<JobFailedEvent>(logging_handler.clone()).await;

    // Setup Queue Connections
    let sled_queue = Arc::new(SledQueueImpl::new(queue_manager.db.clone(), "default"));
    queue_manager
        .add_connection("default", sled_queue.clone())
        .await;

    // Handle CLI Commands
    match cli.command {
        Commands::Enqueue {
            queue,
            payload,
            max_attempts,
        } => {
            let job = Job::new(payload, max_attempts);
            queue_manager.enqueue(&queue, job).await?;
            println!("Job enqueued successfully.");
        }
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
        }
        Commands::Size { queue } => {
            let size = queue_manager.size(&queue).await;
            println!("Queue '{}' has {} jobs.", queue, size);
        }
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
        }
        Commands::Retry { id } => {
            match failed_repo.retry(&id).await? {
                Some(job) => {
                    // Re-enqueue the job
                    let queue = queue_manager.connection(&job.queue).await.unwrap();
                    queue_manager.enqueue(&job.queue, job).await?;
                    println!("Job {} has been retried.", id);
                }
                None => {
                    println!("Failed job with ID {} not found.", id);
                }
            }
        }
    }

    Ok(())
}
