//! Scheduler types per ADR-047.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Ulid);

impl JobId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JobState {
    Scheduled,
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Retrying,
}

impl JobState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobState::Completed | JobState::Failed | JobState::Cancelled
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JobKind {
    OneShot,
    Recurring,
    Delayed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SchedulePolicy {
    At(DateTime<Utc>),
    After(chrono::Duration),
    Cron(String),
    Immediate,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_multiplier: f64,
    pub initial_delay: chrono::Duration,
    pub max_delay: chrono::Duration,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JobPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}

impl JobPriority {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledJob {
    pub id: JobId,
    pub kind: JobKind,
    pub state: JobState,
    pub priority: JobPriority,
    pub schedule_policy: SchedulePolicy,
    pub retry_policy: RetryPolicy,
    pub attempt_count: u32,
    pub due_at: DateTime<Utc>,
    pub payload: SerializedPayload,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ScheduledJob {
    pub fn new(
        kind: JobKind,
        priority: JobPriority,
        schedule_policy: SchedulePolicy,
        retry_policy: RetryPolicy,
        payload: SerializedPayload,
    ) -> Self {
        let now = Utc::now();
        let due_at = match &schedule_policy {
            SchedulePolicy::At(dt) => *dt,
            SchedulePolicy::After(dur) => now + *dur,
            SchedulePolicy::Cron(_) => now,
            SchedulePolicy::Immediate => now,
        };
        Self {
            id: JobId::new(),
            kind,
            state: JobState::Scheduled,
            priority,
            schedule_policy,
            retry_policy,
            attempt_count: 0,
            due_at,
            payload,
            last_error: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedPayload(pub Vec<u8>);

impl SerializedPayload {
    pub fn new(data: Vec<u8>) -> Self {
        Self(data)
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone)]
pub struct SchedulerQueue {
    jobs: VecDeque<JobId>,
    by_id: HashMap<JobId, JobState>,
}

impl SchedulerQueue {
    pub fn new() -> Self {
        Self {
            jobs: VecDeque::new(),
            by_id: HashMap::new(),
        }
    }

    pub fn get_state(&self, job_id: &JobId) -> Option<JobState> {
        self.by_id.get(job_id).copied()
    }

    pub fn insert(&mut self, job_id: JobId, state: JobState) {
        self.by_id.insert(job_id, state);
        if !state.is_terminal() {
            self.jobs.push_back(job_id);
        }
    }
}

impl Default for SchedulerQueue {
    fn default() -> Self {
        Self::new()
    }
}
