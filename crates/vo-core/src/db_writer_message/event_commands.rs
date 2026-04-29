//! Event domain commands: AppendEvent, RecordEffect.

#[cfg(test)]
mod tests {
    use crate::db_writer_message::message::DbWriterMessage;
    use vo_types::{
        EffectIntent, EffectKind, EffectRecord, IdempotencyKey, InstanceId, SequenceNumber,
    };

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01ARYZ6S410000000000000000").expect("valid instance id")
    }

    fn valid_sequence() -> SequenceNumber {
        SequenceNumber::new_unchecked(1)
    }

    fn valid_idempotency_key() -> IdempotencyKey {
        IdempotencyKey::parse("key-1").expect("valid key")
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

    // ========================================================================
    // B01, B07: snake_case tag serialization
    // ========================================================================

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

    // ========================================================================
    // B10, B16: Serde round-trip
    // ========================================================================

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
    fn record_effect_round_trips_through_serde_json() {
        let msg = DbWriterMessage::RecordEffect {
            effect: valid_effect_record(),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let recovered: DbWriterMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, recovered);
    }
}
