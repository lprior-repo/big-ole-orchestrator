use crate::helpers::{make_test_instance_id, make_test_timer_id, timer_set, MockStorage};

#[test]
fn rq_timer_set_rejects_past_fire_time() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);
    let now_ms = 1000u64;

    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1000, // fire_at_ms == now_ms
        500,  // trigger_time_ms
        500,  // duration_ms
        now_ms,
    );
    assert_eq!(
        result,
        Err(vo_storage::codec::StorageError::InvalidArgument),
        "fire_at_ms == now_ms should be rejected"
    );

    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        999,  // fire_at_ms < now_ms
        499,  // trigger_time_ms
        500,  // duration_ms
        now_ms,
    );
    assert_eq!(
        result,
        Err(vo_storage::codec::StorageError::InvalidArgument),
        "fire_at_ms < now_ms should be rejected"
    );
}

#[test]
fn rq_timer_set_accepts_future_fire_time() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        2000,  // fire_at_ms > now_ms
        1500,  // trigger_time_ms
        500,   // duration_ms
        1000,  // now_ms
    );
    assert!(result.is_ok(), "Future fire_at_ms should be accepted");
}

#[test]
fn rq_timer_set_accepts_fire_at_ms_one_ms_in_future() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1001,  // fire_at_ms = now_ms + 1
        501,   // trigger_time_ms
        500,   // duration_ms
        1000,  // now_ms
    );
    assert!(result.is_ok(), "fire_at_ms one ms in future should be accepted");
}

#[test]
fn rq_timer_set_rejects_invalid_dual_clock() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    // 1000 != 500 + 400 -> invalid dual-clock
    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1000,  // fire_at_ms
        500,   // trigger_time_ms
        400,   // duration_ms (500 + 400 = 900, not 1000)
        0,     // now_ms
    );
    assert_eq!(
        result,
        Err(vo_storage::codec::StorageError::InvalidArgument),
        "Invalid dual-clock should be rejected"
    );
}

#[test]
fn rq_timer_set_accepts_valid_dual_clock() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    // 1000 == 500 + 500 -> valid
    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1000,  // fire_at_ms
        500,   // trigger_time_ms
        500,   // duration_ms
        0,     // now_ms
    );
    assert!(result.is_ok(), "Valid dual-clock should be accepted");
}

#[test]
fn rq_timer_set_storage_failure_propagates() {
    let mut storage = MockStorage::with_fail("put");
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        2000,
        1500,
        500,
        1000,
    );
    assert_eq!(
        result,
        Err(vo_storage::codec::StorageError::Storage),
        "Storage failure should be propagated"
    );
}