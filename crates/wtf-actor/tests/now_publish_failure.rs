//! Bug regression: handle_now must NOT reply when event_store publish fails.
//!
//! BUG: handle_now samples a fresh timestamp, then:
//!   1. If event_store is None: returns the ts without persisting it.
//!   2. If publish fails: returns the ts without persisting it.
//! On restart, a different ts will be sampled — breaking determinism.
//!
//! Fix: only call reply.send(ts) AFTER a successful publish + inject_event.
//! If publish fails or store is None, drop the reply port (caller hangs/timeouts).

use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use wtf_actor::{
    instance::{lifecycle::ParadigmState, procedural_utils::handle_now, state::InstanceState},
    messages::{InstanceArguments, InstancePhase, WorkflowParadigm},
    procedural::ProceduralActorState,
};
use wtf_common::{
    storage::{EventStore, ReplayStream},
    InstanceId, NamespaceId, WorkflowEvent, WtfError,
};

/// EventStore that always fails to publish.
#[derive(Debug)]
struct AlwaysFailStore;

#[async_trait]
impl EventStore for AlwaysFailStore {
    async fn publish(
        &self,
        _ns: &NamespaceId,
        _inst: &InstanceId,
        _event: WorkflowEvent,
    ) -> Result<u64, WtfError> {
        Err(WtfError::nats_publish("simulated publish failure"))
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

fn test_state_with_failing_store() -> InstanceState {
    use std::sync::Arc;
    let args = InstanceArguments {
        namespace: NamespaceId::new("test"),
        instance_id: InstanceId::new("inst-01"),
        workflow_type: "wf".into(),
        paradigm: WorkflowParadigm::Procedural,
        input: Bytes::from_static(b"{}"),
        engine_node_id: "node-1".into(),
        event_store: Some(Arc::new(AlwaysFailStore)),
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

/// handle_now must NOT send a reply when publish fails.
/// Currently FAILS because it always calls reply.send(ts) regardless.
#[tokio::test]
async fn handle_now_does_not_reply_when_publish_fails() {
    let mut state = test_state_with_failing_store();
    let (tx, rx) = tokio::sync::oneshot::channel::<chrono::DateTime<chrono::Utc>>();

    handle_now(&mut state, 0, tx.into()).await;

    // rx should be closed (dropped) — not received — because the value wasn't persisted
    let result = tokio::time::timeout(std::time::Duration::from_millis(10), rx).await;

    assert!(
        result.is_err() || result.unwrap().is_err(),
        "handle_now must not send a reply when publish fails — the value was not persisted \
         so returning it would cause non-determinism on restart"
    );
}
