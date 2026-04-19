//! Receipt partition — storage interface for managed connector execution receipts (ADR-041).
//!
//! Architecture: Data (`ConnectorReceipt`, `ReceiptStoreError`)
//!               → Calc (`encode_receipt_key`, `decode_receipt_key`, `encode_receipt`, `decode_receipt`)
//!               → Actions (`ReceiptStore` trait).
//!
//! This module defines the trait and pure encoding/decoding functions. Concrete Fjall
//! implementations are provided separately.

use serde::{Deserialize, Serialize};
use vo_types::InstanceId;

#[cfg(all(test, feature = "proptest"))]
mod proptests;
#[cfg(test)]
mod tests;

pub mod in_memory_receipt_store;
pub use in_memory_receipt_store::InMemoryReceiptStore;

// ---------------------------------------------------------------------------
// Data layer — ConnectorReceipt
// ---------------------------------------------------------------------------

/// Durable receipt for a committed managed connector effect.
///
/// Per ADR-041 §4: "Every successful commit returns a durable receipt suitable for
/// operator audit. Receipts must be persisted in `EffectCommitted`."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorReceipt {
    effect_id: String,
    instance_id: InstanceId,
    workflow_id: String,
    step_id: String,
    connector_id: String,
    connector_version: String,
    receipt_data: serde_json::Value,
    committed_at_ms: u64,
}

impl ConnectorReceipt {
    /// Construct a new `ConnectorReceipt`.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::InvalidArgument` if any string field is empty.
    pub fn new(
        effect_id: String,
        instance_id: InstanceId,
        workflow_id: String,
        step_id: String,
        connector_id: String,
        connector_version: String,
        receipt_data: serde_json::Value,
        committed_at_ms: u64,
    ) -> Result<Self, ReceiptStoreError> {
        if effect_id.is_empty() {
            return Err(ReceiptStoreError::InvalidArgument);
        }
        if workflow_id.is_empty() {
            return Err(ReceiptStoreError::InvalidArgument);
        }
        if step_id.is_empty() {
            return Err(ReceiptStoreError::InvalidArgument);
        }
        if connector_id.is_empty() {
            return Err(ReceiptStoreError::InvalidArgument);
        }
        Ok(Self {
            effect_id,
            instance_id,
            workflow_id,
            step_id,
            connector_id,
            connector_version,
            receipt_data,
            committed_at_ms,
        })
    }

    #[must_use]
    pub fn effect_id(&self) -> &str {
        &self.effect_id
    }

    #[must_use]
    pub fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    #[must_use]
    pub fn workflow_id(&self) -> &str {
        &self.workflow_id
    }

    #[must_use]
    pub fn step_id(&self) -> &str {
        &self.step_id
    }

    #[must_use]
    pub fn connector_id(&self) -> &str {
        &self.connector_id
    }

    #[must_use]
    pub fn connector_version(&self) -> &str {
        &self.connector_version
    }

    #[must_use]
    pub fn receipt_data(&self) -> &serde_json::Value {
        &self.receipt_data
    }

    #[must_use]
    pub fn committed_at_ms(&self) -> u64 {
        self.committed_at_ms
    }
}

// ---------------------------------------------------------------------------
// Data layer — error enum
// ---------------------------------------------------------------------------

/// Errors from the receipt store operations.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReceiptStoreError {
    #[error("receipt for effect {effect_id} already exists")]
    AlreadyExists { effect_id: String },
    #[error("receipt for effect {effect_id} not found")]
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

const RECEIPT_KEY_PREFIX: &str = "receipt:";

/// Encode an effect ID as UTF-8 bytes for use as a storage key.
#[must_use]
pub fn encode_receipt_key(effect_id: &str) -> String {
    format!("{RECEIPT_KEY_PREFIX}{effect_id}")
}

/// Decode an effect ID from a storage key.
#[must_use]
pub fn decode_receipt_key(key: &str) -> Option<String> {
    key.strip_prefix(RECEIPT_KEY_PREFIX).map(String::from)
}

/// Encode a `ConnectorReceipt` as bytes for storage.
pub fn encode_receipt(receipt: &ConnectorReceipt) -> Result<Vec<u8>, ReceiptStoreError> {
    serde_json::to_vec(receipt).map_err(|e| ReceiptStoreError::Codec {
        reason: format!("failed to serialize receipt: {}", e),
    })
}

/// Decode a `ConnectorReceipt` from bytes.
pub fn decode_receipt(bytes: &[u8]) -> Result<ConnectorReceipt, ReceiptStoreError> {
    serde_json::from_slice(bytes).map_err(|e| ReceiptStoreError::Codec {
        reason: format!("failed to deserialize receipt: {}", e),
    })
}

// ---------------------------------------------------------------------------
// Action layer — ReceiptStore trait
// ---------------------------------------------------------------------------

/// Trait for managing connector execution receipts.
///
/// Per ADR-041: "Every successful commit returns a durable receipt suitable for
/// operator audit."
pub trait ReceiptStore {
    /// Save a receipt for a committed effect.
    ///
    /// Idempotent: if a receipt already exists for this effect_id, returns `Ok(())`
    /// without modifying the existing receipt.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::InvalidArgument` if receipt validation fails.
    /// Returns `ReceiptStoreError::Storage` if the underlying storage fails.
    fn save_receipt(&self, receipt: &ConnectorReceipt) -> Result<(), ReceiptStoreError>;

    /// Get a receipt by effect ID.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::NotFound` if no receipt exists for this effect_id.
    /// Returns `ReceiptStoreError::Storage` if the underlying storage fails.
    fn get_receipt(&self, effect_id: &str) -> Result<ConnectorReceipt, ReceiptStoreError>;

    /// Check if a receipt exists for an effect ID.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::Storage` if the underlying storage fails.
    fn has_receipt(&self, effect_id: &str) -> Result<bool, ReceiptStoreError>;
}
