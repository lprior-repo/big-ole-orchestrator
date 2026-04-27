//! Read-only query methods for `CommandHistory`.

use super::ids::CommandId;
use super::types::HistoryEntry;

impl super::CommandHistory {
    /// Returns the maximum capacity of the history.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns whether there are commands available to undo.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns whether there are commands available to redo.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Returns all history entries.
    #[must_use]
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Returns a mutable reference to history entries.
    ///
    /// # Warning
    ///
    /// This bypasses internal invariants. Use with caution.
    pub fn entries_mut(&mut self) -> &mut Vec<HistoryEntry> {
        &mut self.entries
    }

    /// Returns the undo stack command IDs.
    #[must_use]
    pub fn undo_stack(&self) -> &[CommandId] {
        &self.undo_stack
    }

    /// Returns a mutable reference to the undo stack.
    ///
    /// # Warning
    ///
    /// This bypasses internal invariants. Use with caution.
    pub fn undo_stack_mut(&mut self) -> &mut Vec<CommandId> {
        &mut self.undo_stack
    }

    /// Returns the redo stack command IDs.
    #[must_use]
    pub fn redo_stack(&self) -> &[CommandId] {
        &self.redo_stack
    }

    /// Returns a mutable reference to the redo stack.
    ///
    /// # Warning
    ///
    /// This bypasses internal invariants. Use with caution.
    pub fn redo_stack_mut(&mut self) -> &mut Vec<CommandId> {
        &mut self.redo_stack
    }
}
