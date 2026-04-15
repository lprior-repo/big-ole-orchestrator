//! Fjall-backed persistent implementation of `DekStore` for production use.

use std::sync::Arc;

use ulid::Ulid;
use vo_types::{CryptoAlgorithm, DekId, InstanceId, KeyMetadata, WrappedDek};

use super::{DekEntry, DekStatus, DekStore, DekStoreError, DEK_PARTITION};
use crate::crypto::{self, unwrap_dek, wrap_dek};

#[allow(dead_code)]
const DEK_INDEX_PARTITION: &str = "dek_index";

#[allow(dead_code)]
pub struct FjallDekStore {
    dek_partition: Arc<fjall::Keyspace>,
    index_partition: Arc<fjall::Keyspace>,
}

#[allow(dead_code)]
impl FjallDekStore {
    /// Opens the DEK store partitions from the given keyspace.
    ///
    /// # Errors
    ///
    /// Returns `DekStoreError::Storage` if the partition cannot be opened.
    pub fn open(keyspace: &fjall::Keyspace) -> Result<Self, DekStoreError> {
        let dek_partition = keyspace
            .open_partition(DEK_PARTITION, fjall::PartitionCreateOptions::default())
            .map_err(|e| DekStoreError::Storage {
                reason: format!("failed to open dek_store partition: {e}"),
            })?;
        let index_partition = db
            .keyspace(DEK_INDEX_PARTITION, fjall::KeyspaceCreateOptions::default)
            .map_err(|e| DekStoreError::Storage {
                reason: format!("failed to open dek_index partition: {e}"),
            })?;
        Ok(Self {
            dek_partition: Arc::new(dek_partition),
            index_partition: Arc::new(index_partition),
        })
    }

    fn encode_dek_key(dek_id: &DekId) -> Vec<u8> {
        dek_id.as_str().as_bytes().to_vec()
    }

    fn encode_index_key(instance_id: &InstanceId) -> Vec<u8> {
        format!("{instance_id}::active").into_bytes()
    }

    fn get_dek_entry(&self, dek_id: &DekId) -> Result<Option<DekEntry>, DekStoreError> {
        let key = Self::encode_dek_key(dek_id);
        match self.dek_partition.get(&key) {
            Ok(Some(bytes)) => {
                let entry = super::decode_dek_entry(&bytes)?;
                Ok(Some(entry))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(DekStoreError::Storage {
                reason: e.to_string(),
            }),
        }
    }

    fn get_active_dek_id_internal(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Option<DekId>, DekStoreError> {
        let key = Self::encode_index_key(instance_id);
        match self.index_partition.get(&key) {
            Ok(Some(bytes)) => {
                let dek_id_str = std::str::from_utf8(&bytes).map_err(|e| DekStoreError::Codec {
                    reason: format!("invalid UTF-8 in index: {e}"),
                })?;
                let dek_id = DekId::parse(dek_id_str).map_err(|e| DekStoreError::Codec {
                    reason: format!("invalid DekId in index: {e}"),
                })?;
                Ok(Some(dek_id))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(DekStoreError::Storage {
                reason: e.to_string(),
            }),
        }
    }

    fn insert_dek_entry(&self, entry: &DekEntry) -> Result<(), DekStoreError> {
        let key = Self::encode_dek_key(entry.dek_id());
        let value = super::encode_dek_entry(entry);
        self.dek_partition
            .insert(&key, &value)
            .map_err(|e| DekStoreError::Storage {
                reason: format!("failed to insert DEK: {e}"),
            })
    }

    fn set_active_dek_index(
        &self,
        instance_id: &InstanceId,
        dek_id: &DekId,
    ) -> Result<(), DekStoreError> {
        let key = Self::encode_index_key(instance_id);
        let value = dek_id.as_str().as_bytes();
        self.index_partition
            .insert(&key, value)
            .map_err(|e| DekStoreError::Storage {
                reason: format!("failed to update DEK index: {e}"),
            })
    }

    fn clear_active_dek_index(&self, instance_id: &InstanceId) -> Result<(), DekStoreError> {
        let key = Self::encode_index_key(instance_id);
        self.index_partition
            .remove(&key)
            .map_err(|e| DekStoreError::Storage {
                reason: format!("failed to clear DEK index: {e}"),
            })
    }

    fn retire_dek_entry(&self, dek_id: &DekId) -> Result<(), DekStoreError> {
        let key = Self::encode_dek_key(dek_id);
        match self.dek_partition.get(&key) {
            Ok(Some(bytes)) => {
                let mut entry = super::decode_dek_entry(&bytes)?;
                entry.retire();
                let value = super::encode_dek_entry(&entry);
                self.dek_partition
                    .insert(&key, &value)
                    .map_err(|e| DekStoreError::Storage {
                        reason: format!("failed to update retired DEK: {e}"),
                    })
            }
            Ok(None) => Err(DekStoreError::DekNotFound {
                instance_id: dek_id.as_str().to_string(),
            }),
            Err(e) => Err(DekStoreError::Storage {
                reason: e.to_string(),
            }),
        }
    }
}
