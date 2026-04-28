use chrono::{DateTime, Utc};

use crate::error::SchedulerError;
use crate::types::{
    JobId, JobKind, JobPriority, JobState, RetryPolicy, SchedulePolicy, ScheduledJob,
    SchedulerQueue,
};

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

fn future_time() -> DateTime<Utc> {
    Utc::now() + chrono::Duration::hours(1)
}

fn past_time() -> DateTime<Utc> {
    Utc::now() - chrono::Duration::hours(1)
}

#[test]
fn insert_adds_job_to_queue() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    let result = queue.insert(job);
    assert!(result.is_ok(), "insert should succeed");
    assert_eq!(result.unwrap(), id);
    assert_eq!(queue.len(), 1);
}

#[test]
fn insert_returns_job_id() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::High, SchedulePolicy::Immediate);
    let expected_id = job.id;
    let returned_id = queue.insert(job).unwrap();
    assert_eq!(returned_id, expected_id);
}

#[test]
fn insert_respects_capacity() {
    let mut queue = SchedulerQueue::new(1);
    let job1 = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let job2 = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    queue.insert(job1).unwrap();
    let result = queue.insert(job2);
    assert!(
        matches!(result, Err(SchedulerError::QueueFull)),
        "should reject when at capacity"
    );
}

#[test]
fn insert_scheduled_job_starts_in_scheduled_state_for_future() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::At(future_time()));
    let id = job.id;
    queue.insert(job).unwrap();
    let found = queue.lookup(&id).unwrap();
    assert_eq!(found.state, JobState::Scheduled);
}

#[test]
fn insert_immediate_job_starts_pending() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    let found = queue.lookup(&id).unwrap();
    assert_eq!(found.state, JobState::Pending);
}

#[test]
fn lookup_returns_job_by_id() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    let found = queue.lookup(&id).unwrap();
    assert_eq!(found.id, id);
}

#[test]
fn lookup_returns_error_for_missing_job() {
    let queue = SchedulerQueue::new(100);
    let result = queue.lookup(&JobId::generate());
    assert!(
        matches!(result, Err(SchedulerError::JobNotFound)),
        "lookup of missing job should return JobNotFound"
    );
}

#[test]
fn remove_deletes_job_from_queue() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    let removed = queue.remove(&id).unwrap();
    assert_eq!(removed.id, id);
    assert_eq!(queue.len(), 0);
    assert!(queue.lookup(&id).is_err());
}

#[test]
fn remove_returns_error_for_missing_job() {
    let mut queue = SchedulerQueue::new(100);
    let result = queue.remove(&JobId::generate());
    assert!(matches!(result, Err(SchedulerError::JobNotFound)));
}

#[test]
fn update_state_transitions_correctly() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    queue.update_state(&id, JobState::Running).unwrap();
    let found = queue.lookup(&id).unwrap();
    assert_eq!(found.state, JobState::Running);
}

#[test]
fn update_state_rejects_invalid_transition() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    let result = queue.update_state(&id, JobState::Completed);
    assert!(
        matches!(result, Err(SchedulerError::InvalidTransition)),
        "Pending -> Completed should be invalid"
    );
}

#[test]
fn update_state_rejects_terminal_to_running() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    queue.update_state(&id, JobState::Running).unwrap();
    queue.update_state(&id, JobState::Completed).unwrap();
    let result = queue.update_state(&id, JobState::Running);
    assert!(
        matches!(result, Err(SchedulerError::InvalidTransition)),
        "Completed -> Running should be invalid"
    );
}

#[test]
fn update_state_rejects_missing_job() {
    let mut queue = SchedulerQueue::new(100);
    let result = queue.update_state(&JobId::generate(), JobState::Running);
    assert!(matches!(result, Err(SchedulerError::JobNotFound)));
}

#[test]
fn cancel_transitions_to_cancelled() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    queue.cancel(&id).unwrap();
    let found = queue.lookup(&id).unwrap();
    assert_eq!(found.state, JobState::Cancelled);
}

#[test]
fn cancel_rejects_completed_job() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    queue.update_state(&id, JobState::Running).unwrap();
    queue.update_state(&id, JobState::Completed).unwrap();
    let result = queue.cancel(&id);
    assert!(
        matches!(result, Err(SchedulerError::InvalidTransition)),
        "cannot cancel a completed job"
    );
}

#[test]
fn cancel_rejects_failed_job() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    queue.update_state(&id, JobState::Running).unwrap();
    queue.update_state(&id, JobState::Failed).unwrap();
    let result = queue.cancel(&id);
    assert!(
        matches!(result, Err(SchedulerError::InvalidTransition)),
        "cannot cancel a failed job"
    );
}

#[test]
fn update_schedule_changes_due_at() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    let original_due = job.due_at;
    queue.insert(job).unwrap();
    let new_time = future_time();
    queue
        .update_schedule(&id, SchedulePolicy::At(new_time))
        .unwrap();
    let found = queue.lookup(&id).unwrap();
    assert_ne!(found.due_at, original_due);
}

#[test]
fn update_schedule_rejects_running_job() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    queue.update_state(&id, JobState::Running).unwrap();
    let result = queue.update_schedule(&id, SchedulePolicy::At(future_time()));
    assert!(
        matches!(result, Err(SchedulerError::InvalidTransition)),
        "cannot update schedule of a running job"
    );
}

#[test]
fn update_schedule_rejects_completed_job() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    queue.update_state(&id, JobState::Running).unwrap();
    queue.update_state(&id, JobState::Completed).unwrap();
    let result = queue.update_schedule(&id, SchedulePolicy::At(future_time()));
    assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
}

#[test]
fn update_schedule_rejects_missing_job() {
    let mut queue = SchedulerQueue::new(100);
    let result = queue.update_schedule(&JobId::generate(), SchedulePolicy::Immediate);
    assert!(matches!(result, Err(SchedulerError::JobNotFound)));
}

#[test]
fn pop_due_returns_earliest_highest_priority_job() {
    let mut queue = SchedulerQueue::new(100);
    let low_job = make_job(JobPriority::Low, SchedulePolicy::Immediate);
    let high_job = make_job(JobPriority::High, SchedulePolicy::Immediate);
    let high_id = high_job.id;
    queue.insert(low_job).unwrap();
    queue.insert(high_job).unwrap();
    let popped = queue.pop_due(Utc::now()).unwrap();
    assert_eq!(popped.id, high_id, "higher priority job should pop first");
}

#[test]
fn pop_due_returns_none_when_nothing_due() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::At(future_time()));
    queue.insert(job).unwrap();
    let popped = queue.pop_due(Utc::now());
    assert!(popped.is_none(), "future job should not be due");
}

#[test]
fn pop_due_removes_job_from_queue() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    queue.pop_due(Utc::now()).unwrap();
    assert_eq!(queue.len(), 0);
    assert!(queue.lookup(&id).is_err());
}

#[test]
fn pop_due_returns_none_on_empty_queue() {
    let mut queue = SchedulerQueue::new(100);
    let popped = queue.pop_due(Utc::now());
    assert!(popped.is_none());
}

#[test]
fn priority_ordering_critical_before_high() {
    let mut queue = SchedulerQueue::new(100);
    let high_job = make_job(JobPriority::High, SchedulePolicy::Immediate);
    let crit_job = make_job(JobPriority::Critical, SchedulePolicy::Immediate);
    let crit_id = crit_job.id;
    queue.insert(high_job).unwrap();
    queue.insert(crit_job).unwrap();
    let popped = queue.pop_due(Utc::now()).unwrap();
    assert_eq!(popped.id, crit_id);
}

#[test]
fn same_priority_earlier_due_at_comes_first() {
    let mut queue = SchedulerQueue::new(100);
    let earlier = make_job(JobPriority::Normal, SchedulePolicy::At(past_time()));
    let later = make_job(JobPriority::Normal, SchedulePolicy::At(future_time()));
    let earlier_id = earlier.id;
    queue.insert(later).unwrap();
    queue.insert(earlier).unwrap();
    let popped = queue.pop_due(Utc::now()).unwrap();
    assert_eq!(
        popped.id, earlier_id,
        "earlier due_at should pop first at same priority"
    );
}

#[test]
fn recurring_job_can_transition_completed_to_scheduled() {
    let mut queue = SchedulerQueue::new(100);
    let job = ScheduledJob::new(
        JobKind::Recurring,
        JobPriority::Normal,
        SchedulePolicy::Immediate,
        RetryPolicy::default(),
        bytes::Bytes::from_static(b"recurring"),
    )
    .unwrap();
    let id = job.id;
    queue.insert(job).unwrap();
    queue.update_state(&id, JobState::Running).unwrap();
    queue.update_state(&id, JobState::Completed).unwrap();
    queue.update_state(&id, JobState::Scheduled).unwrap();
    let found = queue.lookup(&id).unwrap();
    assert_eq!(found.state, JobState::Scheduled);
}

#[test]
fn non_recurring_job_cannot_reschedule_after_completed() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    queue.update_state(&id, JobState::Running).unwrap();
    queue.update_state(&id, JobState::Completed).unwrap();
    let result = queue.update_state(&id, JobState::Scheduled);
    assert!(
        matches!(result, Err(SchedulerError::InvalidTransition)),
        "OneShot jobs cannot reschedule"
    );
}

#[test]
fn retry_transition_from_failed() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    queue.update_state(&id, JobState::Running).unwrap();
    queue.update_state(&id, JobState::Failed).unwrap();
    queue.update_state(&id, JobState::Retrying).unwrap();
    let found = queue.lookup(&id).unwrap();
    assert_eq!(found.state, JobState::Retrying);
}

#[test]
fn retrying_to_pending_transition() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    queue.update_state(&id, JobState::Running).unwrap();
    queue.update_state(&id, JobState::Failed).unwrap();
    queue.update_state(&id, JobState::Retrying).unwrap();
    queue.update_state(&id, JobState::Pending).unwrap();
    let found = queue.lookup(&id).unwrap();
    assert_eq!(found.state, JobState::Pending);
}

#[test]
fn multiple_inserts_and_lookup() {
    let mut queue = SchedulerQueue::new(100);
    let mut ids = vec![];
    for i in 0..5 {
        let priority = match i % 5 {
            0 => JobPriority::Critical,
            1 => JobPriority::High,
            2 => JobPriority::Normal,
            3 => JobPriority::Low,
            _ => JobPriority::Background,
        };
        let job = make_job(priority, SchedulePolicy::Immediate);
        ids.push(job.id);
        queue.insert(job).unwrap();
    }
    assert_eq!(queue.len(), 5);
    for id in &ids {
        assert!(queue.lookup(id).is_ok());
    }
}

#[test]
fn peek_returns_next_due_job_without_removing() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();

    let peeked = queue.peek(Utc::now());
    assert!(peeked.is_some());
    assert_eq!(peeked.unwrap().id, id);

    assert!(queue.lookup(&id).is_ok());
    assert_eq!(queue.len(), 1);
}

#[test]
fn peek_returns_none_when_nothing_due() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::At(future_time()));
    queue.insert(job).unwrap();

    assert!(queue.peek(Utc::now()).is_none());
}

#[test]
fn peek_respects_priority() {
    let mut queue = SchedulerQueue::new(100);
    let low_job = make_job(JobPriority::Low, SchedulePolicy::Immediate);
    let high_job = make_job(JobPriority::High, SchedulePolicy::Immediate);
    let high_id = high_job.id;
    queue.insert(low_job).unwrap();
    queue.insert(high_job).unwrap();

    let peeked = queue.peek(Utc::now());
    assert_eq!(peeked.unwrap().id, high_id);
}

#[test]
fn peek_skips_cancelled_jobs() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    queue.cancel(&id).unwrap();

    assert!(queue.peek(Utc::now()).is_none());
}

#[test]
fn peek_returns_none_on_empty_queue() {
    let queue = SchedulerQueue::new(100);
    assert!(queue.peek(Utc::now()).is_none());
}

#[test]
fn peek_next_returns_highest_priority_regardless_of_due_time() {
    let mut queue = SchedulerQueue::new(100);
    let future_job = make_job(JobPriority::High, SchedulePolicy::At(future_time()));
    let immediate_job = make_job(JobPriority::Low, SchedulePolicy::Immediate);
    let high_id = future_job.id;
    queue.insert(future_job).unwrap();
    queue.insert(immediate_job).unwrap();

    let peeked = queue.peek_next();
    assert_eq!(peeked.unwrap().id, high_id);
}

#[test]
fn peek_next_skips_terminal_states() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();
    queue.update_state(&id, JobState::Running).unwrap();
    queue.update_state(&id, JobState::Completed).unwrap();

    assert!(queue.peek_next().is_none());
}

#[test]
fn peek_next_returns_none_on_empty_queue() {
    let queue = SchedulerQueue::new(100);
    assert!(queue.peek_next().is_none());
}

#[test]
fn peek_next_does_not_remove_job() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id = job.id;
    queue.insert(job).unwrap();

    let peeked = queue.peek_next();
    assert!(peeked.is_some());
    assert_eq!(peeked.unwrap().id, id);

    assert!(queue.lookup(&id).is_ok());
    assert_eq!(queue.len(), 1);
}

#[test]
fn peek_skips_failed_and_completed_jobs() {
    let mut queue = SchedulerQueue::new(100);
    let job1 = make_job(JobPriority::High, SchedulePolicy::Immediate);
    let job1_id = job1.id;
    queue.insert(job1).unwrap();
    queue.update_state(&job1_id, JobState::Running).unwrap();
    queue.update_state(&job1_id, JobState::Completed).unwrap();

    let job2 = make_job(JobPriority::Low, SchedulePolicy::Immediate);
    let job2_id = job2.id;
    queue.insert(job2).unwrap();

    let peeked = queue.peek(Utc::now());
    assert_eq!(peeked.unwrap().id, job2_id);
}

#[test]
fn list_by_state_returns_matching_jobs() {
    let mut queue = SchedulerQueue::new(100);
    let job1 = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let job2 = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id1 = job1.id;
    let id2 = job2.id;
    queue.insert(job1).unwrap();
    queue.insert(job2).unwrap();

    queue.update_state(&id1, JobState::Running).unwrap();

    let running = queue.list_by_state(JobState::Running);
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].id, id1);

    let pending = queue.list_by_state(JobState::Pending);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, id2);
}

#[test]
fn list_by_state_returns_empty_for_nonexistent_state() {
    let queue = SchedulerQueue::new(10);
    let result = queue.list_by_state(JobState::Completed);
    assert!(result.is_empty());
}

#[test]
fn list_by_states_returns_jobs_in_multiple_states() {
    let mut queue = SchedulerQueue::new(100);
    let job1 = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let job2 = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let job3 = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    let id1 = job1.id;
    let id2 = job2.id;
    queue.insert(job1).unwrap();
    queue.insert(job2).unwrap();
    queue.insert(job3).unwrap();

    queue.update_state(&id1, JobState::Running).unwrap();
    queue.update_state(&id2, JobState::Running).unwrap();
    queue.update_state(&id2, JobState::Failed).unwrap();

    let results = queue.list_by_states(&[JobState::Running, JobState::Failed]);
    assert_eq!(results.len(), 2);

    let ids: Vec<_> = results.iter().map(|j| j.id).collect();
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));
}

#[test]
fn list_by_states_returns_empty_when_no_matches() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    queue.insert(job).unwrap();

    let results = queue.list_by_states(&[JobState::Completed, JobState::Cancelled]);
    assert!(results.is_empty());
}

#[test]
fn list_by_states_with_empty_slice_returns_nothing() {
    let mut queue = SchedulerQueue::new(100);
    let job = make_job(JobPriority::Normal, SchedulePolicy::Immediate);
    queue.insert(job).unwrap();

    let results = queue.list_by_states(&[]);
    assert!(results.is_empty());
}
