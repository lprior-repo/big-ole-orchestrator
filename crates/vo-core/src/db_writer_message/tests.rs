#[cfg(test)]
mod tests {
    use crate::db_writer_message::message::DbWriterMessage;
    use crate::db_writer_message::types::{DbWriterMessageError, SnapshotData, TimerOp};
    use vo_types::{
        EffectIntent, EffectKind, EffectRecord, EventEnvelope, FenceToken, FireAtMs,
        IdempotencyKey, InstanceId, InstanceStatus, SequenceNumber, StepId, TimerId,
        MAX_SUPPORTED_SCHEMA_VERSION,
    };
    use vo_types::events::EventMetadata;

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01ARYZ6S410000000000000000").expect("valid instance id")
    }

    fn valid_sequence() -> SequenceNumber {
        SequenceNumber::new_unchecked(1)
    }

    fn valid_idempotency_key() -> IdempotencyKey {
        IdempotencyKey::parse("key-1").expect("valid key")
    }

    fn valid_step_id() -> StepId {
        StepId::parse("step-1").expect("valid step id")
    }

    fn valid_fence_token() -> FenceToken {
        FenceToken::new(1).expect("valid fence token")
    }

    fn valid_timer_id() -> TimerId {
        TimerId::parse("timer-1").expect("valid timer id")
    }

    fn valid_fire_at() -> FireAtMs {
        FireAtMs::try_from(1712200000000u64).expect("valid fire_at")
    }

    fn valid_event_envelope() -> EventEnvelope {
        EventEnvelope {
            schema_version: 1,
            instance_id: "01ARYZ6S410000000000000000".to_string(),
            sequence: 1,
            timestamp_ms: 1712200000000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        }
    }

    fn valid_effect_record() -> EffectRecord {
        EffectRecord::new(
            "intent-1".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .expect("valid effect record")
    }

    fn valid_snapshot_data() -> SnapshotData {
        SnapshotData::new(
            valid_sequence(),
            MAX_SUPPORTED_SCHEMA_VERSION,
            vec![0x01, 0x02, 0x03],
        )
        .expect("valid snapshot data")
    }

    // B01-B09: snake_case tag serialization

    #[test]
    fn append_event_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            idempotency_key: valid_idempotency_key(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"append_event\""),
            "expected snake_case tag 'append_event', got: {json}"
        );
    }

    #[test]
    fn record_instance_status_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::RecordInstanceStatus {
            instance_id: valid_instance_id(),
            status_byte: 0x01,
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"record_instance_status\""),
            "expected snake_case tag 'record_instance_status', got: {json}"
        );
    }

    #[test]
    fn acquire_lease_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::AcquireLease {
            instance_id: valid_instance_id(),
            step_id: valid_step_id(),
            fence: valid_fence_token(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"acquire_lease\""),
            "expected snake_case tag 'acquire_lease', got: {json}"
        );
    }

    #[test]
    fn release_lease_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::ReleaseLease {
            instance_id: valid_instance_id(),
            step_id: valid_step_id(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"release_lease\""),
            "expected snake_case tag 'release_lease', got: {json}"
        );
    }

    #[test]
    fn upsert_timer_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::UpsertTimer {
            instance_id: valid_instance_id(),
            timer_id: valid_timer_id(),
            fire_at: valid_fire_at(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"upsert_timer\""),
            "expected snake_case tag 'upsert_timer', got: {json}"
        );
    }

    #[test]
    fn delete_timer_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::DeleteTimer {
            instance_id: valid_instance_id(),
            timer_id: valid_timer_id(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"delete_timer\""),
            "expected snake_case tag 'delete_timer', got: {json}"
        );
    }

    #[test]
    fn record_effect_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::RecordEffect {
            effect: valid_effect_record(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"record_effect\""),
            "expected snake_case tag 'record_effect', got: {json}"
        );
    }

    #[test]
    fn take_snapshot_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::TakeSnapshot {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            snapshot_data: valid_snapshot_data(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"take_snapshot\""),
            "expected snake_case tag 'take_snapshot', got: {json}"
        );
    }

    #[test]
    fn atomic_transition_serializes_with_snake_case_tag() {
        let msg = DbWriterMessage::AtomicTransition {
            step_id: Some(valid_step_id()),
            instance_status: Some(InstanceStatus::Running),
            timer_ops: vec![TimerOp::Upsert {
                timer_id: valid_timer_id(),
                fire_at: valid_fire_at(),
            }],
            snapshot: Some(valid_snapshot_data()),
            event: valid_event_envelope(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(
            json.contains("\"atomic_transition\""),
            "expected snake_case tag 'atomic_transition', got: {json}"
        );
    }

    // B10-B21: Serde round-trip tests

    #[test]
    fn append_event_round_trips_through_serde_json() {
        let msg = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            idempotency_key: valid_idempotency_key(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn record_instance_status_round_trips_through_serde_json() {
        let msg = DbWriterMessage::RecordInstanceStatus {
            instance_id: valid_instance_id(),
            status_byte: 0x02,
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn acquire_lease_round_trips_through_serde_json() {
        let msg = DbWriterMessage::AcquireLease {
            instance_id: valid_instance_id(),
            step_id: valid_step_id(),
            fence: valid_fence_token(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn release_lease_round_trips_through_serde_json() {
        let msg = DbWriterMessage::ReleaseLease {
            instance_id: valid_instance_id(),
            step_id: valid_step_id(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn upsert_timer_round_trips_through_serde_json() {
        let msg = DbWriterMessage::UpsertTimer {
            instance_id: valid_instance_id(),
            timer_id: valid_timer_id(),
            fire_at: valid_fire_at(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn delete_timer_round_trips_through_serde_json() {
        let msg = DbWriterMessage::DeleteTimer {
            instance_id: valid_instance_id(),
            timer_id: valid_timer_id(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn record_effect_round_trips_through_serde_json() {
        let msg = DbWriterMessage::RecordEffect {
            effect: valid_effect_record(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn take_snapshot_round_trips_through_serde_json() {
        let msg = DbWriterMessage::TakeSnapshot {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            snapshot_data: valid_snapshot_data(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn atomic_transition_round_trips_through_serde_json_with_all_fields() {
        let msg = DbWriterMessage::AtomicTransition {
            step_id: Some(valid_step_id()),
            instance_status: Some(InstanceStatus::Running),
            timer_ops: vec![
                TimerOp::Upsert {
                    timer_id: valid_timer_id(),
                    fire_at: valid_fire_at(),
                },
                TimerOp::Delete {
                    timer_id: TimerId::parse("timer-del").expect("valid"),
                },
            ],
            snapshot: Some(valid_snapshot_data()),
            event: valid_event_envelope(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn atomic_transition_round_trips_through_serde_json_with_minimal_fields() {
        let msg = DbWriterMessage::AtomicTransition {
            step_id: None,
            instance_status: None,
            timer_ops: vec![],
            snapshot: None,
            event: valid_event_envelope(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }

    #[test]
    fn timer_op_upsert_round_trips_through_serde_json() {
        let op = TimerOp::Upsert {
            timer_id: valid_timer_id(),
            fire_at: valid_fire_at(),
        };
        let json = serde_json::to_string(&op).expect("serialize");
        let recovered: TimerOp = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(op, recovered);
    }

    #[test]
    fn timer_op_delete_round_trips_through_serde_json() {
        let op = TimerOp::Delete {
            timer_id: valid_timer_id(),
        };
        let json = serde_json::to_string(&op).expect("serialize");
        let recovered: TimerOp = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(op, recovered);
    }

    #[test]
    fn snapshot_data_round_trips_through_serde_json() {
        let sd = valid_snapshot_data();
        let json = serde_json::to_string(&sd).expect("serialize");
        let recovered: SnapshotData = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(sd, recovered);
    }

    // B22-B25: Deserialization rejection

    #[test]
    fn db_writer_message_rejects_unknown_variant_when_deserializing() {
        let json = r#"{"not_a_real_variant":{"instance_id":"01ARYZ6S410000000000000000"}}"#;
        let result: Result<DbWriterMessage, _> = serde_json::from_str(json);
        assert!(result.is_err(), "expected Err for unknown variant");
    }

    #[test]
    fn db_writer_message_rejects_missing_required_field_when_deserializing() {
        let json = r#"{"record_instance_status":{}}"#;
        let result: Result<DbWriterMessage, _> = serde_json::from_str(json);
        assert!(result.is_err(), "expected Err for missing required field");
    }

    #[test]
    fn db_writer_message_rejects_malformed_json_when_deserializing() {
        let json = r#"{ invalid }"#;
        let result: Result<DbWriterMessage, _> = serde_json::from_str(json);
        assert!(result.is_err(), "expected Err for malformed JSON");
    }

    #[test]
    fn db_writer_message_rejects_truncated_json_when_deserializing() {
        let json = r#"{"append_ev"#;
        let result: Result<DbWriterMessage, _> = serde_json::from_str(json);
        assert!(result.is_err(), "expected Err for truncated JSON");
    }

    // B26-B29: Nonzero guarantees

    #[test]
    fn fence_token_rejects_zero_value_when_constructed() {
        let result = FenceToken::new(0);
        assert!(result.is_err());
    }

    #[test]
    fn fence_token_rejects_zero_value_when_deserialized_from_json() {
        let result: Result<FenceToken, _> = serde_json::from_str("0");
        assert!(result.is_err());
    }

    #[test]
    fn sequence_number_rejects_zero_value_when_constructed() {
        let result = SequenceNumber::try_from(0u64);
        assert!(result.is_err());
    }

    #[test]
    fn sequence_number_rejects_zero_value_when_deserialized_from_json() {
        let result: Result<SequenceNumber, _> = serde_json::from_str("0");
        assert!(result.is_err());
    }

    // B30-B33: Non-empty string guarantees

    #[test]
    fn instance_id_rejects_empty_string_when_parsed() {
        let result = InstanceId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn step_id_rejects_empty_string_when_parsed() {
        let result = StepId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn timer_id_rejects_empty_string_when_parsed() {
        let result = TimerId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn idempotency_key_rejects_empty_string_when_parsed() {
        let result = IdempotencyKey::parse("");
        assert!(result.is_err());
    }

    // B34-B38: Error display

    #[test]
    fn db_writer_message_error_zero_fence_token_displays_expected_message() {
        let err = DbWriterMessageError::ZeroFenceToken;
        assert_eq!(err.to_string(), "fence token must be nonzero");
    }

    #[test]
    fn db_writer_message_error_zero_sequence_number_displays_expected_message() {
        let err = DbWriterMessageError::ZeroSequenceNumber;
        assert_eq!(err.to_string(), "sequence number must be nonzero");
    }

    #[test]
    fn db_writer_message_error_serialization_error_displays_inner_message() {
        let err = DbWriterMessageError::SerializationError("oops".to_string());
        assert_eq!(err.to_string(), "serialization error: oops");
    }

    #[test]
    fn db_writer_message_error_unknown_variant_displays_variant_name() {
        let err = DbWriterMessageError::UnknownVariant("bogus".to_string());
        assert_eq!(err.to_string(), "unknown DbWriterMessage variant: bogus");
    }

    #[test]
    fn db_writer_message_error_missing_field_displays_field_name() {
        let err = DbWriterMessageError::MissingField("instance_id".to_string());
        assert_eq!(err.to_string(), "missing field: instance_id");
    }

    // B39-B42: PartialEq/Eq correctness

    #[test]
    fn db_writer_message_equal_values_compare_equal() {
        let msg1 = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            idempotency_key: valid_idempotency_key(),
        };
        let msg2 = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            idempotency_key: valid_idempotency_key(),
        };
        assert_eq!(msg1, msg2);
    }

    #[test]
    fn db_writer_message_different_variants_compare_unequal() {
        let msg1 = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            idempotency_key: valid_idempotency_key(),
        };
        let msg2 = DbWriterMessage::RecordInstanceStatus {
            instance_id: valid_instance_id(),
            status_byte: 0x01,
        };
        assert_ne!(msg1, msg2);
    }

    #[test]
    fn timer_op_different_variants_compare_unequal() {
        let op1 = TimerOp::Upsert {
            timer_id: TimerId::parse("t1").expect("valid"),
            fire_at: FireAtMs::try_from(100u64).expect("valid"),
        };
        let op2 = TimerOp::Delete {
            timer_id: TimerId::parse("t1").expect("valid"),
        };
        assert_ne!(op1, op2);
    }

    #[test]
    fn snapshot_data_different_state_bytes_compare_unequal() {
        let sd1 = SnapshotData::new(valid_sequence(), MAX_SUPPORTED_SCHEMA_VERSION, vec![0x01])
            .expect("valid snapshot data");
        let sd2 = SnapshotData::new(valid_sequence(), MAX_SUPPORTED_SCHEMA_VERSION, vec![0x02])
            .expect("valid snapshot data");
        assert_ne!(sd1, sd2);
    }

    // SnapshotData invariants

    #[test]
    fn snapshot_data_new_returns_some_when_state_bytes_non_empty() {
        let snap = SnapshotData::new(
            valid_sequence(),
            MAX_SUPPORTED_SCHEMA_VERSION,
            vec![0x01, 0x02, 0x03],
        );
        assert!(snap.is_some());
    }

    #[test]
    fn snapshot_data_new_returns_none_when_state_bytes_empty() {
        let snap = SnapshotData::new(valid_sequence(), MAX_SUPPORTED_SCHEMA_VERSION, vec![]);
        assert_eq!(snap, None);
    }
}
