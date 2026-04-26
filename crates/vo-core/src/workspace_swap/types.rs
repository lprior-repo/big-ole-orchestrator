//! Atomic workspace swap types: phases, statuses, errors, and recovery outcomes.

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SwapPhase {
    Initial,
    Staging,
    Staged,
    Swapping,
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

#[derive(Debug, PartialEq, Eq)]
pub enum SwapStatus {
    NoPriorSwap,
    Incomplete(SwapPhase),
    Complete,
}

#[derive(Debug, thiserror::Error)]
pub enum SwapError {
    #[error("workspace path is not a directory: {0}")]
    NotADirectory(std::path::PathBuf),

    #[error("workspace does not exist: {0}")]
    WorkspaceNotFound(std::path::PathBuf),

    #[error("shadow directory already exists: {0}")]
    ShadowExists(std::path::PathBuf),

    #[error("failed to create shadow directory: {path}: {source}")]
    ShadowCreate { path: std::path::PathBuf, source: std::io::Error },

    #[error("failed to copy file to shadow: {source}: {from} -> {to}")]
    CopyFailed {
        from: std::path::PathBuf,
        to: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("failed to sync directory: {path}: {source}")]
    SyncFailed { path: std::path::PathBuf, source: std::io::Error },

    #[error("failed to write journal: {path}: {source}")]
    JournalWrite { path: std::path::PathBuf, source: std::io::Error },

    #[error("failed to read journal: {path}: {source}")]
    JournalRead { path: std::path::PathBuf, source: std::io::Error },

    #[error("failed to atomic rename: {from} -> {to}: {source}")]
    RenameFailed {
        from: std::path::PathBuf,
        to: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("failed to remove directory: {path}: {source}")]
    RemoveFailed { path: std::path::PathBuf, source: std::io::Error },

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
