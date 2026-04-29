//! Instance domain commands: RecordInstanceStatus, AtomicTransition.

#[cfg(test)]
mod tests {
    use crate::db_writer_message::message::DbWriterMessage;
    use crate::db_writer_message::snapshot_commands::SnapshotData;
    use crate::db_writer_message::timer_commands::TimerOp;
    use vo_types::events::EventMetadata;
    use vo_types::{
        FireAtMs, InstanceId, InstanceStatus, SequenceNumber, StepId, TimerId,
        MAX_SUPPORTED_SCHEMA_VERSION,
    };

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01ARYZ6S410000000000000000").expect("valid instance id")
    }

    fn valid_sequence() -> SequenceNumber {
        SequenceNumber::new_unchecked(1)
    }

    fn valid_step_id() -> StepId {
        StepId::parse("step-1").expect("valid step id")
    }

    fn valid_timer_id() -> TimerId {
        TimerId::parse("timer-1").expect("valid timer id")
    }

    fn valid_fire_at() -> FireAtMs {
        FireAtMs::try_from(1712200000000u64).expect("valid fire_at")
    }

    fn valid_event_envelope() -> vo_types::EventEnvelope {
        vo_types::EventEnvelope {
            schema_version: 1,
            instance_id: "01ARYZ6S410000000000000000".to_string(),
            sequence: 1,
            timestamp_ms: 1712200000000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        }
    }

    fn valid_snapshot_data() -> SnapshotData {
        SnapshotData::new(
            valid_sequence(),
            MAX_SUPPORTED_SCHEMA_VERSION,
            vec![0x01, 0x02, 0x03],
        )
        .expect("valid snapshot data")
    }

    // ========================================================================
    // B02, B09: snake_case tag serialization
    // ========================================================================

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

    // ========================================================================
    // B11, B18, B19: Serde round-trip
    // ========================================================================

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
}
