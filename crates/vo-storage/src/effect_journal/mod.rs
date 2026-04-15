//! Effect journal partition — storage interface for managed effect lifecycle (ADR-030).
//!
//! Architecture: Data (`EffectId`, `EffectJournalError`) → Calc (`encode_effect_key`,
//! `decode_effect_key`, `encode_effect_record`, `decode_effect_record`) → Actions
//! (`EffectJournal` trait).
//!
//! This module defines the trait and pure encoding/decoding functions. Concrete Fjall
//! implementations are provided separately.

use vo_types::{EffectRecord, InstanceId};

#[cfg(all(test, feature = "proptest"))]
mod proptests;
#[cfg(test)]
mod red_queen_tests;
#[cfg(test)]
mod tests;
#[cfg(kani)]
mod verification;

// ---------------------------------------------------------------------------
// Data layer — EffectId
// ---------------------------------------------------------------------------

/// Stable identity for a managed effect, scoped to a workflow instance.
///
/// Format: `<instance_id>::<intent_id>` ensures uniqueness within a workflow.
/// INV-EJ-003: same `intent_id` always produces the same `EffectId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EffectId(String);

impl EffectId {
    /// Construct a new `EffectId` from `instance_id` and `intent_id`.
    ///
    /// # Errors
    ///
    /// Returns `EffectJournalError::InvalidArgument` if `intent_id` is empty.
    pub fn new(instance_id: &InstanceId, intent_id: &str) -> Result<Self, EffectJournalError> {
        if intent_id.is_empty() {
            return Err(EffectJournalError::InvalidArgument);
        }
        let id_str = format!("{instance_id}::{intent_id}");
        Ok(Self(id_str))
    }

    /// Returns the underlying string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EffectId {
    type Error = EffectJournalError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(EffectJournalError::InvalidArgument);
        }
        Ok(Self(value))
    }
}

impl From<EffectId> for String {
    #[allow(clippy::use_self)]
    fn from(value: EffectId) -> String {
        value.0
    }
}

// ---------------------------------------------------------------------------
// Data layer — error enum
// ---------------------------------------------------------------------------

/// Errors from the effect journal operations.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum EffectJournalError {
    #[error("effect {effect_id} is already terminal with status {current_status}")]
    AlreadyTerminal {
        effect_id: String,
        current_status: String,
    },
    #[error("effect {effect_id} not found")]
    NotFound { effect_id: String },
    #[error("storage error: {reason}")]
    Storage { reason: String },
    #[error("codec error: {reason}")]
    Codec { reason: String },
    #[error("invalid argument")]
    InvalidArgument,
}

// ---------------------------------------------------------------------------
// Calc layer — key encoding/decoding
// ---------------------------------------------------------------------------

/// Encode an `EffectId` as UTF-8 bytes for use as a partition key.
///
/// The key format is simply the UTF-8 representation of the `EffectId` string.
#[must_use]
pub fn encode_effect_key(effect_id: &EffectId) -> Vec<u8> {
    effect_id.as_str().as_bytes().to_vec()
}

/// Decode UTF-8 bytes into an `EffectId`.
///
/// # Errors
///
/// Returns `EffectJournalError::Codec` if the bytes are not valid UTF-8 or empty.
pub fn decode_effect_key(bytes: &[u8]) -> Result<EffectId, EffectJournalError> {
    let s = std::str::from_utf8(bytes).map_err(|e| EffectJournalError::Codec {
        reason: e.to_string(),
    })?;
    if s.is_empty() {
        return Err(EffectJournalError::Codec {
            reason: "empty effect key".to_string(),
        });
    }
    Ok(EffectId(s.to_string()))
}

// ---------------------------------------------------------------------------
// Calc layer — record encoding/decoding
// ---------------------------------------------------------------------------

/// Encode an `EffectRecord` to JSON bytes for storage.
///
/// # Errors
///
/// Returns `EffectJournalError::Codec` if serialization fails.
pub fn encode_effect_record(record: &EffectRecord) -> Result<Vec<u8>, EffectJournalError> {
    serde_json::to_vec(record).map_err(|e| EffectJournalError::Codec {
        reason: e.to_string(),
    })
}

/// Decode JSON bytes into an `EffectRecord`.
///
/// # Errors
///
/// Returns `EffectJournalError::Codec` if deserialization fails.
pub fn decode_effect_record(bytes: &[u8]) -> Result<EffectRecord, EffectJournalError> {
    serde_json::from_slice(bytes).map_err(|e| EffectJournalError::Codec {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Actions layer — EffectJournal trait
// ---------------------------------------------------------------------------

/// Partition name for the effects journal.
pub const EFFECTS_PARTITION: &str = "effects";

/// Storage interface for managed effect lifecycle (ADR-030).
///
/// Implementations persist effect records and manage state transitions:
/// Prepared → Committed | `RolledBack`.
pub trait EffectJournal {
    /// Prepare (journal) a new effect intent. Returns the assigned `EffectId`.
    ///
    /// Idempotent: same `intent_id` within the same instance returns the same `EffectId`.
    ///
    /// # Errors
    ///
    /// Returns `EffectJournalError::InvalidArgument` if the record's `intent_id` is empty.
    /// Returns `EffectJournalError::Storage` if the underlying storage fails.
    fn prepare(
        &self,
        instance_id: &InstanceId,
        record: EffectRecord,
    ) -> Result<EffectId, EffectJournalError>;

    /// Commit a prepared effect. Transitions Prepared → Committed.
    ///
    /// # Errors
    ///
    /// Returns `EffectJournalError::NotFound` if the `effect_id` does not exist.
    /// Returns `EffectJournalError::AlreadyTerminal` if the effect is in a terminal state.
    /// Returns `EffectJournalError::Storage` if the underlying storage fails.
    fn commit(&self, effect_id: &EffectId) -> Result<(), EffectJournalError>;

    /// Rollback a prepared effect. Transitions Prepared → `RolledBack`.
    ///
    /// # Errors
    ///
    /// Returns `EffectJournalError::NotFound` if the `effect_id` does not exist.
    /// Returns `EffectJournalError::AlreadyTerminal` if the effect is in a terminal state.
    /// Returns `EffectJournalError::Storage` if the underlying storage fails.
    fn rollback(&self, effect_id: &EffectId) -> Result<(), EffectJournalError>;

    /// List all pending (Prepared) effects for a given instance.
    ///
    /// # Errors
    ///
    /// Returns `EffectJournalError::Storage` if the underlying storage fails.
    fn list_pending(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Vec<EffectRecord>, EffectJournalError>;

    /// Compact the journal by removing terminal effects older than the given timestamp.
    ///
    /// Removes all effect records where:
    /// - The effect is in a terminal state (`Committed` or `RolledBack`)
    /// - The `committed_at` timestamp is less than `older_than`
    ///
    /// Returns the number of records removed.
    ///
    /// # Errors
    ///
    /// Returns `EffectJournalError::Storage` if the underlying storage fails.
    fn compact(&self, older_than: vo_types::TimestampMs) -> Result<usize, EffectJournalError>;
}

// ---------------------------------------------------------------------------
// Production implementation
// ---------------------------------------------------------------------------

pub mod fjall_journal;
pub use fjall_journal::FjallEffectJournal;

// ---------------------------------------------------------------------------
// Test infrastructure
// ---------------------------------------------------------------------------

pub mod in_memory_journal;
pub use in_memory_journal::InMemoryEffectJournal;
