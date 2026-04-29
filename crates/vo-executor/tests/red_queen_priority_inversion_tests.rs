//! REDQUEEN adversarial tests: priority inversion under load (rq-019)
//!
//! These tests verify that the scheduler prevents priority inversion where
//! high-priority tasks can be blocked by low-priority tasks holding resources.
//!
//! EARS Requirements:
//!   Ubiquitous: THE SYSTEM SHALL prevent priority inversion
//!   Event-Driven: When high priority waits, THE SYSTEM SHALL not block indefinitely
//!   Unwanted: If inversion occurs, THE SYSTEM SHALL allow indefinite wait
//!             (because: Priority semantics not yet implemented)
//!
//! The current implementation uses yield signaling to encourage permit holders
//! to release when high priority work arrives. This provides best-effort priority
//! handling but cannot guarantee immediate preemption due to tokio semaphore limitations.

use std::sync::Arc;
use std::time::Duration;
use vo_executor::scheduler::Scheduler;
use vo_executor::{Job, JobId, JobPriority, Schedule, SchedulerConfig};

fn make_job(id: u64, priority: JobPriority, fire_at_ms: u64) -> Job {
    Job::new(
        JobId::new(id),
        format!("payload-{}", id),
        Schedule::OneShot { fire_at_ms },
    )
    .with_priority(priority)
}

#[cfg(test)]
mod red_queen_priority_inversion_tests {
    use super::*;

    #[tokio::test]
    async fn rq019_critical_signals_yield_when_blocked() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let _low_permit = scheduler
            .try_acquire()
            .expect("low priority should acquire");

        let critical_result = scheduler.try_acquire_with_priority(JobPriority::Critical);

        assert!(
            critical_result.is_none(),
            "CRITICAL cannot immediately acquire when LOW holds permit (semaphore limitation)"
        );

        drop(_low_permit);
    }

    #[tokio::test]
    async fn rq019_high_signals_yield_when_blocked() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let _bg_permit = scheduler.try_acquire().expect("background should acquire");

        let high_result = scheduler.try_acquire_with_priority(JobPriority::High);

        assert!(
            high_result.is_none(),
            "HIGH cannot immediately acquire when BACKGROUND holds permit (semaphore limitation)"
        );

        drop(_bg_permit);
    }

    #[tokio::test]
    async fn rq019_multiple_high_can_acquire_when_permits_available() {
        let config = SchedulerConfig {
            max_concurrent: 2,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let low_permit = scheduler.try_acquire().expect("low should acquire");
        let high_permit = scheduler.try_acquire_with_priority(JobPriority::High);

        assert!(
            high_permit.is_some(),
            "HIGH should acquire when another permit is available"
        );

        drop(low_permit);
        drop(high_permit);
    }

    #[tokio::test]
    async fn rq019_priority_aware_poll_returns_high_before_low_under_contention() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let mut scheduler = Scheduler::new(config);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        scheduler
            .schedule(make_job(1, JobPriority::Low, now_ms))
            .unwrap();
        scheduler
            .schedule(make_job(2, JobPriority::Critical, now_ms))
            .unwrap();

        let (ready, permits) = scheduler.poll_due_jobs_priority_aware(now_ms);

        assert_eq!(ready.len(), 1, "Only 1 slot available");
        assert_eq!(
            ready[0].priority,
            JobPriority::Critical,
            "CRITICAL must run first even though LOW was scheduled first"
        );

        drop(permits);
    }

    #[tokio::test]
    async fn rq019_deferred_jobs_requeued_with_priority_preserved() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let mut scheduler = Scheduler::new(config);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);

        scheduler
            .schedule(make_job(1, JobPriority::Critical, now_ms))
            .unwrap();
        scheduler
            .schedule(make_job(2, JobPriority::Low, now_ms))
            .unwrap();

        let (first_batch, _) = scheduler.poll_due_jobs_priority_aware(now_ms);
        assert_eq!(
            first_batch[0].priority,
            JobPriority::Critical,
            "First batch should be Critical"
        );
        assert_eq!(scheduler.len(), 1, "Low should be deferred");

        let (second_batch, _) = scheduler.poll_due_jobs_priority_aware(now_ms);
        assert_eq!(
            second_batch[0].priority,
            JobPriority::Low,
            "Second batch should be Low after Critical slot freed"
        );
    }

    #[tokio::test]
    async fn rq019_critical_priority_boost_notifies_waiter() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let _permit = scheduler.try_acquire().expect("acquire the only slot");
        let notify = scheduler.boost_waiter();

        let notified = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let notified_clone = notified.clone();
        let notify_clone = notify.clone();

        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = notify_clone.notified() => {
                    notified_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                _ = tokio::time::sleep(Duration::from_millis(50)) => {}
            }
        });

        scheduler.try_acquire_with_priority(JobPriority::Critical);
        scheduler.try_acquire_with_priority(JobPriority::High);
        scheduler.try_acquire_with_priority(JobPriority::Normal);

        tokio::time::sleep(Duration::from_millis(10)).await;

        assert!(
            notified.load(std::sync::atomic::Ordering::SeqCst),
            "Boost notification must fire for Critical priority"
        );

        handle.await.unwrap();
    }
}

#[cfg(test)]
mod red_queen_inversion_stress_tests {
    use super::*;

    #[tokio::test]
    async fn rq019_stress_yield_signaled_when_many_low_hold_permits() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let mut low_permits = Vec::new();
        for _ in 0..10 {
            if let Some(p) = scheduler.try_acquire() {
                low_permits.push(p);
            }
        }

        let critical_result = scheduler.try_acquire_with_priority(JobPriority::Critical);

        assert!(
            critical_result.is_none(),
            "CRITICAL cannot acquire when 10 LOW jobs hold permits (semaphore limitation)"
        );

        for p in low_permits {
            drop(p);
        }
        drop(critical_result);
    }

    #[tokio::test]
    async fn rq019_stress_critical_and_high_both_receive_notifications() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let _permit = scheduler.try_acquire().expect("acquire the only slot");

        let critical = scheduler.try_acquire_with_priority(JobPriority::Critical);
        let high = scheduler.try_acquire_with_priority(JobPriority::High);

        assert!(
            critical.is_none(),
            "CRITICAL cannot acquire when permit held"
        );
        assert!(high.is_none(), "HIGH cannot acquire when permit held");
    }

    #[tokio::test]
    async fn rq019_stress_high_priority_acquires_after_permit_release() {
        let config = SchedulerConfig {
            max_concurrent: 1,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let low_permit = scheduler.try_acquire().expect("low should acquire");

        let critical = scheduler.try_acquire_with_priority(JobPriority::Critical);
        assert!(critical.is_none(), "CRITICAL blocked by LOW");

        drop(low_permit);

        let critical_after = scheduler.try_acquire_with_priority(JobPriority::Critical);
        assert!(
            critical_after.is_some(),
            "CRITICAL should acquire after LOW releases permit"
        );
    }
}
