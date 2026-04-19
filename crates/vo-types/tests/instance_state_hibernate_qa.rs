//! QA: Instance state preservation across hibernate (ve-ody72)

#![allow(clippy::unwrap_used)]

use serde_json;
use vo_types::state::InstanceState;

#[test]
fn instance_state_serializes_and_deserializes_across_hibernate() {
    let original = InstanceState { counter: 42 };

    // Simulate hibernate: serialize state to bytes
    let state_bytes = serde_json::to_vec(&original).unwrap();
    assert!(!state_bytes.is_empty(), "serialized state must be non-empty");

    // Simulate restore: deserialize from bytes
    let restored: InstanceState = serde_json::from_slice(&state_bytes).unwrap();

    assert_eq!(restored, original, "restored state must match original");
}

#[test]
fn instance_state_with_large_counter_survives_round_trip() {
    let original = InstanceState {
        counter: u64::MAX,
    };

    let state_bytes = serde_json::to_vec(&original).unwrap();
    let restored: InstanceState = serde_json::from_slice(&state_bytes).unwrap();

    assert_eq!(restored.counter, u64::MAX);
}

#[test]
fn instance_state_json_format_is_stable_for_hibernate() {
    let state = InstanceState { counter: 99 };
    let json = serde_json::to_string(&state).unwrap();
    assert_eq!(json, r#"{"counter":99}"#);

    let restored: InstanceState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.counter, 99);
}
