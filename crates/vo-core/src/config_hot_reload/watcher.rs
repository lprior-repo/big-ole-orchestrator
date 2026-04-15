use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};

use super::error::Error;

#[derive(Debug, Clone)]
pub struct WatcherConfig {
    pub recursive: bool,
    pub debounce_duration: Option<Duration>,
    pub patterns: Vec<String>,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            recursive: true,
            debounce_duration: Some(Duration::from_millis(300)),
            patterns: vec!["*".to_string()],
        }
    }
}

pub struct FileWatcher {
    watcher: RecommendedWatcher,
    path: PathBuf,
    recursive: bool,
}

impl FileWatcher {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        Self::with_recursive(path, false)
    }

    pub fn with_recursive<P: AsRef<Path>>(path: P, recursive: bool) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();

        let watcher = RecommendedWatcher::new(
            move |_res: Result<notify::Event, notify::Error>| {},
            NotifyConfig::default(),
        )
        .map_err(|e| Error::WatcherError(e.to_string()))?;

        let mut watcher = watcher;
        let mode = if recursive {
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
            recursive,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn is_recursive(&self) -> bool {
        self.recursive
    }

    pub fn unwatch(&mut self) -> Result<(), Error> {
        self.watcher
            .unwatch(&self.path)
            .map_err(|e| Error::WatcherError(e.to_string()))
    }
}

pub struct FilteredFileWatcher {
    watcher: RecommendedWatcher,
    path: PathBuf,
    config: WatcherConfig,
}

impl FilteredFileWatcher {
    pub fn new<P: AsRef<Path>>(path: P, config: WatcherConfig) -> Result<Self, Error> {
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
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn matches_pattern(&self, path: &Path) -> bool {
        if self.config.patterns.is_empty() {
            return true;
        }

        let path_str = path.to_string_lossy();
        for pattern in &self.config.patterns {
            if let Ok(glob) = glob::Pattern::new(pattern) {
                if glob.matches(&path_str) {
                    return true;
                }
            }
        }
        false
    }

    pub fn unwatch(&mut self) -> Result<(), Error> {
        self.watcher
            .unwatch(&self.path)
            .map_err(|e| Error::WatcherError(e.to_string()))
    }
}
