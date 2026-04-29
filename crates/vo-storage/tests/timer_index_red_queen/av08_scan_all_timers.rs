use crate::helpers::{
    make_test_instance_id, make_test_timer_id, scan_all_timers_for_instance, timer_set, MockStorage,
};

#[test]
fn rq_scan_all_timers_empty_when_no_timers() {
    let storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);

    let result = scan_all_timers_for_instance(&storage, &instance_id);
    assert!(result.is_ok(), "Scan should succeed");
    assert!(result.unwrap().is_empty(), "Should be empty when no timers");
}

#[test]
fn rq_scan_all_timers_includes_future_timers() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_1 = make_test_timer_id(0x02);
    let timer_id_2 = make_test_timer_id(0x03);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_1.clone(),
        1000, // fire_at_ms
        500,  // trigger_time_ms
        500,  // duration_ms
        500,  // now_ms
    )
    .unwrap();

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_2.clone(),
        5000, // fire_at_ms
        4500, // trigger_time_ms
        500,  // duration_ms
        2000, // now_ms
    )
    .unwrap();

    let result = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
    assert_eq!(result.len(), 2, "Should return both past and future timers");

    let fire_times: Vec<u64> = result.iter().map(|r| r.fire_at_ms).collect();
    assert!(fire_times.contains(&1000u64), "Should include past timer");
    assert!(fire_times.contains(&5000u64), "Should include future timer");
}

#[test]
fn rq_scan_all_timers_filters_by_instance() {
    let mut storage = MockStorage::new();
    let instance_id_1 = make_test_instance_id(0x01);
    let instance_id_2 = make_test_instance_id(0x02);
    let timer_id_1 = make_test_timer_id(0x03);
    let timer_id_2 = make_test_timer_id(0x04);

    timer_set(
        &mut storage,
        instance_id_1.clone(),
        timer_id_1.clone(),
        1000,
        500,
        500,
        0,
    )
    .unwrap();

    timer_set(
        &mut storage,
        instance_id_2.clone(),
        timer_id_2.clone(),
        2000,
        1500,
        500,
        0,
    )
    .unwrap();

    let result = scan_all_timers_for_instance(&storage, &instance_id_1).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timer_id, timer_id_1);

    let result = scan_all_timers_for_instance(&storage, &instance_id_2).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timer_id, timer_id_2);
}

#[test]
fn rq_scan_all_timers_propagates_storage_failure() {
    let storage = MockStorage::with_fail("scan");
    let instance_id = make_test_instance_id(0x01);

    let result = scan_all_timers_for_instance(&storage, &instance_id);
    assert_eq!(result, Err(vo_storage::codec::StorageError::Storage));
}

#[test]
fn rq_scan_all_timers_returns_correct_fields() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        3000, // fire_at_ms
        2500, // trigger_time_ms
        500,  // duration_ms
        1000, // now_ms
    )
    .unwrap();

    let result = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
    assert_eq!(result.len(), 1);
    let record = &result[0];
    assert_eq!(record.timer_id, timer_id);
    assert_eq!(record.instance_id, instance_id);
    assert_eq!(record.fire_at_ms, 3000);
    assert_eq!(record.trigger_time_ms, 2500);
    assert_eq!(record.duration_ms, 500);
}
