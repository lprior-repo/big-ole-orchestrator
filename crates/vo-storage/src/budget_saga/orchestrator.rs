//! Saga orchestrator and staged write types.
//!
//! This module provides [`DurableBudgetSaga`] which coordinates the
//! two-phase commit protocol and [`StagedWrite`] representing pending
//! budget reservations.

use std::sync::{Arc, Mutex};

use crate::append::{BudgetQueues, ClassifiedWrite, WriteClass};

use super::allocation::{BudgetManifest, SagaError};

/// A write reservation staged for saga commit.
#[derive(Debug, Clone)]
pub struct StagedWrite {
    pub write_key: String,
    pub class: WriteClass,
    pub size_bytes: u64,
    pub staged_at: std::time::SystemTime,
}

impl StagedWrite {
    /// Create a new staged write reservation.
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

/// Orchestrates the two-phase commit protocol for budget reservations.
///
/// Supports both ephemeral (in-memory only) and durable (fjall-backed) modes.
pub struct DurableBudgetSaga {
    store: Option<super::store::SagaStore>,
    manifest: Arc<Mutex<BudgetManifest>>,
    queues: BudgetQueues<StagedWrite>,
}

impl DurableBudgetSaga {
    /// Create a new ephemeral (in-memory only) saga.
    pub fn new(queues: BudgetQueues<StagedWrite>) -> Self {
        Self {
            store: None,
            manifest: Arc::new(Mutex::new(BudgetManifest::default())),
            queues,
        }
    }

    /// Open a durable saga backed by fjall.
    pub fn open(
        db: &fjall::Database,
        queues: BudgetQueues<StagedWrite>,
    ) -> Result<Self, SagaError> {
        let store = super::store::SagaStore::open(db)?;
        Ok(Self {
            store: Some(store),
            manifest: Arc::new(Mutex::new(BudgetManifest::default())),
            queues,
        })
    }

    /// Return the optional saga store reference.
    #[must_use]
    pub const fn store(&self) -> &Option<super::store::SagaStore> {
        &self.store
    }

    /// Create a saga with a pre-built manifest.
    pub fn with_manifest(queues: BudgetQueues<StagedWrite>, manifest: BudgetManifest) -> Self {
        Self {
            store: None,
            manifest: Arc::new(Mutex::new(manifest)),
            queues,
        }
    }

    /// Stage a budget write reservation.
    ///
    /// Writes the entry to the durable store (if available) and reserves
    /// budget via the queue. On budget failure, compensates by rolling
    /// back the store entry.
    pub fn stage_write(
        &self,
        write_key: &str,
        class: WriteClass,
        size_bytes: u64,
    ) -> Result<StagedWrite, SagaError> {
        let staged = StagedWrite::new(write_key.to_string(), class, size_bytes);

        if let Some(ref store) = self.store {
            store.stage_entry(write_key, class, size_bytes)?;
        } else {
            #[expect(clippy::unwrap_used)]
            let mut manifest = self.manifest.lock().unwrap();
            manifest.stage(write_key.to_string(), class, size_bytes)?;
        }

        self.queues.try_enqueue(&staged).map_err(|e| {
            if let Some(ref store) = self.store {
                let _ = store.rollback_entry(write_key);
            } else {
                #[expect(clippy::unwrap_used)]
                let _ = self.manifest.lock().unwrap().rollback(write_key);
            }
            SagaError::BudgetReserveFailed(e.to_string())
        })?;

        Ok(staged)
    }

    /// Commit a staged write by transitioning its saga entry.
    pub fn commit(&self, write_key: &str) -> Result<(), SagaError> {
        self.store.as_ref().map_or_else(
            || {
                #[expect(clippy::unwrap_used)]
                let mut manifest = self.manifest.lock().unwrap();
                manifest.commit(write_key)
            },
            |store| store.commit_entry(write_key),
        )
    }

    /// Roll back a staged write, releasing budget and updating the entry.
    pub fn rollback(&self, write_key: &str) -> Result<(), SagaError> {
        self.queues.dequeue(self.get_class_for_key(write_key)?);
        self.store.as_ref().map_or_else(
            || {
                #[expect(clippy::unwrap_used)]
                let mut manifest = self.manifest.lock().unwrap();
                manifest.rollback(write_key)
            },
            |store| store.rollback_entry(write_key),
        )
    }

    /// Recover from crash: mark all staged entries as rolled back.
    pub fn recover_from_crash(&self) {
        #[expect(clippy::unwrap_used)]
        let mut manifest = self.manifest.lock().unwrap();
        manifest.recover_staged_as_rolled_back();
    }

    /// Look up the write class for a saga entry key.
    fn get_class_for_key(&self, write_key: &str) -> Result<WriteClass, SagaError> {
        #[expect(clippy::unwrap_used)]
        let manifest = self.manifest.lock().unwrap();
        manifest
            .get(write_key)
            .map(|e| e.class)
            .ok_or_else(|| SagaError::NotFound(write_key.to_string()))
    }

    /// Return a clone of the manifest Arc.
    pub fn manifest(&self) -> Arc<Mutex<BudgetManifest>> {
        Arc::clone(&self.manifest)
    }

    /// Return a reference to the budget queues.
    pub const fn queues(&self) -> &BudgetQueues<StagedWrite> {
        &self.queues
    }
}

fn create_test_queues() -> BudgetQueues<StagedWrite> {
    let config = crate::append::QueueConfig::default();
    let budget = crate::append::WriteBudget::new(1000, 1000, 1000);
    BudgetQueues::new(&config, budget)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::append::WriteClass;

    use super::super::allocation::SagaStatus;

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
    fn durable_saga_fjall_stage_and_commit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = fjall::Database::builder(dir.path())
            .open()
            .expect("fjall");
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
