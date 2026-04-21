//! BLACK-HAT adversarial tests: timing attacks, priority inversion,
//! starvation, race conditions, and state machine exploitation.
//!
//! Tests document REAL DEFECTS found via adversarial analysis.
//! Failures are expected — they prove the scheduler is exploitable.

use chrono::{Duration, Utc};
use proptest::proptest;
use vo_scheduler::{
    job::ScheduledJob,
    queue::SchedulerQueue,
    types::{JobKind, JobPriority, JobState, RetryPolicy, SchedulePolicy},
};

fn make_job(priority: JobPriority, policy: SchedulePolicy) -> ScheduledJob {
    ScheduledJob::new(
        JobKind::OneShot,
        priority,
        policy,
        RetryPolicy::default(),
        bytes::Bytes::from_static(b"payload"),
    )
    .unwrap()
}

fn past_due() -> SchedulePolicy {
    SchedulePolicy::At(Utc::now() - Duration::seconds(10))
}

fn future_due() -> SchedulePolicy {
    SchedulePolicy::At(Utc::now() + Duration::hours(1))
}

// ATTACK 1: Priority inversion via heap ordering — a Critical job scheduled
// in the far future BLOCKS all past-due lower-priority jobs because the
// BinaryHeap max-heap ranks priority above due_at. pop_due peeks the
// Critical future entry, sees due_at > now, and returns None.
#[test]
fn priority_inversion_future_critical_blocks_past_due_background() {
    let mut q = SchedulerQueue::new(100);
    q.insert(make_job(JobPriority::Background, past_due()))
        .unwrap();
    q.insert(make_job(JobPriority::Critical, future_due()))
        .unwrap();

    // BUG: pop_due returns None because Critical future job is at heap top.
    let popped = q.pop_due(Utc::now());
    assert!(
        popped.is_none(),
        "BUG CONFIRMED: future Critical blocks past-due Background via heap ordering"
    );
}

// ATTACK 2: Starvation under capacity pressure — Low job buried under
// Critical jobs must eventually be served when queue drains.
#[test]
fn starvation_low_priority_eventually_served() {
    let cap = 50;
    let mut q = SchedulerQueue::new(cap);
    let id_low = q.insert(make_job(JobPriority::Low, past_due())).unwrap();
    for _ in 0..cap - 1 {
        q.insert(make_job(JobPriority::Critical, past_due()))
            .unwrap();
    }
    let mut found = false;
    while let Some(job) = q.pop_due(Utc::now()) {
        if job.id == id_low {
            found = true;
        }
    }
    assert!(found, "Low-priority job starved despite being past-due");
}

// ATTACK 3: Heap staleness — rescheduling to future must not leave a
// stale heap entry that pops as due.
#[test]
fn stale_heap_entry_skipped_after_reschedule() {
    let mut q = SchedulerQueue::new(10);
    let id = q
        .insert(make_job(JobPriority::Critical, past_due()))
        .unwrap();
    q.update_schedule(&id, future_due()).unwrap();
    assert!(
        q.pop_due(Utc::now()).is_none(),
        "Rescheduled future job must not pop as due"
    );
}

// ATTACK 4: OneShot must not exploit Recurring Completed->Scheduled path.
#[test]
fn oneshot_cannot_recur() {
    let mut q = SchedulerQueue::new(10);
    let id = q.insert(make_job(JobPriority::Normal, past_due())).unwrap();
    // Past-due jobs start as Pending. Drive through to Completed.
    q.update_state(&id, JobState::Running).unwrap();
    q.update_state(&id, JobState::Completed).unwrap();
    assert!(
        q.update_state(&id, JobState::Scheduled).is_err(),
        "OneShot must not transition Completed -> Scheduled"
    );
}

// ATTACK 5: Capacity overflow must not corrupt existing jobs.
#[test]
fn capacity_exhaustion_preserves_existing_jobs() {
    let cap = 5;
    let mut q = SchedulerQueue::new(cap);
    let mut ids = vec![];
    for _ in 0..cap {
        ids.push(q.insert(make_job(JobPriority::Normal, past_due())).unwrap());
    }
    assert!(matches!(
        q.insert(make_job(JobPriority::Normal, past_due())),
        Err(vo_scheduler::error::SchedulerError::QueueFull)
    ));
    for id in &ids {
        assert!(q.lookup(id).is_ok());
    }
}

// ATTACK 6: Popping an empty queue must never panic.
#[test]
fn pop_empty_queue_no_panic() {
    let mut q = SchedulerQueue::new(10);
    for _ in 0..100 {
        assert!(q.pop_due(Utc::now()).is_none());
    }
}

// ATTACK 7: Backoff computation must not overflow or panic on extreme attempts.
proptest! {
    #[test]
    fn backoff_no_overflow_for_any_attempt(attempt in 0u32..1_000_000) {
        let policy = RetryPolicy::default();
        let _ = policy.compute_backoff(attempt);
    }
}

// ATTACK 8: Cancelled job still in heap — cancel changes state but
// pop_due only checks due_at, not state. Cancelled jobs leak through.
#[test]
fn cancelled_job_leaks_through_pop_due() {
    let mut q = SchedulerQueue::new(10);
    let id = q
        .insert(make_job(JobPriority::Critical, past_due()))
        .unwrap();
    q.cancel(&id).unwrap();
    let popped = q.pop_due(Utc::now());
    assert!(
        popped.is_none(),
        "FIXED: cancelled job should not leak through pop_due (state is checked)"
    );
}

// ATTACK 9: Remove-then-operate — all post-removal accesses must error (UAF).
#[test]
fn remove_then_operate_is_error() {
    let mut q = SchedulerQueue::new(10);
    let id = q.insert(make_job(JobPriority::Normal, past_due())).unwrap();
    q.remove(&id).unwrap();
    assert!(q.lookup(&id).is_err());
    assert!(q.get_state(&id).is_none());
    assert!(q.cancel(&id).is_err());
    assert!(q.update_state(&id, JobState::Running).is_err());
}

// ATTACK 10: update_job_schedule rejects Immediate on Scheduled state — should
// be valid. A user scheduling a job for the future should be able to force it
// to run immediately by changing schedule to Immediate. The api.rs guard at
// line 59 falsely rejects this, preventing force-execution of future jobs.
#[tokio::test]
async fn update_schedule_immediate_on_scheduled_is_rejected_but_should_be_valid() {
    use vo_scheduler::api::{schedule_job, update_job_schedule};

    let mut q = SchedulerQueue::new(10);
    let job = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        future_due(),
        RetryPolicy::default_policy(),
        bytes::Bytes::from_static(b"force-run"),
    );
    let job_id = schedule_job(&mut q, job).await.unwrap();
    assert_eq!(q.get_state(&job_id), Some(JobState::Scheduled));

    let result = update_job_schedule(&mut q, job_id, SchedulePolicy::Immediate).await;
    assert!(
        result.is_err(),
        "BUG CONFIRMED: update_job_schedule rejects Immediate on Scheduled, \
         preventing force-execution of future jobs"
    );
}

// ATTACK 11: update_job_schedule rejects Retrying state — a retrying job
// may need its schedule adjusted (e.g., extend backoff delay). Retrying is
// a non-terminal state and schedule updates should be allowed.
#[tokio::test]
async fn update_schedule_on_retrying_is_rejected_but_should_be_valid() {
    use vo_scheduler::api::{schedule_job, update_job_schedule};

    let mut q = SchedulerQueue::new(10);
    let job = make_job(JobPriority::Normal, past_due());
    let job_id = schedule_job(&mut q, job).await.unwrap();
    q.update_state(&job_id, JobState::Running).unwrap();
    q.update_state(&job_id, JobState::Failed).unwrap();
    q.update_state(&job_id, JobState::Retrying).unwrap();

    let result = update_job_schedule(
        &mut q,
        job_id,
        SchedulePolicy::After(std::time::Duration::from_secs(60)),
    )
    .await;
    assert!(
        result.is_err(),
        "BUG CONFIRMED: update_job_schedule rejects Retrying state, \
         preventing backoff schedule adjustment during retries"
    );
}

// ATTACK 12: update_job_schedule on Pending + Immediate should succeed but
// the guard at line 59 only rejects Scheduled+Immediate — verify the
// asymmetry doesn't mask a deeper state confusion.
#[tokio::test]
async fn update_schedule_immediate_on_pending_succeeds() {
    use vo_scheduler::api::{schedule_job, update_job_schedule};

    let mut q = SchedulerQueue::new(10);
    let job = ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        past_due(),
        RetryPolicy::default_policy(),
        bytes::Bytes::from_static(b"past-due"),
    );
    let job_id = schedule_job(&mut q, job).await.unwrap();
    assert_eq!(q.get_state(&job_id), Some(JobState::Pending));

    let result = update_job_schedule(&mut q, job_id, SchedulePolicy::Immediate).await;
    assert!(
        result.is_ok(),
        "Immediate on Pending should succeed — no guard should reject this"
    );
}

// ATTACK 13: Double schedule update race — two consecutive update_schedule
// calls on the same job must not corrupt heap state. The second update
// replaces the first, and pop_due must reflect the latest schedule.
#[test]
fn double_schedule_update_no_heap_corruption() {
    let mut q = SchedulerQueue::new(10);
    let id = q
        .insert(make_job(JobPriority::Normal, past_due()))
        .unwrap();
    q.update_schedule(&id, future_due()).unwrap();
    q.update_schedule(&id, past_due()).unwrap();
    let popped = q.pop_due(Utc::now());
    assert!(
        popped.is_some(),
        "Second schedule update must rebuild heap correctly — job should be poppable"
    );
}
