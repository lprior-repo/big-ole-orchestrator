//! Load performance failing tests (TDD-RED phase)
//!
//! Test categories for scheduler load performance:
//! - LP-01: Concurrent job scheduling throughput
//! - LP-02: Priority queue under contention
//! - LP-03: Scheduler poll performance at scale

use std::time::{Duration, Instant};
use vo_executor::scheduler::PriorityQueue;
use vo_executor::scheduler::{Job, JobId, JobPriority, Schedule, Scheduler, SchedulerConfig};

#[test]
fn lp01_scheduler_handles_1000_schedules() {
    let config = SchedulerConfig {
        max_concurrent: 100,
        scan_interval: Duration::from_millis(10),
        max_jobs_per_scan: 1000,
    };
    let mut scheduler = Scheduler::new(config);

    let start = Instant::now();
    for i in 0..1000u64 {
        let job = Job::new(
            JobId::new(i),
            format!("payload-{}", i),
            Schedule::one_shot(Duration::from_secs(60)),
        );
        assert!(
            scheduler.schedule(job).is_ok(),
            "Schedule {} should succeed",
            i
        );
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(500),
        "Scheduling 1000 jobs should complete in < 500ms, took {:?}",
        elapsed
    );
    assert_eq!(scheduler.len(), 1000, "Scheduler should contain 1000 jobs");
}

#[test]
fn lp02_priority_queue_due_jobs_in_priority_order() {
    let mut pq = PriorityQueue::new();
    let now = 1000u64;

    for i in 0..100u64 {
        let job = Job::new(
            JobId::new(i),
            format!("job-{}", i),
            Schedule::one_shot(Duration::from_secs(0)),
        )
        .with_priority(if i % 4 == 0 {
            JobPriority::Critical
        } else if i % 4 == 1 {
            JobPriority::High
        } else if i % 4 == 2 {
            JobPriority::Normal
        } else {
            JobPriority::Low
        });
        pq.push(job, now - 50);
    }

    let due = pq.due_jobs(now, 100);
    assert_eq!(due.len(), 100, "All 100 jobs should be due");

    let mut prev_priority = JobPriority::Critical;
    for (job, _) in &due {
        assert!(
            job.priority >= prev_priority,
            "Jobs should be sorted by priority (critical first)"
        );
        prev_priority = job.priority;
    }
}

#[test]
fn lp03_poll_at_scale_returns_due_jobs_efficiently() {
    let config = SchedulerConfig {
        max_concurrent: 100,
        scan_interval: Duration::from_millis(10),
        max_jobs_per_scan: 100,
    };
    let mut scheduler = Scheduler::new(config);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);

    for i in 0..10000u64 {
        let job = Job::new(
            JobId::new(i),
            format!("job-{}", i),
            Schedule::one_shot(Duration::from_millis(0)),
        );
        let _ = scheduler.schedule(job);
    }

    let start = Instant::now();
    let due = scheduler.poll_due_jobs(now_ms + 1000);
    let elapsed = start.elapsed();

    assert!(
        !due.is_empty(),
        "Should return some due jobs from 10000 scheduled"
    );
    assert!(
        elapsed < Duration::from_millis(200),
        "poll_due_jobs should complete in < 200ms at scale, took {:?}",
        elapsed
    );
}
