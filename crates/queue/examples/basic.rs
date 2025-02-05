use std::time::Duration;
use tokio::time::sleep;
use queue::{Job, SledQueueImpl, Queue, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let queue = SledQueueImpl::new("test_queue".to_string(), "test.db".to_string())?;

    // Producer task
    let queue_clone = queue.clone();
    tokio::spawn(async move {
        loop {
            let job = Job::new("test_job".to_string(), "test_payload".to_string());
            if let Err(e) = queue_clone.push(Box::new(job)).await {
                eprintln!("Failed to push job: {}", e);
            }
            sleep(Duration::from_secs(1)).await;
        }
    });

    // Consumer task
    loop {
        match queue.pop().await {
            Ok(Some(job)) => {
                println!("Processing job: {}", job.id());
                if let Err(e) = job.handle().await {
                    eprintln!("Failed to handle job: {}", e);
                }
            }
            Ok(None) => {
                sleep(Duration::from_secs(1)).await;
            }
            Err(e) => {
                eprintln!("Failed to pop job: {}", e);
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
