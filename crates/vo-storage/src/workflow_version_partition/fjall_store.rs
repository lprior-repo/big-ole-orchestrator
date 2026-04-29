//! Fjall-backed persistent implementation of `WorkflowVersionStore` for production use.

use std::sync::Arc;

use vo_types::BinaryHash;

use super::{
    decode_workflow_version_entry, encode_workflow_version_entry, encode_workflow_version_key,
    WorkflowVersionEntry, WorkflowVersionStore, WorkflowVersionStoreError,
    WORKFLOW_VERSIONS_PARTITION_NAME,
};

pub struct FjallWorkflowVersionStore {
    partition: Arc<fjall::Keyspace>,
}

impl FjallWorkflowVersionStore {
    /// Open a Fjall-backed workflow version store.
    ///
    /// # Errors
    ///
    /// Returns `WorkflowVersionStoreError::Storage` if the partition cannot be opened.
    pub fn open(db: &fjall::Database) -> Result<Self, WorkflowVersionStoreError> {
        let partition = db
            .keyspace(
                WORKFLOW_VERSIONS_PARTITION_NAME,
                fjall::KeyspaceCreateOptions::default,
            )
            .map_err(|e| WorkflowVersionStoreError::Storage {
                reason: format!("failed to open workflow_versions partition: {e}"),
            })?;
        Ok(Self {
            partition: Arc::new(partition),
        })
    }
}

impl WorkflowVersionStore for FjallWorkflowVersionStore {
    fn get(&self, hash: &BinaryHash) -> Result<WorkflowVersionEntry, WorkflowVersionStoreError> {
        let key = encode_workflow_version_key(hash);
        match self.partition.get(&key) {
            Ok(Some(value_bytes)) => decode_workflow_version_entry(&value_bytes),
            Ok(None) => Err(WorkflowVersionStoreError::KeyNotFound {
                hash: hash.to_string(),
            }),
            Err(e) => Err(WorkflowVersionStoreError::Storage {
                reason: e.to_string(),
            }),
        }
    }

    fn put(&self, entry: &WorkflowVersionEntry) -> Result<(), WorkflowVersionStoreError> {
        let key = encode_workflow_version_key(entry.version_hash());
        let value_bytes = encode_workflow_version_entry(entry)?;
        self.partition.insert(&key, &value_bytes).map_err(|e| {
            WorkflowVersionStoreError::Storage {
                reason: e.to_string(),
            }
        })?;
        Ok(())
    }

    fn contains(&self, hash: &BinaryHash) -> Result<bool, WorkflowVersionStoreError> {
        let key = encode_workflow_version_key(hash);
        match self.partition.get(&key) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(WorkflowVersionStoreError::Storage {
                reason: e.to_string(),
            }),
        }
    }

    fn delete(&self, hash: &BinaryHash) -> Result<(), WorkflowVersionStoreError> {
        let key = encode_workflow_version_key(hash);
        match self.partition.get(&key) {
            Ok(Some(_)) => {
                self.partition
                    .remove(&key)
                    .map_err(|e| WorkflowVersionStoreError::Storage {
                        reason: e.to_string(),
                    })?;
                Ok(())
            }
            Ok(None) => Err(WorkflowVersionStoreError::KeyNotFound {
                hash: hash.to_string(),
            }),
            Err(e) => Err(WorkflowVersionStoreError::Storage {
                reason: e.to_string(),
            }),
        }
    }

    fn list_hashes(&self) -> Result<Vec<BinaryHash>, WorkflowVersionStoreError> {
        let mut hashes = Vec::new();
        let iter = self.partition.iter();
        for item in iter {
            let (key_bytes, _value_bytes) =
                item.into_inner()
                    .map_err(|e| WorkflowVersionStoreError::Storage {
                        reason: e.to_string(),
                    })?;
            if let Ok(hash) = super::decode_workflow_version_key(&key_bytes) {
                hashes.push(hash);
            }
        }
        Ok(hashes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::{BinaryHash, TimestampMs, WorkflowName};

    fn make_hash() -> BinaryHash {
        BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap()
    }

    fn make_name(s: &str) -> WorkflowName {
        WorkflowName::parse(s).unwrap()
    }

    fn make_ts(ms: u64) -> TimestampMs {
        TimestampMs::try_from(ms).unwrap()
    }

    fn make_entry() -> WorkflowVersionEntry {
        WorkflowVersionEntry::new(
            make_name("test-workflow"),
            make_hash(),
            1,
            make_ts(1_712_200_000_000u64),
            "/var/wtf/versions/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/test-workflow".to_string(),
        ).unwrap()
    }

    #[test]
    fn workflow_version_store_get_returns_entry_when_exists() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallWorkflowVersionStore::open(&db).unwrap();
        let entry = make_entry();

        store.put(&entry).unwrap();

        let retrieved = store.get(entry.version_hash()).unwrap();
        assert_eq!(retrieved.workflow_name(), entry.workflow_name());
        assert_eq!(retrieved.version_hash(), entry.version_hash());
        assert_eq!(retrieved.schema_version(), entry.schema_version());
        assert_eq!(retrieved.registered_at(), entry.registered_at());
        assert_eq!(retrieved.binary_path(), entry.binary_path());
    }

    #[test]
    fn workflow_version_store_get_returns_not_found_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallWorkflowVersionStore::open(&db).unwrap();

        let result = store.get(&make_hash());
        assert!(matches!(
            result,
            Err(WorkflowVersionStoreError::KeyNotFound { .. })
        ));
    }

    #[test]
    fn workflow_version_store_put_and_contains() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallWorkflowVersionStore::open(&db).unwrap();
        let entry = make_entry();

        assert!(!store.contains(entry.version_hash()).unwrap());

        store.put(&entry).unwrap();

        assert!(store.contains(entry.version_hash()).unwrap());
    }

    #[test]
    fn workflow_version_store_delete_removes_entry() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallWorkflowVersionStore::open(&db).unwrap();
        let entry = make_entry();

        store.put(&entry).unwrap();
        assert!(store.contains(entry.version_hash()).unwrap());

        store.delete(entry.version_hash()).unwrap();
        assert!(!store.contains(entry.version_hash()).unwrap());
    }

    #[test]
    fn workflow_version_store_delete_returns_not_found_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallWorkflowVersionStore::open(&db).unwrap();

        let result = store.delete(&make_hash());
        assert!(matches!(
            result,
            Err(WorkflowVersionStoreError::KeyNotFound { .. })
        ));
    }

    #[test]
    fn workflow_version_store_list_hashes_returns_all_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallWorkflowVersionStore::open(&db).unwrap();

        let hash1 =
            BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .unwrap();
        let hash2 =
            BinaryHash::parse("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
                .unwrap();

        let entry1 = WorkflowVersionEntry::new(
            make_name("workflow-a"),
            hash1.clone(),
            1,
            make_ts(1000),
            "/path/a".to_string(),
        )
        .unwrap();
        let entry2 = WorkflowVersionEntry::new(
            make_name("workflow-b"),
            hash2.clone(),
            1,
            make_ts(2000),
            "/path/b".to_string(),
        )
        .unwrap();

        store.put(&entry1).unwrap();
        store.put(&entry2).unwrap();

        let hashes = store.list_hashes().unwrap();
        assert_eq!(hashes.len(), 2);
        assert!(hashes.contains(&hash1));
        assert!(hashes.contains(&hash2));
    }

    #[test]
    fn workflow_version_store_list_hashes_returns_empty_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallWorkflowVersionStore::open(&db).unwrap();

        let hashes = store.list_hashes().unwrap();
        assert!(hashes.is_empty());
    }

    #[test]
    fn workflow_version_entry_rejects_empty_binary_path() {
        let result = WorkflowVersionEntry::new(
            make_name("test"),
            make_hash(),
            1,
            make_ts(1000),
            String::new(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn workflow_version_store_persists_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallWorkflowVersionStore::open(&db).unwrap();
        let entry = make_entry();

        store.put(&entry).unwrap();
        drop(store);
        drop(db);

        let db2 = fjall::Database::builder(dir.path()).open().unwrap();
        let store2 = FjallWorkflowVersionStore::open(&db2).unwrap();

        let retrieved = store2.get(entry.version_hash()).unwrap();
        assert_eq!(retrieved.version_hash(), entry.version_hash());
    }
}
