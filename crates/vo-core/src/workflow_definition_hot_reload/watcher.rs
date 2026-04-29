use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use crate::debounce::{Debouncer, FileEvent};

use super::error::Error;
use super::loader::WorkflowDefinitionLoader;
use super::registry::SharedWorkflowRegistry;

pub struct WorkflowDefinitionWatcher {
    watcher: RecommendedWatcher,
    path: PathBuf,
}

impl WorkflowDefinitionWatcher {
    pub fn new<P: AsRef<Path>>(
        path: P,
        registry: SharedWorkflowRegistry,
    ) -> Result<(Self, mpsc::Receiver<Result<PathBuf, Error>>), Error> {
        let path = path.as_ref().to_path_buf();
        let loader = Arc::new(WorkflowDefinitionLoader::new(registry.clone()));

        let (event_tx, event_rx) = mpsc::channel(1000);
        let (result_tx, result_rx) = mpsc::channel(1000);

        let watcher_event_tx = event_tx.clone();
        let watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    if let notify::EventKind::Modify(_) = event.kind {
                        for path in event.paths {
                            if is_workflow_binary(&path) {
                                let _ = watcher_event_tx
                                    .blocking_send(FileEvent::Modify(path));
                            }
                        }
                    }
                }
            },
            NotifyConfig::default(),
        )
        .map_err(|e| Error::WatcherError(e.to_string()))?;

        let mut watcher = watcher;
        watcher
            .watch(&path, RecursiveMode::NonRecursive)
            .map_err(|e| Error::WatcherError(e.to_string()))?;

        let debouncer = Debouncer::new(Duration::from_millis(300), event_rx)
            .map_err(|e| Error::DebounceError(e.to_string()))?;

        let loader_clone = loader;
        tokio::spawn(async move {
            let mut debouncer = debouncer;
            loop {
                match debouncer.next_debounced_event().await {
                    Ok(path) => {
                        debug!(path = %path.display(), "workflow binary modified");
                        let loader = loader_clone.as_ref();
                        match loader.reload_from_binary(&path).await {
                            Ok(Some(def)) => {
                                info!(
                                    workflow_name = %def.workflow_name,
                                    path = %path.display(),
                                    "workflow definition reloaded"
                                );
                                let _ = result_tx.send(Ok(path)).await;
                            }
                            Ok(None) => {
                                let _ = result_tx.send(Ok(path)).await;
                            }
                            Err(e) => {
                                error!(
                                    path = %path.display(),
                                    error = %e,
                                    "failed to reload workflow definition"
                                );
                                let _ = result_tx.send(Err(e)).await;
                            }
                        }
                    }
                    Err(crate::debounce::Error::WatcherChannelClosed) => {
                        break;
                    }
                    Err(e) => {
                        error!(error = %e, "debouncer error in workflow watcher");
                        let _ = result_tx
                            .send(Err(Error::DebounceError(e.to_string())))
                            .await;
                    }
                }
            }
        });

        Ok((
            Self {
                watcher,
                path,
            },
            result_rx,
        ))
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn unwatch(&mut self) -> Result<(), Error> {
        self.watcher
            .unwatch(&self.path)
            .map_err(|e| Error::WatcherError(e.to_string()))
    }
}

fn is_workflow_binary(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        return ext_str == "wasm" || ext_str == "elf" || ext_str == "exe" || ext_str == "bin";
    }

    if let Some(name) = path.file_name() {
        let name_str = name.to_string_lossy().to_lowercase();
        return !name_str.starts_with('.')
            && !name_str.contains("lock")
            && !name_str.ends_with(".md")
            && !name_str.ends_with(".txt");
    }

    false
}