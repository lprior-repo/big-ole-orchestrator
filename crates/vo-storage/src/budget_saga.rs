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
    /// Stage a new write entry in the manifest.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::AlreadyExists`] if a write entry with the same key already exists.
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

    /// Commit a previously staged write entry.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::NotFound`] if no entry exists for the given key.
    /// Returns [`SagaError::InvalidState`] if the entry is not in the `Staged` state.
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

    /// Roll back a write entry.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::NotFound`] if no entry exists for the given key.
    /// Returns [`SagaError::AlreadyRolledBack`] if the entry is already rolled back.
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
                    "invalid state for {key}: expected {expected:?}, got {actual:?}"
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

    /// Stage a write in the saga: create a manifest entry and enqueue it in the budget queues.
    /// If enqueue fails, the manifest entry is rolled back automatically.
    ///
    /// # Panics
    ///
    /// Panics if the internal manifest mutex is poisoned.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::AlreadyExists`] if a write entry with the same key already exists.
    /// Returns [`SagaError::BudgetReserveFailed`] if the budget queue enqueue fails.
    pub fn stage_write(
        &self,
        write_key: &str,
        class: WriteClass,
        size_bytes: u64,
    ) -> Result<StagedWrite, SagaError> {
        let staged = StagedWrite::new(write_key.to_string(), class, size_bytes);

        {
            #[expect(clippy::unwrap_used)]
            let mut manifest = self.manifest.lock().unwrap();
            manifest.stage(write_key.to_string(), class, size_bytes)?;
        }

        self.queues.try_enqueue(&staged).map_err(|e| {
            #[expect(clippy::unwrap_used)]
            let _ = self.manifest.lock().unwrap().rollback(write_key);
            SagaError::BudgetReserveFailed(e.to_string())
        })?;

        Ok(staged)
    }

    /// Commit a previously staged write entry.
    ///
    /// # Panics
    ///
    /// Panics if the internal manifest mutex is poisoned.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::NotFound`] if no entry exists for the given key.
    /// Returns [`SagaError::InvalidState`] if the entry is not in the `Staged` state.
    pub fn commit(&self, write_key: &str) -> Result<(), SagaError> {
        #[expect(clippy::unwrap_used)]
        let mut manifest = self.manifest.lock().unwrap();
        manifest.commit(write_key)
    }

    /// Roll back a write entry and dequeue it from the budget queues.
    ///
    /// # Panics
    ///
    /// Panics if the internal manifest mutex is poisoned.
    ///
    /// # Errors
    ///
    /// Returns [`SagaError::NotFound`] if no entry exists for the given key.
    /// Returns [`SagaError::AlreadyRolledBack`] if the entry is already rolled back.
    pub fn rollback(&self, write_key: &str) -> Result<(), SagaError> {
        self.queues.dequeue(self.get_class_for_key(write_key)?);
        #[expect(clippy::unwrap_used)]
        let mut manifest = self.manifest.lock().unwrap();
        manifest.rollback(write_key)
    }

    /// Recover from a crash by rolling back all staged entries.
    ///
    /// # Panics
    ///
    /// Panics if the internal manifest mutex is poisoned.
    pub fn recover_from_crash(&self) {
        #[expect(clippy::unwrap_used)]
        let mut manifest = self.manifest.lock().unwrap();
        manifest.recover_staged_as_rolled_back();
    }

    fn get_class_for_key(&self, write_key: &str) -> Result<WriteClass, SagaError> {
        #[expect(clippy::unwrap_used)]
        let manifest = self.manifest.lock().unwrap();
        manifest
            .get(write_key)
            .map(|e| e.class)
            .ok_or_else(|| SagaError::NotFound(write_key.to_string()))
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
