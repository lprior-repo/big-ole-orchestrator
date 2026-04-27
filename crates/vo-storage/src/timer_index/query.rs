use crate::codec::StorageError;
use vo_types::InstanceId;

use super::storage::Storage;
use super::types::{TimerKey, TimerRecord};

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
            let key_bytes: [u8; 40] = k.as_slice().try_into().ok()?;
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
            let key_bytes: [u8; 40] = k.as_slice().try_into().ok()?;
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
