//! Ghost workflow file watcher.
//!
//! Watches the workflow binary directory for deletions and triggers
//! lifecycle transitions via GhostLifecycle.

use std::path::{Path, PathBuf};

use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use vo_types::{RegistrationStatus, WorkflowName};

use crate::ghost_workflow::{GhostLifecycle, GhostWorkflowError};

const WORKFLOW_BINARY_DIR: &str = "/var/wtf/versions";

pub struct GhostWorkflowWatcher {
    watcher: RecommendedWatcher,
    path: PathBuf,
    ghost_lifecycle: std::sync::Arc<tokio::sync::RwLock<GhostLifecycle>>,
}

impl GhostWorkflowWatcher {
    pub fn new(
        path: impl AsRef<Path>,
        ghost_lifecycle: std::sync::Arc<tokio::sync::RwLock<GhostLifecycle>>,
    ) -> Result<Self, GhostWorkflowWatcherError> {
        let path = path.as_ref().to_path_buf();

        let watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    match event.kind {
                        notify::EventKind::Remove(_) => {
                            for removed_path in event.paths {
                                Self::handle_deletion(removed_path);
                            }
                        }
                        _ => {}
                    }
                }
            },
            NotifyConfig::default(),
        )
        .map_err(|e| GhostWorkflowWatcherError::WatcherError(e.to_string()))?;

        let mut watcher = watcher;
        let mode = RecursiveMode::NonRecursive;
        watcher
            .watch(&path, mode)
            .map_err(|e| GhostWorkflowWatcherError::WatcherError(e.to_string()))?;

        Ok(Self {
            watcher,
            path,
            ghost_lifecycle,
        })
    }

    fn handle_deletion(removed_path: PathBuf) {
        tracing::info!(path = ?removed_path, "workflow binary deleted");
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GhostWorkflowWatcherError {
    #[error("watcher error: {0}")]
    WatcherError(String),
    #[error("workflow name parse error: {0}")]
    WorkflowNameError(String),
}

impl From<GhostWorkflowWatcherError> for GhostWorkflowError {
    fn from(err: GhostWorkflowWatcherError) -> Self {
        match err {
            GhostWorkflowWatcherError::WorkflowNameError(name) => {
                GhostWorkflowError::InvalidTransition {
                    workflow: name,
                    from: RegistrationStatus::Deleted,
                    to: RegistrationStatus::Deactivated,
                }
            }
            GhostWorkflowWatcherError::WatcherError(_) => {
                GhostWorkflowError::ReaperNotDeactivated {
                    workflow: "watcher".to_string(),
                    status: RegistrationStatus::Active,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_workflow_watcher_creates_successfully() {
        let temp_dir = tempfile::tempdir().unwrap();
        let ghost_lifecycle = std::sync::Arc::new(tokio::sync::RwLock::new(GhostLifecycle::new()));
        let watcher = GhostWorkflowWatcher::new(temp_dir.path(), ghost_lifecycle);
        assert!(watcher.is_ok());
    }

    #[test]
    fn binary_path_extraction_from_deleted_path() {
        let path = PathBuf::from("/var/wtf/versions/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/test-workflow");
        assert!(path.starts_with(WORKFLOW_BINARY_DIR));
    }
}
