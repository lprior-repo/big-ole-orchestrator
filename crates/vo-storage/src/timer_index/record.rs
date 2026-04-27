//! Timer record: the decoded representation of a timer entry.
//!
//! Invariant (dual-clock): `fire_at_ms == trigger_time_ms + duration_ms`.

use crate::codec::StorageError;
use vo_types::{InstanceId, TimerId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerRecord {
    pub fire_at_ms: u64,
    pub trigger_time_ms: u64,
    pub duration_ms: u64,
    pub timer_id: TimerId,
    pub instance_id: InstanceId,
}

impl TimerRecord {
    /// Create a minimal `TimerRecord` with only `fire_at_ms`.
    /// Fields `trigger_time_ms` and `duration_ms` default to 0.
    /// `timer_id` and `instance_id` default to nil values.
    #[must_use]
    pub fn new(fire_at_ms: u64) -> Self {
        Self {
            fire_at_ms,
            trigger_time_ms: 0,
            duration_ms: 0,
            timer_id: TimerId::from_bytes([0; 16]),
            instance_id: InstanceId::from_bytes([0; 16]),
        }
    }

    /// Construct a `TimerRecord` from its constituent parts, validating the
    /// dual-clock invariant.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::InvalidArgument` if:
    /// - `duration_ms` is zero
    /// - `fire_at_ms != trigger_time_ms + duration_ms` (dual-clock violation)
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
        if fire_at_ms != trigger_time_ms.saturating_add(duration_ms) {
            return Err(StorageError::InvalidArgument);
        }
        Ok(Self {
            fire_at_ms,
            trigger_time_ms,
            duration_ms,
            timer_id,
            instance_id,
        })
    }
}
