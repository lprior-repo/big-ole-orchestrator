use crate::helpers::{
    make_test_instance_id, make_test_timer_id, scan_all_timers_for_instance, scan_due_timers,
    timer_delete, timer_set, MockStorage,
};

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