//! Persistent fjall-backed saga store.
//!
//! This module provides the [`SagaStore`] for durable saga entry persistence
//! and crash recovery.

use serde::{Deserialize, Serialize};

use super::allocation::{SagaEntry, SagaError, SagaStatus};

/// Persistent saga store backed by a fjall keyspace.
pub struct SagaStore {
    partition: fjall::Keyspace,
}

impl SagaStore {
    /// Open the saga_manifest keyspace from a fjall database.
    pub fn open(db: &fjall::Database) -> Result<Self, SagaError> {
        let partition = db
            .keyspace("saga_manifest", fjall::KeyspaceCreateOptions::default)
            .map_err(|e| SagaError::Storage {
                reason: format!("failed to open saga_manifest partition: {e}"),
            })?;
        Ok(Self { partition })
    }

    /// Stage a new saga entry in the store.
    pub fn stage_entry(
        &self,
        write_key: &str,
        class: crate::append::WriteClass,
        size_bytes: u64,
    ) -> Result<(), SagaError> {
        if self.read_entry(write_key)?.is_some() {
            return Err(SagaError::AlreadyExists(write_key.to_string()));
        }
        let entry = SagaEntry {
            write_key: write_key.to_string(),
            class,
            size_bytes,
            status: SagaStatus::Staged,
        };
        let key = format!("entry:{write_key}").into_bytes();
        let value = serde_json::to_vec(&entry).map_err(|e| SagaError::Storage {
            reason: format!("failed to serialize saga entry: {e}"),
        })?;
        self.partition
            .insert(&key, &value)
            .map_err(|e| SagaError::Storage {
                reason: e.to_string(),
            })
    }

    /// Read a saga entry from the store.
    pub fn read_entry(&self, write_key: &str) -> Result<Option<SagaEntry>, SagaError> {
        let key = format!("entry:{write_key}").into_bytes();
        match self.partition.get(&key) {
            Ok(Some(bytes)) => {
                let entry: SagaEntry =
                    serde_json::from_slice(&bytes).map_err(|e| SagaError::CorruptEntry {
                        key: write_key.to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(Some(entry))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(SagaError::Storage {
                reason: e.to_string(),
            }),
        }
    }

    /// Commit a staged entry by transitioning its status.
    pub fn commit_entry(&self, write_key: &str) -> Result<(), SagaError> {
        let mut entry = self
            .read_entry(write_key)?
            .ok_or_else(|| SagaError::NotFound(write_key.to_string()))?;
        if entry.status != SagaStatus::Staged {
            return Err(SagaError::InvalidState {
                key: write_key.to_string(),
                expected: SagaStatus::Staged,
                actual: entry.status,
            });
        }
        entry.status = SagaStatus::Committed;
        let key = format!("entry:{write_key}").into_bytes();
        let value = serde_json::to_vec(&entry).map_err(|e| SagaError::Storage {
            reason: format!("failed to serialize saga entry: {e}"),
        })?;
        self.partition
            .insert(&key, &value)
            .map_err(|e| SagaError::Storage {
                reason: e.to_string(),
            })
    }

    /// Roll back a saga entry.
    pub fn rollback_entry(&self, write_key: &str) -> Result<(), SagaError> {
        let mut entry = self
            .read_entry(write_key)?
            .ok_or_else(|| SagaError::NotFound(write_key.to_string()))?;
        if entry.status == SagaStatus::RolledBack {
            return Err(SagaError::AlreadyRolledBack(write_key.to_string()));
        }
        entry.status = SagaStatus::RolledBack;
        let key = format!("entry:{write_key}").into_bytes();
        let value = serde_json::to_vec(&entry).map_err(|e| SagaError::Storage {
            reason: format!("failed to serialize saga entry: {e}"),
        })?;
        self.partition
            .insert(&key, &value)
            .map_err(|e| SagaError::Storage {
                reason: e.to_string(),
            })
    }

    /// Recover from crash: roll back any entries still in Staged state.
    pub fn recover(&self) -> Result<RecoveryOutcome, SagaError> {
        let mut count = 0usize;
        let iter = self.partition.iter();
        for item in iter {
            let (key_bytes, value_bytes) = item.into_inner().map_err(|e| SagaError::Storage {
                reason: e.to_string(),
            })?;
            let key_str = std::str::from_utf8(&key_bytes).unwrap_or("");
            let Some(write_key) = key_str.strip_prefix("entry:") else {
                continue;
            };
            let mut entry: SagaEntry =
                serde_json::from_slice(&value_bytes).map_err(|e| SagaError::CorruptEntry {
                    key: write_key.to_string(),
                    reason: e.to_string(),
                })?;
            if entry.status == SagaStatus::Staged {
                entry.status = SagaStatus::RolledBack;
                let value = serde_json::to_vec(&entry).map_err(|e| SagaError::Storage {
                    reason: format!("failed to serialize saga entry: {e}"),
                })?;
                self.partition
                    .insert(format!("entry:{write_key}").as_bytes(), &value)
                    .map_err(|e| SagaError::Storage {
                        reason: e.to_string(),
                    })?;
                count += 1;
            }
        }
        if count > 0 {
            Ok(RecoveryOutcome::RolledBack { count })
        } else {
            Ok(RecoveryOutcome::NothingToRecover)
        }
    }
}

/// Result of a crash recovery operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryOutcome {
    /// No staged entries found.
    NothingToRecover,
    /// Entries were rolled back after crash.
    RolledBack { count: usize },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::append::WriteClass;

    fn create_test_db() -> fjall::Database {
        let dir = tempfile::tempdir().expect("tempdir");
        fjall::Database::builder(dir.path()).open().expect("fjall")
    }

    #[test]
    fn saga_store_stage_and_read_entry() {
        let db = create_test_db();
        let store = SagaStore::open(&db).expect("store");

        store
            .stage_entry("key1", WriteClass::CriticalControlPlane, 100)
            .expect("stage");

        let entry = store.read_entry("key1").expect("read").expect("exists");
        assert_eq!(entry.write_key, "key1");
        assert_eq!(entry.status, SagaStatus::Staged);
        assert_eq!(entry.class, WriteClass::CriticalControlPlane);
    }

    #[test]
    fn saga_store_commit_transitions_to_committed() {
        let db = create_test_db();
        let store = SagaStore::open(&db).expect("store");

        store
            .stage_entry("key1", WriteClass::CriticalControlPlane, 100)
            .expect("stage");
        store.commit_entry("key1").expect("commit");

        let entry = store.read_entry("key1").expect("read").expect("exists");
        assert_eq!(entry.status, SagaStatus::Committed);
    }

    #[test]
    fn saga_store_rollback_transitions_to_rolled_back() {
        let db = create_test_db();
        let store = SagaStore::open(&db).expect("store");

        store
            .stage_entry("key1", WriteClass::CriticalControlPlane, 100)
            .expect("stage");
        store.rollback_entry("key1").expect("rollback");

        let entry = store.read_entry("key1").expect("read").expect("exists");
        assert_eq!(entry.status, SagaStatus::RolledBack);
    }

    #[test]
    fn saga_store_recovery_rolls_back_staged_entries() {
        let db = create_test_db();
        let store = SagaStore::open(&db).expect("store");

        store
            .stage_entry("staged1", WriteClass::CriticalControlPlane, 100)
            .expect("stage");
        store
            .stage_entry("committed1", WriteClass::OperatorProjection, 150)
            .expect("stage");
        store.commit_entry("committed1").expect("commit");

        let outcome = store.recover().expect("recover");
        assert_eq!(outcome, RecoveryOutcome::RolledBack { count: 1 });

        let committed = store
            .read_entry("committed1")
            .expect("read")
            .expect("exists");
        assert_eq!(committed.status, SagaStatus::Committed);

        let staged = store.read_entry("staged1").expect("read").expect("exists");
        assert_eq!(staged.status, SagaStatus::RolledBack);
    }

    #[test]
    fn saga_store_recovery_survives_keyspace_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dir_path = dir.path().to_path_buf();

        {
            let db = fjall::Database::builder(&dir_path).open().expect("fjall");
            let store = SagaStore::open(&db).expect("store");
            store
                .stage_entry("key1", WriteClass::CriticalControlPlane, 100)
                .expect("stage");
            store
                .stage_entry("key2", WriteClass::BulkBlob, 200)
                .expect("stage");
        }

        let db = fjall::Database::builder(&dir_path).open().expect("fjall");
        let store = SagaStore::open(&db).expect("store");
        let outcome = store.recover().expect("recover");
        assert_eq!(outcome, RecoveryOutcome::RolledBack { count: 2 });

        let entry1 = store.read_entry("key1").expect("read").expect("exists");
        assert_eq!(entry1.status, SagaStatus::RolledBack);
    }

    #[test]
    fn saga_store_stage_duplicate_fails() {
        let db = create_test_db();
        let store = SagaStore::open(&db).expect("store");

        store
            .stage_entry("key1", WriteClass::CriticalControlPlane, 100)
            .expect("stage");
        let result = store.stage_entry("key1", WriteClass::BulkBlob, 200);
        assert!(matches!(result, Err(SagaError::AlreadyExists(_))));
    }

    #[test]
    fn saga_store_recovery_is_idempotent() {
        let db = create_test_db();
        let store = SagaStore::open(&db).expect("store");

        store
            .stage_entry("key1", WriteClass::CriticalControlPlane, 100)
            .expect("stage");

        let outcome1 = store.recover().expect("recover");
        assert_eq!(outcome1, RecoveryOutcome::RolledBack { count: 1 });

        let outcome2 = store.recover().expect("recover");
        assert_eq!(outcome2, RecoveryOutcome::NothingToRecover);
    }
}
