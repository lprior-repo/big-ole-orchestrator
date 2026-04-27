//! Unit tests for vo-common public API types.
//!
//! Tests cover:
//! - TimestampMs edge cases and ordering
//! - VoError From conversions
//! - WorkflowEvent and EventDedup through public API

use vo_common::{EventId, VoError, WorkflowEvent};
use vo_common::types::TimestampMs;

#[cfg(test)]
mod timestamp_ms_tests {
    use super::*;

    #[test]
    fn timestamp_ms_new_unchecked_accepts_zero() {
        let ts = TimestampMs::new_unchecked(0);
        assert_eq!(ts.as_u64(), 0);
    }

    #[test]
    fn timestamp_ms_new_unchecked_accepts_u64_max() {
        let ts = TimestampMs::new_unchecked(u64::MAX);
        assert_eq!(ts.as_u64(), u64::MAX);
    }

    #[test]
    fn timestamp_ms_new_unchecked_accepts_large_value() {
        let ts = TimestampMs::new_unchecked(1_000_000_000_000);
        assert_eq!(ts.as_u64(), 1_000_000_000_000);
    }

    #[test]
    fn timestamp_ms_ordering_less_than() {
        let a = TimestampMs::new_unchecked(100);
        let b = TimestampMs::new_unchecked(200);
        assert!(a < b);
    }

    #[test]
    fn timestamp_ms_ordering_greater_than() {
        let a = TimestampMs::new_unchecked(300);
        let b = TimestampMs::new_unchecked(150);
        assert!(a > b);
    }

    #[test]
    fn timestamp_ms_ordering_equal() {
        let a = TimestampMs::new_unchecked(42);
        let b = TimestampMs::new_unchecked(42);
        assert!(a == b);
    }

    #[test]
    fn timestamp_ms_ordering_reflexive() {
        let ts = TimestampMs::new_unchecked(99);
        assert!(ts <= ts);
        assert!(ts >= ts);
    }

    #[test]
    fn timestamp_ms_as_u64_returns_inner_value() {
        assert_eq!(TimestampMs::new_unchecked(12345).as_u64(), 12345);
        assert_eq!(TimestampMs::new_unchecked(0).as_u64(), 0);
        assert_eq!(TimestampMs::new_unchecked(u64::MAX).as_u64(), u64::MAX);
    }

    #[test]
    fn timestamp_ms_now_produces_increasing_values() {
        let before = TimestampMs::now();
        std::thread::yield_now();
        let after = TimestampMs::now();
        assert!(after >= before);
    }

    #[test]
    fn timestamp_ms_debug_format() {
        let ts = TimestampMs::new_unchecked(123);
        let debug_str = format!("{:?}", ts);
        assert!(debug_str.contains("123"));
    }
}

#[cfg(test)]
mod vo_error_from_tests {
    use super::*;

    #[test]
    fn vo_error_from_io_error_not_found() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let vo_err: VoError = io_err.into();
        match vo_err {
            VoError::Internal(msg) => {
                assert!(!msg.is_empty());
            }
            _ => panic!("expected Internal variant"),
        }
    }

    #[test]
    fn vo_error_from_io_error_permission_denied() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let vo_err: VoError = io_err.into();
        match vo_err {
            VoError::Internal(msg) => {
                assert!(!msg.is_empty());
            }
            _ => panic!("expected Internal variant"),
        }
    }

    #[test]
    fn vo_error_from_io_error_eof() {
        let io_err = std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "truncated");
        let vo_err: VoError = io_err.into();
        match vo_err {
            VoError::Internal(msg) => {
                assert!(msg.contains("truncated") || msg.contains("UnexpectedEof"));
            }
            _ => panic!("expected Internal variant"),
        }
    }

    #[test]
    fn vo_error_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let vo_err: VoError = json_err.into();
        match vo_err {
            VoError::Validation(msg) => {
                assert!(msg.contains("invalid json") || msg.contains("parse"));
            }
            _ => panic!("expected Validation variant"),
        }
    }

    #[test]
    fn vo_error_from_serde_json_error_type_mismatch() {
        let json_err = serde_json::from_str::<serde_json::Value>("\"not an object\"").unwrap_err();
        let vo_err: VoError = json_err.into();
        match vo_err {
            VoError::Validation(_) => {}
            _ => panic!("expected Validation variant"),
        }
    }
}

#[cfg(test)]
mod workflow_event_public_tests {
    use super::*;
    use vo_common::events::{EventDedup, DuplicateResult};

    fn make_event(event_id: &str, timer_id: &str, timestamp_ms: u64) -> WorkflowEvent {
        WorkflowEvent::TimerFired {
            event_id: event_id.into(),
            timer_id: timer_id.into(),
            timestamp_ms,
        }
    }

    #[test]
    fn workflow_event_timer_fired_all_fields() {
        let event = make_event("evt-1", "timer-abc", 1234567890);
        match event {
            WorkflowEvent::TimerFired {
                event_id,
                timer_id,
                timestamp_ms,
            } => {
                assert_eq!(event_id, "evt-1");
                assert_eq!(timer_id, "timer-abc");
                assert_eq!(timestamp_ms, 1234567890);
            }
        }
    }

    #[test]
    fn workflow_event_json_roundtrip() {
        let event = make_event("evt-rt", "timer-rt", 9876543210);
        let json = serde_json::to_string(&event).expect("should serialize");
        let deserialized: WorkflowEvent = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(event, deserialized);
    }

    #[test]
    fn event_dedup_check_and_track_new() {
        let mut dedup = EventDedup::new();
        assert_eq!(dedup.check_and_track("evt-new".into()), DuplicateResult::New);
        assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn event_dedup_check_and_track_duplicate() {
        let mut dedup = EventDedup::new();
        dedup.check_and_track("evt-dup".into());
        let result = dedup.check_and_track("evt-dup".into());
        assert_eq!(result, DuplicateResult::Duplicate("evt-dup".into()));
        assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn event_dedup_is_duplicate() {
        let mut dedup = EventDedup::new();
        dedup.track("evt-tracked".into());
        assert!(dedup.is_duplicate(&"evt-tracked".into()));
        assert!(!dedup.is_duplicate(&"evt-other".into()));
    }

    #[test]
    fn event_dedup_empty_initial_state() {
        let dedup = EventDedup::new();
        assert!(dedup.is_empty());
        assert_eq!(dedup.len(), 0);
    }

    #[test]
    fn event_dedup_track_returns_false_for_duplicate() {
        let mut dedup = EventDedup::new();
        assert!(dedup.track("evt-x".into()));
        assert!(!dedup.track("evt-x".into()));
    }

    #[test]
    fn event_dedup_len_after_multiple_inserts() {
        let mut dedup = EventDedup::new();
        dedup.track("a".into());
        dedup.track("b".into());
        dedup.track("c".into());
        assert_eq!(dedup.len(), 3);
    }
}
