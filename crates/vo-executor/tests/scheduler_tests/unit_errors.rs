use vo_executor::scheduler::{JobRunError, SchedulerError};
use vo_executor::JobId;

// =========================================================================
// SchedulerError Taxonomy Tests
// =========================================================================

#[test]
fn scheduler_error_job_not_found() {
    let err = SchedulerError::JobNotFound(JobId::new(42));
    let display = format!("{}", err);
    assert!(display.contains("42") || display.contains("not found"));
}

#[test]
fn scheduler_error_queue_full() {
    let err = SchedulerError::QueueFull;
    let display = format!("{}", err);
    assert!(display.contains("Queue") || display.contains("full"));
}

#[test]
fn scheduler_error_scheduler_stopped() {
    let err = SchedulerError::SchedulerStopped;
    let display = format!("{}", err);
    assert!(display.contains("stopped") || display.contains("Scheduler"));
}

#[test]
fn scheduler_error_invalid_schedule() {
    let err = SchedulerError::InvalidSchedule("bad cron".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Invalid") || display.contains("schedule"));
}

#[test]
fn scheduler_error_concurrency_limit_reached() {
    let err = SchedulerError::ConcurrencyLimitReached;
    let display = format!("{}", err);
    assert!(display.contains("Concurrency") || display.contains("limit"));
}

#[test]
fn scheduler_error_storage_error() {
    let err = SchedulerError::StorageError("disk full".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Storage") || display.contains("disk"));
}

// =========================================================================
// JobRunError Taxonomy Tests
// =========================================================================

#[test]
fn job_run_error_failed() {
    let err = JobRunError::Failed {
        job_id: JobId::new(1),
        reason: "oops".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("1") || display.contains("oops"));
}

#[test]
fn job_run_error_exceeded_retries() {
    let err = JobRunError::ExceededRetries {
        job_id: JobId::new(1),
        attempts: 3,
    };
    let display = format!("{}", err);
    assert!(display.contains("1") || display.contains("3"));
}

#[test]
fn job_run_error_cancelled() {
    let err = JobRunError::Cancelled {
        job_id: JobId::new(1),
    };
    let display = format!("{}", err);
    assert!(display.contains("1") || display.contains("ancelled"));
}
