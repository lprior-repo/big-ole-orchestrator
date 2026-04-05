//! Dedupe partition — storage interface for exactly-once ingress deduplication (ADR-028).
//!
//! Architecture: Data (`AdmissionResult`, `DedupeEntry`, `DedupeStoreError`)
//!             → Calc (`encode_dedupe_key`, `decode_dedupe_key`, `encode_dedupe_entry`, `decode_dedupe_entry`)
//!             → Actions (`DedupeStore` trait).
//!
//! This module defines the trait and pure encoding/decoding functions. Concrete Fjall
//! implementations are provided separately.

use std::fmt;

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
// Data layer — error enum
// ---------------------------------------------------------------------------

/// Errors from the dedupe store operations.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq)]
pub enum DedupeStoreError {
    /// Storage operation failed.
    Storage { reason: String },
    /// Codec/serialization error.
    Codec { reason: String },
    /// Invalid argument.
    InvalidArgument,
}

impl fmt::Display for DedupeStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage { reason } => write!(f, "dedupe storage error: {reason}"),
            Self::Codec { reason } => write!(f, "dedupe codec error: {reason}"),
            Self::InvalidArgument => write!(f, "invalid dedupe argument"),
        }
    }
}

impl std::error::Error for DedupeStoreError {}

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
// Calc layer — entry encoding/decoding
// ---------------------------------------------------------------------------

/// Encode a `DedupeEntry` to JSON bytes for storage.
///
/// # Errors
///
/// Returns `DedupeStoreError::Codec` if serialization fails.
pub fn encode_dedupe_entry(entry: &DedupeEntry) -> Result<Vec<u8>, DedupeStoreError> {
    serde_json::to_vec(entry).map_err(|e| DedupeStoreError::Codec {
        reason: e.to_string(),
    })
}

/// Decode JSON bytes into a `DedupeEntry`.
///
/// # Errors
///
/// Returns `DedupeStoreError::Codec` if deserialization fails.
pub fn decode_dedupe_entry(bytes: &[u8]) -> Result<DedupeEntry, DedupeStoreError> {
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
