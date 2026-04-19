//! In-memory receipt store implementation for testing.

use std::collections::HashMap;
use std::sync::Mutex;

use super::{encode_receipt_key, ConnectorReceipt, ReceiptStore, ReceiptStoreError};
use crate::receipt_store::decode_receipt;

#[derive(Debug, Default)]
pub struct InMemoryReceiptStore {
    receipts: Mutex<HashMap<String, Vec<u8>>>,
}

impl InMemoryReceiptStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            receipts: Mutex::new(HashMap::new()),
        }
    }
}

impl ReceiptStore for InMemoryReceiptStore {
    fn save_receipt(&self, receipt: &ConnectorReceipt) -> Result<(), ReceiptStoreError> {
        let key = encode_receipt_key(receipt.effect_id());
        let mut receipts = self
            .receipts
            .lock()
            .map_err(|e| ReceiptStoreError::Storage {
                reason: e.to_string(),
            })?;
        if receipts.contains_key(&key) {
            return Ok(());
        }
        let bytes = super::encode_receipt(receipt)?;
        receipts.insert(key, bytes);
        Ok(())
    }

    fn get_receipt(&self, effect_id: &str) -> Result<ConnectorReceipt, ReceiptStoreError> {
        let key = encode_receipt_key(effect_id);
        let receipts = self
            .receipts
            .lock()
            .map_err(|e| ReceiptStoreError::Storage {
                reason: e.to_string(),
            })?;
        receipts
            .get(&key)
            .ok_or(ReceiptStoreError::NotFound {
                effect_id: effect_id.to_string(),
            })
            .and_then(|bytes| decode_receipt(bytes))
    }

    fn has_receipt(&self, effect_id: &str) -> Result<bool, ReceiptStoreError> {
        let key = encode_receipt_key(effect_id);
        let receipts = self
            .receipts
            .lock()
            .map_err(|e| ReceiptStoreError::Storage {
                reason: e.to_string(),
            })?;
        Ok(receipts.contains_key(&key))
    }
}

#[cfg(test)]
mod inner_tests {
    use super::*;
    use vo_types::InstanceId;

    fn make_receipt(effect_id: &str) -> ConnectorReceipt {
        ConnectorReceipt::new(
            effect_id.to_string(),
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID"),
            "workflow-1".to_string(),
            "step-1".to_string(),
            "connector-1".to_string(),
            "1.0.0".to_string(),
            serde_json::json!({"status": "ok"}),
            1234567890,
        )
        .expect("valid receipt")
    }

    #[test]
    fn save_and_get_receipt() {
        let store = InMemoryReceiptStore::new();
        let receipt = make_receipt("effect-1");

        store.save_receipt(&receipt).expect("save should succeed");
        let retrieved = store.get_receipt("effect-1").expect("get should succeed");

        assert_eq!(retrieved.effect_id(), receipt.effect_id());
        assert_eq!(retrieved.workflow_id(), receipt.workflow_id());
    }

    #[test]
    fn get_nonexistent_receipt() {
        let store = InMemoryReceiptStore::new();
        let result = store.get_receipt("nonexistent");
        assert!(matches!(result, Err(ReceiptStoreError::NotFound { .. })));
    }

    #[test]
    fn save_receipt_idempotent() {
        let store = InMemoryReceiptStore::new();
        let receipt = make_receipt("effect-1");

        store
            .save_receipt(&receipt)
            .expect("first save should succeed");
        store
            .save_receipt(&receipt)
            .expect("second save should succeed");

        assert!(store
            .has_receipt("effect-1")
            .expect("has_receipt should succeed"));
    }

    #[test]
    fn has_receipt() {
        let store = InMemoryReceiptStore::new();
        let receipt = make_receipt("effect-1");

        assert!(!store
            .has_receipt("effect-1")
            .expect("has_receipt should succeed"));

        store.save_receipt(&receipt).expect("save should succeed");

        assert!(store
            .has_receipt("effect-1")
            .expect("has_receipt should succeed"));
    }
}
