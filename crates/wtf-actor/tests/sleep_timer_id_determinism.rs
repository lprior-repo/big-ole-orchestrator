//! Bug regression: handle_sleep must use a deterministic timer_id derived from op_id.

use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use wtf_actor::{
    instance::{lifecycle::ParadigmState, procedural::handle_sleep, state::InstanceState},
    messages::{InstanceArguments, InstancePhase, WorkflowParadigm},
    procedural::ProceduralActorState,
};
use wtf_common::{
    storage::{EventStore, ReplayStream},
    InstanceId, NamespaceId, TimerId, WorkflowEvent, WtfError,
};

#[derive(Debug)]
struct AlwaysOkStore;

#[async_trait]
impl EventStore for AlwaysOkStore {
    async fn publish(
        &self,
        _ns: &NamespaceId,
        _inst: &InstanceId,
        _event: WorkflowEvent,
    ) -> Result<u64, WtfError> {
        Ok(1)
    }

    async fn open_replay_stream(
        &self,
        _ns: &NamespaceId,
        _inst: &InstanceId,
        _from_seq: u64,
    ) -> Result<Box<dyn ReplayStream>, WtfError> {
        Err(WtfError::nats_publish("not used"))
    }
}

fn test_state(instance_id: &str) -> InstanceState {
    let args = InstanceArguments {
        namespace: NamespaceId::new("test"),
        instance_id: InstanceId::new(instance_id),
        workflow_type: "wf".into(),
        paradigm: WorkflowParadigm::Procedural,
        input: Bytes::from_static(b"{}"),
        engine_node_id: "node-1".into(),
        event_store: Some(Arc::new(AlwaysOkStore)),
        state_store: None,
        task_queue: None,
        snapshot_db: None,
        procedural_workflow: None,
        workflow_definition: None,
    };
    InstanceState {
        paradigm_state: ParadigmState::Procedural(ProceduralActorState::new()),
        args,
        phase: InstancePhase::Live,
        total_events_applied: 0,
        events_since_snapshot: 0,
        pending_activity_calls: HashMap::new(),
        pending_timer_calls: HashMap::new(),
        pending_signal_calls: HashMap::new(),
        procedural_task: None,
        live_subscription_task: None,
    }
}

/// handle_sleep must register the pending timer with a deterministic id = TimerId::procedural(instance_id, op_id).
#[tokio::test]
async fn handle_sleep_uses_deterministic_timer_id_from_op_id() {
    let instance_id = "inst-42";
    let op_id = 7u32;
    let expected_timer_id = TimerId::procedural(&InstanceId::new(instance_id), op_id);

    let mut state = test_state(instance_id);
    let (tx, _rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handle_sleep(
        &mut state,
        op_id,
        std::time::Duration::from_secs(1),
        tx.into(),
    )
    .await;

    assert!(
        state.pending_timer_calls.contains_key(&expected_timer_id),
        "handle_sleep must use deterministic timer_id {:?}, not a random ULID. Keys: {:?}",
        expected_timer_id,
        state.pending_timer_calls.keys().collect::<Vec<_>>()
    );
}

/// Same op_id on two successive calls (simulating restart) must produce the same timer_id.
#[tokio::test]
async fn handle_sleep_same_op_id_produces_same_timer_id_on_restart() {
    let instance_id = "inst-99";
    let op_id = 3u32;

    let mut state1 = test_state(instance_id);
    let (tx1, _rx1) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();
    handle_sleep(
        &mut state1,
        op_id,
        std::time::Duration::from_secs(5),
        tx1.into(),
    )
    .await;

    let mut state2 = test_state(instance_id);
    let (tx2, _rx2) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();
    handle_sleep(
        &mut state2,
        op_id,
        std::time::Duration::from_secs(5),
        tx2.into(),
    )
    .await;

    let keys1: Vec<_> = state1.pending_timer_calls.keys().cloned().collect();
    let keys2: Vec<_> = state2.pending_timer_calls.keys().cloned().collect();
    assert_eq!(
        keys1, keys2,
        "same op_id must yield identical timer_id across restarts"
    );
}
