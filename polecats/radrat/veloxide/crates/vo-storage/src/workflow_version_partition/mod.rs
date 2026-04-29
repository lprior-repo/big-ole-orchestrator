//! Workflow version partition — storage interface for workflow version metadata (ADR-017).
//!
//! Architecture: Data (`WorkflowVersionEntry`, `WorkflowVersionStoreError`)
//!             → Calc (`encode_workflow_version_key`, `decode_workflow_version_key`,
//!                    `encode_workflow_version_entry`, `decode_workflow_version_entry`)
//!             → Actions (`WorkflowVersionStore` trait).
//!
//! This module defines the trait and pure encoding/decoding functions. Concrete Fjall
//! implementations are provided separately.
//!
//! # Invariant
//!
//! Metadata persistence logic contains NO debounce/timer state. This partition is
//! responsible only for storing and retrieving workflow version records.

use serde::{Deserialize, Serialize};
use vo_types::{BinaryHash, TimestampMs, WorkflowName};

#[cfg(test)]
use serde_json;

mod fjall_store;
pub use fjall_store::FjallWorkflowVersionStore;

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::{BinaryHash, TimestampMs, WorkflowName};

    fn make_hash() -> BinaryHash {
        BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap()
    }

    fn make_name(s: &str) -> WorkflowName {
        WorkflowName::parse(s).unwrap()
    }

    fn make_ts(ms: u64) -> TimestampMs {
        TimestampMs::try_from(ms).unwrap()
    }

    fn make_entry() -> WorkflowVersionEntry {
        WorkflowVersionEntry::new(
            make_name("test-workflow"),
            make_hash(),
            1,
            make_ts(1712200000000u64),
            "/var/wtf/versions/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/test-workflow"
                .to_string(),
        )
        .unwrap()
    }

    #[test]
    fn workflow_version_entry_new_creates_valid_entry() {
        let entry = make_entry();
        assert_eq!(entry.workflow_name(), &make_name("test-workflow"));
        assert_eq!(entry.version_hash(), &make_hash());
        assert_eq!(entry.schema_version(), 1);
        assert_eq!(entry.registered_at(), make_ts(1712200000000u64));
        assert!(entry.binary_path().contains("test-workflow"));
    }

    #[test]
    fn workflow_version_entry_new_rejects_empty_binary_path() {
        let result = WorkflowVersionEntry::new(
            make_name("test"),
            make_hash(),
            1,
            make_ts(1000),
            "".to_string(),
        );
        assert!(matches!(
            result,
            Err(WorkflowVersionStoreError::InvalidArgument(_))
        ));
    }

    #[test]
    fn encode_workflow_version_key_produces_bytes() {
        let hash = make_hash();
        let key = encode_workflow_version_key(&hash);
        assert!(!key.is_empty());
        assert_eq!(key, hash.as_str().as_bytes());
    }

    #[test]
    fn roundtrip_encode_decode_entry() {
        let entry = make_entry();
        let key = encode_workflow_version_key(entry.version_hash());
        let value_bytes = encode_workflow_version_entry(&entry).unwrap();

        let decoded = decode_workflow_version_entry(&value_bytes).unwrap();
        assert_eq!(decoded.workflow_name(), entry.workflow_name());
        assert_eq!(decoded.version_hash(), entry.version_hash());
        assert_eq!(decoded.schema_version(), entry.schema_version());
        assert_eq!(decoded.registered_at(), entry.registered_at());
        assert_eq!(decoded.binary_path(), entry.binary_path());

        assert_eq!(key, encode_workflow_version_key(decoded.version_hash()));
    }

    #[test]
    fn error_display_includes_reason() {
        let err = WorkflowVersionStoreError::Storage {
            reason: "disk full".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("disk full"));
        assert!(msg.contains("storage error"));
    }

    #[test]
    fn key_not_found_error_contains_hash() {
        let hash = make_hash();
        let err = WorkflowVersionStoreError::KeyNotFound {
            hash: hash.to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains(&hash.to_string()));
    }
}

// ---------------------------------------------------------------------------
// Data layer — WorkflowVersionEntry
// ---------------------------------------------------------------------------

/// Persisted workflow version record.
///
/// Stores the canonical metadata for a workflow version identified by its hash.
/// This is the data layer representation that gets serialized to storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowVersionEntry {
    pub workflow_name: WorkflowName,
    pub version_hash: BinaryHash,
    pub schema_version: u16,
    pub registered_at: TimestampMs,
    pub binary_path: String,
}

impl WorkflowVersionEntry {
    /// Construct a new `WorkflowVersionEntry`.
    ///
    /// # Errors
    ///
    /// Returns `WorkflowVersionStoreError::InvalidArgument` if `binary_path` is empty.
    pub fn new(
        workflow_name: WorkflowName,
        version_hash: BinaryHash,
        schema_version: u16,
        registered_at: TimestampMs,
        binary_path: String,
    ) -> Result<Self, WorkflowVersionStoreError> {
        if binary_path.is_empty() {
            return Err(WorkflowVersionStoreError::InvalidArgument(
                "binary_path cannot be empty",
            ));
        }
        Ok(Self {
            workflow_name,
            version_hash,
            schema_version,
            registered_at,
            binary_path,
        })
    }

    #[must_use]
    pub fn workflow_name(&self) -> &WorkflowName {
        &self.workflow_name
    }

    #[must_use]
    pub fn version_hash(&self) -> &BinaryHash {
        &self.version_hash
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    #[must_use]
    pub fn registered_at(&self) -> TimestampMs {
        self.registered_at
    }

    #[must_use]
    pub fn binary_path(&self) -> &str {
        &self.binary_path
    }
}

// ---------------------------------------------------------------------------
// Data layer — WorkflowVersionStoreError
// ---------------------------------------------------------------------------

/// Errors from the workflow version store.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkflowVersionStoreError {
    #[error("storage error: {reason}")]
    Storage { reason: String },

    #[error("corrupt value: {reason}")]
    CorruptValue { reason: String },

    #[error("key not found: {hash}")]
    KeyNotFound { hash: String },

    #[error("invalid argument: {0}")]
    InvalidArgument(&'static str),

    #[error("serialization failed: {reason}")]
    SerializationFailed { reason: String },

    #[error("deserialization failed: {reason}")]
    DeserializationFailed { reason: String },
}

// ---------------------------------------------------------------------------
// Calc layer — key encoding/decoding
// ---------------------------------------------------------------------------

/// The partition name used for workflow version records.
pub const WORKFLOW_VERSIONS_PARTITION_NAME: &str = "workflow_versions";

/// Encode a workflow version key (the hash) as bytes for storage.
///
/// The key is the version hash string bytes.
pub fn encode_workflow_version_key(hash: &BinaryHash) -> Vec<u8> {
    hash.as_str().as_bytes().to_vec()
}

/// Decode bytes into a binary hash for workflow version key lookup.
///
/// # Errors
///
/// Returns `WorkflowVersionStoreError::Storage` if decoding fails.
pub fn decode_workflow_version_key(bytes: &[u8]) -> Result<BinaryHash, WorkflowVersionStoreError> {
    let hash_str = std::str::from_utf8(bytes).map_err(|_| WorkflowVersionStoreError::Storage {
        reason: "invalid UTF-8 in workflow version key".to_string(),
    })?;
    BinaryHash::parse(hash_str).map_err(|_| WorkflowVersionStoreError::Storage {
        reason: "invalid binary hash format".to_string(),
    })
}

/// Encode a `WorkflowVersionEntry` to JSON bytes.
///
/// # Errors
///
/// Returns `WorkflowVersionStoreError::SerializationFailed` if serialization fails.
pub fn encode_workflow_version_entry(
    entry: &WorkflowVersionEntry,
) -> Result<Vec<u8>, WorkflowVersionStoreError> {
    serde_json::to_vec(entry).map_err(|e| WorkflowVersionStoreError::SerializationFailed {
        reason: e.to_string(),
    })
}

/// Decode JSON bytes into a `WorkflowVersionEntry`.
///
/// # Errors
///
/// Returns `WorkflowVersionStoreError::DeserializationFailed` if the bytes are not valid JSON
/// or do not represent a valid `WorkflowVersionEntry`.
pub fn decode_workflow_version_entry(
    bytes: &[u8],
) -> Result<WorkflowVersionEntry, WorkflowVersionStoreError> {
    serde_json::from_slice(bytes).map_err(|e| WorkflowVersionStoreError::DeserializationFailed {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Actions layer — trait definition
// ---------------------------------------------------------------------------

/// Trait for workflow version storage operations.
///
/// All methods return `Result` to ensure error handling is explicit.
/// No debounce or timer state is maintained by this trait or implementations.
pub trait WorkflowVersionStore: Send + Sync {
    /// Get a workflow version entry by its hash.
    ///
    /// # Errors
    ///
    /// Returns `WorkflowVersionStoreError::KeyNotFound` if the hash is not found.
    fn get(&self, hash: &BinaryHash) -> Result<WorkflowVersionEntry, WorkflowVersionStoreError>;

    /// Insert or update a workflow version entry.
    ///
    /// # Errors
    ///
    /// Returns `WorkflowVersionStoreError::Storage` if the operation fails.
    fn put(&self, entry: &WorkflowVersionEntry) -> Result<(), WorkflowVersionStoreError>;

    /// Check if a workflow version exists.
    ///
    /// # Errors
    ///
    /// Returns `WorkflowVersionStoreError::Storage` if the check fails.
    fn contains(&self, hash: &BinaryHash) -> Result<bool, WorkflowVersionStoreError>;

    /// Delete a workflow version entry by hash.
    ///
    /// # Errors
    ///
    /// Returns `WorkflowVersionStoreError::KeyNotFound` if the hash is not found.
    fn delete(&self, hash: &BinaryHash) -> Result<(), WorkflowVersionStoreError>;

    /// List all workflow version hashes stored.
    ///
    /// # Errors
    ///
    /// Returns `WorkflowVersionStoreError::Storage` if the listing fails.
    fn list_hashes(&self) -> Result<Vec<BinaryHash>, WorkflowVersionStoreError>;
}
