//! Configuration hot-reload system with file watching, atomic swap, validation, and rollback.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEvent {
    Modify(PathBuf),
    Delete(PathBuf),
}

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
        self.current.read().unwrap().clone()
    }

    pub fn try_update(&self, new_config: T) -> Result<(), Error> {
        self.validator
            .validate(&new_config)
            .map_err(|e| Error::ValidationFailed(e))?;

        let mut pending = self.pending.write().unwrap();
        *pending = Some(new_config);

        Ok(())
    }

    pub fn commit(&self) -> Result<T, Error> {
        let mut pending = self.pending.write().unwrap();
        if let Some(new_config) = pending.take() {
            let mut current = self.current.write().unwrap();
            let old = (*current).clone();
            *current = new_config.clone();
            return Ok(old);
        }
        Err(Error::SwapFailed)
    }

    pub fn rollback(&self) {
        let mut pending = self.pending.write().unwrap();
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
            .map_err(|e| Error::ValidationFailed(e))?;

        let mut current = self.current.write().unwrap();
        let old = (*current).clone();
        *current = new_config;

        Ok(old)
    }
}

pub struct FileWatcher {
    watcher: RecommendedWatcher,
    path: PathBuf,
}

impl FileWatcher {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();

        let watcher = RecommendedWatcher::new(
            move |_res: Result<notify::Event, notify::Error>| {},
            NotifyConfig::default(),
        )
        .map_err(|e| Error::WatcherError(e.to_string()))?;

        let mut watcher = watcher;
        watcher
            .watch(&path, RecursiveMode::NonRecursive)
            .map_err(|e| Error::WatcherError(e.to_string()))?;

        Ok(Self { watcher, path })
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
}
