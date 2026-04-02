#[cfg(kani)]
mod verification {
    use wtf_types::*;

    #[kani::proof]
    fn verify_sequence_number_always_ge_one() {
        let value: u64 = kani::any();
        if let Ok(sn) = SequenceNumber::try_from(value) {
            assert!(sn.as_u64() >= 1);
        }
    }

    #[kani::proof]
    fn verify_event_version_always_ge_one() {
        let value: u64 = kani::any();
        if let Ok(ev) = EventVersion::try_from(value) {
            assert!(ev.as_u64() >= 1);
        }
    }

    #[kani::proof]
    fn verify_event_error_discriminant_coverage() {
        let err = EventError::DeserializationFailed {
            message: "test".to_string(),
        };
        assert!(matches!(err, EventError::DeserializationFailed { .. }));

        let err = EventError::UnsupportedVersion {
            actual: 2,
            max_supported: 1,
        };
        assert!(matches!(err, EventError::UnsupportedVersion { .. }));

        let err = EventError::InvalidPayload {
            message: "test".to_string(),
        };
        assert!(matches!(err, EventError::InvalidPayload { .. }));
    }
}
