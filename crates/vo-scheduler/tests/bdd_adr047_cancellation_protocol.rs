use vo_scheduler::api::{cancel_job_with_drain, get_job_status, schedule_job};
use vo_scheduler::error::SchedulerError;
use vo_scheduler::queue::SchedulerQueue;
use vo_scheduler::types::{
    JobKind, JobPriority, RetryPolicy, SchedulePolicy,
};

fn make_test_job() -> vo_scheduler::types::ScheduledJob {
    vo_scheduler::types::ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::At(chrono::Utc::now() + chrono::Duration::hours(1)),
        RetryPolicy::default(),
        bytes::Bytes::from_static(b"test payload"),
    )
    .unwrap()
}

fn make_immediate_job() -> vo_scheduler::types::ScheduledJob {
    vo_scheduler::types::ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::Immediate,
        RetryPolicy::default(),
        bytes::Bytes::from_static(b"immediate payload"),
    )
    .unwrap()
}

fn make_queue() -> SchedulerQueue {
    SchedulerQueue::new(100)
}

#[tokio::test]
async fn test_cancel_with_drain_false_returns_immediately() {
    let mut queue = make_queue();
    let job = make_test_job();
    let job_id = schedule_job(&mut queue, job).await.unwrap();

    let result = cancel_job_with_drain(&mut queue, job_id, false).await;
    assert!(result.is_ok(), "cancel with drain=false should return immediately for scheduled job");

    let state = get_job_status(&queue, job_id).await.unwrap();
    assert_eq!(state, vo_scheduler::types::JobState::Cancelled);
}

#[tokio::test]
async fn test_cancel_with_drain_true_waits_for_termination() {
    let mut queue = make_queue();
    let job = make_test_job();
    let job_id = schedule_job(&mut queue, job).await.unwrap();

    queue.update_state(&job_id, vo_scheduler::types::JobState::Pending).unwrap();
    queue.update_state(&job_id, vo_scheduler::types::JobState::Running).unwrap();

    let result = cancel_job_with_drain(&mut queue, job_id, true).await;
    assert!(result.is_ok(), "cancel with drain=true should succeed for running job");

    let state = get_job_status(&queue, job_id).await.unwrap();
    assert_eq!(state, vo_scheduler::types::JobState::Cancelled);
}

#[tokio::test]
async fn test_completed_job_cancel_returns_invalid_transition() {
    let mut queue = make_queue();
    let job = make_test_job();
    let job_id = schedule_job(&mut queue, job).await.unwrap();

    queue.update_state(&job_id, vo_scheduler::types::JobState::Pending).unwrap();
    queue.update_state(&job_id, vo_scheduler::types::JobState::Running).unwrap();
    queue.update_state(&job_id, vo_scheduler::types::JobState::Completed).unwrap();

    let result = cancel_job_with_drain(&mut queue, job_id, false).await;
    assert!(matches!(result, Err(SchedulerError::InvalidTransition { .. })),
        "cancel on completed job should return InvalidTransition");
}

#[tokio::test]
async fn test_failed_job_cancel_returns_invalid_transition() {
    let mut queue = make_queue();
    let job = make_test_job();
    let job_id = schedule_job(&mut queue, job).await.unwrap();

    queue.update_state(&job_id, vo_scheduler::types::JobState::Pending).unwrap();
    queue.update_state(&job_id, vo_scheduler::types::JobState::Running).unwrap();
    queue.update_state(&job_id, vo_scheduler::types::JobState::Failed).unwrap();

    let result = cancel_job_with_drain(&mut queue, job_id, false).await;
    assert!(matches!(result, Err(SchedulerError::InvalidTransition { .. })),
        "cancel on failed job should return InvalidTransition");
}

#[tokio::test]
async fn test_already_cancelled_job_returns_invalid_transition() {
    let mut queue = make_queue();
    let job = make_test_job();
    let job_id = schedule_job(&mut queue, job).await.unwrap();

    cancel_job_with_drain(&mut queue, job_id, false).await.unwrap();

    let result = cancel_job_with_drain(&mut queue, job_id, false).await;
    assert!(matches!(result, Err(SchedulerError::InvalidTransition { .. })),
        "double cancel should return InvalidTransition");
}

#[tokio::test]
async fn test_nonexistent_job_cancel_returns_not_found() {
    let mut queue = make_queue();
    let fake_id = vo_scheduler::types::JobId::generate();

    let result = cancel_job_with_drain(&mut queue, fake_id, false).await;
    assert!(matches!(result, Err(SchedulerError::JobNotFound { .. })),
        "cancel non-existent job should return JobNotFound");
}

#[tokio::test]
async fn test_cancel_running_with_drain_false_returns_immediately() {
    let mut queue = make_queue();
    let job = make_test_job();
    let job_id = schedule_job(&mut queue, job).await.unwrap();

    queue.update_state(&job_id, vo_scheduler::types::JobState::Pending).unwrap();
    queue.update_state(&job_id, vo_scheduler::types::JobState::Running).unwrap();

    let result = cancel_job_with_drain(&mut queue, job_id, false).await;
    assert!(result.is_ok(), "cancel with drain=false should return immediately for running job");
}

#[tokio::test]
async fn test_cancel_pending_job_succeeds() {
    let mut queue = make_queue();
    let job = make_immediate_job();
    let job_id = schedule_job(&mut queue, job).await.unwrap();

    let state = get_job_status(&queue, job_id).await.unwrap();
    assert_eq!(state, vo_scheduler::types::JobState::Pending);

    let result = cancel_job_with_drain(&mut queue, job_id, false).await;
    assert!(result.is_ok(), "cancel on pending job should succeed");
}

#[tokio::test]
async fn test_cancel_retrying_job_succeeds() {
    let mut queue = make_queue();
    let job = make_test_job();
    let job_id = schedule_job(&mut queue, job).await.unwrap();

    queue.update_state(&job_id, vo_scheduler::types::JobState::Pending).unwrap();
    queue.update_state(&job_id, vo_scheduler::types::JobState::Running).unwrap();
    queue.update_state(&job_id, vo_scheduler::types::JobState::Failed).unwrap();
    queue.update_state(&job_id, vo_scheduler::types::JobState::Retrying).unwrap();

    let result = cancel_job_with_drain(&mut queue, job_id, false).await;
    assert!(result.is_ok(), "cancel on retrying job should succeed");
}