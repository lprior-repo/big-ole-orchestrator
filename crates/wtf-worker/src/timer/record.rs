use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use wtf_common::{InstanceId, NamespaceId, TimerId, WtfError};

/// A pending timer stored in the `wtf-timers` KV bucket.
///
/// Serialized as msgpack. The KV key is the `timer_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerRecord {
    /// Unique timer ID.
    pub timer_id: TimerId,
    /// Namespace of the owning workflow instance.
    pub namespace: NamespaceId,
    /// Instance ID that scheduled the timer.
    pub instance_id: InstanceId,
    /// UTC timestamp when the timer should fire.
    pub fire_at: DateTime<Utc>,
}

impl TimerRecord {
    /// Serialize to msgpack bytes for KV storage.
    ///
    /// # Errors
    /// Returns `WtfError::NatsPublish` if serialization fails.
    pub fn to_msgpack(&self) -> Result<Bytes, WtfError> {
        rmp_serde::to_vec_named(self)
            .map(Bytes::from)
            .map_err(|e| WtfError::nats_publish(format!("serialize TimerRecord: {e}")))
    }

    /// Deserialize from msgpack bytes.
    ///
    /// # Errors
    /// Returns `WtfError::NatsPublish` if deserialization fails.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, WtfError> {
        rmp_serde::from_slice(bytes)
            .map_err(|e| WtfError::nats_publish(format!("deserialize TimerRecord: {e}")))
    }

    /// Whether this timer is due to fire at or before `now`.
    #[must_use]
    pub fn is_due(&self, now: DateTime<Utc>) -> bool {
        self.fire_at <= now
    }
}
