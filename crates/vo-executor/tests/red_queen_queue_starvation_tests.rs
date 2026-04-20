//! Red Queen adversarial tests: queue starvation under overload (rq-010).
//!
//! These tests attack the scheduler's fairness guarantees under sustained overload.
//! The EARS requirements state:
//!   Ubiquitous: THE SYSTEM SHALL handle overload gracefully
//!   Event-Driven: When system overloaded, THE SYSTEM SHALL shed load fairly
//!   Unwanted: If certain queues starved, THE SYSTEM SHALL completely block some tasks
//!
//! Contract invariants:
//!   - No priority class is completely starved under overload
//!   - Load is shed fairly across all priority classes
//!
//! Current implementation uses a strict BinaryHeap with no fairness mechanism.
//! These tests expose the starvation vulnerability as part of coevolutionary testing.

use std::time::Duration;
use vo_executor::scheduler::{PriorityQueue, SchedulerQueue};
use vo_executor::{Job, JobId, JobPriority, Schedule, SchedulerConfig};

fn make_job(id: u64, priority: JobPriority, fire_at_ms: u64) -> Job {
    Job::new(
        JobId::new(id),
        format!("payload-{}", id),
        Schedule::OneShot { fire_at_ms },
    )
    .with_priority(priority)
}

const ALL_PRIORITIES: [JobPriority; 5] = [
    JobPriority::Critical,
    JobPriority::High,
    JobPriority::Normal,
    JobPriority::Low,
    JobPriority::Background,
];

fn count_by_priority(jobs: &[Job]) -> Vec<(JobPriority, usize)> {
    let mut counts = vec![0usize; 5];
    for job in jobs {
        let idx = job.priority as usize;
        counts[idx] += 1;
    }
    ALL_PRIORITIES
        .iter()
        .zip(counts.iter())
        .map(|(&p, c)| (p, *c))
        .collect()
}

#[cfg(test)]
mod red_queen_starvation_tests {
    use super::*;

    #[test]
    fn starvation_test_all_classes_represented_in_fair_poll() {
        let mut queue = PriorityQueue::new();
        let now_ms: u64 = 1000;
        let total_per_class: u64 = 200;

        for priority in &ALL_PRIORITIES {
            for i in 0..total_per_class {
                let id = (*priority as u64) * 1000 + i;
                queue.push(make_job(id, *priority, now_ms), now_ms);
            }
        }

        let total_jobs = total_per_class as usize * ALL_PRIORITIES.len();
        assert_eq!(queue.len(), total_jobs);

        let due = queue.due_jobs(now_ms, u32::MAX);
        let counts = count_by_priority(&due.iter().map(|(j, _)| j.clone()).collect::<Vec<_>>());

        let critical_count = counts[0].1;
        let _background_count = counts[4].1;
        let high_count = counts[1].1;

        assert!(
            critical_count > 0,
            "Critical jobs must be represented in due_jobs"
        );
        assert!(
            high_count > 0,
            "High jobs must be represented in due_jobs"
        );

        assert_eq!(
            due.len(),
            total_jobs,
            "due_jobs should return all due jobs when max is u32::MAX"
        );

        for i in 0..5 {
            assert_eq!(
                counts[i].1, total_per_class as usize,
                "All {} jobs should be due, found {}",
                format!("{:?}", counts[i].0),
                counts[i].1
            );
        }
    }

    #[test]
    fn starvation_test_pop_due_jobs_under_constrained_scan() {
        let mut queue = PriorityQueue::new();
        let now_ms: u64 = 1000;
        let total_per_class: u64 = 100;

        for priority in &ALL_PRIORITIES {
            for i in 0..total_per_class {
                let id = (*priority as u64) * 1000 + i;
                queue.push(make_job(id, *priority, now_ms), now_ms);
            }
        }

        let scan_limit = 50u32;
        let due = queue.pop_due_jobs(now_ms, scan_limit);

        assert_eq!(
            due.len(),
            scan_limit as usize,
            "Should return exactly max_jobs_per_scan"
        );

        let counts = count_by_priority(&due);
        let critical_count = counts[0].1;
        let background_count = counts[4].1;

        assert!(
            critical_count > 0,
            "Under constrained scan, Critical should always be served"
        );

        if background_count == 0 {
            eprintln!(
                "ADVERSARIAL FINDING: Background jobs completely starved under scan_limit={}",
                scan_limit
            );
            eprintln!(
                "  Distribution: {:?}",
                counts
                    .iter()
                    .map(|(p, c)| format!("{:?}: {}", p, c))
                    .collect::<Vec<_>>()
            );
        }

        let remaining = queue.len();
        assert!(
            remaining > 0,
            "Queue should still have jobs after constrained scan"
        );
        assert_eq!(
            remaining,
            total_per_class as usize * ALL_PRIORITIES.len() - scan_limit as usize
        );
    }

    #[test]
    fn starvation_test_continuous_critical_flood() {
        let mut queue = PriorityQueue::new();
        let now_ms: u64 = 1000;
        let critical_count: u64 = 500;
        let background_count: u64 = 10;

        for i in 0..critical_count {
            queue.push(make_job(i, JobPriority::Critical, now_ms), now_ms);
        }
        for i in 0..background_count {
            let id = critical_count + i;
            queue.push(make_job(id, JobPriority::Background, now_ms), now_ms);
        }

        assert_eq!(queue.len(), (critical_count + background_count) as usize);

        let scan_limit = 100u32;
        let due = queue.pop_due_jobs(now_ms, scan_limit);

        let counts = count_by_priority(&due);
        let critical_served = counts[0].1;
        let background_served = counts[4].1;

        assert!(
            critical_served > 0,
            "Critical should always be served"
        );

        if background_served == 0 && critical_count >= scan_limit as u64 {
            eprintln!(
                "ADVERSARIAL FINDING [rq-010]: Background jobs completely starved by Critical flood"
            );
            eprintln!(
                "  {} Critical jobs served, {} Background jobs served (out of {} Background total)",
                critical_served, background_served, background_count
            );
            eprintln!(
                "  This violates: 'No class completely starved' invariant"
            );
        }
    }

    #[test]
    fn starvation_test_multi_round_drain_with_per_class_guarantee() {
        let mut queue = PriorityQueue::new();
        let now_ms: u64 = 1000;
        let total_per_class: u64 = 50;
        let scan_limit = 25u32;
        let total_rounds = (total_per_class * ALL_PRIORITIES.len() as u64 + scan_limit as u64 - 1)
            / scan_limit as u64;

        for priority in &ALL_PRIORITIES {
            for i in 0..total_per_class {
                let id = (*priority as u64) * 1000 + i;
                queue.push(make_job(id, *priority, now_ms), now_ms);
            }
        }

        let mut all_served: Vec<Job> = Vec::new();
        let mut rounds = 0;

        loop {
            let due = queue.pop_due_jobs(now_ms, scan_limit);
            if due.is_empty() {
                break;
            }
            all_served.extend(due);
            rounds += 1;
            if rounds > total_rounds + 5 {
                panic!("Too many rounds — possible infinite loop");
            }
        }

        assert_eq!(
            all_served.len(),
            total_per_class as usize * ALL_PRIORITIES.len(),
            "All jobs should eventually be served"
        );

        let counts = count_by_priority(&all_served);
        for (priority, count) in &counts {
            assert_eq!(
                *count,
                total_per_class as usize,
                "All {:?} jobs should be served after full drain, got {}",
                priority,
                count
            );
        }
    }

    #[test]
    fn starvation_test_scheduler_queue_overload_scenario() {
        let mut queue = SchedulerQueue::new();
        let now_ms: u64 = 1000;

        for priority in &ALL_PRIORITIES {
            for i in 0..50 {
                let id = (*priority as u64) * 1000 + i;
                queue.push(make_job(id, *priority, now_ms), now_ms);
            }
        }

        let scan_limit = 100u32;
        let due = queue.pop_due_jobs(now_ms, scan_limit);

        let counts = count_by_priority(&due);
        let critical_served = counts[0].1;
        let high_served = counts[1].1;
        let normal_served = counts[2].1;
        let low_served = counts[3].1;
        let bg_served = counts[4].1;

        assert!(critical_served > 0, "Critical must be served");
        assert!(high_served > 0, "High must be served");

        if low_served == 0 || bg_served == 0 {
            eprintln!(
                "ADVERSARIAL FINDING [rq-010]: Low/Background starved in SchedulerQueue overload"
            );
            eprintln!(
                "  Critical={}, High={}, Normal={}, Low={}, Background={}",
                critical_served, high_served, normal_served, low_served, bg_served
            );
        }
    }

    #[test]
    fn starvation_test_continuous_high_priority_injection() {
        let mut queue = PriorityQueue::new();
        let now_ms: u64 = 1000;

        for i in 0..20 {
            queue.push(make_job(i, JobPriority::Background, now_ms), now_ms);
        }

        for i in 0..20 {
            queue.push(
                make_job(100 + i, JobPriority::Critical, now_ms),
                now_ms,
            );
        }

        let first_batch = queue.pop_due_jobs(now_ms, 10);
        let first_counts = count_by_priority(&first_batch);
        assert!(
            first_counts[0].1 > 0,
            "Critical should be in first batch"
        );

        if first_counts[4].1 == 0 {
            for i in 0..20 {
                queue.push(
                    make_job(200 + i, JobPriority::Critical, now_ms + 1),
                    now_ms + 1,
                );
            }

            let second_batch = queue.pop_due_jobs(now_ms + 1, 10);
            let second_counts = count_by_priority(&second_batch);
            assert!(
                second_counts[0].1 > 0,
                "New Critical should be served"
            );
        }
    }

    #[test]
    fn starvation_test_fairness_ratio_under_mixed_load() {
        let mut queue = PriorityQueue::new();
        let now_ms: u64 = 1000;
        let per_class: u64 = 200;

        for priority in &ALL_PRIORITIES {
            for i in 0..per_class {
                let id = (*priority as u64) * 1000 + i;
                queue.push(make_job(id, *priority, now_ms), now_ms);
            }
        }

        let scan_limit = 500u32;
        let due = queue.pop_due_jobs(now_ms, scan_limit);
        let counts = count_by_priority(&due);

        let critical_pct = counts[0].1 as f64 / due.len() as f64 * 100.0;
        let background_pct = counts[4].1 as f64 / due.len() as f64 * 100.0;

        eprintln!(
            "Load distribution: Critical={:.1}%, High={:.1}%, Normal={:.1}%, Low={:.1}%, Background={:.1}%",
            critical_pct,
            counts[1].1 as f64 / due.len() as f64 * 100.0,
            counts[2].1 as f64 / due.len() as f64 * 100.0,
            counts[3].1 as f64 / due.len() as f64 * 100.0,
            background_pct,
        );

        assert!(
            due.len() == scan_limit as usize,
            "Should return exactly scan_limit jobs"
        );
    }

    #[test]
    fn starvation_test_earliest_fire_time_breaks_ties_within_class() {
        let mut queue = PriorityQueue::new();
        let now_ms: u64 = 1000;

        queue.push(make_job(1, JobPriority::Normal, now_ms - 100), now_ms - 100);
        queue.push(make_job(2, JobPriority::Normal, now_ms - 50), now_ms - 50);
        queue.push(make_job(3, JobPriority::Normal, now_ms), now_ms);
        queue.push(make_job(4, JobPriority::High, now_ms), now_ms);
        queue.push(make_job(5, JobPriority::Critical, now_ms), now_ms);

        let due = queue.pop_due_jobs(now_ms + 1, 10);

        assert_eq!(due.len(), 5);
        assert_eq!(due[0].id, JobId::new(5), "Critical first");
        assert_eq!(due[1].id, JobId::new(4), "High second");

        let normal_ids: Vec<u64> = due[2..].iter().map(|j| j.id.0).collect();
        assert!(
            normal_ids.contains(&1) && normal_ids.contains(&2) && normal_ids.contains(&3),
            "All Normal jobs should be served"
        );
    }

    #[test]
    fn starvation_test_background_jobs_never_served_under_infinite_critical() {
        let mut queue = PriorityQueue::new();
        let now_ms: u64 = 1000;

        queue.push(make_job(9999, JobPriority::Background, now_ms), now_ms);

        for i in 0..100 {
            queue.push(make_job(i, JobPriority::Critical, now_ms), now_ms);
        }

        let due = queue.pop_due_jobs(now_ms, 50);
        let counts = count_by_priority(&due);

        assert_eq!(counts[0].1, 50, "All 50 should be Critical");
        assert_eq!(counts[4].1, 0, "Background should be starved in this batch");

        let remaining_bg = queue
            .due_jobs(now_ms, u32::MAX)
            .iter()
            .filter(|(j, _)| j.priority == JobPriority::Background)
            .count();
        assert!(remaining_bg > 0, "Background job should still be in queue");
    }
}

#[cfg(test)]
mod red_queen_scheduler_starvation_tests {
    use super::*;
    use vo_executor::scheduler::Scheduler;

    #[tokio::test]
    async fn scheduler_starvation_test_constrained_concurrency() {
        let config = SchedulerConfig {
            max_concurrent: 5,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 10,
        };
        let mut scheduler = Scheduler::new(config);

        let now_ms: u64 = 1000;

        for priority in &ALL_PRIORITIES {
            for i in 0..30 {
                let id = (*priority as u64) * 1000 + i;
                let job = make_job(id, *priority, 0);
                scheduler.schedule(job).unwrap();
            }
        }

        let due = scheduler.poll_due_jobs(now_ms);

        assert_eq!(due.len(), 10, "Should respect max_jobs_per_scan=10");

        let counts = count_by_priority(&due);
        assert!(
            counts[0].1 > 0,
            "Critical should be served under constrained concurrency"
        );

        if counts[3].1 == 0 && counts[4].1 == 0 {
            eprintln!(
                "ADVERSARIAL FINDING [rq-010]: Low+Background completely starved under scheduler overload"
            );
        }
    }

    #[tokio::test]
    async fn scheduler_starvation_test_progressive_drain() {
        let config = SchedulerConfig {
            max_concurrent: 10,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 20,
        };
        let mut scheduler = Scheduler::new(config);

        for priority in &ALL_PRIORITIES {
            for i in 0..20 {
                let id = (*priority as u64) * 1000 + i;
                let job = make_job(id, *priority, 0);
                scheduler.schedule(job).unwrap();
            }
        }

        let mut all_served: Vec<Job> = Vec::new();
        let mut rounds = 0;

        loop {
            let due = scheduler.poll_due_jobs(u64::MAX);
            if due.is_empty() {
                break;
            }
            all_served.extend(due);
            rounds += 1;
            if rounds > 50 {
                break;
            }
        }

        assert_eq!(
            all_served.len(),
            100,
            "All 100 jobs should be served after progressive drain"
        );

        let counts = count_by_priority(&all_served);
        for (priority, count) in &counts {
            assert_eq!(
                *count, 20,
                "All {:?} jobs should be served after full drain",
                priority
            );
        }
    }

    #[tokio::test]
    async fn scheduler_starvation_test_semaphore_does_not_bias_priority() {
        let config = SchedulerConfig {
            max_concurrent: 3,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let scheduler = Scheduler::new(config);

        let mut permits = Vec::new();
        for _ in 0..3 {
            let permit = scheduler.try_acquire();
            assert!(permit.is_some(), "Should acquire permit under limit");
            permits.push(permit);
        }

        let rejected = scheduler.try_acquire();
        assert!(
            rejected.is_none(),
            "Should reject at concurrency limit regardless of priority"
        );

        drop(permits);

        let permit = scheduler.try_acquire();
        assert!(
            permit.is_some(),
            "Should re-acquire after permits dropped"
        );
    }

    #[tokio::test]
    async fn scheduler_starvation_test_low_priority_eventually_served() {
        let config = SchedulerConfig {
            max_concurrent: 10,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 100,
        };
        let mut scheduler = Scheduler::new(config);

        let bg_job = make_job(9999, JobPriority::Background, 0);
        scheduler.schedule(bg_job).unwrap();

        for i in 0..50 {
            let job = make_job(i, JobPriority::Critical, 0);
            scheduler.schedule(job).unwrap();
        }

        let due = scheduler.poll_due_jobs(u64::MAX);

        let bg_served = due.iter().any(|j| j.priority == JobPriority::Background);
        let critical_count = due.iter().filter(|j| j.priority == JobPriority::Critical).count();

        if !bg_served {
            eprintln!(
                "ADVERSARIAL FINDING [rq-010]: Background job NOT served even though scan_limit=100 > total jobs"
            );
            eprintln!(
                "  {} Critical served, Background served: false",
                critical_count
            );
        }

        assert_eq!(due.len(), 51, "All jobs should be due");
        assert!(
            bg_served,
            "Background job should be served when scan_limit covers all jobs"
        );
    }

    #[tokio::test]
    async fn scheduler_starvation_test_repeated_overload_cycles() {
        let config = SchedulerConfig {
            max_concurrent: 10,
            scan_interval: Duration::from_millis(10),
            max_jobs_per_scan: 10,
        };
        let mut scheduler = Scheduler::new(config);

        for cycle in 0..5 {
            for priority in &ALL_PRIORITIES {
                for i in 0..20 {
                    let id = cycle * 100000 + (*priority as u64) * 1000 + i;
                    let job = make_job(id, *priority, 0);
                    scheduler.schedule(job).unwrap();
                }
            }
        }

        let mut total_per_priority = vec![0usize; 5];
        let mut rounds = 0;

        loop {
            let due = scheduler.poll_due_jobs(u64::MAX);
            if due.is_empty() {
                break;
            }
            for job in &due {
                total_per_priority[job.priority as usize] += 1;
            }
            rounds += 1;
            if rounds > 200 {
                break;
            }
        }

        let total = total_per_priority.iter().sum::<usize>();
        assert_eq!(total, 500, "All 500 jobs should be served across cycles");

        for (idx, count) in total_per_priority.iter().enumerate() {
            assert_eq!(
                *count, 100,
                "{:?} should have 100 served (20 per cycle * 5 cycles), got {}",
                ALL_PRIORITIES[idx], count
            );
        }
    }
}
