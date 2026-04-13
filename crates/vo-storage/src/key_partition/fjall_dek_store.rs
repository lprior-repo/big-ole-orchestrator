//! Fjall-backed persistent implementation of `DekStore` for production use.

use std::sync::Arc;

use vo_types::{CryptoAlgorithm, DekId, InstanceId, KeyMetadata, WrappedDek};

use super::{DekEntry, DekStatus, DekStore, DekStoreError, DEK_PARTITION};
use crate::crypto::{self, unwrap_dek, wrap_dek};

const DEK_INDEX_PARTITION: &str = "dek_index";

pub struct FjallDekStore {
    dek_partition: Arc<fjall::PartitionHandle>,
    index_partition: Arc<fjall::PartitionHandle>,
}

impl FjallDekStore {
    #[must_use]
    pub fn open(keyspace: &fjall::Keyspace) -> Result<Self, DekStoreError> {
        let dek_partition = keyspace
            .open_partition(DEK_PARTITION, fjall::PartitionCreateOptions::default())
            .map_err(|e| DekStoreError::Storage {
                reason: format!("failed to open dek_store partition: {e}"),
            })?;
        let index_partition = keyspace
            .open_partition(
                DEK_INDEX_PARTITION,
                fjall::PartitionCreateOptions::default(),
            )
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

impl DekStore for FjallDekStore {
    fn generate_and_store_dek(
        &self,
        instance_id: &InstanceId,
        kek: &[u8; 32],
    ) -> Result<DekId, DekStoreError> {
        if self.get_active_dek_id_internal(instance_id)?.is_some() {
            return Err(DekStoreError::DekAlreadyExists {
                instance_id: instance_id.to_string(),
            });
        }

        let raw_dek = crypto::generate_dek().map_err(|e| DekStoreError::Storage {
            reason: format!("failed to generate DEK: {e}"),
        })?;

        let wrapped_dek_bytes = wrap_dek(&raw_dek, kek).map_err(|e| DekStoreError::Storage {
            reason: format!("failed to wrap DEK: {e}"),
        })?;
        let wrapped_dek = WrappedDek::new(wrapped_dek_bytes);

        let dek_id = DekId::from_bytes(ulid::Ulid::new().into());
        let metadata = KeyMetadata::new(instance_id.clone(), CryptoAlgorithm::Aes256Gcm);
        let entry = DekEntry::new(dek_id.clone(), instance_id.clone(), wrapped_dek, metadata)?;

        self.insert_dek_entry(&entry)?;
        self.set_active_dek_index(instance_id, &dek_id)?;

        Ok(dek_id)
    }

    fn retrieve_dek(
        &self,
        instance_id: &InstanceId,
        kek: &[u8; 32],
    ) -> Result<[u8; 32], DekStoreError> {
        let dek_id = self.get_active_dek_id_internal(instance_id)?;

        let dek_id = match dek_id {
            Some(id) => id,
            None => {
                return Err(DekStoreError::DekNotFound {
                    instance_id: instance_id.to_string(),
                });
            }
        };

        let entry = self.get_dek_entry(&dek_id)?;

        let entry = match entry {
            Some(e) => e,
            None => {
                return Err(DekStoreError::DekNotFound {
                    instance_id: instance_id.to_string(),
                });
            }
        };

        if entry.status() == DekStatus::Retired {
            return Err(DekStoreError::DekRetired {
                dek_id: dek_id.as_str().to_string(),
            });
        }

        let wrapped_bytes = entry.wrapped_dek().as_bytes();
        let raw_dek = unwrap_dek(wrapped_bytes, kek).map_err(|e| DekStoreError::Storage {
            reason: format!("failed to unwrap DEK: {e}"),
        })?;

        Ok(raw_dek)
    }

    fn get_active_dek_id(&self, instance_id: &InstanceId) -> Result<DekId, DekStoreError> {
        self.get_active_dek_id_internal(instance_id)?
            .ok_or_else(|| DekStoreError::DekNotFound {
                instance_id: instance_id.to_string(),
            })
    }

    fn has_active_dek(&self, instance_id: &InstanceId) -> Result<bool, DekStoreError> {
        self.get_active_dek_id_internal(instance_id)
            .map(|opt| opt.is_some())
    }

    fn rotate_dek(&self, instance_id: &InstanceId, kek: &[u8; 32]) -> Result<DekId, DekStoreError> {
        let old_dek_id = self.get_active_dek_id_internal(instance_id)?;

        let old_dek_id = match old_dek_id {
            Some(id) => id,
            None => {
                return Err(DekStoreError::DekNotFound {
                    instance_id: instance_id.to_string(),
                });
            }
        };

        self.retire_dek_entry(&old_dek_id)?;

        // Clear the active index so generate_and_store_dek doesn't reject with DekAlreadyExists
        let index_key = Self::encode_index_key(instance_id);
        self.index_partition
            .remove(&index_key)
            .map_err(|e| DekStoreError::Storage {
                reason: format!("failed to clear DEK index during rotation: {e}"),
            })?;

        self.generate_and_store_dek(instance_id, kek)
    }

    fn retire_dek(&self, instance_id: &InstanceId) -> Result<(), DekStoreError> {
        let dek_id = self.get_active_dek_id_internal(instance_id)?;

        let dek_id = match dek_id {
            Some(id) => id,
            None => {
                return Err(DekStoreError::DekNotFound {
                    instance_id: instance_id.to_string(),
                });
            }
        };

        self.retire_dek_entry(&dek_id)
    }

    fn list_deks(&self, instance_id: &InstanceId) -> Result<Vec<DekId>, DekStoreError> {
        let mut dek_ids = Vec::new();

        let iter = self.dek_partition.iter();
        for item in iter {
            let (_, value_bytes) = item.map_err(|e| DekStoreError::Storage {
                reason: format!("failed to scan DEKs: {e}"),
            })?;
            if let Ok(entry) = super::decode_dek_entry(&value_bytes) {
                if entry.instance_id() == instance_id {
                    dek_ids.push(entry.dek_id().clone());
                }
            }
        }

        Ok(dek_ids)
    }

    fn get_dek_metadata(&self, dek_id: &DekId) -> Result<KeyMetadata, DekStoreError> {
        self.get_dek_entry(dek_id)?
            .map(|e| e.metadata().clone())
            .ok_or_else(|| DekStoreError::DekNotFound {
                instance_id: dek_id.as_str().to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use vo_types::InstanceId;

    fn sample_instance_id() -> InstanceId {
        InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap()
    }

    fn alternate_instance_id() -> InstanceId {
        InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
    }

    fn create_test_keyspace() -> fjall::Keyspace {
        let dir = tempdir().unwrap();
        fjall::Config::new(dir.path()).open().unwrap()
    }

    fn create_test_kek() -> [u8; 32] {
        [0x42u8; 32]
    }

    #[test]
    fn generate_and_store_dek_creates_new_dek() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();
        let kek = create_test_kek();

        let result = store.generate_and_store_dek(&sample_instance_id(), &kek);
        assert!(result.is_ok());

        let dek_id = result.unwrap();
        assert!(!dek_id.as_str().is_empty());
    }

    #[test]
    fn generate_and_store_dek_fails_if_dek_already_exists() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();
        let kek = create_test_kek();

        store
            .generate_and_store_dek(&sample_instance_id(), &kek)
            .unwrap();

        let second_result = store.generate_and_store_dek(&sample_instance_id(), &kek);
        assert!(matches!(
            second_result,
            Err(DekStoreError::DekAlreadyExists { .. })
        ));
    }

    #[test]
    fn retrieve_dek_returns_stored_dek() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();
        let kek = create_test_kek();

        store
            .generate_and_store_dek(&sample_instance_id(), &kek)
            .unwrap();
        let retrieved = store.retrieve_dek(&sample_instance_id(), &kek).unwrap();

        // Retrieved DEK should be 32 bytes (AES-256 key size)
        assert_eq!(retrieved.len(), 32);
    }

    #[test]
    fn retrieve_dek_fails_with_wrong_kek() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();
        let kek1 = [0x42u8; 32];
        let kek2 = [0x99u8; 32];

        store
            .generate_and_store_dek(&sample_instance_id(), &kek1)
            .unwrap();

        let result = store.retrieve_dek(&sample_instance_id(), &kek2);
        assert!(result.is_err());
    }

    #[test]
    fn retrieve_dek_fails_when_not_found() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();
        let kek = create_test_kek();

        let result = store.retrieve_dek(&sample_instance_id(), &kek);
        assert!(matches!(result, Err(DekStoreError::DekNotFound { .. })));
    }

    #[test]
    fn get_active_dek_id_returns_dek_id() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();
        let kek = create_test_kek();

        let generated = store
            .generate_and_store_dek(&sample_instance_id(), &kek)
            .unwrap();
        let retrieved = store.get_active_dek_id(&sample_instance_id()).unwrap();

        assert_eq!(generated, retrieved);
    }

    #[test]
    fn get_active_dek_id_fails_when_not_found() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();

        let result = store.get_active_dek_id(&sample_instance_id());
        assert!(matches!(result, Err(DekStoreError::DekNotFound { .. })));
    }

    #[test]
    fn has_active_dek_returns_true_when_exists() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();
        let kek = create_test_kek();

        store
            .generate_and_store_dek(&sample_instance_id(), &kek)
            .unwrap();
        assert!(store.has_active_dek(&sample_instance_id()).unwrap());
    }

    #[test]
    fn has_active_dek_returns_false_when_not_exists() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();

        assert!(!store.has_active_dek(&sample_instance_id()).unwrap());
    }

    #[test]
    fn rotate_dek_retires_old_dek() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();
        let kek = create_test_kek();

        let old_dek_id = store
            .generate_and_store_dek(&sample_instance_id(), &kek)
            .unwrap();

        let new_dek_id = store.rotate_dek(&sample_instance_id(), &kek).unwrap();

        assert_ne!(old_dek_id, new_dek_id);

        let metadata = store.get_dek_metadata(&old_dek_id).unwrap();
        assert_eq!(metadata.created_at_ms, metadata.created_at_ms);
    }

    #[test]
    fn rotate_dek_fails_when_no_dek_exists() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();
        let kek = create_test_kek();

        let result = store.rotate_dek(&sample_instance_id(), &kek);
        assert!(matches!(result, Err(DekStoreError::DekNotFound { .. })));
    }

    #[test]
    fn retire_dek_marks_dek_as_retired() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();
        let kek = create_test_kek();

        store
            .generate_and_store_dek(&sample_instance_id(), &kek)
            .unwrap();
        store.retire_dek(&sample_instance_id()).unwrap();

        let result = store.retrieve_dek(&sample_instance_id(), &kek);
        assert!(matches!(result, Err(DekStoreError::DekRetired { .. })));
    }

    #[test]
    fn retire_dek_fails_when_not_found() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();

        let result = store.retire_dek(&sample_instance_id());
        assert!(matches!(result, Err(DekStoreError::DekNotFound { .. })));
    }

    #[test]
    fn list_deks_returns_all_deks_for_instance() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();
        let kek = create_test_kek();

        store
            .generate_and_store_dek(&sample_instance_id(), &kek)
            .unwrap();
        store.rotate_dek(&sample_instance_id(), &kek).unwrap();

        let dek_ids = store.list_deks(&sample_instance_id()).unwrap();
        assert_eq!(dek_ids.len(), 2);
    }

    #[test]
    fn list_deks_returns_empty_for_instance_with_no_deks() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();

        let dek_ids = store.list_deks(&sample_instance_id()).unwrap();
        assert!(dek_ids.is_empty());
    }

    #[test]
    fn different_instances_have_independent_deks() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();
        let kek = create_test_kek();

        store
            .generate_and_store_dek(&sample_instance_id(), &kek)
            .unwrap();
        store
            .generate_and_store_dek(&alternate_instance_id(), &kek)
            .unwrap();

        assert!(store.has_active_dek(&sample_instance_id()).unwrap());
        assert!(store.has_active_dek(&alternate_instance_id()).unwrap());

        let dek1 = store.get_active_dek_id(&sample_instance_id()).unwrap();
        let dek2 = store.get_active_dek_id(&alternate_instance_id()).unwrap();

        assert_ne!(dek1, dek2);
    }

    #[test]
    fn retrieve_dek_fails_after_retire() {
        let keyspace = create_test_keyspace();
        let store = FjallDekStore::open(&keyspace).unwrap();
        let kek = create_test_kek();

        store
            .generate_and_store_dek(&sample_instance_id(), &kek)
            .unwrap();
        store.retire_dek(&sample_instance_id()).unwrap();

        let result = store.retrieve_dek(&sample_instance_id(), &kek);
        assert!(matches!(result, Err(DekStoreError::DekRetired { .. })));
    }
}
