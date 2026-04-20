//! Priority queue for job scheduling
//!
//! SchedulerQueue aligns to ADR-047 with HashMap<JobId, JobState> tracking.

use crate::scheduler::types::{Job, JobId, JobState};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

#[derive(Debug, Clone)]
struct QueuedJob {
    job: Job,
    fire_at_ms: u64,
}

impl PartialEq for QueuedJob {
    fn eq(&self, other: &Self) -> bool {
        self.job.id == other.job.id
    }
}

impl Eq for QueuedJob {}

impl Ord for QueuedJob {
    fn cmp(&self, other: &Self) -> Ordering {
        // Priority queue is a max-heap
        // Higher priority (lower enum value) should come out first
        match self.job.priority.cmp(&other.job.priority) {
            Ordering::Equal => {
                // If same priority, earlier fire time comes first
                // For max-heap: smaller fire_at = earlier = should come out first = greater
                other.fire_at_ms.cmp(&self.fire_at_ms)
            }
            // Reverse for max-heap: lower enum value = higher priority = should come out first
            Ordering::Less => Ordering::Greater,
            Ordering::Greater => Ordering::Less,
        }
    }
}

impl PartialOrd for QueuedJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Default)]
pub struct PriorityQueue {
    heap: BinaryHeap<QueuedJob>,
}

impl PriorityQueue {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, job: Job, fire_at_ms: u64) {
        self.heap.push(QueuedJob { job, fire_at_ms });
    }

    pub fn pop(&mut self) -> Option<(Job, u64)> {
        self.heap.pop().map(|qj| (qj.job, qj.fire_at_ms))
    }

    pub fn peek(&self) -> Option<&Job> {
        self.heap.peek().map(|qj| &qj.job)
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn remove(&mut self, job_id: &JobId) -> Option<Job> {
        // BinaryHeap doesn't support remove, so we need to rebuild
        // This is O(n) but acceptable for a scheduler
        let mut found = None;
        let jobs: Vec<_> = self
            .heap
            .drain()
            .filter(|qj| {
                if qj.job.id == *job_id {
                    found = Some(qj.job.clone());
                    false
                } else {
                    true
                }
            })
            .collect();
        self.heap.extend(jobs);
        found
    }

    pub fn due_jobs(&self, now_ms: u64, max: u32) -> Vec<(Job, u64)> {
        let mut due: Vec<(Job, u64)> = self
            .heap
            .iter()
            .filter(|qj| qj.fire_at_ms <= now_ms)
            .map(|qj| (qj.job.clone(), qj.fire_at_ms))
            .collect();
        due.sort_by(|a, b| match a.0.priority.cmp(&b.0.priority) {
            Ordering::Equal => a.1.cmp(&b.1),
            ord => ord,
        });
        due.truncate(max as usize);
        due
    }

    pub fn pop_due_jobs(&mut self, now_ms: u64, max: u32) -> Vec<Job> {
        let mut results = Vec::new();
        let mut not_due: Vec<QueuedJob> = Vec::new();
        let max = max as usize;

        while let Some(qj) = self.heap.pop() {
            if qj.fire_at_ms <= now_ms && results.len() < max {
                results.push(qj.job);
            } else {
                not_due.push(qj.clone());
                if qj.fire_at_ms > now_ms {
                    break;
                }
            }
        }

        for qj in not_due {
            self.heap.push(qj);
        }

        results
    }

    #[cfg(test)]
    pub fn into_vec(self) -> Vec<Job> {
        self.heap.into_iter().map(|qj| qj.job).collect()
    }
}

#[derive(Debug)]
pub struct SchedulerQueue {
    jobs: PriorityQueue,
    by_id: HashMap<JobId, JobState>,
}

impl SchedulerQueue {
    pub fn new() -> Self {
        Self {
            jobs: PriorityQueue::new(),
            by_id: HashMap::new(),
        }
    }

    pub fn push(&mut self, job: Job, fire_at_ms: u64) {
        self.by_id.insert(job.id, JobState::Scheduled);
        self.jobs.push(job, fire_at_ms);
    }

    pub fn pop(&mut self) -> Option<(Job, u64)> {
        self.jobs.pop().map(|(job, fire_at)| {
            self.by_id.insert(job.id, JobState::Pending);
            (job, fire_at)
        })
    }

    pub fn get_state(&self, job_id: &JobId) -> Option<JobState> {
        self.by_id.get(job_id).copied()
    }

    pub fn set_state(&mut self, job_id: JobId, state: JobState) {
        self.by_id.insert(job_id, state);
    }

    pub fn remove(&mut self, job_id: &JobId) -> Option<Job> {
        self.by_id.remove(job_id);
        self.jobs.remove(job_id)
    }

    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub fn due_jobs(&self, now_ms: u64, max: u32) -> Vec<(Job, u64)> {
        self.jobs.due_jobs(now_ms, max)
    }

    pub fn pop_due_jobs(&mut self, now_ms: u64, max: u32) -> Vec<Job> {
        self.jobs.pop_due_jobs(now_ms, max)
    }

    pub fn reschedule(&mut self, job: Job, next_fire_ms: u64) {
        self.push(job, next_fire_ms);
    }
}

impl Default for SchedulerQueue {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for SchedulerQueue {}
unsafe impl Sync for SchedulerQueue {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{JobPriority, Schedule};

    fn _make_job(id: u64, priority: JobPriority, fire_at_ms: u64) -> (Job, u64) {
        let job = Job::new(
            JobId::new(id),
            format!("payload-{}", id),
            crate::scheduler::Schedule::OneShot { fire_at_ms },
        )
        .with_priority(priority);
        (job, fire_at_ms)
    }

    #[test]
    fn priority_queue_ordering() {
        let mut pq = PriorityQueue::new();

        // Add jobs with different priorities
        pq.push(
            Job::new(
                JobId::new(1),
                "low".to_string(),
                crate::scheduler::Schedule::one_shot(std::time::Duration::from_secs(0)),
            )
            .with_priority(JobPriority::Low),
            100,
        );
        pq.push(
            Job::new(
                JobId::new(2),
                "high".to_string(),
                crate::scheduler::Schedule::one_shot(std::time::Duration::from_secs(0)),
            )
            .with_priority(JobPriority::High),
            100,
        );
        pq.push(
            Job::new(
                JobId::new(3),
                "critical".to_string(),
                crate::scheduler::Schedule::one_shot(std::time::Duration::from_secs(0)),
            )
            .with_priority(JobPriority::Critical),
            100,
        );

        // Should pop in priority order
        let (job1, _) = pq.pop().unwrap();
        assert_eq!(job1.id, JobId::new(3)); // Critical first

        let (job2, _) = pq.pop().unwrap();
        assert_eq!(job2.id, JobId::new(2)); // High second

        let (job3, _) = pq.pop().unwrap();
        assert_eq!(job3.id, JobId::new(1)); // Low last
    }

    #[test]
    fn priority_queue_same_priority_fire_time() {
        let mut pq = PriorityQueue::new();
        let now = 1000u64;

        pq.push(
            Job::new(
                JobId::new(1),
                "job1".to_string(),
                crate::scheduler::Schedule::one_shot(std::time::Duration::from_secs(0)),
            ),
            now + 200,
        );
        pq.push(
            Job::new(
                JobId::new(2),
                "job2".to_string(),
                crate::scheduler::Schedule::one_shot(std::time::Duration::from_secs(0)),
            ),
            now + 100,
        );

        let (job, _) = pq.pop().unwrap();
        assert_eq!(job.id, JobId::new(2)); // Earlier fire time first
    }

    #[test]
    fn priority_queue_remove() {
        let mut pq = PriorityQueue::new();

        pq.push(
            Job::new(
                JobId::new(1),
                "job1".to_string(),
                crate::scheduler::Schedule::one_shot(std::time::Duration::from_secs(0)),
            ),
            100,
        );
        pq.push(
            Job::new(
                JobId::new(2),
                "job2".to_string(),
                crate::scheduler::Schedule::one_shot(std::time::Duration::from_secs(0)),
            ),
            100,
        );

        let removed = pq.remove(&JobId::new(1));
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, JobId::new(1));
        assert_eq!(pq.len(), 1);

        let remaining = pq.pop().unwrap().0;
        assert_eq!(remaining.id, JobId::new(2));
    }

    #[test]
    fn priority_queue_due_jobs() {
        let mut pq = PriorityQueue::new();
        let now = 1000u64;

        pq.push(
            Job::new(
                JobId::new(1),
                "low".to_string(),
                crate::scheduler::Schedule::one_shot(std::time::Duration::from_secs(0)),
            )
            .with_priority(JobPriority::Low),
            now - 50,
        );
        pq.push(
            Job::new(
                JobId::new(2),
                "high".to_string(),
                crate::scheduler::Schedule::one_shot(std::time::Duration::from_secs(0)),
            )
            .with_priority(JobPriority::High),
            now - 50,
        );
        pq.push(
            Job::new(
                JobId::new(3),
                "not-due".to_string(),
                crate::scheduler::Schedule::one_shot(std::time::Duration::from_secs(0)),
            ),
            now + 50,
        );
        pq.push(
            Job::new(
                JobId::new(4),
                "critical".to_string(),
                crate::scheduler::Schedule::one_shot(std::time::Duration::from_secs(0)),
            )
            .with_priority(JobPriority::Critical),
            now - 50,
        );

        let due: Vec<_> = pq.due_jobs(now, 10);
        assert_eq!(due.len(), 3);
        assert_eq!(due[0].0.id, JobId::new(4), "Critical should be first");
        assert_eq!(due[1].0.id, JobId::new(2), "High should be second");
        assert_eq!(due[2].0.id, JobId::new(1), "Low should be last");
    }

    #[test]
    fn priority_queue_due_jobs_sorted_by_fire_time() {
        let mut pq = PriorityQueue::new();
        let now = 1000u64;

        pq.push(
            Job::new(
                JobId::new(1),
                "later".to_string(),
                crate::scheduler::Schedule::one_shot(std::time::Duration::from_secs(0)),
            ),
            now - 10,
        );
        pq.push(
            Job::new(
                JobId::new(2),
                "earlier".to_string(),
                crate::scheduler::Schedule::one_shot(std::time::Duration::from_secs(0)),
            ),
            now - 100,
        );

        let due: Vec<_> = pq.due_jobs(now, 10);
        assert_eq!(due.len(), 2);
        assert_eq!(due[0].0.id, JobId::new(2), "Earlier fire time first");
        assert_eq!(due[1].0.id, JobId::new(1), "Later fire time second");
    }

    #[test]
    fn qa_full_priority_chain_all_five_levels() {
        let mut pq = PriorityQueue::new();
        let fire = 100u64;

        pq.push(
            Job::new(JobId::new(4), "background".into(), Schedule::OneShot { fire_at_ms: fire })
                .with_priority(JobPriority::Background),
            fire,
        );
        pq.push(
            Job::new(JobId::new(3), "low".into(), Schedule::OneShot { fire_at_ms: fire })
                .with_priority(JobPriority::Low),
            fire,
        );
        pq.push(
            Job::new(JobId::new(2), "normal".into(), Schedule::OneShot { fire_at_ms: fire })
                .with_priority(JobPriority::Normal),
            fire,
        );
        pq.push(
            Job::new(JobId::new(1), "high".into(), Schedule::OneShot { fire_at_ms: fire })
                .with_priority(JobPriority::High),
            fire,
        );
        pq.push(
            Job::new(JobId::new(0), "critical".into(), Schedule::OneShot { fire_at_ms: fire })
                .with_priority(JobPriority::Critical),
            fire,
        );

        let ids: Vec<u64> = std::iter::from_fn(|| pq.pop().map(|(j, _)| j.id.0))
            .collect();
        assert_eq!(ids, vec![0, 1, 2, 3, 4], "Must dequeue Critical..Background");
    }

    #[test]
    fn qa_fifo_tiebreak_same_priority_and_fire_time() {
        let mut pq = PriorityQueue::new();
        let fire = 500u64;
        let pri = JobPriority::Normal;

        for id in 10u64..=15 {
            pq.push(
                Job::new(JobId::new(id), format!("job-{}", id), Schedule::OneShot { fire_at_ms: fire })
                    .with_priority(pri),
                fire,
            );
        }

        let first = pq.pop().unwrap().0;
        assert_eq!(first.id, JobId::new(10), "FIFO: first inserted should pop first among ties");
    }

    #[test]
    fn qa_pop_due_jobs_respects_max_limit() {
        let mut pq = PriorityQueue::new();
        let now = 1000u64;

        for id in 1u64..=10 {
            pq.push(
                Job::new(JobId::new(id), format!("job-{}", id), Schedule::OneShot { fire_at_ms: now - 50 })
                    .with_priority(JobPriority::Normal),
                now - 50,
            );
        }

        let due = pq.pop_due_jobs(now, 3);
        assert_eq!(due.len(), 3, "Must return at most max jobs");
        assert_eq!(pq.len(), 7, "Remaining 7 jobs must stay in queue");
    }

    #[test]
    fn qa_pop_due_jobs_returns_priority_order() {
        let mut pq = PriorityQueue::new();
        let now = 1000u64;

        pq.push(
            Job::new(JobId::new(1), "low".into(), Schedule::OneShot { fire_at_ms: now - 10 })
                .with_priority(JobPriority::Low),
            now - 10,
        );
        pq.push(
            Job::new(JobId::new(2), "critical".into(), Schedule::OneShot { fire_at_ms: now - 10 })
                .with_priority(JobPriority::Critical),
            now - 10,
        );
        pq.push(
            Job::new(JobId::new(3), "normal".into(), Schedule::OneShot { fire_at_ms: now - 10 })
                .with_priority(JobPriority::Normal),
            now - 10,
        );

        let due = pq.pop_due_jobs(now, 10);
        assert_eq!(due.len(), 3);
        assert_eq!(due[0].id, JobId::new(2), "Critical first via pop_due_jobs");
        assert_eq!(due[1].id, JobId::new(3), "Normal second via pop_due_jobs");
        assert_eq!(due[2].id, JobId::new(1), "Low last via pop_due_jobs");
    }

    #[test]
    fn qa_pop_due_jobs_does_not_return_future_jobs() {
        let mut pq = PriorityQueue::new();
        let now = 1000u64;

        pq.push(
            Job::new(JobId::new(1), "past".into(), Schedule::OneShot { fire_at_ms: now - 100 }),
            now - 100,
        );
        pq.push(
            Job::new(JobId::new(2), "future".into(), Schedule::OneShot { fire_at_ms: now + 9999 }),
            now + 9999,
        );
        pq.push(
            Job::new(JobId::new(3), "exact-now".into(), Schedule::OneShot { fire_at_ms: now }),
            now,
        );

        let due = pq.pop_due_jobs(now, 10);
        let due_ids: Vec<u64> = due.iter().map(|j| j.id.0).collect();
        assert_eq!(due_ids, vec![1, 3], "Only past and exact-now jobs should be due");
        assert_eq!(pq.len(), 1, "Future job must remain in queue");
    }

    #[test]
    fn qa_pop_due_jobs_excess_due_jobs_stay_in_queue() {
        let mut pq = PriorityQueue::new();
        let now = 1000u64;

        pq.push(
            Job::new(JobId::new(1), "critical".into(), Schedule::OneShot { fire_at_ms: now })
                .with_priority(JobPriority::Critical),
            now,
        );
        pq.push(
            Job::new(JobId::new(2), "high".into(), Schedule::OneShot { fire_at_ms: now })
                .with_priority(JobPriority::High),
            now,
        );
        pq.push(
            Job::new(JobId::new(3), "normal".into(), Schedule::OneShot { fire_at_ms: now })
                .with_priority(JobPriority::Normal),
            now,
        );

        let due = pq.pop_due_jobs(now, 1);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, JobId::new(1), "Must get the Critical job");

        assert_eq!(pq.len(), 2, "Excess due jobs must stay in queue");

        let remaining = pq.pop_due_jobs(now, 10);
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].id, JobId::new(2), "High next");
        assert_eq!(remaining[1].id, JobId::new(3), "Normal last");
    }

    #[test]
    fn qa_empty_queue_operations() {
        let mut pq = PriorityQueue::new();
        assert!(pq.is_empty());
        assert_eq!(pq.len(), 0);
        assert!(pq.pop().is_none());
        assert!(pq.peek().is_none());
        assert!(pq.due_jobs(0, 10).is_empty());
        assert!(pq.pop_due_jobs(0, 10).is_empty());
        assert!(pq.remove(&JobId::new(1)).is_none());
    }

    #[test]
    fn qa_single_item_lifecycle() {
        let mut pq = PriorityQueue::new();
        pq.push(
            Job::new(JobId::new(42), "only".into(), Schedule::OneShot { fire_at_ms: 100 }),
            100,
        );
        assert_eq!(pq.len(), 1);
        assert!(!pq.is_empty());

        let peeked = pq.peek().unwrap();
        assert_eq!(peeked.id, JobId::new(42));

        let (job, fire) = pq.pop().unwrap();
        assert_eq!(job.id, JobId::new(42));
        assert_eq!(fire, 100);
        assert!(pq.is_empty());
    }

    #[test]
    fn qa_scheduler_queue_state_tracking() {
        let mut sq = SchedulerQueue::new();

        sq.push(
            Job::new(JobId::new(1), "test".into(), Schedule::OneShot { fire_at_ms: 100 }),
            100,
        );
        assert_eq!(sq.get_state(&JobId::new(1)), Some(JobState::Scheduled));

        let (job, _) = sq.pop().unwrap();
        assert_eq!(job.id, JobId::new(1));
        assert_eq!(sq.get_state(&JobId::new(1)), Some(JobState::Pending));

        sq.set_state(JobId::new(1), JobState::Running);
        assert_eq!(sq.get_state(&JobId::new(1)), Some(JobState::Running));

        sq.set_state(JobId::new(1), JobState::Completed);
        assert_eq!(sq.get_state(&JobId::new(1)), Some(JobState::Completed));
    }

    #[test]
    fn qa_scheduler_queue_remove_clears_state() {
        let mut sq = SchedulerQueue::new();
        sq.push(
            Job::new(JobId::new(1), "test".into(), Schedule::OneShot { fire_at_ms: 100 }),
            100,
        );
        assert_eq!(sq.get_state(&JobId::new(1)), Some(JobState::Scheduled));

        let removed = sq.remove(&JobId::new(1));
        assert!(removed.is_some());
        assert_eq!(sq.get_state(&JobId::new(1)), None, "State must be cleared on remove");
        assert!(sq.is_empty());
    }

    #[test]
    fn qa_reschedule_preserves_ordering() {
        let mut sq = SchedulerQueue::new();
        let now = 1000u64;

        sq.push(
            Job::new(JobId::new(1), "first".into(), Schedule::OneShot { fire_at_ms: now - 100 })
                .with_priority(JobPriority::High),
            now - 100,
        );
        sq.push(
            Job::new(JobId::new(2), "second".into(), Schedule::OneShot { fire_at_ms: now - 100 })
                .with_priority(JobPriority::Low),
            now - 100,
        );

        assert_eq!(sq.len(), 2, "Should have 2 jobs before pop");

        let due = sq.pop_due_jobs(now, 1);
        assert_eq!(due.len(), 1, "Should get 1 due job");
        assert_eq!(due[0].id, JobId::new(1), "High priority first");

        assert_eq!(sq.len(), 1, "1 job should remain after pop_due_jobs(max=1)");

        sq.reschedule(due[0].clone(), now + 500);
        assert_eq!(sq.len(), 2, "2 jobs after reschedule");

        let remaining = sq.pop_due_jobs(now, 10);
        assert!(!remaining.is_empty(), "Should have remaining due job: len={}", sq.len());
        assert_eq!(remaining[0].id, JobId::new(2), "Low priority still there");

        let after_reschedule = sq.pop_due_jobs(now + 600, 10);
        assert!(!after_reschedule.is_empty(), "Should have rescheduled job");
        assert_eq!(after_reschedule[0].id, JobId::new(1), "Rescheduled High comes back");
    }

    #[test]
    fn qa_pop_due_jobs_drain_rebuild_correctness() {
        let mut pq = PriorityQueue::new();
        let now = 1000u64;

        pq.push(
            Job::new(JobId::new(1), "high".into(), Schedule::OneShot { fire_at_ms: now - 100 })
                .with_priority(JobPriority::High),
            now - 100,
        );
        pq.push(
            Job::new(JobId::new(2), "low".into(), Schedule::OneShot { fire_at_ms: now - 100 })
                .with_priority(JobPriority::Low),
            now - 100,
        );

        let first = pq.pop_due_jobs(now, 1);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, JobId::new(1));

        assert_eq!(pq.len(), 1, "One job should remain");

        let second = pq.pop_due_jobs(now, 10);
        assert_eq!(second.len(), 1, "Second job should be due: pq.len()={}", pq.len());
        assert_eq!(second[0].id, JobId::new(2), "Low job should come out");
    }

    #[test]
    fn qa_due_jobs_and_pop_due_jobs_agree() {
        let mut pq = PriorityQueue::new();
        let now = 1000u64;

        for id in 1u64..=8 {
            let pri = match id % 4 {
                0 => JobPriority::Critical,
                1 => JobPriority::High,
                2 => JobPriority::Normal,
                3 => JobPriority::Low,
                _ => unreachable!(),
            };
            pq.push(
                Job::new(JobId::new(id), format!("job-{}", id), Schedule::OneShot { fire_at_ms: now - 10 })
                    .with_priority(pri),
                now - 10,
            );
        }

        let due_ref: Vec<u64> = pq.due_jobs(now, 8).iter().map(|(j, _)| j.id.0).collect();

        let mut pq2 = PriorityQueue::new();
        for id in 1u64..=8 {
            let pri = match id % 4 {
                0 => JobPriority::Critical,
                1 => JobPriority::High,
                2 => JobPriority::Normal,
                3 => JobPriority::Low,
                _ => unreachable!(),
            };
            pq2.push(
                Job::new(JobId::new(id), format!("job-{}", id), Schedule::OneShot { fire_at_ms: now - 10 })
                    .with_priority(pri),
                now - 10,
            );
        }
        let due_pop: Vec<u64> = pq2.pop_due_jobs(now, 8).iter().map(|j| j.id.0).collect();

        assert_eq!(due_ref, due_pop, "due_jobs and pop_due_jobs must agree on ordering");
    }

    #[test]
    fn qa_peek_does_not_remove() {
        let mut pq = PriorityQueue::new();
        pq.push(
            Job::new(JobId::new(1), "test".into(), Schedule::OneShot { fire_at_ms: 100 })
                .with_priority(JobPriority::Critical),
            100,
        );

        let peeked = pq.peek().unwrap();
        assert_eq!(peeked.id, JobId::new(1));
        assert_eq!(pq.len(), 1, "peek must not remove");

        let peeked2 = pq.peek().unwrap();
        assert_eq!(peeked2.id, JobId::new(1));

        let popped = pq.pop().unwrap().0;
        assert_eq!(popped.id, JobId::new(1));
        assert_eq!(pq.len(), 0);
    }
}
