//! Constants, error types, ID wrappers, and command classification enums.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use ulid::Ulid;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum depth of the command history entries vector.
pub const MAX_HISTORY_DEPTH: usize = 100;

/// Maximum depth of the undo stack.
pub const MAX_UNDO_STACK_DEPTH: usize = 50;

/// Maximum depth of the redo stack.
pub const MAX_REDO_STACK_DEPTH: usize = 50;

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

/// Errors that can occur when performing undo/redo operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CommandHistoryError {
    #[error("Undo stack is empty")]
    UndoStackEmpty,

    #[error("Redo stack is empty")]
    RedoStackEmpty,

    #[error("Snapshot not found: {snapshot_id}")]
    SnapshotNotFound {
        #[allow(dead_code)]
        snapshot_id: String,
    },

    #[error("Entry not found: {command_id}")]
    EntryNotFound {
        #[allow(dead_code)]
        command_id: String,
    },

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: u32, actual: u32 },

    #[error("History capacity exceeded: {capacity}")]
    HistoryCapacityExceeded {
        #[allow(dead_code)]
        capacity: usize,
    },

    #[error("Snapshot serialization error: {reason}")]
    SnapshotSerializationError {
        #[allow(dead_code)]
        reason: String,
    },

    #[error("Invalid history transition: {current_status:?} cannot {attempted_action}")]
    InvalidHistoryTransition {
        #[allow(dead_code)]
        current_status: HistoryEntryStatus,
        attempted_action: String,
    },
}

// ---------------------------------------------------------------------------
// ID Types
// ---------------------------------------------------------------------------

/// Unique identifier for a command in the history.
/// Uses ULID for time-ordered unique identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandId(String);

impl CommandId {
    /// Generate a new unique CommandId.
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new().to_string())
    }

    /// Parse a CommandId from a string.
    ///
    /// # Errors
    ///
    /// Returns `CommandHistoryError::EntryNotFound` if the input is empty.
    pub fn parse(input: &str) -> Result<Self, CommandHistoryError> {
        if input.is_empty() {
            return Err(CommandHistoryError::EntryNotFound {
                command_id: input.to_string(),
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CommandId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for CommandId {
    type Error = CommandHistoryError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<CommandId> for String {
    fn from(id: CommandId) -> Self {
        id.0
    }
}

impl Serialize for CommandId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CommandId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Unique identifier for a snapshot.
/// Uses ULID for time-ordered unique identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SnapshotId(String);

impl SnapshotId {
    /// Generate a new unique SnapshotId.
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new().to_string())
    }

    /// Parse a SnapshotId from a string.
    ///
    /// # Errors
    ///
    /// Returns `CommandHistoryError::SnapshotNotFound` if the input is empty.
    pub fn parse(input: &str) -> Result<Self, CommandHistoryError> {
        if input.is_empty() {
            return Err(CommandHistoryError::SnapshotNotFound {
                snapshot_id: input.to_string(),
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SnapshotId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for SnapshotId {
    type Error = CommandHistoryError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<SnapshotId> for String {
    fn from(id: SnapshotId) -> Self {
        id.0
    }
}

impl Serialize for SnapshotId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SnapshotId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Unique identifier for a batch of extensions.
/// Uses ULID for time-ordered unique identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BatchId(String);

impl std::fmt::Display for BatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl BatchId {
    /// Generate a new unique BatchId.
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for BatchId {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Command Kind
// ---------------------------------------------------------------------------

/// Classification of graph-modifying operations.
///
/// # Variants
///
/// - `ExtensionApply` - Bulk or individual extension application
/// - `ExtensionRevert` - Undo of a prior extension apply
/// - `ExtensionRedo` - Redo of a previously undone extension
/// - `NodeCreate` - Direct node creation via UI
/// - `NodeDelete` - Direct node deletion via UI
/// - `EdgeCreate` - Edge creation via UI
/// - `EdgeDelete` - Edge deletion via UI
/// - `ConfigUpdate` - Node or edge configuration change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    ExtensionApply,
    ExtensionRevert,
    ExtensionRedo,
    NodeCreate,
    NodeDelete,
    EdgeCreate,
    EdgeDelete,
    ConfigUpdate,
}

// ---------------------------------------------------------------------------
// Extension Apply Mode
// ---------------------------------------------------------------------------

/// How extensions were applied in a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionApplyMode {
    Single,
    Bulk,
}

// ---------------------------------------------------------------------------
// History Entry Status
// ---------------------------------------------------------------------------

/// Outcome of a command in the history.
///
/// # Variants
///
/// - `Committed` - Command succeeded, entry is final
/// - `Undone` - Command was reverted via undo
/// - `Redone` - Command was restored via redo
/// - `Failed` - Command failed during execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryEntryStatus {
    Committed,
    Undone,
    Redone,
    Failed,
}

impl std::fmt::Display for HistoryEntryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HistoryEntryStatus::Committed => write!(f, "Committed"),
            HistoryEntryStatus::Undone => write!(f, "Undone"),
            HistoryEntryStatus::Redone => write!(f, "Redone"),
            HistoryEntryStatus::Failed => write!(f, "Failed"),
        }
    }
}
