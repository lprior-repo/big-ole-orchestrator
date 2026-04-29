use crate::helpers::{
    make_test_instance_id, make_test_timer_id, scan_due_timers, timer_set, MockStorage,
};

#[test]
fn rq_crash_recovery_finds_timers_that_fired_during_downtime() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_1 = make_test_timer_id(0x02);
    let timer_id_2 = make_test_timer_id(0x03);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_1.clone(),
        1500, // fire_at_ms
        1000, // trigger_time_ms
        500,  // duration_ms
        1000, // now_ms (server start time)
    )
    .unwrap();

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_2.clone(),
        2000, // fire_at_ms
        1500, // trigger_time_ms
        500,  // duration_ms
        1000, // now_ms
    )
    .unwrap();

    let result = scan_due_timers(&storage, &instance_id, 2500).unwrap();
    assert_eq!(
        result.len(),
        2,
        "Should recover both timers that fired during downtime"
    );

    let fire_times: Vec<u64> = result.iter().map(|r| r.fire_at_ms).collect();
    assert!(
        fire_times.contains(&1500u64),
        "Should find timer 1 that fired at 1500"
    );
    assert!(
        fire_times.contains(&2000u64),
        "Should find timer 2 that fired at 2000"
    );
}

#[test]
fn rq_crash_recovery_only_recovers_target_instance_timers() {
    let mut storage = MockStorage::new();
    let target_instance = make_test_instance_id(0x01);
    let other_instance = make_test_instance_id(0x02);
    let timer_id_target = make_test_timer_id(0x03);
    let timer_id_other = make_test_timer_id(0x04);

    timer_set(
        &mut storage,
        target_instance.clone(),
        timer_id_target.clone(),
        1500,
        1000,
        500,
        1000,
    )
    .unwrap();

    timer_set(
        &mut storage,
        other_instance.clone(),
        timer_id_other.clone(),
        1500,
        1000,
        500,
        1000,
    )
    .unwrap();

    let result = scan_due_timers(&storage, &target_instance, 2000).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timer_id, timer_id_target);
    assert_eq!(result[0].instance_id, target_instance);
}

#[test]
fn rq_crash_recovery_does_not_return_future_timers() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_past = make_test_timer_id(0x02);
    let timer_id_future = make_test_timer_id(0x03);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_past.clone(),
        1500,
        1000,
        500,
        1000,
    )
    .unwrap();

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_future.clone(),
        5000,
        4500,
        500,
        1000,
    )
    .unwrap();

    let result = scan_due_timers(&storage, &instance_id, 2000).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timer_id, timer_id_past);

    let result = scan_due_timers(&storage, &instance_id, 6000).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn rq_crash_recovery_timer_fired_at_boundary() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1000,
        500,
        500,
        500,
    )
    .unwrap();

    let result = scan_due_timers(&storage, &instance_id, 1000).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timer_id, timer_id);
}

#[test]
fn rq_crash_recovery_multiple_timers_different_times() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_1 = make_test_timer_id(0x02);
    let timer_id_2 = make_test_timer_id(0x03);
    let timer_id_3 = make_test_timer_id(0x04);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_1.clone(),
        500,
        0,
        500,
        0,
    )
    .unwrap();

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_2.clone(),
        1000,
        500,
        500,
        0,
    )
    .unwrap();

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_3.clone(),
        1500,
        1000,
        500,
        0,
    )
    .unwrap();

    let result = scan_due_timers(&storage, &instance_id, 2500).unwrap();
    assert_eq!(result.len(), 3);

    let timer_ids: Vec<_> = result.iter().map(|r| r.timer_id.clone()).collect();
    assert!(timer_ids.contains(&timer_id_1));
    assert!(timer_ids.contains(&timer_id_2));
    assert!(timer_ids.contains(&timer_id_3));
}

#[test]
fn rq_crash_recovery_very_old_timer() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        100,
        50,
        50,
        0,
    )
    .unwrap();

    let result = scan_due_timers(&storage, &instance_id, 1_000_000).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].fire_at_ms, 100);
    assert_eq!(result[0].trigger_time_ms, 50);
    assert_eq!(result[0].duration_ms, 50);
}
