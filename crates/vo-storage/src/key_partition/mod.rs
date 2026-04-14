//! Key partition — storage interface for DEK/KEK encryption lifecycle (ADR-035).
//!
//! Architecture: Data (`DekStoreError`, `DekEntry`, `WrappedDek`) → Calc (encode/decode)
//!             → Actions (`DekStore` trait).
//!
//! This module defines the trait and pure encoding/decoding functions. Concrete Fjall
//! implementations are provided separately.
//!
//! ## Key Lifecycle
//!
//! 1. **Generation**: `generate_dek` creates a fresh DEK, `wrap_dek` encrypts it with KEK
//! 2. **Rotation**: Old DEK is retired, new DEK is generated and wrapped
//! 3. **Retirement**: DEK is destroyed (crypto-shredded), making all encrypted data irrecoverable
//!
//! ## Invariants
//!
//! - DEKs are NEVER stored unwrapped - only `WrappedDek` is ever persisted
//! - Each `InstanceId` maps to exactly one active `DekId` at runtime
//! - Each `DekId` maps to exactly one `InstanceId` at runtime
//! - Purge ordering: DEK destruction → index cleanup → blob reference removal

use vo_types::{DekId, InstanceId, KeyMetadata, WrappedDek};

#[cfg(all(test, feature = "proptest"))]
mod proptests;

mod fjall_dek_store;

// ---------------------------------------------------------------------------
// Data layer — DekEntry
// ---------------------------------------------------------------------------

/// Persisted DEK record with lifecycle metadata.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DekEntry {
    dek_id: DekId,
    instance_id: InstanceId,
    wrapped_dek: WrappedDek,
    metadata: KeyMetadata,
    status: DekStatus,
}

impl DekEntry {
    /// Construct a new `DekEntry`.
    ///
    /// # Errors
    ///
    /// Returns `DekStoreError::InvalidArgument` if inputs are invalid.
    pub const fn new(
        dek_id: DekId,
        instance_id: InstanceId,
        wrapped_dek: WrappedDek,
        metadata: KeyMetadata,
    ) -> Result<Self, DekStoreError> {
        Ok(Self {
            dek_id,
            instance_id,
            wrapped_dek,
            metadata,
            status: DekStatus::Active,
        })
    }

    #[must_use]
    pub const fn dek_id(&self) -> &DekId {
        &self.dek_id
    }

    #[must_use]
    pub const fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    #[must_use]
    pub const fn wrapped_dek(&self) -> &WrappedDek {
        &self.wrapped_dek
    }

    #[must_use]
    pub const fn metadata(&self) -> &KeyMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn status(&self) -> DekStatus {
        self.status
    }

    /// Mark this DEK as retired (crypto-shredded).
    pub const fn retire(&mut self) {
        self.status = DekStatus::Retired;
    }
}

/// DEK lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DekStatus {
    /// DEK is active and can be used for encryption/decryption.
    Active,
    /// DEK has been retired (crypto-shredded) and cannot be used.
    Retired,
}

// ---------------------------------------------------------------------------
// Data layer — error enum
// ---------------------------------------------------------------------------

/// Errors from the DEK store operations.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum DekStoreError {
    #[error("DEK not found for instance: {instance_id}")]
    DekNotFound { instance_id: String },
    #[error("DEK has been retired (crypto-shredded): {dek_id}")]
    DekRetired { dek_id: String },
    #[error("DEK already exists for instance: {instance_id}")]
    DekAlreadyExists { instance_id: String },
    #[error("DEK storage error: {reason}")]
    Storage { reason: String },
    #[error("DEK codec error: {reason}")]
    Codec { reason: String },
    #[error("invalid DEK argument")]
    InvalidArgument,
    #[error("key store partition inaccessible")]
    KeyStoreUnavailable,
}

// ---------------------------------------------------------------------------
// Calc layer — key encoding/decoding
// ---------------------------------------------------------------------------

/// Partition name for the DEK key store.
pub const DEK_PARTITION: &str = "dek_store";

/// Encode an `InstanceId` as UTF-8 bytes for use as a partition key.
#[must_use]
pub fn encode_instance_key(instance_id: &InstanceId) -> Vec<u8> {
    instance_id.as_str().as_bytes().to_vec()
}

/// Decode UTF-8 bytes into an `InstanceId`.
///
/// # Errors
///
/// Returns `DekStoreError::Codec` if bytes are not valid UTF-8 or if the
/// resulting string is not a valid `InstanceId`.
pub fn decode_instance_key(bytes: &[u8]) -> Result<InstanceId, DekStoreError> {
    let s = std::str::from_utf8(bytes).map_err(|e| DekStoreError::Codec {
        reason: e.to_string(),
    })?;
    InstanceId::parse(s).map_err(|e| DekStoreError::Codec {
        reason: e.to_string(),
    })
}

/// Encode a `DekEntry` to JSON bytes for storage.
///
/// # Panics
///
/// Panics if serialization fails, which should not happen for valid `DekEntry` instances.
#[must_use]
#[allow(clippy::expect_used)]
pub fn encode_dek_entry(entry: &DekEntry) -> Vec<u8> {
    serde_json::to_vec(entry).expect("DekEntry should always be serializable")
}

/// Decode JSON bytes into a `DekEntry`.
///
/// # Errors
///
/// Returns `DekStoreError::Codec` if the bytes are not valid JSON
/// or do not represent a valid `DekEntry`.
pub fn decode_dek_entry(bytes: &[u8]) -> Result<DekEntry, DekStoreError> {
    serde_json::from_slice(bytes).map_err(|e| DekStoreError::Codec {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Actions layer — DekStore trait
// ---------------------------------------------------------------------------

/// Storage interface for DEK lifecycle management.
///
/// # Key Invariants
///
/// 1. DEKs are NEVER stored unwrapped - only `WrappedDek` is ever persisted
/// 2. Each `InstanceId` maps to exactly one active `DekId` at runtime
/// 3. Each `DekId` maps to exactly one `InstanceId` at runtime
///
/// # Lifecycle Operations
///
/// - **Generate**: Creates a new DEK, wraps it with KEK, stores `WrappedDek`
/// - **Retrieve**: Retrieves and unwraps a DEK for encryption/decryption
/// - **Rotate**: Retires old DEK, generates new DEK
/// - **Retire**: Destroys DEK (crypto-shredding), making encrypted data irrecoverable
pub trait DekStore: Send + Sync {
    /// Generate a new DEK for an instance and store it wrapped with KEK.
    ///
    /// If a DEK already exists for this instance, returns `DekStoreError::DekAlreadyExists`.
    ///
    /// # Errors
    ///
    /// Returns `DekStoreError::DekAlreadyExists` if a DEK already exists.
    /// Returns `DekStoreError::Storage` if the underlying storage fails.
    fn generate_and_store_dek(
        &self,
        instance_id: &InstanceId,
        kek: &[u8; 32],
    ) -> Result<DekId, DekStoreError>;

    /// Retrieve the active DEK for an instance, unwrapped with KEK.
    ///
    /// # Errors
    ///
    /// Returns `DekStoreError::DekNotFound` if no DEK exists for this instance.
    /// Returns `DekStoreError::DekRetired` if the DEK has been retired.
    fn retrieve_dek(
        &self,
        instance_id: &InstanceId,
        kek: &[u8; 32],
    ) -> Result<[u8; 32], DekStoreError>;

    /// Get the active DEK ID for an instance.
    ///
    /// # Errors
    ///
    /// Returns `DekStoreError::DekNotFound` if no DEK exists for this instance.
    fn get_active_dek_id(&self, instance_id: &InstanceId) -> Result<DekId, DekStoreError>;

    /// Check if a DEK exists and is active for an instance.
    ///
    /// # Errors
    ///
    /// Returns `DekStoreError::Storage` if the underlying storage fails.
    fn has_active_dek(&self, instance_id: &InstanceId) -> Result<bool, DekStoreError>;

    /// Rotate the DEK for an instance: retire old DEK, generate new DEK.
    ///
    /// The old DEK is marked as retired. The new DEK is generated, wrapped with KEK,
    /// and stored.
    ///
    /// # Errors
    ///
    /// Returns `DekStoreError::DekNotFound` if no DEK exists to rotate.
    /// Returns `DekStoreError::Storage` if the underlying storage fails.
    fn rotate_dek(
        &self,
        instance_id: &InstanceId,
        kek: &[u8; 32],
    ) -> Result<DekId, DekStoreError>;

    /// Retire a DEK (crypto-shred it).
    ///
    /// After retirement, the DEK cannot be retrieved and all data encrypted with it
    /// becomes irrecoverable.
    ///
    /// # Errors
    ///
    /// Returns `DekStoreError::DekNotFound` if no DEK exists to retire.
    fn retire_dek(&self, instance_id: &InstanceId) -> Result<(), DekStoreError>;

    /// List all DEK IDs for a given instance.
    ///
    /// # Errors
    ///
    /// Returns `DekStoreError::Storage` if the underlying storage fails.
    fn list_deks(&self, instance_id: &InstanceId) -> Result<Vec<DekId>, DekStoreError>;

    /// Get DEK metadata.
    ///
    /// # Errors
    ///
    /// Returns `DekStoreError::DekNotFound` if no DEK exists.
    fn get_dek_metadata(&self, dek_id: &DekId) -> Result<KeyMetadata, DekStoreError>;
}