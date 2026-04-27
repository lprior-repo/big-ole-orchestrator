//! Timer API: persistent timer CRUD over the fjall `timers` partition.
//!
//! Keyspace: `timers`
//! Key encoding: `<fire_at_ms:8BE><instance_id:16><timer_id:16>` (40 bytes)
//! Value encoding: `<duration_ms:8BE>` (8 bytes)
//!
//! The key's prefix-sort order (fire_at_ms first) enables range scans for
//! finding expired timers and the reanimator loop.

use crate::codec::StorageError;
use crate::partitions::TIMERS_PARTITION;
use crate::timer_index::{ScanResult, TimerKey, TimerValue};
use vo_types::{InstanceId, TimerId};

/// Inserts a timer entry into the `timers` keyspace.
///
/// The timer key is constructed from `fire_at_ms`, `instance_id`, and `timer_id`.
/// The value stores the duration in milliseconds.
///
/// # Errors
///
/// Returns `StorageError::InvalidArgument` if the key or value cannot be constructed.
pub fn timer_set(
    db: &fjall::Database,
    fire_at_ms: u64,
    instance_id: &InstanceId,
    timer_id: &TimerId,
) -> Result<(), StorageError> {
    let key = TimerKey::new(fire_at_ms, instance_id.clone(), timer_id.clone())?;

    let value = TimerValue::new(fire_at_ms)
        .map_err(|_| StorageError::InvalidArgument)?;

    let keyspace = db.keyspace(TIMERS_PARTITION, || fjall::KeyspaceCreateOptions::default())?;
    keyspace.insert(key.as_bytes(), &value.as_be_bytes())?;
    Ok(())
}

/// Removes a timer entry from the `timers` keyspace.
///
/// # Errors
///
/// Returns `StorageError` if the keyspace cannot be opened.
pub fn timer_delete(db: &fjall::Database, key: &[u8]) -> Result<(), StorageError> {
    let keyspace = db.keyspace(TIMERS_PARTITION, || fjall::KeyspaceCreateOptions::default())?;
    keyspace.remove(key)?;
    Ok(())
}

/// Scans the `timers` partition for all entries with `fire_at_ms <= now_ms`.
///
/// Returns an iterator over `(key, value)` pairs sorted by key (ascending).
/// Because the key starts with `fire_at_ms` (BE), this returns the oldest
/// due timers first — the correct order for the reanimator loop.
///
/// # Errors
///
/// Returns `StorageError` if the keyspace cannot be opened or the scan fails.
pub fn scan_due_timers(db: &fjall::Database, now_ms: u64) -> Result<ScanResult, StorageError> {
    let keyspace = db.keyspace(TIMERS_PARTITION, || fjall::KeyspaceCreateOptions::default())?;

    let mut result = Vec::new();

    for item in keyspace.iter() {
        let (k, v) = item.into_inner().map_err(|_| StorageError::FjallError)?;
        if k.len() < 8 {
            continue;
        }
        let item_fire_at = u64::from_be_bytes(k[..8].try_into().map_err(|_| StorageError::CorruptKey)?);
        if item_fire_at > now_ms {
            break;
        }
        result.push((k.to_vec(), v.to_vec()));
    }

    Ok(result)
}

/// Atomically polls and removes expired timer entries from the `timers` keyspace.
///
/// This is the core function for the reanimator loop. It:
/// 1. Scans for due timers (fire_at_ms <= now_ms)
/// 2. Removes each found timer key
/// 3. Returns the removed entries
///
/// If the batch commit fails partway, some timers may have been removed without
/// being returned. Callers should handle this idempotently (re-firing is a no-op
/// since the timer was already removed).
///
/// # Arguments
///
/// * `db` - The fjall database handle.
/// * `now_ms` - Current timestamp in milliseconds.
/// * `max_count` - Maximum number of timers to poll (prevents long scans).
///
/// # Errors
///
/// Returns `StorageError` on keyspace or batch operation failures.
pub fn poll_expired_timers(
    db: &fjall::Database,
    now_ms: u64,
    max_count: usize,
) -> Result<ScanResult, StorageError> {
    let keyspace = db.keyspace(TIMERS_PARTITION, || fjall::KeyspaceCreateOptions::default())?;

    let mut result = Vec::with_capacity(max_count);

    // Use a WriteBatch for atomic removal
    let mut batch = db.batch();

    for item in keyspace.iter() {
        if result.len() >= max_count {
            break;
        }

        let (k, _v) = item.into_inner().map_err(|_| StorageError::FjallError)?;
        let item_fire_at = u64::from_be_bytes(k[..8].try_into().map_err(|_| StorageError::CorruptKey)?);
        if item_fire_at > now_ms {
            break;
        }

        batch.remove(&keyspace, &*k);
        result.push((k.to_vec(), Vec::new()));
    }

    if !result.is_empty() {
        batch.commit()?;
    }

    Ok(result)
}

/// Scans the `timers` partition for all entries matching a given instance ID.
///
/// Returns an iterator over `(key, value)` pairs for all timers associated
/// with the specified `instance_id`, regardless of fire time.
///
/// # Errors
///
/// Returns `StorageError` if the keyspace cannot be opened or the scan fails.
pub fn scan_all_timers_for_instance(
    db: &fjall::Database,
    instance_id: &[u8],
) -> Result<ScanResult, StorageError> {
    let keyspace = db.keyspace(TIMERS_PARTITION, || fjall::KeyspaceCreateOptions::default())?;

    // Build the instance ID prefix (16 bytes, offset 8 in key)
    if instance_id.len() != 16 {
        return Ok(vec![]);
    }

    let mut result = Vec::new();

    // Scan all keys and filter by instance_id at offset 8..24
    for item in keyspace.iter() {
        let (k, v) = item.into_inner().map_err(|_| StorageError::FjallError)?;
        if k.len() < 24 {
            continue;
        }
        if k[8..24] == instance_id[..16] {
            result.push((k.to_vec(), v.to_vec()));
        }
    }

    Ok(result)
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
