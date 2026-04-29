use crate::helpers::{
    make_test_instance_id, make_test_timer_id, scan_due_timers, timer_set, MockStorage,
};

#[test]
fn rq_multiple_timers_different_fire_times() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_1 = make_test_timer_id(0x02);
    let timer_id_2 = make_test_timer_id(0x03);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_1.clone(),
        1500,
        1000,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_2.clone(),
        2000,
        1500,
        500,
        0,
    )
    .unwrap();

    let result = scan_due_timers(&storage, &instance_id, 1499).unwrap();
    assert_eq!(result.len(), 0);

    let result = scan_due_timers(&storage, &instance_id, 1500).unwrap();
    assert_eq!(result.len(), 1);

    let result = scan_due_timers(&storage, &instance_id, 1999).unwrap();
    assert_eq!(result.len(), 1);

    let result = scan_due_timers(&storage, &instance_id, 2000).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn rq_trigger_time_saturating_sub() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1500, // fire_at_ms
        500,  // trigger_time_ms (1500 = 500 + 1000, satisfies dual-clock)
        1000, // duration_ms
        0,
    )
    .unwrap();

    let result = scan_due_timers(&storage, &instance_id, 1500).unwrap();
    assert_eq!(result.len(), 1);
    let record = &result[0];
    assert_eq!(record.fire_at_ms, 1500);
    assert_eq!(record.trigger_time_ms, 500);
    assert_eq!(record.duration_ms, 1000);
}
