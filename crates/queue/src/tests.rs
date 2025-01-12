use super::*;
use std::time::Duration as StdDuration;
use tokio::time::sleep;

#[tokio::test]
async fn test_job_priority() -> Result<()> {
    let queue = SledQueueImpl::new("test_priority".to_string(), "test_priority.db".to_string())?;
    queue.clear().await?;

    let job1 = Job::new("1".into(), "job1".into()).with_priority(0);
    let job2 = Job::new("2".into(), "job2".into()).with_priority(1);
    let job3 = Job::new("3".into(), "job3".into()).with_priority(-1);

    queue.push(Box::new(job1.clone())).await?;
    queue.push(Box::new(job2.clone())).await?;
    queue.push(Box::new(job3.clone())).await?;

    let job1 = queue.pop().await?.unwrap();
    let job2 = queue.pop().await?.unwrap();
    let job3 = queue.pop().await?.unwrap();

    let popped1 = job1.as_any().downcast_ref::<Job>().unwrap();
    let popped2 = job2.as_any().downcast_ref::<Job>().unwrap();
    let popped3 = job3.as_any().downcast_ref::<Job>().unwrap();

    assert_eq!(popped1.id, "2"); // Highest priority
    assert_eq!(popped2.id, "1"); // Medium priority
    assert_eq!(popped3.id, "3"); // Lowest priority

    Ok(())
}

#[tokio::test]
async fn test_delayed_jobs() -> Result<()> {
    let queue = SledQueueImpl::new("test_delayed".to_string(), "test_delayed.db".to_string())?;
    queue.clear().await?;

    let job = Job::new("delayed".into(), "test".into())
        .with_delay(Duration::seconds(2));

    queue.push(Box::new(job)).await?;

    assert!(queue.pop().await?.is_none());

    sleep(StdDuration::from_secs(2)).await;

    assert!(queue.pop().await?.is_some());

    Ok(())
}

#[tokio::test]
async fn test_bulk_push() -> Result<()> {
    let queue = SledQueueImpl::new("test_bulk".to_string(), "test_bulk.db".to_string())?;
    queue.clear().await?;

    let jobs: Vec<Box<dyn JobHandler>> = vec![
        Box::new(Job::new("1".into(), "job1".into())),
        Box::new(Job::new("2".into(), "job2".into())),
        Box::new(Job::new("3".into(), "job3".into())),
    ];

    queue.push_bulk(jobs).await?;

    assert_eq!(queue.size().await?, 3);

    Ok(())
}

#[tokio::test]
async fn test_job_tags() -> Result<()> {
    let queue = SledQueueImpl::new("test_tags".to_string(), "test_tags.db".to_string())?;
    queue.clear().await?;

    let job1 = Job::new("1".into(), "job1".into())
        .with_tags(vec!["tag1".into(), "tag2".into()]);
    let job2 = Job::new("2".into(), "job2".into())
        .with_tags(vec!["tag2".into()]);

    queue.push(Box::new(job1)).await?;
    queue.push(Box::new(job2)).await?;

    let tag1_jobs = queue.get_by_tag("tag1").await?;
    let tag2_jobs = queue.get_by_tag("tag2").await?;

    assert_eq!(tag1_jobs.len(), 1);
    assert_eq!(tag2_jobs.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_retry_with_backoff() -> Result<()> {
    let mut queue = SledQueueImpl::new("test".to_string(), "test.db".to_string())?;
    queue.clear().await?;

    let job_id = "test_job".to_string();
    let job = Job::new(job_id.clone(), "test payload".to_string());

    // Push the job to the queue
    queue.push(Box::new(job)).await?;

    // Pop the job and verify it's the one we pushed
    let popped = queue.pop().await?.unwrap();
    assert_eq!(popped.id(), job_id);

    // Fail the job with an error message
    queue.fail(&job_id, "test error".to_string()).await?;

    // Wait for backoff (2^1 = 2 seconds for first failure)
    sleep(StdDuration::from_secs(4)).await;

    // Job should be available again
    let popped = queue.pop().await?.unwrap();
    assert_eq!(popped.id(), job_id);

    Ok(())
}
