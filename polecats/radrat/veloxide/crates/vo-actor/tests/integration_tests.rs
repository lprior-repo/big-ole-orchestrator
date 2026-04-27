//! Integration tests for actor message types.
//!
//! These tests verify that the message types work correctly with the ractor
//! actor framework. Since ractor::Message is auto-implemented for all types
//! that are Send + 'static, we test the Message trait bounds here.

use vo_actor::actor_messages::{ControlActorMessage, InstanceActorMessage};
use vo_actor::test_utilities::TestStateLookup;
use vo_types::{InstanceId, NodeName, SequenceNumber, TimerId, WorkflowName};

/// Integration test: InstanceActorMessage implements Message trait and can be sent.
/// This verifies that the message type satisfies ractor's Message trait bounds.
#[test]
fn instance_actor_message_satisfies_ractor_message_bounds() {
    fn assert_message<T: ractor::Message>() {}
    assert_message::<InstanceActorMessage>();
}

/// Integration test: ControlActorMessage implements Message trait and can be sent.
#[test]
fn control_actor_message_satisfies_ractor_message_bounds() {
    fn assert_message<T: ractor::Message>() {}
    assert_message::<ControlActorMessage>();
}

/// Integration test: InstanceActorMessage variants all satisfy Message trait.
#[test]
fn all_instance_actor_message_variants_satisfy_message_trait() {
    // StartWorkflow
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let workflow_name = WorkflowName::parse("deploy-prod").unwrap();
    let node_name = NodeName::parse("build-step").unwrap();
    let _msg1 = InstanceActorMessage::new_start_workflow(instance_id, workflow_name, node_name);
    fn assert<T: ractor::Message>() {}
    assert::<InstanceActorMessage>();

    // StepCompleted
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let node_name = NodeName::parse("compile-step").unwrap();
    let sequence = SequenceNumber::new_unchecked(1);
    let _msg2 = InstanceActorMessage::new_step_completed(instance_id, node_name, sequence);

    // StepFailed
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let node_name = NodeName::parse("compile-step").unwrap();
    let sequence = SequenceNumber::new_unchecked(42);
    let error = "connection timeout".to_string();
    let _msg3 = InstanceActorMessage::new_step_failed(instance_id, node_name, sequence, error);

    // TimerFired
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let timer_id = TimerId::parse("timer-abc-123").unwrap();
    let _msg4 = InstanceActorMessage::new_timer_fired(instance_id, timer_id);

    // CancelRequested
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let _msg5 = InstanceActorMessage::new_cancel_requested(instance_id);

    // GetStatus
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let _msg6 = InstanceActorMessage::new_get_status(instance_id);

    // All variants are the same type, so if the type implements Message, all variants do
    assert!(std::mem::size_of::<InstanceActorMessage>() > 0);
}

/// Integration test: ControlActorMessage variants all satisfy Message trait.
#[test]
fn all_control_actor_message_variants_satisfy_message_trait() {
    // Cancel
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let _msg1 = ControlActorMessage::new_cancel(instance_id);

    // Resume
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let _msg2 = ControlActorMessage::new_resume(instance_id);

    // All variants are the same type, so if the type implements Message, all variants do
    assert!(std::mem::size_of::<ControlActorMessage>() > 0);
}

/// Integration test: Messages are Send + Sync (required for actor channels).
#[test]
fn instance_actor_message_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<InstanceActorMessage>();
}

/// Integration test: Control messages are Send + Sync.
#[test]
fn control_actor_message_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ControlActorMessage>();
}

/// Integration test: InstanceActorMessage has correct memory size.
/// The enum should be large enough to hold all variant data.
#[test]
fn instance_actor_message_has_valid_memory_size() {
    let size = std::mem::size_of::<InstanceActorMessage>();
    // Should be at least as large as the largest variant
    assert!(size >= std::mem::size_of::<InstanceId>()); // At least the InstanceId size
    assert!(size >= std::mem::size_of::<SequenceNumber>());
    assert!(size >= std::mem::size_of::<TimerId>());
    assert!(size >= 1); // At least 1 byte for the discriminant
}

/// Integration test: ControlActorMessage has valid memory size.
#[test]
fn control_actor_message_has_valid_memory_size() {
    let size = std::mem::size_of::<ControlActorMessage>();
    // Should be at least as large as InstanceId
    assert!(size >= std::mem::size_of::<InstanceId>());
    assert!(size >= 1); // At least 1 byte for the discriminant
}

/// Integration test: Message clone preserves type information.
#[test]
fn instance_actor_message_clone_preserves_type() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let workflow_name = WorkflowName::parse("deploy-prod").unwrap();
    let node_name = NodeName::parse("build-step").unwrap();
    let msg = InstanceActorMessage::new_start_workflow(instance_id, workflow_name, node_name);
    let clone = msg.clone();
    // Clone should be the same variant
    match msg {
        InstanceActorMessage::StartWorkflow { .. } => {
            assert!(matches!(clone, InstanceActorMessage::StartWorkflow { .. }));
        }
        _ => panic!("Expected StartWorkflow variant"),
    }
}

/// Integration test: Message clone preserves type for ControlActorMessage.
#[test]
fn control_actor_message_clone_preserves_type() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let msg = ControlActorMessage::new_cancel(instance_id);
    let clone = msg.clone();
    match msg {
        ControlActorMessage::Cancel { .. } => {
            assert!(matches!(clone, ControlActorMessage::Cancel { .. }));
        }
        _ => panic!("Expected Cancel variant"),
    }
}

// =============================================================================
// Atomic Accept-Resume Integration Tests
// =============================================================================

use std::sync::Arc;
use vo_actor::mock_signal_storage::{MockSignalStorage, MockSignalWorkQueue};
use vo_actor::{AcceptResumeError, ControlActor, SignalPayload, WaitKey};

/// Integration test: accept_and_resume succeeds with valid storage and work queue.
#[test]
fn accept_and_resume_with_storage_and_queue_succeeds() {
    // 'W' at position 22 encodes WaitingForSignal state
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
    let storage = Arc::new(MockSignalStorage::new());
    let work_queue = Arc::new(MockSignalWorkQueue::new());
    let actor = ControlActor::with_storage_and_queue(
        storage.clone(),
        work_queue.clone(),
        Arc::new(TestStateLookup::new()),
    );

    let wait_key = WaitKey::parse("approval").unwrap();
    let payload = SignalPayload::empty();

    let result = actor.accept_and_resume(
        instance_id.clone(),
        wait_key,
        "sig-123".to_string(),
        payload,
    );

    assert!(result.is_ok(), "Expected accept_and_resume to succeed");
    let outcome = result.unwrap();
    assert_eq!(outcome.accepted.instance_id, instance_id);
    assert_eq!(outcome.resumed.instance_id, instance_id);

    // Verify signal was persisted
    let persisted = storage.persisted_signals();
    assert_eq!(persisted.len(), 1, "Expected exactly one persisted signal");
    assert_eq!(persisted[0].signal_id, "sig-123");

    // Verify wake-up was enqueued
    let enqueued = work_queue.enqueued_instances();
    assert_eq!(enqueued.len(), 1, "Expected exactly one enqueued wake-up");
    assert_eq!(enqueued[0], instance_id);
}

/// Integration test: accept_and_resume rolls back when enqueue fails.
#[test]
fn accept_and_resume_rolls_back_when_enqueue_fails() {
    // 'W' at position 22 encodes WaitingForSignal state
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
    let storage = Arc::new(MockSignalStorage::new());
    let work_queue = Arc::new(MockSignalWorkQueue::new());
    work_queue.set_should_fail(true);

    let actor = ControlActor::with_storage_and_queue(
        storage.clone(),
        work_queue.clone(),
        Arc::new(TestStateLookup::new()),
    );

    let wait_key = WaitKey::parse("approval").unwrap();
    let payload = SignalPayload::empty();

    let result = actor.accept_and_resume(
        instance_id.clone(),
        wait_key,
        "sig-456".to_string(),
        payload,
    );

    // Enqueue failure should result in an error
    assert!(result.is_err(), "Expected accept_and_resume to fail");

    // Signal should NOT be persisted (rollback)
    let persisted = storage.persisted_signals();
    assert!(
        persisted.is_empty(),
        "Expected no persisted signals after rollback"
    );
}

/// Integration test: accept_and_resume returns error when storage write fails.
#[test]
fn accept_and_resume_returns_error_when_storage_fails() {
    // 'W' at position 22 encodes WaitingForSignal state
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
    let storage = Arc::new(MockSignalStorage::new());
    storage.set_should_fail(true);
    let work_queue = Arc::new(MockSignalWorkQueue::new());

    let actor = ControlActor::with_storage_and_queue(
        storage.clone(),
        work_queue.clone(),
        Arc::new(TestStateLookup::new()),
    );

    let wait_key = WaitKey::parse("approval").unwrap();
    let payload = SignalPayload::empty();

    let result = actor.accept_and_resume(
        instance_id.clone(),
        wait_key,
        "sig-789".to_string(),
        payload,
    );

    assert!(result.is_err(), "Expected accept_and_resume to fail");
    let err = result.unwrap_err();

    // Should be a storage error
    assert!(
        matches!(err, AcceptResumeError::StorageError { .. }),
        "Expected StorageError, got {:?}",
        err
    );

    // No wake-up should be enqueued since storage failed first
    let enqueued = work_queue.enqueued_instances();
    assert!(
        enqueued.is_empty(),
        "Expected no enqueued wake-ups when storage fails"
    );
}

/// Integration test: accept_and_resume returns error when instance not found.
#[test]
fn accept_and_resume_returns_instance_not_found_when_actor_missing() {
    let storage = Arc::new(MockSignalStorage::new());
    let work_queue = Arc::new(MockSignalWorkQueue::new());
    let actor = ControlActor::with_storage_and_queue(
        storage.clone(),
        work_queue.clone(),
        Arc::new(TestStateLookup::new()),
    );

    // Use an instance_id that triggers non-existent actor pattern
    let instance_id = InstanceId::parse("00000000000000000000000001").unwrap();
    let wait_key = WaitKey::parse("approval").unwrap();
    let payload = SignalPayload::empty();

    let result =
        actor.accept_and_resume(instance_id.clone(), wait_key, "sig-1".to_string(), payload);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, AcceptResumeError::InstanceActorNotFound { .. }),
        "Expected InstanceActorNotFound, got {:?}",
        err
    );

    // Nothing should be persisted or enqueued
    assert!(storage.persisted_signals().is_empty());
    assert!(work_queue.enqueued_instances().is_empty());
}

/// Integration test: ControlActor Debug format includes storage status.
#[test]
fn control_actor_debug_shows_storage_status() {
    let actor_without_storage = ControlActor::new();
    let debug_str = format!("{:?}", actor_without_storage);
    assert!(
        debug_str.contains("signal_storage"),
        "Debug should show signal_storage field"
    );
    assert!(
        debug_str.contains("work_queue"),
        "Debug should show work_queue field"
    );
}
