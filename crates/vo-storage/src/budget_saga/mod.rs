//! Durable dual-write saga for `WriteBudget`.
//!
//! This module implements ADR-034 saga compensation and reversibility for the
//! `BudgetQueues` write path. It provides:
//!
//! - **Atomic staging**: Writes are first placed in a durable staging area
//! - **Manifest update**: Commit by updating the manifest atomically
//! - **Compensating rollback**: If commit fails, rollback the staging
//! - **Crash recovery**: Recover consistent state after process crash mid-saga
//!
//! ## Saga States
//!
//! Each budget reservation goes through:
//! 1. `Staged` - Written to staging, awaiting commit
//! 2. `Committed` - Manifest updated, write is permanent
//! 3. `RolledBack` - Compensating action completed
//!
//! ## Crash Safety
//!
//! On process crash mid-saga:
//! - Staged writes that were not committed are recovered as rolled back
//! - The manifest is the source of truth for committed state

mod allocation;
mod consumption;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::append::{BudgetQueues, ClassifiedWrite, WriteClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SagaStatus {
    Staged,
    Committed,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaEntry {
    pub write_key: String,
    pub class: WriteClass,
    pub size_bytes: u64,
    pub status: SagaStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BudgetManifest {
    entries: HashMap<String, SagaEntry>,
    version: u64,
}

impl BudgetManifest {
    pub fn stage(
        &mut self,
        write_key: String,
        class: WriteClass,
        size_bytes: u64,
    ) -> Result<(), SagaError> {
        if self.entries.contains_key(&write_key) {
            return Err(SagaError::AlreadyExists(write_key));
        }
        self.entries.insert(
            write_key.clone(),
            SagaEntry {
                write_key,
                class,
                size_bytes,
                status: SagaStatus::Staged,
            },
        );
        Ok(())
    }

    pub fn commit(&mut self, write_key: &str) -> Result<(), SagaError> {
        let entry = self
            .entries
            .get_mut(write_key)
            .ok_or_else(|| SagaError::NotFound(write_key.to_string()))?;
        if entry.status != SagaStatus::Staged {
            return Err(SagaError::InvalidState {
                key: write_key.to_string(),
                expected: SagaStatus::Staged,
                actual: entry.status,
            });
        }
        entry.status = SagaStatus::Committed;
        self.version += 1;
        Ok(())
    }

    pub fn rollback(&mut self, write_key: &str) -> Result<(), SagaError> {
        let entry = self
            .entries
            .get_mut(write_key)
            .ok_or_else(|| SagaError::NotFound(write_key.to_string()))?;
        if entry.status == SagaStatus::RolledBack {
            return Err(SagaError::AlreadyRolledBack(write_key.to_string()));
        }
        entry.status = SagaStatus::RolledBack;
        self.version += 1;
        Ok(())
    }

    #[must_use]
    pub fn get(&self, write_key: &str) -> Option<&SagaEntry> {
        self.entries.get(write_key)
    }

    pub fn staged_entries(&self) -> impl Iterator<Item = &SagaEntry> {
        self.entries
            .values()
            .filter(|e| e.status == SagaStatus::Staged)
    }

    pub fn committed_entries(&self) -> impl Iterator<Item = &SagaEntry> {
        self.entries
            .values()
            .filter(|e| e.status == SagaStatus::Committed)
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn recover_staged_as_rolled_back(&mut self) {
        for entry in self.entries.values_mut() {
            if entry.status == SagaStatus::Staged {
                entry.status = SagaStatus::RolledBack;
            }
        }
        self.version += 1;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SagaError {
    #[error("saga entry already exists: {0}")]
    AlreadyExists(String),
    #[error("saga entry not found: {0}")]
    NotFound(String),
    #[error("saga entry already rolled back: {0}")]
    AlreadyRolledBack(String),
    #[error("invalid state for {key}: expected {expected:?}, got {actual:?}")]
    InvalidState {
        key: String,
        expected: SagaStatus,
        actual: SagaStatus,
    },
    #[error("budget reserve failed: {0}")]
    BudgetReserveFailed(String),
    #[error("storage error: {reason}")]
    Storage { reason: String },
    #[error("corrupt saga entry for key {key}: {reason}")]
    CorruptEntry { key: String, reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryOutcome {
    NothingToRecover,
    RolledBack { count: usize },
}

pub struct SagaStore {
    partition: fjall::Keyspace,
}

impl SagaStore {
    pub fn open(db: &fjall::Database) -> Result<Self, SagaError> {
        let partition = db
            .keyspace("saga_manifest", fjall::KeyspaceCreateOptions::default)
            .map_err(|e| SagaError::Storage {
                reason: format!("failed to open saga_manifest partition: {e}"),
            })?;
        Ok(Self { partition })
    }

    pub fn stage_entry(
        &self,
        write_key: &str,
        class: WriteClass,
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

pub struct DurableBudgetSaga {
    pub(crate) store: Option<SagaStore>,
    pub(crate) manifest: Arc<Mutex<BudgetManifest>>,
    pub(crate) queues: BudgetQueues<StagedWrite>,
}

#[derive(Debug, Clone)]
pub struct StagedWrite {
    pub write_key: String,
    pub class: WriteClass,
    pub size_bytes: u64,
    pub staged_at: std::time::SystemTime,
}

impl StagedWrite {
    #[must_use]
    pub fn new(write_key: String, class: WriteClass, size_bytes: u64) -> Self {
        Self {
            write_key,
            class,
            size_bytes,
            staged_at: std::time::SystemTime::now(),
        }
    }
}

impl ClassifiedWrite for StagedWrite {
    fn write_class(&self) -> WriteClass {
        self.class
    }

    fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

impl DurableBudgetSaga {
    pub fn new(queues: BudgetQueues<StagedWrite>) -> Self {
        Self {
            store: None,
            manifest: Arc::new(Mutex::new(BudgetManifest::default())),
            queues,
        }
    }

    pub fn open(
        db: &fjall::Database,
        queues: BudgetQueues<StagedWrite>,
    ) -> Result<Self, SagaError> {
        let store = SagaStore::open(db)?;
        Ok(Self {
            store: Some(store),
            manifest: Arc::new(Mutex::new(BudgetManifest::default())),
            queues,
        })
    }

    #[must_use]
    pub const fn store(&self) -> &Option<SagaStore> {
        &self.store
    }

    pub fn with_manifest(queues: BudgetQueues<StagedWrite>, manifest: BudgetManifest) -> Self {
        Self {
            store: None,
            manifest: Arc::new(Mutex::new(manifest)),
            queues,
        }
    }

    pub fn manifest(&self) -> Arc<Mutex<BudgetManifest>> {
        Arc::clone(&self.manifest)
    }

    pub const fn queues(&self) -> &BudgetQueues<StagedWrite> {
        &self.queues
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_queues() -> BudgetQueues<StagedWrite> {
        let config = crate::append::QueueConfig::default();
        let budget = crate::append::WriteBudget::new(1000, 1000, 1000);
        BudgetQueues::new(&config, budget)
    }

    #[test]
    fn manifest_stage_and_commit() {
        let mut manifest = BudgetManifest::default();
        manifest
            .stage("key1".to_string(), WriteClass::CriticalControlPlane, 100)
            .unwrap();

        let entry = manifest.get("key1").unwrap();
        assert_eq!(entry.status, SagaStatus::Staged);

        manifest.commit("key1").unwrap();
        let entry = manifest.get("key1").unwrap();
        assert_eq!(entry.status, SagaStatus::Committed);
    }

    #[test]
    fn manifest_stage_and_rollback() {
        let mut manifest = BudgetManifest::default();
        manifest
            .stage("key1".to_string(), WriteClass::CriticalControlPlane, 100)
            .unwrap();

        manifest.rollback("key1").unwrap();
        let entry = manifest.get("key1").unwrap();
        assert_eq!(entry.status, SagaStatus::RolledBack);
    }

    #[test]
    fn manifest_recover_staged_as_rolled_back() {
        let mut manifest = BudgetManifest::default();
        manifest
            .stage("key1".to_string(), WriteClass::CriticalControlPlane, 100)
            .unwrap();
        manifest
            .stage("key2".to_string(), WriteClass::BulkBlob, 200)
            .unwrap();
        manifest.commit("key1").unwrap();

        manifest.recover_staged_as_rolled_back();

        assert_eq!(manifest.get("key1").unwrap().status, SagaStatus::Committed);
        assert_eq!(manifest.get("key2").unwrap().status, SagaStatus::RolledBack);
    }

    #[test]
    fn manifest_commit_non_existent_fails() {
        let mut manifest = BudgetManifest::default();
        let result = manifest.commit("nonexistent");
        assert!(matches!(result, Err(SagaError::NotFound(_))));
    }

    #[test]
    fn manifest_double_commit_fails() {
        let mut manifest = BudgetManifest::default();
        manifest
            .stage("key1".to_string(), WriteClass::CriticalControlPlane, 100)
            .unwrap();
        manifest.commit("key1").unwrap();
        let result = manifest.commit("key1");
        assert!(matches!(result, Err(SagaError::InvalidState { .. })));
    }

    #[test]
    fn durable_saga_stage_and_commit() {
        let queues = create_test_queues();
        let saga = DurableBudgetSaga::new(queues);

        saga.stage_write("key1", WriteClass::CriticalControlPlane, 100)
            .unwrap();
        saga.commit("key1").unwrap();

        let manifest_ref = saga.manifest();
        let manifest = manifest_ref.lock().unwrap();
        assert_eq!(manifest.get("key1").unwrap().status, SagaStatus::Committed);
    }

    #[test]
    fn durable_saga_rollback() {
        let queues = create_test_queues();
        let saga = DurableBudgetSaga::new(queues);

        saga.stage_write("key1", WriteClass::CriticalControlPlane, 100)
            .unwrap();
        saga.rollback("key1").unwrap();

        let manifest_ref = saga.manifest();
        let manifest = manifest_ref.lock().unwrap();
        assert_eq!(manifest.get("key1").unwrap().status, SagaStatus::RolledBack);
    }

    #[test]
    fn saga_store_stage_and_read_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let keyspace = fjall::Database::builder(dir.path())
            .open()
            .expect("keyspace");
        let store = SagaStore::open(&keyspace).expect("store");

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
        let dir = tempfile::tempdir().expect("tempdir");
        let keyspace = fjall::Database::builder(dir.path())
            .open()
            .expect("keyspace");
        let store = SagaStore::open(&keyspace).expect("store");

        store
            .stage_entry("key1", WriteClass::CriticalControlPlane, 100)
            .expect("stage");
        store.commit_entry("key1").expect("commit");

        let entry = store.read_entry("key1").expect("read").expect("exists");
        assert_eq!(entry.status, SagaStatus::Committed);
    }

    #[test]
    fn saga_store_rollback_transitions_to_rolled_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let keyspace = fjall::Database::builder(dir.path())
            .open()
            .expect("keyspace");
        let store = SagaStore::open(&keyspace).expect("store");

        store
            .stage_entry("key1", WriteClass::CriticalControlPlane, 100)
            .expect("stage");
        store.rollback_entry("key1").expect("rollback");

        let entry = store.read_entry("key1").expect("read").expect("exists");
        assert_eq!(entry.status, SagaStatus::RolledBack);
    }

    #[test]
    fn saga_store_recovery_rolls_back_staged_entries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let keyspace = fjall::Database::builder(dir.path())
            .open()
            .expect("keyspace");
        let store = SagaStore::open(&keyspace).expect("store");

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
            let keyspace = fjall::Database::builder(&dir_path)
                .open()
                .expect("keyspace");
            let store = SagaStore::open(&keyspace).expect("store");
            store
                .stage_entry("key1", WriteClass::CriticalControlPlane, 100)
                .expect("stage");
            store
                .stage_entry("key2", WriteClass::BulkBlob, 200)
                .expect("stage");
        }

        let keyspace = fjall::Database::builder(&dir_path)
            .open()
            .expect("keyspace");
        let store = SagaStore::open(&keyspace).expect("store");
        let outcome = store.recover().expect("recover");
        assert_eq!(outcome, RecoveryOutcome::RolledBack { count: 2 });

        let entry1 = store.read_entry("key1").expect("read").expect("exists");
        assert_eq!(entry1.status, SagaStatus::RolledBack);
    }

    #[test]
    fn saga_store_stage_duplicate_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let keyspace = fjall::Database::builder(dir.path())
            .open()
            .expect("keyspace");
        let store = SagaStore::open(&keyspace).expect("store");

        store
            .stage_entry("key1", WriteClass::CriticalControlPlane, 100)
            .expect("stage");
        let result = store.stage_entry("key1", WriteClass::BulkBlob, 200);
        assert!(matches!(result, Err(SagaError::AlreadyExists(_))));
    }

    #[test]
    fn saga_store_recovery_is_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let keyspace = fjall::Database::builder(dir.path())
            .open()
            .expect("keyspace");
        let store = SagaStore::open(&keyspace).expect("store");

        store
            .stage_entry("key1", WriteClass::CriticalControlPlane, 100)
            .expect("stage");

        let outcome1 = store.recover().expect("recover");
        assert_eq!(outcome1, RecoveryOutcome::RolledBack { count: 1 });

        let outcome2 = store.recover().expect("recover");
        assert_eq!(outcome2, RecoveryOutcome::NothingToRecover);
    }

    #[test]
    fn durable_saga_fjall_stage_and_commit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = fjall::Database::builder(dir.path())
            .open()
            .expect("keyspace");
        let queues = create_test_queues();
        let saga = DurableBudgetSaga::open(&db, queues).expect("saga");

        saga.stage_write("key1", WriteClass::CriticalControlPlane, 100)
            .expect("stage");
        saga.commit("key1").expect("commit");

        let entry = saga
            .store()
            .as_ref()
            .expect("store")
            .read_entry("key1")
            .expect("read")
            .expect("exists");
        assert_eq!(entry.status, SagaStatus::Committed);
    }
}
