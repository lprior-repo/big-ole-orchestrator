use super::helpers::*;

mod signal_buffer_config_tests {
    use super::*;

    #[test]
    fn config_default_max_buffered_per_key_is_100() {
        assert_eq!(SignalBufferConfig::default().max_buffered_per_key, 100);
    }

    #[test]
    fn config_new_with_zero_yields_one() {
        assert_eq!(SignalBufferConfig::new(0).max_buffered_per_key, 1);
    }

    #[test]
    fn config_new_with_50_yields_50() {
        assert_eq!(SignalBufferConfig::new(50).max_buffered_per_key, 50);
    }

    #[test]
    fn config_equality() {
        assert_eq!(SignalBufferConfig::new(100), SignalBufferConfig::new(100));
        assert_ne!(SignalBufferConfig::new(100), SignalBufferConfig::new(50));
    }
}

mod signal_buffer_config_access_tests {
    use super::*;

    #[test]
    fn config_returns_stored_config() {
        let config = SignalBufferConfig::new(42);
        let buffer = SignalBuffer::new(config);
        assert_eq!(buffer.config().max_buffered_per_key, 42);
    }
}
