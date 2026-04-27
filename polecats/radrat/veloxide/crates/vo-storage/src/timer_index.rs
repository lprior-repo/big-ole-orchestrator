use crate::codec::StorageError;
use vo_types::{InstanceId, TimerId};

type ScanResult = Vec<(Vec<u8>, Vec<u8>)>;

pub struct TimerKey([u8; 40]);

impl TimerKey {
    /// Creates a new `TimerKey` from fire time, instance ID, and timer ID.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::InvalidArgument` if `instance_id` or `timer_id` cannot be converted to bytes.
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        fire_at_ms: u64,
        instance_id: InstanceId,
        timer_id: TimerId,
    ) -> Result<Self, StorageError> {
        let mut bytes = [0u8; 40];
        bytes[0..8].copy_from_slice(&fire_at_ms.to_be_bytes());
        bytes[8..24].copy_from_slice(
            &instance_id
                .to_bytes()
                .map_err(|_| StorageError::InvalidArgument)?,
        );
        bytes[24..40].copy_from_slice(
            &timer_id
                .to_bytes()
                .map_err(|_| StorageError::InvalidArgument)?,
        );
        Ok(Self(bytes))
    }
    #[must_use]
    pub const fn fire_at_ms(&self) -> u64 {
        u64::from_be_bytes([
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7],
        ])
    }
    #[must_use]
    pub fn instance_id(&self) -> InstanceId {
        InstanceId::from_bytes(self.0[8..24].try_into().unwrap_or_default())
    }
    #[must_use]
    pub fn timer_id(&self) -> TimerId {
        TimerId::from_bytes(self.0[24..40].try_into().unwrap_or_default())
    }
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 40] {
        &self.0
    }
}

pub struct TimerValue(u64);

impl TimerValue {
    /// Creates a new `TimerValue` wrapping the given duration in milliseconds.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::InvalidArgument` if `duration_ms` is zero.
    pub const fn new(duration_ms: u64) -> Result<Self, StorageError> {
        if duration_ms == 0 {
            return Err(StorageError::InvalidArgument);
        }
        Ok(Self(duration_ms))
    }
    #[must_use]
    pub const fn duration_ms(&self) -> u64 {
        self.0
    }
    #[must_use]
    pub const fn as_be_bytes(&self) -> [u8; 8] {
        self.0.to_be_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerRecord {
    pub timer_id: TimerId,
    pub instance_id: InstanceId,
    pub fire_at_ms: u64,
    pub trigger_time_ms: u64,
    pub duration_ms: u64,
}

impl TimerRecord {
    /// Constructs a `TimerRecord` from individual parts.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::InvalidArgument` if `duration_ms` is zero or if dual-clock invariant is violated.
    pub fn try_from_parts(
        timer_id: TimerId,
        instance_id: InstanceId,
        fire_at_ms: u64,
        trigger_time_ms: u64,
        duration_ms: u64,
    ) -> Result<Self, StorageError> {
        if duration_ms == 0 {
            return Err(StorageError::InvalidArgument);
        }
        // Dual-clock verification
        if fire_at_ms != trigger_time_ms.saturating_add(duration_ms) {
            return Err(StorageError::InvalidArgument);
        }
        Ok(Self {
            timer_id,
            instance_id,
            fire_at_ms,
            trigger_time_ms,
            duration_ms,
        })
    }
}

pub trait Storage {
    /// Stores a key-value pair.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the underlying storage fails.
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError>;
    /// Retrieves a value by key.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the underlying storage fails.
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError>;
    /// Deletes a key.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the underlying storage fails.
    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError>;
    /// Scans a range of keys.
    ///
    /// # Errors
    ///
    /// Returns `StorageError` if the underlying storage fails.
    fn scan(&self, start: &[u8], end: &[u8]) -> Result<ScanResult, StorageError>;
}

/// Stores a timer in the timers partition.
///
/// # Errors
///
/// Returns `StorageError::InvalidArgument` if `fire_at_ms` <= `now_ms`, `duration_ms` is zero,
/// or if dual-clock invariant is violated.
pub fn timer_set(
    storage: &mut impl Storage,
    instance_id: InstanceId,
    timer_id: TimerId,
    fire_at_ms: u64,
    trigger_time_ms: u64,
    duration_ms: u64,
    now_ms: u64,
) -> Result<(), StorageError> {
    if fire_at_ms <= now_ms {
        return Err(StorageError::InvalidArgument);
    }
    if duration_ms == 0 {
        return Err(StorageError::InvalidArgument);
    }
    if fire_at_ms != trigger_time_ms.saturating_add(duration_ms) {
        return Err(StorageError::InvalidArgument);
    }

    let key = TimerKey::new(fire_at_ms, instance_id, timer_id)?;
    let value = duration_ms.to_be_bytes();
    storage.put(key.as_bytes(), &value)
}

/// Scans the timers partition for due timers for a specific instance.
///
/// # Errors
///
/// Returns `StorageError::CorruptKey` if timer key or value bytes cannot be decoded.
pub fn scan_due_timers(
    storage: &impl Storage,
    instance_id: &InstanceId,
    now_ms: u64,
) -> Result<Vec<TimerRecord>, StorageError> {
    let start = [0u8; 40];
    let end = {
        let mut e = [0u8; 40];
        e[0..8].copy_from_slice(&(now_ms.saturating_add(1)).to_be_bytes());
        e
    };

    let pairs = storage.scan(&start, &end)?;
    let records: Vec<TimerRecord> = pairs
        .into_iter()
        .filter_map(|(k, v)| {
            let key_bytes: [u8; 40] = k.try_into().ok()?;
            let key = TimerKey(key_bytes);
            if key.instance_id() != *instance_id {
                return None;
            }
            let fire_at_ms = key.fire_at_ms();
            if fire_at_ms > now_ms {
                return None;
            }
            let duration_bytes: [u8; 8] = v.try_into().ok()?;
            let duration_ms = u64::from_be_bytes(duration_bytes);
            let trigger_time_ms = fire_at_ms.saturating_sub(duration_ms);
            Some(TimerRecord {
                timer_id: key.timer_id(),
                instance_id: key.instance_id(),
                fire_at_ms,
                trigger_time_ms,
                duration_ms,
            })
        })
        .collect();
    Ok(records)
}

/// Deletes a timer from the timers partition.
///
/// # Errors
///
/// Returns `StorageError` if the underlying storage operation fails.
pub fn timer_delete(
    storage: &mut impl Storage,
    instance_id: &InstanceId,
    timer_id: TimerId,
    fire_at_ms: u64,
) -> Result<(), StorageError> {
    let key = TimerKey::new(fire_at_ms, instance_id.clone(), timer_id)?;
    storage.delete(key.as_bytes())
}

/// Scans for ALL timers (both due and future) for a specific instance.
///
/// This is used when cancelling an instance to ensure ALL timers (including future ones)
/// are properly cleaned up.
///
/// # Errors
///
/// Returns `StorageError::CorruptKey` if timer key or value bytes cannot be decoded.
pub fn scan_all_timers_for_instance(
    storage: &impl Storage,
    instance_id: &InstanceId,
) -> Result<Vec<TimerRecord>, StorageError> {
    let instance_bytes = instance_id
        .to_bytes()
        .map_err(|_| StorageError::InvalidArgument)?;

    let start = {
        let mut s = [0u8; 40];
        s[8..24].copy_from_slice(&instance_bytes);
        s
    };

    let end = {
        let mut e = [0u8; 40];
        e[0..8].copy_from_slice(&u64::MAX.to_be_bytes());
        e[8..24].copy_from_slice(&instance_bytes);
        e[24..40].copy_from_slice(&[0xFFu8; 16]);
        e
    };

    let pairs = storage.scan(&start, &end)?;
    let records: Vec<TimerRecord> = pairs
        .into_iter()
        .filter_map(|(k, v)| {
            let key_bytes: [u8; 40] = k.try_into().ok()?;
            let key = TimerKey(key_bytes);
            if key.instance_id() != *instance_id {
                return None;
            }
            let fire_at_ms = key.fire_at_ms();
            let duration_bytes: [u8; 8] = v.try_into().ok()?;
            let duration_ms = u64::from_be_bytes(duration_bytes);
            let trigger_time_ms = fire_at_ms.saturating_sub(duration_ms);
            Some(TimerRecord {
                timer_id: key.timer_id(),
                instance_id: key.instance_id(),
                fire_at_ms,
                trigger_time_ms,
                duration_ms,
            })
        })
        .collect();
    Ok(records)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::redundant_clone)]
    use super::*;
    use proptest::prelude::*;

    // Mock storage for tests
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

    // Helper to create IDs
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
    fn fn_scan_all_timers_for_instance_returns_all_timers_including_future() {
        let mut storage = MockStorage::new();
        let instance_id = create_instance_id();
        let timer_id_1 = create_timer_id();
        let timer_id_2 = TimerId::from_bytes([3; 16]);

        timer_set(
            &mut storage,
            instance_id.clone(),
            timer_id_1.clone(),
            500,
            400,
            100,
            0,
        )
        .unwrap();

        timer_set(
            &mut storage,
            instance_id.clone(),
            timer_id_2.clone(),
            2000,
            1900,
            100,
            0,
        )
        .unwrap();

        let result = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn fn_scan_all_timers_for_instance_filters_out_different_instance() {
        let mut storage = MockStorage::new();
        let wanted_instance = create_instance_id();
        let other_instance = InstanceId::from_bytes([9; 16]);

        timer_set(
            &mut storage,
            other_instance,
            create_timer_id(),
            500,
            400,
            100,
            0,
        )
        .unwrap();

        let result = scan_all_timers_for_instance(&storage, &wanted_instance).unwrap();
        assert_eq!(result, Vec::<TimerRecord>::new());
    }

    #[test]
    fn fn_scan_all_timers_for_instance_returns_both_due_and_future_timers() {
        let mut storage = MockStorage::new();
        let instance_id = create_instance_id();
        let timer_id_due = create_timer_id();
        let timer_id_future = TimerId::from_bytes([3; 16]);

        timer_set(
            &mut storage,
            instance_id.clone(),
            timer_id_due.clone(),
            500,
            400,
            100,
            0,
        )
        .unwrap();

        timer_set(
            &mut storage,
            instance_id.clone(),
            timer_id_future.clone(),
            2000,
            1900,
            100,
            0,
        )
        .unwrap();

        let result = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
        assert_eq!(result.len(), 2);

        let fire_times: Vec<u64> = result.iter().map(|r| r.fire_at_ms).collect();
        assert!(fire_times.contains(&500));
        assert!(fire_times.contains(&2000));
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
}
