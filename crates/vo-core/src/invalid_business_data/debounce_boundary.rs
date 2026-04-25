mod debounce_boundary {
    use crate::debounce::Error;
    use std::time::Duration;

    #[test]
    fn error_display_all_variants() {
        let errors = vec![
            Error::InvalidDebounceDuration,
            Error::WatcherChannelClosed,
            Error::DebouncerInternal,
            Error::NoRuntime,
        ];
        for err in &errors {
            let msg = err.to_string();
            assert!(!msg.is_empty(), "error display empty for {:?}", err);
        }
    }

    #[test]
    fn error_zero_duration_message_is_descriptive() {
        let msg = Error::InvalidDebounceDuration.to_string();
        assert!(msg.to_lowercase().contains("zero"));
    }
}