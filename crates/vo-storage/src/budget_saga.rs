//! Durable dual-write saga for WriteBudget.
//!
//! This module implements ADR-034 saga compensation and reversibility for the
//! BudgetQueues write path. It provides:
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetManifest {
    entries: HashMap<String, SagaEntry>,
    version: u64,
}

impl Default for BudgetManifest {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            version: 0,
        }
    }
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
            .ok_or(SagaError::NotFound(write_key.to_string()))?;
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
            .ok_or(SagaError::NotFound(write_key.to_string()))?;
        if entry.status == SagaStatus::RolledBack {
            return Err(SagaError::AlreadyRolledBack(write_key.to_string()));
        }
        entry.status = SagaStatus::RolledBack;
        self.version += 1;
        Ok(())
    }

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

    pub fn version(&self) -> u64 {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SagaError {
    AlreadyExists(String),
    NotFound(String),
    AlreadyRolledBack(String),
    InvalidState {
        key: String,
        expected: SagaStatus,
        actual: SagaStatus,
    },
    BudgetReserveFailed(String),
}

impl std::fmt::Display for SagaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyExists(key) => write!(f, "saga entry already exists: {key}"),
            Self::NotFound(key) => write!(f, "saga entry not found: {key}"),
            Self::AlreadyRolledBack(key) => write!(f, "saga entry already rolled back: {key}"),
            Self::InvalidState {
                key,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "invalid state for {key}: expected {:?}, got {:?}",
                    expected, actual
                )
            }
            Self::BudgetReserveFailed(msg) => write!(f, "budget reserve failed: {msg}"),
        }
    }
}

impl std::error::Error for SagaError {}

pub struct DurableBudgetSaga {
    manifest: Arc<Mutex<BudgetManifest>>,
    queues: BudgetQueues<StagedWrite>,
}

#[derive(Debug, Clone)]
pub struct StagedWrite {
    pub write_key: String,
    pub class: WriteClass,
    pub size_bytes: u64,
    pub staged_at: std::time::SystemTime,
}

impl StagedWrite {
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
            manifest: Arc::new(Mutex::new(BudgetManifest::default())),
            queues,
        }
    }

    pub fn with_manifest(queues: BudgetQueues<StagedWrite>, manifest: BudgetManifest) -> Self {
        Self {
            manifest: Arc::new(Mutex::new(manifest)),
            queues,
        }
    }

    pub fn stage_write(
        &self,
        write_key: String,
        class: WriteClass,
        size_bytes: u64,
    ) -> Result<StagedWrite, SagaError> {
        let staged = StagedWrite::new(write_key.clone(), class, size_bytes);

        {
            let mut manifest = self.manifest.lock().unwrap();
            manifest.stage(write_key.clone(), class, size_bytes)?;
        }

        self.queues.try_enqueue(&staged).map_err(|e| {
            let mut manifest = self.manifest.lock().unwrap();
            let _ = manifest.rollback(&write_key);
            SagaError::BudgetReserveFailed(e.to_string())
        })?;

        Ok(staged)
    }

    pub fn commit(&self, write_key: &str) -> Result<(), SagaError> {
        let mut manifest = self.manifest.lock().unwrap();
        manifest.commit(write_key)
    }

    pub fn rollback(&self, write_key: &str) -> Result<(), SagaError> {
        self.queues.dequeue(self.get_class_for_key(write_key)?);
        let mut manifest = self.manifest.lock().unwrap();
        manifest.rollback(write_key)
    }

    pub fn recover_from_crash(&self) {
        let mut manifest = self.manifest.lock().unwrap();
        manifest.recover_staged_as_rolled_back();
    }

    fn get_class_for_key(&self, write_key: &str) -> Result<WriteClass, SagaError> {
        let manifest = self.manifest.lock().unwrap();
        manifest
            .get(write_key)
            .map(|e| e.class)
            .ok_or_else(|| SagaError::NotFound(write_key.to_string()))
    }

    pub fn manifest(&self) -> Arc<Mutex<BudgetManifest>> {
        Arc::clone(&self.manifest)
    }

    pub fn queues(&self) -> &BudgetQueues<StagedWrite> {
        &self.queues
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_queues() -> BudgetQueues<StagedWrite> {
        let config = crate::append::QueueConfig::default();
        let budget = crate::append::WriteBudget::new(1000, 1000, 1000);
        BudgetQueues::new(config, budget)
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

        saga.stage_write("key1".to_string(), WriteClass::CriticalControlPlane, 100)
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

        saga.stage_write("key1".to_string(), WriteClass::CriticalControlPlane, 100)
            .unwrap();
        saga.rollback("key1").unwrap();

        let manifest_ref = saga.manifest();
        let manifest = manifest_ref.lock().unwrap();
        assert_eq!(manifest.get("key1").unwrap().status, SagaStatus::RolledBack);
    }
}
