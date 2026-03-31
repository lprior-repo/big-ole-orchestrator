use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use wtf_types::{BinaryHash, WorkflowDefinition, WorkflowName};

use crate::error::BinaryRegistryError;

/// Absolute path to an executable binary on disk.
/// Invariant: always absolute (starts with `/`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BinaryPath(pub PathBuf);

impl BinaryPath {
    /// Construct from a PathBuf. Returns error if not absolute.
    pub fn new(path: PathBuf) -> Result<Self, BinaryRegistryError> {
        if path.is_absolute() {
            Ok(BinaryPath(path))
        } else {
            Err(BinaryRegistryError::NonAbsolutePath {
                path: path.to_string_lossy().to_string(),
            })
        }
    }

    /// Access the underlying PathBuf.
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Returns the parent directory as a &Path.
    pub fn parent(&self) -> &Path {
        self.0.parent().unwrap_or(Path::new("/"))
    }
}

impl std::fmt::Display for BinaryPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.display().fmt(f)
    }
}

impl From<BinaryPath> for PathBuf {
    fn from(value: BinaryPath) -> PathBuf {
        value.0
    }
}

/// Two-state lifecycle for workflow registrations.
/// `Deleted` is represented by absence from the map (per ADR-021).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegistrationStatus {
    Active,
    Deactivated,
}

/// Full registration record stored in the BinaryRegistry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRegistration {
    pub workflow_name: WorkflowName,
    pub versioned_path: BinaryPath,
    pub binary_hash: BinaryHash,
    pub status: RegistrationStatus,
    pub definition: WorkflowDefinition,
}

/// Result of a single Reaper GC sweep.
#[derive(Debug, Default)]
pub struct ReaperReport {
    /// Workflows successfully reaped (binary deleted, registration removed).
    pub reaped: Vec<WorkflowName>,
    /// Workflows skipped (still have active instances).
    pub skipped: Vec<WorkflowName>,
    /// Failures during binary deletion (path + error).
    pub failures: Vec<(WorkflowName, BinaryRegistryError)>,
}
