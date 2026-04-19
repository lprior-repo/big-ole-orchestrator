use std::time::Duration;

use vo_scheduler::job::ScheduledJob;
use vo_scheduler::queue::SchedulerQueue;
use vo_scheduler::types::{
    JobId, JobKind, JobPriority, JobState, RetryPolicy, RetryPolicyError, SchedulePolicy,
};

#[test]
fn retry_policy_rejects_zero_max_attempts() {
    let err =
        RetryPolicy::try_new(0, 2.0, Duration::from_secs(1), Duration::from_secs(300)).unwrap_err();
    assert_eq!(err, RetryPolicyError::MaxAttemptsZero);
}

#[test]
fn retry_policy_rejects_backoff_below_one() {
    let err =
        RetryPolicy::try_new(3, 0.5, Duration::from_secs(1), Duration::from_secs(300)).unwrap_err();
    match err {
        RetryPolicyError::BackoffMultiplierBelowOne { value } => assert_eq!(value, 0.5),
        other => panic!("expected BackoffMultiplierBelowOne, got {other:?}"),
    }
}

#[test]
fn exponential_backoff_caps_at_max_delay() {
    let policy =
        RetryPolicy::try_new(5, 2.0, Duration::from_secs(1), Duration::from_secs(10)).unwrap();
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
    )
    .unwrap();
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
    )
    .unwrap();
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
    )
    .unwrap();
    assert!(matches!(job.schedule_policy, SchedulePolicy::Cron(ref s) if s == "*/5 * * * *"));
    assert_eq!(job.kind, JobKind::Recurring);
}

#[test]
fn cron_invalid_expression_rejected() {
    let result = ScheduledJob::new(
        JobKind::Recurring,
        JobPriority::Normal,
        SchedulePolicy::Cron("invalid".to_string()),
        RetryPolicy::default_policy(),
        bytes::Bytes::new(),
    );
    assert!(result.is_err());
}

#[test]
fn queue_pops_highest_priority_first() {
    let mut q = SchedulerQueue::new(10);
    let due = chrono::Utc::now();
    q.insert(make_job(JobPriority::Low, due, "low")).unwrap();
    q.insert(make_job(JobPriority::Critical, due, "crit"))
        .unwrap();
    q.insert(make_job(JobPriority::High, due, "high")).unwrap();
    assert_eq!(q.pop_due(due).unwrap().payload, b"crit"[..]);
    assert_eq!(q.pop_due(due).unwrap().payload, b"high"[..]);
    assert_eq!(q.pop_due(due).unwrap().payload, b"low"[..]);
}

#[test]
fn retry_loop_cycles_through_retrying_state() {
    let mut job = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::High,
        SchedulePolicy::Immediate,
        RetryPolicy::default_policy(),
        vec![].into(),
    )
    .unwrap();
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

#[test]
fn full_lifecycle_pending_to_completed() {
    let mut job = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::Immediate,
        RetryPolicy::default_policy(),
        vec![].into(),
    )
    .unwrap();
    assert_eq!(job.state, JobState::Pending);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    assert!(job.state.is_terminal());
}

fn make_job(
    priority: JobPriority,
    due_at: chrono::DateTime<chrono::Utc>,
    tag: &str,
) -> ScheduledJob {
    let mut job = ScheduledJob::new(
        JobKind::OneShot,
        priority,
        SchedulePolicy::At(due_at),
        RetryPolicy::default_policy(),
        tag.as_bytes().to_vec().into(),
    )
    .unwrap();
    job.due_at = due_at;
    job
}

#[test]
fn scheduler_error_is_transient_for_queue_full_and_serialization() {
    use vo_scheduler::error::SchedulerError;
    assert!(SchedulerError::QueueFull.is_transient());
    assert!(SchedulerError::SerializationError("boom".into()).is_transient());
    assert!(!SchedulerError::JobNotFound.is_transient());
    assert!(!SchedulerError::InvalidSchedule.is_transient());
    assert!(!SchedulerError::InvalidTransition.is_transient());
}

#[test]
fn scheduler_error_is_permanent_for_invalid_and_transition() {
    use vo_scheduler::error::SchedulerError;
    assert!(SchedulerError::InvalidSchedule.is_permanent());
    assert!(SchedulerError::InvalidTransition.is_permanent());
    assert!(!SchedulerError::QueueFull.is_permanent());
    assert!(!SchedulerError::JobNotFound.is_permanent());
    assert!(!SchedulerError::SerializationError("x".into()).is_permanent());
}

#[test]
fn execution_error_is_retryable_only_for_resource_exhausted() {
    use vo_scheduler::error::ExecutionError;
    assert!(ExecutionError::ResourceExhausted.is_retryable());
    assert!(!ExecutionError::Panicked.is_retryable());
    assert!(!ExecutionError::TimedOut.is_retryable());
    assert!(!ExecutionError::Cancelled.is_retryable());
}

#[test]
fn execution_error_is_transient_only_for_resource_exhausted() {
    use vo_scheduler::error::ExecutionError;
    assert!(ExecutionError::ResourceExhausted.is_transient());
    assert!(!ExecutionError::Panicked.is_transient());
    assert!(!ExecutionError::TimedOut.is_transient());
    assert!(!ExecutionError::Cancelled.is_transient());
}

#[test]
fn job_state_is_terminal_for_completed_failed_cancelled() {
    assert!(JobState::Completed.is_terminal());
    assert!(JobState::Failed.is_terminal());
    assert!(JobState::Cancelled.is_terminal());
    assert!(!JobState::Scheduled.is_terminal());
    assert!(!JobState::Pending.is_terminal());
    assert!(!JobState::Running.is_terminal());
    assert!(!JobState::Retrying.is_terminal());
}

#[test]
fn job_state_is_non_terminal_opposite_of_terminal() {
    assert!(!JobState::Completed.is_non_terminal());
    assert!(!JobState::Failed.is_non_terminal());
    assert!(!JobState::Cancelled.is_non_terminal());
    assert!(JobState::Scheduled.is_non_terminal());
    assert!(JobState::Pending.is_non_terminal());
    assert!(JobState::Running.is_non_terminal());
    assert!(JobState::Retrying.is_non_terminal());
}

#[test]
fn job_id_generate_produces_unique_ids() {
    let ids: std::collections::HashSet<_> = (0..1000).map(|_| JobId::generate()).collect();
    assert_eq!(ids.len(), 1000, "JobId::generate() must produce unique IDs");
}

#[test]
fn schedule_policy_validate_cron_accepts_valid_expressions() {
    SchedulePolicy::validate_cron("* * * * *").unwrap();
    SchedulePolicy::validate_cron("0 * * * *").unwrap();
    SchedulePolicy::validate_cron("*/5 * * * *").unwrap();
    SchedulePolicy::validate_cron("0-59 * * * *").unwrap();
    SchedulePolicy::validate_cron("0 0 * * *").unwrap();
    SchedulePolicy::validate_cron("0 0 1 * *").unwrap();
    SchedulePolicy::validate_cron("0 0 1 1 *").unwrap();
    SchedulePolicy::validate_cron("0 0 * * 0").unwrap();
}

#[test]
fn schedule_policy_validate_cron_rejects_invalid_expressions() {
    use vo_scheduler::error::SchedulerError;
    assert!(matches!(
        SchedulePolicy::validate_cron("invalid"),
        Err(SchedulerError::InvalidSchedule)
    ));
    assert!(matches!(
        SchedulePolicy::validate_cron("60 * * * *"),
        Err(SchedulerError::InvalidSchedule)
    ));
    assert!(matches!(
        SchedulePolicy::validate_cron("* 24 * * *"),
        Err(SchedulerError::InvalidSchedule)
    ));
    assert!(matches!(
        SchedulePolicy::validate_cron("* * 32 * *"),
        Err(SchedulerError::InvalidSchedule)
    ));
    assert!(matches!(
        SchedulePolicy::validate_cron("* * * 13 *"),
        Err(SchedulerError::InvalidSchedule)
    ));
    assert!(matches!(
        SchedulePolicy::validate_cron("* * * * 7"),
        Err(SchedulerError::InvalidSchedule)
    ));
    assert!(matches!(
        SchedulePolicy::validate_cron("*/0 * * * *"),
        Err(SchedulerError::InvalidSchedule)
    ));
    assert!(matches!(
        SchedulePolicy::validate_cron("5-3 * * * *"),
        Err(SchedulerError::InvalidSchedule)
    ));
}

#[test]
fn job_display_impl_formatters_work() {
    assert_eq!(format!("{}", JobKind::OneShot), "one_shot");
    assert_eq!(format!("{}", JobKind::Recurring), "recurring");
    assert_eq!(format!("{}", JobKind::Delayed), "delayed");
    assert_eq!(format!("{}", JobState::Pending), "pending");
    assert_eq!(format!("{}", JobState::Running), "running");
    assert_eq!(format!("{}", JobState::Completed), "completed");
    assert_eq!(format!("{}", JobState::Failed), "failed");
    assert_eq!(format!("{}", JobState::Cancelled), "cancelled");
    assert_eq!(format!("{}", JobState::Retrying), "retrying");
    assert_eq!(format!("{}", JobState::Scheduled), "scheduled");
    assert_eq!(format!("{}", JobPriority::Critical), "critical");
    assert_eq!(format!("{}", JobPriority::High), "high");
    assert_eq!(format!("{}", JobPriority::Normal), "normal");
    assert_eq!(format!("{}", JobPriority::Low), "low");
    assert_eq!(format!("{}", JobPriority::Background), "background");
}

#[test]
fn queue_lookup_mut_returns_mutable_reference() {
    let mut q = SchedulerQueue::new(10);
    let job = make_job(JobPriority::Normal, chrono::Utc::now(), "test");
    let id = job.id;
    q.insert(job).unwrap();
    let retrieved = q.lookup_mut(&id).unwrap();
    assert_eq!(retrieved.id, id);
    retrieved.attempt_count = 5;
    let viewed = q.lookup(&id).unwrap();
    assert_eq!(viewed.attempt_count, 5);
}

#[test]
fn queue_get_state_returns_state_for_existing_job() {
    let mut q = SchedulerQueue::new(10);
    let job = make_job(JobPriority::Normal, chrono::Utc::now(), "test");
    let id = job.id;
    q.insert(job).unwrap();
    assert_eq!(q.get_state(&id), Some(JobState::Pending));
    q.update_state(&id, JobState::Running).unwrap();
    assert_eq!(q.get_state(&id), Some(JobState::Running));
}

#[test]
fn queue_get_state_returns_none_for_missing_job() {
    let q = SchedulerQueue::new(10);
    assert_eq!(q.get_state(&JobId::generate()), None);
}
