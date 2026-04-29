use super::super::{
    AcceptResumeError, ControlActor, LifecycleState, SignalPayload, WaitKey,
};
use vo_types::InstanceId;

#[test]
fn waitkey_parse_succeeds_for_valid_input() {
    let key = WaitKey::parse("approval-v2").unwrap();
    assert_eq!(key.as_str(), "approval-v2");
}

#[test]
fn waitkey_parse_rejects_empty_string() {
    let result = WaitKey::parse("");
    assert_eq!(result, Err("WaitKey cannot be empty".to_string()));
}

#[test]
fn waitkey_parse_rejects_over_256_chars() {
    let long_key = "a".repeat(257);
    let result = WaitKey::parse(&long_key);
    assert_eq!(
        result,
        Err(format!(
            "WaitKey exceeds 256 characters: {}",
            long_key.len()
        ))
    );
}

#[test]
fn waitkey_new_unchecked_bypasses_validation() {
    let key = WaitKey::new_unchecked("");
    assert_eq!(key.as_str(), "");
}

#[test]
fn signal_payload_from_bytes_succeeds_for_valid_payload() {
    let payload = SignalPayload::from_bytes(vec![1, 2, 3]).unwrap();
    assert_eq!(payload.as_bytes(), &[1, 2, 3]);
}

#[test]
fn signal_payload_from_bytes_rejects_over_64kib() {
    let big = vec![0u8; 65537];
    let result = SignalPayload::from_bytes(big);
    assert_eq!(
        result,
        Err("SignalPayload exceeds 64 KiB: 65537 bytes".to_string())
    );
}

#[test]
fn signal_payload_empty_creates_zero_length_payload() {
    let payload = SignalPayload::empty();
    assert!(payload.is_empty());
    assert_eq!(payload.len(), 0);
}

#[test]
fn signal_payload_len_and_is_empty_are_correct() {
    let payload = SignalPayload::from_bytes(vec![42]).unwrap();
    assert!(!payload.is_empty());
    assert_eq!(payload.len(), 1);
}

#[test]
fn waiting_for_signal_is_not_terminal() {
    assert!(!LifecycleState::WaitingForSignal.is_terminal());
}

#[test]
fn lifecycle_state_all_variants_is_terminal_correctness() {
    assert!(!LifecycleState::Running.is_terminal());
    assert!(!LifecycleState::Failed.is_terminal());
    assert!(LifecycleState::Completed.is_terminal());
    assert!(LifecycleState::Cancelled.is_terminal());
    assert!(!LifecycleState::WaitingForSignal.is_terminal());
}

#[test]
fn accept_resume_error_precondition_variants_are_correct() {
    let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
    let precondition_errors: Vec<AcceptResumeError> = vec![
        AcceptResumeError::InvalidLifecycleState {
            instance_id: iid.clone(),
            actual: LifecycleState::Running,
            expected: LifecycleState::WaitingForSignal,
        },
        AcceptResumeError::WaitKeyMismatch {
            instance_id: iid.clone(),
            expected_key: WaitKey::new_unchecked("a"),
            provided_key: WaitKey::new_unchecked("b"),
        },
        AcceptResumeError::InstanceActorNotFound {
            instance_id: iid.clone(),
        },
        AcceptResumeError::PayloadTooLarge {
            instance_id: iid,
            payload_size: 65537,
            max_size: 65536,
        },
    ];
    for err in &precondition_errors {
        assert!(
            err.is_precondition(),
            "Expected {:?} to be precondition",
            err
        );
        assert!(
            !err.is_transient(),
            "Expected {:?} to NOT be transient",
            err
        );
    }
}

#[test]
fn accept_resume_error_transient_variants_are_correct() {
    let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
    let transient_errors: Vec<AcceptResumeError> = vec![
        AcceptResumeError::LockAcquisitionFailed {
            instance_id: iid.clone(),
            reason: "lock held".to_string(),
        },
        AcceptResumeError::StorageError {
            instance_id: iid,
            reason: "io error".to_string(),
        },
    ];
    for err in &transient_errors {
        assert!(
            !err.is_precondition(),
            "Expected {:?} to NOT be precondition",
            err
        );
        assert!(err.is_transient(), "Expected {:?} to be transient", err);
    }
}

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

#[tokio::test]
async fn accept_and_resume_returns_instance_not_found() {
    let instance_id = InstanceId::parse("00000000000000000000000001").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();

    let result = actor.accept_and_resume(
        instance_id.clone(),
        wait_key,
        "sig-1".to_string(),
        SignalPayload::empty(),
    );

    match result {
        Err(AcceptResumeError::InstanceActorNotFound { instance_id: _ }) => {}
        other => panic!("Expected InstanceActorNotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn accept_and_resume_returns_invalid_lifecycle_when_running() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B000000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();

    let result = actor.accept_and_resume(
        instance_id.clone(),
        wait_key,
        "sig-1".to_string(),
        SignalPayload::empty(),
    );

    match result {
        Err(AcceptResumeError::InvalidLifecycleState {
            instance_id: _,
            actual,
            expected,
        }) => {
            assert_eq!(actual, LifecycleState::Running);
            assert_eq!(expected, LifecycleState::WaitingForSignal);
        }
        other => panic!("Expected InvalidLifecycleState(Running), got {:?}", other),
    }
}

#[tokio::test]
async fn accept_and_resume_returns_wait_key_mismatch() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("wrong-key").unwrap();

    let result = actor.accept_and_resume(
        instance_id.clone(),
        wait_key,
        "mismatch-sig-1".to_string(),
        SignalPayload::empty(),
    );

    match result {
        Err(AcceptResumeError::WaitKeyMismatch {
            instance_id: _,
            expected_key,
            provided_key,
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

    let result = actor.accept_and_resume(
        instance_id.clone(),
        wait_key,
        "sig-1".to_string(),
        big_payload,
    );

    match result {
        Err(AcceptResumeError::PayloadTooLarge {
            instance_id: _,
            payload_size,
            max_size,
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

    let result = actor.accept_and_resume(
        instance_id.clone(),
        wait_key,
        "sig-1".to_string(),
        SignalPayload::empty(),
    );

    match result {
        Err(AcceptResumeError::LockAcquisitionFailed {
            instance_id: _,
            reason: _,
        }) => {}
        other => panic!("Expected LockAcquisitionFailed, got {:?}", other),
    }
}

#[tokio::test]
async fn accept_and_resume_returns_storage_error() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BS0W000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();

    let result = actor.accept_and_resume(
        instance_id.clone(),
        wait_key,
        "sig-1".to_string(),
        SignalPayload::empty(),
    );

    match result {
        Err(AcceptResumeError::StorageError {
            instance_id: _,
            reason: _,
        }) => {}
        other => panic!("Expected StorageError, got {:?}", other),
    }
}

#[tokio::test]
async fn test_workflow_correctly_transitions_from_waiting_to_ready_when_signaled() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();
    let payload = SignalPayload::empty();

    let result =
        actor.accept_and_resume(instance_id.clone(), wait_key, "sig-1".to_string(), payload);

    let outcome = result.expect("accept_and_resume should succeed when workflow is waiting");
    assert_eq!(
        outcome.accepted.instance_id, instance_id,
        "accepted.instance_id should match"
    );
    assert_eq!(
        outcome.resumed.instance_id, instance_id,
        "resumed.instance_id should match"
    );
    assert!(
        outcome.resumed.resumed_at >= outcome.accepted.accepted_at,
        "resumed_at should be >= accepted_at for atomic transition"
    );
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

    let result =
        actor.accept_and_resume(instance_id.clone(), wait_key, "sig-1".to_string(), payload);

    assert!(
        result.is_err(),
        "accept_and_resume should fail when workflow is in terminal state"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, AcceptResumeError::InvalidLifecycleState { .. }),
        "Expected InvalidLifecycleState error, got {:?}",
        err
    );
}

#[tokio::test]
async fn test_transition_fails_gracefully_if_workflow_is_in_a_terminal_state_duplicate_for_sch() {
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00X000").unwrap();
    let actor = ControlActor::new();
    let wait_key = WaitKey::parse("approval-v2").unwrap();
    let payload = SignalPayload::empty();

    let result =
        actor.accept_and_resume(instance_id.clone(), wait_key, "sig-2".to_string(), payload);

    assert!(
        result.is_err(),
        "accept_and_resume should fail when workflow is in terminal state"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, AcceptResumeError::InvalidLifecycleState { .. }),
        "Expected InvalidLifecycleState error for Cancelled state, got {:?}",
        err
    );
}
