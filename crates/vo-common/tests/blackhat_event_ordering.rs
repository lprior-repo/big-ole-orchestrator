//! BLACK-HAT adversarial tests: event ordering attacks (ve-s2ccy).
//!
//! Attacks targeting the ordering invariants of WorkflowEvent::TimerFired.

use vo_common::WorkflowEvent;

fn make_evt(event_id: &str, timer_id: &str, timestamp_ms: u64) -> WorkflowEvent {
    WorkflowEvent::TimerFired {
        event_id: event_id.into(),
        timer_id: timer_id.into(),
        timestamp_ms,
    }
}

#[test]
fn identical_events_are_equal() {
    let a = make_evt("e1", "t1", 100);
    let b = make_evt("e1", "t1", 100);
    assert_eq!(a, b);
}

#[test]
fn replay_same_timer_later_timestamp_is_distinct() {
    let original = make_evt("e1", "t1", 100);
    let replay = make_evt("e2", "t1", 200);
    assert_ne!(original, replay);
}

#[test]
fn replay_same_timer_earlier_timestamp_is_distinct() {
    let original = make_evt("e1", "t1", 200);
    let replay = make_evt("e2", "t1", 50);
    assert_ne!(original, replay);
}

#[test]
fn mass_replay_all_identical_stable_sort() {
    let event = make_evt("shared", "shared", 42);
    let copies: Vec<_> = (0..10_000).map(|_| event.clone()).collect();
    for c in &copies {
        assert_eq!(*c, event);
    }
}

#[test]
fn zero_and_max_timestamp_are_distinct() {
    let zero = make_evt("e", "t", 0);
    let max = make_evt("e", "t", u64::MAX);
    assert_ne!(zero, max);
}

#[test]
fn adjacent_timestamps_are_distinct() {
    let a = make_evt("a", "t", 1000);
    let b = make_evt("b", "t", 1001);
    assert_ne!(a, b);
}

#[test]
fn timestamp_overflow_boundary() {
    let near_max = make_evt("nm", "t", u64::MAX - 1);
    let at_max = make_evt("am", "t", u64::MAX);
    assert_ne!(near_max, at_max);
}

#[test]
fn timestamp_wraparound_does_not_panic() {
    let event = make_evt("wrap", "wrap", u64::MAX);
    let json = serde_json::to_string(&event).unwrap();
    let deser: WorkflowEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, deser);
}

#[test]
fn sort_by_timestamp_produces_correct_order() {
    let mut events = vec![
        make_evt("ea", "a", 500),
        make_evt("eb", "b", 100),
        make_evt("ec", "c", 300),
    ];
    events.sort_by_key(|e| match e {
        WorkflowEvent::TimerFired { timestamp_ms, .. } => *timestamp_ms,
    });
    let timestamps: Vec<u64> = events.iter().map(|e| match e {
        WorkflowEvent::TimerFired { timestamp_ms, .. } => *timestamp_ms,
    }).collect();
    assert_eq!(timestamps, vec![100, 300, 500]);
}

#[test]
fn reverse_chronological_sort_stable() {
    let mut events: Vec<WorkflowEvent> = (0..100).rev().map(|i| make_evt(&format!("e{i}"), &format!("t{i}"), i as u64)).collect();
    events.sort_by_key(|e| match e {
        WorkflowEvent::TimerFired { timestamp_ms, .. } => *timestamp_ms,
    });
    let timestamps: Vec<u64> = events.iter().map(|e| match e {
        WorkflowEvent::TimerFired { timestamp_ms, .. } => *timestamp_ms,
    }).collect();
    let expected: Vec<u64> = (0..100).collect();
    assert_eq!(timestamps, expected);
}

#[test]
fn identical_timestamps_different_timers_preserve_distinctness() {
    let a = make_evt("ea", "timer-a", 100);
    let b = make_evt("eb", "timer-b", 100);
    assert_ne!(a, b);
}

#[test]
fn field_reorder_in_json_still_deserializes() {
    let event = make_evt("e1", "t", 42);
    let json = r#"{"TimerFired":{"event_id":"e1","timestamp_ms":42,"timer_id":"t"}}"#;
    let deser: WorkflowEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event, deser);
}

#[test]
fn extra_whitespace_does_not_affect_equality() {
    let event = make_evt("e1", "t", 42);
    let json = "{  \"TimerFired\"  :  {  \"event_id\"  :  \"e1\"  ,  \"timer_id\"  :  \"t\"  ,  \"timestamp_ms\"  :  42  }  }";
    let deser: WorkflowEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event, deser);
}

#[test]
fn numeric_string_timestamp_rejected() {
    use serde_json::json;
    assert!(serde_json::from_value::<WorkflowEvent>(json!({
        "TimerFired": {"event_id": "e1", "timer_id": "t", "timestamp_ms": "999"}
    })).is_err());
}

#[test]
fn boolean_timestamp_rejected() {
    use serde_json::json;
    assert!(serde_json::from_value::<WorkflowEvent>(json!({
        "TimerFired": {"event_id": "e1", "timer_id": "t", "timestamp_ms": true}
    })).is_err());
}

#[test]
fn max_safe_integer_timestamp_roundtrips() {
    let event = make_evt("boundary", "boundary", i64::MAX as u64);
    let json = serde_json::to_string(&event).unwrap();
    let deser: WorkflowEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, deser);
}

#[test]
fn visually_similar_timer_ids_are_distinct() {
    let a = make_evt("ea", "timer-l", 0);
    let b = make_evt("eb", "timer-1", 0);
    assert_ne!(a, b);
}

#[test]
fn case_sensitive_timer_ids() {
    let upper = make_evt("eu", "TIMER-A", 0);
    let lower = make_evt("el", "timer-a", 0);
    assert_ne!(upper, lower);
}

#[test]
fn timer_id_with_null_bytes_survives_roundtrip() {
    let id = "timer\x00injected";
    let event = make_evt("null-bytes", id, 0);
    let json = serde_json::to_string(&event).unwrap();
    let deser: WorkflowEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, deser);
}

#[test]
fn timer_id_with_newlines_survives_roundtrip() {
    let id = "timer\nwith\nnewlines";
    let event = make_evt("newlines", id, 0);
    let json = serde_json::to_string(&event).unwrap();
    let deser: WorkflowEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, deser);
}

#[test]
fn dedup_map_latest_timestamp_wins() {
    use std::collections::HashMap;
    let mut latest: HashMap<String, u64> = HashMap::new();
    for (i, ts) in [100, 50, 200, 150, 200].into_iter().enumerate() {
        let event = make_evt(&format!("d{i}"), "dedup-test", ts);
        let (key, ts_val) = match &event {
            WorkflowEvent::TimerFired { timer_id, timestamp_ms, .. } => (timer_id.clone(), *timestamp_ms),
        };
        latest.entry(key).and_modify(|e| { if ts_val > *e { *e = ts_val; } }).or_insert(ts_val);
    }
    assert_eq!(latest.get("dedup-test").copied(), Some(200));
}

#[test]
fn many_timers_sort_maintains_total_order() {
    let mut events: Vec<WorkflowEvent> = (0..500).map(|i| make_evt(&format!("e{i}"), &format!("timer-{i}"), (499 - i) as u64)).collect();
    events.sort_by_key(|e| match e {
        WorkflowEvent::TimerFired { timestamp_ms, .. } => *timestamp_ms,
    });
    let timestamps: Vec<u64> = events.iter().map(|e| match e {
        WorkflowEvent::TimerFired { timestamp_ms, .. } => *timestamp_ms,
    }).collect();
    let expected: Vec<u64> = (0..500).collect();
    assert_eq!(timestamps, expected);
}
