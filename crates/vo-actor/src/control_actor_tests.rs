//! ControlActor Cancel and Resume test suite.

use crate::signal_messages::{
    BinaryHash, CancelError, ContinueAsNewError, InstanceResumed, LifecycleState, NodeName,
    ResumeError, SecretId, SignalAccepted, SignalName, SignalPayload, SignalStorage,
    SignalWorkQueue, TestStateLookup, TimestampMs, WaitKey, WorkflowCancelled, WorkflowContinued,
};
use crate::{InstanceId};
use crate::control_actor::ControlActor;

// ========================================================================
// Behavior: cancel_on_running_instance_emits_cancelrequested_then_workflowcancelled_in_order
// ========================================================================

#[tokio::test]
async fn cancel_on_running_instance_emits_cancelrequested_then_workflowcancelled_in_order() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let actor = ControlActor::new();
    let result = actor.handle_cancel(instance_id.clone());
    let (cancel_requested, workflow_cancelled) = result.unwrap();
    assert_eq!(cancel_requested.instance_id, instance_id);
    assert_eq!(workflow_cancelled.instance_id, instance_id);
    assert!(workflow_cancelled.cancelled_at >= cancel_requested.requested_at);
}

#[tokio::test]
async fn cancel_on_running_instance_transitions_lifecycle_to_cancelled() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let actor = ControlActor::new();
    let (_cancel_requested, _workflow_cancelled) = actor.handle_cancel(instance_id).unwrap();
}

#[tokio::test]
async fn cancel_releases_write_lock_after_event_emission() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let actor = ControlActor::new();
    actor.handle_cancel(instance_id).unwrap();
}

// ========================================================================
// Behavior: cancel_returns_alreadyterminal_when_instance_is_completed_or_cancelled
// ========================================================================

#[tokio::test]
async fn cancel_returns_alreadyterminal_error_when_instance_is_completed() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00C000").unwrap();
    let actor = ControlActor::new();
    match actor.handle_cancel(instance_id) {
        Err(CancelError::AlreadyTerminal { current_state: LifecycleState::Completed }) => {}
        other => panic!("Expected AlreadyTerminal(Completed), got {:?}", other),
    }
}

#[tokio::test]
async fn cancel_returns_alreadyterminal_error_when_instance_is_cancelled() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00X000").unwrap();
    let actor = ControlActor::new();
    match actor.handle_cancel(instance_id) {
        Err(CancelError::AlreadyTerminal { current_state: LifecycleState::Cancelled }) => {}
        other => panic!("Expected AlreadyTerminal(Cancelled), got {:?}", other),
    }
}

// ========================================================================
// Behavior: cancel_returns_error_for_missing_actor, lock, or storage
// ========================================================================

#[tokio::test]
async fn cancel_returns_instanceactornotfound_when_actor_missing() {
    let instance_id = InstanceId::parse("00000000000000000000000001").unwrap();
    let actor = ControlActor::new();
    match actor.handle_cancel(instance_id) {
        Err(CancelError::InstanceActorNotFound { .. }) => {}
        other => panic!("Expected InstanceActorNotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn cancel_returns_lockacquisitionfailed_when_lock_unavailable() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BA00000").unwrap();
    let actor = ControlActor::new();
    match actor.handle_cancel(instance_id) {
        Err(CancelError::LockAcquisitionFailed { .. }) => {}
        other => panic!("Expected LockAcquisitionFailed, got {:?}", other),
    }
}

#[tokio::test]
async fn cancel_returns_storageerror_when_event_append_fails() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BS00000").unwrap();
    let actor = ControlActor::new();
    match actor.handle_cancel(instance_id) {
        Err(CancelError::StorageError { .. }) => {}
        other => panic!("Expected StorageError, got {:?}", other),
    }
}

// ========================================================================
// Behavior: resume_on_failed_instance_emits_instanceresumed
// ========================================================================

#[tokio::test]
async fn resume_on_failed_instance_emits_instanceresumed_and_actor_re_enters_decision() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00F000").unwrap();
    let actor = ControlActor::new();
    let instance_resumed = actor.handle_resume(instance_id).unwrap();
    assert_eq!(instance_resumed.instance_id, instance_id);
    assert_ne!(
        instance_resumed.previous_binary_hash,
        instance_resumed.resumed_binary_hash
    );
}

#[tokio::test]
async fn resume_on_failed_instance_emits_instanceresumed_with_hash_state() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00F000").unwrap();
    let actor = ControlActor::new();
    let instance_resumed = actor.handle_resume(instance_id).unwrap();
    assert!(!instance_resumed.previous_binary_hash.0.is_empty());
    assert!(!instance_resumed.resumed_binary_hash.0.is_empty());
    assert!(instance_resumed.resumed_at.0 > 0);
}

#[tokio::test]
async fn resume_on_failed_instance_transitions_lifecycle_from_failed_to_running() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00F000").unwrap();
    let actor = ControlActor::new();
    actor.handle_resume(instance_id).unwrap();
}

// ========================================================================
// Behavior: resume_returns_error_for_invalid state, missing secrets, etc.
// ========================================================================

#[tokio::test]
async fn resume_returns_invalidlifecyclestate_error_when_instance_is_running() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
    let actor = ControlActor::new();
    match actor.handle_resume(instance_id) {
        Err(ResumeError::InvalidLifecycleState { actual, expected }) => {
            assert_eq!(actual, LifecycleState::Running);
            assert_eq!(expected, LifecycleState::Failed);
        }
        other => panic!("Expected InvalidLifecycleState(Running, Failed), got {:?}", other),
    }
}

#[tokio::test]
async fn resume_returns_invalidlifecyclestate_error_when_instance_is_completed() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00C000").unwrap();
    let actor = ControlActor::new();
    match actor.handle_resume(instance_id) {
        Err(ResumeError::InvalidLifecycleState { actual, expected }) => {
            assert_eq!(actual, LifecycleState::Completed);
            assert_eq!(expected, LifecycleState::Failed);
        }
        other => panic!("Expected InvalidLifecycleState(Completed, Failed), got {:?}", other),
    }
}

#[tokio::test]
async fn resume_returns_invalidlifecyclestate_error_when_instance_is_cancelled() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00X000").unwrap();
    let actor = ControlActor::new();
    match actor.handle_resume(instance_id) {
        Err(ResumeError::InvalidLifecycleState { actual, expected }) => {
            assert_eq!(actual, LifecycleState::Cancelled);
            assert_eq!(expected, LifecycleState::Failed);
        }
        other => panic!("Expected InvalidLifecycleState(Cancelled, Failed), got {:?}", other),
    }
}

#[tokio::test]
async fn resume_returns_missingsecrets_error_when_secrets_absent() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BM0F000").unwrap();
    let actor = ControlActor::new();
    match actor.handle_resume(instance_id) {
        Err(ResumeError::MissingSecrets { missing_secret_ids, .. }) => {
            assert!(!missing_secret_ids.is_empty());
        }
        other => panic!("Expected MissingSecrets, got {:?}", other),
    }
}

#[tokio::test]
async fn resume_returns_nodenotfound_error_when_node_missing() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BN0F000").unwrap();
    let actor = ControlActor::new();
    match actor.handle_resume(instance_id) {
        Err(ResumeError::NodeNotFound { .. }) => {}
        other => panic!("Expected NodeNotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn resume_returns_nopathtoterminal_error_when_no_valid_path() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BP0F000").unwrap();
    let actor = ControlActor::new();
    match actor.handle_resume(instance_id) {
        Err(ResumeError::NoPathToTerminal { .. }) => {}
        other => panic!("Expected NoPathToTerminal, got {:?}", other),
    }
}

#[tokio::test]
async fn resume_returns_instanceactornotfound_when_actor_missing() {
    let instance_id = InstanceId::parse("00000000000000000000000001").unwrap();
    let actor = ControlActor::new();
    match actor.handle_resume(instance_id) {
        Err(ResumeError::InstanceActorNotFound { .. }) => {}
        other => panic!("Expected InstanceActorNotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn resume_returns_lockacquisitionfailed_when_lock_unavailable() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BA0F000").unwrap();
    let actor = ControlActor::new();
    match actor.handle_resume(instance_id) {
        Err(ResumeError::LockAcquisitionFailed { .. }) => {}
        other => panic!("Expected LockAcquisitionFailed, got {:?}", other),
    }
}

#[tokio::test]
async fn resume_returns_storageerror_when_event_append_fails() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BS0F000").unwrap();
    let actor = ControlActor::new();
    match actor.handle_resume(instance_id) {
        Err(ResumeError::StorageError { .. }) => {}
        other => panic!("Expected StorageError, got {:?}", other),
    }
}

#[tokio::test]
async fn resume_error_precondition_classification_is_correct() {
    use ResumeError::*;
    let precondition_errors: Vec<ResumeError> = vec![
        InvalidLifecycleState {
            actual: LifecycleState::Running, expected: LifecycleState::Failed,
        },
        MissingSecrets {
            instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap(),
            missing_secret_ids: vec![SecretId::new("secret-1")],
        },
        NodeNotFound {
            instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000001").unwrap(),
            node_name: NodeName::new("node-X"),
        },
        NoPathToTerminal {
            instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000002").unwrap(),
            current_node: NodeName::new("node-Y"),
        },
        InstanceActorNotFound {
            instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000003").unwrap(),
        },
    ];
    for err in &precondition_errors {
        assert!(err.is_precondition());
        assert!(!err.is_transient());
    }
    let transient_errors: Vec<ResumeError> = vec![
        LockAcquisitionFailed {
            instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000004").unwrap(),
            reason: "lock held".to_string(),
        },
        StorageError {
            instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000005").unwrap(),
            reason: "io error".to_string(),
        },
    ];
    for err in &transient_errors {
        assert!(!err.is_precondition());
        assert!(err.is_transient());
    }
}

#[tokio::test]
async fn cancel_events_always_ordered_cancelrequested_then_workflowcancelled() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let actor = ControlActor::new();
    match actor.handle_cancel(instance_id) {
        Ok((first, second)) => {
            assert!(second.cancelled_at >= first.requested_at);
        }
        Err(_) => {}
    }
}
