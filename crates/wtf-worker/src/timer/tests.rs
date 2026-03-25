use super::r#loop::TIMER_POLL_INTERVAL;
use super::record::*;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::time::Duration;
use wtf_common::{InstanceId, NamespaceId, TimerId};

fn make_record(timer_id: &str, fire_at: DateTime<Utc>) -> TimerRecord {
    TimerRecord {
        timer_id: TimerId::new(timer_id),
        namespace: NamespaceId::new("payments"),
        instance_id: InstanceId::new("inst-001"),
        fire_at,
    }
}

#[test]
fn timer_record_is_due_when_fire_at_is_in_the_past() {
    let past = Utc::now() - ChronoDuration::seconds(5);
    let record = make_record("timer-001", past);
    assert!(record.is_due(Utc::now()));
}

#[test]
fn timer_record_is_due_when_fire_at_equals_now() {
    let now = Utc::now();
    let record = make_record("timer-002", now);
    assert!(record.is_due(now));
}

#[test]
fn timer_record_is_not_due_when_fire_at_is_in_the_future() {
    let future = Utc::now() + ChronoDuration::hours(1);
    let record = make_record("timer-003", future);
    assert!(!record.is_due(Utc::now()));
}

#[test]
fn timer_record_msgpack_roundtrip() {
    let fire_at = Utc::now();
    let record = make_record("timer-004", fire_at);
    let bytes = record.to_msgpack().expect("serialize");
    assert!(!bytes.is_empty());
    let decoded = TimerRecord::from_msgpack(&bytes).expect("deserialize");
    assert_eq!(decoded.timer_id.as_str(), "timer-004");
    assert_eq!(decoded.namespace.as_str(), "payments");
    assert_eq!(decoded.instance_id.as_str(), "inst-001");
}

#[test]
fn timer_record_from_msgpack_invalid_bytes_returns_error() {
    let result = TimerRecord::from_msgpack(b"not valid msgpack!!!");
    assert!(result.is_err(), "expected error for invalid msgpack bytes");
}

#[test]
fn timer_poll_interval_is_one_second() {
    assert_eq!(TIMER_POLL_INTERVAL, Duration::from_secs(1));
}
