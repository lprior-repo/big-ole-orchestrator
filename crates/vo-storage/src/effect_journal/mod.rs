//! Effect journal partition — storage interface for managed effect lifecycle (ADR-030).
//!
//! Architecture: Data (`EffectId`, `EffectJournalError`) → Calc (`encode_effect_key`,
//! `decode_effect_key`, `encode_effect_record`, `decode_effect_record`) → Actions
//! (`EffectJournal` trait).
//!
//! This module defines the trait and pure encoding/decoding functions. Concrete Fjall
//! implementations are provided separately.

use std::fmt;

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Mutex;
#[cfg(test)]
use vo_types::EffectIntent;
use vo_types::{EffectRecord, InstanceId};

#[cfg(all(test, proptest))]
mod proptests;
#[cfg(all(test, not(feature = "proptest")))]
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
#[derive(Debug, PartialEq, Eq)]
pub enum EffectJournalError {
    /// The effect is already in a terminal state (Committed or `RolledBack`).
    AlreadyTerminal {
        effect_id: String,
        current_status: String,
    },

    /// The specified `effect_id` was not found in the journal.
    NotFound { effect_id: String },

    /// The underlying storage operation failed.
    Storage { reason: String },

    /// Serialization/deserialization failed.
    Codec { reason: String },

    /// Invalid argument (e.g., empty `intent_id`).
    InvalidArgument,
}

impl fmt::Display for EffectJournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyTerminal {
                effect_id,
                current_status,
            } => write!(
                f,
                "effect {effect_id} is already terminal with status {current_status}"
            ),
            Self::NotFound { effect_id } => write!(f, "effect {effect_id} not found"),
            Self::Storage { reason } => write!(f, "storage error: {reason}"),
            Self::Codec { reason } => write!(f, "codec error: {reason}"),
            Self::InvalidArgument => write!(f, "invalid argument"),
        }
    }
}

impl std::error::Error for EffectJournalError {}

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
}

// ---------------------------------------------------------------------------
// Concrete implementation — InMemoryEffectJournal (test only)
// ---------------------------------------------------------------------------

/// In-memory implementation of `EffectJournal` for testing and development.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct InMemoryEffectJournal {
    records: Mutex<HashMap<String, EffectRecord>>,
}

#[cfg(test)]
impl InMemoryEffectJournal {
    /// Creates a new empty `InMemoryEffectJournal`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
        }
    }

    /// Ensure the record is not already Committed or RolledBack.
    fn ensure_not_terminal(record: &EffectRecord, key: &str) -> Result<(), EffectJournalError> {
        match record.status() {
            EffectIntent::Committed | EffectIntent::RolledBack => {
                Err(EffectJournalError::AlreadyTerminal {
                    effect_id: key.to_string(),
                    current_status: format!("{:?}", record.status()),
                })
            }
            _ => Ok(()),
        }
    }

    /// Constructs the next EffectRecord for the target intent.
    fn construct_next_record(record: &EffectRecord, target: EffectIntent) -> Result<EffectRecord, EffectJournalError> {
        let ts = match target {
            EffectIntent::Committed => Some(vo_types::TimestampMs::parse("100").map_err(|e| {
                EffectJournalError::Storage {
                    reason: format!("failed to parse timestamp: {e}"),
                }
            })?),
            EffectIntent::RolledBack => None,
            _ => unreachable!(),
        };
        EffectRecord::new(record.intent_id().to_string(), record.kind(), record.params_json().clone(), target, ts)
            .ok_or_else(|| EffectJournalError::Storage { reason: "failed to create record".to_string() })
    }

    /// Validate record exists and is in Prepared state, then apply transition.
    fn validate_and_transition(
        record: &EffectRecord,
        key: &str,
        target: EffectIntent,
    ) -> Result<EffectRecord, EffectJournalError> {
        Self::ensure_not_terminal(record, key)?;
        Self::construct_next_record(record, target)
    }
}

/// Suppress `significant_drop_tightening` in lock guard scopes.
#[cfg(test)]
#[allow(clippy::significant_drop_tightening)]
impl EffectJournal for InMemoryEffectJournal {
    fn prepare(
        &self,
        instance_id: &InstanceId,
        record: EffectRecord,
    ) -> Result<EffectId, EffectJournalError> {
        let intent_id = record.intent_id().to_string();
        let effect_id = EffectId::new(instance_id, intent_id.as_str())?;
        let key = effect_id.as_str().to_string();

        let mut records = self
            .records
            .lock()
            .map_err(|e| EffectJournalError::Storage {
                reason: e.to_string(),
            })?;

        if records.contains_key(&key) {
            return Ok(effect_id);
        }

        records.insert(key, record);
        Ok(effect_id)
    }

    fn commit(&self, effect_id: &EffectId) -> Result<(), EffectJournalError> {
        let key = effect_id.as_str().to_string();
        let mut records = self
            .records
            .lock()
            .map_err(|e| EffectJournalError::Storage {
                reason: e.to_string(),
            })?;

        let record = records
            .get_mut(&key)
            .ok_or_else(|| EffectJournalError::NotFound {
                effect_id: key.clone(),
            })?;

        let next = Self::validate_and_transition(record, &key, EffectIntent::Committed)?;
        *record = next;
        Ok(())
    }

    fn rollback(&self, effect_id: &EffectId) -> Result<(), EffectJournalError> {
        let key = effect_id.as_str().to_string();
        let mut records = self
            .records
            .lock()
            .map_err(|e| EffectJournalError::Storage {
                reason: e.to_string(),
            })?;

        let record = records
            .get_mut(&key)
            .ok_or_else(|| EffectJournalError::NotFound {
                effect_id: key.clone(),
            })?;

        let next = Self::validate_and_transition(record, &key, EffectIntent::RolledBack)?;
        *record = next;
        Ok(())
    }

    fn list_pending(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Vec<EffectRecord>, EffectJournalError> {
        let records = self
            .records
            .lock()
            .map_err(|e| EffectJournalError::Storage {
                reason: e.to_string(),
            })?;

        let prefix = format!("{instance_id}::");
        Ok(records
            .iter()
            .filter(|(k, v)| k.starts_with(&prefix) && v.status() == EffectIntent::Prepared)
            .map(|(_, v)| v.clone())
            .collect())
    }
}
