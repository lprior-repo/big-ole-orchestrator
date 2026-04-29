mod circuit_breaker_failure_window_boundary {
    use super::*;
    use crate::circuit_breaker::failure_window::{
        record_failure_in_window, unique_failures_in_window, FailureWindow,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn failure_window_new_is_empty() {
        let window = FailureWindow::new();
        assert!(window.is_empty());
        assert_eq!(window.len(), 0);
    }

    #[test]
    fn failure_window_record_increases_count() {
        let mut window = FailureWindow::new();
        let now = Instant::now();
        let hash = vo_types::BinaryHash::parse("aabbccdd").unwrap();
        let window_duration = Duration::from_secs(60);
        let count = record_failure_in_window(&mut window, hash, now, window_duration);
        assert_eq!(count, 1);
    }

    #[test]
    fn failure_window_duplicate_hash_does_not_increase_count() {
        let mut window = FailureWindow::new();
        let now = Instant::now();
        let hash = vo_types::BinaryHash::parse("aabbccdd").unwrap();
        let window_duration = Duration::from_secs(60);
        record_failure_in_window(&mut window, hash.clone(), now, window_duration);
        let count = record_failure_in_window(&mut window, hash, now, window_duration);
        assert_eq!(count, 1);
    }

    #[test]
    fn failure_window_records_expire() {
        let mut window = FailureWindow::new();
        let window_duration = Duration::from_millis(1);
        let past = Instant::now() - Duration::from_secs(10);
        let hash = vo_types::BinaryHash::parse("aabbccdd").unwrap();
        record_failure_in_window(&mut window, hash, past, window_duration);
        let now = Instant::now();
        let count = unique_failures_in_window(&mut window, now, window_duration);
        assert_eq!(count, 0);
    }
}
