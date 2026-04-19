//! Dedupe partition — storage interface for exactly-once ingress deduplication (ADR-028).
//!
//! Architecture: Data (`AdmissionResult`, `DedupeEntry`, `DedupeStoreError`)
//!             → Calc (`encode_dedupe_key`, `decode_dedupe_key`, `encode_dedupe_entry`, `decode_dedupe_entry`)
//!             → Actions (`DedupeStore` trait).
//!
//! This module defines the trait and pure encoding/decoding functions. Concrete Fjall
//! implementations are provided separately.

use vo_types::{DedupeKey, InstanceId};

#[cfg(all(test, feature = "proptest"))]
mod proptests;
#[cfg(test)]
mod red_queen_constants_expiry;
#[cfg(test)]
mod red_queen_serde_behavior;
#[cfg(test)]
mod red_queen_validation;
#[cfg(test)]
mod tests;
#[cfg(kani)]
mod verification;

mod fjall_dedupe;
pub use fjall_dedupe::FjallDedupeStore;

pub mod in_memory_dedupe;
pub use in_memory_dedupe::InMemoryDedupeStore;

// ---------------------------------------------------------------------------
// Data layer — AdmissionResult
// ---------------------------------------------------------------------------

/// Result of an atomic check-and-insert dedupe operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionResult {
    /// New key admitted (first occurrence).
    Admitted,
    /// Duplicate key rejected (already exists and not expired).
    Duplicate { instance_id: String },
}

// ---------------------------------------------------------------------------
// Data layer — DedupeEntry
// ---------------------------------------------------------------------------

/// Persisted dedupe record with TTL-based expiry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DedupeEntry {
    dedupe_key: String,
    instance_id: String,
    expires_at: u64,
}

impl DedupeEntry {
    /// Construct a new `DedupeEntry`.
    ///
    /// # Errors
    ///
    /// Returns `DedupeStoreError::InvalidArgument` if `dedupe_key` or `instance_id` is empty.
    pub fn new(
        dedupe_key: String,
        instance_id: String,
        expires_at: u64,
    ) -> Result<Self, DedupeStoreError> {
        if dedupe_key.is_empty() || instance_id.is_empty() {
            return Err(DedupeStoreError::InvalidArgument);
        }
        Ok(Self {
            dedupe_key,
            instance_id,
            expires_at,
        })
    }

    #[must_use]
    pub fn dedupe_key(&self) -> &str {
        &self.dedupe_key
    }

    #[must_use]
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    #[must_use]
    pub const fn expires_at(&self) -> u64 {
        self.expires_at
    }

    /// Check if this entry has expired given the current timestamp.
    #[must_use]
    pub const fn is_expired(&self, now_ms: u64) -> bool {
        now_ms >= self.expires_at
    }
}

// ---------------------------------------------------------------------------
// Data layer — DedupeRetentionRecord
// ---------------------------------------------------------------------------

/// Persisted dedupe retention record for exact-once admission tracking (ADR-028).
///
/// Tracks when an instance reached terminal state and when the dedupe retention
/// window expires. This is distinct from `DedupeEntry` which uses raw TTL-based
/// expiry. The retention record implements the two-phase retention policy:
///
/// 1. The instance must reach a terminal state (Completed, Failed, or Cancelled).
/// 2. The configured dedupe retention window must expire after the terminal state.
///
/// This ensures exactly-once admission guarantees even for long-lived workflows
/// where simple TTL expiry would be insufficient.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DedupeRetentionRecord {
    dedupe_key: String,
    instance_id: String,
    terminal_state_at: u64,
    retention_expires_at: u64,
}

impl DedupeRetentionRecord {
    /// Construct a new `DedupeRetentionRecord`.
    ///
    /// # Errors
    ///
    /// Returns `DedupeStoreError::InvalidArgument` if `dedupe_key` or `instance_id` is empty,
    /// or if `terminal_state_at` exceeds `retention_expires_at`.
    pub fn new(
        dedupe_key: String,
        instance_id: String,
        terminal_state_at: u64,
        retention_expires_at: u64,
    ) -> Result<Self, DedupeStoreError> {
        if dedupe_key.is_empty() || instance_id.is_empty() {
            return Err(DedupeStoreError::InvalidArgument);
        }
        if terminal_state_at > retention_expires_at {
            return Err(DedupeStoreError::InvalidArgument);
        }
        Ok(Self {
            dedupe_key,
            instance_id,
            terminal_state_at,
            retention_expires_at,
        })
    }

    #[must_use]
    pub fn dedupe_key(&self) -> &str {
        &self.dedupe_key
    }

    #[must_use]
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    #[must_use]
    pub const fn terminal_state_at(&self) -> u64 {
        self.terminal_state_at
    }

    #[must_use]
    pub const fn retention_expires_at(&self) -> u64 {
        self.retention_expires_at
    }

    /// Check if this retention record has expired given the current timestamp.
    #[must_use]
    pub const fn is_retention_expired(&self, now_ms: u64) -> bool {
        now_ms >= self.retention_expires_at
    }

    /// Compute the retention expiry timestamp from terminal state and retention period.
    ///
    /// This is a convenience method to calculate `retention_expires_at` when creating
    /// a new retention record.
    #[must_use]
    pub const fn compute_retention_expiry(terminal_state_at: u64, retention_period_ms: u64) -> u64 {
        terminal_state_at.saturating_add(retention_period_ms)
    }
}

// ---------------------------------------------------------------------------
// Data layer — error enum
// ---------------------------------------------------------------------------

/// Errors from the dedupe store operations.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum DedupeStoreError {
    #[error("dedupe storage error: {reason}")]
    Storage { reason: String },
    #[error("dedupe codec error: {reason}")]
    Codec { reason: String },
    #[error("invalid dedupe argument")]
    InvalidArgument,
}

// ---------------------------------------------------------------------------
// Calc layer — key encoding/decoding
// ---------------------------------------------------------------------------

/// Encode a `DedupeKey` as UTF-8 bytes for use as a partition key.
#[must_use]
pub fn encode_dedupe_key(key: &DedupeKey) -> Vec<u8> {
    key.as_str().as_bytes().to_vec()
}

/// Decode UTF-8 bytes into a `DedupeKey`.
///
/// # Errors
///
/// Returns `DedupeStoreError::Codec` if bytes are not valid UTF-8 or if the
/// resulting string is empty (empty keys are rejected by `DedupeKey::parse`).
pub fn decode_dedupe_key(bytes: &[u8]) -> Result<DedupeKey, DedupeStoreError> {
    let s = std::str::from_utf8(bytes).map_err(|e| DedupeStoreError::Codec {
        reason: e.to_string(),
    })?;
    DedupeKey::parse(s).map_err(|e| DedupeStoreError::Codec {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Calc layer — entry encoding/decoding (binary, primary path)
// ---------------------------------------------------------------------------
//
// Binary wire format for DedupeEntry:
//   [dk_len: u16_be][dk_bytes][iid_len: u16_be][iid_bytes][expires_at: u64_be]
//
// This replaces JSON encoding on the hot path (every workflow start), reducing
// serialization cost by ~5-10x for the typical 3-field struct.

/// Encode a `DedupeEntry` to compact binary bytes for storage.
///
/// Wire format: `[dk_len:u16_be][dk_bytes][iid_len:u16_be][iid_bytes][expires_at:u64_be]`
///
/// # Errors
///
/// Returns `DedupeStoreError::Codec` if field lengths exceed `u16::MAX`.
pub fn encode_dedupe_entry(entry: &DedupeEntry) -> Result<Vec<u8>, DedupeStoreError> {
    let dk_bytes = entry.dedupe_key().as_bytes();
    let iid_bytes = entry.instance_id().as_bytes();
    let dk_len = u16::try_from(dk_bytes.len()).map_err(|_| DedupeStoreError::Codec {
        reason: "dedupe_key exceeds u16::MAX bytes".to_string(),
    })?;
    let iid_len = u16::try_from(iid_bytes.len()).map_err(|_| DedupeStoreError::Codec {
        reason: "instance_id exceeds u16::MAX bytes".to_string(),
    })?;

    let mut buf = Vec::with_capacity(2 + dk_bytes.len() + 2 + iid_bytes.len() + 8);
    buf.extend_from_slice(&dk_len.to_be_bytes());
    buf.extend_from_slice(dk_bytes);
    buf.extend_from_slice(&iid_len.to_be_bytes());
    buf.extend_from_slice(iid_bytes);
    buf.extend_from_slice(&entry.expires_at().to_be_bytes());
    Ok(buf)
}

/// Decode binary bytes into a `DedupeEntry`.
///
/// Expects wire format: `[dk_len:u16_be][dk_bytes][iid_len:u16_be][iid_bytes][expires_at:u64_be]`
///
/// # Errors
///
/// Returns `DedupeStoreError::Codec` if the buffer is malformed, truncated,
/// contains invalid UTF-8, or yields empty fields.
pub fn decode_dedupe_entry(bytes: &[u8]) -> Result<DedupeEntry, DedupeStoreError> {
    if bytes.len() < 12 {
        return Err(DedupeStoreError::Codec {
            reason: format!(
                "entry too short: {} bytes (minimum 12 for two empty fields + u64)",
                bytes.len()
            ),
        });
    }

    let dk_len = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
    if bytes.len() < 2 + dk_len + 2 {
        return Err(DedupeStoreError::Codec {
            reason: "truncated: cannot read instance_id length after dedupe_key".to_string(),
        });
    }
    let dk_str =
        std::str::from_utf8(&bytes[2..2 + dk_len]).map_err(|e| DedupeStoreError::Codec {
            reason: format!("dedupe_key invalid UTF-8: {e}"),
        })?;

    let offset = 2 + dk_len;
    let iid_len = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
    if bytes.len() < offset + 2 + iid_len + 8 {
        return Err(DedupeStoreError::Codec {
            reason: "truncated: cannot read expires_at after instance_id".to_string(),
        });
    }
    let iid_str = std::str::from_utf8(&bytes[offset + 2..offset + 2 + iid_len]).map_err(|e| {
        DedupeStoreError::Codec {
            reason: format!("instance_id invalid UTF-8: {e}"),
        }
    })?;

    let ts_offset = offset + 2 + iid_len;
    let expires_at = u64::from_be_bytes([
        bytes[ts_offset],
        bytes[ts_offset + 1],
        bytes[ts_offset + 2],
        bytes[ts_offset + 3],
        bytes[ts_offset + 4],
        bytes[ts_offset + 5],
        bytes[ts_offset + 6],
        bytes[ts_offset + 7],
    ]);

    DedupeEntry::new(dk_str.to_string(), iid_str.to_string(), expires_at)
}

// ---------------------------------------------------------------------------
// Calc layer — retention record encoding/decoding
// ---------------------------------------------------------------------------

/// Partition name for the dedupe retention store.
pub const DEDUPE_RETENTION_PARTITION: &str = "dedupe_retention";

/// Encode a `DedupeRetentionRecord` to JSON bytes for storage.
///
/// # Errors
///
/// Returns `DedupeStoreError::Codec` if serialization fails.
pub fn encode_dedupe_retention_record(
    record: &DedupeRetentionRecord,
) -> Result<Vec<u8>, DedupeStoreError> {
    serde_json::to_vec(record).map_err(|e| DedupeStoreError::Codec {
        reason: e.to_string(),
    })
}

/// Decode JSON bytes into a `DedupeRetentionRecord`.
///
/// # Errors
///
/// Returns `DedupeStoreError::Codec` if deserialization fails.
pub fn decode_dedupe_retention_record(
    bytes: &[u8],
) -> Result<DedupeRetentionRecord, DedupeStoreError> {
    serde_json::from_slice(bytes).map_err(|e| DedupeStoreError::Codec {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Calc layer — retention record encoding/decoding
// ---------------------------------------------------------------------------

/// Partition name for the dedupe retention store.
pub const DEDUPE_RETENTION_PARTITION: &str = "dedupe_retention";

/// Encode a `DedupeRetentionRecord` to JSON bytes for storage.
///
/// # Errors
///
/// Returns `DedupeStoreError::Codec` if serialization fails.
pub fn encode_dedupe_retention_record(
    record: &DedupeRetentionRecord,
) -> Result<Vec<u8>, DedupeStoreError> {
    serde_json::to_vec(record).map_err(|e| DedupeStoreError::Codec {
        reason: e.to_string(),
    })
}

/// Decode JSON bytes into a `DedupeRetentionRecord`.
///
/// # Errors
///
/// Returns `DedupeStoreError::Codec` if deserialization fails.
pub fn decode_dedupe_retention_record(
    bytes: &[u8],
) -> Result<DedupeRetentionRecord, DedupeStoreError> {
    serde_json::from_slice(bytes).map_err(|e| DedupeStoreError::Codec {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Actions layer — DedupeStore trait
// ---------------------------------------------------------------------------

/// Partition name for the dedupe store.
pub const DEDUPE_PARTITION: &str = "dedupe";

/// Storage interface for exactly-once ingress deduplication (ADR-028).
///
/// Provides atomic check-and-insert with TTL-based expiry.
pub trait DedupeStore {
    /// Atomically check if a dedupe key exists and insert if not.
    ///
    /// If the key exists and is not expired, returns `AdmissionResult::Duplicate`.
    /// If the key does not exist or is expired, inserts and returns `AdmissionResult::Admitted`.
    ///
    /// # Errors
    ///
    /// Returns `DedupeStoreError::InvalidArgument` if `ttl_ms` is zero.
    /// Returns `DedupeStoreError::Storage` if the underlying storage fails.
    fn check_and_insert(
        &self,
        key: &DedupeKey,
        instance_id: &InstanceId,
        ttl_ms: u64,
    ) -> Result<AdmissionResult, DedupeStoreError>;

    /// Purge all expired dedupe records. Returns the count of purged records.
    ///
    /// # Errors
    ///
    /// Returns `DedupeStoreError::Storage` if the underlying storage fails.
    fn purge_expired(&self, now_ms: u64) -> Result<u64, DedupeStoreError>;

    /// Check if a dedupe key exists and is not expired.
    ///
    /// # Errors
    ///
    /// Returns `DedupeStoreError::Storage` if the underlying storage fails.
    fn contains(&self, key: &DedupeKey) -> Result<bool, DedupeStoreError>;
}
