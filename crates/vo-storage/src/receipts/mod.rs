//! Receipt persistence partition — durable storage for connector execution receipts (ADR-041).
//!
//! Architecture: Data (`Receipt`, `ReceiptStoreError`) → Calc (`encode_receipt_key`,
//! `decode_receipt_key`, `encode_receipt`, `decode_receipt`) → Actions (`ReceiptStore` trait).
//!
//! Receipts enforce exact-once execution boundaries. Once a receipt exists for an effect ID,
//! the system SHALL NOT allow re-execution of that effect.

use crate::effect_journal::EffectId;
use vo_types::ConnectorResult;

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------
// Data layer — Receipt
// ---------------------------------------------------------------------------

/// Immutable execution receipt for a managed connector commit (ADR-041 §4).
///
/// Write-once: once persisted, a receipt MUST NOT be mutated. The system uses
/// receipts to enforce exact-once execution boundaries.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Receipt {
    effect_id: String,
    connector_id: String,
    result: ConnectorResult,
    committed_at_ms: u64,
    payload_json: Option<serde_json::Value>,
}

impl Receipt {
    /// Construct a new `Receipt`.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::InvalidArgument` if `effect_id` or `connector_id` is empty.
    pub fn new(
        effect_id: String,
        connector_id: String,
        result: ConnectorResult,
        committed_at_ms: u64,
        payload_json: Option<serde_json::Value>,
    ) -> Result<Self, ReceiptStoreError> {
        if effect_id.is_empty() || connector_id.is_empty() {
            return Err(ReceiptStoreError::InvalidArgument);
        }
        Ok(Self {
            effect_id,
            connector_id,
            result,
            committed_at_ms,
            payload_json,
        })
    }

    #[must_use]
    pub fn effect_id(&self) -> &str {
        &self.effect_id
    }

    #[must_use]
    pub fn connector_id(&self) -> &str {
        &self.connector_id
    }

    #[must_use]
    pub fn result(&self) -> ConnectorResult {
        self.result
    }

    #[must_use]
    pub const fn committed_at_ms(&self) -> u64 {
        self.committed_at_ms
    }

    #[must_use]
    pub fn payload_json(&self) -> Option<&serde_json::Value> {
        self.payload_json.as_ref()
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
    #[error("receipt for effect {effect_id} already exists")]
    AlreadyExists { effect_id: String },
}

// ---------------------------------------------------------------------------
// Calc layer — key encoding/decoding
// ---------------------------------------------------------------------------

/// Partition name for the receipts store.
pub const RECEIPTS_PARTITION: &str = "receipts";

/// Encode an `EffectId` as UTF-8 bytes for use as a receipt partition key.
#[must_use]
pub fn encode_receipt_key(effect_id: &EffectId) -> Vec<u8> {
    effect_id.as_str().as_bytes().to_vec()
}

/// Decode UTF-8 bytes into an `EffectId`.
///
/// # Errors
///
/// Returns `ReceiptStoreError::Codec` if the bytes are not valid UTF-8 or empty.
pub fn decode_receipt_key(bytes: &[u8]) -> Result<EffectId, ReceiptStoreError> {
    let s = std::str::from_utf8(bytes).map_err(|e| ReceiptStoreError::Codec {
        reason: e.to_string(),
    })?;
    if s.is_empty() {
        return Err(ReceiptStoreError::Codec {
            reason: "empty receipt key".to_string(),
        });
    }
    EffectId::try_from(s.to_string()).map_err(|e| ReceiptStoreError::Codec {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Calc layer — receipt encoding/decoding
// ---------------------------------------------------------------------------

/// Encode a `Receipt` to JSON bytes for storage.
///
/// # Errors
///
/// Returns `ReceiptStoreError::Codec` if serialization fails.
pub fn encode_receipt(receipt: &Receipt) -> Result<Vec<u8>, ReceiptStoreError> {
    serde_json::to_vec(receipt).map_err(|e| ReceiptStoreError::Codec {
        reason: e.to_string(),
    })
}

/// Decode JSON bytes into a `Receipt`.
///
/// # Errors
///
/// Returns `ReceiptStoreError::Codec` if deserialization fails.
pub fn decode_receipt(bytes: &[u8]) -> Result<Receipt, ReceiptStoreError> {
    serde_json::from_slice(bytes).map_err(|e| ReceiptStoreError::Codec {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Actions layer — ReceiptStore trait
// ---------------------------------------------------------------------------

/// Storage interface for connector execution receipts (ADR-041 §4).
///
/// Receipts are write-once and immutable. Storing a receipt for an already-persisted
/// effect ID is idempotent — it MUST return success without mutation.
pub trait ReceiptStore {
    /// Persist a receipt for the given effect ID.
    ///
    /// Idempotent: if a receipt already exists for the effect ID, returns
    /// `Ok` without mutation (no-op success).
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::InvalidArgument` if the receipt's effect_id is empty.
    /// Returns `ReceiptStoreError::Storage` if the underlying storage fails.
    fn store(&self, receipt: Receipt) -> Result<(), ReceiptStoreError>;

    /// Retrieve a receipt by effect ID.
    ///
    /// Returns `Ok(None)` if no receipt exists for the given effect ID.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::Storage` if the underlying storage fails.
    fn get(&self, effect_id: &EffectId) -> Result<Option<Receipt>, ReceiptStoreError>;

    /// Check whether a receipt exists for the given effect ID.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::Storage` if the underlying storage fails.
    fn contains(&self, effect_id: &EffectId) -> Result<bool, ReceiptStoreError>;
}

// ---------------------------------------------------------------------------
// Fjall implementation (stub — no methods, tests will fail)
// ---------------------------------------------------------------------------

pub mod fjall_receipts;
pub use fjall_receipts::FjallReceiptStore;

pub mod in_memory_receipts;
pub use in_memory_receipts::InMemoryReceiptStore;
