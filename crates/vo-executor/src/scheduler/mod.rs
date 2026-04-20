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
    Job, JobId, JobKind, JobPriority, JobResult, JobState, Schedule, SchedulePolicy, ScheduledJob,
    SchedulerConfig, SchedulerRetryPolicy, SerializedPayload,
};

use std::sync::Arc;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

#[derive(Debug)]
pub struct Scheduler {
    queue: PriorityQueue,
    config: SchedulerConfig,
    semaphore: Arc<Semaphore>,
    running: std::sync::atomic::AtomicBool,
    priority_boost: Arc<Notify>,
}

impl Scheduler {
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            queue: PriorityQueue::new(),
            semaphore: Arc::new(Semaphore::new(config.max_concurrent)),
            config,
            running: std::sync::atomic::AtomicBool::new(false),
            priority_boost: Arc::new(Notify::new()),
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

    pub fn poll_due_jobs_priority_aware(
        &mut self,
        now_ms: u64,
    ) -> (Vec<Job>, Vec<OwnedSemaphorePermit>) {
        let poll_limit = self.config.max_jobs_per_scan.min(self.config.max_concurrent as u32);
        let due = self.queue.pop_due_jobs(now_ms, poll_limit);
        if due.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let mut ready: Vec<Job> = Vec::new();
        let mut permits: Vec<OwnedSemaphorePermit> = Vec::new();
        let mut deferred: Vec<Job> = Vec::new();
        for job in due {
            if let Some(permit) = self.try_acquire_with_priority(job.priority) {
                ready.push(job);
                permits.push(permit);
            } else {
                deferred.push(job);
            }
        }
        for job in deferred {
            self.queue.push(job, now_ms);
        }
        (ready, permits)
    }

    pub fn reschedule(&mut self, job: Job, next_fire_ms: u64) {
        self.queue.push(job, next_fire_ms);
    }

    pub fn try_acquire(&self) -> Option<OwnedSemaphorePermit> {
        self.semaphore.clone().try_acquire_owned().ok()
    }

    pub fn try_acquire_with_priority(
        &self,
        priority: JobPriority,
    ) -> Option<OwnedSemaphorePermit> {
        if let Some(permit) = self.semaphore.clone().try_acquire_owned().ok() {
            return Some(permit);
        }
        if priority < JobPriority::Normal {
            self.priority_boost.notify_one();
        }
        None
    }

    #[allow(dead_code)]
    pub async fn acquire(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("scheduler semaphore closed")
    }

    pub fn boost_waiter(&self) -> Arc<Notify> {
        self.priority_boost.clone()
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

    #[tokio::test]
    async fn rq008_priority_inversion_critical_blocked_by_low() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: std::time::Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let low_permit = scheduler.try_acquire().expect("low priority should acquire");
        assert!(
            scheduler.try_acquire().is_none(),
            "semaphore should be exhausted"
        );

        let critical_result = scheduler.try_acquire_with_priority(JobPriority::Critical);
        assert!(
            critical_result.is_none(),
            "critical should not get permit while low holds it"
        );

        drop(low_permit);

        let critical_after = scheduler.try_acquire_with_priority(JobPriority::Critical);
        assert!(
            critical_after.is_some(),
            "critical should acquire immediately after low releases"
        );
    }

    #[tokio::test]
    async fn rq008_priority_aware_poll_high_runs_before_low() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: std::time::Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let mut scheduler = Scheduler::new(config);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        scheduler
            .schedule(
                Job::new(
                    JobId::new(1),
                    "low-job".to_string(),
                    Schedule::OneShot { fire_at_ms: now_ms },
                )
                .with_priority(JobPriority::Low),
            )
            .unwrap();
        scheduler
            .schedule(
                Job::new(
                    JobId::new(2),
                    "critical-job".to_string(),
                    Schedule::OneShot { fire_at_ms: now_ms },
                )
                .with_priority(JobPriority::Critical),
            )
            .unwrap();
        scheduler
            .schedule(
                Job::new(
                    JobId::new(3),
                    "background-job".to_string(),
                    Schedule::OneShot { fire_at_ms: now_ms },
                )
                .with_priority(JobPriority::Background),
            )
            .unwrap();

        let (ready, _permits) = scheduler.poll_due_jobs_priority_aware(now_ms);
        assert_eq!(ready.len(), 1, "only 1 slot available");
        assert_eq!(
            ready[0].priority,
            JobPriority::Critical,
            "critical job must run first when semaphore is contended"
        );
    }

    #[tokio::test]
    async fn rq008_priority_aware_poll_multiple_slots_respects_order() {
        let config = SchedulerConfig {
            max_concurrent: 3,
            scan_interval: std::time::Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let mut scheduler = Scheduler::new(config);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        scheduler
            .schedule(
                Job::new(
                    JobId::new(1),
                    "low".to_string(),
                    Schedule::OneShot { fire_at_ms: now_ms },
                )
                .with_priority(JobPriority::Low),
            )
            .unwrap();
        scheduler
            .schedule(
                Job::new(
                    JobId::new(2),
                    "critical".to_string(),
                    Schedule::OneShot { fire_at_ms: now_ms },
                )
                .with_priority(JobPriority::Critical),
            )
            .unwrap();
        scheduler
            .schedule(
                Job::new(
                    JobId::new(3),
                    "high".to_string(),
                    Schedule::OneShot { fire_at_ms: now_ms },
                )
                .with_priority(JobPriority::High),
            )
            .unwrap();
        scheduler
            .schedule(
                Job::new(
                    JobId::new(4),
                    "normal".to_string(),
                    Schedule::OneShot { fire_at_ms: now_ms },
                )
                .with_priority(JobPriority::Normal),
            )
            .unwrap();

        let (ready, _permits) = scheduler.poll_due_jobs_priority_aware(now_ms);
        assert_eq!(ready.len(), 3, "3 slots, 4 jobs -> 3 dispatched");

        assert_eq!(ready[0].priority, JobPriority::Critical);
        assert_eq!(ready[1].priority, JobPriority::High);
        assert_eq!(ready[2].priority, JobPriority::Normal);

        let (remaining, _) = scheduler.poll_due_jobs_priority_aware(now_ms);
        assert_eq!(remaining.len(), 0, "low deferred, no permits freed");
    }

    #[tokio::test]
    async fn rq008_deferred_low_priority_reclaimed_after_permit_free() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: std::time::Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let mut scheduler = Scheduler::new(config);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        scheduler
            .schedule(
                Job::new(
                    JobId::new(1),
                    "critical".to_string(),
                    Schedule::OneShot { fire_at_ms: now_ms },
                )
                .with_priority(JobPriority::Critical),
            )
            .unwrap();
        scheduler
            .schedule(
                Job::new(
                    JobId::new(2),
                    "low".to_string(),
                    Schedule::OneShot { fire_at_ms: now_ms },
                )
                .with_priority(JobPriority::Low),
            )
            .unwrap();

        let (ready, permits) = scheduler.poll_due_jobs_priority_aware(now_ms);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].priority, JobPriority::Critical);
        assert_eq!(scheduler.len(), 1, "low job deferred back to queue");

        drop(permits);

        let (ready2, _) = scheduler.poll_due_jobs_priority_aware(now_ms);
        assert_eq!(ready2.len(), 1, "low job acquired freed permit");
        assert_eq!(ready2[0].priority, JobPriority::Low);
    }

    #[tokio::test]
    async fn rq008_boost_notified_for_critical_high_not_normal() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: std::time::Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let _permit = scheduler.try_acquire().expect("acquire the only slot");
        let notify = scheduler.boost_waiter();

        let notified_critical = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let nc = notified_critical.clone();
        let notify_clone = notify.clone();

        let waiter = tokio::spawn(async move {
            tokio::select! {
                _ = notify_clone.notified() => {
                    nc.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
            }
        });

        scheduler.try_acquire_with_priority(JobPriority::Normal);

        scheduler.try_acquire_with_priority(JobPriority::Critical);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        assert!(
            notified_critical.load(std::sync::atomic::Ordering::SeqCst),
            "Critical priority SHOULD trigger boost notify"
        );

        waiter.await.unwrap();
    }
}
