use std::time::Duration;

use vo_scheduler::types::{JobKind, JobPriority, JobState, RetryPolicy, RetryPolicyError, SchedulePolicy};
use vo_scheduler::job::ScheduledJob;
use vo_scheduler::queue::SchedulerQueue;

#[test]
fn retry_policy_rejects_zero_max_attempts() {
    let err = RetryPolicy::try_new(0, 2.0, Duration::from_secs(1), Duration::from_secs(300)).unwrap_err();
    assert_eq!(err, RetryPolicyError::MaxAttemptsZero);
}

#[test]
fn retry_policy_rejects_backoff_below_one() {
    let err = RetryPolicy::try_new(3, 0.5, Duration::from_secs(1), Duration::from_secs(300)).unwrap_err();
    match err {
        RetryPolicyError::BackoffMultiplierBelowOne { value } => assert_eq!(value, 0.5),
        other => panic!("expected BackoffMultiplierBelowOne, got {other:?}"),
    }
}

#[test]
fn exponential_backoff_caps_at_max_delay() {
    let policy = RetryPolicy::try_new(5, 2.0, Duration::from_secs(1), Duration::from_secs(10)).unwrap();
    assert_eq!(policy.compute_backoff(0), Duration::from_secs(1));
    assert_eq!(policy.compute_backoff(1), Duration::from_secs(2));
    assert_eq!(policy.compute_backoff(3), Duration::from_secs(8));
    assert_eq!(policy.compute_backoff(4), Duration::from_secs(10)); // capped
    assert_eq!(policy.compute_backoff(100), Duration::from_secs(10));
}

#[test]
fn can_retry_respects_max_attempts() {
    let policy = RetryPolicy::default_policy();
    assert!(policy.can_retry(0));
    assert!(policy.can_retry(2));
    assert!(!policy.can_retry(3));
}

#[test]
fn immediate_job_starts_as_pending() {
    let job = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::Immediate,
        RetryPolicy::default_policy(),
        vec![].into(),
    );
    assert_eq!(job.state, JobState::Pending);
}

#[test]
fn future_scheduled_job_starts_as_scheduled() {
    let future = chrono::Utc::now() + chrono::Duration::hours(1);
    let job = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::At(future),
        RetryPolicy::default_policy(),
        vec![].into(),
    );
    assert_eq!(job.state, JobState::Scheduled);
}

#[test]
fn cron_schedule_accepted_and_stored() {
    let job = ScheduledJob::new(
        JobKind::Recurring,
        JobPriority::Background,
        SchedulePolicy::Cron("*/5 * * * *".into()),
        RetryPolicy::default_policy(),
        vec![].into(),
    );
    assert!(matches!(job.schedule_policy, SchedulePolicy::Cron(ref s) if s == "*/5 * * * *"));
    assert_eq!(job.kind, JobKind::Recurring);
}

#[test]
fn queue_pops_highest_priority_first() {
    let mut q = SchedulerQueue::new(10);
    let due = chrono::Utc::now();
    q.insert(make_job(JobPriority::Low, due, "low")).unwrap();
    q.insert(make_job(JobPriority::Critical, due, "crit")).unwrap();
    q.insert(make_job(JobPriority::High, due, "high")).unwrap();
    assert_eq!(q.pop_due(due).unwrap().payload, b"crit"[..]);
    assert_eq!(q.pop_due(due).unwrap().payload, b"high"[..]);
    assert_eq!(q.pop_due(due).unwrap().payload, b"low"[..]);
}

#[test]
fn full_lifecycle_pending_to_completed() {
    let mut job = ScheduledJob::new(
        JobKind::OneShot, JobPriority::Normal,
        SchedulePolicy::Immediate,
        RetryPolicy::default_policy(),
        vec![].into(),
    );
    assert_eq!(job.state, JobState::Pending);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    assert!(job.state.is_terminal());
}

#[test]
fn retry_loop_cycles_through_retrying_state() {
    let mut job = ScheduledJob::new(
        JobKind::OneShot, JobPriority::High,
        SchedulePolicy::Immediate,
        RetryPolicy::default_policy(),
        vec![].into(),
    );
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();
    assert!(job.state.is_terminal());
    job.transition(JobState::Retrying).unwrap();
    assert!(!job.state.is_terminal());
    job.transition(JobState::Pending).unwrap();
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    assert!(job.state.is_terminal());
}

fn make_job(priority: JobPriority, due_at: chrono::DateTime<chrono::Utc>, tag: &str) -> ScheduledJob {
    let mut job = ScheduledJob::new(
        JobKind::OneShot, priority,
        SchedulePolicy::At(due_at),
        RetryPolicy::default_policy(),
        tag.as_bytes().to_vec().into(),
    );
    job.due_at = due_at;
    job
}
