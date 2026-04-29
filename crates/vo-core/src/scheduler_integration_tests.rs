//! Integration tests for vo-core scheduler module re-exports.

use vo_core::scheduler::{
    CompletionResult, JobId, JobKind, JobState, InMemoryJobStore, RecordingDispatcher,
    ScheduledJob, Scheduler, SchedulePolicy, TickOutcome, WorkerDispatch,
};
use vo_scheduler::error::SchedulerError;

/// Verify all re-exported types are accessible through vo_core::scheduler.
#[test]
fn scheduler_module_reexports_all_types() {
    // Job domain types
    let _id = JobId::parse("test-job-1").unwrap();
    let _kind = JobKind::default();
    let _state = JobState::default();
    let _policy = SchedulePolicy::default();

    // Completion and tick types
    let _success = CompletionResult::Success;
    let _failed = CompletionResult::Failed {
        error: "test".to_string(),
    };
    let _cancelled = CompletionResult::Cancelled;
    let _outcome = TickOutcome::default();

    // Traits
    fn _assert_job_store<T: JobStore>(_s: &T) {}
    fn _assert_worker_dispatch<T: WorkerDispatch>(_d: &T) {}

    // In-memory implementations
    let _store = InMemoryJobStore::default();
    let _dispatcher = RecordingDispatcher::default();
}

/// Verify ScheduledJob can be constructed via re-exports.
#[test]
fn scheduled_job_constructible() {
    let id = JobId::parse("job-1").unwrap();
    let job = ScheduledJob::new(
        id.clone(),
        JobKind::default(),
        SchedulePolicy::default(),
    );
    assert_eq!(job.id, id);
    assert_eq!(job.state, JobState::Scheduled);
}

/// Verify InMemoryJobStore persists and retrieves jobs.
#[test]
fn in_memory_job_store_persist_and_retrieve() {
    let mut store = InMemoryJobStore::default();
    let id = JobId::parse("store-test-1").unwrap();
    let job = ScheduledJob::new(id.clone(), JobKind::default(), SchedulePolicy::default());

    store.persist(&job).unwrap();

    let retrieved = store.get(&id).unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, id);
}

/// Verify InMemoryJobStore removes jobs.
#[test]
fn in_memory_job_store_remove() {
    let mut store = InMemoryJobStore::default();
    let id = JobId::parse("store-test-2").unwrap();
    let job = ScheduledJob::new(id.clone(), JobKind::default(), SchedulePolicy::default());

    store.persist(&job).unwrap();
    store.remove(&id).unwrap();

    let retrieved = store.get(&id).unwrap();
    assert!(retrieved.is_none());
}

/// Verify RecordingDispatcher records dispatched jobs.
#[test]
fn recording_dispatcher_records_jobs() {
    let mut dispatcher = RecordingDispatcher::default();
    let id = JobId::parse("dispatch-test-1").unwrap();
    let job = ScheduledJob::new(id.clone(), JobKind::default(), SchedulePolicy::default());

    dispatcher.dispatch(&job).unwrap();

    let recorded = dispatcher.get_recorded();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].id, id);
}

/// Verify SchedulerError is accessible through re-exports.
#[test]
fn scheduler_error_accessible() {
    let err = SchedulerError::JobNotFound(JobId::parse("nonexistent").unwrap());
    let msg = format!("{}", err);
    assert!(!msg.is_empty());
}

/// Verify CompletionResult debug and clone.
#[test]
fn completion_result_debug_and_clone() {
    let success = CompletionResult::Success;
    let cloned = success.clone();
    assert_eq!(success, cloned);
    let _debug = format!("{:?}", success);

    let failed = CompletionResult::Failed {
        error: "test error".to_string(),
    };
    let failed_clone = failed.clone();
    assert_eq!(failed, failed_clone);
}

/// Verify TickOutcome fields are accessible.
#[test]
fn tick_outcome_fields() {
    let outcome = TickOutcome {
        promoted: 2,
        dispatched: 1,
        completed: 1,
        failed: 0,
        retried: 0,
        rescheduled: 0,
        cancelled: 0,
    };
    assert_eq!(outcome.promoted, 2);
    assert_eq!(outcome.dispatched, 1);
    assert_eq!(outcome.completed, 1);
    assert_eq!(outcome.failed, 0);
    assert_eq!(outcome.retried, 0);
    assert_eq!(outcome.rescheduled, 0);
    assert_eq!(outcome.cancelled, 0);
}
