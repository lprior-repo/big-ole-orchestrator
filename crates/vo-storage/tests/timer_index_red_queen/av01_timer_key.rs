use crate::helpers::{make_test_instance_id, make_test_timer_id};
use vo_types::InstanceId;

#[test]
fn rq_timer_key_rejects_invalid_instance_id_length() {
    let timer_id = make_test_timer_id(0x02);
    let fire_at_ms = 1000u64;

    let result = TimerKey::new(
        fire_at_ms,
        InstanceId::from_bytes([0; 16]),
        timer_id.clone(),
    );
    assert!(result.is_ok(), "Valid 16-byte instance ID should work");

    let zero_id = InstanceId::from_bytes([0; 16]);
    let result = TimerKey::new(fire_at_ms, zero_id, timer_id.clone());
    assert!(result.is_ok(), "Zero-filled instance ID should work");
}

#[test]
fn rq_timer_key_handles_u64_boundary_values() {
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    let result = TimerKey::new(u64::MAX, instance_id.clone(), timer_id.clone());
    assert!(result.is_ok(), "u64::MAX fire_at_ms should be valid");

    let result = TimerKey::new(0u64, instance_id.clone(), timer_id.clone());
    assert!(result.is_ok(), "0 fire_at_ms should be valid");

    let result = TimerKey::new(1u64, instance_id.clone(), timer_id.clone());
    assert!(result.is_ok(), "1 fire_at_ms should be valid");
}

#[test]
fn rq_timer_key_extraction_at_boundaries() {
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    let key = TimerKey::new(u64::MAX, instance_id.clone(), timer_id.clone()).unwrap();
    assert_eq!(key.fire_at_ms(), u64::MAX);
    assert_eq!(key.instance_id(), instance_id);
    assert_eq!(key.timer_id(), timer_id);

    let key = TimerKey::new(0u64, instance_id.clone(), timer_id.clone()).unwrap();
    assert_eq!(key.fire_at_ms(), 0);
}
