//! Scheduler module — re-exports and integration layer for vo-scheduler.
//!
//! This module bridges `vo-core` to the standalone `vo-scheduler` crate,
//! providing a single entry point for all scheduler-related types.
//!
//! # Re-exported Types
//!
//! - [`Scheduler`] — Main tick-based scheduler loop
//! - [`JobStore`] — Trait for persistent job storage
//! - [`WorkerDispatch`] — Trait for dispatching jobs to workers
//! - [`JobId`, `JobKind`, `JobState`, `SchedulePolicy`] — Job domain types
//! - [`ScheduledJob`] — Persistent job representation
//! - [`CompletionResult`] — Worker completion outcome
//! - [`TickOutcome`] — Summary of a single scheduler tick
//! - [`InMemoryJobStore`] — In-memory job store for testing
//! - [`RecordingDispatcher`] — Test-friendly dispatcher
//! - [`SchedulerError`] — Error type for scheduler operations

pub use vo_scheduler::error::SchedulerError;
pub use vo_scheduler::job::ScheduledJob;
pub use vo_scheduler::queue::SchedulerQueue;
pub use vo_scheduler::scheduler::{
    CompletionResult, InMemoryJobStore, JobStore, RecordingDispatcher, Scheduler, TickOutcome,
    WorkerDispatch,
};
pub use vo_scheduler::types::{JobId, JobKind, JobState, SchedulePolicy};
