use crate::queue::TestQueue;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

pub struct QueueBenchConfig {
    pub num_producers: usize,
    pub num_consumers: usize,
    pub jobs_per_producer: usize,
    pub max_parallel_jobs: usize,
    pub test_duration: Duration,
}

impl Default for QueueBenchConfig {
    fn default() -> Self {
        Self {
            num_producers: 4,
            num_consumers: 4,
            jobs_per_producer: 1000,
            max_parallel_jobs: 100,
            test_duration: Duration::from_secs(10),
        }
    }
}

pub struct QueueBenchResults {
    pub total_jobs: usize,
    pub successful_jobs: usize,
    pub failed_jobs: usize,
    pub retried_jobs: usize,
    pub avg_latency: Duration,
}

pub async fn bench_queue<Q: TestQueue + Clone + 'static>(queue: Q, config: QueueBenchConfig) -> QueueBenchResults {
    let start = Instant::now();
    let total_jobs = Arc::new(AtomicUsize::new(0));
    let successful_jobs = Arc::new(AtomicUsize::new(0));
    let failed_jobs = Arc::new(AtomicUsize::new(0));
    let retried_jobs = Arc::new(AtomicUsize::new(0));
    let total_latency = Arc::new(AtomicUsize::new(0));

    // Producer tasks
    let mut producer_handles = Vec::new();
    for _ in 0..config.num_producers {
        let queue = queue.clone();
        let total_jobs = total_jobs.clone();
        let handle = tokio::spawn(async move {
            for _ in 0..config.jobs_per_producer {
                let payload = vec![0u8; 100];
                if queue.push(payload.into()).await {
                    total_jobs.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        producer_handles.push(handle);
    }

    // Consumer tasks
    let mut consumer_handles = Vec::new();
    for _ in 0..config.num_consumers {
        let queue = queue.clone();
        let successful_jobs = successful_jobs.clone();
        let failed_jobs = failed_jobs.clone();
        let retried_jobs = retried_jobs.clone();
        let total_latency = total_latency.clone();
        let handle = tokio::spawn(async move {
            while Instant::now().duration_since(start) < config.test_duration {
                if let Some((id, _)) = queue.pop().await {
                    let job_latency = Instant::now().duration_since(start).as_micros() as usize;
                    total_latency.fetch_add(job_latency, Ordering::SeqCst);

                    if rand::random::<f32>() < 0.1 {
                        // 10% chance of failure
                        if queue.fail(&id).await {
                            failed_jobs.fetch_add(1, Ordering::SeqCst);
                            if queue.retry(&id).await {
                                retried_jobs.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    } else {
                        if queue.complete(&id).await {
                            successful_jobs.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }
            }
        });
        consumer_handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in producer_handles {
        handle.await.unwrap();
    }
    for handle in consumer_handles {
        handle.await.unwrap();
    }

    let total = total_jobs.load(Ordering::SeqCst);
    QueueBenchResults {
        total_jobs: total,
        successful_jobs: successful_jobs.load(Ordering::SeqCst),
        failed_jobs: failed_jobs.load(Ordering::SeqCst),
        retried_jobs: retried_jobs.load(Ordering::SeqCst),
        avg_latency: if total > 0 {
            Duration::from_micros((total_latency.load(Ordering::SeqCst) / total) as u64)
        } else {
            Duration::from_secs(0)
        },
    }
}
