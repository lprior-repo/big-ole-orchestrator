//! Fjall-backed persistent implementation of `ReceiptStore` for production use.

use std::sync::Arc;

use vo_types::InstanceId;

use super::{Receipt, ReceiptStore, ReceiptStoreError, RECEIPTS_PARTITION};

pub struct FjallReceiptStore {
    _partition: Arc<fjall::Keyspace>,
}

impl std::fmt::Debug for FjallReceiptStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FjallReceiptStore").finish()
    }
}

impl FjallReceiptStore {
    pub fn open(db: &fjall::Database) -> Result<Self, ReceiptStoreError> {
        let _partition = db
            .keyspace(RECEIPTS_PARTITION, || fjall::KeyspaceCreateOptions::default())
            .map_err(|e| ReceiptStoreError::Storage {
                reason: format!("failed to open receipts partition: {e}"),
            })?;
        Ok(Self {
            _partition: Arc::new(partition),
        })
    }
}

impl ReceiptStore for FjallReceiptStore {
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
