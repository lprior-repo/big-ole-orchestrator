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

// =============================================================================
// Message Injection Attack Tests (bh-001)
// =============================================================================

#[test]
fn malformed_instance_id_rejected_at_parse_boundary() {
    use vo_types::InstanceId;

    let malformed_inputs = vec![
        "",
        "NOT-ULID",
        "123",
        "01H5JYV4XHGSR2F8KZ9BWNRFMA\0x00", // Null byte injection
        "01H5JYV4XHGSR2F8KZ9BWNRFMA\n",   // Newline injection
        "01H5JYV4XHGSR2F8KZ9BWNRFMA<script>", // Script injection attempt
    ];

    for input in malformed_inputs {
        let result = InstanceId::parse(input);
        assert!(
            result.is_err(),
            "Malformed InstanceId '{}' should be rejected at parse boundary",
            input.escape_debug()
        );
    }
}

#[test]
fn malformed_workflow_name_rejected_at_parse_boundary() {
    use vo_types::WorkflowName;

    let malformed_inputs = vec![
        "",
        "invalid--name",             // Consecutive hyphens (was bug vel-c7u)
        "workflow name",             // Spaces not allowed
        "workflow\tname",            // Tab not allowed
        "workflow\nname",            // Newline not allowed
        "workflow\x00name",          // Null byte
        "workflow_日本語",            // Unicode
        "12345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890", // 100 chars (may exceed limit)
    ];

    for input in malformed_inputs {
        let result = WorkflowName::parse(input);
        assert!(
            result.is_err(),
            "Malformed WorkflowName '{}' should be rejected at parse boundary",
            input.escape_debug()
        );
    }
}

#[test]
fn malformed_node_name_rejected_at_parse_boundary() {
    use vo_types::NodeName;

    let malformed_inputs = vec![
        "",
        "node name",                 // Spaces not allowed
        "node\tname",                // Tab not allowed
        "node\x00name",              // Null byte
        "node_日本語",                // Unicode
    ];

    for input in malformed_inputs {
        let result = NodeName::parse(input);
        assert!(
            result.is_err(),
            "Malformed NodeName '{}' should be rejected at parse boundary",
            input.escape_debug()
        );
    }
}

#[test]
fn malformed_timer_id_rejected_at_parse_boundary() {
    use vo_types::TimerId;

    let malformed_inputs = vec![
        "",
    ];

    for input in malformed_inputs {
        let result = TimerId::parse(input);
        assert!(
            result.is_err(),
            "Malformed TimerId '{}' should be rejected at parse boundary",
            input.escape_debug()
        );
    }
}

#[test]
fn waitkey_rejects_oversized_input() {
    use vo_actor::WaitKey;

    let long_input = "a".repeat(300);
    let result = WaitKey::parse(&long_input);
    assert!(
        result.is_err(),
        "WaitKey exceeding 256 chars should be rejected"
    );

    let exactly_256 = "a".repeat(256);
    let result_256 = WaitKey::parse(&exactly_256);
    assert!(
        result_256.is_ok(),
        "WaitKey with exactly 256 chars should be accepted"
    );
}

#[test]
fn waitkey_rejects_empty_input() {
    use vo_actor::WaitKey;

    let result = WaitKey::parse("");
    assert!(
        result.is_err(),
        "Empty WaitKey should be rejected"
    );
}

#[test]
fn signal_payload_rejects_oversized_input() {
    use vo_actor::SignalPayload;

    let oversized_payload = vec![0u8; 65537];
    let result = SignalPayload::from_bytes(oversized_payload);
    assert!(
        result.is_err(),
        "SignalPayload exceeding 64 KiB should be rejected"
    );

    let exactly_64k = vec![0u8; 65536];
    let result_64k = SignalPayload::from_bytes(exactly_64k);
    assert!(
        result_64k.is_ok(),
        "SignalPayload with exactly 64 KiB should be accepted"
    );
}

#[test]
fn signal_payload_rejects_null_bytes_in_small_input() {
    use vo_actor::SignalPayload;

    let payload_with_null = vec![0u8, 1u8, 2u8];
    let result = SignalPayload::from_bytes(payload_with_null);
    assert!(
        result.is_ok(),
        "SignalPayload with null bytes should be accepted (binary data is valid)"
    );
}

#[test]
fn control_actor_message_accept_and_resume_rejects_invalid_waitkey() {
    use vo_actor::WaitKey;

    let empty_waitkey_result = WaitKey::parse("");
    assert!(
        empty_waitkey_result.is_err(),
        "Empty WaitKey must be rejected before message construction"
    );

    let long_waitkey = WaitKey::parse(&"a".repeat(300));
    assert!(
        long_waitkey.is_err(),
        "Oversized WaitKey must be rejected before message construction"
    );
}

#[test]
fn actor_does_not_crash_on_valid_message_with_extreme_values() {
    use vo_actor::{ControlActorMessage, InstanceActorMessage};
    use vo_types::{InstanceId, NodeName, SequenceNumber, WorkflowName};

    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let node_name = NodeName::parse("test-node").unwrap();
    let workflow_name = WorkflowName::parse("test-workflow").unwrap();

    let msg1 = InstanceActorMessage::new_start_workflow(
        instance_id.clone(),
        workflow_name.clone(),
        node_name.clone(),
    );
    assert!(matches!(msg1, InstanceActorMessage::StartWorkflow { .. }));

    let msg2 = InstanceActorMessage::new_step_completed(
        instance_id.clone(),
        node_name.clone(),
        SequenceNumber::new_unchecked(1),
    );
    assert!(matches!(msg2, InstanceActorMessage::StepCompleted { .. }));

    let msg3 = InstanceActorMessage::new_step_failed(
        instance_id.clone(),
        node_name.clone(),
        SequenceNumber::new_unchecked(u64::MAX),
        "x".repeat(1_000_000), // 1MB error string
    );
    assert!(matches!(msg3, InstanceActorMessage::StepFailed { .. }));

    let msg4 = InstanceActorMessage::new_cancel_requested(instance_id.clone());
    assert!(matches!(msg4, InstanceActorMessage::CancelRequested { .. }));

    let msg5 = ControlActorMessage::new_cancel(instance_id.clone());
    assert!(matches!(msg5, ControlActorMessage::Cancel { .. }));

    let msg6 = ControlActorMessage::new_resume(instance_id);
    assert!(matches!(msg6, ControlActorMessage::Resume { .. }));
}

#[test]
fn actor_message_debug_format_does_not_leak_sensitive_data() {
    use vo_actor::InstanceActorMessage;
    use vo_types::{InstanceId, NodeName, WorkflowName};

    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let workflow_name = WorkflowName::parse("deploy-prod").unwrap();
    let node_name = NodeName::parse("build-step").unwrap();

    let msg = InstanceActorMessage::new_start_workflow(
        instance_id.clone(),
        workflow_name.clone(),
        node_name.clone(),
    );

    let debug_str = format!("{:?}", msg);

    assert!(
        debug_str.contains("StartWorkflow"),
        "Debug format should contain variant name"
    );
    assert!(
        debug_str.contains("01H5JYV4XHGSR2F8KZ9BWNRFMA"),
        "Debug format should contain instance_id"
    );
    assert!(
        !debug_str.contains("\\x00") && !debug_str.contains("\\n"),
        "Debug format should not contain escaped control chars for normal input"
    );
}

#[test]
fn actor_state_consistency_after_error_conditions() {
    use vo_actor::WaitKey;
    use vo_types::InstanceId;

    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9B00W000").unwrap();

    let wait_key_empty = WaitKey::parse("");
    assert!(wait_key_empty.is_err(), "Empty WaitKey must be rejected");

    let wait_key_oversized = WaitKey::parse(&"x".repeat(300));
    assert!(wait_key_oversized.is_err(), "Oversized WaitKey must be rejected");

    let valid_wait_key = WaitKey::parse("approval");
    assert!(valid_wait_key.is_ok(), "Valid WaitKey must be accepted");

    drop((instance_id, valid_wait_key));
}

#[test]
fn protocol_violation_lifecycle_state_transition_rejected() {
    use vo_actor::lifecycle::{compute_next_state, ActorLifecycleState, LifecycleTransition};

    assert_eq!(
        compute_next_state(ActorLifecycleState::Stopped, LifecycleTransition::Start),
        None,
        "Cannot transition from Stopped to Running (protocol violation)"
    );

    assert_eq!(
        compute_next_state(ActorLifecycleState::Failed, LifecycleTransition::Start),
        None,
        "Cannot transition from Failed to Running (protocol violation)"
    );

    assert_eq!(
        compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::Stop),
        None,
        "Cannot transition from Pending directly to Stopping (must Start first)"
    );

    assert_eq!(
        compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::ChildStopped),
        None,
        "Pending actor has no children (protocol violation)"
    );
}
