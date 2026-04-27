//! Timer index API: set, scan, delete operations using the `Storage` trait.

use crate::codec::StorageError;
use crate::timer_index::key::TimerKey;
use crate::timer_index::record::TimerRecord;
use crate::timer_index::storage::Storage;
use crate::timer_index::value::TimerValue;
use crate::timer_index::ScanResult;
use vo_types::{InstanceId, TimerId};

/// Store a timer entry.
///
/// Validates:
/// - `fire_at_ms > now_ms` (timer must be in the future)
/// - `duration_ms > 0`
/// - `fire_at_ms == trigger_time_ms + duration_ms` (dual-clock invariant)
///
/// # Errors
///
/// Returns `StorageError::InvalidArgument` if any validation fails, or
/// `StorageError::Storage` if the underlying storage fails.
#[allow(clippy::too_many_arguments)]
pub fn timer_set(
    storage: &mut dyn Storage,
    instance_id: InstanceId,
    timer_id: TimerId,
    fire_at_ms: u64,
    trigger_time_ms: u64,
    duration_ms: u64,
    now_ms: u64,
) -> Result<(), StorageError> {
    // Validate fire_at_ms is in the future
    if fire_at_ms <= now_ms {
        return Err(StorageError::InvalidArgument);
    }
    // Validate duration is non-zero
    if duration_ms == 0 {
        return Err(StorageError::InvalidArgument);
    }
    // Validate dual-clock invariant: fire_at_ms == trigger_time_ms + duration_ms
    if fire_at_ms != trigger_time_ms.saturating_add(duration_ms) {
        return Err(StorageError::InvalidArgument);
    }

    let key = TimerKey::new(fire_at_ms, instance_id, timer_id)?;
    let value = TimerValue::new(duration_ms)?;
    storage.put(key.as_bytes(), &value.as_be_bytes())
}

/// Delete a timer entry by its identifying fields.
///
/// # Errors
///
/// Returns `StorageError::Storage` if the underlying storage fails.
pub fn timer_delete(
    storage: &mut dyn Storage,
    instance_id: &InstanceId,
    timer_id: TimerId,
    fire_at_ms: u64,
) -> Result<(), StorageError> {
    let key = TimerKey::new(fire_at_ms, instance_id.clone(), timer_id)?;
    storage.delete(key.as_bytes())
}

/// Scan for all timers due at or before `now_ms` for a given `instance_id`.
///
/// Returns timer records sorted by key order (`fire_at_ms` ascending).
///
/// # Errors
///
/// Returns `StorageError::Storage` if the underlying storage fails.
pub fn scan_due_timers(
    storage: &dyn Storage,
    instance_id: &InstanceId,
    now_ms: u64,
) -> Result<ScanResult, StorageError> {
    // Build prefix range for this instance:
    // Start: fire_at_ms=0, instance_id
    // End: fire_at_ms=now_ms+1, same instance_id
    let id_bytes = instance_id.to_bytes().map_err(|_| StorageError::CorruptKey)?;

    // Start key: fire_at_ms=0 + instance_id bytes
    let mut start = vec![0u8; 24];
    // fire_at_ms = 0 (8 bytes of zeros, already set)
    start[8..24].copy_from_slice(&id_bytes);

    // End key: fire_at_ms = now_ms + 1 + instance_id bytes
    let end_fire = (now_ms + 1).to_be_bytes();
    let mut end = vec![0u8; 24];
    end[0..8].copy_from_slice(&end_fire);
    end[8..24].copy_from_slice(&id_bytes);

    let raw = storage.scan(&start, &end)?;
    let mut results = Vec::with_capacity(raw.len());
    for (k, v) in raw {
        if let Some(record) = decode_timer_record(&k, &v)? {
            results.push(record);
        }
    }
    Ok(results)
}

/// Scan for all timers belonging to a given `instance_id`, regardless of fire time.
///
/// # Errors
///
/// Returns `StorageError::Storage` if the underlying storage fails.
pub fn scan_all_timers_for_instance(
    storage: &dyn Storage,
    instance_id: &InstanceId,
) -> Result<ScanResult, StorageError> {
    let id_bytes = instance_id.to_bytes().map_err(|_| StorageError::CorruptKey)?;

    // Start key: fire_at_ms=0 + instance_id bytes + timer_id=0
    let mut start = vec![0u8; 40];
    start[8..24].copy_from_slice(&id_bytes);

    // End key: fire_at_ms=MAX + instance_id bytes + timer_id=MAX
    let mut end = vec![0xFFu8; 40];
    end[8..24].copy_from_slice(&id_bytes);

    let raw = storage.scan(&start, &end)?;
    let mut results = Vec::with_capacity(raw.len());
    for (k, v) in raw {
        if let Some(record) = decode_timer_record(&k, &v)? {
            results.push(record);
        }
    }
    Ok(results)
}

/// Poll for expired timers. Stub: returns empty result.
///
/// # Errors
///
/// Returns `StorageError::Storage` if the underlying storage fails.
#[allow(dead_code)]
pub fn poll_expired_timers(
    storage: &dyn Storage,
    now_ms: u64,
    max_count: usize,
) -> Result<ScanResult, StorageError> {
    let _ = (storage, now_ms, max_count);
    Ok(vec![])
}

/// Decode a raw key-value pair into a `TimerRecord`, if possible.
/// Returns `Ok(None)` if the key/value cannot be decoded (e.g., wrong length).
fn decode_timer_record(key: &[u8], value: &[u8]) -> Result<Option<TimerRecord>, StorageError> {
    if key.len() != 40 || value.len() != 8 {
        return Ok(None);
    }

    let fire_at_ms = u64::from_be_bytes([
        key[0], key[1], key[2], key[3], key[4], key[5], key[6], key[7],
    ]);

    let instance_id = InstanceId::from_bytes([
        key[8], key[9], key[10], key[11], key[12], key[13], key[14], key[15], key[16], key[17],
        key[18], key[19], key[20], key[21], key[22], key[23],
    ]);

    let timer_id = TimerId::from_bytes([
        key[24], key[25], key[26], key[27], key[28], key[29], key[30], key[31], key[32], key[33],
        key[34], key[35], key[36], key[37], key[38], key[39],
    ]);

    let duration_ms = u64::from_be_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]);

    let trigger_time_ms = fire_at_ms.saturating_sub(duration_ms);

    Ok(Some(TimerRecord {
        fire_at_ms,
        trigger_time_ms,
        duration_ms,
        timer_id,
        instance_id,
    }))
}
