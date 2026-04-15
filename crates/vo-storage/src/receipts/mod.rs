//! Receipt persistence partition — durable execution receipts for managed connectors (ADR-041).
//!
//! Architecture: Data (`ExecutionReceipt`, `ReceiptStoreError`)
//!             → Calc (`encode_receipt_key`, `decode_receipt_key`, `encode_receipt`, `decode_receipt`)
//!             → Actions (`ReceiptStore` trait).
//!
//! When a managed connector commit succeeds, the engine writes an execution receipt
//! to durable storage. This receipt enforces exact-once execution boundaries:
//! if a receipt exists for an effect ID, the effect MUST NOT be re-executed.
//!
//! Invariant: Writing a receipt for an already-completed effect ID is a no-op
//! (idempotency success).

use vo_types::{EffectKind, InstanceId};

#[cfg(test)]
mod tests;

mod fjall_receipt_store;
pub use fjall_receipt_store::FjallReceiptStore;

// ---------------------------------------------------------------------------
// Data layer — ExecutionReceipt
// ---------------------------------------------------------------------------

/// Durable record proving a connector effect was successfully committed.
///
/// Immutable once written. Used to enforce exact-once execution boundaries:
/// the presence of a receipt for an effect ID means the effect MUST NOT
/// be re-executed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExecutionReceipt {
    effect_id: String,
    instance_id: String,
    kind: EffectKind,
    committed_at_ms: u64,
    connector_result: String,
}

impl ExecutionReceipt {
    /// Construct a new `ExecutionReceipt`.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::InvalidArgument` if `effect_id` or `instance_id` is empty.
    pub fn new(
        effect_id: String,
        instance_id: String,
        kind: EffectKind,
        committed_at_ms: u64,
        connector_result: String,
    ) -> Result<Self, ReceiptStoreError> {
        if effect_id.is_empty() || instance_id.is_empty() {
            return Err(ReceiptStoreError::InvalidArgument);
        }
        Ok(Self {
            effect_id,
            instance_id,
            kind,
            committed_at_ms,
            connector_result,
        })
    }

    #[must_use]
    pub fn effect_id(&self) -> &str {
        &self.effect_id
    }

    #[must_use]
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    #[must_use]
    pub fn kind(&self) -> EffectKind {
        self.kind
    }

    #[must_use]
    pub const fn committed_at_ms(&self) -> u64 {
        self.committed_at_ms
    }

    #[must_use]
    pub fn connector_result(&self) -> &str {
        &self.connector_result
    }
}

// ---------------------------------------------------------------------------
// Data layer — error enum
// ---------------------------------------------------------------------------

/// Errors from receipt store operations.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReceiptStoreError {
    #[error("receipt storage error: {reason}")]
    Storage { reason: String },
    #[error("receipt codec error: {reason}")]
    Codec { reason: String },
    #[error("invalid receipt argument")]
    InvalidArgument,
}

// ---------------------------------------------------------------------------
// Calc layer — key encoding/decoding
// ---------------------------------------------------------------------------

/// Encode an effect ID as UTF-8 bytes for use as a receipt partition key.
///
/// Key format: the raw effect ID string (e.g., `<instance_id>::<intent_id>`).
#[must_use]
pub fn encode_receipt_key(effect_id: &str) -> Vec<u8> {
    effect_id.as_bytes().to_vec()
}

/// Decode UTF-8 bytes into an effect ID string.
///
/// # Errors
///
/// Returns `ReceiptStoreError::Codec` if the bytes are not valid UTF-8 or empty.
pub fn decode_receipt_key(bytes: &[u8]) -> Result<String, ReceiptStoreError> {
    let s = std::str::from_utf8(bytes).map_err(|e| ReceiptStoreError::Codec {
        reason: e.to_string(),
    })?;
    if s.is_empty() {
        return Err(ReceiptStoreError::Codec {
            reason: "empty receipt key".to_string(),
        });
    }
    Ok(s.to_string())
}

// ---------------------------------------------------------------------------
// Calc layer — receipt encoding/decoding
// ---------------------------------------------------------------------------

/// Encode an `ExecutionReceipt` to JSON bytes for storage.
///
/// # Errors
///
/// Returns `ReceiptStoreError::Codec` if serialization fails.
pub fn encode_receipt(receipt: &ExecutionReceipt) -> Result<Vec<u8>, ReceiptStoreError> {
    serde_json::to_vec(receipt).map_err(|e| ReceiptStoreError::Codec {
        reason: e.to_string(),
    })
}

/// Decode JSON bytes into an `ExecutionReceipt`.
///
/// # Errors
///
/// Returns `ReceiptStoreError::Codec` if deserialization fails.
pub fn decode_receipt(bytes: &[u8]) -> Result<ExecutionReceipt, ReceiptStoreError> {
    serde_json::from_slice(bytes).map_err(|e| ReceiptStoreError::Codec {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Actions layer — ReceiptStore trait
// ---------------------------------------------------------------------------

/// Partition name for the receipts store.
pub const RECEIPTS_PARTITION: &str = "receipts";

/// Storage interface for durable execution receipts (ADR-041).
///
/// Receipts enforce exact-once execution boundaries: if a receipt exists
/// for an effect ID, the effect MUST NOT be re-executed.
pub trait ReceiptStore {
    /// Store an execution receipt. Idempotent: if a receipt already exists
    /// for the same effect ID, this is a no-op and returns `Ok(())`.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::InvalidArgument` if `effect_id` is empty.
    /// Returns `ReceiptStoreError::Storage` if the underlying storage fails.
    fn store_receipt(
        &self,
        receipt: ExecutionReceipt,
    ) -> Result<(), ReceiptStoreError>;

    /// Retrieve an execution receipt by effect ID.
    ///
    /// Returns `Ok(None)` if no receipt exists for the given effect ID.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::Storage` if the underlying storage fails.
    fn get_receipt(&self, effect_id: &str) -> Result<Option<ExecutionReceipt>, ReceiptStoreError>;

    /// Check whether a receipt exists for the given effect ID.
    ///
    /// This is equivalent to `get_receipt` but avoids deserialization overhead.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::Storage` if the underlying storage fails.
    fn has_receipt(&self, effect_id: &str) -> Result<bool, ReceiptStoreError>;

    /// List all receipts for a given instance ID.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::Storage` if the underlying storage fails.
    fn list_by_instance(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Vec<ExecutionReceipt>, ReceiptStoreError>;
}

// ---------------------------------------------------------------------------
// Test infrastructure
// ---------------------------------------------------------------------------

#[cfg(test)]
pub mod in_memory_receipt_store;
