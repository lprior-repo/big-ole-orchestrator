//! Red Queen adversarial tests for `publish_instance_started` and `should_skip_instance_started`.
//!
//! These tests attempt to:
//! 1. Violate contracts
//! 2. Edge cases
//! 3. Failure modes
//! 4. Race conditions
//! 5. Boundary conditions

use crate::instance::init::{publish_instance_started, should_skip_instance_started};
use crate::messages::{InstanceArguments, WorkflowParadigm};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use wtf_common::storage::{EventStore, ReplayBatch, ReplayStream, ReplayedEvent};
use wtf_common::{NamespaceId, WorkflowEvent, WtfError};

#[derive(Debug)]
struct FailingEventStore {
    fail_publish: bool,
    return_seq: u64,
}

#[async_trait]
impl EventStore for FailingEventStore {
    async fn publish(
        &self,
        _ns: &NamespaceId,
        _inst: &wtf_common::InstanceId,
        _event: WorkflowEvent,
    ) -> Result<u64, WtfError> {
        if self.fail_publish {
            return Err(WtfError::nats_publish("forced failure"));
        }
        Ok(self.return_seq)
    }

    async fn open_replay_stream(
        &self,
        _ns: &NamespaceId,
        _inst: &wtf_common::InstanceId,
        _from_seq: u64,
    ) -> Result<Box<dyn ReplayStream>, WtfError> {
        Ok(Box::new(EmptyStream))
    }
}

#[derive(Debug)]
struct EmptyStream;

#[async_trait]
impl ReplayStream for EmptyStream {
    async fn next_event(&mut self) -> Result<ReplayBatch, WtfError> {
        Ok(ReplayBatch::TailReached)
    }
    async fn next_live_event(&mut self) -> Result<ReplayedEvent, WtfError> {
        std::future::pending().await
    }
}

fn make_args(store: Arc<dyn EventStore>) -> InstanceArguments {
    InstanceArguments {
        namespace: NamespaceId::new("test-ns"),
        instance_id: wtf_common::InstanceId::new("test-instance"),
        workflow_type: "test_workflow".into(),
        paradigm: WorkflowParadigm::Fsm,
        input: bytes::Bytes::from_static(b"{}"),
        engine_node_id: "test-node".into(),
        event_store: Some(store),
        state_store: None,
        task_queue: None,
        snapshot_db: None,
        procedural_workflow: None,
        workflow_definition: None,
    }
}

// ============================================================================
// ADVERSARIAL TEST 1: Contract Violation - from_seq = 0
// ============================================================================
// Contract says P-3: "from_seq == 1" means fresh, but implementation allows 0
// The guard is `from_seq > 1` which means 0 passes through and publishes

#[tokio::test]
async fn adversarial_from_seq_zero_publishes_instance_started() {
    let store = Arc::new(FailingEventStore {
        fail_publish: false,
        return_seq: 1,
    });
    let args = make_args(store);

    // from_seq = 0 is NOT valid (should be 1 for fresh), but guard allows it through
    let result: std::result::Result<(), ractor::ActorProcessingErr> =
        publish_instance_started(&args, 0, &[]).await;

    assert!(
        result.is_ok(),
        "BUG: from_seq=0 should be invalid but got {:?}",
        result.err()
    );
    // This is a contract violation - from_seq should be 1, not 0
}

// ============================================================================
// ADVERSARIAL TEST 2: Boundary - from_seq = u64::MAX (potential overflow)
// ============================================================================

#[test]
fn adversarial_from_seq_max_overflow_in_load_initial_state() {
    // load_initial_state computes: from_seq = snap.seq + 1
    // If snap.seq = u64::MAX - 1, then from_seq = u64::MAX (overflow!)
    // The guard correctly skips u64::MAX since u64::MAX > 1
    let should_skip = should_skip_instance_started(u64::MAX, &[]);
    assert!(should_skip, "Guard correctly skips u64::MAX (overflow)");

    // FIXED: In load_initial_state, snap.seq + 1 uses checked_add:
    // from_seq = snap.seq.checked_add(1).ok_or_else(...)?;
    // If snap.seq is u64::MAX, an error is returned instead of wrapping to 0!

    // Also FIXED: from_seq = 0 is now treated as invalid and skips
    // This prevents the overflow case from publishing incorrectly
    let zero_skips = should_skip_instance_started(0, &[]);
    assert!(
        zero_skips,
        "FIXED: from_seq=0 now skips to prevent overflow issues"
    );
}

// ============================================================================
// ADVERSARIAL TEST 3: Edge Case - event_log contains only InstanceStarted
// ============================================================================
// Contract I-2: "InstanceStarted is published at most once per instance_id"
// But if event_log already contains InstanceStarted, the guard only checks is_empty()

#[tokio::test]
async fn adversarial_event_log_has_instance_started_still_publishes() {
    let store = Arc::new(FailingEventStore {
        fail_publish: false,
        return_seq: 1,
    });
    let args = make_args(store);

    // event_log already has InstanceStarted (crash recovery scenario)
    let event_log = vec![WorkflowEvent::InstanceStarted {
        instance_id: "test-instance".into(),
        workflow_type: "test_workflow".into(),
        input: bytes::Bytes::from_static(b"{}"),
    }];

    // Current implementation: is_empty() check only - will skip because non-empty
    // But semantically, if InstanceStarted is in log, we should skip
    let result: std::result::Result<(), ractor::ActorProcessingErr> =
        publish_instance_started(&args, 1, &event_log).await;
    assert!(result.is_ok(), "Expected Ok");

    // NOTE: Current impl skips correctly (non-empty log), but the LOGIC is wrong:
    // It should check if InstanceStarted EXISTS in log, not just if log is non-empty
}

// ============================================================================
// ADVERSARIAL TEST 4: EventStore publish failure
// ============================================================================

#[tokio::test]
async fn adversarial_publish_returns_error() {
    let store = Arc::new(FailingEventStore {
        fail_publish: true,
        return_seq: 1,
    });
    let args = make_args(store);

    let result: std::result::Result<(), ractor::ActorProcessingErr> =
        publish_instance_started(&args, 1, &[]).await;

    assert!(result.is_err(), "Expected error when EventStore fails");
    let err_msg = format!("{:?}", result.err());
    assert!(
        err_msg.contains("forced failure") || err_msg.contains("nats_publish"),
        "Error should contain failure reason, got: {err_msg}"
    );
}

// ============================================================================
// ADVERSARIAL TEST 5: EventStore returns wrong sequence number
// ============================================================================
// Contract S-6: "Sequence assigned to InstanceStarted is 1"
// Contract I-1: "InstanceStarted is always sequence 1"
// But implementation doesn't verify the returned sequence!

#[tokio::test]
async fn adversarial_publish_returns_seq_5_not_detected() {
    let store = Arc::new(FailingEventStore {
        fail_publish: false,
        return_seq: 5, // WRONG! Should be 1
    });
    let args = make_args(store);

    let result: std::result::Result<(), ractor::ActorProcessingErr> =
        publish_instance_started(&args, 1, &[]).await;

    // Current implementation accepts ANY sequence number returned
    // It should verify that the sequence is 1
    assert!(
        result.is_ok(),
        "BUG: Implementation doesn't verify returned sequence is 1"
    );
    // VIOLATION: S-6 and I-1 are not enforced
}

// ============================================================================
// ADVERSARIAL TEST 6: Boundary - from_seq = 1 exactly
// ============================================================================

#[test]
fn adversarial_from_seq_one_is_fresh() {
    // This SHOULD be fresh (publish)
    let should_skip = should_skip_instance_started(1, &[]);
    assert!(!should_skip, "from_seq=1 should NOT skip (fresh instance)");
}

// ============================================================================
// ADVERSARIAL TEST 7: Boundary - from_seq = 2 (snapshot recovery)
// ============================================================================

#[test]
fn adversarial_from_seq_two_skips() {
    // This should skip (snapshot recovery)
    let should_skip = should_skip_instance_started(2, &[]);
    assert!(should_skip, "from_seq=2 should skip (snapshot recovery)");
}

// ============================================================================
// ADVERSARIAL TEST 8: Empty event_log vs event_log with different events
// ============================================================================

#[test]
fn adversarial_event_log_with_other_events_doesnt_skip() {
    // Non-empty log, but contains OTHER events (not InstanceStarted)
    let other_events = vec![
        WorkflowEvent::SnapshotTaken {
            seq: 1,
            checksum: 0,
        },
        WorkflowEvent::TransitionApplied {
            from_state: "init".into(),
            event_name: "start".into(),
            to_state: "running".into(),
            effects: vec![],
        },
    ];

    let should_skip = should_skip_instance_started(1, &other_events);
    // Current implementation: non-empty = skip
    // But if these events are from REPLAY (not fresh), we might incorrectly skip
    assert!(
        should_skip,
        "Non-empty event_log causes skip (guard behavior)"
    );
}

// ============================================================================
// ADVERSARIAL TEST 9: Race Condition - spawn_live_subscription not checked
// ============================================================================
// Contract P-1: "spawn_live_subscription has completed successfully"
// But publish_instance_started has NO check for this precondition

#[tokio::test]
async fn adversarial_publish_without_live_subscription() {
    let store = Arc::new(FailingEventStore {
        fail_publish: false,
        return_seq: 1,
    });
    let args = make_args(store);

    // Calling publish WITHOUT calling spawn_live_subscription first
    // This VIOLATES P-1 but implementation allows it
    let result: std::result::Result<(), ractor::ActorProcessingErr> =
        publish_instance_started(&args, 1, &[]).await;

    assert!(
        result.is_ok(),
        "BUG: No check that spawn_live_subscription completed"
    );
    // VIOLATION: P-1 is not enforced
}

// ============================================================================
// ADVERSARIAL TEST 10: Empty instance_id or workflow_type
// ============================================================================

#[tokio::test]
async fn adversarial_empty_instance_id() {
    let store = Arc::new(FailingEventStore {
        fail_publish: false,
        return_seq: 1,
    });
    let mut args = make_args(store);
    args.instance_id = wtf_common::InstanceId::new("");

    let result: std::result::Result<(), ractor::ActorProcessingErr> =
        publish_instance_started(&args, 1, &[]).await;

    // Empty instance_id - should this be allowed?
    assert!(result.is_ok(), "Empty instance_id allowed?");
}

#[tokio::test]
async fn adversarial_empty_workflow_type() {
    let store = Arc::new(FailingEventStore {
        fail_publish: false,
        return_seq: 1,
    });
    let mut args = make_args(store);
    args.workflow_type = String::new();

    let result: std::result::Result<(), ractor::ActorProcessingErr> =
        publish_instance_started(&args, 1, &[]).await;

    // Empty workflow_type - should this be allowed?
    assert!(result.is_ok(), "Empty workflow_type allowed?");
}

// ============================================================================
// ADVERSARIAL TEST 11: Double publish scenario
// ============================================================================
// I-2: "InstanceStarted is published at most once per instance_id"
// Implementation has no guard against calling this twice with from_seq=1 and empty log

#[tokio::test]
async fn adversarial_double_publish_both_succeed() {
    let call_count = Arc::new(Mutex::new(0u32));

    #[derive(Debug)]
    struct CountingEventStore {
        counter: Arc<Mutex<u32>>,
    }

    impl CountingEventStore {
        fn new(counter: Arc<Mutex<u32>>) -> Self {
            Self { counter }
        }
    }

    #[async_trait]
    impl EventStore for CountingEventStore {
        async fn publish(
            &self,
            _ns: &NamespaceId,
            _inst: &wtf_common::InstanceId,
            _event: WorkflowEvent,
        ) -> Result<u64, WtfError> {
            let mut count = self.counter.lock().unwrap();
            *count += 1;
            Ok(u64::from(*count))
        }

        async fn open_replay_stream(
            &self,
            _ns: &NamespaceId,
            _inst: &wtf_common::InstanceId,
            _from_seq: u64,
        ) -> Result<Box<dyn ReplayStream>, WtfError> {
            Ok(Box::new(EmptyStream))
        }
    }

    let store = Arc::new(CountingEventStore::new(Arc::clone(&call_count)));
    let args = make_args(store);

    // First call
    let result1: std::result::Result<(), ractor::ActorProcessingErr> =
        publish_instance_started(&args, 1, &[]).await;
    assert!(result1.is_ok(), "First publish should succeed");

    // Second call with same params - should this be prevented?
    let result2: std::result::Result<(), ractor::ActorProcessingErr> =
        publish_instance_started(&args, 1, &[]).await;
    assert!(
        result2.is_ok(),
        "BUG: Double publish succeeds - I-2 violated"
    );

    let count = *call_count.lock().unwrap();
    assert_eq!(count, 2, "Both publishes executed - no idempotency guard");
}

// ============================================================================
// ADVERSARIAL TEST 12: No event store - exact contract violation
// ============================================================================

#[tokio::test]
async fn adversarial_no_event_store_returns_specific_error() {
    let mut args = make_args(Arc::new(FailingEventStore {
        fail_publish: false,
        return_seq: 1,
    }));
    args.event_store = None;

    let result: std::result::Result<(), ractor::ActorProcessingErr> =
        publish_instance_started(&args, 1, &[]).await;

    assert!(result.is_err(), "No event_store should return error");
    let err_msg = format!("{:?}", result.err());
    // Contract says: Error::EventStoreUnavailable
    // Implementation returns: "No event store available for InstanceStarted publish"
    assert!(err_msg.contains("No event store"), "Error message mismatch");
}