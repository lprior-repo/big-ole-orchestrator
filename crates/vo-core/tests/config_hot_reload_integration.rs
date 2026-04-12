//! Integration tests for Config hot-reload system.
//!
//! TDD Red Phase: These tests verify cross-component behavior.
//! They should FAIL until the implementation is complete.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;
use vo_core::config_hot_reload::{
    ConfigValidator, DebouncedFileWatcher, Error, EventChannel, FileWatcher, FilteredFileWatcher,
    HotReloadConfig, WatcherConfig,
};

struct AlwaysValid;
impl<T: Clone + Send + Sync> ConfigValidator<T> for AlwaysValid {
    fn validate(&self, _config: &T) -> Result<(), String> {
        Ok(())
    }
}

struct VersionValidator;
impl ConfigValidator<serde_json::Value> for VersionValidator {
    fn validate(&self, config: &serde_json::Value) -> Result<(), String> {
        if config.get("version").and_then(|v| v.as_u64()).is_some() {
            Ok(())
        } else {
            Err("missing version field".to_string())
        }
    }
}

struct MinVersionValidator(u64);
impl ConfigValidator<serde_json::Value> for MinVersionValidator {
    fn validate(&self, config: &serde_json::Value) -> Result<(), String> {
        let version = config.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
        if version >= self.0 {
            Ok(())
        } else {
            Err(format!("version {} below minimum {}", version, self.0))
        }
    }
}

// ============================================================
// Integration: HotReloadConfig + FileWatcher
// ============================================================

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

    let config =
        HotReloadConfig::new(serde_json::json!({"v": 1}), path, Arc::new(AlwaysValid)).unwrap();

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
fn hot_reload_config_reload_from_file_returns_old_config() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 1}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 1}),
        path,
        Arc::new(VersionValidator),
    )
    .unwrap();

    fs::write(&path, r#"{"version": 2}"#).unwrap();
    let old = config.reload_from_file().unwrap();

    assert_eq!(old, serde_json::json!({"version": 1}));
    assert_eq!(config.current(), serde_json::json!({"version": 2}));
}

// ============================================================
// Integration: FilteredFileWatcher + DebouncedFileWatcher
// ============================================================

#[test]
fn filtered_file_watcher_matches_doublestar_glob_pattern() {
    let config = WatcherConfig {
        recursive: true,
        debounce_duration: None,
        patterns: vec!["**/*.json".to_string()],
    };
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let watcher = FilteredFileWatcher::new(&path, config).unwrap();
    assert!(watcher.matches_pattern(Path::new("/some/path/file.json")));
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
fn filtered_file_watcher_returns_watched_path() {
    let config = WatcherConfig::default();
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let watcher = FilteredFileWatcher::new(&path, config).unwrap();
    assert_eq!(watcher.path(), path);
}

#[test]
fn debounced_file_watcher_returns_watched_path() {
    let config = WatcherConfig {
        recursive: false,
        debounce_duration: Some(Duration::from_millis(100)),
        patterns: vec!["*".to_string()],
    };
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let (watcher, _channel) = DebouncedFileWatcher::new(&path, config).unwrap();
    assert_eq!(watcher.path(), path);
}

#[test]
fn debounced_file_watcher_is_recursive_matches_config() {
    let config = WatcherConfig {
        recursive: true,
        debounce_duration: Some(Duration::from_millis(100)),
        patterns: vec!["*".to_string()],
    };
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, "{}").unwrap();

    let (watcher, _channel) = DebouncedFileWatcher::new(&path, config).unwrap();
    assert!(watcher.is_recursive());
}

// ============================================================
// Integration: ConfigValidator integration
// ============================================================

#[test]
fn validator_rejects_missing_version_field() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"key": "value"}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 1}),
        path,
        Arc::new(VersionValidator),
    )
    .unwrap();

    let result = config.try_update(serde_json::json!({"key": "value"}));
    assert!(matches!(result, Err(Error::ValidationFailed(_))));
}

#[test]
fn validator_accepts_valid_version_field() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 1}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 1}),
        path,
        Arc::new(VersionValidator),
    )
    .unwrap();

    let result = config.try_update(serde_json::json!({"version": 2}));
    assert!(result.is_ok());
}

#[test]
fn validator_min_version_rejects_below_threshold() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 5}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 5}),
        path,
        Arc::new(MinVersionValidator(3)),
    )
    .unwrap();

    let result = config.try_update(serde_json::json!({"version": 2}));
    assert!(matches!(result, Err(Error::ValidationFailed(msg)) if msg.contains("below minimum")));
}

#[test]
fn validator_min_version_accepts_at_threshold() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 3}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 3}),
        path,
        Arc::new(MinVersionValidator(3)),
    )
    .unwrap();

    let result = config.try_update(serde_json::json!({"version": 3}));
    assert!(result.is_ok());
}

#[test]
fn reload_from_file_uses_validator_for_file_content() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 1}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 1}),
        path,
        Arc::new(MinVersionValidator(5)),
    )
    .unwrap();

    fs::write(&path, r#"{"version": 3}"#).unwrap();
    let result = config.reload_from_file();
    assert!(matches!(result, Err(Error::ValidationFailed(_))));
    assert_eq!(config.current(), serde_json::json!({"version": 1}));
}

#[test]
fn full_lifecycle_update_commit_rollback_reload() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");
    fs::write(&path, r#"{"version": 1}"#).unwrap();

    let config = HotReloadConfig::new(
        serde_json::json!({"version": 1}),
        path,
        Arc::new(VersionValidator),
    )
    .unwrap();

    config
        .try_update(serde_json::json!({"version": 2}))
        .unwrap();
    assert_eq!(config.current(), serde_json::json!({"version": 1}));

    config.rollback();
    let result = config.commit();
    assert!(matches!(result, Err(Error::SwapFailed)));

    config
        .try_update(serde_json::json!({"version": 3}))
        .unwrap();
    let old = config.commit().unwrap();
    assert_eq!(old, serde_json::json!({"version": 1}));
    assert_eq!(config.current(), serde_json::json!({"version": 3}));

    fs::write(&path, r#"{"version": 4}"#).unwrap();
    let old2 = config.reload_from_file().unwrap();
    assert_eq!(old2, serde_json::json!({"version": 3}));
    assert_eq!(config.current(), serde_json::json!({"version": 4}));
}

// ============================================================
// Integration: Error Display tests
// ============================================================

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
