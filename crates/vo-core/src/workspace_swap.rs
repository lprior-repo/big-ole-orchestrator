//! Atomic workspace swap for branch switches.
//!
//! Stages new state in a shadow directory, fsyncs to ensure durability,
//! then performs an atomic rename to swap the workspace. A journal file
//! tracks the swap state so that crash recovery can always reach a
//! consistent state regardless of when the process dies.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
    fn from_str_lossy(s: &str) -> Option<Self> {
        match s.trim() {
            "staging" => Some(Self::Staging),
            "staged" => Some(Self::Staged),
            "swapping" => Some(Self::Swapping),
            "complete" => Some(Self::Complete),
            _ => None,
        }
    }

    fn as_str(&self) -> &'static str {
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

pub struct AtomicSwap {
    workspace: PathBuf,
    shadow_suffix: String,
    journal_suffix: String,
}

impl AtomicSwap {
    pub fn new<P: AsRef<Path>>(workspace: P) -> Self {
        Self {
            workspace: workspace.as_ref().to_path_buf(),
            shadow_suffix: ".shadow".to_string(),
            journal_suffix: ".swap-journal".to_string(),
        }
    }

    pub fn with_shadow_suffix<P: AsRef<Path>>(workspace: P, suffix: &str) -> Self {
        Self {
            workspace: workspace.as_ref().to_path_buf(),
            shadow_suffix: suffix.to_string(),
            journal_suffix: ".swap-journal".to_string(),
        }
    }

    fn shadow_path(&self) -> PathBuf {
        let mut p = self.workspace.clone();
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        p.set_file_name(format!("{name}{}", self.shadow_suffix));
        p
    }

    fn journal_path(&self) -> PathBuf {
        let mut p = self.workspace.clone();
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        p.set_file_name(format!("{name}{}", self.journal_suffix));
        p
    }

    fn backup_path(&self) -> PathBuf {
        let mut p = self.workspace.clone();
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        p.set_file_name(format!("{name}.backup"));
        p
    }

    fn validate_workspace(&self) -> Result<(), SwapError> {
        if !self.workspace.exists() {
            return Err(SwapError::WorkspaceNotFound(self.workspace.clone()));
        }
        if !self.workspace.is_dir() {
            return Err(SwapError::NotADirectory(self.workspace.clone()));
        }
        Ok(())
    }

    pub fn check_status(&self) -> Result<SwapStatus, SwapError> {
        let journal = self.journal_path();
        if !journal.exists() {
            return Ok(SwapStatus::NoPriorSwap);
        }

        let content = fs::read_to_string(&journal).map_err(|e| SwapError::JournalRead {
            path: journal.clone(),
            source: e,
        })?;

        let phase = SwapPhase::from_str_lossy(&content)
            .ok_or_else(|| SwapError::InvalidJournal(content.clone()))?;

        match phase {
            SwapPhase::Complete => Ok(SwapStatus::Complete),
            other => Ok(SwapStatus::Incomplete(other)),
        }
    }

    pub fn stage(&self) -> Result<SwapPhase, SwapError> {
        self.validate_workspace()?;

        let shadow = self.shadow_path();
        if shadow.exists() {
            return Err(SwapError::ShadowExists(shadow));
        }

        self.write_journal(SwapPhase::Staging)?;

        fs::create_dir_all(&shadow).map_err(|e| SwapError::ShadowCreate {
            path: shadow.clone(),
            source: e,
        })?;

        copy_dir_recursive(&self.workspace, &shadow)?;

        sync_dir(&shadow)?;
        sync_dir(&self.workspace)?;

        self.write_journal(SwapPhase::Staged)?;

        Ok(SwapPhase::Staged)
    }

    pub fn commit(&self) -> Result<(), SwapError> {
        let status = self.check_status()?;
        match status {
            SwapStatus::NoPriorSwap | SwapStatus::Complete => return Ok(()),
            SwapStatus::Incomplete(_) => {}
        }

        self.write_journal(SwapPhase::Swapping)?;

        let shadow = self.shadow_path();
        let backup = self.backup_path();

        if backup.exists() {
            fs::remove_dir_all(&backup).map_err(|e| SwapError::RemoveFailed {
                path: backup.clone(),
                source: e,
            })?;
        }

        fs::rename(&self.workspace, &backup).map_err(|e| SwapError::RenameFailed {
            from: self.workspace.clone(),
            to: backup.clone(),
            source: e,
        })?;

        sync_dir(self.workspace.parent().unwrap_or(Path::new(".")))?;

        fs::rename(&shadow, &self.workspace).map_err(|e| SwapError::RenameFailed {
            from: shadow.clone(),
            to: self.workspace.clone(),
            source: e,
        })?;

        sync_dir(self.workspace.parent().unwrap_or(Path::new(".")))?;

        self.write_journal(SwapPhase::Complete)?;

        if backup.exists() {
            fs::remove_dir_all(&backup).map_err(|e| SwapError::RemoveFailed {
                path: backup,
                source: e,
            })?;
        }

        let journal = self.journal_path();
        if journal.exists() {
            fs::remove_file(&journal).map_err(|e| SwapError::RemoveFailed {
                path: journal,
                source: e,
            })?;
        }

        Ok(())
    }

    pub fn recover(&self) -> Result<RecoveryOutcome, SwapError> {
        let status = self.check_status()?;

        match status {
            SwapStatus::NoPriorSwap => Ok(RecoveryOutcome::NothingToRecover),
            SwapStatus::Complete => {
                self.cleanup_journal()?;
                Ok(RecoveryOutcome::AlreadyComplete)
            }
            SwapStatus::Incomplete(phase) => {
                let shadow = self.shadow_path();
                let backup = self.backup_path();

                match phase {
                    SwapPhase::Staging | SwapPhase::Staged => {
                        if shadow.exists() {
                            fs::remove_dir_all(&shadow).map_err(|e| SwapError::RemoveFailed {
                                path: shadow.clone(),
                                source: e,
                            })?;
                        }
                        self.cleanup_journal()?;
                        Ok(RecoveryOutcome::RolledBack)
                    }
                    SwapPhase::Swapping => {
                        if backup.exists() && !self.workspace.exists() {
                            fs::rename(&backup, &self.workspace).map_err(|e| {
                                SwapError::RenameFailed {
                                    from: backup.clone(),
                                    to: self.workspace.clone(),
                                    source: e,
                                }
                            })?;
                            sync_dir(self.workspace.parent().unwrap_or(Path::new(".")))?;
                        } else if backup.exists() && self.workspace.exists() {
                            fs::remove_dir_all(&backup).map_err(|e| SwapError::RemoveFailed {
                                path: backup.clone(),
                                source: e,
                            })?;
                        }

                        if shadow.exists() {
                            fs::remove_dir_all(&shadow).map_err(|e| SwapError::RemoveFailed {
                                path: shadow.clone(),
                                source: e,
                            })?;
                        }

                        self.cleanup_journal()?;
                        Ok(RecoveryOutcome::RolledBack)
                    }
                    _ => Ok(RecoveryOutcome::NothingToRecover),
                }
            }
        }
    }

    fn write_journal(&self, phase: SwapPhase) -> Result<(), SwapError> {
        let journal = self.journal_path();
        let content = phase.as_str();

        fs::write(&journal, content).map_err(|e| SwapError::JournalWrite {
            path: journal.clone(),
            source: e,
        })?;

        sync_dir(journal.parent().unwrap_or(Path::new(".")))?;

        Ok(())
    }

    fn cleanup_journal(&self) -> Result<(), SwapError> {
        let journal = self.journal_path();
        if journal.exists() {
            fs::remove_file(&journal).map_err(|e| SwapError::RemoveFailed {
                path: journal,
                source: e,
            })?;
        }
        Ok(())
    }

    #[must_use]
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    #[must_use]
    pub fn shadow_dir(&self) -> PathBuf {
        self.shadow_path()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RecoveryOutcome {
    NothingToRecover,
    AlreadyComplete,
    RolledBack,
}

fn sync_dir(path: &Path) -> Result<(), SwapError> {
    std::fs::File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| SwapError::SyncFailed {
            path: path.to_path_buf(),
            source: e,
        })?;
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), SwapError> {
    fs::create_dir_all(dst).map_err(|e| SwapError::ShadowCreate {
        path: dst.to_path_buf(),
        source: e,
    })?;

    for entry in fs::read_dir(src).map_err(|e| SwapError::CopyFailed {
        from: src.to_path_buf(),
        to: dst.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| SwapError::CopyFailed {
            from: src.to_path_buf(),
            to: dst.to_path_buf(),
            source: e,
        })?;

        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let file_type = entry.file_type().map_err(|e| SwapError::CopyFailed {
            from: src_path.clone(),
            to: dst_path.clone(),
            source: e,
        })?;

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path).map_err(|e| SwapError::CopyFailed {
                from: src_path.clone(),
                to: dst_path.clone(),
                source: e,
            })?;
        }
    }

    Ok(())
}

pub fn atomic_swap<P: AsRef<Path>>(workspace: P) -> Result<(), SwapError> {
    let swap = AtomicSwap::new(workspace);

    if let SwapStatus::Incomplete(phase) = swap.check_status()? {
        return Err(SwapError::RecoveryNeeded(phase));
    }

    swap.stage()?;
    swap.commit()?;

    Ok(())
}

pub fn recover_swap<P: AsRef<Path>>(workspace: P) -> Result<RecoveryOutcome, SwapError> {
    let swap = AtomicSwap::new(workspace);
    swap.recover()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn atomic_swap_creates_shadow_then_commits() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "hello").unwrap();

        let swap = AtomicSwap::new(&workspace);

        let phase = swap.stage().unwrap();
        assert_eq!(phase, SwapPhase::Staged);

        let shadow = swap.shadow_path();
        assert!(shadow.exists());
        assert!(shadow.join("file.txt").exists());
        assert_eq!(
            fs::read_to_string(shadow.join("file.txt")).unwrap(),
            "hello"
        );

        assert_eq!(
            swap.check_status().unwrap(),
            SwapStatus::Incomplete(SwapPhase::Staged)
        );

        swap.commit().unwrap();

        assert!(!shadow.exists());
        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "hello"
        );
        assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    }

    #[test]
    fn atomic_swap_preserves_nested_directories() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        let nested = workspace.join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("deep.txt"), "deep content").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();
        swap.commit().unwrap();

        assert_eq!(
            fs::read_to_string(workspace.join("a/b/c/deep.txt")).unwrap(),
            "deep content"
        );
    }

    #[test]
    fn stage_fails_if_shadow_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let swap2 = AtomicSwap::new(&workspace);
        assert!(matches!(swap2.stage(), Err(SwapError::ShadowExists(_))));
    }

    #[test]
    fn commit_is_idempotent_when_no_prior_swap() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        assert!(swap.commit().is_ok());
    }

    #[test]
    fn stage_fails_if_workspace_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nonexistent");

        let swap = AtomicSwap::new(&missing);
        assert!(matches!(swap.stage(), Err(SwapError::WorkspaceNotFound(_))));
    }

    #[test]
    fn stage_fails_if_path_is_file_not_directory() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, "data").unwrap();

        let swap = AtomicSwap::new(&file);
        assert!(matches!(swap.stage(), Err(SwapError::NotADirectory(_))));
    }

    #[test]
    fn commit_idempotent_when_already_complete() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "data").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();
        swap.commit().unwrap();

        assert!(swap.commit().is_ok());
    }

    #[test]
    fn check_status_reports_no_prior_swap_initially() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    }

    #[test]
    fn check_status_reports_incomplete_after_stage() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        assert_eq!(
            swap.check_status().unwrap(),
            SwapStatus::Incomplete(SwapPhase::Staged)
        );
    }

    #[test]
    fn check_status_reports_no_prior_after_commit() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();
        swap.commit().unwrap();

        assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    }

    #[test]
    fn recover_rolls_back_from_staging_phase() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "original").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let journal = swap.journal_path();
        fs::write(&journal, "staging").unwrap();

        let outcome = swap.recover().unwrap();
        assert_eq!(outcome, RecoveryOutcome::RolledBack);

        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "original"
        );
        assert!(!swap.shadow_path().exists());
        assert!(!journal.exists());
    }

    #[test]
    fn recover_rolls_back_from_swapping_phase() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "original").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let journal = swap.journal_path();
        fs::write(&journal, "swapping").unwrap();

        let outcome = swap.recover().unwrap();
        assert_eq!(outcome, RecoveryOutcome::RolledBack);

        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "original"
        );
    }

    #[test]
    fn recover_restores_backup_when_workspace_missing_during_swapping() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "original").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let backup = swap.backup_path();
        let shadow = swap.shadow_path();
        let journal = swap.journal_path();

        fs::rename(&workspace, &backup).unwrap();
        fs::write(&journal, "swapping").unwrap();

        let outcome = swap.recover().unwrap();
        assert_eq!(outcome, RecoveryOutcome::RolledBack);

        assert!(workspace.exists());
        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "original"
        );
        assert!(!shadow.exists());
        assert!(!backup.exists());
    }

    #[test]
    fn recover_returns_nothing_when_no_journal() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        assert_eq!(swap.recover().unwrap(), RecoveryOutcome::NothingToRecover);
    }

    #[test]
    fn atomic_swap_convenience_function_works() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("test.txt"), "content").unwrap();

        assert!(atomic_swap(&workspace).is_ok());

        assert_eq!(
            fs::read_to_string(workspace.join("test.txt")).unwrap(),
            "content"
        );
    }

    #[test]
    fn atomic_swap_returns_recovery_needed_on_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let result = atomic_swap(&workspace);
        assert!(matches!(result, Err(SwapError::RecoveryNeeded(_))));
    }

    #[test]
    fn recover_swap_convenience_function_works() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let outcome = recover_swap(&workspace).unwrap();
        assert_eq!(outcome, RecoveryOutcome::NothingToRecover);
    }

    #[test]
    fn swap_phase_roundtrip() {
        assert_eq!(
            SwapPhase::from_str_lossy("staging"),
            Some(SwapPhase::Staging)
        );
        assert_eq!(SwapPhase::from_str_lossy("staged"), Some(SwapPhase::Staged));
        assert_eq!(
            SwapPhase::from_str_lossy("swapping"),
            Some(SwapPhase::Swapping)
        );
        assert_eq!(
            SwapPhase::from_str_lossy("complete"),
            Some(SwapPhase::Complete)
        );
        assert_eq!(SwapPhase::from_str_lossy("garbage"), None);
        assert_eq!(SwapPhase::from_str_lossy(""), None);
    }

    #[test]
    fn with_shadow_suffix_uses_custom_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::with_shadow_suffix(&workspace, ".custom-shadow");
        swap.stage().unwrap();

        assert!(dir.path().join("ws.custom-shadow").exists());
        assert!(!dir.path().join("ws.shadow").exists());
    }

    #[test]
    fn workspace_accessor_returns_original_path() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");

        let swap = AtomicSwap::new(&workspace);
        assert_eq!(swap.workspace(), workspace);
    }
}
