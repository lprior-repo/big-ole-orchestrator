use crate::job::ScheduledJob;
use crate::types::{JobKind, JobPriority, JobState, RetryPolicy, SchedulePolicy};

fn make_running_job_with_attempt(attempt_count: u32) -> ScheduledJob {
    let mut job = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::Immediate,
        RetryPolicy::default(),
        bytes::Bytes::from_static(b"test-payload"),
    )
    .unwrap();
    job.transition(JobState::Running).unwrap();
    job.attempt_count = attempt_count;
    job
}

#[test]
fn given_retryable_failure_when_transition_runs_then_retry_state_is_atomic() {
    let mut job = make_running_job_with_attempt(1);
    let original_due_at = job.due_at;

    job.transition_to_retrying("transient network error")
        .unwrap();

    assert_eq!(job.attempt_count, 2, "attempt_count must increment to 2");
    assert_eq!(job.state, JobState::Retrying, "state must be Retrying");
    assert!(
        job.due_at > original_due_at,
        "due_at must advance past original"
    );
    assert_eq!(
        job.last_error.as_deref(),
        Some("transient network error"),
        "last_error must be set"
    );
}
