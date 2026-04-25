#![allow(clippy::unwrap_used, clippy::redundant_clone)]
use crate::codec::StorageError;
use crate::timer_index::TimerRecord;
use crate::timer_index_tests::{create_instance_id, create_timer_id};

#[test]
fn fn_timer_record_try_from_parts_returns_record_when_dual_clock_matches() {
    let timer_id = create_timer_id();
    let instance_id = create_instance_id();
    let record =
        TimerRecord::try_from_parts(timer_id.clone(), instance_id.clone(), 1100, 1000, 100);
    assert_eq!(
        record,
        Ok(TimerRecord {
            timer_id,
            instance_id,
            fire_at_ms: 1100,
            trigger_time_ms: 1000,
            duration_ms: 100,
        })
    );
}

#[test]
fn fn_timer_record_try_from_parts_returns_invalid_argument_when_duration_is_zero() {
    let result =
        TimerRecord::try_from_parts(create_timer_id(), create_instance_id(), 1000, 1000, 0);
    assert_eq!(result, Err(StorageError::InvalidArgument));
}

#[test]
fn fn_timer_record_try_from_parts_returns_invalid_argument_when_dual_clock_mismatches() {
    let result =
        TimerRecord::try_from_parts(create_timer_id(), create_instance_id(), 1001, 900, 100);
    assert_eq!(result, Err(StorageError::InvalidArgument));
}