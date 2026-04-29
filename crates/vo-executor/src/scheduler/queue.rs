//! Priority queue for job scheduling
//!
//! SchedulerQueue aligns to ADR-047 with HashMap<JobId, JobState> tracking.

use crate::scheduler::types::{Job, JobId, JobState, SchedulePolicy};
use chrono::{DateTime, Utc};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::time::Duration;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JobPriority;

    fn _make_job(id: u64, priority: JobPriority, fire_at_ms: u64) -> (Job, u64) {
        let fire_at =
            DateTime::from_timestamp(fire_at_ms / 1000, ((fire_at_ms % 1000) * 1_000_000) as u32)
                .unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap());
        let job = Job::new(
            JobId::new(id),
            format!("payload-{}", id),
            SchedulePolicy::At(fire_at),
        )
        .with_priority(priority);
        (job, fire_at_ms)
    }

    #[test]
    fn priority_queue_ordering() {
        let mut pq = PriorityQueue::new();

        // Add jobs with different priorities
        pq.push(
            Job::new(JobId::new(1), "low".to_string(), SchedulePolicy::Immediate)
                .with_priority(JobPriority::Low),
            100,
        );
        pq.push(
            Job::new(JobId::new(2), "high".to_string(), SchedulePolicy::Immediate)
                .with_priority(JobPriority::High),
            100,
        );
        pq.push(
            Job::new(
                JobId::new(3),
                "critical".to_string(),
                SchedulePolicy::Immediate,
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
            Job::new(JobId::new(1), "job1".to_string(), SchedulePolicy::Immediate),
            now + 200,
        );
        pq.push(
            Job::new(JobId::new(2), "job2".to_string(), SchedulePolicy::Immediate),
            now + 100,
        );

        let (job, _) = pq.pop().unwrap();
        assert_eq!(job.id, JobId::new(2)); // Earlier fire time first
    }

    #[test]
    fn priority_queue_remove() {
        let mut pq = PriorityQueue::new();

        pq.push(
            Job::new(JobId::new(1), "job1".to_string(), SchedulePolicy::Immediate),
            100,
        );
        pq.push(
            Job::new(JobId::new(2), "job2".to_string(), SchedulePolicy::Immediate),
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
            Job::new(JobId::new(1), "low".to_string(), SchedulePolicy::Immediate)
                .with_priority(JobPriority::Low),
            now - 50,
        );
        pq.push(
            Job::new(JobId::new(2), "high".to_string(), SchedulePolicy::Immediate)
                .with_priority(JobPriority::High),
            now - 50,
        );
        pq.push(
            Job::new(
                JobId::new(3),
                "not-due".to_string(),
                SchedulePolicy::Immediate,
            ),
            now + 50,
        );
        pq.push(
            Job::new(
                JobId::new(4),
                "critical".to_string(),
                SchedulePolicy::Immediate,
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
                SchedulePolicy::Immediate,
            ),
            now - 10,
        );
        pq.push(
            Job::new(
                JobId::new(2),
                "earlier".to_string(),
                SchedulePolicy::Immediate,
            ),
            now - 100,
        );

        let due: Vec<_> = pq.due_jobs(now, 10);
        assert_eq!(due.len(), 2);
        assert_eq!(due[0].0.id, JobId::new(2), "Earlier fire time first");
        assert_eq!(due[1].0.id, JobId::new(1), "Later fire time second");
    }
}
