//! SignalDedupeKey — Dedupe key per ADR-042 Section 4

use serde::{Deserialize, Serialize};

use crate::IdempotencyKey;
use crate::InstanceId;

use super::wait_key::WaitKey;

/// Deduplication key for signal delivery per ADR-042 Section 4.
///
/// SignalDedupeKey combines lineage_id, wait_key, and command_id to uniquely
/// identify a signal delivery attempt for deduplication purposes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SignalDedupeKey {
    lineage_id: InstanceId,
    wait_key: WaitKey,
    command_id: IdempotencyKey,
}

impl SignalDedupeKey {
    /// Create a new `SignalDedupeKey`.
    #[must_use]
    pub fn new(lineage_id: InstanceId, wait_key: WaitKey, command_id: IdempotencyKey) -> Self {
        Self {
            lineage_id,
            wait_key,
            command_id,
        }
    }

    /// Returns the lineage ID.
    #[must_use]
    pub fn lineage_id(&self) -> &InstanceId {
        &self.lineage_id
    }

    /// Returns the wait key.
    #[must_use]
    pub fn wait_key(&self) -> &WaitKey {
        &self.wait_key
    }

    /// Returns the command ID.
    #[must_use]
    pub fn command_id(&self) -> &IdempotencyKey {
        &self.command_id
    }
}
