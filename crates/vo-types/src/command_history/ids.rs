//! ID types for command history entries and snapshots.

use serde::{Deserialize, Serialize};
use std::fmt;
use ulid::Ulid;

use super::CommandHistoryError;

// ---------------------------------------------------------------------------
// CommandId
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

// ---------------------------------------------------------------------------
// SnapshotId
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// BatchId
// ---------------------------------------------------------------------------

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
