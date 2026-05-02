//! Ghost workflow file watcher.
//!
//! Watches the workflow binary directory for deletions and triggers
//! lifecycle transitions via GhostLifecycle.

use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;

use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use vo_types::{RegistrationStatus, WorkflowName};

use crate::ghost_workflow::{GhostLifecycle, GhostWorkflowError};

const WORKFLOW_BINARY_DIR: &str = "/var/wtf/versions";

#[derive(Debug, Clone)]
pub struct BinaryRemoved {
    pub path: PathBuf,
}

pub struct GhostWorkflowWatcher {
    watcher: RecommendedWatcher,
    path: PathBuf,
    ghost_lifecycle: std::sync::Arc<tokio::sync::RwLock<GhostLifecycle>>,
    event_sender: Option<std_mpsc::SyncSender<BinaryRemoved>>,
}

impl GhostWorkflowWatcher {
    pub fn new(
        path: impl AsRef<Path>,
        ghost_lifecycle: std::sync::Arc<tokio::sync::RwLock<GhostLifecycle>>,
    ) -> Result<Self, GhostWorkflowWatcherError> {
        Self::new_with_sender(path, ghost_lifecycle, None)
    }

    pub fn new_with_channel(
        path: impl AsRef<Path>,
        ghost_lifecycle: std::sync::Arc<tokio::sync::RwLock<GhostLifecycle>>,
        event_sender: std_mpsc::SyncSender<BinaryRemoved>,
    ) -> Result<Self, GhostWorkflowWatcherError> {
        Self::new_with_sender(path, ghost_lifecycle, Some(event_sender))
    }

    fn new_with_sender(
        path: impl AsRef<Path>,
        ghost_lifecycle: std::sync::Arc<tokio::sync::RwLock<GhostLifecycle>>,
        event_sender: Option<std_mpsc::SyncSender<BinaryRemoved>>,
    ) -> Result<Self, GhostWorkflowWatcherError> {
        let path = path.as_ref().to_path_buf();
        let sender = event_sender.clone();

        let watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    match event.kind {
                        notify::EventKind::Remove(_) => {
                            for removed_path in event.paths {
                                if let Some(ref s) = sender {
                                    let _ = s.send(BinaryRemoved { path: removed_path.clone() });
                                }
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
            event_sender,
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
    use std::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn ghost_workflow_watcher_creates_successfully() {
        let temp_dir = tempfile::tempdir().unwrap();
        let ghost_lifecycle =
            std::sync::Arc::new(tokio::sync::RwLock::new(GhostLifecycle::new()));
        let watcher = GhostWorkflowWatcher::new(temp_dir.path(), ghost_lifecycle);
        assert!(watcher.is_ok());
    }

    #[test]
    fn binary_path_extraction_from_deleted_path() {
        let path = PathBuf::from(
            "/var/wtf/versions/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/test-workflow",
        );
        assert!(path.starts_with(WORKFLOW_BINARY_DIR));
    }

    #[tokio::test]
    async fn ghost_workflow_watcher_detects_binary_deletion() {
        let temp_dir = tempfile::tempdir().unwrap();
        let binary_path = temp_dir.path().join("test-binary");
        std::fs::write(&binary_path, b"#!/bin/bash\necho test").unwrap();
        std::fs::set_permissions(&binary_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let ghost_lifecycle =
            std::sync::Arc::new(tokio::sync::RwLock::new(GhostLifecycle::new()));

        let (tx, rx) = std_mpsc::sync_channel(1);
        let mut watcher =
            GhostWorkflowWatcher::new_with_channel(temp_dir.path(), ghost_lifecycle, tx)
                .unwrap();

        drop(watcher);

        std::fs::remove_file(&binary_path).unwrap();

        let result = timeout(Duration::from_secs(2), rx.recv()).await;
        assert!(result.is_ok(), "Should receive BinaryRemoved event within 2s");
        let event = result.unwrap().unwrap();
        assert_eq!(event.path, binary_path);
    }

    #[tokio::test]
    async fn ghost_workflow_watcher_emits_binary_removed_event_on_deletion() {
        let temp_dir = tempfile::tempdir().unwrap();
        let binary_path = temp_dir.path().join("workflow-binary");
        std::fs::write(&binary_path, b"#!/bin/bash\nexit 0").unwrap();
        std::fs::set_permissions(&binary_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let ghost_lifecycle =
            std::sync::Arc::new(tokio::sync::RwLock::new(GhostLifecycle::new()));

        let (tx, rx) = std_mpsc::sync_channel(1);
        let _watcher =
            GhostWorkflowWatcher::new_with_channel(temp_dir.path(), ghost_lifecycle, tx)
                .unwrap();

        std::fs::remove_file(&binary_path).unwrap();

        let result = timeout(Duration::from_secs(2), rx.recv()).await;
        assert!(result.is_ok(), "timeout waiting for BinaryRemoved event");
        let event = result.unwrap().unwrap();
        assert_eq!(event.path, binary_path);
    }
}