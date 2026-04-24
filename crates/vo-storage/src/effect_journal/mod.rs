//! Effect journal partition — storage interface for managed effect lifecycle (ADR-030).
//!
//! Architecture: Data (`EffectId`, `EffectJournalError`) → Calc (`encode_effect_key`,
//! `decode_effect_key`, `encode_effect_record`, `decode_effect_record`) → Actions
//! (`EffectJournal` trait).
//!
//! This module defines the trait and pure encoding/decoding functions. Concrete Fjall
//! implementations are provided separately.

use crate::key_encoding::{
    decode_instance_id, decode_length_prefixed, encode_instance_id, encode_length_prefixed,
};
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
// Calc layer — key encoding/decoding (ADR-020 binary format)
// ---------------------------------------------------------------------------
//
// Key format: [instance_id(16)][intent_id_len_u16_be][intent_id_bytes]
//
// ADR-020 mandates: instance ID as fixed 16-byte binary, variable identifiers
// length-prefixed. The intent_id is the variable component in effect keys.

/// Encode an `EffectId` as binary key bytes (ADR-020).
///
/// Format: `[instance_id(16)][intent_id_len_u16_be][intent_id_bytes]`
///
/// Parses the `instance_id` and `intent_id` from the `EffectId` string representation.
#[must_use]
pub fn encode_effect_key(effect_id: &EffectId) -> Vec<u8> {
    let s = effect_id.as_str();
    // Parse "instance_id::intent_id" from the EffectId string
    if let Some((iid_str, intent_id)) = s.split_once("::") {
        if let Ok(instance_id) = InstanceId::parse(iid_str) {
            if let Ok(iid_bytes) = encode_instance_id(&instance_id) {
                let intent_bytes = intent_id.as_bytes();
                let mut key = Vec::with_capacity(16 + 2 + intent_bytes.len());
                key.extend_from_slice(&iid_bytes);
                key.extend_from_slice(&(intent_bytes.len() as u16).to_be_bytes());
                key.extend_from_slice(intent_bytes);
                return key;
            }
        }
    }
    // Fallback: length-prefixed raw string (for migration safety)
    encode_length_prefixed(s.as_bytes())
}

/// Decode binary effect key bytes into an `EffectId` (ADR-020).
///
/// Expects format: `[instance_id(16)][intent_id_len_u16_be][intent_id_bytes]`.
/// Falls back to length-prefixed string for migration compatibility.
///
/// # Errors
///
/// Returns `EffectJournalError::Codec` if the bytes are malformed or empty.
pub fn decode_effect_key(bytes: &[u8]) -> Result<EffectId, EffectJournalError> {
    if bytes.len() < 18 {
        // Try length-prefixed fallback
        if let Ok((decoded, _)) = decode_length_prefixed(bytes) {
            let s = std::str::from_utf8(decoded).map_err(|e| EffectJournalError::Codec {
                reason: e.to_string(),
            })?;
            if !s.is_empty() {
                return Ok(EffectId(s.to_string()));
            }
        }
        return Err(EffectJournalError::Codec {
            reason: format!("effect key too short: {} bytes", bytes.len()),
        });
    }
    let instance_id = decode_instance_id(&bytes[..16]).map_err(|e| EffectJournalError::Codec {
        reason: format!("invalid instance_id in effect key: {e}"),
    })?;
    let intent_len = u16::from_be_bytes([bytes[16], bytes[17]]) as usize;
    if bytes.len() < 18 + intent_len {
        return Err(EffectJournalError::Codec {
            reason: format!(
                "effect key truncated: expected {} bytes, got {}",
                18 + intent_len,
                bytes.len()
            ),
        });
    }
    let intent_str = std::str::from_utf8(&bytes[18..18 + intent_len])
        .map_err(|e| EffectJournalError::Codec {
            reason: format!("invalid intent_id UTF-8: {e}"),
        })?;
    EffectId::new(&instance_id, intent_str).map_err(|_| EffectJournalError::Codec {
        reason: "empty intent_id in decoded key".to_string(),
    })
}

/// Get binary key prefix for all effects of a given instance (ADR-020).
///
/// Returns the 16-byte instance ID binary prefix.
#[must_use]
pub fn get_effect_key_prefix(instance_id: &InstanceId) -> Vec<u8> {
    encode_instance_id(instance_id)
        .map(|b| b.to_vec())
        .unwrap_or_default()
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
// In-memory implementation (also used for testing)
// ---------------------------------------------------------------------------

pub mod in_memory_journal;
pub use in_memory_journal::InMemoryEffectJournal;
