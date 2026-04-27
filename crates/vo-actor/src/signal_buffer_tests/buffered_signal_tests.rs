use super::helpers::*;

mod buffered_signal_tests {
    use super::*;

    #[test]
    fn buffered_signal_constructs_with_all_fields() {
        let payload = crate::SignalPayload::from_bytes(vec![1, 2, 3]).unwrap();
        let ts = vo_types::TimestampMs::now();
        let signal = BufferedSignal::new(
            crate::SignalName::parse("sig-42").unwrap(),
            payload.clone(),
            ts,
        );
        assert_eq!(signal.signal_id.as_str(), "sig-42");
        assert_eq!(signal.payload.as_bytes(), &[1, 2, 3]);
        assert_eq!(signal.buffered_at, ts);
    }

    #[test]
    fn buffered_signal_clone_is_independent() {
        let signal = make_signal("sig-clone");
        assert_eq!(signal, signal.clone());
    }
}

mod buffer_result_tests {
    use super::*;

    #[test]
    fn buffer_result_variants() {
        assert!(format!("{:?}", BufferResult::Rejected).contains("Rejected"));
        assert!(format!("{:?}", BufferResult::Buffered).contains("Buffered"));
        assert!(format!("{:?}", BufferResult::Dropped).contains("Dropped"));
    }
}
