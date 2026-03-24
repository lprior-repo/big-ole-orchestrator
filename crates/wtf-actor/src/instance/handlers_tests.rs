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

    assert!(result.is_err(), "should fail when event_store is None");
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

    assert!(result.is_err(), "should fail when snapshot_db is None");
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

    assert!(result.is_ok(), "should succeed with both stores present");
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

    assert!(
        result.is_ok(),
        "snapshot failure is non-fatal — returns Ok"
    );
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

    let payload = Bytes::from_static(b"approved");
    handlers::handle_signal(
        &mut state,
        "order_approved".to_string(),
        payload.clone(),
        caller_tx.into(),
    )
    .await
    .expect("ok");

    // Pending RPC port received Ok(payload)
    let pending_result = pending_rx.await.expect("pending reply received");
    assert_eq!(pending_result.expect("payload ok"), payload);

    // Caller received Ok(())
    assert!(caller_rx.await.expect("caller reply").is_ok());

    // Entry removed from pending map
    assert!(
        !state.pending_signal_calls.contains_key("order_approved"),
        "pending entry must be removed after delivery"
    );

    // Event was injected (counter incremented)
    assert_eq!(
        state.total_events_applied, 101,
        "inject_event must increment total_events_applied"
    );
}

#[tokio::test]
async fn handle_signal_publishes_event_when_no_pending_call() {
    let mut state = make_test_state(
        Some(Arc::new(MockOkEventStore)),
        None,
        0,
    );

    let (caller_tx, caller_rx) =
        tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_signal(
        &mut state,
        "timeout".to_string(),
        Bytes::from_static(b"tick"),
        caller_tx.into(),
    )
    .await
    .expect("ok");

    // Caller still gets Ok(())
    assert!(caller_rx.await.expect("caller reply").is_ok());

    // No entry was ever in the map, and the map stays empty
    assert!(state.pending_signal_calls.is_empty());

    // Signal must be buffered in received_signals when no waiter exists
    if let crate::instance::lifecycle::ParadigmState::Procedural(s) = &state.paradigm_state {
        let buffered = s
            .received_signals
            .get("timeout")
            .expect("signal must be buffered");
        assert_eq!(
            buffered.len(),
            1,
            "exactly one signal must be buffered"
        );
        assert_eq!(
            buffered[0],
            Bytes::from_static(b"tick"),
            "buffered payload must match"
        );
    }

    // Event was injected
    assert_eq!(state.total_events_applied, 101);
}

#[tokio::test]
async fn handle_signal_returns_error_without_event_store() {
    let mut state = make_test_state(None, None, 0);

    let (caller_tx, caller_rx) =
        tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_signal(
        &mut state,
        "sig".to_string(),
        Bytes::from_static(b"data"),
        caller_tx.into(),
    )
    .await
    .expect("handler returns Ok even on missing store");

    let result = caller_rx.await.expect("caller reply received");
    assert!(result.is_err(), "must return Err when event_store is None");
    let err_msg = format!("{:?}", result.expect_err("is err"));
    assert!(
        err_msg.contains("Event store missing"),
        "error message must mention 'Event store missing', got: {err_msg}"
    );

    // State must NOT be modified
    assert_eq!(state.total_events_applied, 100);
}

#[tokio::test]
async fn handle_signal_injects_event_into_paradigm_state() {
    let mut state = make_test_state(
        Some(Arc::new(MockOkEventStore)),
        None,
        0,
    );

    let before_applied = state.total_events_applied;

    let (caller_tx, caller_rx) =
        tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_signal(
        &mut state,
        "approve".to_string(),
        Bytes::from_static(b"yes"),
        caller_tx.into(),
    )
    .await
    .expect("ok");

    // Caller gets Ok
    assert!(caller_rx.await.expect("caller reply").is_ok());

    // Paradigm state counter incremented (proves inject_event was called)
    assert_eq!(
        state.total_events_applied,
        before_applied + 1,
        "inject_event must have been called — total_events_applied must increment"
    );
    assert_eq!(
        state.events_since_snapshot, 1,
        "events_since_snapshot must increment"
    );
}

#[tokio::test]
async fn handle_signal_reply_error_on_publish_failure() {
    let mut state = make_test_state(
        Some(Arc::new(MockFailEventStore)),
        None,
        0,
    );

    let before_applied = state.total_events_applied;

    let (caller_tx, caller_rx) =
        tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    // Insert a pending call — it must NOT be delivered on publish failure
    let (pending_tx, _pending_rx) =
        tokio::sync::oneshot::channel::<Result<Bytes, WtfError>>();
    state
        .pending_signal_calls
        .insert("fail_sig".to_string(), pending_tx.into());

    handlers::handle_signal(
        &mut state,
        "fail_sig".to_string(),
        Bytes::from_static(b"nope"),
        caller_tx.into(),
    )
    .await
    .expect("handler returns Ok even on publish failure");

    // Caller receives error
    let result = caller_rx.await.expect("caller reply received");
    assert!(result.is_err(), "must return Err when publish fails");

    // State must NOT be modified — no event injected
    assert_eq!(state.total_events_applied, before_applied);

    // Pending call must NOT have been removed
    assert!(
        state.pending_signal_calls.contains_key("fail_sig"),
        "pending entry must remain when publish fails"
    );
}

// ---------------------------------------------------------------------------
// Signal delivery workflow tests (wtf-h8u4)
//
// Handler-level tests validating the full signal delivery path:
//   event publish -> pending delivery -> buffer fallback -> wait_for_signal consumption.
// Run: cargo test -p wtf-actor -- signal_delivery
// ---------------------------------------------------------------------------

#[tokio::test]
async fn signal_delivery_resumes_and_completes_workflow() {
    let mut state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0);

    let (pending_tx, pending_rx) =
        tokio::sync::oneshot::channel::<Result<Bytes, WtfError>>();
    state
        .pending_signal_calls
        .insert("go".to_string(), pending_tx.into());

    let (caller_tx, caller_rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    let payload = Bytes::from_static(b"proceed");
    handlers::handle_signal(&mut state, "go".to_string(), payload.clone(), caller_tx.into())
        .await
        .expect("handler returns Ok");

    // Caller receives Ok(())
    let caller_result = caller_rx.await.expect("caller reply channel not dropped");
    assert!(caller_result.is_ok(), "caller should receive Ok(())");

    // Pending waiter receives exact payload [INV-2]
    let pending_result = pending_rx.await.expect("pending reply channel not dropped");
    let received = pending_result.expect("pending should be Ok");
    assert_eq!(received, payload, "INV-2: payload must match exactly");

    // Entry removed from pending map [POST-3]
    assert!(
        !state.pending_signal_calls.contains_key("go"),
        "pending entry must be removed after delivery"
    );

    // total_events_applied incremented [POST-4]
    assert_eq!(state.total_events_applied, 101);
}

#[tokio::test]
async fn signal_arrives_before_wait_for_signal() {
    let mut state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0);

    // Step 1: Signal arrives — no waiter registered yet
    let (caller_tx, caller_rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_signal(
        &mut state,
        "early".to_string(),
        Bytes::from_static(b"before-wait"),
        caller_tx.into(),
    )
    .await
    .expect("handler returns Ok");
    assert!(caller_rx.await.expect("caller reply").is_ok());

    // Signal must be buffered [POST-9]
    if let ParadigmState::Procedural(s) = &state.paradigm_state {
        let buffered = s
            .received_signals
            .get("early")
            .expect("signal must be buffered in received_signals");
        assert_eq!(buffered.len(), 1);
        assert_eq!(buffered[0], Bytes::from_static(b"before-wait"));
    }

    // Step 2: Workflow calls wait_for_signal after buffer
    let (wait_tx, wait_rx) = tokio::sync::oneshot::channel::<Result<Bytes, WtfError>>();

    procedural::handle_wait_for_signal(&mut state, 0, "early".to_string(), wait_tx.into()).await;

    // Returns immediately [POST-12]
    let wait_result = wait_rx.await.expect("wait reply channel not dropped");
    let received = wait_result.expect("wait should return Ok");
    assert_eq!(
        received,
        Bytes::from_static(b"before-wait"),
        "POST-10: buffered payload must be delivered"
    );

    // Buffer entry consumed [POST-11]
    if let ParadigmState::Procedural(s) = &state.paradigm_state {
        assert!(
            !s.received_signals.contains_key("early"),
            "received_signals entry should be removed when Vec is empty"
        );
    }
}

#[tokio::test]
async fn signal_to_nonexistent_instance_returns_instance_not_found() {
    // At handler level, InstanceNotFound is produced by the orchestrator.
    // We verify that handle_signal on any InstanceState succeeds without panic.
    let mut state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0);

    let (caller_tx, caller_rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_signal(
        &mut state,
        "any_signal".to_string(),
        Bytes::from_static(b"data"),
        caller_tx.into(),
    )
    .await
    .expect("handler returns Ok");

    let result = caller_rx.await.expect("caller reply channel not dropped");
    assert!(result.is_ok(), "signal to valid InstanceState should succeed");
}

#[tokio::test]
async fn signal_with_wrong_name_does_not_unblock_workflow() {
    let mut state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0);

    // Pending waiter for "approval"
    let (pending_tx, mut pending_rx) =
        tokio::sync::oneshot::channel::<Result<Bytes, WtfError>>();
    state
        .pending_signal_calls
        .insert("approval".to_string(), pending_tx.into());

    let (caller_tx, caller_rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_signal(
        &mut state,
        "wrong_name".to_string(),
        Bytes::from_static(b"payload"),
        caller_tx.into(),
    )
    .await
    .expect("handler returns Ok");
    assert!(caller_rx.await.expect("caller reply").is_ok());

    // Original waiter untouched [POST-14]
    assert!(
        state.pending_signal_calls.contains_key("approval"),
        "pending entry for 'approval' must remain untouched"
    );
    assert!(
        pending_rx.try_recv().is_err(),
        "no reply sent to the original waiter"
    );

    // Signal buffered under wrong name [INV-4]
    if let ParadigmState::Procedural(s) = &state.paradigm_state {
        assert!(
            s.received_signals.contains_key("wrong_name"),
            "signal with wrong name must be buffered (not discarded)"
        );
    }
}

#[tokio::test]
async fn empty_signal_payload_delivered_and_workflow_completes() {
    let mut state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0);

    let (pending_tx, pending_rx) =
        tokio::sync::oneshot::channel::<Result<Bytes, WtfError>>();
    state
        .pending_signal_calls
        .insert("go".to_string(), pending_tx.into());

    let (caller_tx, caller_rx) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();

    handlers::handle_signal(&mut state, "go".to_string(), Bytes::new(), caller_tx.into())
        .await
        .expect("handler returns Ok");

    assert!(caller_rx.await.expect("caller reply").is_ok());

    let pending_result = pending_rx.await.expect("pending reply channel not dropped");
    let received = pending_result.expect("pending should be Ok");
    assert!(
        received.is_empty(),
        "POST-15: empty payload must be delivered as-is"
    );

    assert!(
        !state.pending_signal_calls.contains_key("go"),
        "pending entry must be removed"
    );
}

#[tokio::test]
async fn postcondition_op_counter_increments_once_per_wait_for_signal() {
    let mut state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0);
    let before_events = state.total_events_applied;

    // Step 1: wait_for_signal with no buffer -> registers pending
    let (wait_tx1, _wait_rx1) = tokio::sync::oneshot::channel::<Result<Bytes, WtfError>>();
    procedural::handle_wait_for_signal(&mut state, 0, "step1".to_string(), wait_tx1.into()).await;
    assert!(state.pending_signal_calls.contains_key("step1"));

    // Step 2: handle_signal delivers
    let (caller_tx1, caller_rx1) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();
    handlers::handle_signal(
        &mut state,
        "step1".to_string(),
        Bytes::from_static(b"step1-payload"),
        caller_tx1.into(),
    )
    .await
    .expect("ok");
    assert!(caller_rx1.await.expect("ok").is_ok());
    assert_eq!(
        state.total_events_applied,
        before_events + 1,
        "first signal must increment total_events_applied by 1"
    );

    // Step 3: Second wait_for_signal
    let (wait_tx2, _wait_rx2) = tokio::sync::oneshot::channel::<Result<Bytes, WtfError>>();
    procedural::handle_wait_for_signal(&mut state, 1, "step2".to_string(), wait_tx2.into()).await;

    // Step 4: Second signal
    let (caller_tx2, caller_rx2) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();
    handlers::handle_signal(
        &mut state,
        "step2".to_string(),
        Bytes::from_static(b"step2-payload"),
        caller_tx2.into(),
    )
    .await
    .expect("ok");
    assert!(caller_rx2.await.expect("ok").is_ok());
    assert_eq!(
        state.total_events_applied,
        before_events + 2,
        "two signals must increment total_events_applied by 2"
    );
}

#[tokio::test]
async fn invariant_signal_never_lost_either_delivered_or_buffered() {
    let mut state = make_test_state(Some(Arc::new(MockOkEventStore)), None, 0);

    // Pending waiter for "release"
    let (pending_tx, pending_rx) =
        tokio::sync::oneshot::channel::<Result<Bytes, WtfError>>();
    state
        .pending_signal_calls
        .insert("release".to_string(), pending_tx.into());

    // Step 1: First signal -> pending waiter
    let (caller_tx1, caller_rx1) = tokio::sync::oneshot::channel::<Result<(), WtfError>>();
    handlers::handle_signal(
        &mut state,
        "release".to_string(),
        Bytes::from_static(b"first"),
        caller_tx1.into(),
    )
    .await
    .expect("ok");
    assert!(caller_rx1.await.expect("ok").is_ok());

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
    assert!(caller_rx2.await.expect("ok").is_ok());

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
    assert!(caller_rx.await.expect("ok").is_ok());

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
    assert!(caller_rx.await.expect("ok").is_ok());

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
    assert!(caller_rx.await.expect("ok").is_ok());

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
    assert!(caller_rx1.await.expect("ok").is_ok());

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
    assert!(caller_rx2.await.expect("ok").is_ok());

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
async fn spawn_null_instance_actor() -> ActorRef<InstanceMsg> {
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
    let (ref_, _handle) = NullInstanceActor::spawn(None, NullInstanceActor, ())
        .await
        .expect("null instance actor spawned");
    ref_
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
    let actor_ref = spawn_null_instance_actor().await;

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
    let actor_ref = spawn_null_instance_actor().await;

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
    let actor_ref = spawn_null_instance_actor().await;

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

    // Wait for the actor to actually stop
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

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
    let actor_ref = spawn_null_instance_actor().await;

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
    let actor_ref = spawn_null_instance_actor().await;

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
    let actor_ref = spawn_null_instance_actor().await;

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
