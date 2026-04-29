//! WaitRecord — Record of a workflow currently waiting for a signal

use serde::{Deserialize, Serialize};

use crate::types::TimestampMs;
use crate::InstanceId;
use crate::ParseError;

use super::buffer_policy::BufferPolicy;
use super::wait_key::WaitKey;

/// Record of a workflow currently waiting for a signal.
///
/// WaitRecord captures the state of a signal wait: which instance is waiting,
/// what key it's waiting for, and what buffer policy applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitRecord {
    instance_id: InstanceId,
    wait_key: WaitKey,
    buffer_policy: BufferPolicy,
    registered_at: TimestampMs,
}

impl WaitRecord {
    /// Create a new `WaitRecord`.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if `wait_key` is empty or exceeds max length.
    pub fn new(
        instance_id: InstanceId,
        wait_key: WaitKey,
        buffer_policy: BufferPolicy,
        registered_at: TimestampMs,
    ) -> Result<Self, ParseError> {
        // WaitKey is already validated at construction, but re-validate the
        // invariant as a defensive check per contract.
        if wait_key.as_str().is_empty() {
            return Err(ParseError::Empty {
                type_name: "WaitKey",
            });
        }
        Ok(Self {
            instance_id,
            wait_key,
            buffer_policy,
            registered_at,
        })
    }

    /// Returns the instance ID.
    #[must_use]
    pub fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    /// Returns the wait key.
    #[must_use]
    pub fn wait_key(&self) -> &WaitKey {
        &self.wait_key
    }

    /// Returns the buffer policy.
    #[must_use]
    pub fn buffer_policy(&self) -> BufferPolicy {
        self.buffer_policy
    }

    /// Returns the registration timestamp.
    #[must_use]
    pub fn registered_at(&self) -> TimestampMs {
        self.registered_at
    }
}
