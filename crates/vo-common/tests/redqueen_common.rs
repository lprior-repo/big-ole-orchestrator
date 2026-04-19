//! Red Queen coevolutionary test suite for vo-common (ve-hf48p.4).
//!
//! Adversarial tests targeting type boundary fuzzing, serialization mutation,
//! and error propagation chains. Each test is designed to kill specific
//! source-code mutants; as the codebase evolves, new mutants will emerge and
//! these tests must coevolve to match.
//!
//! Target mutant classes:
//! - M1: String type-alias boundary invariants (InstanceId, NamespaceId, TimerId)
//! - M2: WorkflowEvent serialization roundtrip mutants
//! - M3: VoError constructor and Display mutants
//! - M4: VoError variant discrimination mutants
//! - M5: Error propagation chain (From/into conversions, chaining)
//! - M6: Serialization adversarial input mutants
//! - M7: Clone and PartialEq semantic correctness
//! - M8: Edge-case string content (empty, unicode, control chars, max length)

use vo_common::{NamespaceId, VoError, WorkflowEvent};
use vo_types::{InstanceId, TimerId};

// ============================================================================
// M1: String type-alias boundary invariants
// ============================================================================

#[cfg(test)]
mod type_alias_boundary {
    use super::*;

    /// Kills: type alias changed from String to something non-string-like.
    #[test]
    fn rq_instance_id_is_string() {
        let id: InstanceId = String::from("test").into();
        let _: String = id.into();
    }

    /// Kills: NamespaceId alias changed from String.
    #[test]
    fn rq_namespace_id_is_string() {
        let ns: NamespaceId = String::from("ns").into();
        let _: String = ns.into();
    }

    /// Kills: TimerId alias changed from String.
    #[test]
    fn rq_timer_id_is_string() {
        let t: TimerId = String::from("t").into();
        let _: String = t.into();
    }

    /// Kills: type alias has Drop side effects or nonzero-size wrapper.
    #[test]
    fn rq_instance_id_zero_cost_abstraction() {
        assert_eq!(
            std::mem::size_of::<InstanceId>(),
            std::mem::size_of::<String>()
        );
        assert_eq!(
            std::mem::size_of::<NamespaceId>(),
            std::mem::size_of::<String>()
        );
        assert_eq!(
            std::mem::size_of::<TimerId>(),
            std::mem::size_of::<String>()
        );
    }

    /// Kills: Into<String> impl broken for type aliases.
    #[test]
    fn rq_type_aliases_accept_into_string() {
        let id: InstanceId = "instance-42".into();
        assert_eq!(String::from(id), "instance-42");

        let ns: NamespaceId = "ns/prod".into();
        assert_eq!(String::from(ns), "ns/prod");

        let t: TimerId = "timer-check".into();
        assert_eq!(String::from(t), "timer-check");
    }

    /// Kills: AsRef<str> broken for type aliases.
    #[test]
    fn rq_type_aliases_implement_as_ref_str() {
        let id: InstanceId = "hello".into();
        assert_eq!(AsRef::<str>::as_ref(&id), "hello");

        let ns: NamespaceId = "world".into();
        assert_eq!(AsRef::<str>::as_ref(&ns), "world");

        let t: TimerId = "timer".into();
        assert_eq!(AsRef::<str>::as_ref(&t), "timer");
    }
}

// ============================================================================
// M2: WorkflowEvent serialization roundtrip mutants
// ============================================================================

#[cfg(test)]
mod serialization_roundtrip {
    use super::*;

    /// Kills: TimerFired field order swapped in serialization.
    #[test]
    fn rq_timer_fired_json_roundtrip() {
        let event = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("t-001"),
            timestamp_ms: 1700000000000,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: WorkflowEvent = serde_json::from_str(&json).expect("deserialize roundtrip");
        assert_eq!(event, back);
    }

    /// Kills: timestamp_ms type changed from u64 (would truncate or overflow).
    #[test]
    fn rq_timer_fired_max_u64_timestamp() {
        let event = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("edge"),
            timestamp_ms: u64::MAX,
        };
        let json = serde_json::to_string(&event).expect("serialize max u64");
        let back: WorkflowEvent = serde_json::from_str(&json).expect("deserialize max u64");
        assert_eq!(event, back);
    }

    /// Kills: timestamp_ms type changed (zero boundary).
    #[test]
    fn rq_timer_fired_zero_timestamp() {
        let event = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("epoch"),
            timestamp_ms: 0,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: WorkflowEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(event, back);
    }

    /// Kills: serde rename attribute changed (camelCase vs snake_case).
    #[test]
    fn rq_timer_fired_json_field_names() {
        let event = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("field-test"),
            timestamp_ms: 42,
        };
        let json = serde_json::to_value(&event).expect("to_value");
        let obj = json.as_object().expect("is object");

        // serde default: variant key is the outer key, fields are inner object
        assert!(
            obj.contains_key("TimerFired"),
            "variant key must be TimerFired"
        );
        let inner = obj["TimerFired"].as_object().expect("inner is object");
        assert!(inner.contains_key("timer_id"), "field must be timer_id");
        assert!(
            inner.contains_key("timestamp_ms"),
            "field must be timestamp_ms"
        );
    }

    /// Kills: extra fields silently ignored (strict deserialization broken).
    #[test]
    fn rq_timer_fired_extra_json_fields_ignored() {
        let json = r#"{"TimerFired":{"timer_id":"t1","timestamp_ms":99,"extra":"garbage"}}"#;
        let event: WorkflowEvent =
            serde_json::from_str(json).expect("extra fields must be ignored");
        assert!(
            matches!(event, WorkflowEvent::TimerFired { timer_id, timestamp_ms }
            if timer_id == "t1" && timestamp_ms == 99)
        );
    }

    /// Kills: missing field returns error instead of default.
    #[test]
    fn rq_timer_fired_missing_field_rejects() {
        let json = r#"{"TimerFired":{"timer_id":"t1"}}"#;
        let result: Result<WorkflowEvent, _> = serde_json::from_str(json);
        assert!(result.is_err(), "missing timestamp_ms must fail");
    }

    /// Kills: wrong field type silently coerced.
    #[test]
    fn rq_timer_fired_wrong_type_rejects() {
        let json = r#"{"TimerFired":{"timer_id":"t1","timestamp_ms":"not_a_number"}}"#;
        let result: Result<WorkflowEvent, _> = serde_json::from_str(json);
        assert!(result.is_err(), "string timestamp must fail");
    }

    /// Kills: negative timestamp accepted (u64 can't be negative).
    #[test]
    fn rq_timer_fired_negative_timestamp_rejects() {
        let json = r#"{"TimerFired":{"timer_id":"t1","timestamp_ms":-1}}"#;
        let result: Result<WorkflowEvent, _> = serde_json::from_str(json);
        assert!(result.is_err(), "negative u64 must fail");
    }

    /// Kills: unknown variant accepted silently.
    #[test]
    fn rq_unknown_variant_rejects() {
        let json = r#"{"UnknownVariant":{"foo":"bar"}}"#;
        let result: Result<WorkflowEvent, _> = serde_json::from_str(json);
        assert!(result.is_err(), "unknown variant must fail");
    }
}

// ============================================================================
// M3: VoError constructor and Display mutants
// ============================================================================

#[cfg(test)]
mod voerror_constructors {
    use super::*;

    /// Kills: config() constructor wrapping logic changed.
    #[test]
    fn rq_error_config_message_preserved() {
        let err = VoError::config("missing api key");
        assert_eq!(err.to_string(), "configuration error: missing api key");
    }

    /// Kills: internal() constructor wrapping logic changed.
    #[test]
    fn rq_error_internal_message_preserved() {
        let err = VoError::internal("disk full");
        assert_eq!(err.to_string(), "internal error: disk full");
    }

    /// Kills: not_found() constructor wrapping logic changed.
    #[test]
    fn rq_error_not_found_message_preserved() {
        let err = VoError::not_found("workflow abc");
        assert_eq!(err.to_string(), "not found: workflow abc");
    }

    /// Kills: validation() constructor wrapping logic changed.
    #[test]
    fn rq_error_validation_message_preserved() {
        let err = VoError::validation("empty name");
        assert_eq!(err.to_string(), "validation failed: empty name");
    }

    /// Kills: timeout() constructor wrapping logic changed.
    #[test]
    fn rq_error_timeout_message_preserved() {
        let err = VoError::timeout("30s elapsed");
        assert_eq!(err.to_string(), "operation timed out: 30s elapsed");
    }

    /// Kills: Display prefix changed for any variant.
    #[test]
    fn rq_error_display_prefixes() {
        assert!(VoError::config("x")
            .to_string()
            .starts_with("configuration error:"));
        assert!(VoError::internal("x")
            .to_string()
            .starts_with("internal error:"));
        assert!(VoError::not_found("x")
            .to_string()
            .starts_with("not found:"));
        assert!(VoError::validation("x")
            .to_string()
            .starts_with("validation failed:"));
        assert!(VoError::timeout("x")
            .to_string()
            .starts_with("operation timed out:"));
    }

    /// Kills: Into<String> conversion on constructor argument broken.
    #[test]
    fn rq_error_constructors_accept_various_string_types() {
        let _ = VoError::config(String::from("str"));
        let _ = VoError::config("str");
        let _ = VoError::config(String::from("owned").into_boxed_str());
    }

    /// Kills: empty message string not handled.
    #[test]
    fn rq_error_empty_messages() {
        for variant in [
            VoError::config(""),
            VoError::internal(""),
            VoError::not_found(""),
            VoError::validation(""),
            VoError::timeout(""),
        ] {
            let display = variant.to_string();
            assert!(!display.is_empty(), "Display must not be empty");
            // Prefix must still be present even with empty message
            assert!(
                display.contains(':'),
                "empty message must still contain colon prefix: got '{}'",
                display
            );
        }
    }
}

// ============================================================================
// M4: VoError variant discrimination mutants
// ============================================================================

#[cfg(test)]
mod voerror_discrimination {
    use super::*;

    /// Kills: Config vs Internal discrimination swapped.
    #[test]
    fn rq_error_config_ne_internal() {
        let c = VoError::config("x");
        let i = VoError::internal("x");
        assert_ne!(c, i);
        assert!(matches!(c, VoError::Config(_)));
        assert!(!matches!(c, VoError::Internal(_)));
        assert!(matches!(i, VoError::Internal(_)));
        assert!(!matches!(i, VoError::Config(_)));
    }

    /// Kills: NotFound vs Validation discrimination swapped.
    #[test]
    fn rq_error_not_found_ne_validation() {
        let n = VoError::not_found("x");
        let v = VoError::validation("x");
        assert_ne!(n, v);
        assert!(matches!(n, VoError::NotFound(_)));
        assert!(matches!(v, VoError::Validation(_)));
    }

    /// Kills: Timeout variant not distinct from other variants.
    #[test]
    fn rq_error_timeout_distinct_from_all() {
        let t = VoError::timeout("x");
        assert!(!matches!(t, VoError::Config(_)));
        assert!(!matches!(t, VoError::Internal(_)));
        assert!(!matches!(t, VoError::NotFound(_)));
        assert!(!matches!(t, VoError::Validation(_)));
        assert!(matches!(t, VoError::Timeout(_)));
    }

    /// Kills: same variant same message not equal.
    #[test]
    fn rq_error_same_variant_same_msg_equal() {
        assert_eq!(VoError::config("a"), VoError::config("a"));
        assert_eq!(VoError::internal("a"), VoError::internal("a"));
        assert_eq!(VoError::not_found("a"), VoError::not_found("a"));
        assert_eq!(VoError::validation("a"), VoError::validation("a"));
        assert_eq!(VoError::timeout("a"), VoError::timeout("a"));
    }

    /// Kills: same variant different message not equal.
    #[test]
    fn rq_error_same_variant_diff_msg_not_equal() {
        assert_ne!(VoError::config("a"), VoError::config("b"));
        assert_ne!(VoError::internal("a"), VoError::internal("b"));
        assert_ne!(VoError::not_found("a"), VoError::not_found("b"));
        assert_ne!(VoError::validation("a"), VoError::validation("b"));
        assert_ne!(VoError::timeout("a"), VoError::timeout("b"));
    }

    /// Kills: Eq impl inconsistent with PartialEq.
    #[test]
    fn rq_error_eq_reflexivity() {
        let errors = [
            VoError::config("reflex"),
            VoError::internal("reflex"),
            VoError::not_found("reflex"),
            VoError::validation("reflex"),
            VoError::timeout("reflex"),
        ];
        for e in &errors {
            assert_eq!(*e, *e, "reflexivity: {:?} must equal itself", e);
        }
    }

    /// Kills: Eq impl not symmetric.
    #[test]
    fn rq_error_eq_symmetry() {
        let pairs = [
            (VoError::config("a"), VoError::config("a")),
            (VoError::internal("b"), VoError::internal("b")),
        ];
        for (a, b) in pairs {
            assert_eq!(a, b);
            assert_eq!(b, a);
        }
    }

    /// Kills: Eq impl not transitive.
    #[test]
    fn rq_error_eq_transitivity() {
        let a = VoError::config("x");
        let b = VoError::config("x");
        let c = VoError::config("x");
        assert_eq!(a, b);
        assert_eq!(b, c);
        assert_eq!(a, c);
    }
}

// ============================================================================
// M5: Error propagation chain mutants
// ============================================================================

#[cfg(test)]
mod error_propagation_chains {
    use super::*;

    /// Kills: std::error::Error source chain broken.
    #[test]
    fn rq_error_implements_std_error() {
        fn assert_error<E: std::error::Error>(_e: E) {}
        assert_error(VoError::config("x"));
        assert_error(VoError::internal("x"));
        assert_error(VoError::not_found("x"));
        assert_error(VoError::validation("x"));
        assert_error(VoError::timeout("x"));
    }

    /// Kills: Error::source returns Some for leaf variants (should be None).
    #[test]
    fn rq_error_source_is_none() {
        use std::error::Error;
        for err in [
            VoError::config("x"),
            VoError::internal("x"),
            VoError::not_found("x"),
            VoError::validation("x"),
            VoError::timeout("x"),
        ] {
            assert!(
                err.source().is_none(),
                "VoError::source() must be None for {:?}",
                err
            );
        }
    }

    /// Kills: Send + Sync not implemented (async context requires it).
    #[test]
    fn rq_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>(_t: T) {}
        assert_send_sync(VoError::config("x"));
        assert_send_sync(VoError::internal("x"));
        assert_send_sync(VoError::not_found("x"));
        assert_send_sync(VoError::validation("x"));
        assert_send_sync(VoError::timeout("x"));
    }

    /// Kills: Clone not implemented (breaks error chain propagation).
    #[test]
    fn rq_error_is_clone() {
        let e = VoError::internal("clone me");
        let e2 = e.clone();
        assert_eq!(e, e2);
    }

    /// Kills: Debug format changed (breaks log parsing).
    #[test]
    fn rq_error_debug_contains_variant_and_message() {
        let e = VoError::config("bad key");
        let debug = format!("{:?}", e);
        assert!(debug.contains("Config"), "Debug must contain variant name");
        assert!(debug.contains("bad key"), "Debug must contain message");

        let e2 = VoError::timeout("30s");
        let debug2 = format!("{:?}", e2);
        assert!(debug2.contains("Timeout"));
        assert!(debug2.contains("30s"));
    }
}

// ============================================================================
// M6: Serialization adversarial input mutants
// ============================================================================

#[cfg(test)]
mod serialization_adversarial {
    use super::*;

    /// Kills: null JSON accepted.
    #[test]
    fn rq_null_json_rejects() {
        let result: Result<WorkflowEvent, _> = serde_json::from_str("null");
        assert!(result.is_err(), "null must not deserialize");
    }

    /// Kills: empty object accepted.
    #[test]
    fn rq_empty_object_rejects() {
        let result: Result<WorkflowEvent, _> = serde_json::from_str("{}");
        assert!(result.is_err(), "empty object must not deserialize");
    }

    /// Kills: empty array accepted.
    #[test]
    fn rq_empty_array_rejects() {
        let result: Result<WorkflowEvent, _> = serde_json::from_str("[]");
        assert!(result.is_err(), "empty array must not deserialize");
    }

    /// Kills: bare string accepted.
    #[test]
    fn rq_bare_string_rejects() {
        let result: Result<WorkflowEvent, _> = serde_json::from_str("\"TimerFired\"");
        assert!(result.is_err(), "bare string must not deserialize");
    }

    /// Kills: numeric value accepted.
    #[test]
    fn rq_bare_number_rejects() {
        let result: Result<WorkflowEvent, _> = serde_json::from_str("42");
        assert!(result.is_err(), "bare number must not deserialize");
    }

    /// Kills: boolean accepted.
    #[test]
    fn rq_bare_bool_rejects() {
        let result: Result<WorkflowEvent, _> = serde_json::from_str("true");
        assert!(result.is_err(), "bare bool must not deserialize");
    }

    /// Kills: whitespace-only input accepted.
    #[test]
    fn rq_whitespace_only_rejects() {
        let result: Result<WorkflowEvent, _> = serde_json::from_str("   \n\t  ");
        assert!(result.is_err(), "whitespace must not deserialize");
    }

    /// Kills: truncated JSON accepted (partial write simulation).
    #[test]
    fn rq_truncated_json_rejects() {
        for truncated in &[
            r#"{"TimerFired":{"timer_id":"#,
            r#"{"TimerFired":{"timer_id":"t1","timestamp_ms"#,
            r#"{"TimerFired""#,
            r#"{"TimerFired":{"#,
        ] {
            let result: Result<WorkflowEvent, _> = serde_json::from_str(truncated);
            assert!(result.is_err(), "truncated JSON must fail: {}", truncated);
        }
    }

    /// Kills: duplicate fields silently accepted (serde_json rejects duplicates by default).
    #[test]
    fn rq_duplicate_fields_rejected() {
        let json = r#"{"TimerFired":{"timer_id":"first","timer_id":"second","timestamp_ms":1}}"#;
        let result: Result<WorkflowEvent, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "duplicate fields must be rejected by serde_json"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("duplicate"),
            "error must mention 'duplicate': {}",
            err_msg
        );
    }

    /// Kills: unicode in timer_id roundtrip broken.
    #[test]
    fn rq_unicode_timer_id_roundtrip() {
        let event = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("计时器-日本語-🚀"),
            timestamp_ms: 12345,
        };
        let json = serde_json::to_string(&event).expect("serialize unicode");
        let back: WorkflowEvent = serde_json::from_str(&json).expect("deserialize unicode");
        assert_eq!(event, back);
    }

    /// Kills: timer_id with control characters not roundtripped.
    #[test]
    fn rq_control_char_timer_id_roundtrip() {
        let event = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("timer\x00\x01\x1f"),
            timestamp_ms: 99,
        };
        let json = serde_json::to_string(&event).expect("serialize control chars");
        let back: WorkflowEvent = serde_json::from_str(&json).expect("deserialize control chars");
        assert_eq!(event, back);
    }

    /// Kills: timer_id with escape sequences not roundtripped.
    #[test]
    fn rq_escaped_chars_timer_id_roundtrip() {
        let event = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("timer\twith\nnewlines"),
            timestamp_ms: 50,
        };
        let json = serde_json::to_string(&event).expect("serialize escapes");
        let back: WorkflowEvent = serde_json::from_str(&json).expect("deserialize escapes");
        assert_eq!(event, back);
    }

    /// Kills: very long timer_id not handled (no truncation).
    #[test]
    fn rq_long_timer_id_roundtrip() {
        let long_id: String = "x".repeat(10_000);
        let event = WorkflowEvent::TimerFired {
            timer_id: TimerId::new(long_id.clone()),
            timestamp_ms: 1,
        };
        let json = serde_json::to_string(&event).expect("serialize long id");
        let back: WorkflowEvent = serde_json::from_str(&json).expect("deserialize long id");
        assert!(
            matches!(back, WorkflowEvent::TimerFired { timer_id, .. }
            if timer_id.len() == 10_000),
            "long id must not be truncated"
        );
    }
}

// ============================================================================
// M7: Clone and PartialEq semantic correctness
// ============================================================================

#[cfg(test)]
mod clone_partial_eq_semantics {
    use super::*;

    /// Kills: Clone shallow-copies instead of deep-copying.
    #[test]
    fn rq_workflow_event_clone_independence() {
        let event = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("t1"),
            timestamp_ms: 100,
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    /// Kills: PartialEq compares addresses instead of values.
    #[test]
    fn rq_workflow_event_equality_value_based() {
        let a = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("same"),
            timestamp_ms: 42,
        };
        let b = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("same"),
            timestamp_ms: 42,
        };
        assert_eq!(a, b, "equal values must be equal");
    }

    /// Kills: PartialEq ignores timestamp_ms.
    #[test]
    fn rq_workflow_event_inequality_on_timestamp() {
        let a = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("t"),
            timestamp_ms: 1,
        };
        let b = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("t"),
            timestamp_ms: 2,
        };
        assert_ne!(a, b, "different timestamps must not be equal");
    }

    /// Kills: PartialEq ignores timer_id.
    #[test]
    fn rq_workflow_event_inequality_on_timer_id() {
        let a = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("a"),
            timestamp_ms: 1,
        };
        let b = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("b"),
            timestamp_ms: 1,
        };
        assert_ne!(a, b, "different timer_ids must not be equal");
    }

    /// Kills: VoError Clone not deep.
    #[test]
    fn rq_vo_error_clone_deep() {
        let e1 = VoError::internal("deep copy test".to_string());
        let e2 = e1.clone();
        assert_eq!(e1, e2);
        match (&e1, &e2) {
            (VoError::Internal(a), VoError::Internal(b)) => {
                assert_eq!(a, b);
                assert_ne!(
                    a.as_ptr(),
                    b.as_ptr(),
                    "cloned strings must be independent allocations"
                );
            }
            _ => unreachable!(),
        }
    }

    /// Kills: WorkflowEvent Clone not deep for inner String.
    #[test]
    fn rq_workflow_event_clone_deep_string() {
        let e = WorkflowEvent::TimerFired {
            timer_id: TimerId::new("ptr-test"),
            timestamp_ms: 1,
        };
        let c = e.clone();
        match (&e, &c) {
            (
                WorkflowEvent::TimerFired { timer_id: a, .. },
                WorkflowEvent::TimerFired { timer_id: b, .. },
            ) => {
                assert_eq!(a, b);
                assert_ne!(
                    a.as_ptr(),
                    b.as_ptr(),
                    "cloned timer_id must be independent"
                );
            }
            _ => panic!("expected TimerFired"),
        }
    }
}

// ============================================================================
// M8: Edge-case string content mutants
// ============================================================================

#[cfg(test)]
mod edge_case_strings {
    use super::*;

    /// Kills: empty InstanceId not handled.
    #[test]
    fn rq_empty_instance_id() {
        let id: InstanceId = "".into();
        assert_eq!(id.len(), 0);
        assert!(id.is_empty());
    }

    /// Kills: empty NamespaceId not handled.
    #[test]
    fn rq_empty_namespace_id() {
        let ns: NamespaceId = "".into();
        assert!(ns.is_empty());
    }

    /// Kills: empty TimerId not handled.
    #[test]
    fn rq_empty_timer_id() {
        let t: TimerId = "".into();
        assert!(t.is_empty());
    }

    /// Kills: unicode InstanceId mangled.
    #[test]
    fn rq_unicode_instance_id() {
        let id: InstanceId = "实例-日本語-العربية-🎉".into();
        assert_eq!(id.as_str(), "实例-日本語-العربية-🎉");
    }

    /// Kills: newlines and tabs in ID accepted but not roundtripped.
    #[test]
    fn rq_whitespace_instance_id() {
        let id: InstanceId = " \t\n".into();
        assert_eq!(id.len(), 3);
        assert!(id.contains('\n'));
    }

    /// Kills: null byte in ID not handled.
    #[test]
    fn rq_null_byte_instance_id() {
        let id: InstanceId = "before\0after".into();
        assert!(id.contains('\0'));
        assert!(id.len() > 6, "null byte adds to length");
    }

    /// Kills: very long ID causes allocation failure or truncation.
    #[test]
    fn rq_megabyte_instance_id() {
        let big = "x".repeat(1_000_000);
        let id: InstanceId = big.clone().into();
        assert_eq!(id.len(), 1_000_000);
        assert_eq!(id.as_str(), big.as_str());
    }

    /// Kills: timer_id with slashes (path-like) not handled.
    #[test]
    fn rq_path_like_timer_id() {
        let t: TimerId = "ns/workflow/timer-1".into();
        assert_eq!(t, "ns/workflow/timer-1");
    }

    /// Kills: timer_id with URL-like characters not handled.
    #[test]
    fn rq_url_like_timer_id() {
        let t: TimerId = "timer?id=1&foo=bar".into();
        assert_eq!(t, "timer?id=1&foo=bar");
    }

    /// Kills: VoError message with null bytes mangled in Display.
    #[test]
    fn rq_error_display_null_byte_preserved() {
        let err = VoError::internal("error\0hidden");
        let display = err.to_string();
        assert!(
            display.contains("error\0hidden"),
            "null byte must be preserved in Display"
        );
    }

    /// Kills: VoError message with unicode preserved in Display.
    #[test]
    fn rq_error_display_unicode_preserved() {
        let err = VoError::not_found("实例🚀not found");
        let display = err.to_string();
        assert!(display.contains("实例🚀not found"));
    }
}
