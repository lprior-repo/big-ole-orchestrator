use super::*;

#[test]
fn serde_round_trip_sequence_number_inline() {
    let original = SequenceNumber::new_unchecked(42);
    let json = serde_json::to_value(original).expect("serialize");
    let restored: SequenceNumber = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_event_version_inline() {
    let original = EventVersion::new_unchecked(1);
    let json = serde_json::to_value(original).expect("serialize");
    let restored: EventVersion = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_attempt_number_inline() {
    let original = AttemptNumber::new_unchecked(3);
    let json = serde_json::to_value(original).expect("serialize");
    let restored: AttemptNumber = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_timeout_ms_inline() {
    let original = TimeoutMs::new_unchecked(5000);
    let json = serde_json::to_value(original).expect("serialize");
    let restored: TimeoutMs = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_duration_ms_inline() {
    let original = DurationMs(5000);
    let json = serde_json::to_value(original).expect("serialize");
    let restored: DurationMs = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_timestamp_ms_inline() {
    let original = TimestampMs(1710000000000);
    let json = serde_json::to_value(original).expect("serialize");
    let restored: TimestampMs = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_fire_at_ms_inline() {
    let original = FireAtMs(1710000000000);
    let json = serde_json::to_value(original).expect("serialize");
    let restored: FireAtMs = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}

#[test]
fn serde_round_trip_max_attempts_inline() {
    let original = MaxAttempts::new_unchecked(3);
    let json = serde_json::to_value(original).expect("serialize");
    let restored: MaxAttempts = serde_json::from_value(json).expect("deserialize");
    assert_eq!(restored, original);
}