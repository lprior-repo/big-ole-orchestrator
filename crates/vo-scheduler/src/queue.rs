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
        other
            .priority
            .cmp(&self.priority)
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
        if self.jobs.len() >= self.capacity {
            return Err(SchedulerError::QueueFull);
        }
        let job_id = job.id;
        let entry = QueueEntry {
            priority: job.priority,
            due_at: job.due_at,
            job_id,
        };
        self.heap.push(entry);
        self.jobs.insert(job_id, job);
        Ok(job_id)
    }

    pub fn remove(&mut self, job_id: &JobId) -> Result<ScheduledJob, SchedulerError> {
        let job = self
            .jobs
            .remove(job_id)
            .ok_or(SchedulerError::JobNotFound)?;
        self.heap.retain(|entry| &entry.job_id != job_id);
        Ok(job)
    }

    pub fn lookup(&self, job_id: &JobId) -> Result<&ScheduledJob, SchedulerError> {
        self.jobs.get(job_id).ok_or(SchedulerError::JobNotFound)
    }

    pub fn lookup_mut(&mut self, job_id: &JobId) -> Result<&mut ScheduledJob, SchedulerError> {
        self.jobs.get_mut(job_id).ok_or(SchedulerError::JobNotFound)
    }

    pub fn get_state(&self, job_id: &JobId) -> Option<JobState> {
        self.jobs.get(job_id).map(|j| j.state)
    }

    pub fn update_state(
        &mut self,
        job_id: &JobId,
        new_state: JobState,
    ) -> Result<(), SchedulerError> {
        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or(SchedulerError::JobNotFound)?;
        job.transition(new_state)?;
        Ok(())
    }

    pub fn update_schedule(
        &mut self,
        job_id: &JobId,
        new_schedule: SchedulePolicy,
    ) -> Result<(), SchedulerError> {
        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or(SchedulerError::JobNotFound)?;
        if matches!(job.state, JobState::Running | JobState::Completed) {
            return Err(SchedulerError::InvalidTransition);
        }
        job.schedule_policy = new_schedule.clone();
        match new_schedule {
            SchedulePolicy::At(t) => job.due_at = t,
            SchedulePolicy::After(d) => {
                job.due_at = Utc::now() + chrono::Duration::from_std(d).unwrap_or_default()
            }
            SchedulePolicy::Immediate => job.due_at = Utc::now(),
            SchedulePolicy::Cron(_) => job.due_at = Utc::now(),
        }
        self.rebuild_heap_entry(job_id);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub fn pop_due(&mut self, now: DateTime<Utc>) -> Option<ScheduledJob> {
        loop {
            let entry = self.heap.peek()?.clone();
            if entry.due_at > now {
                return None;
            }
            self.heap.pop();
            let job_id = entry.job_id;
            if let Some(job) = self.jobs.get(&job_id) {
                if job.due_at <= now {
                    let job = self.jobs.remove(&job_id)?;
                    return Some(job);
                }
            }
        }
    }

    pub fn cancel(&mut self, job_id: &JobId) -> Result<(), SchedulerError> {
        let job = self.jobs.get(job_id).ok_or(SchedulerError::JobNotFound)?;
        if matches!(job.state, JobState::Completed | JobState::Failed) {
            return Err(SchedulerError::InvalidTransition);
        }
        self.update_state(job_id, JobState::Cancelled)
    }

    fn rebuild_heap_entry(&mut self, job_id: &JobId) {
        self.heap.retain(|entry| &entry.job_id != job_id);
        if let Some(job) = self.jobs.get(job_id) {
            let entry = QueueEntry {
                priority: job.priority,
                due_at: job.due_at,
                job_id: *job_id,
            };
            self.heap.push(entry);
        }
    }
}
