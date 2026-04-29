//! Signal/lease domain commands: AcquireLease, ReleaseLease.

#[cfg(test)]
mod tests {
    use crate::db_writer_message::message::DbWriterMessage;
    use vo_types::{FenceToken, InstanceId, StepId};

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01ARYZ6S410000000000000000").expect("valid instance id")
    }

    fn valid_step_id() -> StepId {
        StepId::parse("step-1").expect("valid step id")
    }

    fn valid_fence_token() -> FenceToken {
        FenceToken::new(1).expect("valid fence token")
    }

    // ========================================================================
    // B03, B04: snake_case tag serialization
    // ========================================================================

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

    // ========================================================================
    // B12, B13: Serde round-trip
    // ========================================================================

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
}
