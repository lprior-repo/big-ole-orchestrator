//! In-memory implementation of `ReceiptStore` for testing and development.

use std::collections::HashMap;
use std::sync::Mutex;

use vo_types::{EffectKind, InstanceId};

use super::{ExecutionReceipt, ReceiptStore, ReceiptStoreError};

/// In-memory implementation of `ReceiptStore` for testing and development.
#[derive(Debug, Default)]
pub struct InMemoryReceiptStore {
    receipts: Mutex<HashMap<String, ExecutionReceipt>>,
}

impl InMemoryReceiptStore {
    /// Creates a new empty `InMemoryReceiptStore`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            receipts: Mutex::new(HashMap::new()),
        }
    }
}

#[allow(clippy::significant_drop_tightening)]
impl ReceiptStore for InMemoryReceiptStore {
    fn store_receipt(&self, receipt: ExecutionReceipt) -> Result<(), ReceiptStoreError> {
        let effect_id = receipt.effect_id().to_string();
        if effect_id.is_empty() {
            return Err(ReceiptStoreError::InvalidArgument);
        }

        let mut receipts = self
            .receipts
            .lock()
            .map_err(|e| ReceiptStoreError::Storage {
                reason: e.to_string(),
            })?;

        if receipts.contains_key(&effect_id) {
            return Ok(());
        }

        receipts.insert(effect_id, receipt);
        Ok(())
    }

    fn get_receipt(&self, effect_id: &str) -> Result<Option<ExecutionReceipt>, ReceiptStoreError> {
        if effect_id.is_empty() {
            return Err(ReceiptStoreError::InvalidArgument);
        }

        let receipts = self
            .receipts
            .lock()
            .map_err(|e| ReceiptStoreError::Storage {
                reason: e.to_string(),
            })?;
        Ok(receipts.get(effect_id).cloned())
    }

    fn has_receipt(&self, effect_id: &str) -> Result<bool, ReceiptStoreError> {
        if effect_id.is_empty() {
            return Err(ReceiptStoreError::InvalidArgument);
        }

        let receipts = self
            .receipts
            .lock()
            .map_err(|e| ReceiptStoreError::Storage {
                reason: e.to_string(),
            })?;
        Ok(receipts.contains_key(effect_id))
    }

    fn list_by_instance(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Vec<ExecutionReceipt>, ReceiptStoreError> {
        let receipts = self
            .receipts
            .lock()
            .map_err(|e| ReceiptStoreError::Storage {
                reason: e.to_string(),
            })?;

        let prefix = format!("{instance_id}::");
        Ok(receipts
            .values()
            .filter(|r| r.effect_id().starts_with(&prefix))
            .cloned()
            .collect())
    }
}
