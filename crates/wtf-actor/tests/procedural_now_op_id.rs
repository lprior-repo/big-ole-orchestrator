//! Bug regression: handle_now and handle_random must use the operation_id
//! sent in the message, not s.operation_counter from paradigm state.
//!
//! BUG: ProceduralNow and ProceduralRandom messages carry no operation_id.
//! The handlers read `s.operation_counter` to determine which checkpoint to look up.
//! When operation_counter != the caller's ctx.op_counter (e.g. after replay where
//! ActivityDispatched increments the counter but NowSampled/RandomSampled do not),
//! the wrong checkpoint slot is checked, causing a fresh value to be sampled instead
//! of replaying the recorded one.
//!
//! Example:
//!   op 0: activity dispatch  → s.operation_counter = 1
//!   op 1: now()              → NowSampled { operation_id: 1 } → checkpoint_map[1]
//!   op 2: activity dispatch  → s.operation_counter = 3
//!   (restart) handle_now reads s.operation_counter = 3 → checkpoint[3] missing → new ts!

use bytes::Bytes;
use std::collections::HashMap;
use wtf_actor::{
    instance::{lifecycle::ParadigmState, procedural_utils::handle_now, state::InstanceState},
    messages::{InstanceArguments, InstancePhase, WorkflowParadigm},
    procedural::{
        state::apply_event as proc_apply,
        ProceduralActorState,
    },
};
use wtf_common::{InstanceId, NamespaceId, RetryPolicy, WorkflowEvent};

fn test_args() -> InstanceArguments {
    InstanceArguments {
        namespace: NamespaceId::new("test"),
        instance_id: InstanceId::new("wf-01"),
        workflow_type: "flow".into(),
        paradigm: WorkflowParadigm::Procedural,
        input: Bytes::from_static(b"{}"),
        engine_node_id: "node-1".into(),
        event_store: None,
        state_store: None,
        task_queue: None,
        snapshot_db: None,
        procedural_workflow: None,
        workflow_definition: None,
    }
}

/// Build a state with s.operation_counter=2 but checkpoint_map[1] = a known ts.
/// This simulates the state after:
///   seq 1: ActivityDispatched (op 0) → s.operation_counter = 1
///   seq 2: NowSampled { operation_id: 1 } → checkpoint_map[1] = fixed_ts
///   seq 3: ActivityDispatched (op 2) → s.operation_counter = 3
/// Note: ActivityDispatched at op 2 means s.operation_counter = 3 now.
fn build_state_with_now_checkpoint_at_op1() -> (InstanceState, chrono::DateTime<chrono::Utc>) {
    let fixed_ts = chrono::DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
    let _ts_bytes = Bytes::copy_from_slice(&fixed_ts.timestamp_millis().to_le_bytes());

    let s0 = ProceduralActorState::new();

    // Apply ActivityDispatched for op 0
    let dispatch0 = WorkflowEvent::ActivityDispatched {
        activity_id: "wf-01:0".into(),
        activity_type: "step_a".into(),
        payload: Bytes::new(),
        retry_policy: RetryPolicy::default(),
        attempt: 1,
    };
    let (s1, _) = proc_apply(&s0, &dispatch0, 1).expect("dispatch0");
    assert_eq!(
        s1.operation_counter, 1,
        "precondition: op_counter=1 after first dispatch"
    );

    // Apply NowSampled at operation_id=1
    let now_ev = WorkflowEvent::NowSampled {
        operation_id: 1,
        ts: fixed_ts,
    };
    let (s2, _) = proc_apply(&s1, &now_ev, 2).expect("now_sampled");
    assert!(
        s2.checkpoint_map.contains_key(&1),
        "precondition: checkpoint_map[1] exists"
    );
    assert_eq!(
        s2.operation_counter, 1,
        "precondition: op_counter unchanged by NowSampled"
    );

    // Apply ActivityDispatched for op 2 — this increments operation_counter to 2
    let dispatch2 = WorkflowEvent::ActivityDispatched {
        activity_id: "wf-01:2".into(),
        activity_type: "step_b".into(),
        payload: Bytes::new(),
        retry_policy: RetryPolicy::default(),
        attempt: 1,
    };
    let (s3, _) = proc_apply(&s2, &dispatch2, 3).expect("dispatch2");
    assert_eq!(
        s3.operation_counter, 2,
        "precondition: op_counter=2 after second dispatch"
    );
    assert!(
        !s3.checkpoint_map.contains_key(&2),
        "precondition: no checkpoint at index 2"
    );

    let state = InstanceState {
        paradigm_state: ParadigmState::Procedural(s3),
        args: test_args(),
        phase: InstancePhase::Live,
        total_events_applied: 3,
        events_since_snapshot: 3,
        outbox: Vec::new(),
        pending_activity_calls: HashMap::new(),
        pending_timer_calls: HashMap::new(),
        pending_signal_calls: HashMap::new(),
        procedural_task: None,
        live_subscription_task: None,
    };
    (state, fixed_ts)
}

/// handle_now must use the operation_id from the message context (op 1), NOT s.operation_counter (2).
/// With the bug, it reads op_counter=2, finds no checkpoint[2], samples a fresh timestamp.
/// After the fix, it uses the caller's operation_id=1, finds checkpoint[1], returns fixed_ts.
#[tokio::test]
async fn handle_now_returns_checkpointed_value_when_called_for_op1_but_op_counter_is_2() {
    let (mut state, fixed_ts) = build_state_with_now_checkpoint_at_op1();

    // The workflow context at this point has replayed op0 (checkpoint found) and is now
    // executing the now() call at op_id=1. But s.operation_counter=2 (two dispatches applied).
    // handle_now must use operation_id=1 (caller's op) not s.operation_counter=2.
    let (tx, rx) = tokio::sync::oneshot::channel::<chrono::DateTime<chrono::Utc>>();
    handle_now(&mut state, 1, tx.into()).await;
    let returned_ts = rx.await.expect("reply");

    assert_eq!(
        returned_ts, fixed_ts,
        "handle_now must return the checkpointed timestamp for op 1, not a fresh sample"
    );
}
