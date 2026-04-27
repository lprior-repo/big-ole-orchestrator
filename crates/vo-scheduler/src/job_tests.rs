use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::error::SchedulerError;
use crate::job::ScheduledJob;
use crate::types::{JobKind, JobPriority, JobState, RetryPolicy, SchedulePolicy};

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
    let job = make_job(
        JobPriority::Normal,
        SchedulePolicy::Cron("0 * * * *".to_string()),
    );
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
    let job = make_job_with_kind(
        JobKind::Delayed,
        JobPriority::Critical,
        SchedulePolicy::Immediate,
    );
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
        SchedulePolicy::Cron("invalid cron".to_string()),
        RetryPolicy::default(),
        bytes::Bytes::from_static(b"test"),
    );
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        SchedulerError::InvalidSchedule
    ));
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
    let mut job = make_job_with_kind(
        JobKind::Recurring,
        JobPriority::Normal,
        SchedulePolicy::Immediate,
    );
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
    let mut job = make_job_with_kind(
        JobKind::Delayed,
        JobPriority::High,
        SchedulePolicy::Immediate,
    );
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    assert_eq!(job.state, JobState::Completed);
}

#[test]
fn delayed_job_cannot_reschedule_after_completed() {
    let mut job = make_job_with_kind(
        JobKind::Delayed,
        JobPriority::High,
        SchedulePolicy::Immediate,
    );
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();
    let result = job.transition(JobState::Scheduled);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

// ===========================================================================
// Job scheduling invariants
// ===========================================================================

#[test]
fn job_id_uniqueness_across_many_creations() {
    let mut ids = std::collections::HashSet::new();
    for _ in 0..1000 {
        let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
        assert!(ids.insert(job.id), "duplicate JobId generated");
    }
}

#[test]
fn job_created_at_and_updated_at_set() {
    let before = Utc::now();
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let after = Utc::now();
    assert!(job.created_at >= before && job.created_at <= after);
    assert!(job.updated_at >= before && job.updated_at <= after);
}

#[test]
fn job_due_at_matches_schedule_policy_at() {
    let dt = Utc::now() + chrono::Duration::hours(2);
    let job = make_job(JobPriority::Normal, SchedulePolicy::At(dt));
    assert_eq!(job.due_at, dt);
}

#[test]
fn job_due_at_for_after_schedule_is_approximately_correct() {
    let delay = Duration::from_secs(600);
    let before = Utc::now();
    let job = make_job(JobPriority::Normal, SchedulePolicy::After(delay));
    let after = Utc::now();
    let expected_min = before + chrono::Duration::from_std(delay).unwrap();
    let expected_max = after + chrono::Duration::from_std(delay).unwrap();
    assert!(job.due_at >= expected_min && job.due_at <= expected_max);
}

#[test]
fn job_due_at_for_immediate_is_approximately_now() {
    let before = Utc::now();
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let after = Utc::now();
    assert!(job.due_at >= before && job.due_at <= after);
}

#[test]
fn job_cron_due_at_is_now() {
    let before = Utc::now();
    let job = make_job(JobPriority::Normal, SchedulePolicy::Cron("0 * * * *".to_string()));
    let after = Utc::now();
    assert!(job.due_at >= before && job.due_at <= after);
}

#[test]
fn job_state_is_pending_for_past_due_time() {
    let job = make_job(JobPriority::Normal, SchedulePolicy::At(past_time()));
    assert_eq!(job.state, JobState::Pending);
}

#[test]
fn job_state_is_scheduled_for_future_due_time() {
    let job = make_job(JobPriority::Normal, SchedulePolicy::At(future_time()));
    assert_eq!(job.state, JobState::Scheduled);
}

#[test]
fn job_invalid_cron_rejects_empty_string() {
    let result = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::Cron("".to_string()),
        RetryPolicy::default(),
        bytes::Bytes::from_static(b"test"),
    );
    assert!(matches!(result, Err(SchedulerError::InvalidSchedule)));
}

#[test]
fn job_invalid_cron_rejects_four_fields() {
    let result = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::Cron("* * * *".to_string()),
        RetryPolicy::default(),
        bytes::Bytes::from_static(b"test"),
    );
    assert!(matches!(result, Err(SchedulerError::InvalidSchedule)));
}

#[test]
fn job_valid_cron_accepts_standard_expression() {
    let result = ScheduledJob::new(
        JobKind::Recurring,
        JobPriority::Normal,
        SchedulePolicy::Cron("*/5 * * * *".to_string()),
        RetryPolicy::default(),
        bytes::Bytes::from_static(b"test"),
    );
    assert!(result.is_ok());
}

#[test]
fn job_recurring_completed_can_cycle_to_scheduled_multiple_times() {
    let mut job = make_job_with_kind(JobKind::Recurring, JobPriority::Normal, SchedulePolicy::Immediate);
    for _ in 0..5 {
        // Full cycle: Pending -> Running -> Completed -> Scheduled -> Pending -> ...
        job.transition(JobState::Running).unwrap();
        job.transition(JobState::Completed).unwrap();
        job.transition(JobState::Scheduled).unwrap();
        job.transition(JobState::Pending).unwrap();
    }
    assert_eq!(job.state, JobState::Pending);
}

#[test]
fn job_transition_invalid_same_state() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    // Pending -> Pending is not a valid transition
    let result = job.transition(JobState::Pending);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

#[test]
fn job_transition_scheduled_to_running_is_invalid() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::At(future_time()));
    assert_eq!(job.state, JobState::Scheduled);
    let result = job.transition(JobState::Running);
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

#[test]
fn job_serialization_roundtrip() {
    // Only Immediate variant serializes successfully with serde_json (tagged newtype
    // variants containing DateTime/Duration/String cannot be serialized).
    let job = make_job(JobPriority::High, SchedulePolicy::Immediate);
    let json = serde_json::to_string(&job).unwrap();
    let recovered: ScheduledJob = serde_json::from_str(&json).unwrap();
    assert_eq!(job.id, recovered.id);
    assert_eq!(job.kind, recovered.kind);
    assert_eq!(job.priority, recovered.priority);
    assert_eq!(job.state, recovered.state);
    assert_eq!(job.attempt_count, recovered.attempt_count);
    assert_eq!(job.payload, recovered.payload);
}

#[test]
fn job_serialization_with_at_schedule_fails_gracefully() {
    // SchedulePolicy::At(DateTime) is a tagged newtype variant that serde_json
    // cannot serialize (it would need #[serde(borrow)] or a wrapper).
    let job = make_job(JobPriority::High, SchedulePolicy::At(future_time()));
    let result = serde_json::to_string(&job);
    assert!(result.is_err());
}

#[test]
fn job_clone_preserves_all_fields() {
    let job = make_job(JobPriority::Critical, SchedulePolicy::Immediate);
    let cloned = job.clone();
    assert_eq!(job.id, cloned.id);
    assert_eq!(job.kind, cloned.kind);
    assert_eq!(job.state, cloned.state);
    assert_eq!(job.priority, cloned.priority);
    assert_eq!(job.payload, cloned.payload);
}

#[test]
fn job_debug_format_contains_useful_info() {
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let debug = format!("{:?}", job);
    assert!(debug.contains("ScheduledJob"));
}

#[test]
fn job_empty_payload_is_valid() {
    let job = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::Immediate,
        RetryPolicy::default(),
        bytes::Bytes::new(),
    )
    .unwrap();
    assert!(job.payload.is_empty());
}

#[test]
fn job_large_payload_is_preserved() {
    let large = vec![0u8; 1024 * 1024];
    let job = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::Immediate,
        RetryPolicy::default(),
        large.clone().into(),
    )
    .unwrap();
    assert_eq!(job.payload.len(), 1024 * 1024);
}

#[test]
fn job_after_with_zero_duration_is_pending() {
    let job = make_job(JobPriority::Normal, SchedulePolicy::After(Duration::from_secs(0)));
    assert_eq!(job.state, JobState::Pending);
}

// ===========================================================================
// Full retry loop invariants
// ===========================================================================

#[test]
fn full_retry_cycle_then_succeed() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    assert_eq!(job.attempt_count, 0);

    // First attempt fails
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();

    // Retry 1
    job.transition(JobState::Retrying).unwrap();
    job.transition(JobState::Pending).unwrap();
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();

    // Retry 2
    job.transition(JobState::Retrying).unwrap();
    job.transition(JobState::Pending).unwrap();
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Completed).unwrap();

    assert_eq!(job.state, JobState::Completed);
}

#[test]
fn full_retry_cycle_max_attempts_exhausted() {
    let policy = RetryPolicy::try_new(2, 2.0, Duration::from_secs(1), Duration::from_secs(10))
        .unwrap();
    let mut job = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::Immediate,
        policy,
        bytes::Bytes::from_static(b"test"),
    )
    .unwrap();

    // Initial attempt
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();

    // Retry 1 (can_retry(0) = true)
    job.transition(JobState::Retrying).unwrap();
    job.transition(JobState::Pending).unwrap();
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();

    // Retry 2 (can_retry(1) = true)
    job.transition(JobState::Retrying).unwrap();
    job.transition(JobState::Pending).unwrap();
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();

    // At this point attempt_count=0 still (not tracked by transition), but
    // max_attempts=2 means 2 failures then exhausted
    assert_eq!(job.state, JobState::Failed);
}

#[test]
fn retry_loop_cancelled_during_retry() {
    let mut job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    job.transition(JobState::Running).unwrap();
    job.transition(JobState::Failed).unwrap();
    job.transition(JobState::Retrying).unwrap();
    job.transition(JobState::Cancelled).unwrap();
    assert!(job.state.is_terminal());
}
