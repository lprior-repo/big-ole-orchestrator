use crate::codec::StorageError;
use vo_types::InstanceId;
use vo_types::TimerId;

use super::types::{TimerKey, TimerRecord};

pub type ScanResult = Vec<(Vec<u8>, Vec<u8>)>;

use super::ScanResult;

pub const TIMER_INDEX_PARTITION: &str = "timer_index";

pub trait Storage {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError>;
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError>;
    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError>;
    fn scan(&self, start: &[u8], end: &[u8]) -> Result<ScanResult, StorageError>;
}

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

pub fn timer_delete(
    storage: &mut impl Storage,
    instance_id: &InstanceId,
    timer_id: TimerId,
    fire_at_ms: u64,
) -> Result<(), StorageError> {
    let key = TimerKey::new(fire_at_ms, instance_id.clone(), timer_id)?;
    storage.delete(key.as_bytes())
}

pub fn poll_expired_timers(
    storage: &mut impl Storage,
    instance_id: &InstanceId,
    now_ms: u64,
    max_timers: usize,
) -> Result<Vec<TimerRecord>, StorageError> {
    let start = [0u8; 40];
    let end = {
        let mut e = [0u8; 40];
        e[0..8].copy_from_slice(&(now_ms.saturating_add(1)).to_be_bytes());
        e
    };

    let pairs = storage.scan(&start, &end)?;

    let mut claimed = Vec::with_capacity(max_timers);
    let mut deleted = 0;

    for (k, v) in pairs {
        if deleted >= max_timers {
            break;
        }

        let key_bytes: [u8; 40] = match k.as_slice().try_into() {
            Ok(b) => b,
            Err(_) => continue,
        };
        let key = TimerKey(key_bytes);

        if key.instance_id() != *instance_id {
            continue;
        }

        let fire_at_ms = key.fire_at_ms();
        if fire_at_ms > now_ms {
            continue;
        }

        let duration_bytes: [u8; 8] = match v.try_into() {
            Ok(b) => b,
            Err(_) => continue,
        };
        let duration_ms = u64::from_be_bytes(duration_bytes);
        let trigger_time_ms = fire_at_ms.saturating_sub(duration_ms);

        storage.delete(&k)?;

        deleted += 1;
        claimed.push(TimerRecord {
            timer_id: key.timer_id(),
            instance_id: key.instance_id(),
            fire_at_ms,
            trigger_time_ms,
            duration_ms,
        });
    }

    Ok(claimed)
}
