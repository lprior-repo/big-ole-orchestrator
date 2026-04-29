//! Scheduler API operations per ADR-047 §5.

use crate::error::SchedulerError;
use crate::types::{JobId, JobState, SchedulePolicy, ScheduledJob, SchedulerQueue, SerializedPayload};

pub async fn schedule_job(
    queue: &mut SchedulerQueue,
    mut job: ScheduledJob,
) -> Result<JobId, SchedulerError> {
    let job_id = job.id;
    let state = if matches!(job.schedule_policy, SchedulePolicy::Immediate) {
        job.state = JobState::Pending;
        JobState::Pending
    } else {
        job.state
    };
    queue.insert(job)?;
    if let Some(current) = queue.get_state(&job_id) {
        if current != state {
            queue.update_state(&job_id, state)?;
        }
    }
    Ok(job_id)
}

pub async fn cancel_job(
    queue: &mut SchedulerQueue,
    job_id: JobId,
) -> Result<(), SchedulerError> {
    let state = queue.get_state(&job_id).ok_or_else(|| SchedulerError::JobNotFound { job_id: job_id.to_string() })?;
    
    match state {
        JobState::Scheduled | JobState::Pending | JobState::Running | JobState::Retrying => {
            queue.update_state(&job_id, JobState::Cancelled)?;
            Ok(())
        }
        JobState::Completed | JobState::Failed | JobState::Cancelled => {
            Err(SchedulerError::InvalidTransition { from_state: state.to_string(), action: "cancel".to_string() })
        }
    }
}

pub async fn get_job_status(
    queue: &SchedulerQueue,
    job_id: JobId,
) -> Result<JobState, SchedulerError> {
    queue.get_state(&job_id).ok_or_else(|| SchedulerError::JobNotFound { job_id: job_id.to_string() })
}

pub async fn update_job_schedule(
    queue: &mut SchedulerQueue,
    job_id: JobId,
    new_schedule: SchedulePolicy,
) -> Result<(), SchedulerError> {
    let state = queue.get_state(&job_id).ok_or_else(|| SchedulerError::JobNotFound { job_id: job_id.to_string() })?;

    match state {
        JobState::Scheduled | JobState::Pending => {
            if matches!(new_schedule, SchedulePolicy::Immediate) && state == JobState::Scheduled {
                return Err(SchedulerError::InvalidTransition { from_state: state.to_string(), action: "update_schedule".to_string() });
            }
            queue.update_schedule(&job_id, new_schedule)?;
            Ok(())
        }
        JobState::Running | JobState::Completed | JobState::Failed | JobState::Cancelled | JobState::Retrying => {
            Err(SchedulerError::InvalidTransition { from_state: state.to_string(), action: "update_schedule".to_string() })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        priority::JobPriority,
        JobKind, JobState, RetryPolicy, SchedulePolicy, SerializedPayload,
    };
    use chrono::{Duration, Utc};

    fn make_test_job() -> ScheduledJob {
        ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::At(Utc::now() + Duration::hours(1)),
            RetryPolicy::default(),
            bytes::Bytes::from_static(b"test payload"),
        ).unwrap()
    }

    fn make_queue() -> SchedulerQueue {
        SchedulerQueue::new(100)
    }

    #[tokio::test]
    async fn schedule_job_returns_job_id() {
        let mut queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&mut queue, job).await.unwrap();
        assert!(!job_id.0.is_nil());
    }

    #[tokio::test]
    async fn schedule_job_persists_job() {
        let mut queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&mut queue, job).await.unwrap();
        let state = get_job_status(&queue, job_id).await.unwrap();
        assert_eq!(state, crate::types::JobState::Scheduled);
    }

    #[tokio::test]
    async fn schedule_immediate_job_transitions_to_pending() {
        let mut queue = make_queue();
        let job = ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::Immediate,
            RetryPolicy::default(),
            bytes::Bytes::from_static(b""),
        );
        let job_id = schedule_job(&mut queue, job).await.unwrap();
        let state = get_job_status(&queue, job_id).await.unwrap();
        assert_eq!(state, crate::types::JobState::Pending);
    }

    #[tokio::test]
    async fn cancel_job_returns_ok_for_existing_job() {
        let mut queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&mut queue, job).await.unwrap();
        let result = cancel_job(&mut queue, job_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn cancel_job_returns_not_found_for_nonexistent() {
        let mut queue = make_queue();
        let fake_id = JobId::generate();
        let result = cancel_job(&mut queue, fake_id).await;
        assert!(matches!(result, Err(SchedulerError::JobNotFound { .. })));
    }

    #[tokio::test]
    async fn cancel_job_returns_invalid_transition_for_completed() {
        let mut queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&mut queue, job).await.unwrap();
        cancel_job(&mut queue, job_id).await.unwrap();
        let result = cancel_job(&mut queue, job_id).await;
        assert!(matches!(result, Err(SchedulerError::InvalidTransition { .. })));
    }

    #[tokio::test]
    async fn get_job_status_returns_state() {
        let mut queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&mut queue, job).await.unwrap();
        let state = get_job_status(&queue, job_id).await.unwrap();
        assert_eq!(state, crate::types::JobState::Scheduled);
    }

    #[tokio::test]
    async fn get_job_status_returns_not_found_for_nonexistent() {
        let queue = make_queue();
        let fake_id = JobId::generate();
        let result = get_job_status(&queue, fake_id).await;
        assert!(matches!(result, Err(SchedulerError::JobNotFound { .. })));
    }

    #[tokio::test]
    async fn update_job_schedule_returns_ok() {
        let mut queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&mut queue, job).await.unwrap();
        let new_schedule = SchedulePolicy::After(Duration::hours(1));
        let result = update_job_schedule(&mut queue, job_id, new_schedule).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn update_job_schedule_allows_in_scheduled_state() {
        let mut queue = make_queue();
        let job = ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::At(Utc::now() + Duration::hours(1)),
            RetryPolicy::default(),
            bytes::Bytes::from_static(b""),
        );
        let job_id = schedule_job(&mut queue, job).await.unwrap();
        let new_schedule = SchedulePolicy::After(Duration::hours(2));
        let result = update_job_schedule(&mut queue, job_id, new_schedule).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn update_job_schedule_rejects_in_cancelled_state() {
        let mut queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&mut queue, job).await.unwrap();
        cancel_job(&mut queue, job_id).await.unwrap();
        let new_schedule = SchedulePolicy::Immediate;
        let result = update_job_schedule(&mut queue, job_id, new_schedule).await;
        assert!(matches!(result, Err(SchedulerError::InvalidTransition { .. })));
    }

    #[tokio::test]
    async fn update_job_schedule_rejects_in_running_state() {
        let mut queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&mut queue, job).await.unwrap();
        queue.update_state(&job_id, JobState::Running).unwrap();
        let result = update_job_schedule(&mut queue, job_id, SchedulePolicy::At(Utc::now() + Duration::hours(1))).await;
        assert!(matches!(result, Err(SchedulerError::InvalidTransition { .. })));
    }
}