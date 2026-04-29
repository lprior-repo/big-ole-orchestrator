//! AcceptAndResume operation tests for ControlActor.

use crate::control_actor_ops::ControlActor;
use crate::signal_messages::{AcceptResumeError, SignalPayload, WaitKey};
use crate::InstanceId;

// ── Group E: accept_and_resume success path ──

#[tokio::test]
async fn accept_and_resume_succeeds_when_waiting_for_signal() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();
    let payload = SignalPayload::empty();
    let result =
        actor.accept_and_resume(instance_id.clone(), wait_key, "sig-1".to_string(), payload);
    let outcome = result.unwrap();
    assert_eq!(outcome.accepted.instance_id, instance_id);
    assert_eq!(outcome.resumed.instance_id, instance_id);
}

#[tokio::test]
async fn accept_and_resume_outcome_has_correct_instance_id() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();
    let result = actor.accept_and_resume(
        instance_id.clone(),
        wait_key,
        "sig-2".to_string(),
        SignalPayload::empty(),
    );
    let outcome = result.unwrap();
    assert_eq!(outcome.accepted.instance_id, instance_id);
    assert_eq!(outcome.resumed.instance_id, instance_id);
}

#[tokio::test]
async fn accept_and_resume_outcome_timestamps_are_ordered() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();
    let result = actor.accept_and_resume(
        instance_id,
        wait_key,
        "sig-3".to_string(),
        SignalPayload::empty(),
    );
    let outcome = result.unwrap();
    assert!(outcome.resumed.resumed_at >= outcome.accepted.accepted_at);
}

// ── Group F: accept_and_resume error paths ──

#[tokio::test]
async fn accept_and_resume_returns_instance_not_found() {
    let instance_id = InstanceId::parse("00000000000000000000000001").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();
    match actor.accept_and_resume(
        instance_id,
        wait_key,
        "sig-1".to_string(),
        SignalPayload::empty(),
    ) {
        Err(AcceptResumeError::InstanceActorNotFound { .. }) => {}
        other => panic!("Expected InstanceActorNotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn accept_and_resume_returns_invalid_lifecycle_when_running() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();
    match actor.accept_and_resume(
        instance_id,
        wait_key,
        "sig-1".to_string(),
        SignalPayload::empty(),
    ) {
        Err(AcceptResumeError::InvalidLifecycleState { actual, expected, .. }) => {
            assert_eq!(actual, crate::signal_messages::LifecycleState::Running);
            assert_eq!(
                expected,
                crate::signal_messages::LifecycleState::WaitingForSignal
            );
        }
        other => panic!("Expected InvalidLifecycleState(Running), got {:?}", other),
    }
}

#[tokio::test]
async fn accept_and_resume_returns_wait_key_mismatch() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("wrong-key").unwrap();
    match actor.accept_and_resume(
        instance_id,
        wait_key,
        "mismatch-sig-1".to_string(),
        SignalPayload::empty(),
    ) {
        Err(AcceptResumeError::WaitKeyMismatch {
            expected_key,
            provided_key,
            ..
        }) => {
            assert_eq!(expected_key.as_str(), "expected-key");
            assert_eq!(provided_key.as_str(), "wrong-key");
        }
        other => panic!("Expected WaitKeyMismatch, got {:?}", other),
    }
}

#[tokio::test]
async fn accept_and_resume_returns_payload_too_large() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();
    let big_payload = SignalPayload::new_unchecked(vec![0u8; 65537]);
    match actor.accept_and_resume(instance_id, wait_key, "sig-1".to_string(), big_payload) {
        Err(AcceptResumeError::PayloadTooLarge {
            payload_size,
            max_size,
            ..
        }) => {
            assert_eq!(payload_size, 65537);
            assert_eq!(max_size, 65536);
        }
        other => panic!("Expected PayloadTooLarge, got {:?}", other),
    }
}

#[tokio::test]
async fn accept_and_resume_returns_lock_acquisition_failed() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BA0W000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();
    match actor.accept_and_resume(
        instance_id,
        wait_key,
        "sig-1".to_string(),
        SignalPayload::empty(),
    ) {
        Err(AcceptResumeError::LockAcquisitionFailed { .. }) => {}
        other => panic!("Expected LockAcquisitionFailed, got {:?}", other),
    }
}

#[tokio::test]
async fn accept_and_resume_returns_storage_error() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BS0W000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();
    match actor.accept_and_resume(
        instance_id,
        wait_key,
        "sig-1".to_string(),
        SignalPayload::empty(),
    ) {
        Err(AcceptResumeError::StorageError { .. }) => {}
        other => panic!("Expected StorageError, got {:?}", other),
    }
}

// ── Group G: Schema-required acceptance tests ──

#[tokio::test]
async fn test_workflow_correctly_transitions_from_waiting_to_ready_when_signaled() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();
    let payload = SignalPayload::empty();
    let result =
        actor.accept_and_resume(instance_id.clone(), wait_key, "sig-1".to_string(), payload);
    let outcome = result.expect("accept_and_resume should succeed when workflow is waiting");
    assert_eq!(outcome.accepted.instance_id, instance_id);
    assert_eq!(outcome.resumed.instance_id, instance_id);
    assert!(outcome.resumed.resumed_at >= outcome.accepted.accepted_at);
}

#[tokio::test]
async fn test_workflow_correctly_transitions_from_waiting_to_ready_when_signaled_duplicate_for() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("webhook").unwrap();
    let payload = SignalPayload::from_bytes(vec![1, 2, 3]).expect("valid payload");
    let result = actor.accept_and_resume(
        instance_id.clone(),
        wait_key,
        "sig-duplicate".to_string(),
        payload,
    );
    let outcome = result.expect("accept_and_resume should succeed");
    assert_eq!(outcome.accepted.instance_id, instance_id);
    assert_eq!(outcome.resumed.instance_id, instance_id);
}

#[tokio::test]
async fn test_transition_fails_gracefully_if_workflow_is_in_a_terminal_state() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00C000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();
    let payload = SignalPayload::empty();
    let result = actor.accept_and_resume(instance_id, wait_key, "sig-1".to_string(), payload);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        AcceptResumeError::InvalidLifecycleState { .. }
    ));
}

#[tokio::test]
async fn test_transition_fails_gracefully_if_workflow_is_in_a_terminal_state_duplicate_for_sch() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00X000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();
    let payload = SignalPayload::empty();
    let result = actor.accept_and_resume(instance_id, wait_key, "sig-2".to_string(), payload);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        AcceptResumeError::InvalidLifecycleState { .. }
    ));
}
