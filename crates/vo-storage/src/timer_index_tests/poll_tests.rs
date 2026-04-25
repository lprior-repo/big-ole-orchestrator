#![allow(clippy::unwrap_used, clippy::redundant_clone)]
use crate::timer_index::{poll_expired_timers, timer_set};
use crate::timer_index_tests::{create_instance_id, create_timer_id, MockStorage};

#[test]
fn fn_poll_expired_timers_returns_only_unclaimed_expired_timers() {
    let mut storage = MockStorage::new();
    let instance_id = create_instance_id();
    let timer_id_1 = create_timer_id();
    let timer_id_2 = create_timer_id();

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

    let result = poll_expired_timers(&mut storage, &instance_id, 1500, 10).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timer_id, timer_id_1);
    assert_eq!(result[0].fire_at_ms, 1000);
}

#[test]
fn fn_poll_expired_timers_does_not_return_already_claimed_timers() {
    let mut storage = MockStorage::new();
    let instance_id = create_instance_id();
    let timer_id = create_timer_id();

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

    let first_poll = poll_expired_timers(&mut storage, &instance_id, 1500, 10).unwrap();
    assert_eq!(first_poll.len(), 1);

    let second_poll = poll_expired_timers(&mut storage, &instance_id, 1500, 10).unwrap();
    assert_eq!(second_poll.len(), 0);
}

#[test]
fn fn_poll_expired_timers_respects_max_timers_parameter() {
    let mut storage = MockStorage::new();
    let instance_id = create_instance_id();
    let timer_id_1 = create_timer_id();
    let timer_id_2 = create_timer_id();
    let timer_id_3 = create_timer_id();

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
        1001,
        501,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_3.clone(),
        1002,
        502,
        500,
        0,
    )
    .unwrap();

    let result = poll_expired_timers(&mut storage, &instance_id, 2000, 2).unwrap();
    assert_eq!(result.len(), 2);

    let remaining = poll_expired_timers(&mut storage, &instance_id, 2000, 10).unwrap();
    assert_eq!(remaining.len(), 1);
}