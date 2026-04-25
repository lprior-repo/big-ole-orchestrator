use std::time::Duration;

use super::common::prelude::*;

#[tokio::test]
async fn scheduler_e2e_job_scheduled_then_polled_and_executed() {
    let _guard = state_guard();
    let config = SchedulerConfig {
        max_concurrent: 10,
        scan_interval: Duration::from_millis(10),
        max_jobs_per_scan: 100,
    };
    let mut scheduler = Scheduler::new(config);

    let job = Job::new(
        JobId::new(1),
        "step-1".to_string(),
        Schedule::one_shot(Duration::from_millis(50)),
    );
    scheduler.schedule(job).expect("Schedule should succeed");

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);

    let due_jobs = scheduler.poll_due_jobs(now_ms + 100);
    assert_eq!(due_jobs.len(), 1, "Should have 1 due job");
    assert_eq!(due_jobs[0].id, JobId::new(1));
    assert_eq!(due_jobs[0].payload, "step-1", "Job payload is step name");
}

#[tokio::test]
async fn scheduler_e2e_multiple_jobs_with_different_priorities() {
    let _guard = state_guard();
    let config = SchedulerConfig::default();
    let mut scheduler = Scheduler::new(config);

    let job_critical = Job::new(
        JobId::new(1),
        "step-1".to_string(),
        Schedule::one_shot(Duration::from_millis(50)),
    )
    .with_priority(JobPriority::Critical);

    let job_low = Job::new(
        JobId::new(2),
        "step-good".to_string(),
        Schedule::one_shot(Duration::from_millis(50)),
    )
    .with_priority(JobPriority::Low);

    scheduler.schedule(job_low).unwrap();
    scheduler.schedule(job_critical).unwrap();

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);

    let due_jobs = scheduler.poll_due_jobs(now_ms + 100);
    assert_eq!(due_jobs.len(), 2);

    let critical_idx = due_jobs
        .iter()
        .position(|j| j.id == JobId::new(1))
        .expect("Critical job should be present");
    let low_idx = due_jobs
        .iter()
        .position(|j| j.id == JobId::new(2))
        .expect("Low job should be present");
    assert!(
        critical_idx < low_idx,
        "Critical job should come before Low (higher priority first)"
    );
}

#[tokio::test]
async fn scheduler_e2e_recurring_job_rescheduled_after_execution() {
    let _guard = state_guard();
    let config = SchedulerConfig::default();
    let mut scheduler = Scheduler::new(config);

    let job = Job::new(
        JobId::new(1),
        "step-1".to_string(),
        Schedule::interval(Duration::from_millis(100)),
    );
    scheduler.schedule(job).unwrap();

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);

    let due_jobs = scheduler.poll_due_jobs(now_ms + 200);
    assert_eq!(due_jobs.len(), 1, "First firing should be due");

    let job_id = due_jobs[0].id;
    if let Schedule::Interval { interval_ms } = &due_jobs[0].schedule {
        let next_fire = now_ms + 200 + interval_ms;
        scheduler.reschedule(due_jobs[0].clone(), next_fire);
    }

    let later_due = scheduler.poll_due_jobs(now_ms + 400);
    assert!(
        !later_due.is_empty(),
        "Rescheduled job should be due in next window"
    );
}

#[tokio::test]
async fn scheduler_e2e_cancel_removes_job_from_queue() {
    let _guard = state_guard();
    let config = SchedulerConfig::default();
    let mut scheduler = Scheduler::new(config);

    let job = Job::new(
        JobId::new(42),
        "step-1".to_string(),
        Schedule::one_shot(Duration::from_millis(50)),
    );
    scheduler.schedule(job).unwrap();

    let removed = scheduler.cancel(JobId::new(42));
    assert!(removed.is_some(), "Cancel should return the removed job");

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let due_jobs = scheduler.poll_due_jobs(now_ms + 100);
    assert!(due_jobs.is_empty(), "Cancelled job should not be due");
}

#[tokio::test]
async fn scheduler_e2e_concurrent_limit_enforced() {
    let _guard = state_guard();
    let config = SchedulerConfig {
        max_concurrent: 2,
        scan_interval: Duration::from_millis(10),
        max_jobs_per_scan: 100,
    };
    let scheduler = Scheduler::new(config);

    let permit1 = scheduler.try_acquire();
    let permit2 = scheduler.try_acquire();
    let permit3 = scheduler.try_acquire();

    assert!(permit1.is_some(), "First permit should succeed");
    assert!(permit2.is_some(), "Second permit should succeed");
    assert!(permit3.is_none(), "Third permit should fail (limit=2)");
}

#[tokio::test]
async fn scheduler_e2e_start_stop_lifecycle() {
    let _guard = state_guard();
    let config = SchedulerConfig::default();
    let mut scheduler = Scheduler::new(config);

    assert!(
        !scheduler.is_running(),
        "Scheduler should not be running initially"
    );

    scheduler.start();
    assert!(
        scheduler.is_running(),
        "Scheduler should be running after start"
    );

    scheduler.stop();
    assert!(
        !scheduler.is_running(),
        "Scheduler should not be running after stop"
    );
}
