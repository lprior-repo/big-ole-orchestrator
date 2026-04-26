use std::io::Error;
use std::path::{Path, PathBuf};

use super::types::*;

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

        let content =
            std::fs::read_to_string(&journal)
                .map_err(|e| SwapError::JournalRead {
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

        std::fs::create_dir_all(&shadow).map_err(|e| SwapError::ShadowCreate {
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
            std::fs::remove_dir_all(&backup).map_err(|e| SwapError::RemoveFailed {
                path: backup.clone(),
                source: e,
            })?;
        }

        std::fs::rename(&self.workspace, &backup).map_err(|e| SwapError::RenameFailed {
            from: self.workspace.clone(),
            to: backup.clone(),
            source: e,
        })?;

        sync_dir(self.workspace.parent().unwrap_or(Path::new(".")))?;

        std::fs::rename(&shadow, &self.workspace).map_err(|e| SwapError::RenameFailed {
            from: shadow.clone(),
            to: self.workspace.clone(),
            source: e,
        })?;

        sync_dir(self.workspace.parent().unwrap_or(Path::new(".")))?;

        self.write_journal(SwapPhase::Complete)?;

        if backup.exists() {
            std::fs::remove_dir_all(&backup).map_err(|e| SwapError::RemoveFailed {
                path: backup,
                source: e,
            })?;
        }

        let journal = self.journal_path();
        if journal.exists() {
            std::fs::remove_file(&journal).map_err(|e| SwapError::RemoveFailed {
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
                            std::fs::remove_dir_all(&shadow).map_err(|e| SwapError::RemoveFailed {
                                path: shadow.clone(),
                                source: e,
                            })?;
                        }
                        self.cleanup_journal()?;
                        Ok(RecoveryOutcome::RolledBack)
                    }
                    SwapPhase::Swapping => {
                        if backup.exists() && !self.workspace.exists() {
                            std::fs::rename(&backup, &self.workspace).map_err(|e| {
                                SwapError::RenameFailed {
                                    from: backup.clone(),
                                    to: self.workspace.clone(),
                                    source: e,
                                }
                            })?;
                            sync_dir(self.workspace.parent().unwrap_or(Path::new(".")))?;
                        } else if backup.exists() && self.workspace.exists() {
                            std::fs::remove_dir_all(&backup).map_err(|e| SwapError::RemoveFailed {
                                path: backup.clone(),
                                source: e,
                            })?;
                        }

                        if shadow.exists() {
                            std::fs::remove_dir_all(&shadow).map_err(|e| SwapError::RemoveFailed {
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

        std::fs::write(&journal, content).map_err(|e| SwapError::JournalWrite {
            path: journal.clone(),
            source: e,
        })?;

        sync_dir(journal.parent().unwrap_or(Path::new(".")))?;

        Ok(())
    }

    fn cleanup_journal(&self) -> Result<(), SwapError> {
        let journal = self.journal_path();
        if journal.exists() {
            std::fs::remove_file(&journal).map_err(|e| SwapError::RemoveFailed {
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

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), SwapError> {
    std::fs::create_dir_all(dst).map_err(|e| SwapError::ShadowCreate {
        path: dst.to_path_buf(),
        source: e,
    })?;

    for entry in std::fs::read_dir(src).map_err(|e| SwapError::CopyFailed {
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
            std::fs::copy(&src_path, &dst_path).map_err(|e| SwapError::CopyFailed {
                from: src_path.clone(),
                to: dst_path.clone(),
                source: e,
            })?;
        }
    }

    Ok(())
}
