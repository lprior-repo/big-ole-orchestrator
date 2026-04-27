//! AcceptAndResume test suite for ControlActor.

use crate::control_actor_ops::ControlActor;
use crate::signal_messages::{AcceptResumeError, LifecycleState, SignalPayload, WaitKey};
use crate::InstanceId;

// ── Group A: WaitKey validation ──

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

// ── Group B: SignalPayload validation ──

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

// ── Group C: LifecycleState::WaitingForSignal ──

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

// ── Group D: AcceptResumeError classification ──

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
        assert!(err.is_precondition());
        assert!(!err.is_transient());
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
        assert!(!err.is_precondition());
        assert!(err.is_transient());
    }
}
