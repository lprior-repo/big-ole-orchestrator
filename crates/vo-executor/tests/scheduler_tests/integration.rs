use std::time::Duration;
use vo_executor::scheduler::Scheduler;
use vo_executor::{Job, JobId, JobPriority, Schedule, SchedulerConfig};

#[tokio::test]
async fn scheduler_schedule_one_shot() {
    let config = SchedulerConfig::default();
    let mut scheduler = Scheduler::new(config);

    let job = Job::new(
        JobId::new(1),
        "test".to_string(),
        Schedule::one_shot(Duration::from_millis(50)),
    );
    let result = scheduler.schedule(job);
    assert!(result.is_ok(), "Schedule should succeed: {:?}", result);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);

    let due = scheduler.poll_due_jobs(now_ms + 100);
    assert_eq!(due.len(), 1, "Should have 1 job due");
    assert_eq!(due[0].id, JobId::new(1));
}

#[tokio::test]
async fn scheduler_schedule_multiple() {
    let config = SchedulerConfig::default();
    let mut scheduler = Scheduler::new(config);

    for i in 0..5 {
        let job = Job::new(
            JobId::new(i),
            format!("job-{}", i),
            Schedule::one_shot(Duration::from_millis(50)),
        );
        scheduler.schedule(job).unwrap();
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);

    let due = scheduler.poll_due_jobs(now_ms + 100);
    assert_eq!(due.len(), 5, "Should have 5 jobs due");
}

#[tokio::test]
async fn scheduler_cancel_existing() {
    let config = SchedulerConfig::default();
    let mut scheduler = Scheduler::new(config);

    let job = Job::new(
        JobId::new(1),
        "test".to_string(),
        Schedule::one_shot(Duration::from_millis(50)),
    );
    scheduler.schedule(job).unwrap();

    let removed = scheduler.cancel(JobId::new(1));
    assert!(removed.is_some(), "Cancel should return removed job");

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);

    let due = scheduler.poll_due_jobs(now_ms + 100);
    assert!(due.is_empty(), "Cancelled job should not be in due jobs");
}

#[tokio::test]
async fn scheduler_cancel_nonexistent() {
    let config = SchedulerConfig::default();
    let mut scheduler = Scheduler::new(config);

    let removed = scheduler.cancel(JobId::new(999));
    assert!(removed.is_none(), "Cancel non-existent should return None");
}

#[tokio::test]
async fn scheduler_poll_due_jobs_empty() {
    let config = SchedulerConfig::default();
    let mut scheduler = Scheduler::new(config);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);

    let due = scheduler.poll_due_jobs(now_ms);
    assert!(due.is_empty(), "Poll with nothing due should return empty");
}

#[tokio::test]
async fn scheduler_poll_due_jobs_respects_max() {
    let config = SchedulerConfig {
        max_concurrent: 10,
        scan_interval: Duration::from_millis(10),
        max_jobs_per_scan: 2,
    };
    let mut scheduler = Scheduler::new(config);

    for i in 0..5 {
        let job = Job::new(
            JobId::new(i),
            format!("job-{}", i),
            Schedule::one_shot(Duration::from_millis(10)),
        );
        scheduler.schedule(job).unwrap();
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);

    let due = scheduler.poll_due_jobs(now_ms + 100);
    assert!(due.len() <= 2, "Should respect max_jobs_per_scan=2");
}
