use std::path::{Path, PathBuf};

use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};

use crate::debounce::{Debouncer, FileEvent as DebouncedFileEvent};

use super::channel::EventChannel;
use super::error::Error;
use super::watcher::WatcherConfig;

pub struct DebouncedFileWatcher {
    watcher: RecommendedWatcher,
    path: PathBuf,
    config: WatcherConfig,
    #[allow(dead_code)]
    debouncer: Option<Debouncer>,
}

impl DebouncedFileWatcher {
    pub fn new<P: AsRef<Path>>(
        path: P,
        config: WatcherConfig,
    ) -> Result<(Self, EventChannel), Error> {
        let path = path.as_ref().to_path_buf();
        let config = config.clone();

        let (event_tx, event_rx) = tokio::sync::mpsc::channel(1000);

        let debouncer = if let Some(duration) = config.debounce_duration {
            Some(
                Debouncer::new(duration, event_rx)
                    .map_err(|e| Error::DebounceError(e.to_string()))?,
            )
        } else {
            None
        };

        let watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    match event.kind {
                        notify::EventKind::Modify(_) => {
                            for path in event.paths {
                                let _ = event_tx.blocking_send(DebouncedFileEvent::Modify(path));
                            }
                        }
                        notify::EventKind::Remove(_) => {
                            for path in event.paths {
                                let _ = event_tx.blocking_send(DebouncedFileEvent::Delete(path));
                            }
                        }
                        _ => {}
                    }
                }
            },
            NotifyConfig::default(),
        )
        .map_err(|e| Error::WatcherError(e.to_string()))?;

        let mut watcher = watcher;
        let mode = if config.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        watcher
            .watch(&path, mode)
            .map_err(|e| Error::WatcherError(e.to_string()))?;

        let channel = EventChannel::new(1000);

        Ok((
            Self {
                watcher,
                path,
                config,
                debouncer,
            },
            channel,
        ))
    }

    pub fn with_debouncer<P: AsRef<Path>>(
        path: P,
        config: WatcherConfig,
        debouncer: Debouncer,
    ) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();

        let watcher = RecommendedWatcher::new(
            move |_res: Result<notify::Event, notify::Error>| {},
            NotifyConfig::default(),
        )
        .map_err(|e| Error::WatcherError(e.to_string()))?;

        let mut watcher = watcher;
        let mode = if config.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        watcher
            .watch(&path, mode)
            .map_err(|e| Error::WatcherError(e.to_string()))?;

        Ok(Self {
            watcher,
            path,
            config,
            debouncer: Some(debouncer),
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn is_recursive(&self) -> bool {
        self.config.recursive
    }

    pub fn unwatch(&mut self) -> Result<(), Error> {
        self.watcher
            .unwatch(&self.path)
            .map_err(|e| Error::WatcherError(e.to_string()))
    }
}
