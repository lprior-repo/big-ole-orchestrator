//! Bug regression tests: inject_event must apply events to paradigm_state.
//!
//! BUG: handlers::inject_event only increments counters but never calls
//! paradigm_state.apply_event(). During Live phase this means:
//!   - ActivityDispatched events don't increment operation_counter
//!   - ActivityCompleted events don't clear in_flight or record checkpoints
//! This leads to stale state and duplicate activity IDs on restart.

use bytes::Bytes;
use std::collections::HashMap;
use wtf_actor::{
    instance::{
        handlers,
        lifecycle::ParadigmState,
        state::InstanceState,
    },
    messages::{InstanceArguments, InstancePhase, WorkflowParadigm},
    procedural::{apply_event as proc_apply, ProceduralActorState},
};
use wtf_common::{InstanceId, NamespaceId, RetryPolicy, WorkflowEvent};

fn test_args() -> InstanceArguments {
    InstanceArguments {
        namespace: NamespaceId::new("test"),
        instance_id: InstanceId::new("inst-01"),
        workflow_type: "order_flow".into(),
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

/// inject_event must apply ActivityCompleted to paradigm state during Live phase.
/// Currently FAILS because inject_event skips paradigm_state.apply_event().
#[tokio::test]
async fn inject_live_activity_completed_clears_in_flight_and_records_checkpoint() {
    // Replay one dispatch to put op 0 into in_flight
    let s0 = ProceduralActorState::new();
    let dispatch_ev = WorkflowEvent::ActivityDispatched {
        activity_id: "inst-01:0".into(),
        activity_type: "step1".into(),
        payload: Bytes::new(),
        retry_policy: RetryPolicy::default(),
        attempt: 1,
    };
    let (s1, _) = proc_apply(&s0, &dispatch_ev, 1).expect("dispatch");
    assert!(
        s1.in_flight.contains_key(&0),
        "precondition: op 0 in_flight"
    );

    let args = test_args();
    let mut state = InstanceState {
        paradigm_state: ParadigmState::Procedural(s1),
        args,
        phase: InstancePhase::Live,
        total_events_applied: 1,
        events_since_snapshot: 1,
        outbox: Vec::new(),
        pending_activity_calls: HashMap::new(),
        pending_timer_calls: HashMap::new(),
        pending_signal_calls: HashMap::new(),
        procedural_task: None,
        live_subscription_task: None,
    };

    let complete_ev = WorkflowEvent::ActivityCompleted {
        activity_id: "inst-01:0".into(),
        result: Bytes::from_static(b"result"),
        duration_ms: 5,
    };
    handlers::inject_event(&mut state, 2, &complete_ev)
        .await
        .expect("ok");

    let ParadigmState::Procedural(s) = &state.paradigm_state else {
        panic!("expected Procedural state");
    };
    assert!(
        s.in_flight.is_empty(),
        "inject_event must apply ActivityCompleted to paradigm_state — in_flight should be empty"
    );
    assert!(
        s.checkpoint_map.contains_key(&0),
        "inject_event must apply ActivityCompleted to paradigm_state — checkpoint for op 0 should exist"
    );
}
