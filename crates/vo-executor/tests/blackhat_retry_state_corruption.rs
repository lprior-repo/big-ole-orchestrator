//! BLACKHAT bh-018: Adversarial tests for retry state corruption
//!
//! Attacks:
//! 1. TOCTOU race in execute_step — concurrent double-execution via DashMap check-then-act
//! 2. Concurrent retry exhaustion — race between two retry loops on same step
//! 3. Error map / state map inconsistency under concurrent writes
//! 4. State clobbering — late writer overwrites an already-transitioned state

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use vo_executor::{
    execute_step_with_retry, get_last_error, reset_all_state, RetryPolicy, StepId,
};
use vo_executor::state::{get_state, StepState};

/// CONCURRENT-001: TOCTOU race in execute_step
///
/// Attack: Two tasks call execute_step("step-1") simultaneously.
/// check_not_executing uses a non-atomic DashMap read, then start_execution writes.
/// If both pass the check before either writes, both proceed to "execute".
///
/// Expected: One succeeds, one gets InvalidTransition. But the DashMap gap
/// makes this non-deterministic — the blackhat test PROVES whether the race exists.
#[tokio::test]
async fn bh_toctou_concurrent_execute_step_double_execution() {
    use vo_executor::execute_step;

    reset_all_state();

    let completions = Arc::new(AtomicU32::new(0));

    let c1 = completions.clone();
    let c2 = completions.clone();

    let h1 = tokio::spawn(async move {
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        if result.is_ok() {
            c1.fetch_add(1, Ordering::SeqCst);
        }
    });

    let h2 = tokio::spawn(async move {
        let result = execute_step(StepId::new("step-1".to_string()), 5000).await;
        if result.is_ok() {
            c2.fetch_add(1, Ordering::SeqCst);
        }
    });

    let _ = h1.await;
    let _ = h2.await;

    let total_completions = completions.load(Ordering::SeqCst);
    let final_state = get_state("step-1");

    // INVARIANT: At most one execution should succeed.
    // If both succeeded, the TOCTOU race was exploited.
    assert!(
        total_completions <= 1,
        "RACE CONDITION DETECTED: {} tasks completed (expected ≤1). \
         The check_not_executing → start_execution gap allows double execution.",
        total_completions
    );

    // INVARIANT: Final state must be Ready (clean terminal state).
    assert!(
        matches!(final_state, StepState::Ready),
        "final state is {:?}, expected Ready",
        final_state
    );
}

/// CONCURRENT-002: Concurrent retry loops on the same step
///
/// Attack: Two tasks both call execute_step_with_retry("step-flaky") simultaneously.
/// Both enter the flaky path and independently record errors / retry counts.
/// The global DashMap error entry is overwritten by whichever finishes last.
///
/// Expected: Both should get RetryExhausted. The error map should have a consistent
/// entry (not a partial write from one and count from another).
#[tokio::test]
async fn bh_concurrent_retry_exhaustion_error_map_consistency() {
    reset_all_state();

    let policy = RetryPolicy::new(2, 1, 1.0).unwrap();

    let p1 = policy.clone();
    let p2 = policy.clone();

    let h1 = tokio::spawn(async move {
        execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, p1).await
    });

    let h2 = tokio::spawn(async move {
        execute_step_with_retry(StepId::new("step-flaky".to_string()), 5000, p2).await
    });

    let r1 = h1.await.unwrap();
    let r2 = h2.await.unwrap();

    // Both should fail — flaky step always exhausts
    assert!(r1.is_err(), "h1 should exhaust retries");
    assert!(r2.is_err(), "h2 should exhaust retries");

    // INVARIANT: Error map should contain exactly one entry for step-flaky.
    let last_err = get_last_error(&StepId::new("step-flaky".to_string()));
    assert!(
        last_err.is_some(),
        "error map entry for step-flaky was lost during concurrent writes"
    );

    // INVARIANT: The stored error must be well-formed TransientError.
    match &last_err.unwrap() {
        vo_executor::ExecuteNodeError::TransientError { reason, .. } => {
            assert!(
                !reason.is_empty(),
                "error reason was corrupted (empty string)"
            );
            assert!(
                reason.len() < 1000,
                "error reason suspiciously long ({} chars) — possible corruption",
                reason.len()
            );
        }
        other => {
            panic!(
                "error map contains unexpected error type: {:?} — expected TransientError",
                other
            );
        }
    }
}

/// CONCURRENT-003: State map and error map non-atomic update
///
/// Attack: One task sets state to Executing while another concurrently clears the error.
/// If the state transition and error clear are not atomic, a reader could see
/// Executing state with a stale error from a previous run.
#[tokio::test]
async fn bh_state_error_map_non_atomic_consistency() {
    use vo_executor::{clear_error, get_execution_status, set_error};
    use vo_executor::state::set_state;

    reset_all_state();

    let step = StepId::new("bh-concurrent-003".to_string());

    // Seed an initial error
    set_error(
        step.as_str(),
        vo_executor::ExecuteNodeError::TransientError {
            reason: "initial error".to_string(),
            recoverable: true,
        },
    );

    let s1 = step.clone();
    let h1 = tokio::spawn(async move {
        clear_error(s1.as_str());
        set_state(
            s1.as_str(),
            StepState::Executing {
                step_id: s1.clone(),
                start_time: std::time::Instant::now(),
            },
        );
    });

    let s2 = step.clone();
    let h2 = tokio::spawn(async move {
        let status = get_execution_status(&s2);
        let err = get_last_error(&s2);
        (status, err)
    });

    let _ = h1.await;
    let (status, err) = h2.await.unwrap();

    let is_executing = matches!(status, vo_executor::ExecutionStatus::Executing { .. });
    let has_error = err.is_some();

    assert!(
        !(is_executing && has_error),
        "STATE/ERROR INCONSISTENCY: status is Executing but error map still has entry. \
         Non-atomic update allows readers to see inconsistent state. \
         status={:?}, error={:?}",
        status,
        err
    );
}

/// CONCURRENT-004: State clobbering via late writer
///
/// Attack: Task A completes execution and sets state to Ready.
/// Task B (a stale/delayed task) then overwrites with its own Executing state.
/// This simulates a network-partitioned node that finally responds.
///
/// Expected: DashMap last-write-wins semantics mean the late writer clobbers.
/// This test documents the vulnerability — no optimistic concurrency control exists.
#[tokio::test]
async fn bh_late_writer_state_clobbering() {
    use vo_executor::state::set_state;

    reset_all_state();

    let step = "bh-clobber-004";

    // Task A: complete normally
    set_state(
        step,
        StepState::Executing {
            step_id: StepId::new(step.to_string()),
            start_time: std::time::Instant::now(),
        },
    );
    set_state(step, StepState::Ready);

    // Task B: stale writer clobbers the Ready state
    set_state(
        step,
        StepState::Executing {
            step_id: StepId::new(step.to_string()),
            start_time: std::time::Instant::now(),
        },
    );

    let final_state = get_state(step);

    // PROVEN: Late writer successfully clobbers Ready → Executing.
    // This is a VULNERABILITY: no OCC/CAS to prevent stale writes.
    assert!(
        matches!(final_state, StepState::Executing { .. }),
        "PROVEN: Late writer can clobber Ready state back to Executing. \
         No optimistic concurrency control exists on DashMap writes. \
         final_state={:?}",
        final_state
    );
}
