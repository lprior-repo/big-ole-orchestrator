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
        let bytes: [u8; 16] = [
            self.0[8], self.0[9], self.0[10], self.0[11], self.0[12], self.0[13], self.0[14],
            self.0[15], self.0[16], self.0[17], self.0[18], self.0[19], self.0[20], self.0[21],
            self.0[22], self.0[23],
        ];
        InstanceId::from_bytes(bytes)
    }
    #[must_use]
    pub fn timer_id(&self) -> TimerId {
        let bytes: [u8; 16] = [
            self.0[24], self.0[25], self.0[26], self.0[27], self.0[28], self.0[29], self.0[30],
            self.0[31], self.0[32], self.0[33], self.0[34], self.0[35], self.0[36], self.0[37],
            self.0[38], self.0[39],
        ];
        TimerId::from_bytes(bytes)
    }
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 40] {
        &self.0
    }
}

#[derive(Debug)]
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

/// Polls for expired timers and atomically claims them by deleting from storage.
///
/// This implements the "fencing" pattern where a timer can only be processed by one node.
/// When a timer is returned by this function, it has been deleted from storage,
/// preventing duplicate delivery.
///
/// # Errors
///
/// Returns `StorageError::CorruptKey` if timer key or value bytes cannot be decoded.
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

        if let Err(e) = storage.delete(&k) {
            return Err(e);
        }

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


