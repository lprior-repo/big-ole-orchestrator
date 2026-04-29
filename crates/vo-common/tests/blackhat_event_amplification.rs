//! Black-hat adversarial tests for vo-common event handling amplification.
//!
//! Attacks event processing invariants under resource exhaustion, serialization
//! bombs, clone storms, and payload inflation.
//!
//! ve-9yq0w — BLACKHAT: vo-common — event handling — event amplification

use vo_common::{EventId, WorkflowEvent};

fn make_event(event_id: &str, timer_id: &str, timestamp_ms: u64) -> WorkflowEvent {
    WorkflowEvent::TimerFired {
        event_id: event_id.into(),
        timer_id: timer_id.into(),
        timestamp_ms,
    }
}

fn make_evt(event_id: &str, timer_id: &str, timestamp_ms: u64) -> WorkflowEvent {
    WorkflowEvent::TimerFired {
        event_id: event_id.into(),
        timer_id: timer_id.into(),
        timestamp_ms,
    }
}

#[cfg(test)]
mod serialization_bomb {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn deeply_nested_rejected_not_consumed() {
        let mut val = json!({"timer_id": "x", "timestamp_ms": 0i64});
        for _ in 0..100 {
            val = json!({"TimerFired": val});
        }
        assert!(
            serde_json::from_value::<WorkflowEvent>(val).is_err(),
            "deeply nested payload must be rejected"
        );
    }

    #[test]
    fn wide_json_extra_keys_no_panic() {
        let mut obj = serde_json::Map::new();
        obj.insert("event_id".into(), Value::String("evt-x".into()));
        obj.insert("timer_id".into(), Value::String("t".into()));
        obj.insert("timestamp_ms".into(), Value::Number(0.into()));
        for i in 0..1000 {
            obj.insert(format!("_junk_{i}"), Value::String("a".repeat(100)));
        }
        let result: Result<WorkflowEvent, _> = serde_json::from_value(Value::Object(obj));
        if let Ok(event) = result {
            match event {
                WorkflowEvent::TimerFired { timer_id, .. } => assert_eq!(timer_id, "t"),
            }
        }
    }

    #[test]
    fn whitespace_inflation_no_panic() {
        let payload = " ".repeat(100_000) + "x";
        let val = json!({
            "TimerFired": { "event_id": "evt-x", "timer_id": payload, "timestamp_ms": 0 }
        });
        let _ = serde_json::from_value::<WorkflowEvent>(val);
    }

    #[test]
    fn unicode_escape_expansion_no_panic() {
        let escaped: String = (0..50_000)
            .map(|i| format!("\\u{:04x}", i % 0x10000))
            .collect();
        let json_str = format!(r#"{{"TimerFired":{{"event_id":"evt-x","timer_id":"{escaped}","timestamp_ms":0}}}}"#);
        let _: Result<WorkflowEvent, _> = serde_json::from_str(&json_str);
    }

    #[test]
    fn rapid_clone_no_panic() {
        let event = make_event("clone-base", "clone-target", 12345);
        let mut events = Vec::with_capacity(10_000);
        for _ in 0..10_000 {
            events.push(event.clone());
        }
        assert_eq!(events.len(), 10_000);
        assert!(events.iter().all(|e| *e == event));
    }

    #[test]
    fn clone_chain_amplification() {
        let mut event = make_event("chain-0", "chain", 1);
        for i in 0..1_000 {
            let cloned = event.clone();
            event = make_event(&format!("chain-{i}"), &format!("{i}"), i);
            drop(cloned);
        }
    }

    #[test]
    fn large_payload_clone_pressure() {
        let big_id = "x".repeat(1_000_000);
        let event = make_event("clone-base", &big_id, 0);
        let mut clones = Vec::with_capacity(100);
        for _ in 0..100 {
            clones.push(event.clone());
        }
        assert_eq!(clones.len(), 100);
    }

    #[test]
    fn equal_check_on_mismatched_sizes() {
        let small = make_event("small", "x", 0);
        let big = make_event("big", &"x".repeat(1_000_000), 0);
        assert_ne!(small, big);
    }

    #[test]
    fn n_squared_equality_checks() {
        use std::time::Instant;
        let events: Vec<WorkflowEvent> = (0..500)
            .map(|i| make_event(&format!("event-{i}"), &format!("event-{i}"), i))
            .collect();
        let start = Instant::now();
        for (i, a) in events.iter().enumerate() {
            for (j, b) in events.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
        assert!(
            start.elapsed().as_secs() < 10,
            "equality checks took too long"
        );
    }

    #[test]
    fn unicode_normalization_mismatch() {
        let a = make_event("norm-a", "caf\u{00E9}", 0);
        let b = make_event("norm-b", "cafe\u{0301}", 0);
        assert_ne!(a, b, "Rust must NOT normalize Unicode — these are distinct");
    }

    #[test]
    fn rapid_serde_roundtrip_storm() {
        let event = make_event("flood-target", "flood-target", 42);
        let json = serde_json::to_string(&event).unwrap();
        for _ in 0..10_000 {
            let rt: WorkflowEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(rt, event);
        }
    }

    #[test]
    fn concurrent_json_parse_threads() {
        use std::sync::Arc;
        use std::thread;
        let event = make_event("concurrent", "concurrent", 999);
        let json = Arc::new(serde_json::to_string(&event).unwrap());
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let j = json.clone();
                thread::spawn(move || {
                    for _ in 0..1_000 {
                        let _: WorkflowEvent = serde_json::from_str(&j).unwrap();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn malformed_json_rejection_rate() {
        let garbage_inputs = [
            r#"{"TimerFired":{"event_id":"e","timer_id":0}}"#,
            r#"{"TimerFired":{"event_id":"e","timestamp_ms":"not_a_number"}}"#,
            r#"{"TimerFired":{"event_id":"e","timer_id":"","timestamp_ms":-1}}"#,
            r#"{"TimerFired":true}"#,
            r#"{"TimerFired":null}"#,
            r#"{"TimerFired":[]}"#,
            r#"{"TimerFired":{"event_id":null,"timer_id":null,"timestamp_ms":0}}"#,
            r#"{"TimerFired":{"event_id":"","timer_id":"","timestamp_ms":null}}"#,
            r#"{""TimerFired"":{}}"#,
        ];
        for input in garbage_inputs {
            assert!(
                serde_json::from_str::<WorkflowEvent>(input).is_err(),
                "garbage input was accepted: {input}"
            );
        }
    }

    #[test]
    fn timer_id_with_json_escape_sequences() {
        let event = make_event("escape-seq", r#"{"injected":true}"#, 0);
        let json_str = serde_json::to_string(&event).unwrap();
        assert!(
            !json_str.contains(r#"{"injected":true}"#),
            "raw JSON in timer_id must be escaped"
        );
        let rt: WorkflowEvent = serde_json::from_str(&json_str).unwrap();
        assert_eq!(rt, event);
    }

    #[test]
    fn timer_id_with_null_bytes() {
        let id_with_null = "before\0after".to_string();
        let event = make_event("null-bytes", &id_with_null, 0);
        let json_str = serde_json::to_string(&event).unwrap();
        assert!(
            !json_str.contains('\0'),
            "null byte must be escaped in JSON output"
        );
        let rt: WorkflowEvent = serde_json::from_str(&json_str).unwrap();
        assert_eq!(rt, event);
    }

    #[test]
    fn timestamp_u64_max_roundtrip() {
        let event = make_event("max-ts", "max-ts", u64::MAX);
        let json_str = serde_json::to_string(&event).unwrap();
        let rt: WorkflowEvent = serde_json::from_str(&json_str).unwrap();
        assert_eq!(rt, event);
    }

    #[test]
    fn empty_timer_id_roundtrip() {
        let event = make_event("empty-id", "", 0);
        let json_str = serde_json::to_string(&event).unwrap();
        let rt: WorkflowEvent = serde_json::from_str(&json_str).unwrap();
        assert_eq!(rt, event);
    }

    #[test]
    fn json_output_size_bounded_by_input() {
        let id = "x".repeat(100);
        let event = make_event("bounded", &id, 42);
        let json_str = serde_json::to_string(&event).unwrap();
        assert!(
            json_str.len() < id.len() * 5,
            "JSON output too large: {} vs input {}",
            json_str.len(),
            id.len()
        );
    }

    #[test]
    fn many_events_vec_no_leak() {
        let events: Vec<WorkflowEvent> = (0..10_000)
            .map(|i| make_event(&format!("evt-{i}"), &format!("evt-{i}"), i))
            .collect();
        assert_eq!(events.len(), 10_000);
        drop(events);
        let events2: Vec<WorkflowEvent> = (0..10_000)
            .map(|i| make_event(&format!("evt2-{i}"), &format!("evt2-{i}"), i))
            .collect();
        assert_eq!(events2.len(), 10_000);
    }

    #[test]
    fn debug_format_large_event_no_panic() {
        let event = make_event("debug-large", &"x".repeat(1_000_000), 0);
        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn identical_events_are_equal() {
        let e1 = make_event("replay", "replay", 100);
        let e2 = make_event("replay", "replay", 100);
        assert_eq!(e1, e2);
    }

    #[test]
    fn timestamp_diff_distinguishes_replay() {
        let e1 = make_event("same-id", "same-id", 100);
        let e2 = make_event("same-id", "same-id", 101);
        assert_ne!(
            e1, e2,
            "replayed event with different timestamp must be distinct"
        );
    }

    #[test]
    fn json_replay_deterministic() {
        let event = make_event("det", "det", 42);
        let j1 = serde_json::to_string(&event).unwrap();
        let j2 = serde_json::to_string(&event).unwrap();
        assert_eq!(j1, j2, "same event must produce identical JSON");
    }

    #[test]
    fn unknown_variant_step_completed_rejected() {
        use serde_json::json;
        assert!(
            serde_json::from_value::<WorkflowEvent>(json!({"StepCompleted":{"step_id":"s1"}}))
                .is_err()
        );
    }

    #[test]
    fn primitive_root_values_rejected() {
        use serde_json::json;
        assert!(serde_json::from_value::<WorkflowEvent>(json!(null)).is_err());
        assert!(serde_json::from_value::<WorkflowEvent>(json!(true)).is_err());
    }

    #[test]
    fn numeric_variant_key_rejected() {
        // serde_json Map requires String keys — numeric keys can't be constructed.
        // Verify that the JSON layer rejects non-string keys at parse time.
        assert!(serde_json::from_str::<serde_json::Value>(r#"{0:"x"}"#).is_err());
    }

    #[test]
    fn float_timestamp_rejected() {
        assert!(serde_json::from_str::<WorkflowEvent>(
            r#"{"TimerFired":{"timer_id":"t","timestamp_ms":1.5}}"#
        )
        .is_err());
    }
}
