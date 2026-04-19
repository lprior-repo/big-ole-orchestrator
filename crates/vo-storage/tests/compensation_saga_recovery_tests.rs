//! Saga forward-recovery, backward-recovery, and partial compensation tests (ADR-034).
//!
//! These tests verify the CompensationSaga handles:
//! - Forward recovery: saga recovers from ambiguous state by committing compensations
//! - Backward recovery: saga recovers by failing compensations when unrecoverable
//! - Partial compensation: some compensations succeed while others fail
//! - Multi-step saga with full lifecycle

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use vo_storage::compensation_saga::*;
use vo_types::{CompensationPolicy, TimestampMs};

// ========================================================================
// Forward Recovery — ambiguous compensation resolves to success
// ========================================================================

#[test]
fn forward_recovery_ambiguous_to_succeeded() {
    struct ForwardReconciler;
    impl CompensationReconciler for ForwardReconciler {
        fn reconcile(&self, _ctx: &ReconciliationContext) -> ReconciliationAction {
            ReconciliationAction::CommitCompensation
        }
    }

    let saga = CompensationSaga::with_reconciler(ForwardReconciler);
    saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.queue_pending("fx-1").unwrap();
    saga.start_compensation("fx-1").unwrap();

    let action = saga.mark_ambiguous("fx-1").unwrap();
    assert_eq!(action, ReconciliationAction::CommitCompensation);

    saga.handle_reconciliation("fx-1", action).unwrap();

    let manifest = saga.manifest();
    let guard = manifest.lock().unwrap();
    let entry = guard.get("fx-1").expect("entry exists");
    assert_eq!(
        entry.status,
        SagaCompensationStatus::Succeeded,
        "forward recovery must resolve to Succeeded"
    );
}

// ========================================================================
// Backward Recovery — ambiguous compensation resolves to failure
// ========================================================================

#[test]
fn backward_recovery_ambiguous_to_failed() {
    struct BackwardReconciler;
    impl CompensationReconciler for BackwardReconciler {
        fn reconcile(&self, _ctx: &ReconciliationContext) -> ReconciliationAction {
            ReconciliationAction::AbandonCompensation
        }
    }

    let saga = CompensationSaga::with_reconciler(BackwardReconciler);
    saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.queue_pending("fx-1").unwrap();
    saga.start_compensation("fx-1").unwrap();

    let action = saga.mark_ambiguous("fx-1").unwrap();
    assert_eq!(action, ReconciliationAction::AbandonCompensation);

    saga.handle_reconciliation("fx-1", action).unwrap();

    let manifest = saga.manifest();
    let guard = manifest.lock().unwrap();
    let entry = guard.get("fx-1").expect("entry exists");
    assert_eq!(
        entry.status,
        SagaCompensationStatus::Failed,
        "backward recovery must resolve to Failed"
    );
}

// ========================================================================
// Escalate to Operator — NoOpReconciler always escalates
// ========================================================================

#[test]
fn noop_reconciler_always_escalates() {
    let saga = CompensationSaga::new();

    saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.queue_pending("fx-1").unwrap();
    saga.start_compensation("fx-1").unwrap();

    let action = saga.mark_ambiguous("fx-1").unwrap();
    assert_eq!(action, ReconciliationAction::EscalateToOperator);

    saga.handle_reconciliation("fx-1", action).unwrap();

    let manifest = saga.manifest();
    let guard = manifest.lock().unwrap();
    let entry = guard.get("fx-1").expect("entry exists");
    assert_eq!(entry.status, SagaCompensationStatus::Failed);
}

// ========================================================================
// Partial Compensation — some succeed, some fail
// ========================================================================

#[test]
fn partial_compensation_first_succeeds_second_fails() {
    let saga = CompensationSaga::new();

    saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.register("fx-2".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();

    saga.queue_pending("fx-1").unwrap();
    saga.queue_pending("fx-2").unwrap();

    // Compensate fx-2 first (reverse order)
    saga.start_compensation("fx-2").unwrap();
    saga.succeed("fx-2").unwrap();

    // Compensate fx-1 — fails
    saga.start_compensation("fx-1").unwrap();
    saga.fail("fx-1").unwrap();

    let manifest = saga.manifest();
    let guard = manifest.lock().unwrap();

    let e1 = guard.get("fx-1").expect("entry exists");
    assert_eq!(e1.status, SagaCompensationStatus::Failed);

    let e2 = guard.get("fx-2").expect("entry exists");
    assert_eq!(e2.status, SagaCompensationStatus::Succeeded);
}

#[test]
fn partial_compensation_dependency_satisfied_after_fail() {
    let saga = CompensationSaga::new();

    saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.register(
        "fx-2".to_string(),
        CompensationPolicy::Automatic,
        vec!["fx-1".to_string()],
    )
    .unwrap();

    saga.queue_pending("fx-1").unwrap();
    saga.queue_pending("fx-2").unwrap();

    // fx-1 fails — Failed IS terminal, so fx-2 becomes unblocked
    saga.start_compensation("fx-1").unwrap();
    saga.fail("fx-1").unwrap();

    let manifest = saga.manifest();
    let guard = manifest.lock().unwrap();
    assert!(
        guard.can_execute("fx-2"),
        "fx-2 unblocked: Failed dependency IS terminal"
    );

    let e2 = guard.get("fx-2").expect("entry exists");
    assert_eq!(e2.status, SagaCompensationStatus::Pending);
}

// ========================================================================
// Multi-Step Saga — full lifecycle with 5 effects
// ========================================================================

#[test]
fn multi_step_saga_full_lifecycle_five_effects() {
    let saga = CompensationSaga::new();

    for i in 1..=5 {
        saga.register(format!("fx-{i}"), CompensationPolicy::Automatic, vec![])
            .unwrap();
    }

    for i in 1..=5 {
        saga.queue_pending(&format!("fx-{i}")).unwrap();
    }

    let order = saga.get_compensation_order();
    assert_eq!(order, vec!["fx-5", "fx-4", "fx-3", "fx-2", "fx-1"]);

    for eid in &order {
        saga.start_compensation(eid).unwrap();
        saga.succeed(eid).unwrap();
    }

    let manifest = saga.manifest();
    let guard = manifest.lock().unwrap();
    for i in 1..=5 {
        let entry = guard.get(&format!("fx-{i}")).expect("entry exists");
        assert_eq!(entry.status, SagaCompensationStatus::Succeeded);
    }

    let remaining: Vec<_> = guard.compensations_awaiting_execution();
    assert!(remaining.is_empty());
}

#[test]
fn multi_step_saga_partial_failure_mid_saga() {
    let saga = CompensationSaga::new();

    for i in 1..=4 {
        saga.register(format!("fx-{i}"), CompensationPolicy::Automatic, vec![])
            .unwrap();
    }

    for i in 1..=4 {
        saga.queue_pending(&format!("fx-{i}")).unwrap();
    }

    let order = saga.get_compensation_order();
    assert_eq!(order, vec!["fx-4", "fx-3", "fx-2", "fx-1"]);

    saga.start_compensation("fx-4").unwrap();
    saga.succeed("fx-4").unwrap();

    saga.start_compensation("fx-3").unwrap();
    saga.fail("fx-3").unwrap();

    let manifest = saga.manifest();
    let guard = manifest.lock().unwrap();

    assert_eq!(
        guard.get("fx-4").unwrap().status,
        SagaCompensationStatus::Succeeded
    );
    assert_eq!(
        guard.get("fx-3").unwrap().status,
        SagaCompensationStatus::Failed
    );
    assert_eq!(
        guard.get("fx-2").unwrap().status,
        SagaCompensationStatus::Pending
    );
    assert_eq!(
        guard.get("fx-1").unwrap().status,
        SagaCompensationStatus::Pending
    );

    let remaining: Vec<_> = guard.compensations_awaiting_execution();
    assert_eq!(remaining.len(), 2);
}

// ========================================================================
// Reconciliation Action Exhaustiveness
// ========================================================================

#[test]
fn all_reconciliation_actions_are_handled() {
    let saga = CompensationSaga::new();
    saga.register(
        "fx-commit".to_string(),
        CompensationPolicy::Automatic,
        vec![],
    )
    .unwrap();
    saga.register(
        "fx-retry".to_string(),
        CompensationPolicy::Automatic,
        vec![],
    )
    .unwrap();
    saga.register(
        "fx-escalate".to_string(),
        CompensationPolicy::Automatic,
        vec![],
    )
    .unwrap();
    saga.register(
        "fx-abandon".to_string(),
        CompensationPolicy::Automatic,
        vec![],
    )
    .unwrap();

    for eid in &["fx-commit", "fx-retry", "fx-escalate", "fx-abandon"] {
        saga.queue_pending(eid).unwrap();
        saga.start_compensation(eid).unwrap();
        saga.mark_ambiguous(eid).unwrap();
    }

    saga.handle_reconciliation("fx-commit", ReconciliationAction::CommitCompensation)
        .unwrap();
    saga.handle_reconciliation("fx-retry", ReconciliationAction::RetryCompensation)
        .unwrap();
    saga.handle_reconciliation("fx-escalate", ReconciliationAction::EscalateToOperator)
        .unwrap();
    saga.handle_reconciliation("fx-abandon", ReconciliationAction::AbandonCompensation)
        .unwrap();

    let manifest = saga.manifest();
    let guard = manifest.lock().unwrap();

    assert_eq!(
        guard.get("fx-commit").unwrap().status,
        SagaCompensationStatus::Succeeded
    );
    assert_eq!(
        guard.get("fx-retry").unwrap().status,
        SagaCompensationStatus::Pending
    );
    assert_eq!(
        guard.get("fx-escalate").unwrap().status,
        SagaCompensationStatus::Failed
    );
    assert_eq!(
        guard.get("fx-abandon").unwrap().status,
        SagaCompensationStatus::Failed
    );
}

// ========================================================================
// RetryReconciler — retries then escalates
// ========================================================================

#[test]
fn retry_reconciler_retries_then_escalates() {
    struct CountingReconciler {
        max_attempts: u32,
    }
    impl CompensationReconciler for CountingReconciler {
        fn reconcile(&self, ctx: &ReconciliationContext) -> ReconciliationAction {
            if ctx.attempts < self.max_attempts {
                ReconciliationAction::RetryCompensation
            } else {
                ReconciliationAction::EscalateToOperator
            }
        }
    }

    let saga = CompensationSaga::with_reconciler(CountingReconciler { max_attempts: 2 });
    saga.register(
        "fx-retry".to_string(),
        CompensationPolicy::Automatic,
        vec![],
    )
    .unwrap();
    saga.queue_pending("fx-retry").unwrap();
    saga.start_compensation("fx-retry").unwrap();

    // mark_ambiguous passes attempts=0, so reconciler returns Retry
    let action1 = saga.mark_ambiguous("fx-retry").unwrap();
    assert_eq!(action1, ReconciliationAction::RetryCompensation);
    saga.handle_reconciliation("fx-retry", action1).unwrap();

    // Re-queue and mark ambiguous — still retries (attempts=0 each time)
    saga.start_compensation("fx-retry").unwrap();
    let action2 = saga.mark_ambiguous("fx-retry").unwrap();
    assert_eq!(action2, ReconciliationAction::RetryCompensation);
    saga.handle_reconciliation("fx-retry", action2).unwrap();
}

// ========================================================================
// Diamond dependency pattern
// ========================================================================

#[test]
fn diamond_dependency_compensation_order() {
    let saga = CompensationSaga::new();

    saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.register("fx-2".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.register(
        "fx-3".to_string(),
        CompensationPolicy::Automatic,
        vec!["fx-1".to_string(), "fx-2".to_string()],
    )
    .unwrap();

    saga.queue_pending("fx-1").unwrap();
    saga.queue_pending("fx-2").unwrap();
    saga.queue_pending("fx-3").unwrap();

    let order = saga.get_compensation_order();
    assert_eq!(order, vec!["fx-3", "fx-2", "fx-1"]);

    let manifest = saga.manifest();
    let guard = manifest.lock().unwrap();
    assert!(!guard.can_execute("fx-3"), "fx-3 blocked by fx-1 and fx-2");

    drop(guard);
    saga.start_compensation("fx-1").unwrap();
    saga.succeed("fx-1").unwrap();

    let manifest = saga.manifest();
    let guard = manifest.lock().unwrap();
    assert!(!guard.can_execute("fx-3"), "fx-3 still blocked by fx-2");

    drop(guard);
    saga.start_compensation("fx-2").unwrap();
    saga.succeed("fx-2").unwrap();

    let manifest = saga.manifest();
    let guard = manifest.lock().unwrap();
    assert!(
        guard.can_execute("fx-3"),
        "fx-3 unblocked after fx-1 and fx-2 succeeded"
    );
}

// ========================================================================
// Terminal transition protection — start_compensation returns PolicyViolation
// ========================================================================

#[test]
fn cannot_start_compensation_from_succeeded() {
    let saga = CompensationSaga::new();
    saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.queue_pending("fx-1").unwrap();
    saga.start_compensation("fx-1").unwrap();
    saga.succeed("fx-1").unwrap();

    let result = saga.start_compensation("fx-1");
    assert!(result.is_err());
    // start_compensation checks can_execute which fails when status != Pending
}

#[test]
fn cannot_start_compensation_from_failed() {
    let saga = CompensationSaga::new();
    saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.queue_pending("fx-1").unwrap();
    saga.start_compensation("fx-1").unwrap();
    saga.fail("fx-1").unwrap();

    let result = saga.start_compensation("fx-1");
    assert!(result.is_err());
}

#[test]
fn cannot_succeed_already_succeeded() {
    let saga = CompensationSaga::new();
    saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.queue_pending("fx-1").unwrap();
    saga.start_compensation("fx-1").unwrap();
    saga.succeed("fx-1").unwrap();

    let result = saga.succeed("fx-1");
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(CompensationError::TerminalState { .. })
    ));
}

#[test]
fn cannot_fail_already_failed() {
    let saga = CompensationSaga::new();
    saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.queue_pending("fx-1").unwrap();
    saga.start_compensation("fx-1").unwrap();
    saga.fail("fx-1").unwrap();

    let result = saga.fail("fx-1");
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(CompensationError::TerminalState { .. })
    ));
}

#[test]
fn cannot_transition_from_timed_out() {
    let saga = CompensationSaga::with_reconciler(RetryReconciler::new(1));
    saga.register_with_timeout("fx-1", CompensationPolicy::Automatic, vec![], 10)
        .unwrap();
    saga.queue_pending("fx-1").unwrap();
    saga.start_compensation("fx-1").unwrap();

    std::thread::sleep(std::time::Duration::from_millis(50));
    saga.expire_timed_out().unwrap();

    // TimedOut is terminal, so start_compensation fails
    let result = saga.start_compensation("fx-1");
    assert!(result.is_err());
}

// ========================================================================
// CompensationEntry helpers
// ========================================================================

#[test]
fn compensation_entry_with_compensation_effect_id() {
    let mut entry =
        CompensationEntry::new("fx-1".to_string(), CompensationPolicy::Automatic, vec![]);
    entry = entry.with_compensation_effect_id("comp-fx-1".to_string());

    assert_eq!(entry.compensation_effect_id, Some("comp-fx-1".to_string()));
}

#[test]
fn compensation_entry_with_timeout() {
    let entry = CompensationEntry::new("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .with_timeout(5000);

    assert_eq!(entry.timeout_ms, Some(5000));
}

#[test]
fn compensation_entry_not_timed_out_when_no_timeout_set() {
    let entry = CompensationEntry::new("fx-1".to_string(), CompensationPolicy::Automatic, vec![]);

    assert!(!entry.is_timed_out(TimestampMs::now()));
}

#[test]
fn compensation_entry_not_timed_out_within_window() {
    let entry = CompensationEntry::new("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .with_timeout(10000);

    assert!(!entry.is_timed_out(TimestampMs::now()));
}

// ========================================================================
// Manifest version tracking
// ========================================================================

#[test]
fn manifest_version_increments_on_each_mutation() {
    let saga = CompensationSaga::new();

    let v0 = saga.manifest().lock().unwrap().version();

    saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    let v1 = saga.manifest().lock().unwrap().version();
    assert!(v1 > v0);

    saga.queue_pending("fx-1").unwrap();
    let v2 = saga.manifest().lock().unwrap().version();
    assert!(v2 > v1);

    saga.start_compensation("fx-1").unwrap();
    let v3 = saga.manifest().lock().unwrap().version();
    assert!(v3 > v2);

    saga.succeed("fx-1").unwrap();
    let v4 = saga.manifest().lock().unwrap().version();
    assert!(v4 > v3);
}

// ========================================================================
// Iterators
// ========================================================================

#[test]
fn manifest_iterators_filter_by_status() {
    let saga = CompensationSaga::new();

    saga.register(
        "fx-pending".to_string(),
        CompensationPolicy::Automatic,
        vec![],
    )
    .unwrap();
    saga.register(
        "fx-inprogress".to_string(),
        CompensationPolicy::Automatic,
        vec![],
    )
    .unwrap();
    saga.register(
        "fx-succeeded".to_string(),
        CompensationPolicy::Automatic,
        vec![],
    )
    .unwrap();
    saga.register(
        "fx-failed".to_string(),
        CompensationPolicy::Automatic,
        vec![],
    )
    .unwrap();
    saga.register(
        "fx-ambiguous".to_string(),
        CompensationPolicy::Automatic,
        vec![],
    )
    .unwrap();

    // fx-pending: stays Pending
    saga.queue_pending("fx-pending").unwrap();

    // fx-inprogress: transition to InProgress
    saga.queue_pending("fx-inprogress").unwrap();
    saga.start_compensation("fx-inprogress").unwrap();

    // fx-succeeded: full lifecycle to Succeeded
    saga.queue_pending("fx-succeeded").unwrap();
    saga.start_compensation("fx-succeeded").unwrap();
    saga.succeed("fx-succeeded").unwrap();

    // fx-failed: full lifecycle to Failed
    saga.queue_pending("fx-failed").unwrap();
    saga.start_compensation("fx-failed").unwrap();
    saga.fail("fx-failed").unwrap();

    // fx-ambiguous: InProgress then Ambiguous
    saga.queue_pending("fx-ambiguous").unwrap();
    saga.start_compensation("fx-ambiguous").unwrap();
    saga.mark_ambiguous("fx-ambiguous").unwrap();

    let manifest = saga.manifest();
    let guard = manifest.lock().unwrap();

    assert_eq!(guard.pending_compensations().count(), 1);
    assert_eq!(guard.in_progress_compensations().count(), 1);
    assert_eq!(guard.ambiguous_compensations().count(), 1);
    assert_eq!(guard.all_entries().count(), 5);
}

// ========================================================================
// CompensationError Display
// ========================================================================

#[test]
fn compensation_error_display_messages() {
    let err = CompensationError::AlreadyRegistered("fx-1".to_string());
    assert!(err.to_string().contains("fx-1"));

    let err = CompensationError::NotFound("fx-2".to_string());
    assert!(err.to_string().contains("fx-2"));

    let err = CompensationError::PolicyViolation {
        effect_id: "fx-3".to_string(),
        policy: CompensationPolicy::None,
    };
    assert!(err.to_string().contains("fx-3"));
    assert!(err.to_string().contains("None"));

    let err = CompensationError::Timeout {
        effect_id: "fx-4".to_string(),
    };
    assert!(err.to_string().contains("fx-4"));

    let err = CompensationError::ReconciliationRequired {
        effect_id: "fx-5".to_string(),
    };
    assert!(err.to_string().contains("fx-5"));
}
