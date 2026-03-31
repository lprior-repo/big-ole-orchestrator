use super::*;
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn envelope_from_bytes_returns_ok_when_input_is_valid_json() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123"}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        let envelope = result.unwrap();
        assert_eq!(envelope.version, 1);
        assert_eq!(envelope.instance_id, "wf-123");
        assert_eq!(envelope.sequence, 1);
        assert_eq!(envelope.timestamp_ms, 1000);
    }

    #[test]
    fn envelope_from_bytes_returns_invalid_envelope_format_when_json_is_malformed() {
        let json = r#"{"version": 1, "instance_id": "wf-123""#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(result, Err(Error::InvalidEnvelopeFormat));
    }

    #[test]
    fn envelope_from_bytes_returns_invalid_input_when_bytes_are_not_valid_utf8() {
        let bytes = vec![0xFF, 0xFE, 0xFD, 0x00];
        let result = EventEnvelope::from_bytes(&bytes);
        assert_eq!(result, Err(Error::InvalidInput));
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_version_is_absent() {
        let json = r#"{"instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(
            result,
            Err(Error::MissingEnvelopeField("version".to_string()))
        );
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_instance_id_is_absent() {
        let json = r#"{"version": 1, "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(
            result,
            Err(Error::MissingEnvelopeField("instance_id".to_string()))
        );
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_sequence_is_absent() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(
            result,
            Err(Error::MissingEnvelopeField("sequence".to_string()))
        );
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_timestamp_ms_is_absent() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(
            result,
            Err(Error::MissingEnvelopeField("timestamp_ms".to_string()))
        );
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_payload_is_absent() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(
            result,
            Err(Error::MissingEnvelopeField("payload".to_string()))
        );
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_metadata_is_absent() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(
            result,
            Err(Error::MissingEnvelopeField("metadata".to_string()))
        );
    }

    #[test]
    fn envelope_from_bytes_returns_invalid_envelope_field_when_version_is_not_integer() {
        let json = r#"{"version": "1", "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert!(matches!(result, Err(Error::InvalidEnvelopeField(_))));
    }

    #[test]
    fn envelope_from_bytes_returns_invalid_envelope_field_when_instance_id_is_empty() {
        let json = r#"{"version": 1, "instance_id": "", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert!(matches!(result, Err(Error::InvalidEnvelopeField(_))));
    }

    #[test]
    fn envelope_from_bytes_returns_invalid_envelope_field_when_sequence_is_zero() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 0, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert!(matches!(result, Err(Error::InvalidEnvelopeField(_))));
    }

    #[test]
    fn envelope_from_bytes_returns_unsupported_envelope_version_when_version_exceeds_max() {
        let json = r#"{"version": 2, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(result, Err(Error::UnsupportedEnvelopeVersion(2)));
    }

    #[test]
    fn envelope_from_bytes_returns_unsupported_envelope_version_when_version_is_u8_max() {
        let json = r#"{"version": 255, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(result, Err(Error::UnsupportedEnvelopeVersion(255)));
    }

    #[test]
    fn envelope_from_bytes_returns_invalid_envelope_field_when_metadata_is_not_object() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": []}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert!(matches!(result, Err(Error::InvalidEnvelopeField(_))));
    }

    #[test]
    fn envelope_from_str_returns_ok_when_input_is_valid_json() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123"}, "metadata": {}}"#;
        let result = EventEnvelope::from_str(json);
        result.unwrap();
    }

    #[test]
    fn envelope_from_str_returns_invalid_envelope_format_when_json_is_malformed() {
        let json = r#"{"version": 1, "instance_id": "wf-123""#;
        let result = EventEnvelope::from_str(json);
        assert_eq!(result, Err(Error::InvalidEnvelopeFormat));
    }

    #[test]
    fn envelope_is_supported_returns_true_when_version_is_zero() {
        let envelope = EventEnvelope {
            version: 0,
            instance_id: "wf-123".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: serde_json::json!({}),
        };
        assert!(envelope.is_supported());
    }

    #[test]
    fn envelope_is_supported_returns_true_when_version_is_one() {
        let envelope = EventEnvelope {
            version: 1,
            instance_id: "wf-123".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: serde_json::json!({}),
        };
        assert!(envelope.is_supported());
    }

    #[test]
    fn envelope_is_supported_returns_false_when_version_is_two() {
        let envelope = EventEnvelope {
            version: 2,
            instance_id: "wf-123".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: serde_json::json!({}),
        };
        assert!(!envelope.is_supported());
    }

    #[test]
    fn payload_try_from_json_returns_workflow_started_when_type_is_workflow_started() {
        let json =
            serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::WorkflowStarted {
                workflow_id: "wf-123".to_string()
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_workflow_completed_when_type_is_workflow_completed() {
        let json = serde_json::json!({"type": "WorkflowCompleted", "workflow_id": "wf-123", "completion_time_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::WorkflowCompleted {
                workflow_id: "wf-123".to_string(),
                completion_time_ms: 1000
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_workflow_failed_when_type_is_workflow_failed() {
        let json = serde_json::json!({"type": "WorkflowFailed", "workflow_id": "wf-123", "failure_reason": "timeout", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::WorkflowFailed {
                workflow_id: "wf-123".to_string(),
                failure_reason: "timeout".to_string()
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_workflow_cancelled_when_type_is_workflow_cancelled() {
        let json = serde_json::json!({"type": "WorkflowCancelled", "workflow_id": "wf-123", "cancelled_by": "user", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::WorkflowCancelled {
                workflow_id: "wf-123".to_string(),
                cancelled_by: "user".to_string()
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_step_scheduled_when_type_is_step_scheduled() {
        let json = serde_json::json!({"type": "StepScheduled", "workflow_id": "wf-123", "step_id": "step-1", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::StepScheduled {
                workflow_id: "wf-123".to_string(),
                step_id: "step-1".to_string()
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_step_started_when_type_is_step_started() {
        let json = serde_json::json!({"type": "StepStarted", "workflow_id": "wf-123", "step_id": "step-1", "started_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::StepStarted {
                workflow_id: "wf-123".to_string(),
                step_id: "step-1".to_string(),
                started_at_ms: 1000
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_step_completed_when_type_is_step_completed() {
        let json = serde_json::json!({"type": "StepCompleted", "workflow_id": "wf-123", "step_id": "step-1", "completed_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::StepCompleted {
                workflow_id: "wf-123".to_string(),
                step_id: "step-1".to_string(),
                completed_at_ms: 1000
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_step_failed_when_type_is_step_failed() {
        let json = serde_json::json!({"type": "StepFailed", "workflow_id": "wf-123", "step_id": "step-1", "failure_reason": "error", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::StepFailed {
                workflow_id: "wf-123".to_string(),
                step_id: "step-1".to_string(),
                failure_reason: "error".to_string()
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_timer_set_when_type_is_timer_set() {
        let json = serde_json::json!({"type": "TimerSet", "workflow_id": "wf-123", "timer_id": "timer-1", "fire_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::TimerSet {
                workflow_id: "wf-123".to_string(),
                timer_id: "timer-1".to_string(),
                fire_at_ms: 1000
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_timer_fired_when_type_is_timer_fired() {
        let json = serde_json::json!({"type": "TimerFired", "workflow_id": "wf-123", "timer_id": "timer-1", "fired_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::TimerFired {
                workflow_id: "wf-123".to_string(),
                timer_id: "timer-1".to_string(),
                fired_at_ms: 1000
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_cancel_requested_when_type_is_cancel_requested() {
        let json = serde_json::json!({"type": "CancelRequested", "workflow_id": "wf-123", "requested_by": "user", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::CancelRequested {
                workflow_id: "wf-123".to_string(),
                requested_by: "user".to_string()
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_instance_resumed_when_type_is_instance_resumed() {
        let json = serde_json::json!({"type": "InstanceResumed", "workflow_id": "wf-123", "resumed_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::InstanceResumed {
                workflow_id: "wf-123".to_string(),
                resumed_at_ms: 1000
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_unknown_payload_type_when_type_is_unrecognized() {
        let json =
            serde_json::json!({"type": "UnknownType", "workflow_id": "wf-123", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Err(Error::UnknownPayloadType("UnknownType".to_string()))
        );
    }

    #[test]
    fn payload_try_from_json_returns_unsupported_payload_version_when_version_exceeds_max() {
        let json =
            serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 2});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(result, Err(Error::UnsupportedPayloadVersion(2)));
    }

    #[test]
    fn payload_try_from_json_returns_missing_payload_field_when_type_is_absent() {
        let json = serde_json::json!({"workflow_id": "wf-123", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(result, Err(Error::MissingPayloadField("type".to_string())));
    }

    #[test]
    fn payload_try_from_json_returns_invalid_payload_field_when_variant_field_is_absent() {
        let json = serde_json::json!({"type": "WorkflowStarted", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert!(matches!(result, Err(Error::InvalidPayloadField(_))));
    }

    #[test]
    fn payload_try_from_json_returns_invalid_payload_format_when_json_is_malformed() {
        let json = serde_json::Value::String("not an object".to_string());
        let result = EventPayload::try_from_json(&json);
        assert_eq!(result, Err(Error::InvalidPayloadFormat));
    }

    #[test]
    fn payload_is_version_supported_returns_true_when_version_is_zero() {
        assert!(EventPayload::is_version_supported(0));
    }

    #[test]
    fn payload_is_version_supported_returns_true_when_version_is_one() {
        assert!(EventPayload::is_version_supported(1));
    }

    #[test]
    fn payload_is_version_supported_returns_false_when_version_is_two() {
        assert!(!EventPayload::is_version_supported(2));
    }

    #[test]
    fn payload_is_version_supported_returns_false_when_version_is_u8_max() {
        assert!(!EventPayload::is_version_supported(u8::MAX));
    }

    #[test]
    fn decode_event_returns_ok_when_envelope_and_payload_are_valid() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1}, "metadata": {}}"#;
        let result = decode_event(json.as_bytes());
        let (envelope, payload) = result.unwrap();
        assert_eq!(envelope.version, 1);
        assert_eq!(envelope.instance_id, "wf-123");
        assert_eq!(envelope.sequence, 1);
        assert_eq!(envelope.timestamp_ms, 1000);
        assert!(matches!(payload, EventPayload::WorkflowStarted { .. }));
    }

    #[test]
    fn decode_event_returns_envelope_decode_failed_when_envelope_is_malformed() {
        let json = r#"{"version": 1, "instance_id": "wf-123""#;
        let result = decode_event(json.as_bytes());
        assert!(matches!(result, Err(Error::EnvelopeDecodeFailed(_))));
    }

    #[test]
    fn decode_event_returns_payload_decode_failed_when_payload_is_invalid() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "UnknownType", "version": 1}, "metadata": {}}"#;
        let result = decode_event(json.as_bytes());
        assert!(matches!(result, Err(Error::PayloadDecodeFailed(_))));
    }

    #[test]
    fn decode_event_returns_payload_decode_skipped_when_envelope_version_exceeds_max() {
        let json = r#"{"version": 2, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "version": 1}, "metadata": {}}"#;
        let result = decode_event(json.as_bytes());
        assert_eq!(result, Err(Error::PayloadDecodeSkipped));
    }

    #[rstest]
    #[case(0, 1, "wf-1", 0)]
    #[case(1, 100, "wf-abc", 1000)]
    fn envelope_roundtrip_preserves_data(
        #[case] version: u8,
        #[case] seq: u64,
        #[case] instance_id: &str,
        #[case] ts: u64,
    ) {
        let json = serde_json::json!({
            "version": version,
            "instance_id": instance_id,
            "sequence": seq,
            "timestamp_ms": ts,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        
        let expected = EventEnvelope {
            version,
            instance_id: instance_id.to_string(),
            sequence: seq,
            timestamp_ms: ts,
            payload: serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1}),
            metadata: serde_json::json!({}),
        };
        assert_eq!(result, Ok(expected));
    }

    #[rstest]
    #[case(0)]
    #[case(1)]
    #[case(2)]
    #[case(3)]
    #[case(4)]
    #[case(5)]
    fn proptest_version_support_is_consistent_across_envelope_and_payload(#[case] version: u8) {
        let envelope = EventEnvelope {
            version,
            instance_id: "wf-123".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: serde_json::json!({}),
        };
        let envelope_supported = envelope.is_supported();
        let payload_supported = EventPayload::is_version_supported(version);
        assert_eq!(
            envelope_supported, payload_supported,
            "Inconsistent for version {}",
            version
        );
    }

    #[rstest]
    #[case(1, "wf-1")]
    #[case(100, "wf-100")]
    #[case(999_999_999, "wf-max")]
    fn envelope_parsing_accepts_positive_sequence(#[case] seq: u64, #[case] instance_id: &str) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": instance_id,
            "sequence": seq,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        let Ok(envelope) = result else {
            panic!("Expected Ok, got {:?}", result);
        };
        assert_eq!(envelope.sequence, seq);
    }

    #[rstest]
    #[case(0, "wf-zero")]
    fn envelope_parsing_rejects_zero_sequence(#[case] seq: u64, #[case] instance_id: &str) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": instance_id,
            "sequence": seq,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        let Err(err) = result else {
            panic!("Expected Err, got {:?}", result);
        };
        assert!(matches!(err, Error::InvalidEnvelopeField(_)));
    }

    #[rstest]
    #[case("a")]
    #[case("wf-123")]
    #[case("instance_with_underscores")]
    fn envelope_parsing_accepts_nonempty_instance_id(#[case] instance_id: &str) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": instance_id,
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        let Ok(envelope) = result else {
            panic!("Expected Ok, got {:?}", result);
        };
        assert_eq!(envelope.instance_id, instance_id);
    }

    #[rstest]
    #[case("")]
    fn envelope_parsing_rejects_empty_instance_id(#[case] instance_id: &str) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": instance_id,
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        let Err(err) = result else {
            panic!("Expected Err, got {:?}", result);
        };
        assert!(matches!(err, Error::InvalidEnvelopeField(_)));
    }

    #[rstest]
    #[case(serde_json::json!({}))]
    #[case(serde_json::json!({"key": "value"}))]
    #[case(serde_json::json!({"key1": "value1", "key2": 123}))]
    fn envelope_parsing_accepts_object_metadata(#[case] metadata: serde_json::Value) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": "wf-123",
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": metadata
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        let Ok(envelope) = result else {
            panic!("Expected Ok, got {:?}", result);
        };
        assert_eq!(envelope.metadata, metadata);
    }

    #[rstest]
    #[case(serde_json::json!([]))]
    #[case(serde_json::json!("string"))]
    #[case(serde_json::Value::Null)]
    #[case(serde_json::json!(123))]
    fn envelope_parsing_rejects_non_object_metadata(#[case] metadata: serde_json::Value) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": "wf-123",
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": metadata
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        let Err(err) = result else {
            panic!("Expected Err, got {:?}", result);
        };
        assert!(matches!(err, Error::InvalidEnvelopeField(_)));
    }

    #[rstest]
    #[case(r#"{"version": 1, "instance_id": "wf-123", "sequence": "bad", "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "w1"}, "metadata": {}}"#, Error::InvalidEnvelopeField("sequence must be an integer".to_string()))]
    #[case(r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": "bad", "payload": {"type": "WorkflowStarted", "workflow_id": "w1"}, "metadata": {}}"#, Error::InvalidEnvelopeField("timestamp_ms must be an integer".to_string()))]
    fn envelope_from_str_invalid_types(#[case] json: &str, #[case] expected: Error) {
        let result = EventEnvelope::from_str(json);
        assert_eq!(result, Err(expected));
    }

    #[rstest]
    #[case(serde_json::json!({"type": "WorkflowStarted", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowStarted", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCompleted", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCompleted", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCompleted", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("completion_time_ms".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCompleted", "workflow_id": "w1", "completion_time_ms": "bad", "version": 1}), Error::InvalidPayloadField("completion_time_ms must be an integer".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowFailed", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowFailed", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowFailed", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("failure_reason".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowFailed", "workflow_id": "w1", "failure_reason": 123, "version": 1}), Error::InvalidPayloadField("failure_reason must be a string".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCancelled", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCancelled", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCancelled", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("cancelled_by".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCancelled", "workflow_id": "w1", "cancelled_by": 123, "version": 1}), Error::InvalidPayloadField("cancelled_by must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepScheduled", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "StepScheduled", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("step_id".to_string()))]
    #[case(serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": 123, "version": 1}), Error::InvalidPayloadField("step_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepStarted", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "StepStarted", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepStarted", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("step_id".to_string()))]
    #[case(serde_json::json!({"type": "StepStarted", "workflow_id": "w1", "step_id": 123, "version": 1}), Error::InvalidPayloadField("step_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepStarted", "workflow_id": "w1", "step_id": "s1", "version": 1}), Error::MissingPayloadField("started_at_ms".to_string()))]
    #[case(serde_json::json!({"type": "StepStarted", "workflow_id": "w1", "step_id": "s1", "started_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("started_at_ms must be an integer".to_string()))]
    #[case(serde_json::json!({"type": "StepCompleted", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "StepCompleted", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("step_id".to_string()))]
    #[case(serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "step_id": 123, "version": 1}), Error::InvalidPayloadField("step_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "step_id": "s1", "version": 1}), Error::MissingPayloadField("completed_at_ms".to_string()))]
    #[case(serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "step_id": "s1", "completed_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("completed_at_ms must be an integer".to_string()))]
    #[case(serde_json::json!({"type": "StepFailed", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "StepFailed", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("step_id".to_string()))]
    #[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": 123, "version": 1}), Error::InvalidPayloadField("step_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": "s1", "version": 1}), Error::MissingPayloadField("failure_reason".to_string()))]
    #[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": "s1", "failure_reason": 123, "version": 1}), Error::InvalidPayloadField("failure_reason must be a string".to_string()))]
    #[case(serde_json::json!({"type": "TimerSet", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "TimerSet", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "TimerSet", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("timer_id".to_string()))]
    #[case(serde_json::json!({"type": "TimerSet", "workflow_id": "w1", "timer_id": 123, "version": 1}), Error::InvalidPayloadField("timer_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "TimerSet", "workflow_id": "w1", "timer_id": "t1", "version": 1}), Error::MissingPayloadField("fire_at_ms".to_string()))]
    #[case(serde_json::json!({"type": "TimerSet", "workflow_id": "w1", "timer_id": "t1", "fire_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("fire_at_ms must be an integer".to_string()))]
    #[case(serde_json::json!({"type": "TimerFired", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "TimerFired", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "TimerFired", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("timer_id".to_string()))]
    #[case(serde_json::json!({"type": "TimerFired", "workflow_id": "w1", "timer_id": 123, "version": 1}), Error::InvalidPayloadField("timer_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "TimerFired", "workflow_id": "w1", "timer_id": "t1", "version": 1}), Error::MissingPayloadField("fired_at_ms".to_string()))]
    #[case(serde_json::json!({"type": "TimerFired", "workflow_id": "w1", "timer_id": "t1", "fired_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("fired_at_ms must be an integer".to_string()))]
    #[case(serde_json::json!({"type": "CancelRequested", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "CancelRequested", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "CancelRequested", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("requested_by".to_string()))]
    #[case(serde_json::json!({"type": "CancelRequested", "workflow_id": "w1", "requested_by": 123, "version": 1}), Error::InvalidPayloadField("requested_by must be a string".to_string()))]
    #[case(serde_json::json!({"type": "InstanceResumed", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "InstanceResumed", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "InstanceResumed", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("resumed_at_ms".to_string()))]
    #[case(serde_json::json!({"type": "InstanceResumed", "workflow_id": "w1", "resumed_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("resumed_at_ms must be an integer".to_string()))]

    fn payload_invalid_fields(#[case] json: serde_json::Value, #[case] expected: Error) {
        let result = EventPayload::try_from_json(&json);
        assert_eq!(result, Err(expected));
    }
}
