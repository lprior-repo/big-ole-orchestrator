use std::path::{Path, PathBuf};

use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::debounce::{Debouncer, FileEvent as DebouncedFileEvent};

use super::error::Error;
use super::watcher::WatcherConfig;

pub struct DebouncedFileWatcher {
    watcher: RecommendedWatcher,
    path: PathBuf,
    config: WatcherConfig,
    #[allow(dead_code)]
    debouncer: Option<Debouncer>,
    #[allow(dead_code)]
    event_tx: mpsc::Sender<DebouncedFileEvent>,
}

impl DebouncedFileWatcher {
    pub fn new<P: AsRef<Path>>(
        path: P,
        config: WatcherConfig,
    ) -> Result<(Self, mpsc::Receiver<Result<PathBuf, Error>>), Error> {
        let path = path.as_ref().to_path_buf();
        let config = config.clone();

        let (event_tx, event_rx) = mpsc::channel(1000);
        let (result_tx, result_rx) = mpsc::channel(1000);

        if let Some(duration) = config.debounce_duration {
            let debouncer = Debouncer::new(duration, event_rx)
                .map_err(|e| Error::DebounceError(e.to_string()))?;
            let tx = result_tx;
            tokio::spawn(async move {
                let mut debouncer = debouncer;
                loop {
                    match debouncer.next_debounced_event().await {
                        Ok(path) => {
                            if tx.send(Ok(path)).await.is_err() {
                                break;
                            }
                        }
                        Err(crate::debounce::Error::WatcherChannelClosed) => {
                            break;
                        }
                        Err(e) => {
                            let _ = tx.send(Err(Error::DebounceError(e.to_string()))).await;
                        }
                    }
                }
            });
        };

        let watcher_event_tx = event_tx.clone();
        let watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    match event.kind {
                        notify::EventKind::Modify(_) => {
                            for path in event.paths {
                                let _ = watcher_event_tx
                                    .blocking_send(DebouncedFileEvent::Modify(path));
                            }
                        }
                        notify::EventKind::Remove(_) => {
                            for path in event.paths {
                                let _ = watcher_event_tx
                                    .blocking_send(DebouncedFileEvent::Delete(path));
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

        Ok((
            Self {
                watcher,
                path,
                config,
                debouncer: None,
                event_tx,
            },
            result_rx,
        ))
    }

    pub fn with_debouncer<P: AsRef<Path>>(
        path: P,
        config: WatcherConfig,
        debouncer: Debouncer,
    ) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();

        let (event_tx, _event_rx) = mpsc::channel(1000);
        let (result_tx, _result_rx) = mpsc::channel(1000);

        let mut debouncer = debouncer;
        let tx = result_tx;
        tokio::spawn(async move {
            loop {
                match debouncer.next_debounced_event().await {
                    Ok(path) => {
                        if tx.send(Ok(path)).await.is_err() {
                            break;
                        }
                    }
                    Err(crate::debounce::Error::WatcherChannelClosed) => {
                        break;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(Error::DebounceError(e.to_string()))).await;
                    }
                }
            }
        });

        let watcher_event_tx = event_tx.clone();
        let watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    match event.kind {
                        notify::EventKind::Modify(_) => {
                            for path in event.paths {
                                let _ = watcher_event_tx
                                    .blocking_send(DebouncedFileEvent::Modify(path));
                            }
                        }
                        notify::EventKind::Remove(_) => {
                            for path in event.paths {
                                let _ = watcher_event_tx
                                    .blocking_send(DebouncedFileEvent::Delete(path));
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

        Ok(Self {
            watcher,
            path,
            config,
            debouncer: None,
            event_tx,
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
