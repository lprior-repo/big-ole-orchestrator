use crate::append::WriteClass;

use super::{DurableBudgetSaga, SagaError, StagedWrite};

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
    ) -> Result<StagedWrite, SagaError> {
        let staged = StagedWrite::new(write_key.to_string(), class, size_bytes);

        if let Some(ref store) = self.store {
            store.stage_entry(write_key, class, size_bytes)?;
        } else {
            #[expect(clippy::unwrap_used)]
            let mut manifest = self.manifest.lock().unwrap();
            manifest.stage(write_key.to_string(), class, size_bytes)?;
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
            SagaError::BudgetReserveFailed(e.to_string())
        })?;

        Ok(staged)
    }
}
