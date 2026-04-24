#![allow(clippy::unwrap_used, clippy::redundant_clone)]
use super::*;
use proptest::prelude::*;

struct MockStorage {
    data: std::collections::BTreeMap<Vec<u8>, Vec<u8>>,
    fail_on_op: Option<String>,
}

impl MockStorage {
    fn new() -> Self {
        Self {
            data: std::collections::BTreeMap::new(),
            fail_on_op: None,
        }
    }
}

impl Storage for MockStorage {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        if self.fail_on_op.as_deref() == Some("put") {
            return Err(StorageError::Storage);
        }
        self.data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        if self.fail_on_op.as_deref() == Some("get") {
            return Err(StorageError::Storage);
        }
        Ok(self.data.get(key).cloned())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError> {
        if self.fail_on_op.as_deref() == Some("delete") {
            return Err(StorageError::Storage);
        }
        self.data.remove(key);
        Ok(())
    }

    fn scan(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        if self.fail_on_op.as_deref() == Some("scan") {
            return Err(StorageError::Storage);
        }
        Ok(self
            .data
            .range(start.to_vec()..end.to_vec())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
}

fn create_instance_id() -> InstanceId {
    InstanceId::from_bytes([1; 16])
}

fn create_timer_id() -> TimerId {
    TimerId::from_bytes([2; 16])
}

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
fn fn_timer_key_new_encodes_bytes_correctly() {
    let key = TimerKey::new(1234, create_instance_id(), create_timer_id()).unwrap();
    assert_eq!(key.as_bytes().len(), 40);
    assert_eq!(key.fire_at_ms(), 1234);
}

#[test]
fn fn_timer_key_instance_id_returns_original_instance_id() {
    let instance_id = create_instance_id();
    let key = TimerKey::new(1234, instance_id.clone(), create_timer_id()).unwrap();
    assert_eq!(key.instance_id(), instance_id);
}

#[test]
fn fn_timer_key_timer_id_returns_original_timer_id() {
    let timer_id = create_timer_id();
    let key = TimerKey::new(1234, create_instance_id(), timer_id.clone()).unwrap();
    assert_eq!(key.timer_id(), timer_id);
}

#[test]
fn fn_timer_value_returns_invalid_argument_when_duration_is_zero() {
    assert_eq!(
        TimerValue::new(0).map(|value| value.duration_ms()),
        Err(StorageError::InvalidArgument)
    );
}

#[test]
fn fn_timer_value_returns_duration_when_duration_is_non_zero() {
    let value = TimerValue::new(250).unwrap();
    assert_eq!(value.duration_ms(), 250);
}

#[test]
fn fn_timer_value_returns_big_endian_bytes_for_duration() {
    let value = TimerValue::new(0x0102_0304_0506_0708).unwrap();
    assert_eq!(value.as_be_bytes(), [1, 2, 3, 4, 5, 6, 7, 8]);
}

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

proptest! {
    #[test]
    fn fn_proptest_timer_key_ordering_preserves_lexicographic_order(a in 0u64..1000, b in 1001u64..2000) {
        let iid = create_instance_id();
        let tid = create_timer_id();
        let key_a = TimerKey::new(a, iid.clone(), tid.clone()).unwrap();
        let key_b = TimerKey::new(b, iid, tid).unwrap();
        prop_assert!(key_a.as_bytes() < key_b.as_bytes());
    }
}
