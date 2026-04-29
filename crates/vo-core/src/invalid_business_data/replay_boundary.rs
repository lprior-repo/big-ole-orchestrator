mod replay_boundary {
    use super::*;
    use crate::replay::ReplayError;

    #[test]
    fn replay_error_instance_mismatch_display() {
        let err = ReplayError::InstanceMismatch {
            expected: "inst-001".to_string(),
            actual: "inst-999".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("inst-001"));
        assert!(msg.contains("inst-999"));
        assert!(msg.contains("mismatch"));
    }

    #[test]
    fn replay_error_sequence_gap_display() {
        let err = ReplayError::SequenceGap {
            expected: 5,
            actual: 10,
            at_index: 4,
        };
        let msg = err.to_string();
        assert!(msg.contains("gap"));
        assert!(msg.contains("5"));
        assert!(msg.contains("10"));
    }

    #[test]
    fn replay_error_sequence_duplicate_display() {
        let err = ReplayError::SequenceDuplicate {
            sequence: 42,
            first_at_index: 3,
            second_at_index: 7,
        };
        let msg = err.to_string();
        assert!(msg.contains("Duplicate"));
        assert!(msg.contains("42"));
    }

    #[test]
    fn replay_error_payload_decode_failed_display() {
        let err = ReplayError::PayloadDecodeFailed {
            sequence: 10,
            source: "invalid UTF-8".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("decode failed"));
        assert!(msg.contains("invalid UTF-8"));
    }

    #[test]
    fn replay_error_transition_failed_display() {
        let err = ReplayError::TransitionFailed {
            sequence: 5,
            state: vo_types::state::LifecycleState::Completed,
            reason: "invalid transition".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Transition failed"));
        assert!(msg.contains("invalid transition"));
    }

    #[test]
    fn replay_error_unexpected_event_type_display() {
        let err = ReplayError::UnexpectedEventType {
            payload_type: "UnknownVariant".to_string(),
            sequence: 3,
        };
        let msg = err.to_string();
        assert!(msg.contains("UnknownVariant"));
    }

    #[test]
    fn replay_error_upcasting_failed_display() {
        let err = ReplayError::UpcastingFailed {
            sequence: 7,
            reason: "version too new".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Upcasting failed"));
        assert!(msg.contains("version too new"));
    }
}
