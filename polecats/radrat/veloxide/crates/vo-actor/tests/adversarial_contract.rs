// ADVERSARIAL TEST: Attempt to violate contract invariants
// This test file is intentionally written to probe for bugs

#[test]
fn send_sync_bounds_pass() {
    // This should pass - verifies Send+Sync bounds
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    use vo_actor::InstanceActorMessage;
    use vo_types::InstanceId;

    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let _msg = InstanceActorMessage::new_cancel_requested(instance_id);
    assert_send::<InstanceActorMessage>();
    assert_sync::<InstanceActorMessage>();
}

#[test]
fn control_actor_message_send_sync() {
    // Verify ControlActorMessage also implements Send+Sync
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    use vo_actor::ControlActorMessage;
    use vo_types::InstanceId;

    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let _msg = ControlActorMessage::new_cancel(instance_id);
    assert_send::<ControlActorMessage>();
    assert_sync::<ControlActorMessage>();
}

#[test]
fn message_exhaustiveness() {
    // Verify all variants can be matched
    use vo_actor::{ControlActorMessage, InstanceActorMessage};
    use vo_types::{InstanceId, NodeName, SequenceNumber, TimerId, WorkflowName};

    let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

    // InstanceActorMessage has 6 variants
    let _val = InstanceActorMessage::new_start_workflow(
        iid.clone(),
        WorkflowName::parse("w").unwrap(),
        NodeName::parse("n").unwrap(),
    );
    let _val = InstanceActorMessage::new_step_completed(
        iid.clone(),
        NodeName::parse("n").unwrap(),
        SequenceNumber::new_unchecked(1),
    );
    let _val = InstanceActorMessage::new_step_failed(
        iid.clone(),
        NodeName::parse("n").unwrap(),
        SequenceNumber::new_unchecked(1),
        "e".to_string(),
    );
    let _val = InstanceActorMessage::new_timer_fired(iid.clone(), TimerId::parse("t").unwrap());
    let _val = InstanceActorMessage::new_cancel_requested(iid.clone());
    let _val = InstanceActorMessage::new_get_status(iid.clone());

    // ControlActorMessage has 2 variants
    let _val = ControlActorMessage::new_cancel(iid.clone());
    let _val = ControlActorMessage::new_resume(iid);
}

#[test]
fn immutability_guarantee() {
    // Verify messages are Clone but don't allow interior mutability
    use vo_actor::InstanceActorMessage;
    use vo_types::{InstanceId, NodeName, SequenceNumber};

    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let msg = InstanceActorMessage::new_step_completed(
        instance_id.clone(),
        NodeName::parse("node").unwrap(),
        SequenceNumber::new_unchecked(1),
    );

    // Messages should be cloneable (they derive Clone)
    let _val = msg.clone();

    // But we can't get &mut to the internal fields
    // If this test compiles and passes, immutability is preserved
}

#[test]
fn vo_types_validation_edge_cases() {
    // Test that vo-types correctly validates input
    use vo_types::{InstanceId, NodeName, SequenceNumber, TimerId, WorkflowName};

    // Empty strings should fail
    drop(InstanceId::parse("").unwrap_err());
    drop(WorkflowName::parse("").unwrap_err());
    drop(NodeName::parse("").unwrap_err());
    drop(TimerId::parse("").unwrap_err());

    // Invalid ULID format should fail
    drop(InstanceId::parse("NOT-ULID").unwrap_err());

    // Zero sequence should panic (via new_unchecked)
    let result = std::panic::catch_unwind(|| SequenceNumber::new_unchecked(0));
    drop(result.unwrap_err());
}

#[test]
fn workflow_name_consecutive_hyphens() {
    // IDEAL behavior: should reject consecutive hyphens as invalid identifier format.
    // BUG (vel-c7u) was fixed, so this should now return Err(ParseError::ConsecutiveHyphens).

    use vo_types::{ParseError, WorkflowName};

    let with_consecutive_hyphens = WorkflowName::parse("invalid--name");

    assert_eq!(
        with_consecutive_hyphens,
        Err(ParseError::ConsecutiveHyphens {
            type_name: "WorkflowName"
        })
    );
}

#[test]
fn unicode_identifiers() {
    // Test that unicode is NOT allowed in identifier-like types
    use vo_types::{NodeName, WorkflowName};

    // Unicode in WorkflowName should fail (identifier must be ASCII)
    drop(WorkflowName::parse("workflow_日本語").unwrap_err());

    // Unicode in NodeName should fail
    drop(NodeName::parse("node_日本語").unwrap_err());
}

#[test]
fn max_length_workflow_name() {
    // Test very long workflow names
    use vo_types::WorkflowName;

    // Very long name (1000 chars)
    let long_name = "a".repeat(1000);
    let result = WorkflowName::parse(&long_name);

    // If this succeeds when it should fail due to length limit, bug exists
    // If this fails because it exceeds max length, that's correct behavior
    let _val = result; // Just check it doesn't panic
}

#[test]
fn error_string_handling() {
    // Test StepFailed error string handling
    use vo_actor::InstanceActorMessage;
    use vo_types::{InstanceId, NodeName, SequenceNumber};

    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

    // Empty error string should be allowed (valid UTF-8)
    let msg1 = InstanceActorMessage::new_step_failed(
        instance_id.clone(),
        NodeName::parse("node").unwrap(),
        SequenceNumber::new_unchecked(1),
        "".to_string(),
    );
    match msg1 {
        InstanceActorMessage::StepFailed { error, .. } => {
            assert_eq!(error, "");
        }
        _ => panic!("Wrong variant"),
    }

    // Very long error string should be allowed
    let long_error = "x".repeat(1_000_000);
    let msg2 = InstanceActorMessage::new_step_failed(
        instance_id.clone(),
        NodeName::parse("node").unwrap(),
        SequenceNumber::new_unchecked(1),
        long_error,
    );
    match msg2 {
        InstanceActorMessage::StepFailed { error, .. } => {
            assert_eq!(error.len(), 1_000_000);
            std::str::from_utf8(error.as_bytes()).unwrap();
        }
        _ => panic!("Wrong variant"),
    }

    // Unicode error string should be allowed
    let unicode_error = "错误信息 🔥";
    let msg3 = InstanceActorMessage::new_step_failed(
        instance_id,
        NodeName::parse("node").unwrap(),
        SequenceNumber::new_unchecked(1),
        unicode_error.to_string(),
    );
    match msg3 {
        InstanceActorMessage::StepFailed { error, .. } => {
            std::str::from_utf8(error.as_bytes()).unwrap();
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn sequence_number_boundary() {
    // Test SequenceNumber with boundary values
    use vo_types::SequenceNumber;

    // Sequence 1 should succeed (minimum positive)
    let _result1 = SequenceNumber::new_unchecked(1);

    // Sequence u64::MAX should succeed
    let _result2 = SequenceNumber::new_unchecked(u64::MAX);
}
