#![allow(clippy::unwrap_used, clippy::redundant_clone)]
use crate::timer_index::{TimerKey, TimerValue};
use crate::timer_index_tests::{create_instance_id, create_timer_id};

#[test]
fn fn_timer_key_new_encodes_bytes_correctly() {
    let key = TimerKey::new(1234, create_instance_id(), create_timer_id()).unwrap();
    assert_eq!(key.as_bytes().len(), 40);
    assert_eq!(key.fire_at_ms(), 1234);
}

#[test]
fn fn_timer_key_instance_id_returns_original_instance_id() {
    let instance_id = create_instance_id();
    let key = TimerKey::new(1234, instance_id.clone(), create_timer_id()).unwrap();
    assert_eq!(key.instance_id(), instance_id);
}

#[test]
fn fn_timer_key_timer_id_returns_original_timer_id() {
    let timer_id = create_timer_id();
    let key = TimerKey::new(1234, create_instance_id(), timer_id.clone()).unwrap();
    assert_eq!(key.timer_id(), timer_id);
}

#[test]
fn fn_timer_value_returns_invalid_argument_when_duration_is_zero() {
    assert_eq!(
        TimerValue::new(0).map(|value| value.duration_ms()),
        Err(crate::codec::StorageError::InvalidArgument)
    );
}

#[test]
fn fn_timer_value_returns_duration_when_duration_is_non_zero() {
    let value = TimerValue::new(250).unwrap();
    assert_eq!(value.duration_ms(), 250);
}

#[test]
fn fn_timer_value_returns_big_endian_bytes_for_duration() {
    let value = TimerValue::new(0x0102_0304_0506_0708).unwrap();
    assert_eq!(value.as_be_bytes(), [1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn fn_proptest_timer_key_ordering_preserves_lexicographic_order(a in 0u64..1000, b in 1001u64..2000) {
    let iid = create_instance_id();
    let tid = create_timer_id();
    let key_a = TimerKey::new(a, iid.clone(), tid.clone()).unwrap();
    let key_b = TimerKey::new(b, iid, tid).unwrap();
        proptest::prop_assert!(key_a.as_bytes() < key_b.as_bytes());
}

proptest! {
    #[test]
    fn fn_proptest_timer_key_ordering_preserves_lexicographic_order(a in 0u64..1000, b in 1001u64..2000) {
        let iid = create_instance_id();
        let tid = create_timer_id();
        let key_a = TimerKey::new(a, iid.clone(), tid.clone()).unwrap();
        let key_b = TimerKey::new(b, iid, tid).unwrap();
        proptest::prop_assert!(key_a.as_bytes() < key_b.as_bytes());
    }
}