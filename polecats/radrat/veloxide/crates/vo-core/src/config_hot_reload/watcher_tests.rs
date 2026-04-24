#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::TempDir;

use super::*;

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
