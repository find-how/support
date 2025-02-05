# Queue

A Laravel-inspired job queuing and processing system with sled backend.

## Features

- Async job processing with tokio
- Persistent storage using sled
- Job prioritization
- Delayed job execution
- Job tagging and filtering
- Retry with exponential backoff
- Bulk operations
- Comprehensive error handling

## Usage

```rust
use queue::{Job, SledQueueImpl, Queue};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new queue
    let queue = SledQueueImpl::new("my_queue".to_string(), "queue.db".to_string())?;

    // Create and push a job
    let job = Job::new("job1".to_string(), "payload".to_string())
        .with_priority(1)
        .with_tags(vec!["tag1".to_string(), "tag2".to_string()]);

    queue.push(Box::new(job)).await?;

    // Process jobs
    while let Some(job) = queue.pop().await? {
        match job.handle().await {
            Ok(_) => println!("Job completed successfully"),
            Err(e) => {
                println!("Job failed: {}", e);
                queue.fail(&job.id(), e.to_string()).await?;
            }
        }
    }

    Ok(())
}
```

## Job Features

- **Priority**: Jobs can be assigned priorities to control processing order
- **Delay**: Jobs can be scheduled to run after a delay
- **Tags**: Jobs can be tagged for filtering and organization
- **Retry**: Failed jobs are automatically retried with exponential backoff
- **Bulk Operations**: Multiple jobs can be pushed efficiently

## Storage

The queue uses sled as its storage backend, providing:

- Persistence across restarts
- ACID transactions
- High performance
- Durability guarantees

## Error Handling

Comprehensive error handling with custom error types:

- Storage errors
- Serialization errors
- Job-specific errors
- Queue operation errors

## Testing

The crate includes extensive tests:

```bash
cargo test
```

## Examples

See the `examples` directory for more usage examples.
