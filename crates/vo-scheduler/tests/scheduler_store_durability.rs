//! BDD test: tw-4y6h.12.4 — Durable scheduler store boundary
//!
//! Scenario: Given a job is scheduled,
//!   When schedule_job returns,
//!   Then the job is durable in SchedulerStore and survives reopen/restart.

use vo_scheduler::job::ScheduledJob;
use vo_scheduler::store::{InMemorySchedulerStore, SchedulerStore};
use vo_scheduler::types::{JobId, JobKind, JobPriority, JobState, RetryPolicy, SchedulePolicy};

fn make_job() -> ScheduledJob {
    ScheduledJob::new(
        JobKind::OneShot,
        JobPriority::Normal,
        SchedulePolicy::Immediate,
        RetryPolicy::default(),
        bytes::Bytes::from_static(b"durability-proof"),
    )
    .unwrap()
}

#[test]
fn given_job_scheduled_when_store_reopens_then_job_is_still_present() {
    let job_id: JobId;

    let serialized = {
        let mut store = InMemorySchedulerStore::new();
        let job = make_job();
        job_id = job.id;
        store.put(job).unwrap();

        let retrieved = store.get(&job_id).unwrap().unwrap();
        assert_eq!(
            retrieved.state,
            JobState::Pending,
            "job must be Pending after put"
        );

        store.serialized.clone()
    };

    let reopened_store = InMemorySchedulerStore { serialized };

    let survived = reopened_store
        .get(&job_id)
        .expect("get must succeed on reopened store")
        .expect("job must survive reopen");

    assert_eq!(survived.id, job_id, "job ID must match after reopen");
    assert_eq!(
        survived.state,
        JobState::Pending,
        "state must survive reopen"
    );
    assert_eq!(
        survived.payload.as_ref(),
        b"durability-proof",
        "payload must survive reopen"
    );
    assert_eq!(survived.kind, JobKind::OneShot);
    assert_eq!(survived.priority, JobPriority::Normal);
}

#[test]
fn given_multiple_jobs_when_store_reopens_then_all_survive() {
    let job1 = make_job();
    let job2 = make_job();
    let job3 = make_job();
    let ids: Vec<JobId> = vec![job1.id, job2.id, job3.id];

    let serialized = {
        let mut store = InMemorySchedulerStore::new();
        store.put(job1).unwrap();
        store.put(job2).unwrap();
        store.put(job3).unwrap();
        store.serialized.clone()
    };

    let reopened = InMemorySchedulerStore { serialized };
    let all = reopened.list_all().unwrap();
    assert_eq!(all.len(), 3, "all 3 jobs must survive reopen");

    for id in &ids {
        assert!(
            reopened.contains(id).unwrap(),
            "job {id} must be present after reopen"
        );
    }
}

#[test]
fn given_state_transition_when_store_reopens_then_new_state_persists() {
    let job = make_job();
    let job_id = job.id;

    let serialized = {
        let mut store = InMemorySchedulerStore::new();
        store.put(job).unwrap();

        let mut retrieved = store.get(&job_id).unwrap().unwrap();
        retrieved.transition(JobState::Running).unwrap();
        store.update(retrieved).unwrap();

        let after = store.get(&job_id).unwrap().unwrap();
        assert_eq!(after.state, JobState::Running);

        store.serialized.clone()
    };

    let reopened = InMemorySchedulerStore { serialized };
    let survived = reopened.get(&job_id).unwrap().unwrap();
    assert_eq!(
        survived.state,
        JobState::Running,
        "state transition must survive reopen"
    );
}
