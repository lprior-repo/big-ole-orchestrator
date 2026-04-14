//! Configuration hot-reload system with file watching, atomic swap, validation, and rollback.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;

use crate::debounce::{Debouncer, FileEvent as DebouncedFileEvent};

pub use crate::debounce::FileEvent;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("Config file not found: {0}")]
    ConfigFileNotFound(PathBuf),

    #[error("Failed to read config file: {0}")]
    ReadError(PathBuf),

    #[error("Failed to parse config: {0}")]
    ParseError(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Watcher error: {0}")]
    WatcherError(String),

    #[error("Channel closed unexpectedly")]
    ChannelClosed,

    #[error("Swap failed: no valid config to swap to")]
    SwapFailed,

    #[error("Invalid glob pattern: {0}")]
    InvalidGlobPattern(String),

    #[error("Debounce error: {0}")]
    DebounceError(String),

    #[error("Event queue closed unexpectedly")]
    EventQueueClosed,
}

pub trait ConfigValidator<T: Clone + Send + Sync>: Send + Sync {
    fn validate(&self, config: &T) -> Result<(), String>;
}

pub struct HotReloadConfig<T: Clone + Send + Sync> {
    current: RwLock<T>,
    pending: RwLock<Option<T>>,
    path: PathBuf,
    validator: Arc<dyn ConfigValidator<T>>,
}

impl<T: Clone + Send + Sync + 'static> HotReloadConfig<T> {
    pub fn new(
        initial: T,
        path: PathBuf,
        validator: Arc<dyn ConfigValidator<T>>,
    ) -> Result<Self, Error> {
        if !path.exists() {
            return Err(Error::ConfigFileNotFound(path));
        }

        Ok(Self {
            current: RwLock::new(initial),
            pending: RwLock::new(None),
            path,
            validator,
        })
    }

    #[must_use]
    pub fn current(&self) -> T
    where
        T: Clone,
    {
        self.current
            .read()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock")
            .clone()
    }

    pub fn try_update(&self, new_config: T) -> Result<(), Error> {
        self.validator
            .validate(&new_config)
            .map_err(Error::ValidationFailed)?;

        let mut pending = self
            .pending
            .write()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        *pending = Some(new_config);

        Ok(())
    }

    pub fn commit(&self) -> Result<T, Error> {
        let mut pending = self
            .pending
            .write()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        if let Some(new_config) = pending.take() {
            let mut current = self.current.write().expect(
                "SAFETY: RwLock not poisoned — no code path panics while holding this lock",
            );
            let old = (*current).clone();
            *current = new_config.clone();
            return Ok(old);
        }
        Err(Error::SwapFailed)
    }

    pub fn rollback(&self) {
        let mut pending = self
            .pending
            .write()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        *pending = None;
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn reload_from_file(&self) -> Result<T, Error>
    where
        T: for<'de> serde::de::DeserializeOwned,
    {
        let content =
            std::fs::read_to_string(&self.path).map_err(|_| Error::ReadError(self.path.clone()))?;

        let new_config: T =
            serde_json::from_str(&content).map_err(|e| Error::ParseError(e.to_string()))?;

        self.validator
            .validate(&new_config)
            .map_err(Error::ValidationFailed)?;

        let mut current = self
            .current
            .write()
            .expect("SAFETY: RwLock not poisoned — no code path panics while holding this lock");
        let old = (*current).clone();
        *current = new_config;

        Ok(old)
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

pub struct EventChannel {
    tx: tokio::sync::mpsc::Sender<DebouncedFileEvent>,
}

impl EventChannel {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = tokio::sync::mpsc::channel(capacity);
        Self { tx }
    }

    pub async fn send(&self, event: DebouncedFileEvent) -> Result<(), Error> {
        self.tx
            .send(event)
            .await
            .map_err(|_| Error::EventQueueClosed)
    }

    pub fn sender(&self) -> tokio::sync::mpsc::Sender<DebouncedFileEvent> {
        self.tx.clone()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    struct AlwaysValid;
    impl<T: Clone + Send + Sync> ConfigValidator<T> for AlwaysValid {
        fn validate(&self, _config: &T) -> Result<(), String> {
            Ok(())
        }
    }

    struct AlwaysInvalid;
    impl<T: Clone + Send + Sync> ConfigValidator<T> for AlwaysInvalid {
        fn validate(&self, _config: &T) -> Result<(), String> {
            Err("always invalid".to_string())
        }
    }

    #[test]
    fn hot_reload_config_new_returns_error_when_file_missing() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("missing.json");
        let result = HotReloadConfig::<serde_json::Value>::new(
            serde_json::json!({}),
            path,
            Arc::new(AlwaysValid),
        );
        assert!(matches!(result, Err(Error::ConfigFileNotFound(_))));
    }

    #[test]
    fn hot_reload_config_new_succeeds_when_file_exists() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let result = HotReloadConfig::<serde_json::Value>::new(
            serde_json::json!({"key": "value"}),
            path,
            Arc::new(AlwaysValid),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn hot_reload_config_current_returns_initial_value() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"key": "value"}),
            path,
            Arc::new(AlwaysValid),
        )
        .unwrap();

        assert_eq!(config.current(), serde_json::json!({"key": "value"}));
    }

    #[test]
    fn hot_reload_config_try_update_succeeds_with_valid_config() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"key": "value"}),
            path,
            Arc::new(AlwaysValid),
        )
        .unwrap();

        let result = config.try_update(serde_json::json!({"new": "config"}));
        assert!(result.is_ok());
    }

    #[test]
    fn hot_reload_config_try_update_fails_with_invalid_config() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"key": "value"}),
            path,
            Arc::new(AlwaysInvalid),
        )
        .unwrap();

        let result = config.try_update(serde_json::json!({"new": "config"}));
        assert!(matches!(result, Err(Error::ValidationFailed(_))));
    }

    #[test]
    fn hot_reload_config_commit_applies_pending_config() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"key": "value"}),
            path,
            Arc::new(AlwaysValid),
        )
        .unwrap();

        config
            .try_update(serde_json::json!({"new": "config"}))
            .unwrap();
        let old = config.commit().unwrap();

        assert_eq!(old, serde_json::json!({"key": "value"}));
        assert_eq!(config.current(), serde_json::json!({"new": "config"}));
    }

    #[test]
    fn hot_reload_config_commit_returns_error_when_no_pending() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"key": "value"}),
            path,
            Arc::new(AlwaysValid),
        )
        .unwrap();

        let result = config.commit();
        assert!(matches!(result, Err(Error::SwapFailed)));
    }

    #[test]
    fn hot_reload_config_rollback_clears_pending() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"key": "value"}),
            path,
            Arc::new(AlwaysValid),
        )
        .unwrap();

        config
            .try_update(serde_json::json!({"new": "config"}))
            .unwrap();
        config.rollback();

        let result = config.commit();
        assert!(matches!(result, Err(Error::SwapFailed)));
    }

    #[test]
    fn hot_reload_config_reload_from_file_updates_config() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, r#"{"updated": true}"#).unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"key": "value"}),
            path,
            Arc::new(AlwaysValid),
        )
        .unwrap();

        let old = config.reload_from_file().unwrap();
        assert_eq!(old, serde_json::json!({"key": "value"}));
        assert_eq!(config.current(), serde_json::json!({"updated": true}));
    }

    #[test]
    fn hot_reload_config_reload_from_file_validates_before_update() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, r#"{"updated": true}"#).unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"key": "value"}),
            path,
            Arc::new(AlwaysInvalid),
        )
        .unwrap();

        let result = config.reload_from_file();
        assert!(matches!(result, Err(Error::ValidationFailed(_))));
        assert_eq!(config.current(), serde_json::json!({"key": "value"}));
    }

    #[test]
    fn file_event_modify_contains_path() {
        let path = PathBuf::from("/tmp/test.json");
        let event = FileEvent::Modify(path.clone());
        assert!(matches!(event, FileEvent::Modify(p) if p == path));
    }

    #[test]
    fn file_event_delete_contains_path() {
        let path = PathBuf::from("/tmp/test.json");
        let event = FileEvent::Delete(path.clone());
        assert!(matches!(event, FileEvent::Delete(p) if p == path));
    }

    #[test]
    fn watcher_config_default_is_recursive_with_debounce() {
        let config = WatcherConfig::default();
        assert!(config.recursive);
        assert!(config.debounce_duration.is_some());
        assert_eq!(config.patterns, vec!["*".to_string()]);
    }

    #[test]
    fn file_watcher_with_recursive_creates_recursive_watcher() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let watcher = FileWatcher::with_recursive(&path, true).unwrap();
        assert!(watcher.is_recursive());
        assert_eq!(watcher.path(), path);
    }

    #[test]
    fn file_watcher_with_non_recursive_creates_non_recursive_watcher() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let watcher = FileWatcher::with_recursive(&path, false).unwrap();
        assert!(!watcher.is_recursive());
        assert_eq!(watcher.path(), path);
    }

    #[test]
    fn filtered_file_watcher_matches_wildcard_pattern() {
        let config = WatcherConfig {
            recursive: true,
            debounce_duration: None,
            patterns: vec!["*.json".to_string()],
        };
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let watcher = FilteredFileWatcher::new(&path, config).unwrap();
        assert!(watcher.matches_pattern(Path::new("/some/path/file.json")));
        assert!(!watcher.matches_pattern(Path::new("/some/path/file.txt")));
    }

    #[test]
    fn filtered_file_watcher_matches_multiple_patterns() {
        let config = WatcherConfig {
            recursive: true,
            debounce_duration: None,
            patterns: vec!["*.json".to_string(), "*.toml".to_string()],
        };
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let watcher = FilteredFileWatcher::new(&path, config).unwrap();
        assert!(watcher.matches_pattern(Path::new("/some/path/file.json")));
        assert!(watcher.matches_pattern(Path::new("/some/path/file.toml")));
        assert!(!watcher.matches_pattern(Path::new("/some/path/file.txt")));
    }

    #[test]
    fn filtered_file_watcher_empty_patterns_matches_all() {
        let config = WatcherConfig {
            recursive: true,
            debounce_duration: None,
            patterns: vec![],
        };
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let watcher = FilteredFileWatcher::new(&path, config).unwrap();
        assert!(watcher.matches_pattern(Path::new("/any/path/file.json")));
        assert!(watcher.matches_pattern(Path::new("/any/path/file.txt")));
    }

    #[test]
    fn event_channel_new_creates_channel() {
        let channel = EventChannel::new(100);
        let sender = channel.sender();
        assert!(sender.capacity() > 0);
    }

    #[test]
    fn hot_reload_config_path_returns_exact_provided_path() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("subdir").join("config.json");
        fs::create_dir_all(temp_dir.path().join("subdir")).unwrap();
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"key": "value"}),
            path.clone(),
            Arc::new(AlwaysValid),
        )
        .unwrap();

        assert_eq!(config.path(), path);
    }

    #[test]
    fn hot_reload_config_try_update_does_not_modify_current() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"key": "value"}),
            path,
            Arc::new(AlwaysValid),
        )
        .unwrap();

        config
            .try_update(serde_json::json!({"new": "config"}))
            .unwrap();

        assert_eq!(config.current(), serde_json::json!({"key": "value"}));
    }

    #[test]
    fn hot_reload_config_try_update_twice_overwrites_previous_pending() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"key": "original"}),
            path,
            Arc::new(AlwaysValid),
        )
        .unwrap();

        config
            .try_update(serde_json::json!({"key": "first"}))
            .unwrap();
        config
            .try_update(serde_json::json!({"key": "second"}))
            .unwrap();

        let old = config.commit().unwrap();
        assert_eq!(old, serde_json::json!({"key": "original"}));
        assert_eq!(config.current(), serde_json::json!({"key": "second"}));
    }

    #[test]
    fn hot_reload_config_commit_clears_pending_after_promotion() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config =
            HotReloadConfig::new(serde_json::json!({"v": 1}), path, Arc::new(AlwaysValid)).unwrap();

        config.try_update(serde_json::json!({"v": 2})).unwrap();
        let _old = config.commit().unwrap();

        let result = config.commit();
        assert!(matches!(result, Err(Error::SwapFailed)));
    }

    #[test]
    fn hot_reload_config_rollback_when_no_pending_is_noop() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config =
            HotReloadConfig::new(serde_json::json!({"v": 1}), path, Arc::new(AlwaysValid)).unwrap();

        config.rollback();
        assert_eq!(config.current(), serde_json::json!({"v": 1}));
    }

    #[test]
    fn hot_reload_config_reload_from_file_returns_parse_error_for_malformed_json() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "not valid json {{{").unwrap();

        let config =
            HotReloadConfig::new(serde_json::json!({"v": 1}), path, Arc::new(AlwaysValid)).unwrap();

        let result = config.reload_from_file();
        assert!(matches!(result, Err(Error::ParseError(_))));
        assert_eq!(config.current(), serde_json::json!({"v": 1}));
    }

    #[test]
    fn hot_reload_config_reload_from_file_does_not_affect_pending() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, r#"{"v": 1}"#).unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"v": 1}),
            path.clone(),
            Arc::new(AlwaysValid),
        )
        .unwrap();

        config.try_update(serde_json::json!({"v": 999})).unwrap();

        fs::write(&path, r#"{"v": 2}"#).unwrap();
        let old = config.reload_from_file().unwrap();

        assert_eq!(old, serde_json::json!({"v": 1}));
        assert_eq!(config.current(), serde_json::json!({"v": 2}));

        let committed = config.commit().unwrap();
        assert_eq!(committed, serde_json::json!({"v": 2}));
        assert_eq!(config.current(), serde_json::json!({"v": 999}));
    }

    #[test]
    fn hot_reload_config_current_returns_independent_clones() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"key": "value"}),
            path,
            Arc::new(AlwaysValid),
        )
        .unwrap();

        let mut clone1 = config.current();
        let mut clone2 = config.current();

        clone1
            .as_object_mut()
            .unwrap()
            .insert("modified".to_string(), serde_json::json!(true));
        clone2
            .as_object_mut()
            .unwrap()
            .insert("other".to_string(), serde_json::json!(42));

        assert_eq!(config.current(), serde_json::json!({"key": "value"}));
    }

    #[test]
    fn hot_reload_config_commit_returns_previous_current_for_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config =
            HotReloadConfig::new(serde_json::json!({"v": 1}), path, Arc::new(AlwaysValid)).unwrap();

        config.try_update(serde_json::json!({"v": 2})).unwrap();
        let old = config.commit().unwrap();

        assert_eq!(old, serde_json::json!({"v": 1}));
        assert_eq!(config.current(), serde_json::json!({"v": 2}));
    }

    #[test]
    fn hot_reload_config_new_initializes_current_with_provided_value() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let initial = serde_json::json!({"nested": {"deep": true}});
        let config = HotReloadConfig::new(initial.clone(), path, Arc::new(AlwaysValid)).unwrap();

        assert_eq!(config.current(), initial);
    }

    #[test]
    fn hot_reload_config_try_update_validates_before_staging() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let config =
            HotReloadConfig::new(serde_json::json!({"v": 1}), path, Arc::new(AlwaysInvalid))
                .unwrap();

        let result = config.try_update(serde_json::json!({"v": 2}));
        assert!(matches!(result, Err(Error::ValidationFailed(_))));

        let result = config.commit();
        assert!(matches!(result, Err(Error::SwapFailed)));
    }

    #[test]
    fn hot_reload_config_reload_from_file_reads_from_stored_path() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, r#"{"original": true}"#).unwrap();

        let config = HotReloadConfig::new(
            serde_json::json!({"original": true}),
            path.clone(),
            Arc::new(AlwaysValid),
        )
        .unwrap();

        assert_eq!(config.path(), path);

        fs::write(&path, r#"{"updated": true}"#).unwrap();
        config.reload_from_file().unwrap();

        assert_eq!(config.current(), serde_json::json!({"updated": true}));
    }

    #[test]
    fn file_watcher_new_creates_non_recursive_watcher_by_default() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let watcher = FileWatcher::new(&path).unwrap();
        assert!(!watcher.is_recursive());
        assert_eq!(watcher.path(), path);
    }

    #[test]
    fn filtered_file_watcher_matches_doublestar_pattern() {
        let config = WatcherConfig {
            recursive: true,
            debounce_duration: None,
            patterns: vec!["**/*.json".to_string()],
        };
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let watcher = FilteredFileWatcher::new(&path, config).unwrap();
        assert!(watcher.matches_pattern(Path::new("/deep/nested/dir/file.json")));
        assert!(!watcher.matches_pattern(Path::new("/some/path/file.txt")));
    }

    #[test]
    fn filtered_file_watcher_invalid_glob_pattern_returns_false() {
        let config = WatcherConfig {
            recursive: true,
            debounce_duration: None,
            patterns: vec!["[invalid".to_string()],
        };
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let watcher = FilteredFileWatcher::new(&path, config).unwrap();
        assert!(!watcher.matches_pattern(Path::new("/any/path/file.json")));
    }

    #[test]
    fn filtered_file_watcher_path_returns_watched_path() {
        let config = WatcherConfig::default();
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("config.json");
        fs::write(&path, "{}").unwrap();

        let watcher = FilteredFileWatcher::new(&path, config).unwrap();
        assert_eq!(watcher.path(), path);
    }

    #[test]
    fn watcher_config_default_debounce_is_300ms() {
        let config = WatcherConfig::default();
        assert_eq!(config.debounce_duration, Some(Duration::from_millis(300)));
    }

    #[tokio::test]
    async fn event_channel_send_returns_error_when_receiver_dropped() {
        let channel = EventChannel::new(10);
        drop(channel.sender());

        let result = channel
            .send(crate::debounce::FileEvent::Modify(PathBuf::from("/test")))
            .await;
        assert!(matches!(result, Err(Error::EventQueueClosed)));
    }

    #[test]
    fn error_config_file_not_found_display_contains_path() {
        let err = Error::ConfigFileNotFound(PathBuf::from("/tmp/missing.json"));
        let msg = format!("{err}");
        assert!(msg.contains("/tmp/missing.json"));
    }

    #[test]
    fn error_read_error_display_contains_path() {
        let err = Error::ReadError(PathBuf::from("/tmp/unreadable.json"));
        let msg = format!("{err}");
        assert!(msg.contains("/tmp/unreadable.json"));
    }

    #[test]
    fn error_parse_error_display_contains_message() {
        let err = Error::ParseError("expected `:` at line 1".to_string());
        let msg = format!("{err}");
        assert!(msg.contains("expected `:` at line 1"));
    }

    #[test]
    fn error_validation_failed_display_contains_reason() {
        let err = Error::ValidationFailed("missing version field".to_string());
        let msg = format!("{err}");
        assert!(msg.contains("missing version field"));
    }

    #[test]
    fn error_watcher_error_display_contains_message() {
        let err = Error::WatcherError("path not found".to_string());
        let msg = format!("{err}");
        assert!(msg.contains("path not found"));
    }

    #[test]
    fn error_swap_failed_display() {
        let err = Error::SwapFailed;
        let msg = format!("{err}");
        assert!(msg.contains("Swap failed"));
    }

    #[test]
    fn error_channel_closed_display() {
        let err = Error::ChannelClosed;
        let msg = format!("{err}");
        assert!(msg.contains("closed"));
    }

    #[test]
    fn error_debounce_error_display_contains_message() {
        let err = Error::DebounceError("duration cannot be zero".to_string());
        let msg = format!("{err}");
        assert!(msg.contains("duration cannot be zero"));
    }

    #[test]
    fn error_event_queue_closed_display() {
        let err = Error::EventQueueClosed;
        let msg = format!("{err}");
        assert!(msg.contains("closed"));
    }

    #[test]
    fn error_invalid_glob_pattern_display_contains_pattern() {
        let err = Error::InvalidGlobPattern("[invalid".to_string());
        let msg = format!("{err}");
        assert!(msg.contains("[invalid"));
    }

    #[test]
    fn error_partial_eq_consistency() {
        assert_eq!(
            Error::ConfigFileNotFound(PathBuf::from("/a")),
            Error::ConfigFileNotFound(PathBuf::from("/a"))
        );
        assert_ne!(
            Error::ConfigFileNotFound(PathBuf::from("/a")),
            Error::ConfigFileNotFound(PathBuf::from("/b"))
        );
        assert_eq!(Error::SwapFailed, Error::SwapFailed);
        assert_eq!(
            Error::ValidationFailed("err".to_string()),
            Error::ValidationFailed("err".to_string())
        );
    }
}
