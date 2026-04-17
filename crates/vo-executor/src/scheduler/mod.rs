//! Background job scheduler module
//!
//! Provides cron-like job scheduling with:
//! - Recurring and one-shot jobs
//! - Priority queue
//! - Concurrency limits
//! - Failure handling with retries
//!
//! Types aligned to ADR-047 Background Job Scheduler Contract.

mod error;
mod queue;
mod types;

pub use error::{ExecutionError, JobRunError, RetryExhaustedError, SchedulerError};
pub use queue::{PriorityQueue, SchedulerQueue};
pub use types::{
    Job, JobId, JobKind, JobPriority, JobResult, JobState, Schedule, SchedulePolicy,
    SchedulerConfig, SchedulerRetryPolicy, ScheduledJob, SerializedPayload,
};

use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Debug)]
pub struct Scheduler {
    queue: PriorityQueue,
    config: SchedulerConfig,
    semaphore: Arc<Semaphore>,
    running: std::sync::atomic::AtomicBool,
}

impl Scheduler {
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            queue: PriorityQueue::new(),
            semaphore: Arc::new(Semaphore::new(config.max_concurrent)),
            config,
            running: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn schedule(&mut self, job: Job) -> Result<(), SchedulerError> {
        let fire_at_ms = match job.schedule.next_fire_time(0) {
            Some(t) => t,
            None => {
                return Err(SchedulerError::InvalidSchedule(
                    "Cannot determine fire time".into(),
                ))
            }
        };
        self.queue.push(job, fire_at_ms);
        Ok(())
    }

    pub fn cancel(&mut self, job_id: JobId) -> Option<Job> {
        self.queue.remove(&job_id)
    }

    pub fn poll_due_jobs(&mut self, now_ms: u64) -> Vec<Job> {
        self.queue
            .pop_due_jobs(now_ms, self.config.max_jobs_per_scan)
    }

    pub fn reschedule(&mut self, job: Job, next_fire_ms: u64) {
        self.queue.push(job, next_fire_ms);
    }

    pub fn try_acquire(&self) -> Option<OwnedSemaphorePermit> {
        self.semaphore.clone().try_acquire_owned().ok()
    }

    #[allow(dead_code)]
    pub async fn acquire(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("scheduler semaphore closed")
    }

    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn start(&mut self) {
        self.running
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn stop(&mut self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scheduler_schedule_and_poll() {
        let config = SchedulerConfig {
            max_concurrent: 2,
            scan_interval: std::time::Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let mut scheduler = Scheduler::new(config);

        let job = Job::new(
            JobId::new(1),
            "test".to_string(),
            Schedule::one_shot(std::time::Duration::from_millis(50)),
        );
        scheduler.schedule(job).unwrap();

        assert_eq!(scheduler.len(), 1);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms + 100);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, JobId::new(1));
    }

    #[tokio::test]
    async fn scheduler_cancel() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job = Job::new(
            JobId::new(1),
            "test".to_string(),
            Schedule::one_shot(std::time::Duration::from_millis(50)),
        );
        scheduler.schedule(job).unwrap();

        assert_eq!(scheduler.len(), 1);

        let removed = scheduler.cancel(JobId::new(1));
        assert!(removed.is_some());
        assert_eq!(scheduler.len(), 0);
    }

    #[tokio::test]
    async fn scheduler_concurrency_limit() {
        let config = SchedulerConfig {
            max_concurrent: 2,
            scan_interval: std::time::Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let permit1 = scheduler.try_acquire();
        let permit2 = scheduler.try_acquire();
        let permit3 = scheduler.try_acquire();

        assert!(permit1.is_some());
        assert!(permit2.is_some());
        assert!(permit3.is_none());
    }

    #[tokio::test]
    async fn scheduler_reschedule_recurring() {
        let config = SchedulerConfig::default();
        let mut scheduler = Scheduler::new(config);

        let job = Job::new(
            JobId::new(1),
            "recurring".to_string(),
            Schedule::interval(std::time::Duration::from_millis(100)),
        );
        scheduler.schedule(job).unwrap();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        let due = scheduler.poll_due_jobs(now_ms + 200);
        assert_eq!(due.len(), 1);

        let job_id = due[0].id;
        scheduler.cancel(job_id);

        if let Schedule::Interval { interval_ms } = &due[0].schedule {
            let next_fire = now_ms + 200 + interval_ms;
            scheduler.reschedule(due[0].clone(), next_fire);
        }

        assert_eq!(scheduler.len(), 1);
    }
}
