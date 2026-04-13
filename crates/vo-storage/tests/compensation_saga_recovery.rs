//! Compensation saga recovery tests — forward-recovery, backward-recovery,
//! partial compensation handling (ADR-034).

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use vo_storage::compensation_saga::{
    CompensationReconciler, CompensationSaga, ReconciliationAction, ReconciliationContext,
    RetryReconciler, SagaCompensationStatus,
};
use vo_types::CompensationPolicy;

#[test]
fn backward_recovery_full_chain_in_reverse_order() {
    let saga = CompensationSaga::new();

    // Register three effects: charge → reserve → ship
    saga.register("charge".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.register("reserve".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.register("ship".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();

    // Queue all for compensation
    saga.queue_pending("charge").unwrap();
    saga.queue_pending("reserve").unwrap();
    saga.queue_pending("ship").unwrap();

    // Verify reverse order
    let order = saga.get_compensation_order();
    assert_eq!(order, vec!["ship", "reserve", "charge"]);

    // Execute full backward-recovery chain
    for eid in &order {
        saga.start_compensation(eid).unwrap();
        saga.succeed(eid).unwrap();
    }

    let manifest_arc = saga.manifest();
    let manifest = manifest_arc.lock().unwrap();
    for eid in &["charge", "reserve", "ship"] {
        let entry = manifest.get(*eid).expect("entry exists");
        assert_eq!(entry.status, SagaCompensationStatus::Succeeded);
    }
}

#[test]
fn partial_compensation_mid_chain_failure_preserves_prior_successes() {
    let saga = CompensationSaga::new();

    saga.register("charge".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.register("reserve".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.register("ship".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();

    saga.queue_pending("charge").unwrap();
    saga.queue_pending("reserve").unwrap();
    saga.queue_pending("ship").unwrap();

    // Succeed ship compensation
    saga.start_compensation("ship").unwrap();
    saga.succeed("ship").unwrap();

    // Fail reserve compensation (mid-chain)
    saga.start_compensation("reserve").unwrap();
    saga.fail("reserve").unwrap();

    // Charge should still be Pending (not yet attempted)
    let manifest_arc = saga.manifest();
    let manifest = manifest_arc.lock().unwrap();

    assert_eq!(
        manifest.get("ship").unwrap().status,
        SagaCompensationStatus::Succeeded
    );
    assert_eq!(
        manifest.get("reserve").unwrap().status,
        SagaCompensationStatus::Failed
    );
    assert_eq!(
        manifest.get("charge").unwrap().status,
        SagaCompensationStatus::Pending
    );
}

#[test]
fn mixed_policies_none_skipped_automatic_and_manual_queued() {
    let saga = CompensationSaga::new();

    saga.register("email".to_string(), CompensationPolicy::None, vec![])
        .unwrap();
    saga.register("charge".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.register("reserve".to_string(), CompensationPolicy::Manual, vec![])
        .unwrap();

    // None policy cannot be queued
    let result = saga.queue_pending("email");
    assert!(result.is_err());

    // Automatic can be queued
    saga.queue_pending("charge").unwrap();

    // Manual can be queued
    saga.queue_pending("reserve").unwrap();

    let order = saga.get_compensation_order();
    assert_eq!(order, vec!["reserve", "charge"]);
}

#[test]
fn reconciliation_exhaustion_escalates_to_operator() {
    let saga = CompensationSaga::with_reconciler(RetryReconciler::new(1));
    saga.register("fx-1".to_string(), CompensationPolicy::Automatic, vec![])
        .unwrap();
    saga.queue_pending("fx-1").unwrap();
    saga.start_compensation("fx-1").unwrap();
    saga.mark_ambiguous("fx-1").unwrap();

    // First retry attempt requeues
    let ctx = ReconciliationContext {
        effect_id: "fx-1".to_string(),
        compensation_effect_id: None,
        last_known_outcome: None,
        attempts: 1,
        last_attempt_at: None,
    };
    let reconciler = RetryReconciler::new(1);
    let action = reconciler.reconcile(&ctx);
    assert_eq!(action, ReconciliationAction::EscalateToOperator);

    saga.handle_reconciliation("fx-1", action).unwrap();

    let manifest_arc = saga.manifest();
    let manifest = manifest_arc.lock().unwrap();
    assert_eq!(
        manifest.get("fx-1").unwrap().status,
        SagaCompensationStatus::Failed
    );
}
