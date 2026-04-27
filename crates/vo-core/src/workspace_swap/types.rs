//! Atomic workspace swap for branch switches.
//!
//! Stages new state in a shadow directory, fsyncs to ensure durability,
//! then performs an atomic rename to swap the workspace. A journal file
//! tracks the swap state so that crash recovery can always reach a
//! consistent state regardless of when the process dies.

use std::io;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SwapPhase {
    Initial,
    Staging,
    Staged,
    Swapping,
    Complete,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SwapStatus {
    NoPriorSwap,
    Incomplete(SwapPhase),
    Complete,
}

impl SwapPhase {
    pub(crate) fn from_str_lossy(s: &str) -> Option<Self> {
        match s.trim() {
            "staging" => Some(Self::Staging),
            "staged" => Some(Self::Staged),
            "swapping" => Some(Self::Swapping),
            "complete" => Some(Self::Complete),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Staging => "staging",
            Self::Staged => "staged",
            Self::Swapping => "swapping",
            Self::Complete => "complete",
            Self::Initial => "initial",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SwapError {
    #[error("workspace path is not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("workspace does not exist: {0}")]
    WorkspaceNotFound(PathBuf),

    #[error("shadow directory already exists: {0}")]
    ShadowExists(PathBuf),

    #[error("failed to create shadow directory: {path}: {source}")]
    ShadowCreate { path: PathBuf, source: io::Error },

    #[error("failed to copy file to shadow: {source}: {from} -> {to}")]
    CopyFailed {
        from: PathBuf,
        to: PathBuf,
        source: io::Error,
    },

    #[error("failed to sync directory: {path}: {source}")]
    SyncFailed { path: PathBuf, source: io::Error },

    #[error("failed to write journal: {path}: {source}")]
    JournalWrite { path: PathBuf, source: io::Error },

    #[error("failed to read journal: {path}: {source}")]
    JournalRead { path: PathBuf, source: io::Error },

    #[error("failed to atomic rename: {from} -> {to}: {source}")]
    RenameFailed {
        from: PathBuf,
        to: PathBuf,
        source: io::Error,
    },

    #[error("failed to remove directory: {path}: {source}")]
    RemoveFailed { path: PathBuf, source: io::Error },

    #[error("swap not staged; call stage() first")]
    NotStaged,

    #[error("invalid journal content: {0}")]
    InvalidJournal(String),

    #[error("recovery needed: swap incomplete at phase {0:?}")]
    RecoveryNeeded(SwapPhase),
}

#[derive(Debug, PartialEq, Eq)]
pub enum RecoveryOutcome {
    NothingToRecover,
    AlreadyComplete,
    RolledBack,
}
