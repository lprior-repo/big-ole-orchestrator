//! Fjall-backed persistent implementation of `ReceiptStore` for production use.

use std::sync::Arc;

use vo_types::InstanceId;

use super::{
    decode_receipt, encode_receipt, encode_receipt_key, ExecutionReceipt, ReceiptStore,
    ReceiptStoreError, RECEIPTS_PARTITION,
};

pub struct FjallReceiptStore {
    partition: Arc<fjall::PartitionHandle>,
}

impl std::fmt::Debug for FjallReceiptStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FjallReceiptStore").finish()
    }
}

impl FjallReceiptStore {
    /// Opens a new receipt store backed by the given keyspace.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError::Storage` if the receipts partition cannot be opened.
    pub fn open(keyspace: &fjall::Keyspace) -> Result<Self, ReceiptStoreError> {
        let partition = keyspace
            .open_partition(RECEIPTS_PARTITION, fjall::PartitionCreateOptions::default())
            .map_err(|e| ReceiptStoreError::Storage {
                reason: format!("failed to open receipts partition: {e}"),
            })?;
        Ok(Self {
            partition: Arc::new(partition),
        })
    }
}

impl ReceiptStore for FjallReceiptStore {
    fn store_receipt(&self, receipt: ExecutionReceipt) -> Result<(), ReceiptStoreError> {
        let effect_id = receipt.effect_id();
        if effect_id.is_empty() {
            return Err(ReceiptStoreError::InvalidArgument);
        }

        let key = encode_receipt_key(effect_id);

        if let Ok(Some(_)) = self.partition.get(&key) {
            return Ok(());
        }

        let value = encode_receipt(&receipt)?;
        self.partition
            .insert(&key, &value)
            .map_err(|e| ReceiptStoreError::Storage {
                reason: e.to_string(),
            })
    }

    fn get_receipt(&self, effect_id: &str) -> Result<Option<ExecutionReceipt>, ReceiptStoreError> {
        if effect_id.is_empty() {
            return Err(ReceiptStoreError::InvalidArgument);
        }

        let key = encode_receipt_key(effect_id);
        match self.partition.get(&key) {
            Ok(Some(bytes)) => {
                let receipt = decode_receipt(&bytes)?;
                Ok(Some(receipt))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(ReceiptStoreError::Storage {
                reason: e.to_string(),
            }),
        }
    }

    fn has_receipt(&self, effect_id: &str) -> Result<bool, ReceiptStoreError> {
        if effect_id.is_empty() {
            return Err(ReceiptStoreError::InvalidArgument);
        }

        let key = encode_receipt_key(effect_id);
        match self.partition.get(&key) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(ReceiptStoreError::Storage {
                reason: e.to_string(),
            }),
        }
    }

    fn list_by_instance(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Vec<ExecutionReceipt>, ReceiptStoreError> {
        let prefix = format!("{instance_id}::");
        let prefix_bytes = prefix.as_bytes();
        let mut results = Vec::new();

        let iter = self.partition.iter();
        for item in iter {
            let (key_bytes, value_bytes) = item.map_err(|e| ReceiptStoreError::Storage {
                reason: e.to_string(),
            })?;

            if !key_bytes.starts_with(prefix_bytes) {
                continue;
            }

            let receipt = decode_receipt(&value_bytes)?;
            results.push(receipt);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use vo_types::EffectKind;

    fn create_test_keyspace() -> fjall::Keyspace {
        let dir = tempdir().unwrap();
        fjall::Config::new(dir.path()).open().unwrap()
    }

    fn sample_instance_id() -> InstanceId {
        InstanceId::from_bytes([1u8; 16])
    }

    fn make_receipt(effect_id: &str, instance_id: &InstanceId) -> ExecutionReceipt {
        let full_id = format!("{instance_id}::{effect_id}");
        ExecutionReceipt::new(
            full_id,
            instance_id.to_string(),
            EffectKind::HttpCall,
            1000,
            "Success".to_string(),
        )
        .unwrap()
    }

    #[test]
    fn fjall_store_receipt_and_retrieve() {
        let keyspace = create_test_keyspace();
        let store = FjallReceiptStore::open(&keyspace).unwrap();
        let id = sample_instance_id();
        let receipt = make_receipt("fx-1", &id);

        store.store_receipt(receipt.clone()).unwrap();
        let found = store.get_receipt(receipt.effect_id()).unwrap();

        assert!(found.is_some());
        assert_eq!(found.unwrap(), receipt);
    }

    #[test]
    fn fjall_store_receipt_is_idempotent() {
        let keyspace = create_test_keyspace();
        let store = FjallReceiptStore::open(&keyspace).unwrap();
        let id = sample_instance_id();
        let receipt = make_receipt("fx-2", &id);

        store.store_receipt(receipt.clone()).unwrap();
        store.store_receipt(receipt).unwrap();

        let all = store.list_by_instance(&id).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn fjall_get_nonexistent_receipt_returns_none() {
        let keyspace = create_test_keyspace();
        let store = FjallReceiptStore::open(&keyspace).unwrap();

        let result = store.get_receipt("nonexistent");
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn fjall_has_receipt_returns_true_for_existing() {
        let keyspace = create_test_keyspace();
        let store = FjallReceiptStore::open(&keyspace).unwrap();
        let id = sample_instance_id();

        store.store_receipt(make_receipt("fx-3", &id)).unwrap();

        assert_eq!(store.has_receipt(&format!("{id}::fx-3")), Ok(true));
    }

    #[test]
    fn fjall_has_receipt_returns_false_for_missing() {
        let keyspace = create_test_keyspace();
        let store = FjallReceiptStore::open(&keyspace).unwrap();

        assert_eq!(store.has_receipt("missing"), Ok(false));
    }

    #[test]
    fn fjall_list_by_instance_returns_only_matching_receipts() {
        let keyspace = create_test_keyspace();
        let store = FjallReceiptStore::open(&keyspace).unwrap();
        let id = sample_instance_id();
        let other_id = InstanceId::from_bytes([2u8; 16]);

        store.store_receipt(make_receipt("fx-a", &id)).unwrap();
        store.store_receipt(make_receipt("fx-b", &id)).unwrap();
        store
            .store_receipt(make_receipt("fx-c", &other_id))
            .unwrap();

        let results = store.list_by_instance(&id).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn fjall_store_rejects_empty_effect_id() {
        let keyspace = create_test_keyspace();
        let store = FjallReceiptStore::open(&keyspace).unwrap();
        let receipt = ExecutionReceipt::new(
            "".to_string(),
            "inst-1".to_string(),
            EffectKind::HttpCall,
            1000,
            "Success".to_string(),
        );
        assert!(receipt.is_err());
    }

    #[test]
    fn fjall_get_rejects_empty_effect_id() {
        let keyspace = create_test_keyspace();
        let store = FjallReceiptStore::open(&keyspace).unwrap();

        let result = store.get_receipt("");
        assert_eq!(result, Err(ReceiptStoreError::InvalidArgument));
    }
}
