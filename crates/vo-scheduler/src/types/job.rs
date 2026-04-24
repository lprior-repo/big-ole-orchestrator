use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::SchedulerError;
use crate::types::priority::JobPriority;
use crate::types::{JobId, JobKind, JobState, RetryPolicy, SchedulePolicy};

pub type SerializedPayload = bytes::Bytes;

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
    ) -> Result<Self, SchedulerError> {
        if let SchedulePolicy::Cron(ref expr) = schedule_policy {
            SchedulePolicy::validate_cron(expr)?;
        }
        let now = Utc::now();
        let due_at = match &schedule_policy {
            SchedulePolicy::At(t) => *t,
            SchedulePolicy::After(d) => now + chrono::Duration::from_std(*d).unwrap_or_default(),
            SchedulePolicy::Immediate => now,
            SchedulePolicy::Cron(_) => now,
        };
        let state = if due_at <= now {
            JobState::Pending
        } else {
            JobState::Scheduled
        };
        Ok(Self {
            id: JobId::generate(),
            kind,
            state,
            priority,
            schedule_policy,
            retry_policy,
            attempt_count: 0,
            due_at,
            payload,
            last_error: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn transition(&mut self, new_state: JobState) -> Result<(), SchedulerError> {
        let valid = match (&self.state, &new_state) {
            (JobState::Scheduled, JobState::Pending) => true,
            (JobState::Scheduled, JobState::Cancelled) => true,
            (JobState::Pending, JobState::Running) => true,
            (JobState::Pending, JobState::Cancelled) => true,
            (JobState::Running, JobState::Completed) => true,
            (JobState::Running, JobState::Failed) => true,
            (JobState::Running, JobState::Cancelled) => true,
            (JobState::Failed, JobState::Retrying) => true,
            (JobState::Retrying, JobState::Pending) => true,
            (JobState::Retrying, JobState::Cancelled) => true,
            (JobState::Completed, JobState::Scheduled) => self.kind == JobKind::Recurring,
            _ => false,
        };
        if !valid {
            return Err(SchedulerError::InvalidTransition);
        }
        self.state = new_state;
        self.updated_at = Utc::now();
        Ok(())
    }
}