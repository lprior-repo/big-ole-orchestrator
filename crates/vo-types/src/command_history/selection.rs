//! Entry selection and filtering logic for `CommandHistory`.

use super::ids::CommandId;
use super::types::{CommandHistoryError, HistoryEntry, HistoryEntryStatus};

impl super::CommandHistory {
    /// Find a mutable reference to an entry by its command ID.
    pub(crate) fn find_entry_mut(
        &mut self,
        command_id: &CommandId,
    ) -> Result<&mut HistoryEntry, CommandHistoryError> {
        self.entries
            .iter_mut()
            .find(|e| e.envelope.metadata.command_id.as_str() == command_id.as_str())
            .ok_or_else(|| CommandHistoryError::EntryNotFound {
                command_id: command_id.as_str().to_string(),
            })
    }

    /// Find the index of the oldest committed entry (for capacity eviction).
    pub(crate) fn find_oldest_committed_index(&self) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.status == HistoryEntryStatus::Committed)
    }
}
