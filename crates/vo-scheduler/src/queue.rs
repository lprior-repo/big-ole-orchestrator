use std::collections::{BinaryHeap, HashMap};

use chrono::{DateTime, Utc};

use crate::error::SchedulerError;
use crate::job::ScheduledJob;
use crate::types::{JobId, JobPriority, JobState, SchedulePolicy};

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueueEntry {
    priority: JobPriority,
    due_at: DateTime<Utc>,
    job_id: JobId,
}

impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.due_at.cmp(&self.due_at))
    }
}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
pub struct SchedulerQueue {
    heap: BinaryHeap<QueueEntry>,
    jobs: HashMap<JobId, ScheduledJob>,
    capacity: usize,
}

impl SchedulerQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            heap: BinaryHeap::new(),
            jobs: HashMap::new(),
            capacity,
        }
    }

    pub fn insert(&mut self, job: ScheduledJob) -> Result<JobId, SchedulerError> {
        let _ = (&mut self.heap, &mut self.jobs, &mut self.capacity, &job);
        todo!("TDD-RED: insert not yet implemented")
    }

    pub fn remove(&mut self, job_id: &JobId) -> Result<ScheduledJob, SchedulerError> {
        let _ = (&mut self.heap, &mut self.jobs, job_id);
        todo!("TDD-RED: remove not yet implemented")
    }

    pub fn lookup(&self, job_id: &JobId) -> Result<&ScheduledJob, SchedulerError> {
        let _ = (&self.jobs, job_id);
        todo!("TDD-RED: lookup not yet implemented")
    }

    pub fn lookup_mut(&mut self, job_id: &JobId) -> Result<&mut ScheduledJob, SchedulerError> {
        let _ = (&mut self.jobs, &mut self.heap, job_id);
        todo!("TDD-RED: lookup_mut not yet implemented")
    }

    pub fn update_state(
        &mut self,
        job_id: &JobId,
        new_state: JobState,
    ) -> Result<(), SchedulerError> {
        let _ = (&mut self.jobs, &mut self.heap, job_id, new_state);
        todo!("TDD-RED: update_state not yet implemented")
    }

    pub fn update_schedule(
        &mut self,
        job_id: &JobId,
        new_schedule: SchedulePolicy,
    ) -> Result<(), SchedulerError> {
        let _ = (&mut self.jobs, &mut self.heap, job_id, &new_schedule);
        todo!("TDD-RED: update_schedule not yet implemented")
    }

    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub fn pop_due(&mut self, now: DateTime<Utc>) -> Option<ScheduledJob> {
        let _ = (&mut self.heap, &mut self.jobs, now);
        todo!("TDD-RED: pop_due not yet implemented")
    }

    pub fn cancel(&mut self, job_id: &JobId) -> Result<(), SchedulerError> {
        let _ = (&mut self.jobs, &mut self.heap, job_id);
        todo!("TDD-RED: cancel not yet implemented")
    }
}
