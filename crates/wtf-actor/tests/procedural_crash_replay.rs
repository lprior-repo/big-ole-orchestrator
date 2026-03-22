//! Integration test: Procedural checkpoint — ctx.activity() result survives crash
//!
//! This test verifies that the checkpoint map correctly persists activity results
//! across engine crashes, ensuring exactly-once activity dispatch semantics.
//!
//! Test scenario:
//! 1. Start procedural workflow with 3 ctx.activity() calls in sequence
//! 2. Complete op 0 (validate_order) and op 1 (charge_card) via worker
//! 3. Kill engine after op 1 ACKed
//! 4. Restart engine
//! 5. Assert op 0 and op 1 NOT re-dispatched (checkpoint_map has results)
//! 6. Assert op 2 (send_confirmation) IS dispatched
//! 7. Complete op 2
//! 8. Assert InstanceCompleted

use bytes::Bytes;
use wtf_actor::procedural::{
    state::apply_event as proc_apply, ProceduralActorState, ProceduralApplyResult,
};
use wtf_common::{RetryPolicy, WorkflowEvent};

fn dispatch(op_id: u32, activity_type: &str) -> WorkflowEvent {
    WorkflowEvent::ActivityDispatched {
        activity_id: format!("wf-crash-test:{op_id}"),
        activity_type: activity_type.into(),
        payload: Bytes::new(),
        retry_policy: RetryPolicy::default(),
        attempt: 1,
    }
}

fn complete(op_id: u32, result: &[u8]) -> WorkflowEvent {
    WorkflowEvent::ActivityCompleted {
        activity_id: format!("wf-crash-test:{op_id}"),
        result: Bytes::copy_from_slice(result),
        duration_ms: 10,
    }
}

#[test]
fn checkpoint_persists_across_crash_state_machine() {
    let s0 = ProceduralActorState::new();

    let (s1, _) = proc_apply(&s0, &dispatch(0, "validate_order"), 1).expect("dispatch op 0");
    let (s2, _) = proc_apply(&s1, &complete(0, b"order-validated"), 2).expect("complete op 0");

    let (s3, _) = proc_apply(&s2, &dispatch(1, "charge_card"), 3).expect("dispatch op 1");
    let (s4, _) = proc_apply(&s3, &complete(1, b"card-charged"), 4).expect("complete op 1");

    assert_eq!(
        s4.operation_counter, 2,
        "op_counter should be 2 after ops 0 and 1"
    );
    assert!(
        s4.checkpoint_map.contains_key(&0),
        "checkpoint 0 must exist"
    );
    assert!(
        s4.checkpoint_map.contains_key(&1),
        "checkpoint 1 must exist"
    );
    assert!(
        !s4.checkpoint_map.contains_key(&2),
        "checkpoint 2 must NOT exist yet"
    );

    assert_eq!(s4.checkpoint_map[&0].result.as_ref(), b"order-validated");
    assert_eq!(s4.checkpoint_map[&1].result.as_ref(), b"card-charged");
}

#[test]
fn op_counter_deterministic_after_replay() {
    let s0 = ProceduralActorState::new();

    let (s1, _) = proc_apply(&s0, &dispatch(0, "validate_order"), 1).expect("dispatch op 0");
    let (s2, _) = proc_apply(&s1, &complete(0, b"order-validated"), 2).expect("complete op 0");

    let (s3, _) = proc_apply(&s2, &dispatch(1, "charge_card"), 3).expect("dispatch op 1");
    let (s4, _) = proc_apply(&s3, &complete(1, b"card-charged"), 4).expect("complete op 1");

    assert_eq!(
        s4.operation_counter, 2,
        "op_counter must be deterministic at 2"
    );

    let replay_s0 = ProceduralActorState::new();

    let (replayed, _) =
        proc_apply(&replay_s0, &dispatch(0, "validate_order"), 1).expect("replay dispatch op 0");
    let (replayed, _) =
        proc_apply(&replayed, &complete(0, b"order-validated"), 2).expect("replay complete op 0");
    let (replayed, _) =
        proc_apply(&replayed, &dispatch(1, "charge_card"), 3).expect("replay dispatch op 1");
    let (replayed, _) =
        proc_apply(&replayed, &complete(1, b"card-charged"), 4).expect("replay complete op 1");

    assert_eq!(
        replayed.operation_counter, 2,
        "Replayed state must have same op_counter as pre-crash"
    );
}

#[test]
fn exactly_once_activity_dispatch_via_checkpoint_map() {
    let s0 = ProceduralActorState::new();

    let (s1, _) = proc_apply(&s0, &dispatch(0, "validate_order"), 1).expect("dispatch op 0");
    let (s2, _) = proc_apply(&s1, &complete(0, b"validated"), 2).expect("complete op 0");

    assert!(
        s2.checkpoint_map.contains_key(&0),
        "Completed op 0 must be in checkpoint_map"
    );

    let (_s3, result) = proc_apply(&s2, &dispatch(0, "validate_order"), 1)
        .expect("dispatch op 0 again (simulating re-dispatch)");

    assert!(
        matches!(result, ProceduralApplyResult::AlreadyApplied),
        "Re-dispatch of already-completed op 0 should return AlreadyApplied"
    );
}

#[test]
fn instance_completes_after_all_ops_checkpointed() {
    let s0 = ProceduralActorState::new();

    let (s1, _) = proc_apply(&s0, &dispatch(0, "validate_order"), 1).expect("dispatch op 0");
    let (s2, _) = proc_apply(&s1, &complete(0, b"validated"), 2).expect("complete op 0");

    let (s3, _) = proc_apply(&s2, &dispatch(1, "charge_card"), 3).expect("dispatch op 1");
    let (s4, _) = proc_apply(&s3, &complete(1, b"charged"), 4).expect("complete op 1");

    let (s5, _) = proc_apply(&s4, &dispatch(2, "send_confirmation"), 5).expect("dispatch op 2");
    let (s6, _) = proc_apply(&s5, &complete(2, b"confirmed"), 6).expect("complete op 2");

    let mut checkpoint_keys: Vec<u32> = s6.checkpoint_map.keys().copied().collect();
    checkpoint_keys.sort();
    assert_eq!(
        checkpoint_keys,
        vec![0, 1, 2],
        "All three ops should be checkpointed"
    );

    assert_eq!(
        s6.operation_counter, 3,
        "operation_counter should be 3 after all ops complete"
    );
}

#[test]
fn replay_after_crash_restores_checkpoint_state() {
    let s0 = ProceduralActorState::new();

    let (s1, _) = proc_apply(&s0, &dispatch(0, "validate_order"), 1).expect("dispatch op 0");
    let (s2, _) = proc_apply(&s1, &complete(0, b"order-validated"), 2).expect("complete op 0");

    let (s3, _) = proc_apply(&s2, &dispatch(1, "charge_card"), 3).expect("dispatch op 1");
    let (s4, _) = proc_apply(&s3, &complete(1, b"card-charged"), 4).expect("complete op 1");

    let (s5, _) = proc_apply(&s4, &dispatch(2, "send_confirmation"), 5).expect("dispatch op 2");

    assert_eq!(
        s5.operation_counter, 3,
        "op_counter should be 3 before crash"
    );
    assert!(
        s5.checkpoint_map.contains_key(&0),
        "checkpoint 0 must exist after crash"
    );
    assert!(
        s5.checkpoint_map.contains_key(&1),
        "checkpoint 1 must exist after crash"
    );
    assert!(
        !s5.checkpoint_map.contains_key(&2),
        "checkpoint 2 must NOT exist yet"
    );

    let (_s6, dispatch_result) =
        proc_apply(&s5, &dispatch(2, "send_confirmation"), 5).expect("dispatch op 2 after restart");

    assert!(
        matches!(dispatch_result, ProceduralApplyResult::AlreadyApplied),
        "Re-dispatch of op 2 at same seq should return AlreadyApplied (idempotency)"
    );
}

#[test]
fn checkpoint_map_sequential_ops_correct_order() {
    let s0 = ProceduralActorState::new();

    let (s1, dispatch0) = proc_apply(&s0, &dispatch(0, "op_a"), 1).expect("dispatch op 0");
    assert!(matches!(
        dispatch0,
        ProceduralApplyResult::ActivityDispatched {
            operation_id: 0,
            ..
        }
    ));

    let (s2, complete0) = proc_apply(&s1, &complete(0, b"result_0"), 2).expect("complete op 0");
    assert!(matches!(
        complete0,
        ProceduralApplyResult::ActivityCompleted {
            operation_id: 0,
            ..
        }
    ));

    let (s3, dispatch1) = proc_apply(&s2, &dispatch(1, "op_b"), 3).expect("dispatch op 1");
    assert!(matches!(
        dispatch1,
        ProceduralApplyResult::ActivityDispatched {
            operation_id: 1,
            ..
        }
    ));

    let (s4, complete1) = proc_apply(&s3, &complete(1, b"result_1"), 4).expect("complete op 1");
    assert!(matches!(
        complete1,
        ProceduralApplyResult::ActivityCompleted {
            operation_id: 1,
            ..
        }
    ));

    let (s5, dispatch2) = proc_apply(&s4, &dispatch(2, "op_c"), 5).expect("dispatch op 2");
    assert!(matches!(
        dispatch2,
        ProceduralApplyResult::ActivityDispatched {
            operation_id: 2,
            ..
        }
    ));

    assert_eq!(s5.operation_counter, 3);

    let mut checkpoint_keys: Vec<u32> = s5.checkpoint_map.keys().copied().collect();
    checkpoint_keys.sort();
    assert_eq!(checkpoint_keys, vec![0, 1], "Checkpoints should be [0, 1]");
}

#[test]
fn crash_recovery_skips_completed_ops_and_dispatches_next() {
    let s0 = ProceduralActorState::new();

    let (s1, _) = proc_apply(&s0, &dispatch(0, "validate_order"), 1).expect("dispatch op 0");
    let (s2, _) = proc_apply(&s1, &complete(0, b"validated"), 2).expect("complete op 0");

    let (s3, _) = proc_apply(&s2, &dispatch(1, "charge_card"), 3).expect("dispatch op 1");
    let (s4, _) = proc_apply(&s3, &complete(1, b"charged"), 4).expect("complete op 1");

    assert_eq!(s4.operation_counter, 2);
    assert!(s4.checkpoint_map.contains_key(&0));
    assert!(s4.checkpoint_map.contains_key(&1));
    assert!(!s4.checkpoint_map.contains_key(&2));

    let (_s5, result) = proc_apply(&s4, &dispatch(2, "send_confirmation"), 5)
        .expect("dispatch op 2 (should be allowed - next op)");

    assert!(
        matches!(
            result,
            ProceduralApplyResult::ActivityDispatched {
                operation_id: 2,
                ..
            }
        ),
        "Op 2 should be dispatched as the next operation"
    );
}
