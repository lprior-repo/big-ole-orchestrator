//! Comprehensive unit and integration tests for vo-common utility functions
//! and shared types (vel-4x6).
//!
//! This module provides exhaustive testing covering:
//! - Utility function tests (string helpers, conversion functions, validation helpers)
//! - Shared type tests (constructor validation, Display/Debug implementations, equality/ordering semantics)
//! - Edge cases (boundary values, empty inputs, unicode handling, error type conversions)

use vo_common::{
    EventId, InstanceId, NamespaceId, TimerId, VoError, WorkflowEvent, EventDedup, DuplicateResult,
    types::TimestampMs,
};

// ============================================================================
// Type Alias Edge Case Tests
// ============================================================================

mod type_alias_edge_cases {
    use super::*;

    #[test]
    fn instance_id_empty_string() {
        let id: InstanceId = "".into();
        assert_eq!(id.as_str(), "");
        assert_eq!(id.len(), 0);
    }

    #[test]
    fn instance_id_unicode_content() {
        let id: InstanceId = "实例-123-🔱".into();
        assert_eq!(id.as_str(), "实例-123-🔱");
        assert_eq!(id.len(), 15); // UTF-8 bytes: 6 + 1 + 3 + 1 + 4
    }

    #[test]
    fn instance_id_ascii_content() {
        let id: InstanceId = "abc123XYZ".into();
        assert_eq!(id.as_str(), "abc123XYZ");
        assert_eq!(id.len(), 9);
    }

    #[test]
    fn instance_id_special_chars() {
        let id: InstanceId = "id:with:colons".into();
        assert_eq!(id.as_str(), "id:with:colons");
    }

    #[test]
    fn instance_id_path_like() {
        let id: InstanceId = "/path/to/resource".into();
        assert_eq!(id.as_str(), "/path/to/resource");
    }

    #[test]
    fn instance_id_url_like() {
        let id: InstanceId = "https://example.com/id".into();
        assert_eq!(id.as_str(), "https://example.com/id");
    }

    #[test]
    fn namespace_id_empty_string() {
        let ns: NamespaceId = "".into();
        assert_eq!(ns.as_str(), "");
        assert_eq!(ns.len(), 0);
    }

    #[test]
    fn namespace_id_unicode() {
        let ns: NamespaceId = "名前空間-نیم".into();
        assert_eq!(ns.as_str(), "名前空間-نیم");
    }

    #[test]
    fn namespace_id_hierarchical() {
        let ns: NamespaceId = "org/department/team".into();
        assert_eq!(ns.as_str(), "org/department/team");
    }

    #[test]
    fn timer_id_empty_string() {
        let t: TimerId = "".into();
        assert_eq!(t.as_str(), "");
    }

    #[test]
    fn timer_id_unicode() {
        let t: TimerId = "定时器-타이머".into();
        assert_eq!(t.as_str(), "定时器-타이머");
    }

    #[test]
    fn event_id_empty_string() {
        let e: EventId = "".into();
        assert_eq!(e.as_str(), "");
    }

    #[test]
    fn event_id_unicode() {
        let e: EventId = "événement-événement".into();
        assert_eq!(e.as_str(), "événement-événement");
    }

    #[test]
    fn event_id_uuid_like() {
        let e: EventId = "550e8400-e29b-41d4-a716-446655440000".into();
        assert_eq!(e.as_str(), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn event_id_ulid_like() {
        let e: EventId = "01ARZ3NDEKTSV4RRFFQ69G5FAV".into();
        assert_eq!(e.as_str(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    }

    #[test]
    fn type_alias_long_content() {
        let content = "x".repeat(10000);
        let id: InstanceId = content.clone().into();
        assert_eq!(id.len(), 10000);
        assert_eq!(id.as_str(), content);
    }

    #[test]
    fn type_alias_control_characters() {
        let id: InstanceId = "id\twith\ncontrol\rchars".into();
        assert!(id.contains('\t'));
        assert!(id.contains('\n'));
    }

    #[test]
    fn type_alias_json_like() {
        let id: InstanceId = r#"{"type":"user","id":123}"#.into();
        assert!(id.starts_with('{'));
    }
}

// ============================================================================
// TimestampMs Edge Case Tests
// ============================================================================

mod timestamp_ms_edge_cases {
    use super::*;

    #[test]
    fn timestamp_ms_zero() {
        let ts = TimestampMs::new_unchecked(0);
        assert_eq!(ts.as_u64(), 0);
    }

    #[test]
    fn timestamp_ms_max() {
        let ts = TimestampMs::new_unchecked(u64::MAX);
        assert_eq!(ts.as_u64(), u64::MAX);
    }

    #[test]
    fn timestamp_ms_one() {
        let ts = TimestampMs::new_unchecked(1);
        assert_eq!(ts.as_u64(), 1);
    }

    #[test]
    fn timestamp_ms_u64_midpoint() {
        let ts = TimestampMs::new_unchecked(u64::MAX / 2);
        assert_eq!(ts.as_u64(), u64::MAX / 2);
    }

    #[test]
    fn timestamp_ms_now_reasonable_range() {
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let ts = TimestampMs::now();
        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let ts_val = u64::try_from(ts.as_u64()).unwrap();
        assert!(u64::try_from(before).unwrap() <= ts_val);
        assert!(ts_val <= u64::try_from(after).unwrap());
    }

    #[test]
    fn timestamp_ms_ordering_consistency() {
        let ts1 = TimestampMs::new_unchecked(100);
        let ts2 = TimestampMs::new_unchecked(200);
        let ts3 = TimestampMs::new_unchecked(100);

        assert!(ts1 < ts2);
        assert!(ts2 > ts1);
        assert_eq!(ts1, ts3);
        assert!(ts1 <= ts3);
        assert!(ts1 >= ts3);
        assert!(ts1 <= ts2);
        assert!(ts2 >= ts1);
    }

    #[test]
    fn timestamp_ms_debug_format() {
        let ts = TimestampMs::new_unchecked(42);
        let debug = format!("{:?}", ts);
        assert!(debug.contains("42"));
    }

    #[test]
    fn timestamp_ms_clone_independence() {
        let ts1 = TimestampMs::new_unchecked(999);
        let ts2 = ts1;
        assert_eq!(ts1.as_u64(), ts2.as_u64());
    }

    #[test]
    fn timestamp_ms_serde_min_value() {
        let ts = TimestampMs::new_unchecked(u64::MIN);
        let json = serde_json::to_string(&ts).unwrap();
        let deserialized: TimestampMs = serde_json::from_str(&json).unwrap();
        assert_eq!(ts.as_u64(), deserialized.as_u64());
    }

    #[test]
    fn timestamp_ms_serde_max_value() {
        let ts = TimestampMs::new_unchecked(u64::MAX);
        let json = serde_json::to_string(&ts).unwrap();
        let deserialized: TimestampMs = serde_json::from_str(&json).unwrap();
        assert_eq!(ts.as_u64(), deserialized.as_u64());
    }

    #[test]
    fn timestamp_ms_ordering_total() {
        let ts1 = TimestampMs::new_unchecked(100);
        let ts2 = TimestampMs::new_unchecked(200);
        let ts3 = TimestampMs::new_unchecked(300);

        assert!(ts1 < ts2);
        assert!(ts2 < ts3);
        assert!(ts1 < ts3);

        assert!(ts1 <= ts1);
        assert!(ts1 >= ts1);
        assert!(ts2 <= ts2);
        assert!(ts2 >= ts2);
    }

    #[test]
    fn timestamp_ms_neq_different_values() {
        let ts1 = TimestampMs::new_unchecked(100);
        let ts2 = TimestampMs::new_unchecked(200);
        assert_ne!(ts1, ts2);
    }
}

// ============================================================================
// VoError Conversion and Display Tests
// ============================================================================

mod vo_error_tests {
    use super::*;
    use std::io;

    #[test]
    fn vo_error_config_display() {
        let err = VoError::config("db connection failed");
        assert!(err.to_string().contains("configuration error"));
        assert!(err.to_string().contains("db connection failed"));
    }

    #[test]
    fn vo_error_internal_display() {
        let err = VoError::internal("assertion failed");
        assert!(err.to_string().contains("internal error"));
        assert!(err.to_string().contains("assertion failed"));
    }

    #[test]
    fn vo_error_not_found_display() {
        let err = VoError::not_found("user-123");
        assert!(err.to_string().contains("not found"));
        assert!(err.to_string().contains("user-123"));
    }

    #[test]
    fn vo_error_validation_display() {
        let err = VoError::validation("invalid email format");
        assert!(err.to_string().contains("validation failed"));
        assert!(err.to_string().contains("invalid email format"));
    }

    #[test]
    fn vo_error_timeout_display() {
        let err = VoError::timeout("30s");
        assert!(err.to_string().contains("operation timed out"));
        assert!(err.to_string().contains("30s"));
    }

    #[test]
    fn vo_error_from_io_not_found() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let vo_err: VoError = io_err.into();
        match vo_err {
            VoError::Internal(msg) => assert!(msg.contains("file not found")),
            _ => panic!("Expected VoError::Internal"),
        }
    }

    #[test]
    fn vo_error_from_io_permission_denied() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let vo_err: VoError = io_err.into();
        match vo_err {
            VoError::Internal(msg) => assert!(msg.contains("access denied")),
            _ => panic!("Expected VoError::Internal"),
        }
    }

    #[test]
    fn vo_error_from_io_other() {
        let io_err = io::Error::new(io::ErrorKind::Other, "custom error");
        let vo_err: VoError = io_err.into();
        match vo_err {
            VoError::Internal(msg) => assert!(msg.contains("custom error")),
            _ => panic!("Expected VoError::Internal"),
        }
    }

    #[test]
    fn vo_error_from_json_invalid() {
        let json_err = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let vo_err: VoError = json_err.into();
        match vo_err {
            VoError::Validation(msg) => assert!(!msg.is_empty()),
            _ => panic!("Expected VoError::Validation"),
        }
    }

    #[test]
    fn vo_error_serde_roundtrip_all_variants() {
        let variants = [
            VoError::Config("cfg".into()),
            VoError::Internal("int".into()),
            VoError::NotFound("find".into()),
            VoError::Validation("val".into()),
            VoError::Timeout("tim".into()),
        ];
        for err in variants {
            let json = serde_json::to_string(&err).unwrap();
            let deserialized: VoError = serde_json::from_str(&json).unwrap();
            assert_eq!(err, deserialized);
        }
    }

    #[test]
    fn vo_error_partial_eq() {
        assert_eq!(VoError::Config("x".into()), VoError::Config("x".into()));
        assert_ne!(VoError::Config("x".into()), VoError::Config("y".into()));
        assert_ne!(VoError::Config("x".into()), VoError::Internal("x".into()));
    }

    #[test]
    fn vo_error_clone() {
        let err = VoError::NotFound("resource".into());
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn vo_error_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<VoError>();
    }

    #[test]
    fn vo_error_string_constructors() {
        let s = String::from("test");
        assert_eq!(
            VoError::config(s.clone()).to_string(),
            "configuration error: test"
        );
        assert_eq!(
            VoError::internal(s.clone()).to_string(),
            "internal error: test"
        );
        assert_eq!(
            VoError::not_found(s.clone()).to_string(),
            "not found: test"
        );
        assert_eq!(
            VoError::validation(s.clone()).to_string(),
            "validation failed: test"
        );
        assert_eq!(
            VoError::timeout(s.clone()).to_string(),
            "operation timed out: test"
        );
    }

    #[test]
    fn vo_error_static_str_constructors() {
        assert_eq!(
            VoError::config("static").to_string(),
            "configuration error: static"
        );
        assert_eq!(
            VoError::internal("static").to_string(),
            "internal error: static"
        );
    }
}

// ============================================================================
// WorkflowEvent and EventDedup Tests
// ============================================================================

mod event_tests {
    use super::*;

    fn make_event(event_id: &str, timer_id: &str, timestamp_ms: u64) -> WorkflowEvent {
        WorkflowEvent::TimerFired {
            event_id: event_id.into(),
            timer_id: timer_id.into(),
            timestamp_ms,
        }
    }

    #[test]
    fn workflow_event_construction() {
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
        let event = make_event("evt-rt", "timer-test", 9876543210);
        let json = serde_json::to_string(&event).expect("should serialize");
        let deserialized: WorkflowEvent = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(event, deserialized);
    }

    #[test]
    fn workflow_event_json_deserialize() {
        let json = r#"{"TimerFired":{"event_id":"e1","timer_id":"t1","timestamp_ms":42}}"#;
        let event: WorkflowEvent = serde_json::from_str(json).expect("should deserialize");
        match event {
            WorkflowEvent::TimerFired {
                event_id,
                timer_id,
                timestamp_ms,
            } => {
                assert_eq!(event_id, "e1");
                assert_eq!(timer_id, "t1");
                assert_eq!(timestamp_ms, 42);
            }
        }
    }

    #[test]
    fn workflow_event_clone() {
        let event = make_event("evt-clone", "timer-clone", 1111111111);
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn workflow_event_unicode() {
        let event = WorkflowEvent::TimerFired {
            event_id: "evt-unicode".into(),
            timer_id: "计时🚀".into(),
            timestamp_ms: 1,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: WorkflowEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn workflow_event_u64_max_timestamp() {
        let event = WorkflowEvent::TimerFired {
            event_id: "evt-max".into(),
            timer_id: "t".into(),
            timestamp_ms: u64::MAX,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: WorkflowEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn workflow_event_rejects_null() {
        assert!(serde_json::from_str::<WorkflowEvent>("null").is_err());
    }

    #[test]
    fn workflow_event_rejects_unknown_variant() {
        assert!(serde_json::from_str::<WorkflowEvent>(r#"{"Unknown":{}}"#).is_err());
    }

    #[test]
    fn event_dedup_new_event() {
        let mut dedup = EventDedup::new();
        assert_eq!(dedup.check_and_track("evt-a".into()), DuplicateResult::New);
        assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn event_dedup_duplicate_detected() {
        let mut dedup = EventDedup::new();
        dedup.check_and_track("evt-x".into());
        let result = dedup.check_and_track("evt-x".into());
        assert_eq!(result, DuplicateResult::Duplicate("evt-x".into()));
        assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn event_dedup_different_events() {
        let mut dedup = EventDedup::new();
        assert_eq!(dedup.check_and_track("evt-1".into()), DuplicateResult::New);
        assert_eq!(dedup.check_and_track("evt-2".into()), DuplicateResult::New);
        assert_eq!(dedup.len(), 2);
    }

    #[test]
    fn event_dedup_empty_event_id() {
        let mut dedup = EventDedup::new();
        dedup.check_and_track("".into());
        assert_eq!(dedup.check_and_track("".into()), DuplicateResult::Duplicate("".into()));
    }

    #[test]
    fn event_dedup_is_duplicate() {
        let mut dedup = EventDedup::new();
        dedup.track("evt-pending".into());
        assert!(dedup.is_duplicate(&"evt-pending".into()));
        assert!(!dedup.is_duplicate(&"evt-other".into()));
    }

    #[test]
    fn event_dedup_empty() {
        let dedup = EventDedup::new();
        assert!(dedup.is_empty());
        assert_eq!(dedup.len(), 0);
    }

    #[test]
    fn event_dedup_track_returns_false_for_duplicate() {
        let mut dedup = EventDedup::new();
        assert!(dedup.track("evt-new".into()));
        assert!(!dedup.track("evt-new".into()));
    }

    #[test]
    fn duplicate_result_new() {
        let result = DuplicateResult::New;
        assert!(matches!(result, DuplicateResult::New));
    }

    #[test]
    fn duplicate_result_duplicate() {
        let result = DuplicateResult::Duplicate("evt-1".into());
        match result {
            DuplicateResult::Duplicate(id) => assert_eq!(id, "evt-1"),
            _ => panic!("Expected Duplicate variant"),
        }
    }
}

// ============================================================================
// Shared Type Display and Debug Tests
// ============================================================================

mod display_debug_tests {
    use super::*;

    #[test]
    fn timestamp_ms_debug() {
        let ts = TimestampMs::new_unchecked(12345);
        let debug = format!("{:?}", ts);
        assert!(debug.contains("12345") || debug.contains("TimestampMs"));
    }

    #[test]
    fn vo_error_debug_contains_variant() {
        let err = VoError::Config("test".into());
        let debug = format!("{:?}", err);
        assert!(debug.contains("Config") || debug.contains("test"));
    }

    #[test]
    fn instance_id_display() {
        let id: InstanceId = "my-instance".into();
        let display = format!("{}", id);
        assert_eq!(display, "my-instance");
    }

    #[test]
    fn namespace_id_display() {
        let ns: NamespaceId = "my-namespace".into();
        let display = format!("{}", ns);
        assert_eq!(display, "my-namespace");
    }

    #[test]
    fn timer_id_display() {
        let t: TimerId = "my-timer".into();
        let display = format!("{}", t);
        assert_eq!(display, "my-timer");
    }

    #[test]
    fn event_id_display() {
        let e: EventId = "my-event".into();
        let display = format!("{}", e);
        assert_eq!(display, "my-event");
    }

    #[test]
    fn workflow_event_debug() {
        let event = WorkflowEvent::TimerFired {
            event_id: "e1".into(),
            timer_id: "t1".into(),
            timestamp_ms: 42,
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("TimerFired") || debug.contains("e1"));
    }
}

// ============================================================================
// Equality and Ordering Semantics Tests
// ============================================================================

mod equality_ordering_tests {
    use super::*;

    #[test]
    fn instance_id_equality() {
        let a: InstanceId = "same".into();
        let b: InstanceId = "same".into();
        let c: InstanceId = "different".into();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn namespace_id_equality() {
        let a: NamespaceId = "ns-same".into();
        let b: NamespaceId = "ns-same".into();
        assert_eq!(a, b);
    }

    #[test]
    fn timer_id_equality() {
        let a: TimerId = "timer-same".into();
        let b: TimerId = "timer-same".into();
        assert_eq!(a, b);
    }

    #[test]
    fn event_id_equality() {
        let a: EventId = "evt-same".into();
        let b: EventId = "evt-same".into();
        assert_eq!(a, b);
    }

    #[test]
    fn timestamp_ms_equality() {
        let a = TimestampMs::new_unchecked(100);
        let b = TimestampMs::new_unchecked(100);
        let c = TimestampMs::new_unchecked(200);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn timestamp_ms_ord() {
        let ts1 = TimestampMs::new_unchecked(1);
        let ts2 = TimestampMs::new_unchecked(2);
        assert!(ts1 < ts2);
        assert!(ts1 <= ts2);
        assert!(ts2 > ts1);
        assert!(ts2 >= ts1);
    }

    #[test]
    fn vo_error_equality() {
        let a = VoError::Config("same".into());
        let b = VoError::Config("same".into());
        let c = VoError::Config("different".into());
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, VoError::Internal("same".into()));
    }

    #[test]
    fn workflow_event_equality() {
        let a = WorkflowEvent::TimerFired {
            event_id: "e1".into(),
            timer_id: "t1".into(),
            timestamp_ms: 100,
        };
        let b = WorkflowEvent::TimerFired {
            event_id: "e1".into(),
            timer_id: "t1".into(),
            timestamp_ms: 100,
        };
        let c = WorkflowEvent::TimerFired {
            event_id: "e2".into(),
            timer_id: "t1".into(),
            timestamp_ms: 100,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}

// ============================================================================
// Boundary Value and Extreme Input Tests
// ============================================================================

mod boundary_tests {
    use super::*;

    #[test]
    fn type_alias_boundary_empty() {
        let id: InstanceId = "".into();
        assert_eq!(id.len(), 0);
        assert!(id.is_empty());
    }

    #[test]
    fn type_alias_boundary_single_char() {
        let id: InstanceId = "x".into();
        assert_eq!(id.len(), 1);
        assert_eq!(id.as_str(), "x");
    }

    #[test]
    fn timestamp_ms_boundary_u64_min() {
        let ts = TimestampMs::new_unchecked(u64::MIN);
        assert_eq!(ts.as_u64(), 0);
    }

    #[test]
    fn timestamp_ms_boundary_u64_max() {
        let ts = TimestampMs::new_unchecked(u64::MAX);
        assert_eq!(ts.as_u64(), u64::MAX);
    }

    #[test]
    fn workflow_event_boundary_timestamp_zero() {
        let event = WorkflowEvent::TimerFired {
            event_id: "e".into(),
            timer_id: "t".into(),
            timestamp_ms: 0,
        };
        match event {
            WorkflowEvent::TimerFired { timestamp_ms, .. } => assert_eq!(timestamp_ms, 0),
        }
    }

    #[test]
    fn workflow_event_boundary_timestamp_max() {
        let event = WorkflowEvent::TimerFired {
            event_id: "e".into(),
            timer_id: "t".into(),
            timestamp_ms: u64::MAX,
        };
        match event {
            WorkflowEvent::TimerFired { timestamp_ms, .. } => assert_eq!(timestamp_ms, u64::MAX),
        }
    }

    #[test]
    fn event_dedup_many_unique_events() {
        let mut dedup = EventDedup::new();
        for i in 0..1000 {
            let result = dedup.check_and_track(format!("evt-{}", i).into());
            assert!(matches!(result, DuplicateResult::New));
        }
        assert_eq!(dedup.len(), 1000);
    }

    #[test]
    fn event_dedup_all_duplicates_after_first() {
        let mut dedup = EventDedup::new();
        dedup.track("evt-1".into());
        for _ in 0..100 {
            let result = dedup.check_and_track("evt-1".into());
            assert!(matches!(result, DuplicateResult::Duplicate(_)));
        }
        assert_eq!(dedup.len(), 1);
    }

    #[test]
    fn mixed_ascii_unicode() {
        let id: InstanceId = "ascii-日本語-العربية-emoji🚀".into();
        assert!(id.len() > 20);
        assert!(id.contains("ascii"));
        assert!(id.contains("日本語"));
    }

    #[test]
    fn very_long_single_character() {
        let content = "界".repeat(10000);
        let id: InstanceId = content.clone().into();
        assert_eq!(id.len(), 30000); // 3 bytes per Chinese char
        let s: String = id;
        assert_eq!(s, content);
    }
}

// ============================================================================
// Error Type Conversion Tests
// ============================================================================

mod error_conversion_tests {
    use super::*;
    use std::io;

    #[test]
    fn vo_error_from_io_kind_not_found() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
        let vo_err: VoError = io_err.into();
        assert!(matches!(vo_err, VoError::Internal(_)));
    }

    #[test]
    fn vo_error_from_io_kind_permission_denied() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "test");
        let vo_err: VoError = io_err.into();
        assert!(matches!(vo_err, VoError::Internal(_)));
    }

    #[test]
    fn vo_error_from_io_kind_connection_refused() {
        let io_err = io::Error::new(io::ErrorKind::ConnectionRefused, "test");
        let vo_err: VoError = io_err.into();
        assert!(matches!(vo_err, VoError::Internal(_)));
    }

    #[test]
    fn vo_error_from_io_kind_timed_out() {
        let io_err = io::Error::new(io::ErrorKind::TimedOut, "test");
        let vo_err: VoError = io_err.into();
        assert!(matches!(vo_err, VoError::Internal(_)));
    }

    #[test]
    fn vo_error_from_io_kind_other() {
        let io_err = io::Error::new(io::ErrorKind::Other, "custom");
        let vo_err: VoError = io_err.into();
        assert!(matches!(vo_err, VoError::Internal(_)));
    }

    #[test]
    fn vo_error_from_json_parse_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err();
        let vo_err: VoError = json_err.into();
        assert!(matches!(vo_err, VoError::Validation(_)));
    }

    #[test]
    fn vo_error_from_json_missing_field() {
        let json_err = serde_json::from_str::<WorkflowEvent>(r#"{"TimerFired":{"event_id":"e1","timer_id":"t1"}}"#).unwrap_err();
        let vo_err: VoError = json_err.into();
        assert!(matches!(vo_err, VoError::Validation(_)));
    }

    #[test]
    fn vo_error_chain_source() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file.txt");
        let vo_err: VoError = io_err.into();
        let display = format!("{}", vo_err);
        assert!(display.contains("file.txt") || display.contains("NotFound"));
    }
}

// ============================================================================
// Constructor Validation Tests
// ============================================================================

mod constructor_validation_tests {
    use super::*;

    #[test]
    fn timestamp_ms_new_unchecked_accepts_any_u64() {
        let _ = TimestampMs::new_unchecked(0);
        let _ = TimestampMs::new_unchecked(1);
        let _ = TimestampMs::new_unchecked(u64::MAX);
        let _ = TimestampMs::new_unchecked(u64::MAX / 2);
    }

    #[test]
    fn instance_id_from_string() {
        let s = String::from("test-instance");
        let id: InstanceId = s.into();
        assert_eq!(id.as_str(), "test-instance");
    }

    #[test]
    fn namespace_id_from_string() {
        let s = String::from("test-namespace");
        let id: NamespaceId = s.into();
        assert_eq!(id.as_str(), "test-namespace");
    }

    #[test]
    fn timer_id_from_string() {
        let s = String::from("test-timer");
        let id: TimerId = s.into();
        assert_eq!(id.as_str(), "test-timer");
    }

    #[test]
    fn event_id_from_string() {
        let s = String::from("test-event");
        let id: EventId = s.into();
        assert_eq!(id.as_str(), "test-event");
    }

    #[test]
    fn event_dedup_new_default_empty() {
        let dedup = EventDedup::new();
        assert!(dedup.is_empty());
        assert_eq!(dedup.len(), 0);
    }

    #[test]
    fn workflow_event_variant_fields() {
        let event = WorkflowEvent::TimerFired {
            event_id: "e1".into(),
            timer_id: "t1".into(),
            timestamp_ms: 42,
        };
        let WorkflowEvent::TimerFired {
            event_id,
            timer_id,
            timestamp_ms,
        } = event;
        assert_eq!(event_id, "e1");
        assert_eq!(timer_id, "t1");
        assert_eq!(timestamp_ms, 42);
    }
}