//! vo-scheduler crate - Background Job Scheduler (ADR-047)
//!
//! This crate manages background job scheduling for workflow instances.

mod api;
mod error;
mod types;

pub use error::{ExecutionError, RetryExhaustedError, SchedulerError};
pub use types::{
    JobId, JobKind, JobPriority, JobState, RetryPolicy, ScheduledJob, SchedulePolicy, SchedulerQueue,
};

pub use api::{cancel_job, get_job_status, schedule_job, update_job_schedule};
