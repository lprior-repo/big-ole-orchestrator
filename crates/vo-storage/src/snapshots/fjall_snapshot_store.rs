//! Fjall-backed persistent implementation of snapshot store for replay acceleration.

use std::sync::Arc;

use vo_types::state::InstanceState;
use vo_types::InstanceId;

use super::SnapshotStoreError;
use crate::partitions::{get_partition_config, SNAPSHOTS_PARTITION};

pub struct FjallSnapshotStore {
    db: Arc<fjall::Database>,
    partition: Arc<fjall::Keyspace>,
}

impl FjallSnapshotStore {
    /// Opens a new snapshot store backed by the given database.
    ///
    /// # Errors
    ///
    /// Returns `SnapshotStoreError::Storage` if the snapshots partition cannot be opened.
    #[must_use]
    pub fn open(db: &fjall::Database) -> Result<Self, SnapshotStoreError> {
        let config = get_partition_config(SNAPSHOTS_PARTITION);
        let partition = db
            .keyspace(SNAPSHOTS_PARTITION, || config.to_fjall_options())
            .map_err(|e| SnapshotStoreError::Storage {
                reason: format!("failed to open snapshots partition: {e}"),
            })?;
        Ok(Self {
            db: Arc::new(db.clone()),
            partition: Arc::new(partition),
        })
    }

    /// Returns the snapshots partition keyspace for direct operations.
    #[must_use]
    pub fn partition(&self) -> &Arc<fjall::Keyspace> {
        &self.partition
    }

    /// Writes a snapshot of `state` at the given `sequence` for `instance_id`.
    ///
    /// # Errors
    ///
    /// Returns `SnapshotStoreError::Codec` if the instance ID cannot be serialized.
    /// Returns `SnapshotStoreError::SerializationFailed` if serialization fails.
    /// Returns `SnapshotStoreError::FjallError` if the storage engine fails.
    pub fn write_snapshot(
        &self,
        instance_id: InstanceId,
        sequence: u64,
        state: &InstanceState,
    ) -> Result<(), SnapshotStoreError> {
        let key = super::encode_snapshot_key(&instance_id, sequence).map_err(|_| {
            SnapshotStoreError::Codec {
                reason: "failed to encode snapshot key".to_string(),
            }
        })?;

        let state_json =
            serde_json::to_vec(state).map_err(|_| SnapshotStoreError::SerializationFailed)?;

        self.partition
            .insert(key, state_json)
            .map_err(|_| SnapshotStoreError::FjallError)
    }

    /// Loads the latest (highest-sequence) snapshot for `instance_id`.
    ///
    /// # Errors
    ///
    /// Returns `SnapshotStoreError::Codec` if the instance ID cannot be serialized.
    /// Returns `SnapshotStoreError::FjallError` if the storage engine fails.
    /// Returns `SnapshotStoreError::InvalidKey` if a stored key is not exactly 24 bytes.
    /// Returns `SnapshotStoreError::DeserializationFailed` if the stored value is not valid JSON.
    #[must_use]
    pub fn load_latest(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Option<(u64, InstanceState)>, SnapshotStoreError> {
        super::snapshot_load_latest(&self.partition, instance_id).map_err(SnapshotStoreError::from)
    }
}

impl std::fmt::Debug for FjallSnapshotStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FjallSnapshotStore").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_db() -> fjall::Database {
        let dir = tempdir().unwrap();
        fjall::Database::builder(dir.path()).open().unwrap()
    }

    fn make_instance_id() -> InstanceId {
        InstanceId::from_bytes([1u8; 16])
    }

    #[test]
    fn fjall_snapshot_store_open_succeeds() {
        let db = create_test_db();
        let store = FjallSnapshotStore::open(&db);
        assert!(store.is_ok());
    }

    #[test]
    fn fjall_snapshot_store_write_and_load_latest() {
        let db = create_test_db();
        let store = FjallSnapshotStore::open(&db).unwrap();
        let instance_id = make_instance_id();
        let state = InstanceState {
            counter: 42,
            status: "Running".to_string(),
            variables: Default::default(),
        };

        let result = store.write_snapshot(instance_id, 100, &state);
        assert!(result.is_ok());

        let loaded = store.load_latest(&instance_id);
        assert!(loaded.is_ok());
        let (seq, loaded_state) = loaded.unwrap().expect("should have a snapshot");
        assert_eq!(seq, 100);
        assert_eq!(loaded_state.counter, 42);
    }

    #[test]
    fn fjall_snapshot_store_load_latest_returns_none_for_new_instance() {
        let db = create_test_db();
        let store = FjallSnapshotStore::open(&db).unwrap();
        let instance_id = make_instance_id();

        let result = store.load_latest(&instance_id);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn fjall_snapshot_store_load_latest_returns_highest_sequence() {
        let db = create_test_db();
        let store = FjallSnapshotStore::open(&db).unwrap();
        let instance_id = make_instance_id();

        store
            .write_snapshot(
                instance_id,
                50,
                &InstanceState {
                    counter: 1,
                    status: "First".to_string(),
                    variables: Default::default(),
                },
            )
            .unwrap();

        store
            .write_snapshot(
                instance_id,
                100,
                &InstanceState {
                    counter: 2,
                    status: "Second".to_string(),
                    variables: Default::default(),
                },
            )
            .unwrap();

        let loaded = store.load_latest(&instance_id).unwrap().unwrap();
        assert_eq!(loaded.0, 100);
        assert_eq!(loaded.1.counter, 2);
    }
}
