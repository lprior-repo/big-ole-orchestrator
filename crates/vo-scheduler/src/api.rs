//! Scheduler API operations per ADR-047 §5.

use crate::error::SchedulerError;
use crate::types::{JobId, JobState, SchedulePolicy, ScheduledJob, SchedulerQueue};

pub async fn schedule_job(
    _queue: &SchedulerQueue,
    _job: ScheduledJob,
) -> Result<JobId, SchedulerError> {
    unimplemented!("TDD-RED: Implementation pending")
}

pub async fn cancel_job(
    _queue: &SchedulerQueue,
    _job_id: JobId,
) -> Result<(), SchedulerError> {
    unimplemented!("TDD-RED: Implementation pending")
}

pub async fn get_job_status(_job_id: JobId) -> Result<JobState, SchedulerError> {
    unimplemented!("TDD-RED: Implementation pending")
}

pub async fn update_job_schedule(
    _queue: &SchedulerQueue,
    _job_id: JobId,
    _new_schedule: SchedulePolicy,
) -> Result<(), SchedulerError> {
    unimplemented!("TDD-RED: Implementation pending")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        JobKind, JobPriority, RetryPolicy, SchedulePolicy, ScheduledJob, SerializedPayload,
    };
    use chrono::Duration;

    fn make_test_job() -> ScheduledJob {
        ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::Immediate,
            RetryPolicy {
                max_attempts: 3,
                backoff_multiplier: 2.0,
                initial_delay: Duration::seconds(1),
                max_delay: Duration::minutes(5),
            },
            SerializedPayload::new(b"test payload".to_vec()),
        )
    }

    fn make_queue() -> SchedulerQueue {
        SchedulerQueue::new()
    }

    #[tokio::test]
    async fn schedule_job_returns_job_id() {
        let queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&queue, job).await.unwrap();
        assert!(!job_id.0.is_nil());
    }

    #[tokio::test]
    async fn schedule_job_persists_job() {
        let mut queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&queue, job).await.unwrap();
        let state = get_job_status(job_id).await.unwrap();
        assert_eq!(state, crate::types::JobState::Scheduled);
    }

    #[tokio::test]
    async fn schedule_immediate_job_transitions_to_pending() {
        let queue = make_queue();
        let job = ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::Immediate,
            RetryPolicy {
                max_attempts: 1,
                backoff_multiplier: 1.0,
                initial_delay: Duration::zero(),
                max_delay: Duration::minutes(1),
            },
            SerializedPayload::new(vec![]),
        );
        let job_id = schedule_job(&queue, job).await.unwrap();
        let state = get_job_status(job_id).await.unwrap();
        assert_eq!(state, crate::types::JobState::Pending);
    }

    #[tokio::test]
    async fn cancel_job_returns_ok_for_existing_job() {
        let queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&queue, job).await.unwrap();
        let result = cancel_job(&queue, job_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn cancel_job_returns_not_found_for_nonexistent() {
        let queue = make_queue();
        let fake_id = JobId::new();
        let result = cancel_job(&queue, fake_id).await;
        assert!(matches!(result, Err(SchedulerError::JobNotFound(_))));
    }

    #[tokio::test]
    async fn cancel_job_returns_invalid_transition_for_completed() {
        let queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&queue, job).await.unwrap();
        cancel_job(&queue, job_id).await.unwrap();
        let result = cancel_job(&queue, job_id).await;
        assert!(matches!(result, Err(SchedulerError::InvalidTransition(_))));
    }

    #[tokio::test]
    async fn get_job_status_returns_state() {
        let queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&queue, job).await.unwrap();
        let state = get_job_status(job_id).await.unwrap();
        assert_eq!(state, crate::types::JobState::Scheduled);
    }

    #[tokio::test]
    async fn get_job_status_returns_not_found_for_nonexistent() {
        let fake_id = JobId::new();
        let result = get_job_status(fake_id).await;
        assert!(matches!(result, Err(SchedulerError::JobNotFound(_))));
    }

    #[tokio::test]
    async fn update_job_schedule_returns_ok() {
        let queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&queue, job).await.unwrap();
        let new_schedule = SchedulePolicy::After(Duration::hours(1));
        let result = update_job_schedule(&queue, job_id, new_schedule).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn update_job_schedule_allows_in_scheduled_state() {
        let queue = make_queue();
        let job = ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::At(chrono::Utc::now() + Duration::hours(1)),
            RetryPolicy {
                max_attempts: 1,
                backoff_multiplier: 1.0,
                initial_delay: Duration::zero(),
                max_delay: Duration::minutes(1),
            },
            SerializedPayload::new(vec![]),
        );
        let job_id = schedule_job(&queue, job).await.unwrap();
        let new_schedule = SchedulePolicy::After(Duration::hours(2));
        let result = update_job_schedule(&queue, job_id, new_schedule).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn update_job_schedule_rejects_in_completed_state() {
        let queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&queue, job).await.unwrap();
        cancel_job(&queue, job_id).await.unwrap();
        let new_schedule = SchedulePolicy::Immediate;
        let result = update_job_schedule(&queue, job_id, new_schedule).await;
        assert!(matches!(result, Err(SchedulerError::InvalidTransition(_))));
    }

    #[tokio::test]
    async fn update_job_schedule_rejects_in_running_state() {
        let queue = make_queue();
        let job = make_test_job();
        let job_id = schedule_job(&queue, job).await.unwrap();
        let result = update_job_schedule(&queue, job_id, SchedulePolicy::Immediate).await;
        assert!(matches!(result, Err(SchedulerError::InvalidTransition(_))));
    }
}
