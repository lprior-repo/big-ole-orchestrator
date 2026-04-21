//! Red Queen adversarial tests for priority inversion under load (rq-019).
//!
//! These tests attack the scheduler's priority semantics under concurrency pressure.
//! The scheduler uses a BinaryHeap for priority ordering but a tokio::Semaphore
//! (FIFO, not priority-aware) for concurrency control.
//!
//! EARS Requirements:
//! - Ubiquitous: THE SYSTEM SHALL prevent priority inversion
//! - Event-Driven: When high priority waits, THE SYSTEM SHALL not block indefinitely
//! - Unwanted: If inversion occurs, THE SYSTEM SHALL allow indefinite wait (priority semantics required)
//!
//! Contracts:
//! - Preconditions: Mixed priority tasks
//! - Postconditions: High priority proceeds
//! - Invariants: No indefinite inversion

use std::time::Duration;
use vo_executor::scheduler::Scheduler;
use vo_executor::{Job, JobId, JobPriority, Schedule, SchedulerConfig};

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn make_job(id: u64, priority: JobPriority, fire_offset_ms: u64) -> Job {
    Job::new(
        JobId::new(id),
        format!("job-{}", id),
        Schedule::one_shot(Duration::from_millis(fire_offset_ms)),
    )
    .with_priority(priority)
}

fn scheduler_with_concurrency(max_concurrent: usize) -> Scheduler {
    Scheduler::new(SchedulerConfig {
        max_concurrent,
        scan_interval: Duration::from_millis(10),
        max_jobs_per_scan: 1000,
    })
}

#[cfg(test)]
mod rq019_priority_queue_ordering_under_load {
    use super::*;

    #[test]
    fn critical_always_dequeued_first_regardless_of_insertion_order() {
        let mut scheduler = scheduler_with_concurrency(10);

        scheduler.schedule(make_job(1, JobPriority::Low, 10)).unwrap();
        scheduler.schedule(make_job(2, JobPriority::Normal, 10)).unwrap();
        scheduler.schedule(make_job(3, JobPriority::Background, 10)).unwrap();
        scheduler.schedule(make_job(4, JobPriority::Critical, 10)).unwrap();
        scheduler.schedule(make_job(5, JobPriority::High, 10)).unwrap();
        scheduler.schedule(make_job(6, JobPriority::Normal, 10)).unwrap();
        scheduler.schedule(make_job(7, JobPriority::Critical, 10)).unwrap();

        let due = scheduler.poll_due_jobs(now_ms() + 100);
        assert!(due.len() >= 2, "Should have at least 2 due jobs");
        assert_eq!(due[0].priority, JobPriority::Critical, "First job must be Critical");
        assert_eq!(due[1].priority, JobPriority::Critical, "Second job must be Critical");
    }

    #[test]
    fn high_priority_precedes_normal_under_saturation() {
        let mut scheduler = scheduler_with_concurrency(1);

        for i in 0..50 {
            scheduler
                .schedule(make_job(i, JobPriority::Normal, 10))
                .unwrap();
        }
        scheduler
            .schedule(make_job(99, JobPriority::High, 10))
            .unwrap();

        let due = scheduler.poll_due_jobs(now_ms() + 100);
        assert!(!due.is_empty());
        let high_found = due.iter().position(|j| j.id == JobId::new(99));
        assert!(high_found.is_some(), "High priority job must be dequeued");
        let high_idx = high_found.unwrap();
        for j in &due[..high_idx] {
            assert!(
                j.priority <= JobPriority::High,
                "Only Critical jobs should precede High, found {:?}",
                j.priority
            );
        }
    }

    #[test]
    fn critical_precedes_all_under_massive_load() {
        let mut scheduler = scheduler_with_concurrency(10);

        for i in 0..200 {
            let priority = match i % 4 {
                0 => JobPriority::Low,
                1 => JobPriority::Normal,
                2 => JobPriority::Background,
                _ => JobPriority::High,
            };
            scheduler.schedule(make_job(i, priority, 10)).unwrap();
        }
        scheduler
            .schedule(make_job(999, JobPriority::Critical, 10))
            .unwrap();

        let due = scheduler.poll_due_jobs(now_ms() + 100);
        assert!(!due.is_empty());
        assert_eq!(
            due[0].id,
            JobId::new(999),
            "Critical job must be first out of {} due jobs",
            due.len()
        );
    }
}

#[cfg(test)]
mod rq019_semaphore_fifo_priority_inversion {
    use super::*;

    #[tokio::test]
    async fn semaphore_does_not_respect_priority_when_full() {
        let scheduler = scheduler_with_concurrency(2);

        let _low_permit1 = scheduler.try_acquire();
        let _low_permit2 = scheduler.try_acquire();

        let critical_permit = scheduler.try_acquire();
        assert!(
            critical_permit.is_none(),
            "Semaphore is FIFO: Critical job cannot acquire when Low jobs hold all permits. \
             This documents the priority inversion vulnerability (rq-019)."
        );
    }

    #[tokio::test]
    async fn permit_released_allows_next_job() {
        let scheduler = scheduler_with_concurrency(2);

        let low_permit1 = scheduler.try_acquire();
        let _low_permit2 = scheduler.try_acquire();

        assert!(scheduler.try_acquire().is_none());

        drop(low_permit1);

        let next_permit = scheduler.try_acquire();
        assert!(
            next_permit.is_some(),
            "After releasing a permit, a new acquisition must succeed"
        );
    }

    #[tokio::test]
    async fn high_priority_blocked_by_low_jobs_inversion_detected() {
        let scheduler = scheduler_with_concurrency(1);

        let _low_permit = scheduler.try_acquire();

        let high_permit = scheduler.try_acquire();
        assert!(
            high_permit.is_none(),
            "PRIORITY INVERSION DETECTED (rq-019): High-priority job blocked by Low job \
             holding the only permit. tokio::Semaphore is FIFO, not priority-aware."
        );
    }
}

#[cfg(test)]
mod rq019_poll_and_acquire_gap {
    use super::*;

    #[tokio::test]
    async fn critical_dequeued_but_cannot_acquire_permit() {
        let mut scheduler = scheduler_with_concurrency(1);

        scheduler.schedule(make_job(1, JobPriority::Low, 10)).unwrap();
        let low_due = scheduler.poll_due_jobs(now_ms() + 100);
        assert_eq!(low_due.len(), 1);

        let _low_permit = scheduler.try_acquire();
        assert!(low_due[0].priority == JobPriority::Low);

        scheduler
            .schedule(make_job(2, JobPriority::Critical, 10))
            .unwrap();

        let critical_due = scheduler.poll_due_jobs(now_ms() + 100);
        assert_eq!(critical_due.len(), 1);
        assert_eq!(critical_due[0].priority, JobPriority::Critical);

        let critical_permit = scheduler.try_acquire();
        assert!(
            critical_permit.is_none(),
            "PRIORITY INVERSION: Critical job is correctly dequeued first by priority queue, \
             but cannot acquire semaphore permit because Low job holds it. \
             The gap between queue ordering (priority-aware) and semaphore (FIFO) \
             is the inversion vulnerability."
        );
    }

    #[tokio::test]
    async fn normal_job_prevents_critical_when_concurrency_saturated() {
        let mut scheduler = scheduler_with_concurrency(3);

        for i in 1..=3 {
            scheduler
                .schedule(make_job(i, JobPriority::Normal, 10))
                .unwrap();
        }
        let normal_due = scheduler.poll_due_jobs(now_ms() + 100);
        assert_eq!(normal_due.len(), 3);

        let _p1 = scheduler.try_acquire();
        let _p2 = scheduler.try_acquire();
        let _p3 = scheduler.try_acquire();

        scheduler
            .schedule(make_job(99, JobPriority::Critical, 10))
            .unwrap();

        let crit_due = scheduler.poll_due_jobs(now_ms() + 100);
        assert_eq!(crit_due.len(), 1);
        assert_eq!(crit_due[0].priority, JobPriority::Critical);

        assert!(
            scheduler.try_acquire().is_none(),
            "Critical job dequeued correctly but blocked at semaphore — inversion confirmed"
        );
    }
}

#[cfg(test)]
mod rq019_priority_ordering_invariants {
    use super::*;

    #[test]
    fn all_five_priority_levels_are_ordered() {
        assert!(JobPriority::Critical < JobPriority::High);
        assert!(JobPriority::High < JobPriority::Normal);
        assert!(JobPriority::Normal < JobPriority::Low);
        assert!(JobPriority::Low < JobPriority::Background);
    }

    #[test]
    fn priority_queue_preserves_full_ordering_under_interleaved_insert() {
        let mut scheduler = scheduler_with_concurrency(10);
        let base_ms = now_ms();

        let jobs = [
            (JobPriority::Normal, 1u64, base_ms + 100),
            (JobPriority::Critical, 2u64, base_ms + 100),
            (JobPriority::Low, 3u64, base_ms + 100),
            (JobPriority::High, 4u64, base_ms + 100),
            (JobPriority::Background, 5u64, base_ms + 100),
            (JobPriority::Critical, 6u64, base_ms + 100),
            (JobPriority::Normal, 7u64, base_ms + 100),
        ];

        for (prio, id, fire_at) in jobs {
            let job = Job::new(
                JobId::new(id),
                format!("job-{}", id),
                Schedule::OneShot { fire_at_ms: fire_at },
            )
            .with_priority(prio);
            scheduler.schedule(job).unwrap();
        }

        let due = scheduler.poll_due_jobs(base_ms + 200);
        let priorities: Vec<JobPriority> = due.iter().map(|j| j.priority).collect();
        assert_eq!(priorities, vec![
            JobPriority::Critical,
            JobPriority::Critical,
            JobPriority::High,
            JobPriority::Normal,
            JobPriority::Normal,
            JobPriority::Low,
            JobPriority::Background,
        ], "All jobs same fire_at_ms: must come out in strict priority order");
    }

    #[test]
    fn same_priority_jobs_preserved_up_to_max_per_scan() {
        let mut scheduler = scheduler_with_concurrency(10);

        for i in 0..20 {
            scheduler
                .schedule(make_job(i, JobPriority::Normal, 10))
                .unwrap();
        }

        let due = scheduler.poll_due_jobs(now_ms() + 100);
        assert!(
            due.len() <= 100,
            "Should respect max_jobs_per_scan limit"
        );
        for job in &due {
            assert_eq!(job.priority, JobPriority::Normal);
        }
    }

    #[test]
    fn critical_jobs_never_behind_lower_priority() {
        let mut scheduler = scheduler_with_concurrency(10);

        for i in 0..100 {
            let prio = if i == 50 {
                JobPriority::Critical
            } else {
                JobPriority::Normal
            };
            scheduler.schedule(make_job(i, prio, 10)).unwrap();
        }

        let due = scheduler.poll_due_jobs(now_ms() + 100);
        let critical_positions: Vec<usize> = due
            .iter()
            .enumerate()
            .filter(|(_, j)| j.priority == JobPriority::Critical)
            .map(|(i, _)| i)
            .collect();

        for pos in &critical_positions {
            for j in &due[..*pos] {
                assert!(
                    j.priority <= JobPriority::Critical,
                    "Job at position {} has priority {:?} which should not precede Critical",
                    pos,
                    j.priority
                );
            }
        }
    }
}

#[cfg(test)]
mod rq019_reschedule_priority_inversion {
    use super::*;

    #[test]
    fn rescheduled_critical_does_not_lose_priority() {
        let mut scheduler = scheduler_with_concurrency(10);

        scheduler
            .schedule(make_job(1, JobPriority::Normal, 10))
            .unwrap();
        scheduler
            .schedule(make_job(2, JobPriority::Critical, 10))
            .unwrap();

        let due = scheduler.poll_due_jobs(now_ms() + 100);
        assert_eq!(due[0].id, JobId::new(2));

        let critical_job = due[0].clone();
        scheduler.cancel(critical_job.id);
        scheduler.reschedule(critical_job, now_ms() + 200);

        let due2 = scheduler.poll_due_jobs(now_ms() + 300);
        assert_eq!(
            due2[0].priority,
            JobPriority::Critical,
            "Rescheduled Critical must retain priority"
        );
    }

    #[test]
    fn rescheduled_normal_does_not_overtake_critical() {
        let mut scheduler = scheduler_with_concurrency(10);
        let base_ms = now_ms();

        let crit = Job::new(
            JobId::new(1),
            "critical".to_string(),
            Schedule::OneShot { fire_at_ms: base_ms + 100 },
        )
        .with_priority(JobPriority::Critical);

        let normal = Job::new(
            JobId::new(2),
            "normal".to_string(),
            Schedule::OneShot { fire_at_ms: base_ms + 100 },
        )
        .with_priority(JobPriority::Normal);

        scheduler.schedule(crit).unwrap();
        scheduler.schedule(normal).unwrap();

        let normal_job = scheduler.cancel(JobId::new(2)).unwrap();
        scheduler.reschedule(normal_job, base_ms + 50);

        let due = scheduler.poll_due_jobs(base_ms + 200);
        assert_eq!(due[0].priority, JobPriority::Critical,
            "Critical must come first even when Normal has earlier fire_at_ms");
    }
}

#[cfg(test)]
mod rq019_cancel_under_inversion {
    use super::*;

    #[test]
    fn cancelling_blocking_jobs_frees_queue_slots() {
        let mut scheduler = scheduler_with_concurrency(10);

        for i in 0..5 {
            scheduler
                .schedule(make_job(i, JobPriority::Low, 10))
                .unwrap();
        }
        scheduler
            .schedule(make_job(99, JobPriority::Critical, 10))
            .unwrap();

        let removed = scheduler.cancel(JobId::new(3));
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().priority, JobPriority::Low);

        let due = scheduler.poll_due_jobs(now_ms() + 100);
        assert_eq!(due[0].id, JobId::new(99));
    }

    #[tokio::test]
    async fn cancelled_job_releases_permit() {
        let scheduler = scheduler_with_concurrency(1);

        let low_permit = scheduler.try_acquire();
        assert!(low_permit.is_some());

        drop(low_permit);

        let critical_permit = scheduler.try_acquire();
        assert!(
            critical_permit.is_some(),
            "After dropping Low permit, Critical must acquire"
        );
    }
}

#[cfg(test)]
mod rq019_inversion_summary {
    use super::*;

    #[test]
    fn invariant_priority_queue_is_correct() {
        let mut scheduler = scheduler_with_concurrency(10);

        let cases: Vec<(JobPriority, u64)> = vec![
            (JobPriority::Background, 1),
            (JobPriority::Low, 2),
            (JobPriority::Normal, 3),
            (JobPriority::High, 4),
            (JobPriority::Critical, 5),
        ];

        for (prio, id) in &cases {
            scheduler.schedule(make_job(*id, *prio, 10)).unwrap();
        }

        let due = scheduler.poll_due_jobs(now_ms() + 100);
        let priorities: Vec<JobPriority> = due.iter().map(|j| j.priority).collect();
        assert_eq!(
            priorities,
            vec![
                JobPriority::Critical,
                JobPriority::High,
                JobPriority::Normal,
                JobPriority::Low,
                JobPriority::Background,
            ],
            "Priority queue ordering is CORRECT — the BinaryHeap correctly implements priority ordering"
        );
    }

    #[tokio::test]
    async fn invariant_semaphore_is_fifo_not_priority_aware() {
        let scheduler = scheduler_with_concurrency(2);

        let _bg = scheduler.try_acquire();
        let _low = scheduler.try_acquire();

        let critical = scheduler.try_acquire();

        assert!(
            critical.is_none(),
            "INVARIANT DOCUMENTED: tokio::Semaphore is FIFO. \
             When Background and Low jobs hold all permits, Critical cannot proceed. \
             This is the priority inversion vulnerability identified by rq-019. \
             The priority queue correctly orders Critical first, but the semaphore \
             does not honor that ordering — it grants permits in FIFO arrival order."
        );
    }
}
