use super::*;
use crate::instance::lifecycle::ParadigmState;
use crate::instance::state::InstanceState;
use crate::messages::{InstanceArguments, InstancePhase, WorkflowParadigm};
use bytes::Bytes;
use std::collections::HashMap;
use wtf_common::{InstanceId, NamespaceId};

fn make_procedural_state() -> InstanceState {
    let args = InstanceArguments {
        namespace: NamespaceId::new("ns"),
        instance_id: InstanceId::new("i1"),
        workflow_type: "wf".into(),
        paradigm: WorkflowParadigm::Procedural,
        input: Bytes::new(),
        engine_node_id: "n1".into(),
        event_store: None,
        state_store: None,
        task_queue: None,
        snapshot_db: None,
        procedural_workflow: None,
        workflow_definition: None,
    };
    InstanceState {
        paradigm_state: ParadigmState::Procedural(
            crate::procedural::ProceduralActorState::new(),
        ),
        phase: InstancePhase::Live,
        total_events_applied: 0,
        events_since_snapshot: 0,
        outbox: Vec::new(),
        pending_activity_calls: HashMap::new(),
        pending_timer_calls: HashMap::new(),
        pending_signal_calls: HashMap::new(),
        procedural_task: None,
        live_subscription_task: None,
        args,
    }
}

#[tokio::test]
async fn get_checkpoint_returns_none_for_empty_state() {
    let state = make_procedural_state();
    let (tx, rx) = tokio::sync::oneshot::channel();
    handle_get_checkpoint(&state, 0, tx.into()).await;
    let result = rx.await.expect("reply");
    assert!(result.is_none());
}

#[tokio::test]
async fn get_checkpoint_returns_some_after_activity_completed() {
    use crate::procedural::state::apply_event;
    use wtf_common::WorkflowEvent;

    let dispatch_ev = WorkflowEvent::ActivityDispatched {
        activity_id: "i1:0".into(),
        activity_type: "work".into(),
        payload: Bytes::new(),
        retry_policy: wtf_common::RetryPolicy::default(),
        attempt: 1,
    };
    let complete_ev = WorkflowEvent::ActivityCompleted {
        activity_id: "i1:0".into(),
        result: Bytes::from_static(b"done"),
        duration_ms: 1,
    };

    let s0 = crate::procedural::ProceduralActorState::new();
    let (s1, _) = apply_event(&s0, &dispatch_ev, 1).expect("dispatch");
    let (s2, _) = apply_event(&s1, &complete_ev, 2).expect("complete");

    let mut state = make_procedural_state();
    state.paradigm_state = ParadigmState::Procedural(s2);
    state.total_events_applied = 2;
    state.events_since_snapshot = 2;

    let (tx, rx) = tokio::sync::oneshot::channel();
    handle_get_checkpoint(&state, 0, tx.into()).await;
    let result = rx.await.expect("reply");
    assert!(
        result.is_some(),
        "checkpoint must be present after ActivityCompleted"
    );
    assert_eq!(
        result.expect("checkpoint present").result,
        Bytes::from_static(b"done")
    );
}

// -----------------------------------------------------------------------
// handle_wait_for_signal tests
// -----------------------------------------------------------------------

#[tokio::test]
async fn wait_for_signal_returns_buffered_immediately() {
    let mut state = make_procedural_state();

    // Seed a buffered signal in ProceduralActorState.
    if let ParadigmState::Procedural(s) = &mut state.paradigm_state {
        s.received_signals
            .insert("approval".to_string(), vec![Bytes::from_static(b"ok")]);
    }

    let (tx, rx) = tokio::sync::oneshot::channel();
    handle_wait_for_signal(&mut state, 0, "approval".to_string(), tx.into()).await;

    let result = rx.await.expect("reply received");
    assert_eq!(result.expect("ok"), Bytes::from_static(b"ok"));

    // Buffer must be empty after consuming
    if let ParadigmState::Procedural(s) = &state.paradigm_state {
        assert!(
            !s.received_signals.contains_key("approval"),
            "buffered signal must be consumed"
        );
    }
}

#[tokio::test]
async fn wait_for_signal_registers_pending_when_no_buffer() {
    let mut state = make_procedural_state();

    let (tx, mut rx) = tokio::sync::oneshot::channel();
    handle_wait_for_signal(&mut state, 0, "missing".to_string(), tx.into()).await;

    // Reply must NOT be sent yet (signal not arrived)
    assert!(
        rx.try_recv().is_err(),
        "no reply should be sent when no buffer exists — waiter must be registered"
    );

    // Waiter must be in pending_signal_calls
    assert!(
        state.pending_signal_calls.contains_key("missing"),
        "pending waiter must be registered in pending_signal_calls"
    );
}

#[tokio::test]
async fn wait_for_signal_consumes_fifo_from_vec() {
    let mut state = make_procedural_state();

    // Seed two buffered signals for same name.
    if let ParadigmState::Procedural(s) = &mut state.paradigm_state {
        s.received_signals.insert(
            "retry".to_string(),
            vec![Bytes::from_static(b"1"), Bytes::from_static(b"2")],
        );
    }

    // First wait_for_signal consumes "1"
    let (tx1, rx1) = tokio::sync::oneshot::channel();
    handle_wait_for_signal(&mut state, 0, "retry".to_string(), tx1.into()).await;
    assert_eq!(
        rx1.await.expect("reply").expect("ok"),
        Bytes::from_static(b"1")
    );

    // Second wait_for_signal consumes "2"
    let (tx2, rx2) = tokio::sync::oneshot::channel();
    handle_wait_for_signal(&mut state, 1, "retry".to_string(), tx2.into()).await;
    assert_eq!(
        rx2.await.expect("reply").expect("ok"),
        Bytes::from_static(b"2")
    );

    // Buffer should be empty now
    if let ParadigmState::Procedural(s) = &state.paradigm_state {
        assert!(
            s.received_signals.get("retry").is_none_or(std::vec::Vec::is_empty),
            "buffer must be empty after consuming both signals"
        );
    }
}
