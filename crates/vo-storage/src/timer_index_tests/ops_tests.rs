#![allow(clippy::unwrap_used, clippy::redundant_clone)]
use crate::codec::StorageError;
use crate::timer_index::{scan_due_timers, timer_delete, timer_set, TimerKey, TimerRecord};
use crate::timer_index_tests::{create_instance_id, create_timer_id, MockStorage};
use vo_types::InstanceId;

#[test]
fn fn_timer_set_stores_timer_when_fire_at_ms_greater_than_now_ms() {
    let mut storage = MockStorage::new();
    let instance_id = create_instance_id();
    let timer_id = create_timer_id();
    let fire_at_ms = 1001;
    let now_ms = 1000;
    let trigger_time_ms = 901;
    let duration_ms = 100;

    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        fire_at_ms,
        trigger_time_ms,
        duration_ms,
        now_ms,
    );

    let expected_key = TimerKey::new(fire_at_ms, instance_id, timer_id).unwrap();
    assert_eq!(result, Ok(()));
    assert_eq!(storage.data.len(), 1);
    assert_eq!(
        storage.data.get(expected_key.as_bytes().as_slice()),
        Some(&duration_ms.to_be_bytes().to_vec())
    );
}

#[test]
fn fn_timer_set_overwrites_existing_timer_when_same_key() {
    let mut storage = MockStorage::new();
    let instance_id = create_instance_id();
    let timer_id = create_timer_id();
    let expected_key = TimerKey::new(1001, instance_id.clone(), timer_id.clone()).unwrap();

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1001,
        901,
        100,
        1000,
    )
    .unwrap();
    let result = timer_set(&mut storage, instance_id, timer_id, 1001, 801, 200, 1000);

    assert_eq!(result, Ok(()));
    assert_eq!(storage.data.len(), 1);
    assert_eq!(
        storage.data.get(expected_key.as_bytes().as_slice()),
        Some(&200u64.to_be_bytes().to_vec())
    );
}

#[test]
fn fn_timer_set_rejects_fire_at_ms_equal_to_now_ms() {
    let mut storage = MockStorage::new();
    let result = timer_set(
        &mut storage,
        create_instance_id(),
        create_timer_id(),
        1000,
        900,
        100,
        1000,
    );
    assert_eq!(result, Err(StorageError::InvalidArgument));
}

#[test]
fn fn_timer_set_rejects_zero_duration_ms_exact_variant() {
    let mut storage = MockStorage::new();
    let result = timer_set(
        &mut storage,
        create_instance_id(),
        create_timer_id(),
        1001,
        1001,
        0,
        1000,
    );
    assert_eq!(result, Err(StorageError::InvalidArgument));
}

#[test]
fn fn_scan_due_timers_due_when_fire_at_equals_now() {
    let mut storage = MockStorage::new();
    let instance_id = create_instance_id();
    let timer_id = create_timer_id();
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id,
        1000,
        900,
        100,
        999,
    )
    .unwrap();

    let result = scan_due_timers(&storage, &instance_id, 1000).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].fire_at_ms, 1000);
}

#[test]
fn fn_scan_due_timers_not_due_when_fire_at_greater_than_now() {
    let mut storage = MockStorage::new();
    let instance_id = create_instance_id();
    timer_set(
        &mut storage,
        instance_id.clone(),
        create_timer_id(),
        1001,
        901,
        100,
        1000,
    )
    .unwrap();

    let result = scan_due_timers(&storage, &instance_id, 1000).unwrap();
    assert_eq!(result, vec![]);
}

#[test]
fn fn_timer_delete_removes_existing_timer() {
    let mut storage = MockStorage::new();
    let iid = create_instance_id();
    let tid = create_timer_id();
    let expected_key = TimerKey::new(1001, iid.clone(), tid.clone()).unwrap();
    timer_set(&mut storage, iid.clone(), tid.clone(), 1001, 901, 100, 1000).unwrap();
    assert_eq!(storage.data.len(), 1);
    assert_eq!(
        storage.data.get(expected_key.as_bytes().as_slice()),
        Some(&100u64.to_be_bytes().to_vec())
    );

    let result = timer_delete(&mut storage, &iid, tid, 1001);
    assert_eq!(result, Ok(()));
    assert_eq!(storage.data.len(), 0);
    assert_eq!(storage.data.get(expected_key.as_bytes().as_slice()), None);
}

#[test]
fn fn_timer_set_returns_storage_when_put_fails() {
    let mut storage = MockStorage::new();
    storage.fail_on_op = Some("put".to_string());
    let result = timer_set(
        &mut storage,
        create_instance_id(),
        create_timer_id(),
        1001,
        901,
        100,
        1000,
    );
    assert_eq!(result, Err(StorageError::Storage));
}

#[test]
fn fn_timer_set_rejects_when_dual_clock_invariant_is_broken() {
    let mut storage = MockStorage::new();
    let result = timer_set(
        &mut storage,
        create_instance_id(),
        create_timer_id(),
        1001,
        950,
        100,
        1000,
    );
    assert_eq!(result, Err(StorageError::InvalidArgument));
}

#[test]
fn fn_scan_due_timers_returns_storage_when_scan_fails() {
    let mut storage = MockStorage::new();
    storage.fail_on_op = Some("scan".to_string());
    let result = scan_due_timers(&storage, &create_instance_id(), 1000);
    assert_eq!(result, Err(StorageError::Storage));
}

#[test]
fn fn_scan_due_timers_filters_out_different_instance_id() {
    let mut storage = MockStorage::new();
    let wanted_instance = create_instance_id();
    let other_instance = InstanceId::from_bytes([9; 16]);
    timer_set(
        &mut storage,
        other_instance,
        create_timer_id(),
        1000,
        900,
        100,
        999,
    )
    .unwrap();
    let result = scan_due_timers(&storage, &wanted_instance, 1000).unwrap();
    assert_eq!(result, Vec::<TimerRecord>::new());
}

#[test]
fn fn_scan_due_timers_returns_trigger_time_reconstructed_from_duration() {
    let mut storage = MockStorage::new();
    let instance_id = create_instance_id();
    let timer_id = create_timer_id();
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        2000,
        1500,
        500,
        1499,
    )
    .unwrap();

    let result = scan_due_timers(&storage, &instance_id, 2000).unwrap();
    assert_eq!(
        result,
        vec![TimerRecord {
            timer_id,
            instance_id,
            fire_at_ms: 2000,
            trigger_time_ms: 1500,
            duration_ms: 500,
        }]
    );
}

#[test]
fn fn_scan_due_timers_skips_entry_when_key_length_is_corrupt() {
    let mut storage = MockStorage::new();
    storage
        .data
        .insert(vec![0; 39], 100u64.to_be_bytes().to_vec());
    let result = scan_due_timers(&storage, &create_instance_id(), 1000).unwrap();
    assert_eq!(result, Vec::<TimerRecord>::new());
}

#[test]
fn fn_scan_due_timers_skips_entry_when_value_length_is_corrupt() {
    let mut storage = MockStorage::new();
    let key = TimerKey::new(1000, create_instance_id(), create_timer_id()).unwrap();
    storage.data.insert(key.as_bytes().to_vec(), vec![0; 7]);
    let result = scan_due_timers(&storage, &create_instance_id(), 1000).unwrap();
    assert_eq!(result, Vec::<TimerRecord>::new());
}

#[test]
fn fn_timer_delete_returns_storage_when_delete_fails() {
    let mut storage = MockStorage::new();
    storage.fail_on_op = Some("delete".to_string());
    let result = timer_delete(&mut storage, &create_instance_id(), create_timer_id(), 1001);
    assert_eq!(result, Err(StorageError::Storage));
}

#[test]
fn fn_timer_delete_returns_ok_when_key_is_absent() {
    let mut storage = MockStorage::new();
    let result = timer_delete(&mut storage, &create_instance_id(), create_timer_id(), 1001);
    assert_eq!(result, Ok(()));
}