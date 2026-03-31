//! Integration tests for actor message types.
//!
//! These tests verify that the message types work correctly with the ractor
//! actor framework. Since ractor::Message is auto-implemented for all types
//! that are Send + 'static, we test the Message trait bounds here.

use vo_actor::actor_messages::{ControlActorMessage, InstanceActorMessage};
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
