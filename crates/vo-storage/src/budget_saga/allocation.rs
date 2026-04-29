use crate::append::WriteClass;

use super::{DurableBudgetSaga, SagaError, StagedWrite};

impl DurableBudgetSaga {
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
}
