//! Black-hat adversarial tests for vo-common types (ve-hf48p.3).

use vo_common::{InstanceId, NamespaceId, TimerId, VoError, WorkflowEvent};

#[cfg(test)]
mod deser_attacks {
    use super::*;
    use serde_json::json;

    #[test]
    fn reject_missing_fields() {
        assert!(
            serde_json::from_value::<WorkflowEvent>(json!({"TimerFired": {"timer_id": "x"}}))
                .is_err()
        );
    }

    #[test]
    fn reject_missing_task_result() {
        assert!(serde_json::from_value::<WorkflowEvent>(
            json!({"TaskCompleted": {"task_id": "x"}})
        )
        .is_err());
    }

    #[test]
    fn reject_missing_task_error() {
        assert!(
            serde_json::from_value::<WorkflowEvent>(json!({"TaskFailed": {"task_id": "x"}}))
                .is_err()
        );
    }

    #[test]
    fn reject_missing_signal_name() {
        assert!(serde_json::from_value::<WorkflowEvent>(
            json!({"SignalReceived": {"payload_json": "x"}})
        )
        .is_err());
    }

    #[test]
    fn reject_missing_workflow_id_started() {
        assert!(serde_json::from_value::<WorkflowEvent>(
            json!({"WorkflowStarted": {"input_json": "x"}})
        )
        .is_err());
    }

    #[test]
    fn reject_missing_workflow_id_completed() {
        assert!(serde_json::from_value::<WorkflowEvent>(
            json!({"WorkflowCompleted": {"result_json": "x"}})
        )
        .is_err());
    }

    #[test]
    fn extra_fields_silently_accepted() {
        let r: WorkflowEvent = serde_json::from_value(json!({
            "TimerFired": {"timer_id": "t1", "timestamp_ms": 100, "_poison": "evil"}
        }))
        .unwrap();
        if let WorkflowEvent::TimerFired {
            timer_id,
            timestamp_ms,
        } = r
        {
            assert_eq!((timer_id, timestamp_ms), ("t1".to_string(), 100));
        } else {
            panic!("expected TimerFired");
        }
    }

    #[test]
    fn reject_unknown_variant() {
        assert!(
            serde_json::from_value::<WorkflowEvent>(json!({"UnknownVariant": {"x": 1}})).is_err()
        );
    }

    #[test]
    fn reject_null_timer_id() {
        assert!(serde_json::from_value::<WorkflowEvent>(
            json!({"TimerFired": {"timer_id": null, "timestamp_ms": 0}})
        )
        .is_err());
    }

    #[test]
    fn reject_null_task_id() {
        assert!(serde_json::from_value::<WorkflowEvent>(
            json!({"TaskCompleted": {"task_id": null, "result_json": "x"}})
        )
        .is_err());
    }

    #[test]
    fn reject_array_payload() {
        assert!(serde_json::from_value::<WorkflowEvent>(json!({"TimerFired": [1, 2, 3]})).is_err());
    }

    #[test]
    fn reject_string_timestamp() {
        assert!(serde_json::from_value::<WorkflowEvent>(
            json!({"TimerFired": {"timer_id": "t", "timestamp_ms": "x"}})
        )
        .is_err());
    }

    #[test]
    fn reject_negative_timestamp() {
        assert!(serde_json::from_value::<WorkflowEvent>(
            json!({"TimerFired": {"timer_id": "t", "timestamp_ms": -1}})
        )
        .is_err());
    }

    #[test]
    fn reject_float_timestamp() {
        assert!(serde_json::from_value::<WorkflowEvent>(
            json!({"TimerFired": {"timer_id": "t", "timestamp_ms": 1.5}})
        )
        .is_err());
    }

    #[test]
    fn accept_u64_max_timestamp() {
        let val: serde_json::Value = serde_json::from_str(&format!(
            r#"{{"TimerFired":{{"timer_id":"t","timestamp_ms":{}}}}}"#,
            u64::MAX
        ))
        .unwrap();
        let event = serde_json::from_value(val).unwrap();
        if let WorkflowEvent::TimerFired { timestamp_ms, .. } = event {
            assert_eq!(timestamp_ms, u64::MAX);
        } else {
            panic!("expected TimerFired");
        }
    }

    #[test]
    fn accept_zero_timestamp() {
        let event =
            serde_json::from_value(json!({"TimerFired": {"timer_id": "t", "timestamp_ms": 0}}))
                .unwrap();
        if let WorkflowEvent::TimerFired { timestamp_ms, .. } = event {
            assert_eq!(timestamp_ms, 0);
        } else {
            panic!("expected TimerFired");
        }
    }

    #[test]
    fn empty_strings_accepted() {
        let e: WorkflowEvent = serde_json::from_value(json!({
            "TaskCompleted": {"task_id": "", "result_json": ""}
        }))
        .unwrap();
        assert!(
            matches!(e, WorkflowEvent::TaskCompleted { task_id, result_json }
            if task_id.is_empty() && result_json.is_empty())
        );
    }

    #[test]
    fn empty_and_malformed_root() {
        assert!(serde_json::from_value::<WorkflowEvent>(json!({})).is_err());
        assert!(serde_json::from_value::<WorkflowEvent>(json!([1, 2])).is_err());
    }
}

#[cfg(test)]
mod string_attacks {
    use super::*;

    #[test]
    fn empty_strings_no_panic() {
        let _: InstanceId = String::new();
        let _: NamespaceId = String::new();
        let _: TimerId = String::new();
    }

    #[test]
    fn unicode_roundtrips() {
        for payload in [
            "\u{0}",
            "\u{FFFF}",
            "\u{10FFFF}",
            "\u{FEFF}",
            "\u{202E}evil\u{202C}",
            "████████████████████████",
        ] {
            let e = WorkflowEvent::TimerFired {
                timer_id: payload.into(),
                timestamp_ms: 42,
            };
            let json = serde_json::to_string(&e).unwrap();
            assert_eq!(e, serde_json::from_str::<WorkflowEvent>(&json).unwrap());
        }
    }

    #[test]
    fn megabyte_string_no_panic() {
        let big = "x".repeat(1_000_000);
        let e = WorkflowEvent::TimerFired {
            timer_id: big.clone(),
            timestamp_ms: 0,
        };
        let json = serde_json::to_string(&e).unwrap();
        let event = serde_json::from_str::<WorkflowEvent>(&json).unwrap();
        if let WorkflowEvent::TimerFired { timer_id, .. } = event {
            assert_eq!(timer_id.len(), 1_000_000);
        } else {
            panic!("expected TimerFired");
        }
    }
}

#[cfg(test)]
mod error_attacks {
    use super::*;

    #[test]
    fn empty_messages_still_display() {
        for e in [
            VoError::config(""),
            VoError::internal(""),
            VoError::not_found(""),
            VoError::validation(""),
            VoError::timeout(""),
        ] {
            assert!(!e.to_string().is_empty());
        }
    }

    #[test]
    fn clone_unicode_equality() {
        let e = VoError::internal("\u{1F4A9}".repeat(1000));
        assert_eq!(e, e.clone());
    }

    #[test]
    fn all_variants_distinct() {
        let v = [
            VoError::config("x"),
            VoError::internal("x"),
            VoError::not_found("x"),
            VoError::validation("x"),
            VoError::timeout("x"),
        ];
        for (i, a) in v.iter().enumerate() {
            for (j, b) in v.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "variants {i} and {j} collided");
                }
            }
        }
    }
}
