//! Signal matching and wait types per ADR-042.
//!
//! This module defines pure data types for signal routing, wait-state matching,
//! buffer policies, and signal delivery outcomes.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::TimestampMs;
use crate::ParseError;

// ---------------------------------------------------------------------------
// WaitKey — Opaque newtype string for signal wait keys
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WaitKey(pub(crate) String);

impl WaitKey {
    /// Parse a `WaitKey` from a string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError::Empty` if the input is empty.
    /// Returns `ParseError::ExceedsMaxLength` if the input exceeds 256 characters.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "WaitKey";
        const MAX_LEN: usize = 256;
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        if input.chars().count() > MAX_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: TYPE_NAME,
                max: MAX_LEN,
                actual: input.chars().count(),
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WaitKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for WaitKey {
    type Error = ParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<WaitKey> for String {
    fn from(value: WaitKey) -> String {
        value.0
    }
}

// ---------------------------------------------------------------------------
// BufferPolicy — Determines signal buffering behavior per ADR-042 Section 3
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum BufferPolicy {
    /// Return a structured mismatch error when no matching wait is active.
    #[default]
    Reject,
    /// Store exactly one pending signal for the matching key.
    BufferOne,
    /// Store a bounded queue of pending signals for the matching key.
    BufferMany,
}

impl BufferPolicy {
    /// Returns `true` if this policy buffers signals (BufferOne or BufferMany).
    #[must_use]
    pub const fn is_buffering(&self) -> bool {
        matches!(self, Self::BufferOne | Self::BufferMany)
    }
}

// ---------------------------------------------------------------------------
// SignalDelivery — Outcome of attempting to deliver a signal
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignalDelivery {
    /// Signal matched a wait and was consumed.
    Accepted,
    /// Signal did not match (Reject policy) or was a duplicate.
    Rejected,
    /// Signal was buffered for later delivery (BufferOne/BufferMany policy).
    Buffered,
}

impl SignalDelivery {
    /// Returns `true` if signal processing is complete (no further action needed).
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Accepted | Self::Rejected)
    }

    /// Returns `true` if the signal is pending (buffered for future delivery).
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Buffered)
    }
}

// ---------------------------------------------------------------------------
// SignalAddress — Routing address for a signal
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SignalAddress {
    instance_id: crate::InstanceId,
    wait_key: WaitKey,
}

impl SignalAddress {
    /// Create a new `SignalAddress`.
    #[must_use]
    pub fn new(instance_id: crate::InstanceId, wait_key: WaitKey) -> Self {
        Self {
            instance_id,
            wait_key,
        }
    }

    #[must_use]
    pub fn instance_id(&self) -> &crate::InstanceId {
        &self.instance_id
    }

    #[must_use]
    pub fn wait_key(&self) -> &WaitKey {
        &self.wait_key
    }
}

impl fmt::Display for SignalAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}",
            self.instance_id.as_str(),
            self.wait_key.as_str()
        )
    }
}

// ---------------------------------------------------------------------------
// WaitRecord — Record of a workflow currently waiting for a signal
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitRecord {
    instance_id: crate::InstanceId,
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
        instance_id: crate::InstanceId,
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

    #[must_use]
    pub fn instance_id(&self) -> &crate::InstanceId {
        &self.instance_id
    }

    #[must_use]
    pub fn wait_key(&self) -> &WaitKey {
        &self.wait_key
    }

    #[must_use]
    pub fn buffer_policy(&self) -> BufferPolicy {
        self.buffer_policy
    }

    #[must_use]
    pub fn registered_at(&self) -> TimestampMs {
        self.registered_at
    }
}

// ---------------------------------------------------------------------------
// SignalDedupeKey — Dedupe key per ADR-042 Section 4
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SignalDedupeKey {
    lineage_id: crate::InstanceId,
    wait_key: WaitKey,
    command_id: crate::IdempotencyKey,
}

impl SignalDedupeKey {
    /// Create a new `SignalDedupeKey`.
    #[must_use]
    pub fn new(
        lineage_id: crate::InstanceId,
        wait_key: WaitKey,
        command_id: crate::IdempotencyKey,
    ) -> Self {
        Self {
            lineage_id,
            wait_key,
            command_id,
        }
    }

    #[must_use]
    pub fn lineage_id(&self) -> &crate::InstanceId {
        &self.lineage_id
    }

    #[must_use]
    pub fn wait_key(&self) -> &WaitKey {
        &self.wait_key
    }

    #[must_use]
    pub fn command_id(&self) -> &crate::IdempotencyKey {
        &self.command_id
    }
}

#[cfg(test)]
mod tests;
