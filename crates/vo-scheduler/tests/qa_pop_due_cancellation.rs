use chrono::{DateTime, Utc};
use vo_scheduler::job::ScheduledJob;
use vo_scheduler::queue::SchedulerQueue;
use vo_scheduler::types::{JobKind, JobPriority, JobState, RetryPolicy, SchedulePolicy};

fn make_job(priority: JobPriority, due_at: DateTime<Utc>, tag: &str) -> ScheduledJob {
    let mut job = ScheduledJob::new(
        JobKind::OneShot,
        priority,
        SchedulePolicy::At(due_at),
        RetryPolicy::default_policy(),
        tag.as_bytes().to_vec().into(),
    )
    .unwrap();
    job.due_at = due_at;
    job
}

fn now() -> DateTime<Utc> {
    Utc::now()
}

fn future() -> DateTime<Utc> {
    now() + chrono::Duration::hours(1)
}

fn past() -> DateTime<Utc> {
    now() - chrono::Duration::hours(1)
}

#[test]
fn pop_due_skips_single_cancelled_job_and_returns_next_due() {
    let mut q = SchedulerQueue::new(10);
    let cancelled = make_job(JobPriority::Critical, past(), "cancelled");
    let cancelled_id = cancelled.id;
    let due = make_job(JobPriority::Normal, past(), "due");
    let due_id = due.id;
    q.insert(cancelled).unwrap();
    q.insert(due).unwrap();
    q.cancel(&cancelled_id).unwrap();

    let popped = q.pop_due(now());
    assert!(popped.is_some(), "should return the non-cancelled due job");
    assert_eq!(popped.unwrap().id, due_id);
    assert_eq!(q.len(), 1);
}

#[test]
fn pop_due_skips_multiple_cancelled_jobs() {
    let mut q = SchedulerQueue::new(10);
    let c1 = make_job(JobPriority::Critical, past(), "c1");
    let c1_id = c1.id;
    let c2 = make_job(JobPriority::High, past(), "c2");
    let c2_id = c2.id;
    let due = make_job(JobPriority::Low, past(), "due");
    let due_id = due.id;
    q.insert(c1).unwrap();
    q.insert(c2).unwrap();
    q.insert(due).unwrap();
    q.cancel(&c1_id).unwrap();
    q.cancel(&c2_id).unwrap();

    let popped = q.pop_due(now());
    assert!(popped.is_some());
    assert_eq!(popped.unwrap().id, due_id);
}

#[test]
fn pop_due_returns_none_when_all_jobs_cancelled() {
    let mut q = SchedulerQueue::new(10);
    let j1 = make_job(JobPriority::Critical, past(), "j1");
    let j1_id = j1.id;
    let j2 = make_job(JobPriority::High, past(), "j2");
    let j2_id = j2.id;
    q.insert(j1).unwrap();
    q.insert(j2).unwrap();
    q.cancel(&j1_id).unwrap();
    q.cancel(&j2_id).unwrap();

    let popped = q.pop_due(now());
    assert!(popped.is_none(), "all jobs cancelled, nothing due");
}

#[test]
fn pop_due_skips_cancelled_but_preserves_future_job() {
    let mut q = SchedulerQueue::new(10);
    let cancelled = make_job(JobPriority::Critical, past(), "cancelled");
    let cancelled_id = cancelled.id;
    let future_job = make_job(JobPriority::Normal, future(), "future");
    let future_id = future_job.id;
    q.insert(cancelled).unwrap();
    q.insert(future_job).unwrap();
    q.cancel(&cancelled_id).unwrap();

    let popped = q.pop_due(now());
    assert!(popped.is_none(), "future job not yet due");
    assert!(q.lookup(&future_id).is_ok(), "future job should still be in queue");
    assert_eq!(q.len(), 2, "cancelled job stays in jobs map; future job also stays");
}

#[test]
fn pop_due_due_job_behind_future_job_after_cancelled_is_not_reached() {
    // FINDING: pop_due stops scanning after encountering the first non-due, non-cancelled
    // entry. If a cancelled job is at the top of the heap, and the next entry in the
    // heap is a future (not-due) job, pop_due pushes the future job back and breaks.
    // Any due jobs positioned AFTER the future job in the heap are NEVER reached.
    // This is because BinaryHeap has no guaranteed order beyond the max element.
    let mut q = SchedulerQueue::new(10);
    let cancelled = make_job(JobPriority::Critical, past(), "cancelled");
    let cancelled_id = cancelled.id;
    let due = make_job(JobPriority::Normal, past(), "due");
    let due_id = due.id;
    let future_job = make_job(JobPriority::High, future(), "future");
    let future_id = future_job.id;
    q.insert(cancelled).unwrap();
    q.insert(due).unwrap();
    q.insert(future_job).unwrap();
    q.cancel(&cancelled_id).unwrap();

    // pop_due skips cancelled, then hits future job, pushes it back and breaks.
    // The due job is unreachable.
    let popped = q.pop_due(now());
    assert!(
        popped.is_none(),
        "pop_due stops at first non-due entry after skipping cancelled"
    );

    // Verify both the due and future jobs are still in the queue
    assert!(q.lookup(&due_id).is_ok(), "due job is still in queue but unreachable via pop_due");
    assert!(q.lookup(&future_id).is_ok(), "future job preserved");
    assert_eq!(q.len(), 3);
}

#[test]
fn pop_due_maintains_priority_order_after_cancellation_skip() {
    let mut q = SchedulerQueue::new(10);
    let cancelled_crit = make_job(JobPriority::Critical, past(), "cancelled_crit");
    let cancelled_id = cancelled_crit.id;
    let high = make_job(JobPriority::High, past(), "high");
    let high_id = high.id;
    let normal = make_job(JobPriority::Normal, past(), "normal");
    let normal_id = normal.id;
    q.insert(cancelled_crit).unwrap();
    q.insert(normal).unwrap();
    q.insert(high).unwrap();
    q.cancel(&cancelled_id).unwrap();

    let first = q.pop_due(now()).unwrap();
    assert_eq!(first.id, high_id, "high should pop before normal");
    let second = q.pop_due(now()).unwrap();
    assert_eq!(second.id, normal_id);
    assert!(q.pop_due(now()).is_none());
}

#[test]
fn pop_due_handles_stale_heap_entry_gracefully() {
    let mut q = SchedulerQueue::new(10);
    let job = make_job(JobPriority::Normal, past(), "job");
    let job_id = job.id;
    q.insert(job).unwrap();

    let removed = q.remove(&job_id).unwrap();
    assert_eq!(removed.id, job_id);
    assert_eq!(q.len(), 0);

    let popped = q.pop_due(now());
    assert!(popped.is_none());
}

#[test]
fn pop_due_cleans_up_cancelled_job_from_heap() {
    let mut q = SchedulerQueue::new(10);
    let cancelled = make_job(JobPriority::Critical, past(), "cancelled");
    let cancelled_id = cancelled.id;
    let due = make_job(JobPriority::Normal, past(), "due");
    q.insert(cancelled).unwrap();
    q.insert(due).unwrap();
    q.cancel(&cancelled_id).unwrap();

    q.pop_due(now()).unwrap();

    assert_eq!(q.len(), 1);
    assert!(q.lookup(&cancelled_id).is_ok(), "cancelled job still tracked in jobs map");
}

#[test]
fn pop_due_does_not_remove_cancelled_job_from_jobs_map() {
    let mut q = SchedulerQueue::new(10);
    let job = make_job(JobPriority::Critical, past(), "job");
    let job_id = job.id;
    q.insert(job).unwrap();
    q.cancel(&job_id).unwrap();

    q.pop_due(now());

    assert!(q.lookup(&job_id).is_ok());
    assert_eq!(
        q.lookup(&job_id).unwrap().state,
        JobState::Cancelled
    );
}

#[test]
fn pop_due_empty_queue_returns_none() {
    let mut q = SchedulerQueue::new(10);
    assert!(q.pop_due(now()).is_none());
}

#[test]
fn pop_due_single_non_cancelled_due_job() {
    let mut q = SchedulerQueue::new(10);
    let job = make_job(JobPriority::Normal, past(), "job");
    let job_id = job.id;
    q.insert(job).unwrap();

    let popped = q.pop_due(now()).unwrap();
    assert_eq!(popped.id, job_id);
    assert!(q.pop_due(now()).is_none());
}

#[test]
fn pop_due_interleaved_cancelled_and_active_across_priorities() {
    let mut q = SchedulerQueue::new(10);
    let c_crit = make_job(JobPriority::Critical, past(), "c_crit");
    let c_crit_id = c_crit.id;
    let active_high = make_job(JobPriority::High, past(), "active_high");
    let active_high_id = active_high.id;
    let c_normal = make_job(JobPriority::Normal, past(), "c_normal");
    let c_normal_id = c_normal.id;
    let active_low = make_job(JobPriority::Low, past(), "active_low");
    let active_low_id = active_low.id;

    q.insert(c_crit).unwrap();
    q.insert(active_high).unwrap();
    q.insert(c_normal).unwrap();
    q.insert(active_low).unwrap();
    q.cancel(&c_crit_id).unwrap();
    q.cancel(&c_normal_id).unwrap();

    let first = q.pop_due(now()).unwrap();
    assert_eq!(first.id, active_high_id);
    let second = q.pop_due(now()).unwrap();
    assert_eq!(second.id, active_low_id);
    assert!(q.pop_due(now()).is_none());
}

#[test]
fn pop_due_after_multiple_cancel_still_pops_correct_order() {
    let mut q = SchedulerQueue::new(10);
    let a = make_job(JobPriority::Critical, past(), "a");
    let a_id = a.id;
    let b = make_job(JobPriority::High, past(), "b");
    let b_id = b.id;
    let c = make_job(JobPriority::Normal, past(), "c");
    let c_id = c.id;
    q.insert(a).unwrap();
    q.insert(b).unwrap();
    q.insert(c).unwrap();
    q.cancel(&b_id).unwrap();

    let first = q.pop_due(now()).unwrap();
    assert_eq!(first.id, a_id);
    let second = q.pop_due(now()).unwrap();
    assert_eq!(second.id, c_id);
    assert!(q.pop_due(now()).is_none());
}

#[test]
fn pop_due_exactly_at_due_time_pops() {
    let mut q = SchedulerQueue::new(10);
    let due_time = now();
    let job = make_job(JobPriority::Normal, due_time, "exact");
    let job_id = job.id;
    q.insert(job).unwrap();

    let popped = q.pop_due(due_time);
    assert!(popped.is_some(), "job due exactly at `now` should pop");
    assert_eq!(popped.unwrap().id, job_id);
}

#[test]
fn pop_due_one_millisecond_before_due_does_not_pop() {
    let mut q = SchedulerQueue::new(10);
    let due_time = now();
    let job = make_job(JobPriority::Normal, due_time, "just_after");
    q.insert(job).unwrap();

    let before = due_time - chrono::Duration::milliseconds(1);
    let popped = q.pop_due(before);
    assert!(popped.is_none(), "1ms before due time should not pop");
}

#[test]
fn pop_due_one_millisecond_after_due_pops() {
    let mut q = SchedulerQueue::new(10);
    let due_time = now();
    let job = make_job(JobPriority::Normal, due_time, "just_before");
    let job_id = job.id;
    q.insert(job).unwrap();

    let after = due_time + chrono::Duration::milliseconds(1);
    let popped = q.pop_due(after);
    assert!(popped.is_some(), "1ms after due time should pop");
    assert_eq!(popped.unwrap().id, job_id);
}

#[test]
fn cancel_removed_job_returns_error() {
    let mut q = SchedulerQueue::new(10);
    let job = make_job(JobPriority::Normal, past(), "job");
    let job_id = job.id;
    q.insert(job).unwrap();
    q.remove(&job_id).unwrap();

    let result = q.cancel(&job_id);
    assert!(result.is_err(), "cannot cancel a removed job");
}
