//! In-memory implementation of `ReceiptStore` for testing.

use super::{Receipt, ReceiptStore, ReceiptStoreError};

#[derive(Debug, Default)]
pub struct InMemoryReceiptStore {
    receipts: std::collections::HashMap<String, Receipt>,
}

impl ReceiptStore for InMemoryReceiptStore {
    fn store(&self, _receipt: Receipt) -> Result<(), ReceiptStoreError> {
        Err(ReceiptStoreError::Storage {
            reason: "not implemented".to_string(),
        })
    }

    fn get(
        &self,
        _effect_id: &crate::effect_journal::EffectId,
    ) -> Result<Option<Receipt>, ReceiptStoreError> {
        Err(ReceiptStoreError::Storage {
            reason: "not implemented".to_string(),
        })
    }

    fn contains(
        &self,
        _effect_id: &crate::effect_journal::EffectId,
    ) -> Result<bool, ReceiptStoreError> {
        Err(ReceiptStoreError::Storage {
            reason: "not implemented".to_string(),
        })
    }
}
