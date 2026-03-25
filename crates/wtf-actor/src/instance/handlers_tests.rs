//! Tests for `instance::handlers`.

use super::*;
use crate::instance::lifecycle::ParadigmState;
use crate::messages::{InstanceArguments, InstanceMsg, TerminateError};
use async_trait::async_trait;
use bytes::Bytes;
use ractor::{Actor as _, ActorRef};
use std::sync::Arc;
use wtf_common::storage::{EventStore, ReplayBatch, ReplayStream, ReplayedEvent};
use wtf_common::{InstanceId, NamespaceId, WorkflowEvent, WorkflowParadigm, WtfError};
use wtf_storage::snapshots::open_snapshot_db;

// ---------------------------------------------------------------------------
// Mock stores
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct EmptyReplayStream;

#[async_trait]
impl ReplayStream for EmptyReplayStream {
    async fn next_event(&mut self) -> Result<ReplayBatch, WtfError> {
        Ok(ReplayBatch::TailReached)
    }
    async fn next_live_event(&mut self) -> Result<ReplayedEvent, WtfError> {
        std::future::pending().await
    }
}

/// `EventStore` that publishes successfully and returns seq=42.
#[derive(Debug)]
struct MockOkEventStore;

#[async_trait]
impl EventStore for MockOkEventStore {
    async fn publish(
        &self,
        _ns: &NamespaceId,
        _inst: &InstanceId,
        _event: WorkflowEvent,
    ) -> Result<u64, WtfError> {
        Ok(42)
    }
    async fn open_replay_stream(
        &self,
        _ns: &NamespaceId,
        _inst: &InstanceId,
        _from_seq: u64,
    ) -> Result<Box<dyn ReplayStream>, WtfError> {
        Ok(Box::new(EmptyReplayStream))
    }
}

/// `EventStore` that always fails on publish (for failure-path tests).
#[derive(Debug)]
struct MockFailEventStore;

#[async_trait]
impl EventStore for MockFailEventStore {
    async fn publish(
        &self,
        _ns: &NamespaceId,
        _inst: &InstanceId,
        _event: WorkflowEvent,
    ) -> Result<u64, WtfError> {
        Err(WtfError::nats_publish("mock publish failure"))
    }
    async fn open_replay_stream(
        &self,
        _ns: &NamespaceId,
        _inst: &InstanceId,
        _from_seq: u64,
    ) -> Result<Box<dyn ReplayStream>, WtfError> {
        Ok(Box::new(EmptyReplayStream))
    }
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn test_args_with_stores(
    event_store: Option<Arc<dyn EventStore>>,
    snapshot_db: Option<sled::Db>,
) -> InstanceArguments {
    InstanceArguments {
        namespace: NamespaceId::new("test-ns"),
        instance_id: InstanceId::new("test-instance"),
        workflow_type: "test-workflow".into(),
        paradigm: WorkflowParadigm::Procedural,
        input: Bytes::from_static(b"{}"),
        engine_node_id: "test-node".into(),
        event_store,
        state_store: None,
        task_queue: None,
        snapshot_db,
        procedural_workflow: None,
        workflow_definition: None,
    }
}

fn make_test_state(
    event_store: Option<Arc<dyn EventStore>>,
    snapshot_db: Option<sled::Db>,
    events_since: u32,
) -> InstanceState {
    let args = test_args_with_stores(event_store, snapshot_db);
    let mut state = InstanceState::initial(args);
    state.total_events_applied = 100;
    state.events_since_snapshot = events_since;
    state
}

fn make_temp_sled() -> sled::Db {
    let dir = tempfile::tempdir().expect("tempdir");
    open_snapshot_db(dir.path()).expect("open db")
}

// ---------------------------------------------------------------------------
// Snapshot trigger tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn snapshot_trigger_no_event_store_returns_error() {
    let db = make_temp_sled();
    let mut state = make_test_state(None, Some(db), SNAPSHOT_INTERVAL);

    let result = handlers::snapshot::handle_snapshot_trigger(&mut state).await;

    assert!(matches!(result, Err(_)), "should fail when event_store is None");
    assert_eq!(
        state.events_since_snapshot, SNAPSHOT_INTERVAL,
        "counter must NOT be reset on error"
    );
}

#[tokio::test]
async fn snapshot_trigger_no_snapshot_db_returns_error() {
    let mut state =
        make_test_state(Some(Arc::new(MockOkEventStore)), None, SNAPSHOT_INTERVAL);

    let result = handlers::snapshot::handle_snapshot_trigger(&mut state).await;

    assert!(matches!(result, Err(_)), "should fail when snapshot_db is None");
    assert_eq!(
        state.events_since_snapshot, SNAPSHOT_INTERVAL,
        "counter must NOT be reset on error"
    );
}

#[tokio::test]
async fn snapshot_trigger_success_resets_counter() {
    let db = make_temp_sled();
    let mut state =
        make_test_state(Some(Arc::new(MockOkEventStore)), Some(db), SNAPSHOT_INTERVAL);

    let result = handlers::snapshot::handle_snapshot_trigger(&mut state).await;

    let Ok(()) = result else {
        panic!("should succeed with both stores present, got: {:?}", result)
    };
    assert_eq!(
        state.events_since_snapshot, 0,
        "counter must be reset on success"
    );
}

#[tokio::test]
async fn snapshot_trigger_failure_keeps_counter() {
    let db = make_temp_sled();
    let mut state =
        make_test_state(Some(Arc::new(MockFailEventStore)), Some(db), SNAPSHOT_INTERVAL);

    let result = handlers::snapshot::handle_snapshot_trigger(&mut state).await;

    let Ok(()) = result else {
        panic!("snapshot failure is non-fatal — returns Ok, got: {:?}", result)
    };
    assert_eq!(
        state.events_since_snapshot, SNAPSHOT_INTERVAL,
        "counter must NOT be reset when write_instance_snapshot fails"
    );
}

#[tokio::test]
async fn snapshot_trigger_preserves_paradigm_state() {
    let db = make_temp_sled();
    let mut state =
        make_test_state(Some(Arc::new(MockOkEventStore)), Some(db), SNAPSHOT_INTERVAL);

    let before_serialized = rmp_serde::to_vec_named(&state.paradigm_state)
        .expect("serialize before");
    let _ = handlers::snapshot::handle_snapshot_trigger(&mut state).await;
    let after_serialized = rmp_serde::to_vec_named(&state.paradigm_state)
        .expect("serialize after");

    assert_eq!(
        before_serialized, after_serialized,
        "paradigm_state must be unchanged (write-aside)"
    );
}

// ---------------------------------------------------------------------------
// Signal handler tests
// ---------------------------------------------------------------------------

#[test]
fn initial_state_has_empty_pending_signal_calls() {
    let args = test_args_with_stores(None, None);
    let state = InstanceState::initial(args);
    assert!(
        state.pending_signal_calls.is_empty(),
        "pending_signal_calls must be empty after initial()"
    );
}

#[tokio::test]
async fn handle_signal_delivers_payload_to_pending_call() {
    let mut state = make_test_state(
        Some(Arc::new(MockOkEventStore)),
        None,
        0,
    );

    // Register a pending signal call
    let (pending_tx, pending_rx) =
        tokio::sync::oneshot::channel::<Result<Bytes, WtfError>>();
    state
        .pending_signal_calls
        .insert("order_approved".to_string(), pending_tx.into());

    // Caller's reply port
    let (caller_tx, caller_rx) =
        tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    let payload = Bytes::from_static(b"first");
    handlers::handle_signal(
        &mut state,
        "order_approved".to_string(),
        payload.clone(),
        caller_tx.into(),
    )
    .await
    .expect("ok");
    let caller_result = caller_rx.await;
    let Ok(Ok(())) = caller_result else {
        panic!("caller should receive Ok(()), got: {:?}", caller_result)
    };

    // First delivered immediately [INV-4]
    let first_result = pending_rx.await.expect("pending reply channel not dropped");
    let first_received = first_result.expect("ok");
    assert_eq!(first_received, Bytes::from_static(b"first"));
    assert!(!state.pending_signal_calls.contains_key("release"));

    // Step 2: Second signal -> no waiter -> buffered
    let (caller_tx2, caller_rx2) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();
    handlers::handle_signal(
        &mut state,
        "release".to_string(),
        Bytes::from_static(b"second"),
        caller_tx2.into(),
    )
    .await
    .expect("ok");
    let caller_result = caller_rx2.await;
    let Ok(Ok(())) = caller_result else {
        panic!("caller should receive Ok(()), got: {:?}", caller_result)
    };

    // No signal discarded [INV-4]
    if let ParadigmState::Procedural(s) = &state.paradigm_state {
        let buffered = s
            .received_signals
            .get("release")
            .expect("second signal must be buffered");
        assert_eq!(buffered.len(), 1);
        assert_eq!(buffered[0], Bytes::from_static(b"second"));
    }
}

#[tokio::test]
async fn postcondition_signal_event_published_to_event_store() {
    let mut state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0);

    let (caller_tx, caller_rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_signal(
        &mut state,
        "test".to_string(),
        Bytes::from_static(b"data"),
        caller_tx.into(),
    )
    .await
    .expect("ok");
    let caller_result = caller_rx.await;
    let Ok(Ok(())) = caller_result else {
        panic!("caller should receive Ok(()), got: {:?}", caller_result)
    };

    assert_eq!(
        state.total_events_applied, 101,
        "POST-2: total_events_applied must be 101 after one signal event"
    );
    assert_eq!(state.events_since_snapshot, 1, "POST-4: events_since_snapshot must increment");
}

#[tokio::test]
async fn postcondition_pending_signal_call_removed_after_delivery() {
    let mut state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0);

    let (pending_tx, pending_rx) =
        tokio::sync::oneshot::channel::<Result<Bytes, WtfError>>();
    state
        .pending_signal_calls
        .insert("delivery".to_string(), pending_tx.into());

    let (caller_tx, caller_rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_signal(
        &mut state,
        "delivery".to_string(),
        Bytes::from_static(b"payload"),
        caller_tx.into(),
    )
    .await
    .expect("ok");
    let caller_result = caller_rx.await;
    let Ok(Ok(())) = caller_result else {
        panic!("caller should receive Ok(()), got: {:?}", caller_result)
    };

    // [POST-3] pending entry removed
    assert!(
        !state.pending_signal_calls.contains_key("delivery"),
        "pending_signal_calls must NOT contain 'delivery' after delivery"
    );

    let pending_result = pending_rx.await.expect("pending reply channel not dropped");
    let received = pending_result.expect("ok");
    assert_eq!(received, Bytes::from_static(b"payload"));
}

#[tokio::test]
async fn invariant_signal_payload_matches_what_was_sent() {
    let mut state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0);

    let (pending_tx, pending_rx) =
        tokio::sync::oneshot::channel::<Result<Bytes, WtfError>>();
    state
        .pending_signal_calls
        .insert("match".to_string(), pending_tx.into());

    let (caller_tx, caller_rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    let original_payload = Bytes::from_static(b"exact-match-payload");
    handlers::handle_signal(
        &mut state,
        "match".to_string(),
        original_payload.clone(),
        caller_tx.into(),
    )
    .await
    .expect("ok");
    let caller_result = caller_rx.await;
    let Ok(Ok(())) = caller_result else {
        panic!("caller should receive Ok(()), got: {:?}", caller_result)
    };

    // [INV-2] Exact byte equality
    let pending_result = pending_rx.await.expect("pending reply channel not dropped");
    let received = pending_result.expect("ok");
    assert_eq!(
        received, original_payload,
        "INV-2: received payload must exactly match sent payload"
    );
}

#[tokio::test]
async fn invariant_received_signals_fifo_ordering() {
    let mut state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0);

    // Buffer first signal
    let (caller_tx1, caller_rx1) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();
    handlers::handle_signal(
        &mut state,
        "queue".to_string(),
        Bytes::from_static(b"alpha"),
        caller_tx1.into(),
    )
    .await
    .expect("ok");
    let caller_result = caller_rx1.await;
    let Ok(Ok(())) = caller_result else {
        panic!("caller should receive Ok(()), got: {:?}", caller_result)
    };

    // Buffer second signal
    let (caller_tx2, caller_rx2) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();
    handlers::handle_signal(
        &mut state,
        "queue".to_string(),
        Bytes::from_static(b"beta"),
        caller_tx2.into(),
    )
    .await
    .expect("ok");
    let caller_result = caller_rx2.await;
    let Ok(Ok(())) = caller_result else {
        panic!("caller should receive Ok(()), got: {:?}", caller_result)
    };

    // Consume first -> "alpha"
    let (wait_tx1, wait_rx1) = tokio::sync::oneshot::channel::<Result<Bytes, WtfError>>();
    procedural::handle_wait_for_signal(&mut state, 0, "queue".to_string(), wait_tx1.into()).await;
    let first = wait_rx1
        .await
        .expect("wait reply channel not dropped")
        .expect("ok");
    assert_eq!(first, Bytes::from_static(b"alpha"), "first consumed must be 'alpha'");

    // Consume second -> "beta"
    let (wait_tx2, wait_rx2) = tokio::sync::oneshot::channel::<Result<Bytes, WtfError>>();
    procedural::handle_wait_for_signal(&mut state, 1, "queue".to_string(), wait_tx2.into()).await;
    let second = wait_rx2
        .await
        .expect("wait reply channel not dropped")
        .expect("ok");
    assert_eq!(second, Bytes::from_static(b"beta"), "second consumed must be 'beta'");

    // [INV-3] FIFO order preserved
    assert_ne!(first, second, "FIFO: alpha and beta must arrive in order");
}

// ---------------------------------------------------------------------------
// Terminate (handle_cancel) handler-level tests (wtf-k00f)
//
// Validates the full cancel path from instance-level handler:
//   event publish -> reply -> actor stop.
// Run: cargo test -p wtf-actor -- terminate
// ---------------------------------------------------------------------------

/// `EventStore` that captures the last published event for assertion.
#[derive(Debug)]
struct CapturingEventStore {
    last_published: std::sync::Mutex<Option<WorkflowEvent>>,
}

impl CapturingEventStore {
    fn new() -> Self {
        Self {
            last_published: std::sync::Mutex::new(None),
        }
    }

    fn take_last_published(&self) -> Option<WorkflowEvent> {
        self.last_published
            .lock()
            .expect("mutex")
            .take()
    }
}

#[async_trait]
impl EventStore for CapturingEventStore {
    async fn publish(
        &self,
        _ns: &NamespaceId,
        _inst: &InstanceId,
        event: WorkflowEvent,
    ) -> Result<u64, WtfError> {
        let mut guard = self.last_published.lock().expect("mutex");
        *guard = Some(event);
        Ok(42)
    }
    async fn open_replay_stream(
        &self,
        _ns: &NamespaceId,
        _inst: &InstanceId,
        _from_seq: u64,
    ) -> Result<Box<dyn ReplayStream>, WtfError> {
        Ok(Box::new(EmptyReplayStream))
    }
}

fn cancel_test_state(
    event_store: Option<Arc<dyn EventStore>>,
) -> InstanceState {
    let args = InstanceArguments {
        namespace: NamespaceId::new("e2e-term-test"),
        instance_id: InstanceId::new("inst-cancel-01"),
        workflow_type: "test-workflow".into(),
        paradigm: WorkflowParadigm::Procedural,
        input: Bytes::from_static(b"{}"),
        engine_node_id: "test-node".into(),
        event_store,
        state_store: None,
        task_queue: None,
        snapshot_db: None,
        procedural_workflow: None,
        workflow_definition: None,
    };
    InstanceState::initial(args)
}

/// Helper: spawn a `NullActor` that accepts `InstanceMsg` so we get a valid `ActorRef`.
/// The actor ignores all messages (including Cancel).
/// Returns both the ref and JoinHandle so callers can await actor termination.
async fn spawn_null_instance_actor() -> (ActorRef<InstanceMsg>, ractor::concurrency::tokio_primitives::JoinHandle<()>) {
    struct NullInstanceActor;
    #[async_trait::async_trait]
    impl ractor::Actor for NullInstanceActor {
        type Msg = InstanceMsg;
        type State = ();
        type Arguments = ();
        async fn pre_start(
            &self,
            _: ActorRef<Self::Msg>,
            _: Self::Arguments,
        ) -> Result<(), ractor::ActorProcessingErr> {
            Ok(())
        }
    }
    NullInstanceActor::spawn(None, NullInstanceActor, ())
        .await
        .expect("null instance actor spawned")
}

/// Helper: spawn an actor that deliberately drops Cancel reply ports (never replies).
/// Used to test the timeout path at the orchestrator level.
async fn spawn_silent_cancel_actor() -> ActorRef<InstanceMsg> {
    struct SilentCancelActor;
    #[async_trait::async_trait]
    impl ractor::Actor for SilentCancelActor {
        type Msg = InstanceMsg;
        type State = ();
        type Arguments = ();
        async fn pre_start(
            &self,
            _: ActorRef<Self::Msg>,
            _: Self::Arguments,
        ) -> Result<(), ractor::ActorProcessingErr> {
            Ok(())
        }
        async fn handle(
            &self,
            _myself: ActorRef<Self::Msg>,
            msg: Self::Msg,
            _state: &mut Self::State,
        ) -> Result<(), ractor::ActorProcessingErr> {
            // Swallow Cancel messages — never reply
            if let InstanceMsg::Cancel { reply: _, .. } = msg {
                std::future::pending::<()>().await;
            }
            Ok(())
        }
    }
    let (ref_, _handle) = SilentCancelActor::spawn(None, SilentCancelActor, ())
        .await
        .expect("silent cancel actor spawned");
    ref_
}

// --- Happy path tests ---

#[tokio::test]
async fn terminate_running_instance_returns_ok() {
    let mut state = cancel_test_state(Some(Arc::new(MockOkEventStore)));
    let (actor_ref, _handle) = spawn_null_instance_actor().await;

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_cancel(
        actor_ref.clone(),
        &mut state,
        "api-terminate".to_string(),
        tx.into(),
    )
    .await
    .expect("handle_cancel returns Ok(())");

    let reply = rx.await.expect("reply channel not dropped");
    assert!(reply.is_ok(), "cancel reply must be Ok(()), got: {:?}", reply.err());

    actor_ref.stop(Some("test complete".into()));
}

#[tokio::test]
async fn terminate_publishes_instance_cancelled_event() {
    let store = Arc::new(CapturingEventStore::new());
    let mut state = cancel_test_state(Some(store.clone() as Arc<dyn EventStore>));
    let (actor_ref, _handle) = spawn_null_instance_actor().await;

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_cancel(
        actor_ref.clone(),
        &mut state,
        "api-terminate".to_string(),
        tx.into(),
    )
    .await
    .expect("handle_cancel ok");

    let _ = rx.await;

    let captured = store.take_last_published();
    assert!(
        captured.is_some(),
        "EventStore.publish must have been called with InstanceCancelled"
    );
    if let Some(WorkflowEvent::InstanceCancelled { reason }) = captured {
        assert_eq!(
            reason, "api-terminate",
            "I-3: reason must match the reason passed to handle_cancel"
        );
    } else {
        panic!(
            "expected WorkflowEvent::InstanceCancelled, got: {captured:?}"
        );
    }

    actor_ref.stop(Some("test complete".into()));
}

// --- Not found tests (orchestrator-level) ---

#[tokio::test]
async fn terminate_nonexistent_instance_returns_not_found() {
    let mut state =
        crate::master::state::OrchestratorState::new(crate::master::state::OrchestratorConfig::default());
    let instance_id = InstanceId::new("nonexistent-fake-id");

    let (tx, rx) = ractor::concurrency::oneshot();
    crate::master::handlers::handle_terminate(
        &mut state,
        instance_id.clone(),
        "test".to_owned(),
        tx.into(),
    )
    .await;

    let reply = rx.await.expect("reply received");
    assert!(
        reply.is_err(),
        "terminate nonexistent instance must return Err"
    );
    if let Err(TerminateError::NotFound(id)) = reply {
        assert_eq!(id, instance_id);
    } else {
        panic!(
            "expected TerminateError::NotFound, got: {reply:?}"
        );
    }
}

// --- Double terminate tests ---

#[tokio::test]
async fn double_terminate_returns_not_found() {
    // First terminate: use a real actor that will be stopped by handle_cancel
    let mut state = cancel_test_state(Some(Arc::new(MockOkEventStore)));
    let (actor_ref, handle) = spawn_null_instance_actor().await;

    let (tx1, rx1) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();
    handlers::handle_cancel(
        actor_ref.clone(),
        &mut state,
        "api-terminate".to_string(),
        tx1.into(),
    )
    .await
    .expect("first cancel ok");
    let _ = rx1.await;

    // Wait for the actor to actually stop using event-driven synchronization
    let _ = handle.await;

    // Second terminate: actor is dead, so call_cancel should get SenderError
    // which maps to NotFound. We test this via the orchestrator handler.
    let mut orch_state =
        crate::master::state::OrchestratorState::new(crate::master::state::OrchestratorConfig::default());
    let instance_id = InstanceId::new("double-term-inst");
    // Re-register the (now dead) actor ref to test the SenderError path
    orch_state.register(instance_id.clone(), actor_ref.clone());

    let (tx2, rx2) = ractor::concurrency::oneshot();
    crate::master::handlers::handle_terminate(
        &mut orch_state,
        instance_id.clone(),
        "again".to_owned(),
        tx2.into(),
    )
    .await;

    let reply = rx2.await.expect("second reply received");
    assert!(
        matches!(reply, Err(TerminateError::NotFound(_))),
        "double terminate must return NotFound, got: {reply:?}"
    );
}

// --- Timeout tests ---

#[tokio::test]
async fn terminate_returns_timeout_when_instance_does_not_respond() {
    let silent_ref = spawn_silent_cancel_actor().await;

    let mut orch_state =
        crate::master::state::OrchestratorState::new(crate::master::state::OrchestratorConfig::default());
    let instance_id = InstanceId::new("timeout-inst");
    orch_state.register(instance_id.clone(), silent_ref.clone());

    let (tx, rx) = ractor::concurrency::oneshot();
    crate::master::handlers::handle_terminate(
        &mut orch_state,
        instance_id.clone(),
        "test-timeout".to_owned(),
        tx.into(),
    )
    .await;

    let reply = rx.await.expect("timeout reply received");
    assert!(
        matches!(reply, Err(TerminateError::Timeout(ref id)) if id == &instance_id),
        "expected TerminateError::Timeout, got: {reply:?}"
    );

    silent_ref.stop(Some("test complete".into()));
}

// --- No EventStore tests ---

#[tokio::test]
async fn terminate_with_no_event_store_still_replies_ok() {
    let mut state = cancel_test_state(None);
    let (actor_ref, _handle) = spawn_null_instance_actor().await;

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_cancel(
        actor_ref.clone(),
        &mut state,
        "no-store".to_string(),
        tx.into(),
    )
    .await
    .expect("handle_cancel ok even without event_store");

    let reply = rx.await.expect("reply channel not dropped");
    assert!(
        reply.is_ok(),
        "PO-E3: handle_cancel must reply Ok(()) even when event_store is None, got: {:?}",
        reply.err()
    );

    actor_ref.stop(Some("test complete".into()));
}

// --- Publish failure tests ---

#[tokio::test]
async fn terminate_when_publish_fails_still_replies_ok() {
    let mut state = cancel_test_state(Some(Arc::new(FailingEventStore)));
    let (actor_ref, _handle) = spawn_null_instance_actor().await;

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_cancel(
        actor_ref.clone(),
        &mut state,
        "publish-fail".to_string(),
        tx.into(),
    )
    .await
    .expect("handle_cancel ok despite publish failure");

    let reply = rx.await.expect("reply channel not dropped");
    assert!(
        reply.is_ok(),
        "handle_cancel must reply Ok(()) even when publish fails (data-loss scenario)"
    );

    actor_ref.stop(Some("test complete".into()));
}

/// `EventStore` that always fails on publish — used to test the data-loss path.
#[derive(Debug)]
struct FailingEventStore;

#[async_trait]
impl EventStore for FailingEventStore {
    async fn publish(
        &self,
        _ns: &NamespaceId,
        _inst: &InstanceId,
        _event: WorkflowEvent,
    ) -> Result<u64, WtfError> {
        Err(WtfError::nats_publish("simulated failure"))
    }
    async fn open_replay_stream(
        &self,
        _ns: &NamespaceId,
        _inst: &InstanceId,
        _from_seq: u64,
    ) -> Result<Box<dyn ReplayStream>, WtfError> {
        Ok(Box::new(EmptyReplayStream))
    }
}

// --- Event ordering tests ---

#[tokio::test]
async fn terminate_reason_propagates_to_instance_cancelled_event() {
    let store = Arc::new(CapturingEventStore::new());
    let mut state = cancel_test_state(Some(store.clone() as Arc<dyn EventStore>));
    let (actor_ref, _handle) = spawn_null_instance_actor().await;

    let custom_reason = "my-custom-reason".to_string();
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_cancel(actor_ref, &mut state, custom_reason.clone(), tx.into())
        .await
        .expect("handle_cancel ok");

    let _ = rx.await;

    let captured = store.take_last_published();
    if let Some(WorkflowEvent::InstanceCancelled { reason }) = captured {
        assert_eq!(
            reason, custom_reason,
            "I-3: reason in InstanceCancelled must match the reason passed to handle_cancel"
        );
    } else {
        panic!(
            "expected WorkflowEvent::InstanceCancelled, got: {captured:?}"
        );
    }
}

// --- Structural invariant tests (source-level, no NATS required) ---

#[test]
fn invariant_reply_sent_before_actor_stop() {
    // I-2: reply.send must appear before myself_ref.stop in handle_cancel.
    // This is a source-level structural invariant verified by string analysis.
    let source = include_str!("handlers.rs");

    // Extract just the handle_cancel function
    let cancel_start = source
        .find("pub(crate) async fn handle_cancel")
        .expect("source must contain handle_cancel");
    let cancel_end = source[cancel_start..]
        .find("\nasync fn ")
        .map_or(source.len(), |i| cancel_start + i);
    let cancel_fn = &source[cancel_start..cancel_end];

    let reply_pos = cancel_fn
        .find("reply.send")
        .expect("handle_cancel must contain 'reply.send'");
    let stop_pos = cancel_fn
        .find("myself_ref.stop")
        .expect("handle_cancel must contain 'myself_ref.stop'");

    assert!(
        reply_pos < stop_pos,
        "I-2 violated: 'reply.send' (pos {reply_pos}) must appear before 'myself_ref.stop' (pos {stop_pos})"
    );
}

#[test]
fn invariant_event_published_before_actor_stop() {
    // I-1: store.publish must appear before myself_ref.stop in handle_cancel.
    // This is a source-level structural invariant verified by string analysis.
    let source = include_str!("handlers.rs");

    // Extract just the handle_cancel function
    let cancel_start = source
        .find("pub(crate) async fn handle_cancel")
        .expect("source must contain handle_cancel");
    let cancel_end = source[cancel_start..]
        .find("\nasync fn ")
        .map_or(source.len(), |i| cancel_start + i);
    let cancel_fn = &source[cancel_start..cancel_end];

    let publish_pos = cancel_fn
        .find(".publish(")
        .expect("handle_cancel must contain '.publish('");
    let stop_pos = cancel_fn
        .find("myself_ref.stop")
        .expect("handle_cancel must contain 'myself_ref.stop'");

    assert!(
        publish_pos < stop_pos,
        "I-1 violated: '.publish(' (pos {publish_pos}) must appear before 'myself_ref.stop' (pos {stop_pos})"
    );
}

#[test]
fn invariant_no_unwrap_in_terminate_path() {
    // I-5: The entire terminate chain must use only match/map_err — no unwrap/expect.
    // Source-level assertion: handle_cancel in handlers.rs must not contain unwrap.
    let source = include_str!("handlers.rs");
    // Extract just the handle_cancel function body
    let cancel_start = source
        .find("pub(crate) async fn handle_cancel")
        .expect("source must contain handle_cancel");
    let cancel_end = source[cancel_start..]
        .find("\nasync fn ")
        .map_or(source.len(), |i| cancel_start + i);
    let cancel_fn = &source[cancel_start..cancel_end];

    assert!(
        !cancel_fn.contains(".unwrap()"),
        "I-5 violated: handle_cancel must not contain .unwrap()"
    );
    assert!(
        !cancel_fn.contains(".expect("),
        "I-5 violated: handle_cancel must not contain .expect("
    );
}
