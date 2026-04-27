use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::error::SchedulerError;
use crate::job::SerializedPayload;
use crate::job::ScheduledJob;
use crate::types::{JobId, JobKind, JobPriority, JobState, RetryPolicy, SchedulePolicy};

fn make_job(priority: JobPriority, policy: SchedulePolicy) -> ScheduledJob {
    ScheduledJob::new(
        JobKind::OneShot,
        priority,
        policy,
        RetryPolicy::default(),
        bytes::Bytes::from_static(b"test-payload"),
    )
    .unwrap()
}

fn make_job_with_kind(
    kind: JobKind,
    priority: JobPriority,
    policy: SchedulePolicy,
) -> ScheduledJob {
    ScheduledJob::new(
        kind,
        priority,
        policy,
        RetryPolicy::default(),
        bytes::Bytes::from_static(b"test-payload"),
    )
    .unwrap()
}

fn future_time() -> DateTime<Utc> {
    Utc::now() + chrono::Duration::hours(1)
}

fn past_time() -> DateTime<Utc> {
    Utc::now() - chrono::Duration::hours(1)
}

// === ScheduledJob::new() tests ===

#[test]
fn new_immediate_job_has_pending_state() {
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    assert_eq!(job.state, JobState::Pending);
}

#[test]
fn new_scheduled_job_has_scheduled_state_for_future() {
    let job = make_job(JobPriority::Normal, SchedulePolicy::At(future_time()));
    assert_eq!(job.state, JobState::Scheduled);
}

#[test]
fn new_scheduled_job_has_scheduled_state_for_after() {
    let job = make_job(
        JobPriority::Normal,
        SchedulePolicy::After(Duration::from_secs(3600)),
    );
    assert_eq!(job.state, JobState::Scheduled);
}

#[test]
fn new_cron_job_has_pending_state() {
    let job = make_job(JobPriority::Normal, SchedulePolicy::Cron { expr: "0 * * * *".to_string() });
    assert_eq!(job.state, JobState::Pending);
}

#[test]
fn new_immediate_job_started_in_past_has_pending_state() {
    let job = make_job(JobPriority::Normal, SchedulePolicy::At(past_time()));
    assert_eq!(job.state, JobState::Pending);
}

#[test]
fn new_job_has_zero_attempt_count() {
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    assert_eq!(job.attempt_count, 0);
}

#[test]
fn new_job_has_none_last_error() {
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    assert!(job.last_error.is_none());
}

#[test]
fn new_job_generates_unique_id() {
    let job1 = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let job2 = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    assert_ne!(job1.id, job2.id);
}

#[test]
fn new_job_has_correct_kind() {
    let job = make_job_with_kind(JobKind::Delayed, JobPriority::Critical, SchedulePolicy::Immediate);
    assert_eq!(job.kind, JobKind::Delayed);
}

#[test]
fn new_job_preserves_priority() {
    let job = make_job(JobPriority::High, SchedulePolicy::Immediate);
    assert_eq!(job.priority, JobPriority::High);
}

#[test]
fn new_job_preserves_payload() {
    let payload = bytes::Bytes::from_static(b"custom-payload-data");
    let job = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::Immediate,
        RetryPolicy::default(),
        payload.clone(),
    )
    .unwrap();
    assert_eq!(job.payload, payload);
}

#[test]
fn new_job_with_cron_invalid_returns_error() {
    let result = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::Cron { expr: "invalid cron".to_string() },
        RetryPolicy::default(),
        bytes::Bytes::from_static(b"test"),
    );
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SchedulerError::InvalidSchedule));
}

#[test]
fn new_job_has_created_at_close_to_now() {
    let before = Utc::now();
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let after = Utc::now();
    assert!(job.created_at >= before);
    assert!(job.created_at <= after);
}

#[test]
fn new_job_updated_at_equals_created_at() {
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    assert_eq!(job.updated_at, job.created_at);
}

#[test]
fn new_after_schedule_with_zero_duration_is_immediate() {
    let job = make_job(
        JobPriority::Normal,
        SchedulePolicy::After(Duration::from_secs(0)),
    );
    assert_eq!(job.state, JobState::Pending);
}

// === ScheduledJob::transition() tests ===

#[test]
fn transition_scheduled_to_pending() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::At(future_time()));
    assert_eq!(job.state, JobState::Scheduled);
    job.transition(JobState::Pending).unwrap();
    assert_eq!(job.state, JobState::Pending);
}

#[test]
fn transition_scheduled_to_cancelled() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::At(future_time()));
    job.transition(JobState::Cancelled).unwrap();
    assert_eq!(job.state, JobState::Cancelled);
}

#[test]
fn transition_pending_to_running() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    assert_eq!(job.state, JobState::Running);
}

#[test]
fn transition_pending_to_cancelled() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Cancelled).unwrap();
    assert_eq!(job.state, JobState::Cancelled);
}

#[test]
fn transition_running_to_completed() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    assert_eq!(job.state, JobState::Completed);
}

#[test]
fn transition_running_to_failed() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();
    assert_eq!(job.state, JobState::Failed);
}

#[test]
fn transition_running_to_cancelled() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Cancelled).unwrap();
    assert_eq!(job.state, JobState::Cancelled);
}

#[test]
fn transition_failed_to_retrying() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();
    job.transition(JobState::Retrying).unwrap();
    assert_eq!(job.state, JobState::Retrying);
}

#[test]
fn transition_retrying_to_pending() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();
    job.transition(JobState::Retrying).unwrap();
    job.transition(JobState::Pending).unwrap();
    assert_eq!(job.state, JobState::Pending);
}

#[test]
fn transition_retrying_to_cancelled() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();
    job.transition(JobState::Retrying).unwrap();
    job.transition(JobState::Cancelled).unwrap();
    assert_eq!(job.state, JobState::Cancelled);
}

#[test]
fn transition_invalid_scheduled_to_running() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::At(future_time()));
    let result = job.transition(JobState::Running);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

#[test]
fn transition_invalid_scheduled_to_completed() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::At(future_time()));
    let result = job.transition(JobState::Completed);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

#[test]
fn transition_invalid_pending_to_completed() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let result = job.transition(JobState::Completed);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

#[test]
fn transition_invalid_running_to_pending() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    let result = job.transition(JobState::Pending);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

#[test]
fn transition_invalid_running_to_scheduled() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    let result = job.transition(JobState::Scheduled);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

#[test]
fn transition_invalid_completed_to_anything() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    for state in [
        JobState::Pending,
        JobState::Running,
        JobState::Failed,
        JobState::Retrying,
    ] {
        let result = job.transition(state);
        assert!(
            matches!(result, Err(SchedulerError::InvalidTransition)),
            "Completed -> {} should be invalid",
            state
        );
    }
}

#[test]
fn transition_completed_to_scheduled_is_one_shot_invalid() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    let result = job.transition(JobState::Scheduled);
    assert!(
        matches!(result, Err(SchedulerError::InvalidTransition)),
        "OneShot completed cannot go back to scheduled"
    );
}

#[test]
fn transition_completed_to_scheduled_is_recurring_valid() {
    let mut job = make_job_with_kind(JobKind::Recurring, JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    job.transition(JobState::Scheduled).unwrap();
    assert_eq!(job.state, JobState::Scheduled);
}

#[test]
fn transition_updates_updated_at() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let original_updated = job.updated_at;
    std::thread::sleep(Duration::from_millis(10));
    job.transition(JobState::Running).unwrap();
    assert!(job.updated_at > original_updated);
}

#[test]
fn transition_cancelled_is_terminal_cannot_change() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Cancelled).unwrap();
    for state in [
        JobState::Pending,
        JobState::Running,
        JobState::Completed,
        JobState::Failed,
    ] {
        let result = job.transition(state);
        assert!(
            matches!(result, Err(SchedulerError::InvalidTransition)),
            "Cancelled -> {} should be invalid",
            state
        );
    }
}

#[test]
fn transition_failed_cannot_go_to_running() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();
    let result = job.transition(JobState::Running);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

#[test]
fn transition_failed_cannot_go_to_completed() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();
    let result = job.transition(JobState::Completed);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

#[test]
fn transition_failed_cannot_go_to_scheduled() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();
    let result = job.transition(JobState::Scheduled);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

// === ScheduledJob::transition() with delayed job kind ===

#[test]
fn delayed_job_state_transitions_work() {
    let mut job = make_job_with_kind(JobKind::Delayed, JobPriority::High, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    assert_eq!(job.state, JobState::Completed);
}

#[test]
fn delayed_job_cannot_reschedule_after_completed() {
    let mut job = make_job_with_kind(JobKind::Delayed, JobPriority::High, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    let result = job.transition(JobState::Scheduled);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

// === Scheduling Invariant Tests ===

#[test]
fn duplicate_run_prevention_cannot_start_already_running_job() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    let result = job.transition(JobState::Running);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

#[test]
fn duplicate_run_prevention_job_in_pending_cannot_start_twice() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    assert_eq!(job.state, JobState::Pending);
    job.transition(JobState::Running).unwrap();
    let result = job.transition(JobState::Running);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

#[test]
fn recurring_job_catch_up_after_completion() {
    let mut job = make_job_with_kind(JobKind::Recurring, JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    let result = job.transition(JobState::Scheduled);
    assert!(result.is_ok());
    assert_eq!(job.state, JobState::Scheduled);
}

#[test]
fn one_shot_job_cannot_catch_up_after_completion() {
    let mut job = make_job_with_kind(JobKind::OneShot, JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    let result = job.transition(JobState::Scheduled);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

#[test]
fn scheduled_job_has_correct_due_at() {
    let future = future_time();
    let job = make_job(JobPriority::Normal, SchedulePolicy::At(future));
    assert_eq!(job.due_at, future);
    assert_eq!(job.state, JobState::Scheduled);
}

#[test]
fn scheduled_job_past_time_becomes_pending() {
    let past = past_time();
    let job = make_job(JobPriority::Normal, SchedulePolicy::At(past));
    assert_eq!(job.state, JobState::Pending);
}

#[test]
fn retry_can_recover_from_failed_state() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();
    assert!(job.state.is_terminal());
    let result = job.transition(JobState::Retrying);
    assert!(result.is_ok());
    assert_eq!(job.state, JobState::Retrying);
}

#[test]
fn retry_job_can_run_again_after_retry() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();
    job.transition(JobState::Retrying).unwrap();
    job.transition(JobState::Pending).unwrap();
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    assert_eq!(job.state, JobState::Completed);
}

#[test]
fn immediate_job_has_due_at_equal_to_now() {
    let before = Utc::now();
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let after = Utc::now();
    assert!(job.due_at >= before && job.due_at <= after);
}

#[test]
fn after_schedule_computes_correct_due_at() {
    let delay = Duration::from_secs(3600);
    let before = Utc::now();
    let job = make_job(JobPriority::Normal, SchedulePolicy::After(delay));
    let after = Utc::now();
    let expected_min = before + chrono::Duration::from_std(delay).unwrap();
    let expected_max = after + chrono::Duration::from_std(delay).unwrap();
    assert!(job.due_at >= expected_min && job.due_at <= expected_max);
}
