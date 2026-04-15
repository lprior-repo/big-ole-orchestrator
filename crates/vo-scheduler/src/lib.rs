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

use dashmap::DashMap;
use std::sync::LazyLock;

static JOB_REGISTRY: LazyLock<DashMap<JobId, JobState>, fn() -> DashMap<JobId, JobState>> =
    LazyLock::new(DashMap::new);

pub(crate) fn registry_get_state(job_id: &JobId) -> Option<JobState> {
    JOB_REGISTRY.get(job_id).map(|r| *r.value())
}

pub(crate) fn registry_set_state(job_id: JobId, state: JobState) {
    JOB_REGISTRY.insert(job_id, state);
}
