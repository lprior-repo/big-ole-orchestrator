use crate::helpers::{
    make_test_instance_id, make_test_timer_id, scan_due_timers, timer_set, MockStorage,
};

#[test]
fn rq_scan_due_timers_empty_when_no_timers() {
    let storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);

    let result = scan_due_timers(&storage, &instance_id, 1000);
    assert!(result.is_ok(), "Scan should succeed");
    assert!(result.unwrap().is_empty(), "Should be empty when no timers");
}

#[test]
fn rq_scan_due_timers_filters_by_instance() {
    let mut storage = MockStorage::new();
    let instance_id_1 = make_test_instance_id(0x01);
    let instance_id_2 = make_test_instance_id(0x02);
    let timer_id = make_test_timer_id(0x03);

    timer_set(
        &mut storage,
        instance_id_1.clone(),
        timer_id.clone(),
        500, // fire_at_ms
        0,   // trigger_time_ms
        500, // duration_ms
        0,   // now_ms
    )
    .unwrap();

    let result = scan_due_timers(&storage, &instance_id_2, 1000);
    assert!(result.is_ok(), "Scan should succeed");
    assert!(
        result.unwrap().is_empty(),
        "Should not find timer for different instance"
    );
}

#[test]
fn rq_scan_due_timers_boundary_now_ms() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1000, // fire_at_ms
        500,  // trigger_time_ms
        500,  // duration_ms
        0,    // now_ms
    )
    .unwrap();

    let result = scan_due_timers(&storage, &instance_id, 999);
    assert!(result.is_ok());
    assert!(
        result.unwrap().is_empty(),
        "Timer at 1000 should not be due at 999"
    );

    let result = scan_due_timers(&storage, &instance_id, 1000);
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap().len(),
        1,
        "Timer at 1000 should be due at 1000"
    );
}

#[test]
fn rq_scan_due_timers_propagates_storage_failure() {
    let storage = MockStorage::with_fail("scan");
    let instance_id = make_test_instance_id(0x01);

    let result = scan_due_timers(&storage, &instance_id, 1000);
    assert_eq!(
        result,
        Err(vo_storage::codec::StorageError::Storage),
        "Storage failure should be propagated"
    );
}
