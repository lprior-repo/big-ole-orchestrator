use crate::helpers::{make_test_instance_id, make_test_timer_id};
use vo_storage::codec::StorageError;
use vo_storage::timer_index::TimerRecord;

#[test]
fn rq_timer_record_rejects_zero_duration() {
    let timer_id = make_test_timer_id(0x02);
    let instance_id = make_test_instance_id(0x01);
    let fire_at_ms = 1000u64;
    let trigger_time_ms = 500u64;
    let duration_ms = 0u64;

    let result = TimerRecord::try_from_parts(
        timer_id.clone(),
        instance_id.clone(),
        fire_at_ms,
        trigger_time_ms,
        duration_ms,
    );
    assert_eq!(
        result,
        Err(StorageError::InvalidArgument),
        "Zero duration should be rejected"
    );
}

#[test]
fn rq_timer_record_rejects_dual_clock_violation() {
    let timer_id = make_test_timer_id(0x02);
    let instance_id = make_test_instance_id(0x01);
    let fire_at_ms = 1000u64;
    let trigger_time_ms = 500u64;
    let duration_ms = 600u64; // 500 + 600 = 1100 != 1000

    let result = TimerRecord::try_from_parts(
        timer_id.clone(),
        instance_id.clone(),
        fire_at_ms,
        trigger_time_ms,
        duration_ms,
    );
    assert_eq!(
        result,
        Err(StorageError::InvalidArgument),
        "Dual-clock violation should be rejected"
    );
}

#[test]
fn rq_timer_record_accepts_valid_dual_clock() {
    let timer_id = make_test_timer_id(0x02);
    let instance_id = make_test_instance_id(0x01);
    let fire_at_ms = 1000u64;
    let trigger_time_ms = 400u64;
    let duration_ms = 600u64; // 400 + 600 = 1000

    let result = TimerRecord::try_from_parts(
        timer_id.clone(),
        instance_id.clone(),
        fire_at_ms,
        trigger_time_ms,
        duration_ms,
    );
    assert!(result.is_ok(), "Valid dual-clock should be accepted");
}