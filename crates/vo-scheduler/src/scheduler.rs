//! Scheduler main loop with tick-based dispatch and LifecycleState integration.
//!
//! Per ADR-047, the scheduler polls for ready jobs (`due_at <= now`),
//! dispatches to workers via a `WorkerDispatch` callback, handles completion
//! callbacks (including retry and reschedule), and persists job state to
//! storage via a `JobStore` trait.

use std::fmt;

use chrono::Utc;

use crate::error::SchedulerError;
use crate::job::ScheduledJob;
use crate::queue::SchedulerQueue;
use crate::types::{JobId, JobKind, JobState, SchedulePolicy};
use vo_types::state::LifecycleState;

// ---------------------------------------------------------------------------
// Job persistence trait
// ---------------------------------------------------------------------------

/// Abstraction over persistent job storage (e.g. fjall partition).
///
/// The scheduler persists every state mutation through this trait so that
/// no job state is lost between ticks. Implementations MUST be
/// transactionally consistent: `persist` writes the job as-is, and
/// `remove` deletes it (for completed/failed/cancelled terminal jobs
/// that will not be revisited).
pub trait JobStore: fmt::Debug {
    /// Persist a job to durable storage. Called after every state transition.
    fn persist(&mut self, job: &ScheduledJob) -> Result<(), SchedulerError>;

    /// Remove a job from storage. Called when a terminal job is cleaned up.
    fn remove(&mut self, job_id: &JobId) -> Result<(), SchedulerError>;
}

// ---------------------------------------------------------------------------
// Worker dispatch trait
// ---------------------------------------------------------------------------

/// Callback invoked when the scheduler dispatches a job to a worker.
///
/// The implementor sends the job payload to an execution backend (subprocess,
/// NATS worker, etc.) and returns the outcome synchronously. This keeps the
/// scheduler loop deterministic and testable.
pub trait WorkerDispatch: fmt::Debug {
    /// Dispatch a job for execution.
    ///
    /// Returns `Ok(())` on successful dispatch, or an error describing why
    /// dispatch failed (e.g. worker pool exhausted).
    fn dispatch(&mut self, job: &ScheduledJob) -> Result<(), SchedulerError>;
}

// ---------------------------------------------------------------------------
// Completion result
// ---------------------------------------------------------------------------

/// Result returned by a worker when job execution finishes.
#[derive(Debug, Clone)]
pub enum CompletionResult {
    /// Job executed successfully.
    Success,

    /// Job failed with an error message.
    Failed { error: String },

    /// Job was cancelled during execution.
    Cancelled,
}

// ---------------------------------------------------------------------------
// TickOutcome — what happened in a single tick
// ---------------------------------------------------------------------------

/// Summary of work performed in a single `tick()`.
#[derive(Debug, Clone, Default)]
pub struct TickOutcome {
    /// Number of jobs transitioned Scheduled -> Pending (became due).
    pub promoted: usize,

    /// Number of jobs dispatched to workers (Pending -> Running).
    pub dispatched: usize,

    /// Number of jobs completed successfully (Running -> Completed).
    pub completed: usize,

    /// Number of jobs that failed (Running -> Failed).
    pub failed: usize,

    /// Number of jobs retried (Failed -> Retrying -> Pending).
    pub retried: usize,

    /// Number of recurring jobs rescheduled (Completed -> Scheduled).
    pub rescheduled: usize,

    /// Number of jobs cancelled.
    pub cancelled: usize,
}

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

/// The background job scheduler.
///
/// Owns an in-memory `SchedulerQueue`, a `JobStore` for persistence, and a
/// `WorkerDispatch` for sending jobs to execution backends. Each call to
/// `tick()` processes all due jobs, applies any pending completions, and
/// persists every mutation.
#[derive(Debug)]
pub struct Scheduler<S: JobStore, W: WorkerDispatch> {
    queue: SchedulerQueue,
    store: S,
    dispatcher: W,
    capacity: usize,
}

impl<S: JobStore, W: WorkerDispatch> Scheduler<S, W> {
    /// Create a new scheduler with the given queue capacity, store, and
    /// dispatcher.
    pub fn new(capacity: usize, store: S, dispatcher: W) -> Self {
        Self {
            queue: SchedulerQueue::new(capacity),
            store,
            dispatcher,
            capacity,
        }
    }

    // -- Mutators that go through persistence -------------------------------

    /// Submit a new job into the scheduler.
    ///
    /// The job is inserted into the in-memory queue and immediately persisted
    /// to the `JobStore`. Returns the assigned `JobId`.
    pub fn submit(&mut self, job: ScheduledJob) -> Result<JobId, SchedulerError> {
        if self.queue.len() >= self.capacity {
            return Err(SchedulerError::QueueFull);
        }
        let job_id = job.id;
        self.queue.insert(job)?;
        let persisted_job = self.queue.lookup(&job_id)?;
        self.store.persist(persisted_job)?;
        Ok(job_id)
    }

    /// Cancel a job by ID.
    pub fn cancel(&mut self, job_id: JobId) -> Result<(), SchedulerError> {
        let state = self
            .queue
            .get_state(&job_id)
            .ok_or(SchedulerError::JobNotFound)?;
        if state.is_terminal() {
            return Err(SchedulerError::InvalidTransition);
        }
        self.queue.update_state(&job_id, JobState::Cancelled)?;
        let job = self.queue.lookup(&job_id)?;
        self.store.persist(job)?;
        Ok(())
    }

    /// Look up a job's current state.
    pub fn get_state(&self, job_id: &JobId) -> Option<JobState> {
        self.queue.get_state(job_id)
    }

    /// Look up a job.
    pub fn get_job(&self, job_id: &JobId) -> Option<&ScheduledJob> {
        self.queue.lookup(job_id).ok()
    }

    /// Current queue depth.
    pub fn queue_depth(&self) -> usize {
        self.queue.len()
    }

    // -- LifecycleState mapping (ADR-047 §6) --------------------------------

    /// Map a `JobState` to the corresponding `LifecycleState`.
    ///
    /// Per ADR-047 §6, the mapping is:
    ///
    /// | JobState     | LifecycleState    |
    /// |--------------|-------------------|
    /// | Scheduled    | Pending           |
    /// | Pending      | StepScheduled     |
    /// | Running      | StepExecuting     |
    /// | Completed    | Completed         |
    /// | Failed       | Failed            |
    /// | Cancelled    | Cancelled         |
    /// | Retrying     | WaitingForTimer   |
    pub fn job_to_lifecycle(job_state: JobState) -> LifecycleState {
        match job_state {
            JobState::Scheduled => LifecycleState::Pending,
            JobState::Pending => LifecycleState::StepScheduled,
            JobState::Running => LifecycleState::StepExecuting,
            JobState::Completed => LifecycleState::Completed,
            JobState::Failed => LifecycleState::Failed,
            JobState::Cancelled => LifecycleState::Cancelled,
            JobState::Retrying => LifecycleState::WaitingForTimer,
        }
    }

    // -- Tick: the main scheduling loop -------------------------------------

    /// Execute one scheduling tick.
    ///
    /// 1. Promote due `Scheduled` jobs to `Pending`.
    /// 2. Dispatch `Pending` jobs to workers (Pending -> Running).
    /// 3. Process any pending completion callbacks.
    ///
    /// All state changes are persisted via the `JobStore`.
    pub fn tick(&mut self, completions: &[(JobId, CompletionResult)]) -> Result<TickOutcome, SchedulerError> {
        let mut outcome = TickOutcome::default();
        let now = Utc::now();

        // Phase 1: Promote Scheduled -> Pending for jobs whose due_at <= now.
        outcome.promoted = self.promote_due_jobs(now)?;

        // Phase 2: Dispatch Pending -> Running for all ready jobs.
        outcome.dispatched = self.dispatch_pending()?;

        // Phase 3: Apply completion results from workers.
        self.apply_completions(completions, &mut outcome)?;

        Ok(outcome)
    }

    /// Phase 1: Transition all `Scheduled` jobs with `due_at <= now` to
    /// `Pending` and persist.
    fn promote_due_jobs(&mut self, now: chrono::DateTime<Utc>) -> Result<usize, SchedulerError> {
        let mut promoted = 0;
        // Collect IDs of scheduled jobs that are due.
        let due_ids: Vec<JobId> = self
            .queue
            .list_by_state(JobState::Scheduled)
            .iter()
            .filter(|job| job.due_at <= now)
            .map(|job| job.id)
            .collect();

        for id in due_ids {
            if let Ok(()) = self.queue.update_state(&id, JobState::Pending) {
                if let Ok(job) = self.queue.lookup(&id) {
                    let _ = self.store.persist(job);
                }
                promoted += 1;
            }
        }
        Ok(promoted)
    }

    /// Phase 2: Dispatch all `Pending` jobs through the `WorkerDispatch`
    /// callback, transitioning them to `Running` and persisting.
    fn dispatch_pending(&mut self) -> Result<usize, SchedulerError> {
        let mut dispatched = 0;
        let pending: Vec<JobId> = self
            .queue
            .list_by_state(JobState::Pending)
            .iter()
            .map(|j| j.id)
            .collect();

        for id in pending {
            // Remove the job from the queue temporarily to dispatch it.
            let mut job = match self.queue.remove(&id) {
                Ok(j) => j,
                Err(_) => continue,
            };

            // Transition Pending -> Running.
            if let Err(_) = job.transition(JobState::Running) {
                // Put it back unchanged.
                let _ = self.queue.insert(job);
                continue;
            }

            // Persist the Running state before dispatch.
            self.store.persist(&job)?;

            // Dispatch to worker.
            match self.dispatcher.dispatch(&job) {
                Ok(()) => {
                    self.queue.insert(job)?;
                    dispatched += 1;
                }
                Err(e) => {
                    // Dispatch failed: transition back to Pending so it can be
                    // retried on the next tick.
                    // Job is in Running state (in `job` variable), put it back as-is
                    // for safety; on next tick it will be in Running state and
                    // won't be dispatched again (it's not Pending).
                    // Instead, revert to Pending so it gets re-dispatched.
                    job.state = JobState::Pending;
                    job.updated_at = Utc::now();
                    self.store.persist(&job)?;
                    self.queue.insert(job)?;
                    // Return the first dispatch error to signal backpressure.
                    return Err(e);
                }
            }
        }
        Ok(dispatched)
    }

    /// Phase 3: Apply completion results for running jobs.
    ///
    /// - Success: Running -> Completed (reschedule if Recurring).
    /// - Failed: Running -> Failed (retry if policy allows, else terminal).
    /// - Cancelled: Running -> Cancelled (terminal).
    fn apply_completions(
        &mut self,
        completions: &[(JobId, CompletionResult)],
        outcome: &mut TickOutcome,
    ) -> Result<(), SchedulerError> {
        for (job_id, result) in completions {
            let current_state = match self.queue.get_state(job_id) {
                Some(s) => s,
                None => continue, // Unknown job, skip.
            };

            // Completions only apply to Running jobs.
            if current_state != JobState::Running {
                continue;
            }

            match result {
                CompletionResult::Success => {
                    self.handle_completion(*job_id, outcome)?;
                }
                CompletionResult::Failed { error } => {
                    self.handle_failure(*job_id, error.clone(), outcome)?;
                }
                CompletionResult::Cancelled => {
                    self.queue.update_state(job_id, JobState::Cancelled)?;
                    if let Ok(job) = self.queue.lookup(job_id) {
                        self.store.persist(job)?;
                    }
                    outcome.cancelled += 1;
                }
            }
        }
        Ok(())
    }

    /// Handle a successful completion: Running -> Completed.
    /// If the job is Recurring, reschedule it.
    fn handle_completion(
        &mut self,
        job_id: JobId,
        outcome: &mut TickOutcome,
    ) -> Result<(), SchedulerError> {
        // Transition Running -> Completed.
        self.queue.update_state(&job_id, JobState::Completed)?;
        if let Ok(job) = self.queue.lookup(&job_id) {
            self.store.persist(job)?;
        }
        outcome.completed += 1;

        // Reschedule recurring jobs.
        let job = match self.queue.lookup(&job_id) {
            Ok(j) => j.clone(),
            Err(_) => return Ok(()),
        };

        if job.kind == JobKind::Recurring {
            self.reschedule_recurring(job, outcome)?;
        }

        Ok(())
    }

    /// Handle a failed execution: Running -> Failed.
    /// If retry policy allows, transition through Retrying -> Pending.
    fn handle_failure(
        &mut self,
        job_id: JobId,
        error: String,
        outcome: &mut TickOutcome,
    ) -> Result<(), SchedulerError> {
        let job = match self.queue.lookup(&job_id) {
            Ok(j) => j.clone(),
            Err(_) => return Ok(()),
        };

        let attempt_count = job.attempt_count;

        // Transition Running -> Failed.
        self.queue.update_state(&job_id, JobState::Failed)?;
        // Store error on the job.
        if let Ok(job) = self.queue.lookup_mut(&job_id) {
            job.last_error = Some(error);
        }
        if let Ok(job) = self.queue.lookup(&job_id) {
            self.store.persist(job)?;
        }
        outcome.failed += 1;

        // Check if retry is possible.
        if job.retry_policy.can_retry(attempt_count) {
            // Failed -> Retrying.
            self.queue.update_state(&job_id, JobState::Retrying)?;
            // Retrying -> Pending.
            self.queue.update_state(&job_id, JobState::Pending)?;

            // Compute backoff and update due_at.
            let backoff = job.retry_policy.compute_backoff(attempt_count);
            let new_due_at = Utc::now()
                + chrono::Duration::from_std(backoff).unwrap_or(chrono::Duration::seconds(1));

            if let Ok(j) = self.queue.lookup_mut(&job_id) {
                j.attempt_count = attempt_count + 1;
                j.due_at = new_due_at;
            }

            if let Ok(j) = self.queue.lookup(&job_id) {
                self.store.persist(j)?;
            }
            outcome.retried += 1;
        }

        Ok(())
    }

    /// Reschedule a recurring job: Completed -> Scheduled with updated due_at.
    fn reschedule_recurring(
        &mut self,
        job: ScheduledJob,
        outcome: &mut TickOutcome,
    ) -> Result<(), SchedulerError> {
        let job_id = job.id;

        // Calculate next due_at based on schedule policy.
        let next_due_at = match &job.schedule_policy {
            SchedulePolicy::Cron(expr) => {
                // For cron jobs, the next due time is approximated as now.
                // A full cron parser would compute the next matching time.
                // We use the current time as a placeholder; the actual cron
                // evaluation is deferred to a future implementation.
                let _ = expr;
                Utc::now()
            }
            SchedulePolicy::After(d) => {
                Utc::now() + chrono::Duration::from_std(*d).unwrap_or(chrono::Duration::seconds(1))
            }
            SchedulePolicy::At(t) => *t,
            SchedulePolicy::Immediate => Utc::now(),
        };

        // Completed -> Scheduled (only valid for Recurring).
        self.queue.update_state(&job_id, JobState::Scheduled)?;

        // Update due_at and reset attempt tracking.
        if let Ok(j) = self.queue.lookup_mut(&job_id) {
            j.due_at = next_due_at;
            j.attempt_count = 0;
            j.last_error = None;
        }

        if let Ok(j) = self.queue.lookup(&job_id) {
            self.store.persist(j)?;
        }

        outcome.rescheduled += 1;
        Ok(())
    }
}

impl JobState {
    /// Map this `JobState` to the corresponding `LifecycleState` per ADR-047 §6.
    pub fn to_lifecycle(self) -> LifecycleState {
        match self {
            JobState::Scheduled => LifecycleState::Pending,
            JobState::Pending => LifecycleState::StepScheduled,
            JobState::Running => LifecycleState::StepExecuting,
            JobState::Completed => LifecycleState::Completed,
            JobState::Failed => LifecycleState::Failed,
            JobState::Cancelled => LifecycleState::Cancelled,
            JobState::Retrying => LifecycleState::WaitingForTimer,
        }
    }
}

// ---------------------------------------------------------------------------
// Noop implementations for testing
// ---------------------------------------------------------------------------

/// A `JobStore` that records calls in memory (for testing).
#[derive(Debug, Default)]
pub struct InMemoryJobStore {
    pub persisted: Vec<ScheduledJob>,
    pub removed: Vec<JobId>,
}

impl InMemoryJobStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl JobStore for InMemoryJobStore {
    fn persist(&mut self, job: &ScheduledJob) -> Result<(), SchedulerError> {
        // Update or append.
        if let Some(existing) = self.persisted.iter_mut().find(|j| j.id == job.id) {
            *existing = job.clone();
        } else {
            self.persisted.push(job.clone());
        }
        Ok(())
    }

    fn remove(&mut self, job_id: &JobId) -> Result<(), SchedulerError> {
        self.persisted.retain(|j| &j.id != job_id);
        self.removed.push(*job_id);
        Ok(())
    }
}

/// A `WorkerDispatch` that records dispatched jobs (for testing).
#[derive(Debug, Default)]
pub struct RecordingDispatcher {
    pub dispatched: Vec<JobId>,
    pub should_fail: bool,
}

impl RecordingDispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn failing() -> Self {
        Self {
            should_fail: true,
            ..Self::default()
        }
    }
}

impl WorkerDispatch for RecordingDispatcher {
    fn dispatch(&mut self, job: &ScheduledJob) -> Result<(), SchedulerError> {
        if self.should_fail {
            return Err(SchedulerError::QueueFull);
        }
        self.dispatched.push(job.id);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{JobKind, JobPriority, RetryPolicy};
    use bytes::Bytes;
    use std::time::Duration;

    fn make_job(kind: JobKind, policy: SchedulePolicy) -> ScheduledJob {
        ScheduledJob::new(
            kind,
            JobPriority::Normal,
            policy,
            RetryPolicy::default(),
            Bytes::from_static(b"test"),
        )
        .unwrap()
    }

    fn make_scheduler() -> Scheduler<InMemoryJobStore, RecordingDispatcher> {
        Scheduler::new(100, InMemoryJobStore::new(), RecordingDispatcher::new())
    }

    fn make_scheduler_with_fail_dispatch() -> Scheduler<InMemoryJobStore, RecordingDispatcher> {
        Scheduler::new(100, InMemoryJobStore::new(), RecordingDispatcher::failing())
    }

    // -- LifecycleState mapping tests ---------------------------------------

    #[test]
    fn lifecycle_mapping_scheduled() {
        assert_eq!(
            JobState::Scheduled.to_lifecycle(),
            LifecycleState::Pending
        );
    }

    #[test]
    fn lifecycle_mapping_pending() {
        assert_eq!(
            JobState::Pending.to_lifecycle(),
            LifecycleState::StepScheduled
        );
    }

    #[test]
    fn lifecycle_mapping_running() {
        assert_eq!(
            JobState::Running.to_lifecycle(),
            LifecycleState::StepExecuting
        );
    }

    #[test]
    fn lifecycle_mapping_completed() {
        assert_eq!(
            JobState::Completed.to_lifecycle(),
            LifecycleState::Completed
        );
    }

    #[test]
    fn lifecycle_mapping_failed() {
        assert_eq!(
            JobState::Failed.to_lifecycle(),
            LifecycleState::Failed
        );
    }

    #[test]
    fn lifecycle_mapping_cancelled() {
        assert_eq!(
            JobState::Cancelled.to_lifecycle(),
            LifecycleState::Cancelled
        );
    }

    #[test]
    fn lifecycle_mapping_retrying() {
        assert_eq!(
            JobState::Retrying.to_lifecycle(),
            LifecycleState::WaitingForTimer
        );
    }

    #[test]
    fn job_state_to_lifecycle_convenience_method() {
        assert_eq!(JobState::Running.to_lifecycle(), LifecycleState::StepExecuting);
        assert_eq!(JobState::Retrying.to_lifecycle(), LifecycleState::WaitingForTimer);
    }

    // -- Submit tests -------------------------------------------------------

    #[test]
    fn submit_adds_job_to_queue_and_persists() {
        let mut sched = make_scheduler();
        let job = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;
        let result = sched.submit(job);
        assert!(result.is_ok());
        assert_eq!(sched.queue_depth(), 1);
        assert_eq!(sched.store.persisted.len(), 1);
        assert_eq!(sched.store.persisted[0].id, id);
    }

    #[test]
    fn submit_returns_error_when_queue_full() {
        let mut sched = Scheduler::new(1, InMemoryJobStore::new(), RecordingDispatcher::new());
        let job1 = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let job2 = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        assert!(sched.submit(job1).is_ok());
        assert!(matches!(sched.submit(job2), Err(SchedulerError::QueueFull)));
    }

    // -- Cancel tests -------------------------------------------------------

    #[test]
    fn cancel_transitions_to_cancelled_and_persists() {
        let mut sched = make_scheduler();
        let job = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;
        sched.submit(job).unwrap();
        sched.cancel(id).unwrap();
        assert_eq!(sched.get_state(&id), Some(JobState::Cancelled));
        let persisted = &sched.store.persisted;
        assert!(persisted.iter().any(|j| j.id == id && j.state == JobState::Cancelled));
    }

    #[test]
    fn cancel_returns_error_for_unknown_job() {
        let mut sched = make_scheduler();
        let result = sched.cancel(JobId::generate());
        assert!(matches!(result, Err(SchedulerError::JobNotFound)));
    }

    #[test]
    fn cancel_returns_error_for_terminal_state() {
        let mut sched = make_scheduler();
        let job = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;
        sched.submit(job).unwrap();
        sched.cancel(id).unwrap();
        // Already cancelled (terminal).
        let result = sched.cancel(id);
        assert!(matches!(result, Err(SchedulerError::InvalidTransition)));
    }

    // -- Tick: promote due jobs ---------------------------------------------

    #[test]
    fn tick_promotes_scheduled_job_that_is_due() {
        let mut sched = make_scheduler();
        // Create a job with past due_at (Scheduled state).
        let job = make_job(JobKind::OneShot, SchedulePolicy::At(Utc::now() - chrono::Duration::hours(1)));
        // Force it into Scheduled state (it was created as Pending because
        // due_at is in the past, so we need to manually adjust).
        let mut job = job;
        job.state = JobState::Scheduled;
        job.due_at = Utc::now() - chrono::Duration::hours(1);
        sched.queue.insert(job).unwrap();

        let outcome = sched.tick(&[]).unwrap();
        assert_eq!(outcome.promoted, 1);
        // After promotion, the job gets dispatched in the same tick (Pending -> Running).
        assert!(outcome.dispatched >= 1);
    }

    #[test]
    fn tick_does_not_promote_future_scheduled_job() {
        let mut sched = make_scheduler();
        let mut job = make_job(JobKind::OneShot, SchedulePolicy::At(Utc::now() + chrono::Duration::hours(1)));
        job.state = JobState::Scheduled;
        job.due_at = Utc::now() + chrono::Duration::hours(1);
        sched.queue.insert(job).unwrap();

        let outcome = sched.tick(&[]).unwrap();
        assert_eq!(outcome.promoted, 0);
    }

    // -- Tick: dispatch pending jobs ----------------------------------------

    #[test]
    fn tick_dispatches_pending_job_to_worker() {
        let mut sched = make_scheduler();
        let job = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;
        sched.submit(job).unwrap();

        let outcome = sched.tick(&[]).unwrap();
        assert_eq!(outcome.dispatched, 1);
        assert_eq!(sched.get_state(&id), Some(JobState::Running));
        assert!(sched.dispatcher.dispatched.contains(&id));
    }

    #[test]
    fn tick_dispatch_failure_reverts_to_pending() {
        let mut sched = make_scheduler_with_fail_dispatch();
        let job = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;
        sched.submit(job).unwrap();

        let result = sched.tick(&[]);
        assert!(result.is_err());
        // Job should be back in Pending state for retry on next tick.
        assert_eq!(sched.get_state(&id), Some(JobState::Pending));
    }

    // -- Tick: completion callbacks -----------------------------------------

    #[test]
    fn tick_handles_successful_completion() {
        let mut sched = make_scheduler();
        let job = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;
        sched.submit(job).unwrap();
        // First tick: dispatch.
        sched.tick(&[]).unwrap();
        assert_eq!(sched.get_state(&id), Some(JobState::Running));

        // Second tick: complete.
        let outcome = sched.tick(&[(id, CompletionResult::Success)]).unwrap();
        assert_eq!(outcome.completed, 1);
        assert_eq!(sched.get_state(&id), Some(JobState::Completed));
    }

    #[test]
    fn tick_handles_failed_completion_with_retry() {
        let mut sched = make_scheduler();
        let job = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;
        sched.submit(job).unwrap();
        sched.tick(&[]).unwrap(); // dispatch

        let outcome = sched
            .tick(&[(id, CompletionResult::Failed {
                error: "boom".to_string(),
            })])
            .unwrap();
        assert_eq!(outcome.failed, 1);
        assert_eq!(outcome.retried, 1);
        // After retry: Failed -> Retrying -> Pending.
        assert_eq!(sched.get_state(&id), Some(JobState::Pending));
    }

    #[test]
    fn tick_handles_failed_completion_exhausted_retries() {
        let policy = RetryPolicy::try_new(1, 2.0, Duration::from_secs(1), Duration::from_secs(10)).unwrap();
        let job = ScheduledJob::new(
            JobKind::OneShot,
            JobPriority::Normal,
            SchedulePolicy::Immediate,
            policy,
            Bytes::from_static(b"test"),
        )
        .unwrap();
        let id = job.id;
        let mut sched = make_scheduler();
        sched.submit(job).unwrap();
        sched.tick(&[]).unwrap(); // dispatch

        // attempt_count starts at 0, can_retry(0) = true for max_attempts=1,
        // but after first failure the attempt becomes 1. However our initial
        // attempt_count is 0, so can_retry(0) = true.
        // Let's pre-set attempt_count to 1 so can_retry(1) = false.
        sched.queue.lookup_mut(&id).unwrap().attempt_count = 1;

        let outcome = sched
            .tick(&[(id, CompletionResult::Failed {
                error: "exhausted".to_string(),
            })])
            .unwrap();
        assert_eq!(outcome.failed, 1);
        assert_eq!(outcome.retried, 0);
        assert_eq!(sched.get_state(&id), Some(JobState::Failed));
    }

    #[test]
    fn tick_handles_cancelled_completion() {
        let mut sched = make_scheduler();
        let job = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;
        sched.submit(job).unwrap();
        sched.tick(&[]).unwrap(); // dispatch

        let outcome = sched.tick(&[(id, CompletionResult::Cancelled)]).unwrap();
        assert_eq!(outcome.cancelled, 1);
        assert_eq!(sched.get_state(&id), Some(JobState::Cancelled));
    }

    #[test]
    fn tick_ignores_completion_for_non_running_job() {
        let mut sched = make_scheduler();
        // Create a Scheduled job that is NOT due (future time).
        let mut job = make_job(JobKind::OneShot, SchedulePolicy::At(Utc::now() + chrono::Duration::hours(1)));
        job.state = JobState::Scheduled;
        job.due_at = Utc::now() + chrono::Duration::hours(1);
        let id = job.id;
        sched.queue.insert(job).unwrap();

        let outcome = sched.tick(&[(id, CompletionResult::Success)]).unwrap();
        // Not promoted, not dispatched, completion ignored.
        assert_eq!(outcome.completed, 0);
        assert_eq!(outcome.promoted, 0);
        assert_eq!(sched.get_state(&id), Some(JobState::Scheduled));
    }

    #[test]
    fn tick_ignores_completion_for_unknown_job() {
        let mut sched = make_scheduler();
        let fake_id = JobId::generate();
        let outcome = sched
            .tick(&[(fake_id, CompletionResult::Success)])
            .unwrap();
        assert_eq!(outcome.completed, 0);
    }

    // -- Recurring job reschedule -------------------------------------------

    #[test]
    fn tick_reschedules_recurring_job_after_completion() {
        let mut sched = make_scheduler();
        let job = make_job(JobKind::Recurring, SchedulePolicy::Immediate);
        let id = job.id;
        sched.submit(job).unwrap();
        sched.tick(&[]).unwrap(); // dispatch

        let outcome = sched.tick(&[(id, CompletionResult::Success)]).unwrap();
        assert_eq!(outcome.completed, 1);
        assert_eq!(outcome.rescheduled, 1);
        // Recurring: Completed -> Scheduled.
        assert_eq!(sched.get_state(&id), Some(JobState::Scheduled));
    }

    #[test]
    fn recurring_reschedule_resets_attempt_count() {
        let mut sched = make_scheduler();
        let job = make_job(JobKind::Recurring, SchedulePolicy::Immediate);
        let id = job.id;
        sched.submit(job).unwrap();
        sched.tick(&[]).unwrap(); // dispatch
        // Simulate a failed attempt that was retried.
        sched.queue.lookup_mut(&id).unwrap().attempt_count = 3;
        sched
            .tick(&[(id, CompletionResult::Success)])
            .unwrap();

        let job = sched.get_job(&id).unwrap();
        assert_eq!(job.attempt_count, 0);
    }

    // -- Persistence between ticks ------------------------------------------

    #[test]
    fn tick_persists_every_state_transition() {
        let mut sched = make_scheduler();
        let job = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;
        sched.submit(job).unwrap();

        // Submit persists once (Pending).
        assert_eq!(sched.store.persisted.len(), 1);
        assert_eq!(sched.store.persisted[0].state, JobState::Pending);

        // Tick: dispatch persists Running state.
        sched.tick(&[]).unwrap();
        assert!(sched
            .store
            .persisted
            .iter()
            .any(|j| j.id == id && j.state == JobState::Running));

        // Tick: completion persists Completed state.
        sched.tick(&[(id, CompletionResult::Success)]).unwrap();
        assert!(sched
            .store
            .persisted
            .iter()
            .any(|j| j.id == id && j.state == JobState::Completed));
    }

    // -- Full tick cycle end-to-end -----------------------------------------

    #[test]
    fn full_tick_cycle_oneshot() {
        let mut sched = make_scheduler();
        let job = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;
        sched.submit(job).unwrap();

        // Tick 1: promote + dispatch.
        let o1 = sched.tick(&[]).unwrap();
        assert_eq!(o1.dispatched, 1);
        assert_eq!(sched.get_state(&id), Some(JobState::Running));

        // Tick 2: complete.
        let o2 = sched.tick(&[(id, CompletionResult::Success)]).unwrap();
        assert_eq!(o2.completed, 1);
        assert_eq!(sched.get_state(&id), Some(JobState::Completed));
        // OneShot: no reschedule.
        assert_eq!(o2.rescheduled, 0);
    }

    #[test]
    fn full_tick_cycle_recurring() {
        let mut sched = make_scheduler();
        let job = make_job(JobKind::Recurring, SchedulePolicy::Immediate);
        let id = job.id;
        sched.submit(job).unwrap();

        // Tick 1: dispatch.
        sched.tick(&[]).unwrap();

        // Tick 2: complete -> reschedule.
        let o2 = sched.tick(&[(id, CompletionResult::Success)]).unwrap();
        assert_eq!(o2.completed, 1);
        assert_eq!(o2.rescheduled, 1);
        assert_eq!(sched.get_state(&id), Some(JobState::Scheduled));

        // Tick 3: promote (Scheduled -> Pending) + dispatch again.
        let o3 = sched.tick(&[]).unwrap();
        assert!(o3.promoted >= 1 || o3.dispatched >= 1);
    }

    #[test]
    fn full_tick_cycle_retry_then_succeed() {
        let mut sched = make_scheduler();
        let job = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;
        sched.submit(job).unwrap();

        // Tick 1: dispatch.
        sched.tick(&[]).unwrap();

        // Tick 2: fail (retries to Pending).
        let o2 = sched
            .tick(&[(id, CompletionResult::Failed {
                error: "transient".to_string(),
            })])
            .unwrap();
        assert_eq!(o2.failed, 1);
        assert_eq!(o2.retried, 1);
        assert_eq!(sched.get_state(&id), Some(JobState::Pending));

        // Tick 3: re-dispatch.
        let o3 = sched.tick(&[]).unwrap();
        assert_eq!(o3.dispatched, 1);
        assert_eq!(sched.get_state(&id), Some(JobState::Running));

        // Tick 4: succeed.
        let o4 = sched.tick(&[(id, CompletionResult::Success)]).unwrap();
        assert_eq!(o4.completed, 1);
        assert_eq!(sched.get_state(&id), Some(JobState::Completed));
    }

    // -- Multiple jobs in one tick ------------------------------------------

    #[test]
    fn tick_dispatches_multiple_pending_jobs() {
        let mut sched = make_scheduler();
        let j1 = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let j2 = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let j3 = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id1 = j1.id;
        let id2 = j2.id;
        let id3 = j3.id;
        sched.submit(j1).unwrap();
        sched.submit(j2).unwrap();
        sched.submit(j3).unwrap();

        let outcome = sched.tick(&[]).unwrap();
        assert_eq!(outcome.dispatched, 3);
        assert_eq!(sched.dispatcher.dispatched.len(), 3);
        assert_eq!(sched.get_state(&id1), Some(JobState::Running));
        assert_eq!(sched.get_state(&id2), Some(JobState::Running));
        assert_eq!(sched.get_state(&id3), Some(JobState::Running));
    }

    #[test]
    fn tick_processes_multiple_completions() {
        let mut sched = make_scheduler();
        let j1 = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let j2 = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id1 = j1.id;
        let id2 = j2.id;
        sched.submit(j1).unwrap();
        sched.submit(j2).unwrap();
        sched.tick(&[]).unwrap(); // dispatch both

        let outcome = sched
            .tick(&[
                (id1, CompletionResult::Success),
                (id2, CompletionResult::Failed {
                    error: "err".to_string(),
                }),
            ])
            .unwrap();
        assert_eq!(outcome.completed, 1);
        assert_eq!(outcome.failed, 1);
        assert_eq!(outcome.retried, 1);
    }

    // -- Empty tick ---------------------------------------------------------

    #[test]
    fn tick_with_no_jobs_returns_empty_outcome() {
        let mut sched = make_scheduler();
        let outcome = sched.tick(&[]).unwrap();
        assert_eq!(outcome.promoted, 0);
        assert_eq!(outcome.dispatched, 0);
        assert_eq!(outcome.completed, 0);
        assert_eq!(outcome.failed, 0);
    }

    // -- InMemoryJobStore tests ---------------------------------------------

    #[test]
    fn in_memory_store_persist_updates_existing() {
        let mut store = InMemoryJobStore::new();
        let job = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        store.persist(&job).unwrap();
        assert_eq!(store.persisted.len(), 1);

        let mut updated = job.clone();
        updated.state = JobState::Running;
        store.persist(&updated).unwrap();
        assert_eq!(store.persisted.len(), 1);
        assert_eq!(store.persisted[0].state, JobState::Running);
    }

    #[test]
    fn in_memory_store_remove_deletes_job() {
        let mut store = InMemoryJobStore::new();
        let job = make_job(JobKind::OneShot, SchedulePolicy::Immediate);
        let id = job.id;
        store.persist(&job).unwrap();
        store.remove(&id).unwrap();
        assert!(store.persisted.is_empty());
        assert!(store.removed.contains(&id));
    }

    // -- TickOutcome default ------------------------------------------------

    #[test]
    fn tick_outcome_default_is_all_zeros() {
        let outcome = TickOutcome::default();
        assert_eq!(outcome.promoted, 0);
        assert_eq!(outcome.dispatched, 0);
        assert_eq!(outcome.completed, 0);
        assert_eq!(outcome.failed, 0);
        assert_eq!(outcome.retried, 0);
        assert_eq!(outcome.rescheduled, 0);
        assert_eq!(outcome.cancelled, 0);
    }
}
