use crate::append::WriteClass;

use super::{DurableBudgetSaga, SagaError};

impl DurableBudgetSaga {
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
        self.store.as_ref().map_or_else(
            || {
                #[expect(clippy::unwrap_used)]
                let mut manifest = self.manifest.lock().unwrap();
                manifest.commit(write_key)
            },
            |store| store.commit_entry(write_key),
        )
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
        self.store.as_ref().map_or_else(
            || {
                #[expect(clippy::unwrap_used)]
                let mut manifest = self.manifest.lock().unwrap();
                manifest.rollback(write_key)
            },
            |store| store.rollback_entry(write_key),
        )
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

    pub(crate) fn get_class_for_key(&self, write_key: &str) -> Result<WriteClass, SagaError> {
        #[expect(clippy::unwrap_used)]
        let manifest = self.manifest.lock().unwrap();
        manifest
            .get(write_key)
            .map(|e| e.class)
            .ok_or_else(|| SagaError::NotFound(write_key.to_string()))
    }
}
