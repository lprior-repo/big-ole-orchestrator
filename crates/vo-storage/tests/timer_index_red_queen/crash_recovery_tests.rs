use super::helpers::*;
use vo_storage::timer_index::{
    scan_all_timers_for_instance, scan_due_timers, timer_delete, timer_set,
};

// ===========================================================================
// ATTACK VECTOR 9: Crash-recovery timer correctness
// ===========================================================================

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
        1500,
        1000,
        500,
        1000,
    )
    .unwrap();

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_2.clone(),
        2000,
        1500,
        500,
        1000,
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
    assert_eq!(result[0].fire_at_ms, 1000);

    let result = scan_due_timers(&storage, &instance_id, 999).unwrap();
    assert!(result.is_empty(), "Timer at 1000 should not be due at 999");
}

#[test]
fn rq_crash_recovery_multiple_timers_same_fire_time() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_1 = make_test_timer_id(0x02);
    let timer_id_2 = make_test_timer_id(0x03);
    let timer_id_3 = make_test_timer_id(0x04);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_1.clone(),
        2000,
        1500,
        500,
        1000,
    )
    .unwrap();
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_2.clone(),
        2000,
        1500,
        500,
        1000,
    )
    .unwrap();
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_3.clone(),
        2000,
        1500,
        500,
        1000,
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

// ===========================================================================
// ATTACK VECTOR 10: Timer cancellation on instance completion
// ===========================================================================

#[test]
fn rq_timer_cancellation_all_timers_for_instance() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_1 = make_test_timer_id(0x02);
    let timer_id_2 = make_test_timer_id(0x03);
    let timer_id_3 = make_test_timer_id(0x04);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_1.clone(),
        1000,
        500,
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
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_3.clone(),
        5000,
        4500,
        500,
        0,
    )
    .unwrap();

    let all_timers = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
    assert_eq!(all_timers.len(), 3);

    for timer in &all_timers {
        timer_delete(
            &mut storage,
            &instance_id,
            timer.timer_id.clone(),
            timer.fire_at_ms,
        )
        .unwrap();
    }

    let all_timers = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
    assert!(
        all_timers.is_empty(),
        "All timers should be cancelled on completion"
    );

    let due_timers = scan_due_timers(&storage, &instance_id, 10_000).unwrap();
    assert!(due_timers.is_empty());
}

#[test]
fn rq_timer_cancellation_specific_timer_only() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_1 = make_test_timer_id(0x02);
    let timer_id_2 = make_test_timer_id(0x03);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_1.clone(),
        1000,
        500,
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

    timer_delete(&mut storage, &instance_id, timer_id_1, 1000).unwrap();

    let result = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timer_id, timer_id_2);
}

#[test]
fn rq_timer_cancellation_nonexistent_is_idempotent() {
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
        0,
    )
    .unwrap();

    let non_existent_timer_id = make_test_timer_id(0xFF);
    let result = timer_delete(&mut storage, &instance_id, non_existent_timer_id, 1000);
    assert!(result.is_ok());

    let result = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn rq_timer_cancellation_already_fired_is_idempotent() {
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
        0,
    )
    .unwrap();

    let due = scan_due_timers(&storage, &instance_id, 1000).unwrap();
    assert_eq!(due.len(), 1);

    let result = timer_delete(&mut storage, &instance_id, timer_id.clone(), 1000);
    assert!(result.is_ok());

    let due = scan_due_timers(&storage, &instance_id, 1000).unwrap();
    assert!(due.is_empty());
}
