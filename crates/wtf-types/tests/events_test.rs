use serde_json::json;
use wtf_types::{
    AttemptNumber, BinaryHash, EventEnvelope, EventError, EventPayload, EventVersion, FireAtMs,
    IdempotencyKey, InstanceId, NodeName, SequenceNumber, TimerId, TimestampMs, WorkflowName,
    MAX_SUPPORTED_VERSION,
};

fn ts(ms: u64) -> TimestampMs {
    TimestampMs::try_from(ms).expect("valid TimestampMs")
}

fn fa(ms: u64) -> FireAtMs {
    FireAtMs::try_from(ms).expect("valid FireAtMs")
}

fn valid_envelope_json(payload: serde_json::Value, version: u64) -> String {
    json!({
        "version": version,
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "sequence": 3,
        "timestamp_ms": 1710000000000u64,
        "payload": payload,
        "metadata": null,
    })
    .to_string()
}

fn valid_envelope_json_with_metadata(
    payload: serde_json::Value,
    version: u64,
    metadata: serde_json::Value,
) -> String {
    json!({
        "version": version,
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "sequence": 3,
        "timestamp_ms": 1710000000000u64,
        "payload": payload,
        "metadata": metadata,
    })
    .to_string()
}

fn make_envelope(version: u64, payload: serde_json::Value) -> EventEnvelope {
    EventEnvelope {
        version: EventVersion::new_unchecked(version),
        instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid"),
        sequence: SequenceNumber::new_unchecked(1),
        timestamp_ms: ts(1710000000000),
        payload,
        metadata: None,
    }
}

// ===========================================================================
// EventEnvelope::decode
// ===========================================================================

#[test]
fn event_envelope_decode_returns_valid_envelope_when_given_well_formed_json() {
    let json_str = valid_envelope_json(
        json!({"StepStarted": {
            "node_name": "compile-artifact",
            "attempt": 1,
            "idempotency_key": "key-20240101-abc",
            "binary_hash": "abcdef0123456789"
        }}),
        1,
    );
    let result = EventEnvelope::decode(json_str.as_bytes());
    assert_eq!(
        result,
        Ok(EventEnvelope {
            version: EventVersion::new_unchecked(1),
            instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid"),
            sequence: SequenceNumber::new_unchecked(3),
            timestamp_ms: ts(1710000000000),
            payload: json!({"StepStarted": {
                "node_name": "compile-artifact",
                "attempt": 1,
                "idempotency_key": "key-20240101-abc",
                "binary_hash": "abcdef0123456789"
            }}),
            metadata: None,
        })
    );
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_given_malformed_json() {
    let result = EventEnvelope::decode(b"{{{not json");
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { ref message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_version_missing() {
    let json_str = json!({
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "sequence": 3,
        "timestamp_ms": 1710000000000u64,
        "payload": {}
    })
    .to_string();
    let result = EventEnvelope::decode(json_str.as_bytes());
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_instance_id_missing() {
    let json_str = json!({
        "version": 1,
        "sequence": 3,
        "timestamp_ms": 1710000000000u64,
        "payload": {}
    })
    .to_string();
    let result = EventEnvelope::decode(json_str.as_bytes());
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_sequence_missing() {
    let json_str = json!({
        "version": 1,
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "timestamp_ms": 1710000000000u64,
        "payload": {}
    })
    .to_string();
    let result = EventEnvelope::decode(json_str.as_bytes());
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_timestamp_ms_missing() {
    let json_str = json!({
        "version": 1,
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "sequence": 3,
        "payload": {}
    })
    .to_string();
    let result = EventEnvelope::decode(json_str.as_bytes());
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_payload_missing() {
    let json_str = json!({
        "version": 1,
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "sequence": 3,
        "timestamp_ms": 1710000000000u64,
    })
    .to_string();
    let result = EventEnvelope::decode(json_str.as_bytes());
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_given_empty_bytes() {
    let result = EventEnvelope::decode(b"");
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { ref message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_given_json_array() {
    let result = EventEnvelope::decode(b"[1,2,3]");
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_version_is_zero() {
    let json_str = json!({
        "version": 0,
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "sequence": 3,
        "timestamp_ms": 1710000000000u64,
        "payload": {}
    })
    .to_string();
    let result = EventEnvelope::decode(json_str.as_bytes());
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_accepts_metadata_null_when_metadata_is_absent_or_null() {
    let json_str = json!({
        "version": 1,
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "sequence": 1,
        "timestamp_ms": 0,
        "payload": {},
        "metadata": null,
    })
    .to_string();
    let result = EventEnvelope::decode(json_str.as_bytes());
    match result {
        Ok(envelope) => assert_eq!(envelope.metadata, None),
        Err(e) => panic!("Expected Ok with metadata None, got: {e:?}"),
    }
}

#[test]
fn event_envelope_decode_accepts_metadata_as_json_object() {
    let json_str = valid_envelope_json_with_metadata(json!({}), 1, json!({"trace_id": "abc-123"}));
    let result = EventEnvelope::decode(json_str.as_bytes());
    match result {
        Ok(envelope) => assert_eq!(envelope.metadata, Some(json!({"trace_id": "abc-123"}))),
        Err(e) => panic!("Expected Ok with metadata, got: {e:?}"),
    }
}

#[test]
fn event_envelope_decode_accepts_future_version_when_version_exceeds_max_supported() {
    let json_str = valid_envelope_json(json!({}), 999);
    let result = EventEnvelope::decode(json_str.as_bytes());
    match result {
        Ok(envelope) => assert_eq!(envelope.version, EventVersion::new_unchecked(999)),
        Err(e) => panic!("Expected Ok with future version, got: {e:?}"),
    }
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_sequence_is_zero() {
    let json_str = json!({
        "version": 1,
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "sequence": 0,
        "timestamp_ms": 1710000000000u64,
        "payload": {}
    })
    .to_string();
    let result = EventEnvelope::decode(json_str.as_bytes());
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_sequence_is_wrong_type() {
    let json_str = json!({
        "version": 1,
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "sequence": "3",
        "timestamp_ms": 1710000000000u64,
        "payload": {}
    })
    .to_string();
    let result = EventEnvelope::decode(json_str.as_bytes());
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_given_json_string() {
    let result = EventEnvelope::decode(b"\"hello\"");
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_given_json_number() {
    let result = EventEnvelope::decode(b"42");
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_given_json_null() {
    let result = EventEnvelope::decode(b"null");
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_returns_deserialization_failed_when_instance_id_is_wrong_type() {
    let json_str = json!({
        "version": 1,
        "instance_id": 42,
        "sequence": 3,
        "timestamp_ms": 1710000000000u64,
        "payload": {}
    })
    .to_string();
    let result = EventEnvelope::decode(json_str.as_bytes());
    assert!(matches!(
        result,
        Err(EventError::DeserializationFailed { message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_ignores_extra_fields_when_json_contains_unknown_keys() {
    let json_str = json!({
        "version": 1,
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "sequence": 1,
        "timestamp_ms": 0,
        "payload": {},
        "bogus_field": true,
    })
    .to_string();
    let result = EventEnvelope::decode(json_str.as_bytes());
    match result {
        Ok(envelope) => assert_eq!(envelope.version, EventVersion::new_unchecked(1)),
        Err(e) => panic!("Expected Ok with extra fields ignored, got: {e:?}"),
    }
}

#[test]
fn event_envelope_decode_accepts_metadata_as_none_when_metadata_key_absent() {
    let json_str = json!({
        "version": 1,
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "sequence": 1,
        "timestamp_ms": 0,
        "payload": {},
    })
    .to_string();
    let result = EventEnvelope::decode(json_str.as_bytes());
    match result {
        Ok(envelope) => assert_eq!(envelope.metadata, None),
        Err(e) => panic!("Expected Ok with absent metadata, got: {e:?}"),
    }
}

// ===========================================================================
// EventEnvelope::decode_payload
// ===========================================================================

#[test]
fn event_envelope_decode_payload_returns_typed_payload_when_version_is_supported_and_payload_valid()
{
    let envelope = make_envelope(
        1,
        json!({
            "WorkflowStarted": {
                "workflow_name": "deploy-prod",
                "binary_hash": "abcdef0123456789"
            }
        }),
    );
    let result = envelope.decode_payload();
    assert_eq!(
        result,
        Ok(EventPayload::WorkflowStarted {
            workflow_name: WorkflowName::parse("deploy-prod").expect("valid"),
            binary_hash: BinaryHash::parse("abcdef0123456789").expect("valid"),
        })
    );
}

#[test]
fn event_envelope_decode_payload_returns_unsupported_version_when_version_exceeds_max_supported() {
    let envelope = make_envelope(999, json!({}));
    let result = envelope.decode_payload();
    assert_eq!(
        result,
        Err(EventError::UnsupportedVersion {
            actual: 999,
            max_supported: MAX_SUPPORTED_VERSION,
        })
    );
}

#[test]
fn event_envelope_decode_payload_returns_invalid_payload_when_variant_tag_unknown() {
    let envelope = make_envelope(1, json!({"BogusVariant": {}}));
    let result = envelope.decode_payload();
    assert!(matches!(
        result,
        Err(EventError::InvalidPayload { ref message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_payload_returns_invalid_payload_when_field_types_wrong() {
    let envelope = make_envelope(
        1,
        json!({
            "WorkflowStarted": {
                "workflow_name": 42,
                "binary_hash": "abcdef0123456789"
            }
        }),
    );
    let result = envelope.decode_payload();
    assert!(matches!(
        result,
        Err(EventError::InvalidPayload { ref message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_payload_returns_invalid_payload_when_variant_fields_missing() {
    let envelope = make_envelope(
        1,
        json!({
            "StepStarted": {
                "node_name": "build"
            }
        }),
    );
    let result = envelope.decode_payload();
    assert!(matches!(
        result,
        Err(EventError::InvalidPayload { ref message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_payload_decodes_workflow_started_when_payload_matches() {
    let envelope = make_envelope(
        1,
        json!({
            "WorkflowStarted": {"workflow_name": "deploy-prod", "binary_hash": "abcdef0123456789"}
        }),
    );
    assert!(matches!(
        envelope.decode_payload(),
        Ok(EventPayload::WorkflowStarted {
            workflow_name: _,
            binary_hash: _,
        })
    ));
}

#[test]
fn event_envelope_decode_payload_decodes_workflow_completed_when_payload_matches() {
    let envelope = make_envelope(
        1,
        json!({
            "WorkflowCompleted": {"result": null}
        }),
    );
    assert_eq!(
        envelope.decode_payload(),
        Ok(EventPayload::WorkflowCompleted { result: None })
    );
}

#[test]
fn event_envelope_decode_payload_decodes_workflow_failed_when_payload_matches() {
    let envelope = make_envelope(
        1,
        json!({
            "WorkflowFailed": {"error_message": "disk full"}
        }),
    );
    assert!(matches!(
        envelope.decode_payload(),
        Ok(EventPayload::WorkflowFailed {
            ref error_message,
        }) if error_message == "disk full"
    ));
}

#[test]
fn event_envelope_decode_payload_decodes_workflow_cancelled_when_payload_matches() {
    let envelope = make_envelope(
        1,
        json!({
            "WorkflowCancelled": {"reason": "user request"}
        }),
    );
    assert!(matches!(
        envelope.decode_payload(),
        Ok(EventPayload::WorkflowCancelled {
            ref reason,
        }) if reason == "user request"
    ));
}

#[test]
fn event_envelope_decode_payload_decodes_step_scheduled_when_payload_matches() {
    let envelope = make_envelope(
        1,
        json!({
            "StepScheduled": {"node_name": "build", "attempt": 1}
        }),
    );
    assert!(matches!(
        envelope.decode_payload(),
        Ok(EventPayload::StepScheduled {
            node_name: _,
            attempt: _,
        })
    ));
}

#[test]
fn event_envelope_decode_payload_decodes_step_started_when_payload_matches() {
    let envelope = make_envelope(
        1,
        json!({
            "StepStarted": {
                "node_name": "compile-artifact",
                "attempt": 1,
                "idempotency_key": "key-20240101-abc",
                "binary_hash": "abcdef0123456789"
            }
        }),
    );
    assert!(matches!(
        envelope.decode_payload(),
        Ok(EventPayload::StepStarted {
            node_name: _,
            attempt: _,
            idempotency_key: _,
            binary_hash: _,
        })
    ));
}

#[test]
fn event_envelope_decode_payload_decodes_step_completed_when_payload_matches() {
    let envelope = make_envelope(
        1,
        json!({
            "StepCompleted": {"node_name": "build", "attempt": 1, "result": null}
        }),
    );
    assert!(matches!(
        envelope.decode_payload(),
        Ok(EventPayload::StepCompleted { result: None, .. })
    ));
}

#[test]
fn event_envelope_decode_payload_decodes_step_failed_when_payload_matches() {
    let envelope = make_envelope(
        1,
        json!({
            "StepFailed": {"node_name": "build", "attempt": 1, "error_message": "OOM", "retryable": true}
        }),
    );
    assert!(matches!(
        envelope.decode_payload(),
        Ok(EventPayload::StepFailed {
            retryable: true,
            ..
        })
    ));
}

#[test]
fn event_envelope_decode_payload_decodes_timer_set_when_payload_matches() {
    let envelope = make_envelope(
        1,
        json!({
            "TimerSet": {"timer_id": "timer-123", "fire_at": 1710000000000u64}
        }),
    );
    assert!(matches!(
        envelope.decode_payload(),
        Ok(EventPayload::TimerSet {
            timer_id: _,
            fire_at: _,
        })
    ));
}

#[test]
fn event_envelope_decode_payload_decodes_timer_fired_when_payload_matches() {
    let envelope = make_envelope(
        1,
        json!({
            "TimerFired": {"timer_id": "timer-123"}
        }),
    );
    assert!(matches!(
        envelope.decode_payload(),
        Ok(EventPayload::TimerFired { timer_id: _ })
    ));
}

#[test]
fn event_envelope_decode_payload_decodes_cancel_requested_when_payload_matches() {
    let envelope = make_envelope(1, json!("CancelRequested"));
    assert_eq!(envelope.decode_payload(), Ok(EventPayload::CancelRequested));
}

#[test]
fn event_envelope_decode_payload_decodes_instance_resumed_when_payload_matches() {
    let envelope = make_envelope(
        1,
        json!({
            "InstanceResumed": {
                "previous_binary_hash": "aaaaaaaa00000000",
                "resumed_binary_hash": "bbbbbbbb00000000"
            }
        }),
    );
    assert!(matches!(
        envelope.decode_payload(),
        Ok(EventPayload::InstanceResumed {
            previous_binary_hash: _,
            resumed_binary_hash: _,
        })
    ));
}

#[test]
fn event_envelope_decode_payload_returns_unsupported_version_when_version_is_exactly_two() {
    let envelope = make_envelope(2, json!({}));
    let result = envelope.decode_payload();
    assert_eq!(
        result,
        Err(EventError::UnsupportedVersion {
            actual: 2,
            max_supported: MAX_SUPPORTED_VERSION,
        })
    );
}

#[test]
fn event_envelope_decode_payload_returns_invalid_payload_when_payload_is_null() {
    let envelope = make_envelope(1, json!(null));
    let result = envelope.decode_payload();
    assert!(matches!(
        result,
        Err(EventError::InvalidPayload { ref message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_payload_returns_invalid_payload_when_payload_is_empty_object() {
    let envelope = make_envelope(1, json!({}));
    let result = envelope.decode_payload();
    assert!(matches!(
        result,
        Err(EventError::InvalidPayload { ref message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_payload_returns_invalid_payload_when_newtype_field_wrong_type() {
    let envelope = make_envelope(
        1,
        json!({
            "TimerSet": {"timer_id": 42}
        }),
    );
    let result = envelope.decode_payload();
    assert!(matches!(
        result,
        Err(EventError::InvalidPayload { ref message }) if !message.is_empty()
    ));
}

#[test]
fn event_envelope_decode_payload_ignores_extra_fields_when_variant_json_contains_unknown_keys() {
    let envelope = make_envelope(
        1,
        json!({
            "TimerFired": {"timer_id": "timer-123", "extra_field": true}
        }),
    );
    assert!(matches!(
        envelope.decode_payload(),
        Ok(EventPayload::TimerFired { timer_id: _ })
    ));
}

#[test]
fn event_payload_workflow_completed_accepts_null_result_when_deserialized() {
    let envelope = make_envelope(
        1,
        json!({
            "WorkflowCompleted": {"result": null}
        }),
    );
    assert_eq!(
        envelope.decode_payload(),
        Ok(EventPayload::WorkflowCompleted { result: None })
    );
}

#[test]
fn event_payload_workflow_completed_defaults_result_to_none_when_field_absent() {
    let envelope = make_envelope(
        1,
        json!({
            "WorkflowCompleted": {}
        }),
    );
    assert_eq!(
        envelope.decode_payload(),
        Ok(EventPayload::WorkflowCompleted { result: None })
    );
}

#[test]
fn event_envelope_decode_payload_decodes_step_failed_with_retryable_false_when_payload_matches() {
    let envelope = make_envelope(
        1,
        json!({
            "StepFailed": {"node_name": "build", "attempt": 3, "error_message": "OOM", "retryable": false}
        }),
    );
    assert!(matches!(
        envelope.decode_payload(),
        Ok(EventPayload::StepFailed {
            retryable: false,
            ..
        })
    ));
}

// ===========================================================================
// EventEnvelope serde round-trip (Behavior 20)
// ===========================================================================

#[test]
fn event_envelope_serde_round_trips_without_loss_when_all_fields_populated() {
    let original = EventEnvelope {
        version: EventVersion::new_unchecked(1),
        instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid"),
        sequence: SequenceNumber::new_unchecked(3),
        timestamp_ms: ts(1710000000000),
        payload: json!({"StepStarted": {
            "node_name": "compile-artifact",
            "attempt": 1,
            "idempotency_key": "key-20240101-abc",
            "binary_hash": "abcdef0123456789"
        }}),
        metadata: Some(json!({"trace_id": "abc-123"})),
    };
    let bytes = serde_json::to_vec(&original).expect("serialize");
    let restored = EventEnvelope::decode(&bytes).expect("decode round-trip");
    assert_eq!(restored, original);
}

// ===========================================================================
// EventPayload serde round-trips (Behaviors 21-32)
// ===========================================================================

#[test]
fn event_payload_workflow_started_round_trips_through_serde() {
    let original = EventPayload::WorkflowStarted {
        workflow_name: WorkflowName::parse("deploy-prod").expect("valid"),
        binary_hash: BinaryHash::parse("abcdef0123456789").expect("valid"),
    };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_workflow_completed_round_trips_with_null_result_through_serde() {
    let original = EventPayload::WorkflowCompleted { result: None };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_workflow_completed_round_trips_with_some_result_through_serde() {
    let original = EventPayload::WorkflowCompleted {
        result: Some(json!({"output": "done"})),
    };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_workflow_failed_round_trips_through_serde() {
    let original = EventPayload::WorkflowFailed {
        error_message: "disk full".to_string(),
    };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_workflow_cancelled_round_trips_through_serde() {
    let original = EventPayload::WorkflowCancelled {
        reason: "user request".to_string(),
    };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_step_scheduled_round_trips_through_serde() {
    let original = EventPayload::StepScheduled {
        node_name: NodeName::parse("build").expect("valid"),
        attempt: AttemptNumber::new_unchecked(1),
    };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_step_started_round_trips_through_serde() {
    let original = EventPayload::StepStarted {
        node_name: NodeName::parse("compile-artifact").expect("valid"),
        attempt: AttemptNumber::new_unchecked(1),
        idempotency_key: IdempotencyKey::parse("key-20240101-abc").expect("valid"),
        binary_hash: BinaryHash::parse("abcdef0123456789").expect("valid"),
    };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_step_completed_round_trips_with_null_result_through_serde() {
    let original = EventPayload::StepCompleted {
        node_name: NodeName::parse("build").expect("valid"),
        attempt: AttemptNumber::new_unchecked(1),
        result: None,
    };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_step_completed_round_trips_with_some_result_through_serde() {
    let original = EventPayload::StepCompleted {
        node_name: NodeName::parse("build").expect("valid"),
        attempt: AttemptNumber::new_unchecked(1),
        result: Some(json!({"status": "success"})),
    };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_step_failed_round_trips_through_serde() {
    let original = EventPayload::StepFailed {
        node_name: NodeName::parse("build").expect("valid"),
        attempt: AttemptNumber::new_unchecked(2),
        error_message: "OOM killed".to_string(),
        retryable: true,
    };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_step_failed_round_trips_with_retryable_false_through_serde() {
    let original = EventPayload::StepFailed {
        node_name: NodeName::parse("build").expect("valid"),
        attempt: AttemptNumber::new_unchecked(3),
        error_message: "final failure".to_string(),
        retryable: false,
    };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_timer_set_round_trips_through_serde() {
    let original = EventPayload::TimerSet {
        timer_id: TimerId::parse("timer-123").expect("valid"),
        fire_at: fa(1710000000000),
    };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_timer_fired_round_trips_through_serde() {
    let original = EventPayload::TimerFired {
        timer_id: TimerId::parse("timer-123").expect("valid"),
    };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_cancel_requested_round_trips_through_serde() {
    let original = EventPayload::CancelRequested;
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn event_payload_instance_resumed_round_trips_through_serde() {
    let original = EventPayload::InstanceResumed {
        previous_binary_hash: BinaryHash::parse("aaaaaaaa00000000").expect("valid"),
        resumed_binary_hash: BinaryHash::parse("bbbbbbbb00000000").expect("valid"),
    };
    let json = serde_json::to_value(&original).expect("serialize");
    let restored: EventPayload = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

// ===========================================================================
// Proptests
// ===========================================================================

mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_event_envelope_decode_round_trip_identity(
            version in 1u64..=100u64,
            sequence in 1u64..=1000u64,
            timestamp_ms in 0u64..=1_000_000_000_000u64,
        ) {
            let envelope = EventEnvelope {
                version: EventVersion::new_unchecked(version),
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid"),
                sequence: SequenceNumber::new_unchecked(sequence),
                timestamp_ms: ts(timestamp_ms),
                payload: json!({"WorkflowCompleted": {"result": null}}),
                metadata: None,
            };
            let bytes = serde_json::to_vec(&envelope).expect("serialize");
            let restored = EventEnvelope::decode(&bytes).expect("decode round-trip");
            prop_assert_eq!(restored, envelope);
        }

        #[test]
        fn proptest_event_payload_serde_round_trip_all_variants(
            variant_idx in 0usize..12,
        ) {
            let payload = match variant_idx {
                0 => EventPayload::WorkflowStarted {
                    workflow_name: WorkflowName::parse("test-workflow").expect("valid"),
                    binary_hash: BinaryHash::parse("abcdef0123456789").expect("valid"),
                },
                1 => EventPayload::WorkflowCompleted { result: None },
                2 => EventPayload::WorkflowFailed {
                    error_message: "error".to_string(),
                },
                3 => EventPayload::WorkflowCancelled {
                    reason: "reason".to_string(),
                },
                4 => EventPayload::StepScheduled {
                    node_name: NodeName::parse("node-a").expect("valid"),
                    attempt: AttemptNumber::new_unchecked(1),
                },
                5 => EventPayload::StepStarted {
                    node_name: NodeName::parse("node-b").expect("valid"),
                    attempt: AttemptNumber::new_unchecked(1),
                    idempotency_key: IdempotencyKey::parse("key-1").expect("valid"),
                    binary_hash: BinaryHash::parse("abcdef0123456789").expect("valid"),
                },
                6 => EventPayload::StepCompleted {
                    node_name: NodeName::parse("node-c").expect("valid"),
                    attempt: AttemptNumber::new_unchecked(1),
                    result: Some(json!({"data": 42})),
                },
                7 => EventPayload::StepFailed {
                    node_name: NodeName::parse("node-d").expect("valid"),
                    attempt: AttemptNumber::new_unchecked(1),
                    error_message: "fail".to_string(),
                    retryable: true,
                },
                8 => EventPayload::TimerSet {
                    timer_id: TimerId::parse("t1").expect("valid"),
                    fire_at: fa(1000),
                },
                9 => EventPayload::TimerFired {
                    timer_id: TimerId::parse("t1").expect("valid"),
                },
                10 => EventPayload::CancelRequested,
                11 => EventPayload::InstanceResumed {
                    previous_binary_hash: BinaryHash::parse("aaaa0000").expect("valid"),
                    resumed_binary_hash: BinaryHash::parse("bbbb0000").expect("valid"),
                },
                _ => panic!("invalid variant index"),
            };
            let json_val = serde_json::to_value(&payload).expect("serialize");
            let restored: EventPayload = serde_json::from_value(json_val).expect("deserialize");
            prop_assert_eq!(restored, payload);
        }

        #[test]
        fn proptest_sequence_always_ge_one_after_decode(
            bytes in proptest::collection::vec(any::<u8>(), 0..=4096),
        ) {
            if let Ok(envelope) = EventEnvelope::decode(&bytes) {
                prop_assert!(envelope.sequence.as_u64() >= 1);
            }
        }

        #[test]
        fn proptest_version_always_ge_one_after_decode(
            bytes in proptest::collection::vec(any::<u8>(), 0..=4096),
        ) {
            if let Ok(envelope) = EventEnvelope::decode(&bytes) {
                prop_assert!(envelope.version.as_u64() >= 1);
            }
        }

        #[test]
        fn proptest_event_envelope_decode_never_panics_on_arbitrary_input(
            bytes in proptest::collection::vec(any::<u8>(), 0..=4096),
        ) {
            drop(EventEnvelope::decode(&bytes));
        }

        #[test]
        fn proptest_decode_payload_never_panics_when_version_exceeds_max(
            version in 2u64..=1000u64,
            payload_bytes in proptest::collection::vec(any::<u8>(), 0..=1024),
        ) {
            let payload: serde_json::Value = match serde_json::from_slice(&payload_bytes) {
                Ok(v) => v,
                Err(_) => json!(null),
            };
            let envelope = EventEnvelope {
                version: EventVersion::new_unchecked(version),
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid"),
                sequence: SequenceNumber::new_unchecked(1),
                timestamp_ms: ts(0),
                payload,
                metadata: None,
            };
            drop(envelope.decode_payload());
        }

        #[test]
        fn proptest_decode_payload_version_gate_is_exact(version in 1u64..=10u64) {
            let envelope = EventEnvelope {
                version: EventVersion::new_unchecked(version),
                instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid"),
                sequence: SequenceNumber::new_unchecked(1),
                timestamp_ms: ts(0),
                payload: json!({"WorkflowCompleted": {"result": null}}),
                metadata: None,
            };
            let result = envelope.decode_payload();
            if version > MAX_SUPPORTED_VERSION {
                let is_unsupported = matches!(result, Err(EventError::UnsupportedVersion { .. }));
                prop_assert!(is_unsupported);
            }
        }

        #[test]
        fn proptest_event_payload_externally_tagged_variant_names(
            variant_idx in 0usize..12,
        ) {
            let payload = match variant_idx {
                0 => EventPayload::WorkflowStarted {
                    workflow_name: WorkflowName::parse("w").expect("valid"),
                    binary_hash: BinaryHash::parse("abcdef0123456789").expect("valid"),
                },
                1 => EventPayload::WorkflowCompleted { result: None },
                2 => EventPayload::WorkflowFailed { error_message: "e".to_string() },
                3 => EventPayload::WorkflowCancelled { reason: "r".to_string() },
                4 => EventPayload::StepScheduled {
                    node_name: NodeName::parse("n").expect("valid"),
                    attempt: AttemptNumber::new_unchecked(1),
                },
                5 => EventPayload::StepStarted {
                    node_name: NodeName::parse("n").expect("valid"),
                    attempt: AttemptNumber::new_unchecked(1),
                    idempotency_key: IdempotencyKey::parse("k").expect("valid"),
                    binary_hash: BinaryHash::parse("abcdef0123456789").expect("valid"),
                },
                6 => EventPayload::StepCompleted {
                    node_name: NodeName::parse("n").expect("valid"),
                    attempt: AttemptNumber::new_unchecked(1),
                    result: None,
                },
                7 => EventPayload::StepFailed {
                    node_name: NodeName::parse("n").expect("valid"),
                    attempt: AttemptNumber::new_unchecked(1),
                    error_message: "e".to_string(),
                    retryable: false,
                },
                8 => EventPayload::TimerSet {
                    timer_id: TimerId::parse("t").expect("valid"),
                    fire_at: fa(0),
                },
                9 => EventPayload::TimerFired {
                    timer_id: TimerId::parse("t").expect("valid"),
                },
                10 => EventPayload::CancelRequested,
                11 => EventPayload::InstanceResumed {
                    previous_binary_hash: BinaryHash::parse("aaaa0000").expect("valid"),
                    resumed_binary_hash: BinaryHash::parse("bbbb0000").expect("valid"),
                },
                _ => panic!("invalid variant index"),
            };
            let json_val = serde_json::to_value(&payload).expect("serialize");
            let expected_tags = [
                "WorkflowStarted",
                "WorkflowCompleted",
                "WorkflowFailed",
                "WorkflowCancelled",
                "StepScheduled",
                "StepStarted",
                "StepCompleted",
                "StepFailed",
                "TimerSet",
                "TimerFired",
                "CancelRequested",
                "InstanceResumed",
            ];
            let tag = expected_tags[variant_idx];
            if variant_idx == 10 {
                prop_assert_eq!(json_val, serde_json::json!(tag));
            } else {
                prop_assert!(json_val.get(tag).is_some(), "Expected tag '{tag}' in {json_val}");
            }
        }
    }
}
